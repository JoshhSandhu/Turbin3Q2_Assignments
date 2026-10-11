use anchor_lang::prelude::*;
use anchor_spl::token_2022::Token2022;

use super::extensions::{MintAccounts, BASE_EXTENSIONS};

#[derive(Accounts)]
pub struct InitializeMint<'info> {
    #[account(mut)]
    pub issuer: Signer<'info>,
    #[account(mut)]
    pub mint: Signer<'info>,
    pub token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> InitializeMint<'info> {
    pub fn initialize_mint(&self, decimals: u8, fee_basis_points: u16, maximum_fee: u64) -> Result<()> {
        let accounts = MintAccounts {
            issuer: self.issuer.to_account_info(),
            mint: self.mint.to_account_info(),
            token_program: self.token_program.to_account_info(),
            system_program: self.system_program.to_account_info(),
        };
        accounts.allocate(&BASE_EXTENSIONS)?;
        accounts.init_base_extensions(fee_basis_points, maximum_fee)?;
        accounts.finish(decimals)
    }
}
