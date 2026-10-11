use anchor_lang::prelude::*;

mod errors;
mod instructions;
mod state;
mod utils;
use instructions::*;

declare_id!("72Zim5YCQs4goV9mhSAJJwGL4ELgNjjmkKxWC3dSmgGZ");

#[program]
pub mod nft_staking_core {
    use super::*;

    pub fn create_collection(ctx: Context<CreateCollection>, name: String, uri: String) -> Result<()> {
        ctx.accounts.create_collection(name, uri, &ctx.bumps)
    }

    pub fn mint_nft(ctx: Context<Mint>, name: String, uri: String) -> Result<()> {
        ctx.accounts.mint_nft(name, uri, &ctx.bumps)
    }

    pub fn initialize_config(
        ctx: Context<InitConfig>,
        points_per_stake: u32,
        freeze_period: u8,
        burn_bonus: u64,
    ) -> Result<()> {
        ctx.accounts.init_config(points_per_stake, freeze_period, burn_bonus, &ctx.bumps)
    }

    pub fn stake(ctx: Context<Stake>) -> Result<()> {
        ctx.accounts.stake(&ctx.bumps)
    }

    pub fn unstake(ctx: Context<Unstake>) -> Result<()> {
        ctx.accounts.unstake(&ctx.bumps)
    }

    pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
        ctx.accounts.claim_rewards(&ctx.bumps)
    }

    pub fn burn_staked_nft(ctx: Context<BurnStakedNft>) -> Result<()> {
        ctx.accounts.burn_staked_nft(&ctx.bumps)
    }
}
