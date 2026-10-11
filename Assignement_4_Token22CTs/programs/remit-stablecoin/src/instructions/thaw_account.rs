use anchor_lang::prelude::*;
use anchor_spl::token_2022::{
    spl_token_2022, thaw_account as thaw_token_account, ThawAccount as ThawTokenAccount, Token2022,
};
use anchor_spl::token_interface::{Mint, TokenAccount};
use spl_token_2022::extension::StateWithExtensions;
use spl_token_2022::state::{Account as TokenAccountState, AccountState, Mint as MintState};

use crate::errors::StablecoinError;

#[derive(Accounts)]
pub struct ThawAccount<'info> {
    pub freeze_authority: Signer<'info>,
    #[account(mint::token_program = token_program)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        token::mint = mint,
        token::token_program = token_program,
    )]
    pub token_account: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Program<'info, Token2022>,
}

impl<'info> ThawAccount<'info> {
    pub fn thaw_account(&self) -> Result<()> {
        let (freeze_authority, frozen) = {
            let mint_info = self.mint.to_account_info();
            let mint_data = mint_info.try_borrow_data()?;
            let mint = StateWithExtensions::<MintState>::unpack(&mint_data)?;

            let account_info = self.token_account.to_account_info();
            let account_data = account_info.try_borrow_data()?;
            let account = StateWithExtensions::<TokenAccountState>::unpack(&account_data)?;

            (mint.base.freeze_authority, account.base.state == AccountState::Frozen)
        };
        require!(
            freeze_authority == Some(self.freeze_authority.key()).into(),
            StablecoinError::NotFreezeAuthority
        );
        require!(frozen, StablecoinError::AccountNotFrozen);

        thaw_token_account(CpiContext::new(
            self.token_program.key(),
            ThawTokenAccount {
                account: self.token_account.to_account_info(),
                mint: self.mint.to_account_info(),
                authority: self.freeze_authority.to_account_info(),
            },
        ))
    }
}
