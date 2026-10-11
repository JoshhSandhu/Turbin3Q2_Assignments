use anchor_lang::prelude::*;
use anchor_spl::token_2022::{spl_token_2022, Token2022};
use anchor_spl::token_2022_extensions::{transfer_checked_with_fee, TransferCheckedWithFee};
use anchor_spl::token_interface::{Mint, TokenAccount};
use spl_token_2022::extension::transfer_fee::TransferFeeConfig;
use spl_token_2022::extension::{BaseStateWithExtensions, StateWithExtensions};
use spl_token_2022::state::Mint as MintState;

use crate::errors::StablecoinError;

#[derive(Accounts)]
pub struct TransferWithFee<'info> {
    pub owner: Signer<'info>,
    #[account(mint::token_program = token_program)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        token::mint = mint,
        token::authority = owner,
        token::token_program = token_program,
    )]
    pub source: InterfaceAccount<'info, TokenAccount>,
    #[account(
        mut,
        token::mint = mint,
        token::token_program = token_program,
    )]
    pub destination: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Program<'info, Token2022>,
}

impl<'info> TransferWithFee<'info> {
    pub fn transfer_with_fee(&self, amount: u64) -> Result<()> {
        // The typed mint drops the extension data, so read the raw bytes. The fee is worked out
        // for the current epoch on every call because the fee authority can schedule a new rate.
        let (fee, decimals) = {
            let mint_info = self.mint.to_account_info();
            let data = mint_info.try_borrow_data()?;
            let mint = StateWithExtensions::<MintState>::unpack(&data)?;
            let fee = mint
                .get_extension::<TransferFeeConfig>()?
                .calculate_epoch_fee(Clock::get()?.epoch, amount)
                .ok_or(StablecoinError::FeeCalculation)?;
            (fee, mint.base.decimals)
        };

        transfer_checked_with_fee(
            CpiContext::new(
                self.token_program.key(),
                TransferCheckedWithFee {
                    token_program_id: self.token_program.to_account_info(),
                    source: self.source.to_account_info(),
                    mint: self.mint.to_account_info(),
                    destination: self.destination.to_account_info(),
                    authority: self.owner.to_account_info(),
                },
            ),
            amount,
            decimals,
            fee,
        )
    }
}
