use anchor_lang::prelude::*;
use anchor_spl::{associated_token::AssociatedToken, token_interface::{Mint, TokenAccount, TokenInterface}};
use mpl_core::{
    ID as MPL_CORE_ID,
    accounts::{BaseAssetV1, BaseCollectionV1},
    instructions::UpdatePluginV1CpiBuilder,
    types::{Attributes, FreezeDelegate, Plugin, UpdateAuthority},
};

use crate::errors::StakingError;
use crate::state::Config;
use crate::utils::{
    full_days, get_timestamp, last_claimed_at, mint_rewards, reward_for, set_attribute, staked_attributes,
    update_total_staked, LAST_CLAIMED_AT, STAKED, STAKED_AT,
};

#[derive(Accounts)]
pub struct Unstake<'info> {
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
impl<'info> Unstake<'info> {
    pub fn unstake(&mut self, bumps: &UnstakeBumps) -> Result<()> {
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
        let mut attribute_list = staked_attributes(&nft)?;

        let staked_days = full_days(get_timestamp(&attribute_list, STAKED_AT)?, now)?;
        require!(staked_days > 0, StakingError::FreezePeriodNotElapsed);
        require!(staked_days >= self.config.freeze_period as i64, StakingError::FreezePeriodNotElapsed);

        // Days already paid by claim_rewards are skipped.
        let unclaimed_days = full_days(last_claimed_at(&attribute_list)?, now)?;
        let amount = reward_for(unclaimed_days, self.config.points_per_stake)?;

        set_attribute(&mut attribute_list, STAKED, "false");
        set_attribute(&mut attribute_list, STAKED_AT, "0");
        set_attribute(&mut attribute_list, LAST_CLAIMED_AT, "0");

        UpdatePluginV1CpiBuilder::new(&mpl_core)
            .asset(&nft)
            .collection(Some(&collection))
            .payer(&user)
            .authority(Some(&update_authority))
            .system_program(&system_program)
            .plugin(Plugin::Attributes(Attributes { attribute_list }))
            .invoke_signed(&[signer_seeds])?;

        UpdatePluginV1CpiBuilder::new(&mpl_core)
            .asset(&nft)
            .collection(Some(&collection))
            .payer(&user)
            .authority(Some(&update_authority))
            .system_program(&system_program)
            .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: false }))
            .invoke_signed(&[signer_seeds])?;

        mint_rewards(&self.token_program, &self.rewards_mint, &self.user_rewards_ata, &self.config, collection_key, amount)?;

        update_total_staked(&mpl_core, &collection, &user, &update_authority, &system_program, signer_seeds, false)
    }
}
