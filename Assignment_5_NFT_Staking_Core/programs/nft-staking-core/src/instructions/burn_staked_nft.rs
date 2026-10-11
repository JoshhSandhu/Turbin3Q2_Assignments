use anchor_lang::prelude::*;
use anchor_spl::{associated_token::AssociatedToken, token_interface::{Mint, TokenAccount, TokenInterface}};
use mpl_core::{
    ID as MPL_CORE_ID,
    accounts::{BaseAssetV1, BaseCollectionV1},
    instructions::{BurnV1CpiBuilder, UpdatePluginV1CpiBuilder},
    types::{FreezeDelegate, Plugin, UpdateAuthority},
};

use crate::errors::StakingError;
use crate::state::Config;
use crate::utils::{full_days, last_claimed_at, mint_rewards, reward_for, staked_attributes, update_total_staked};

#[derive(Accounts)]
pub struct BurnStakedNft<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    /// CHECK: PDA Update authority, also the BurnDelegate authority
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
    #[account(
        mut,
        seeds = [b"rewards", config.key().as_ref()],
        bump = config.rewards_bump
    )]
    pub rewards_mint: InterfaceAccount<'info, Mint>,
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = rewards_mint,
        associated_token::authority = user,
    )]
    pub user_rewards_ata: InterfaceAccount<'info, TokenAccount>,
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
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}
impl<'info> BurnStakedNft<'info> {
    pub fn burn_staked_nft(&mut self, bumps: &BurnStakedNftBumps) -> Result<()> {
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

        let now = Clock::get()?.unix_timestamp;
        let attribute_list = staked_attributes(&nft)?;
        let earned = reward_for(full_days(last_claimed_at(&attribute_list)?, now)?, self.config.points_per_stake)?;
        let amount = earned.checked_add(self.config.burn_bonus).ok_or(StakingError::Overflow)?;

        // Core refuses to burn a frozen asset, so thaw it first.
        UpdatePluginV1CpiBuilder::new(&mpl_core)
            .asset(&nft)
            .collection(Some(&collection))
            .payer(&user)
            .authority(Some(&update_authority))
            .system_program(&system_program)
            .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: false }))
            .invoke_signed(&[signer_seeds])?;

        BurnV1CpiBuilder::new(&mpl_core)
            .asset(&nft)
            .collection(Some(&collection))
            .payer(&user)
            .authority(Some(&update_authority))
            .system_program(Some(&system_program))
            .invoke_signed(&[signer_seeds])?;

        mint_rewards(&self.token_program, &self.rewards_mint, &self.user_rewards_ata, &self.config, collection_key, amount)?;

        update_total_staked(&mpl_core, &collection, &user, &update_authority, &system_program, signer_seeds, false)
    }
}
