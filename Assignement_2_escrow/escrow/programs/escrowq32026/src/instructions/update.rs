use anchor_lang::prelude::*;

use crate::error::ErrorCode;
use crate::{state::Escrow, ESCROW_SEED};

#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,
    #[account(
        mut,
        has_one = maker,
        seeds = [ESCROW_SEED, maker.key().as_ref(), escrow.seed.to_le_bytes().as_ref()],
        bump = escrow.bump,
        constraint = Clock::get()?.unix_timestamp <= escrow.expiration @ ErrorCode::EscrowExpired,
    )]
    pub escrow: Account<'info, Escrow>,
}

impl<'info> Update<'info> {
    pub fn update(&mut self, receive: u64, expiration: Option<i64>) -> Result<()> {
        self.escrow.receive = receive;

        if let Some(expiration) = expiration {
            require!(
                expiration >= Clock::get()?.unix_timestamp,
                ErrorCode::EscrowExpired
            );
            self.escrow.expiration = expiration;
        }

        Ok(())
    }
}
