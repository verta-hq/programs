use crate::constants::*;
use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct PlatformConfig {
    pub admin: Pubkey,
    pub platform_wallet: Pubkey, // Dedicated wallet for platform fees
    pub revenue_share_collection: Pubkey,
    pub paused: bool,
    pub price_sol: u64,    // Global price in lamports
    pub creator_bps: u16,  // Default split if needed (BPS)
    pub platform_bps: u16, // Default split if needed (BPS)
    pub bump: u8,
}

#[account]
pub struct TitleConfig {
    pub title_id: String,
    pub authority: Pubkey,
    pub creator_wallet: Pubkey,
    pub creator_bps: u16,  // Per-series royalty split (set by Admin)
    pub platform_bps: u16, // Per-series platform share (set by Admin)
    pub active: bool,
    pub bump: u8,
}

impl TitleConfig {
    pub const MAX_SIZE: usize = 8 + (4 + MAX_TITLE_ID_LEN) + 32 + 32 + 2 + 2 + 1 + 1;
}

#[account]
#[derive(InitSpace)]
pub struct UserVault {
    pub user: Pubkey,
    pub delegate: Pubkey,
    pub auto_play_enabled: bool,
    pub total_spent: u64,
    pub bump: u8,
}

#[account]
pub struct EpisodeUnlockReceipt {
    pub series: Pubkey, // Reference to the TitleConfig
    pub user: Pubkey,
    pub episode_id: String,
    pub paid_amount: u64, // Historical price paid
    pub unlocked_at: i64,
    pub bump: u8,
}

impl EpisodeUnlockReceipt {
    pub const MAX_SIZE: usize = 8 + 32 + 32 + (4 + MAX_EPISODE_ID_LEN) + 8 + 8 + 1;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum CrowdfundStatus {
    Active,
    Succeeded,
    Refunded,
    Cancelled,
}

#[account]
#[derive(InitSpace)]
pub struct Crowdfund {
    #[max_len(32)]
    pub series_id: String, // Matches TitleConfig title_id
    pub goal: u64,         // micro-USDC (6 dp)
    pub total_raised: u64, // micro-USDC
    pub deadline: i64,
    pub production_wallet: Pubkey,
    pub backer_revenue_bps: u16, // Share of revenue reserved for backers (BPS)
    pub status: CrowdfundStatus,
    pub bump: u8,
    // Revenue accumulator (frozen denominators set at finalize; SCALE-scaled)
    pub cumulative_revenue_per_bps: u128, // micro-USDC × SCALE / bps
    pub total_issued_bps: u16,
    pub accumulator_at_finalize: u128,
    pub collection: Pubkey, // Global "Verta Revenue Shares" Core collection
    pub mint: Pubkey,       // Settlement token (USDC); escrow + revenue vaults hold this
}

#[account]
#[derive(InitSpace)]
pub struct Contribution {
    pub user: Pubkey,
    pub amount_contributed: u64,
    pub minted: bool,
    pub bump: u8,
}

/// Accounting source of truth for one Revenue Share Pass.
/// Written once at mint; share_bps immutable thereafter. The Core asset's
/// attributes mirror share_bps for display only and are never trusted for money.
#[account]
#[derive(InitSpace)]
pub struct SharePosition {
    pub crowdfund: Pubkey,
    pub asset: Pubkey,
    pub share_bps: u16,
    pub claimed_checkpoint: u128,
    pub bump: u8,
}

/// Daily coin-economy checkpoint — the public audit anchor for the
/// DB-authoritative coin ledger (coins are a closed-loop credit sold via
/// store IAP; see verta/docs/v-currency-architecture-review.md). One PDA per
/// UTC day; the backend's checkpoint sweep writes it after the nightly
/// ledger reconciliation passes. Re-recording the same day overwrites —
/// idempotent job re-runs, admin-gated.
#[account]
#[derive(InitSpace)]
pub struct CreditEpoch {
    pub epoch_day: u32, // days since Unix epoch, UTC
    pub coins_sold: u64,
    pub coins_spent: u64,
    pub coins_refunded: u64,
    pub usd_liability_micro: u64, // outstanding coins x face value (micro-USD)
    pub balances_root: [u8; 32],  // merkle root of (wallet, balance); zeroed until shipped
    pub recorded_at: i64,
    pub bump: u8,
}
