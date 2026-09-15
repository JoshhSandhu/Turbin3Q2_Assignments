use anchor_lang::prelude::*;

use crate::{error::AmmError, state::Config};

#[derive(Accounts)]
pub struct Update<'info> {
    pub user: Signer<'info>,
    #[account(
        mut,
        seeds = [b"config", config.seed.to_le_bytes().as_ref()],
        bump = config.config_bump,
    )]
    pub config: Account<'info, Config>,
}

impl<'info> Update<'info> {
    pub fn lock(&mut self) -> Result<()> {
        self.check_authority()?;
        self.config.locked = true;
        Ok(())
    }

    pub fn unlock(&mut self) -> Result<()> {
        self.check_authority()?;
        self.config.locked = false;
        Ok(())
    }

    fn check_authority(&self) -> Result<()> {
        let authority = self.config.authority.ok_or(AmmError::NoAuthoritySet)?;
        require_keys_eq!(authority, self.user.key(), AmmError::InvalidAuthority);
        Ok(())
    }
}
