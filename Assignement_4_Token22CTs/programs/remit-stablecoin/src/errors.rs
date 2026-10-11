use anchor_lang::error_code;

#[error_code]
pub enum StablecoinError {
    #[msg("Transfer fee basis points must be at most 10,000")]
    FeeTooHigh,
    #[msg("Could not calculate the transfer fee for this epoch")]
    FeeCalculation,
    #[msg("Signer is not the mint's freeze authority")]
    NotFreezeAuthority,
    #[msg("Token account is not frozen")]
    AccountNotFrozen,
}
