use crate::constants::*;
use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct PlatformConfig {
    pub admin: Pubkey,
    pub pending_admin: Pubkey,
    pub operator: Pubkey,
    pub settlement_mint: Pubkey,
    pub revenue_share_collection: Pubkey,
    pub paused: bool,
    pub bump: u8,
}

#[account]
pub struct TitleConfig {
    pub title_id: String,
    pub creator_wallet: Pubkey,
    pub creator_bps: u16,
    pub platform_bps: u16,
    pub active: bool,
    pub bump: u8,
}

impl TitleConfig {
    pub const MAX_SIZE: usize = 8 + (4 + MAX_TITLE_ID_LEN) + 32 + 2 + 2 + 1 + 1;
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
    pub series_id: String,
    pub goal: u64,
    pub total_raised: u64,
    pub deadline: i64,
    pub production_wallet: Pubkey,
    pub backer_revenue_bps: u16,
    pub status: CrowdfundStatus,
    pub bump: u8,
    pub cumulative_revenue_per_bps: u128,
    pub total_issued_bps: u16,
    pub accumulator_at_finalize: u128,
    pub collection: Pubkey,
    pub mint: Pubkey,
}

#[account]
#[derive(InitSpace)]
pub struct Contribution {
    pub user: Pubkey,
    pub rent_payer: Pubkey,
    pub amount_contributed: u64,
    pub minted: bool,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct SharePosition {
    pub crowdfund: Pubkey,
    pub asset: Pubkey,
    pub share_bps: u16,
    pub claimed_checkpoint: u128,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct CreditEpoch {
    pub epoch_day: u32,
    pub coins_sold: u64,
    pub coins_spent: u64,
    pub coins_refunded: u64,
    pub usd_liability_micro: u64,
    pub balances_root: [u8; 32],
    pub recorded_at: i64,
    pub bump: u8,
}
