use {
    crate::Pool,
    anchor_lang::{
        solana_program::instruction::Instruction, system_program::ID as SYSTEM_PROGRAM_ID,
        InstructionData, ToAccountMetas,
    },
    anchor_spl::associated_token::ID as ASSOCIATED_TOKEN_PROGRAM_ID,
    litesvm_token::spl_token::ID as TOKEN_PROGRAM_ID,
    solana_pubkey::Pubkey,
};

pub fn create_initialise_ix(
    initializer: Pubkey,
    pool: &Pool,
    fee: u16,
    protocol_fee: u16,
    authority: Option<Pubkey>,
) -> Instruction {
    Instruction::new_with_bytes(
        amm_video::id(),
        &amm_video::instruction::Initialize {
            seed: pool.seed,
            fee,
            protocol_fee,
            authority,
        }
        .data(),
        amm_video::accounts::Initialize {
            initializer,
            mint_x: pool.mint_x,
            mint_y: pool.mint_y,
            treasury: pool.treasury,
            mint_lp: pool.mint_lp,
            vault_x: pool.vault_x,
            vault_y: pool.vault_y,
            config: pool.config,
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
    )
}
