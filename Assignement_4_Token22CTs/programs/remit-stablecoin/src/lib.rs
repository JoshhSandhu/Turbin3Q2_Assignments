use anchor_lang::prelude::*;

pub mod errors;
pub mod instructions;

pub use instructions::*;

declare_id!("GBKGGvYbHe2Eq2DCkupKeNNLibNcmtGbDfRsoVdpsBoa");

#[program]
pub mod remit_stablecoin {
    use super::*;

    pub fn initialize_mint(
        ctx: Context<InitializeMint>,
        decimals: u8,
        fee_basis_points: u16,
        maximum_fee: u64,
    ) -> Result<()> {
        ctx.accounts.initialize_mint(decimals, fee_basis_points, maximum_fee)
    }

    pub fn transfer_with_fee(ctx: Context<TransferWithFee>, amount: u64) -> Result<()> {
        ctx.accounts.transfer_with_fee(amount)
    }

    pub fn thaw_account(ctx: Context<ThawAccount>) -> Result<()> {
        ctx.accounts.thaw_account()
    }

    pub fn initialize_mint_confidential(
        ctx: Context<InitializeMintConfidential>,
        decimals: u8,
        fee_basis_points: u16,
        maximum_fee: u64,
        withdraw_withheld_elgamal_pubkey: [u8; 32],
    ) -> Result<()> {
        ctx.accounts.initialize_mint_confidential(
            decimals,
            fee_basis_points,
            maximum_fee,
            withdraw_withheld_elgamal_pubkey,
        )
    }
}
