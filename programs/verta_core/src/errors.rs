use anchor_lang::prelude::*;

#[error_code]
pub enum VertaError {
    #[msg("You are not authorized to perform this action.")]
    Unauthorized,
    #[msg("Basis points must sum to 10,000.")]
    InvalidBasisPoints,
    #[msg("Invalid title ID length.")]
    InvalidTitleId,
    #[msg("Amount must be greater than zero.")]
    InvalidAmount,
    #[msg("Invalid recipient address.")]
    InvalidRecipient,
    #[msg("The platform is currently paused.")]
    PlatformPaused,
    #[msg("This title is currently inactive.")]
    TitleInactive,
    #[msg("A math overflow occurred.")]
    MathOverflow,
    #[msg("The crowdfunding campaign is not active.")]
    CampaignInactive,
    #[msg("The crowdfunding campaign has ended.")]
    CampaignEnded,
    #[msg("The crowdfunding campaign has not ended yet.")]
    CampaignNotEnded,
    #[msg("The funding goal has not been reached.")]
    GoalNotReached,
    #[msg("There is no revenue available to claim.")]
    NoRevenueToClaim,
    #[msg("The provided account is not a valid Metaplex Core asset.")]
    InvalidAsset,
    #[msg("The signer does not own this asset.")]
    NotOwner,
    #[msg("The asset does not belong to the expected collection.")]
    WrongCollection,
    #[msg("The funding goal has already been reached.")]
    GoalAlreadyReached,
    #[msg("A Revenue Share Pass has already been minted for this contribution.")]
    AlreadyMinted,
    #[msg("The campaign has been cancelled.")]
    CampaignCancelled,
    #[msg("The campaign has not been finalized.")]
    NotFinalized,
    #[msg("The vault does not hold enough funds for this payout.")]
    InsufficientVaultBalance,
    #[msg("Token account mint does not match the campaign's settlement mint.")]
    InvalidMint,
    #[msg("Mint is not the platform settlement mint.")]
    InvalidSettlementMint,
    #[msg("Deadline must be in the future.")]
    InvalidDeadline,
    #[msg("There is no pending admin to accept.")]
    NoPendingAdmin,
    #[msg("Token transfer did not move the expected amount.")]
    TransferAmountMismatch,
    #[msg("The campaign deadline has passed.")]
    CampaignDeadlinePassed,
    #[msg("Signer is not the program upgrade authority.")]
    NotUpgradeAuthority,
}
