use {
    crate::Pool,
    anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas},
    solana_pubkey::Pubkey,
};

pub fn create_lock_ix(user: Pubkey, pool: &Pool) -> Instruction {
    Instruction::new_with_bytes(
        amm_video::id(),
        &amm_video::instruction::Lock {}.data(),
        amm_video::accounts::Update {
            user,
            config: pool.config,
        }
        .to_account_metas(None),
    )
}

pub fn create_unlock_ix(user: Pubkey, pool: &Pool) -> Instruction {
    Instruction::new_with_bytes(
        amm_video::id(),
        &amm_video::instruction::Unlock {}.data(),
        amm_video::accounts::Update {
            user,
            config: pool.config,
        }
        .to_account_metas(None),
    )
}
