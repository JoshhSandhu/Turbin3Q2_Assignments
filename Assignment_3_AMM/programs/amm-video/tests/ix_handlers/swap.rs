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

pub fn create_swap_ix(
    user: Pubkey,
    pool: &Pool,
    is_x: bool,
    amount_in: u64,
    min_amount_out: u64,
) -> Instruction {
    Instruction::new_with_bytes(
        amm_video::id(),
        &amm_video::instruction::Swap {
            is_x,
            amount_in,
            min_amount_out,
        }
        .data(),
        amm_video::accounts::Swap {
            user,
            mint_x: pool.mint_x,
            mint_y: pool.mint_y,
            config: pool.config,
            mint_lp: pool.mint_lp,
            vault_x: pool.vault_x,
            vault_y: pool.vault_y,
            user_x: associated_token::get_associated_token_address(&user, &pool.mint_x),
            user_y: associated_token::get_associated_token_address(&user, &pool.mint_y),
            treasury: pool.treasury,
            treasury_x: associated_token::get_associated_token_address(
                &pool.treasury,
                &pool.mint_x,
            ),
            treasury_y: associated_token::get_associated_token_address(
                &pool.treasury,
                &pool.mint_y,
            ),
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
    )
}
