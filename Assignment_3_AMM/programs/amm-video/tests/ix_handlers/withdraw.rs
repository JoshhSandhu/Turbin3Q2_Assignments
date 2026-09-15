use {
    crate::Pool,
    anchor_lang::{
        solana_program::instruction::Instruction, system_program::ID as SYSTEM_PROGRAM_ID,
        InstructionData, ToAccountMetas,
    },
    anchor_spl::associated_token::{self, ID as ASSOCIATED_TOKEN_PROGRAM_ID},
    litesvm_token::spl_token::ID as TOKEN_PROGRAM_ID,
    solana_pubkey::Pubkey,
};

pub fn create_withdraw_ix(
    user: Pubkey,
    pool: &Pool,
    amount: u64,
    min_x: u64,
    min_y: u64,
) -> Instruction {
    Instruction::new_with_bytes(
        amm_video::id(),
        &amm_video::instruction::Withdraw {
            amount,
            min_x,
            min_y,
        }
        .data(),
        amm_video::accounts::Withdraw {
            user,
            mint_x: pool.mint_x,
            mint_y: pool.mint_y,
            config: pool.config,
            mint_lp: pool.mint_lp,
            vault_x: pool.vault_x,
            vault_y: pool.vault_y,
            user_x: associated_token::get_associated_token_address(&user, &pool.mint_x),
            user_y: associated_token::get_associated_token_address(&user, &pool.mint_y),
            user_lp: associated_token::get_associated_token_address(&user, &pool.mint_lp),
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
    )
}
