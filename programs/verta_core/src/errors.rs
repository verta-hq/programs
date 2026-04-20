use anchor_lang::prelude::*;

#[error_code]
pub enum VertaError {
    #[msg("You are not authorized to perform this action.")]
    Unauthorized,
    #[msg("Basis points must sum to 10,000.")]
    InvalidBasisPoints,
    #[msg("Invalid title ID length.")]
    InvalidTitleId,
    #[msg("Invalid episode ID length.")]
    InvalidEpisodeId,
    #[msg("Price must be greater than zero.")]
    InvalidPrice,
    #[msg("Invalid recipient address.")]
    InvalidRecipient,
    #[msg("The platform is currently paused.")]
    PlatformPaused,
    #[msg("This title is currently inactive.")]
    TitleInactive,
    #[msg("A math overflow occurred.")]
    MathOverflow,
    #[msg("Automatic playback is not enabled for this user.")]
    AutoPlayDisabled,
    #[msg("The vault does not have enough funds.")]
    InsufficientVaultFunds,
    #[msg("Only the designated delegate can perform this action.")]
    OnlyDelegate,
}
