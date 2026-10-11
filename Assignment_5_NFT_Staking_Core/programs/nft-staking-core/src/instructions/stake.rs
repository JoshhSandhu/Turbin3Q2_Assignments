use anchor_lang::prelude::*;
use mpl_core::{
    ID as MPL_CORE_ID,
    accounts::{BaseAssetV1, BaseCollectionV1},
    fetch_plugin,
    instructions::{AddPluginV1CpiBuilder, UpdatePluginV1CpiBuilder},
    types::{Attributes, BurnDelegate, FreezeDelegate, Plugin, PluginAuthority, PluginType, UpdateAuthority},
};

use crate::errors::StakingError;
use crate::state::Config;
use crate::utils::{attribute, get_attribute, set_attribute, update_total_staked, LAST_CLAIMED_AT, STAKED, STAKED_AT};

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    /// CHECK: PDA Update authority
    #[account(
        seeds = [b"update_authority", collection.key().as_ref()],
        bump
    )]
    pub update_authority: UncheckedAccount<'info>,
    #[account(
        seeds = [b"config", collection.key().as_ref()],
        bump = config.config_bump
    )]
    pub config: Account<'info, Config>,
    /// CHECK: NFT account will be checked by the mpl core program
    #[account(mut)]
    pub nft: UncheckedAccount<'info>,
    /// CHECK: Collection account will be checked by the mpl core program
    #[account(mut)]
    pub collection: UncheckedAccount<'info>,
    /// CHECK: This is the ID of the Metaplex Core program
    #[account(address = MPL_CORE_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}
impl<'info> Stake<'info> {
    pub fn stake(&mut self, bumps: &StakeBumps) -> Result<()> {
        let nft = self.nft.to_account_info();
        let collection = self.collection.to_account_info();
        let user = self.user.to_account_info();
        let update_authority = self.update_authority.to_account_info();
        let system_program = self.system_program.to_account_info();
        let mpl_core = self.mpl_core_program.to_account_info();

        let base_asset = BaseAssetV1::try_from(&nft)?;
        require!(base_asset.owner == user.key(), StakingError::InvalidOwner);
        require!(base_asset.update_authority == UpdateAuthority::Collection(collection.key()), StakingError::InvalidAuthority);
        let base_collection = BaseCollectionV1::try_from(&collection)?;
        require!(base_collection.update_authority == update_authority.key(), StakingError::InvalidAuthority);

        let collection_key = collection.key();
        let signer_seeds = &[
            b"update_authority",
            collection_key.as_ref(),
            &[bumps.update_authority],
        ];

        let now = Clock::get()?.unix_timestamp.to_string();

        match fetch_plugin::<BaseAssetV1, Attributes>(&nft, PluginType::Attributes) {
            Err(_) => {
                AddPluginV1CpiBuilder::new(&mpl_core)
                    .asset(&nft)
                    .collection(Some(&collection))
                    .payer(&user)
                    .authority(Some(&update_authority))
                    .system_program(&system_program)
                    .plugin(Plugin::Attributes(Attributes {
                        attribute_list: vec![
                            attribute(STAKED, "true"),
                            attribute(STAKED_AT, &now),
                            attribute(LAST_CLAIMED_AT, &now),
                        ],
                    }))
                    .init_authority(PluginAuthority::UpdateAuthority)
                    .invoke_signed(&[signer_seeds])?;
            }
            Ok((_, attributes, _)) => {
                let mut attribute_list = attributes.attribute_list;
                require!(get_attribute(&attribute_list, STAKED) != Some("true"), StakingError::AlreadyStaked);
                set_attribute(&mut attribute_list, STAKED, "true");
                set_attribute(&mut attribute_list, STAKED_AT, &now);
                set_attribute(&mut attribute_list, LAST_CLAIMED_AT, &now);

                UpdatePluginV1CpiBuilder::new(&mpl_core)
                    .asset(&nft)
                    .collection(Some(&collection))
                    .payer(&user)
                    .authority(Some(&update_authority))
                    .system_program(&system_program)
                    .plugin(Plugin::Attributes(Attributes { attribute_list }))
                    .invoke_signed(&[signer_seeds])?;
            }
        }

        // Owner-managed, so it can only be added here while the owner signs. Needed later by burn_staked_nft.
        if fetch_plugin::<BaseAssetV1, BurnDelegate>(&nft, PluginType::BurnDelegate).is_err() {
            AddPluginV1CpiBuilder::new(&mpl_core)
                .asset(&nft)
                .collection(Some(&collection))
                .payer(&user)
                .authority(Some(&user))
                .system_program(&system_program)
                .plugin(Plugin::BurnDelegate(BurnDelegate {}))
                .init_authority(PluginAuthority::UpdateAuthority)
                .invoke()?;
        }

        match fetch_plugin::<BaseAssetV1, FreezeDelegate>(&nft, PluginType::FreezeDelegate) {
            Err(_) => {
                AddPluginV1CpiBuilder::new(&mpl_core)
                    .asset(&nft)
                    .collection(Some(&collection))
                    .payer(&user)
                    .authority(Some(&user))
                    .system_program(&system_program)
                    .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: true }))
                    .init_authority(PluginAuthority::UpdateAuthority)
                    .invoke()?;
            }
            Ok(_) => {
                UpdatePluginV1CpiBuilder::new(&mpl_core)
                    .asset(&nft)
                    .collection(Some(&collection))
                    .payer(&user)
                    .authority(Some(&update_authority))
                    .system_program(&system_program)
                    .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: true }))
                    .invoke_signed(&[signer_seeds])?;
            }
        }

        update_total_staked(&mpl_core, &collection, &user, &update_authority, &system_program, signer_seeds, true)
    }
}
