use crate::constants::*;
use crate::errors::*;
use crate::state::*;
use anchor_lang::prelude::*;

/// Records (or re-records) the coin-economy checkpoint for one UTC day.
/// Admin-gated via PlatformConfig, same pattern as update_platform_config.
/// init_if_needed makes the backend sweep idempotent: a re-run for the same
/// day overwrites the PDA instead of failing.
pub fn record_credit_epoch(
    ctx: Context<RecordCreditEpoch>,
    epoch_day: u32,
    coins_sold: u64,
    coins_spent: u64,
    coins_refunded: u64,
    usd_liability_micro: u64,
    balances_root: [u8; 32],
) -> Result<()> {
    let epoch = &mut ctx.accounts.credit_epoch;
    epoch.epoch_day = epoch_day;
    epoch.coins_sold = coins_sold;
    epoch.coins_spent = coins_spent;
    epoch.coins_refunded = coins_refunded;
    epoch.usd_liability_micro = usd_liability_micro;
    epoch.balances_root = balances_root;
    epoch.recorded_at = Clock::get()?.unix_timestamp;
    epoch.bump = ctx.bumps.credit_epoch;
    Ok(())
}

#[derive(Accounts)]
#[instruction(epoch_day: u32)]
pub struct RecordCreditEpoch<'info> {
    #[account(
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_config.bump,
        has_one = admin @ VertaError::Unauthorized
    )]
    pub platform_config: Account<'info, PlatformConfig>,
    pub admin: Signer<'info>,
    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + CreditEpoch::INIT_SPACE,
        seeds = [CREDIT_EPOCH_SEED, &epoch_day.to_le_bytes()],
        bump
    )]
    pub credit_epoch: Account<'info, CreditEpoch>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
