use anchor_lang::prelude::*;
use anchor_lang::system_program::{create_account, CreateAccount};
use anchor_spl::token_2022::{initialize_mint2, spl_token_2022, InitializeMint2};
use anchor_spl::token_2022_extensions::{
    default_account_state_initialize, metadata_pointer_initialize, mint_close_authority_initialize,
    transfer_fee_initialize, DefaultAccountStateInitialize, MetadataPointerInitialize,
    MintCloseAuthorityInitialize, TransferFeeInitialize,
};
use spl_token_2022::extension::ExtensionType;
use spl_token_2022::state::{AccountState, Mint as MintState};

use crate::errors::StablecoinError;

pub const MAX_FEE_BASIS_POINTS: u16 = 10_000;

pub const BASE_EXTENSIONS: [ExtensionType; 4] = [
    ExtensionType::TransferFeeConfig,
    ExtensionType::MetadataPointer,
    ExtensionType::DefaultAccountState,
    ExtensionType::MintCloseAuthority,
];

pub struct MintAccounts<'info> {
    pub issuer: AccountInfo<'info>,
    pub mint: AccountInfo<'info>,
    pub token_program: AccountInfo<'info>,
    pub system_program: AccountInfo<'info>,
}

impl<'info> MintAccounts<'info> {
    // Every extension is initialized after the account exists and before InitializeMint2.
    pub fn allocate(&self, extensions: &[ExtensionType]) -> Result<()> {
        let space = ExtensionType::try_calculate_account_len::<MintState>(extensions)?;
        let lamports = Rent::get()?.minimum_balance(space);
        create_account(
            CpiContext::new(
                self.system_program.key(),
                CreateAccount {
                    from: self.issuer.clone(),
                    to: self.mint.clone(),
                },
            ),
            lamports,
            space as u64,
            &self.token_program.key(),
        )
    }

    pub fn init_base_extensions(&self, fee_basis_points: u16, maximum_fee: u64) -> Result<()> {
        require!(fee_basis_points <= MAX_FEE_BASIS_POINTS, StablecoinError::FeeTooHigh);
        let program = self.token_program.key();
        let issuer = self.issuer.key();

        transfer_fee_initialize(
            CpiContext::new(
                program,
                TransferFeeInitialize {
                    token_program_id: self.token_program.clone(),
                    mint: self.mint.clone(),
                },
            ),
            Some(&issuer),
            Some(&issuer),
            fee_basis_points,
            maximum_fee,
        )?;

        metadata_pointer_initialize(
            CpiContext::new(
                program,
                MetadataPointerInitialize {
                    token_program_id: self.token_program.clone(),
                    mint: self.mint.clone(),
                },
            ),
            Some(issuer),
            Some(self.mint.key()),
        )?;

        default_account_state_initialize(
            CpiContext::new(
                program,
                DefaultAccountStateInitialize {
                    token_program_id: self.token_program.clone(),
                    mint: self.mint.clone(),
                },
            ),
            &AccountState::Frozen,
        )?;

        mint_close_authority_initialize(
            CpiContext::new(
                program,
                MintCloseAuthorityInitialize {
                    token_program_id: self.token_program.clone(),
                    mint: self.mint.clone(),
                },
            ),
            Some(&issuer),
        )
    }

    // A default state of Frozen is rejected unless the mint has a freeze authority.
    pub fn finish(&self, decimals: u8) -> Result<()> {
        let issuer = self.issuer.key();
        initialize_mint2(
            CpiContext::new(
                self.token_program.key(),
                InitializeMint2 {
                    mint: self.mint.clone(),
                },
            ),
            decimals,
            &issuer,
            Some(&issuer),
        )
    }
}
