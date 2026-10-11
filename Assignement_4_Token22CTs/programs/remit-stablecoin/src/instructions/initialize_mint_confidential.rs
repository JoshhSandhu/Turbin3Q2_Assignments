use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::token_2022::{spl_token_2022, Token2022};
use anchor_spl::token_2022_extensions::{permanent_delegate_initialize, PermanentDelegateInitialize};
use spl_token_2022::extension::{confidential_transfer, confidential_transfer_fee, ExtensionType};
use spl_token_2022::solana_zk_sdk::encryption::pod::elgamal::PodElGamalPubkey;

use super::extensions::{MintAccounts, BASE_EXTENSIONS};

#[derive(Accounts)]
pub struct InitializeMintConfidential<'info> {
    #[account(mut)]
    pub issuer: Signer<'info>,
    #[account(mut)]
    pub mint: Signer<'info>,
    pub token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> InitializeMintConfidential<'info> {
    pub fn initialize_mint_confidential(
        &self,
        decimals: u8,
        fee_basis_points: u16,
        maximum_fee: u64,
        withdraw_withheld_elgamal_pubkey: [u8; 32],
    ) -> Result<()> {
        let accounts = MintAccounts {
            issuer: self.issuer.to_account_info(),
            mint: self.mint.to_account_info(),
            token_program: self.token_program.to_account_info(),
            system_program: self.system_program.to_account_info(),
        };
        let program = self.token_program.key();
        let mint = self.mint.key();
        let issuer = self.issuer.key();

        // With both a transfer fee and confidential transfers on the mint, Token-2022 also
        // requires ConfidentialTransferFeeConfig, or InitializeMint2 fails.
        let mut extensions = BASE_EXTENSIONS.to_vec();
        extensions.extend([
            ExtensionType::PermanentDelegate,
            ExtensionType::ConfidentialTransferMint,
            ExtensionType::ConfidentialTransferFeeConfig,
        ]);

        accounts.allocate(&extensions)?;
        accounts.init_base_extensions(fee_basis_points, maximum_fee)?;

        permanent_delegate_initialize(
            CpiContext::new(
                program,
                PermanentDelegateInitialize {
                    token_program_id: self.token_program.to_account_info(),
                    mint: self.mint.to_account_info(),
                },
            ),
            &issuer,
        )?;

        let confidential_ix =
            confidential_transfer::instruction::initialize_mint(&program, &mint, Some(issuer), false, None)?;
        invoke(
            &confidential_ix,
            &[self.mint.to_account_info(), self.token_program.to_account_info()],
        )?;

        let fee_ix = confidential_transfer_fee::instruction::initialize_confidential_transfer_fee_config(
            &program,
            &mint,
            Some(issuer),
            &PodElGamalPubkey::from(withdraw_withheld_elgamal_pubkey),
        )?;
        invoke(&fee_ix, &[self.mint.to_account_info(), self.token_program.to_account_info()])?;

        accounts.finish(decimals)
    }
}
