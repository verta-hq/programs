use crate::constants::*;
use crate::errors::*;
use crate::instructions::crowdfunding::record_revenue_from_source;
use crate::state::*;
use anchor_lang::prelude::*;

pub fn auto_unlock_episode(ctx: Context<AutoUnlockEpisode>, episode_id: String) -> Result<()> {
    let total_amount = ctx.accounts.platform_config.price_sol;
    let (creator_amount, platform_amount) =
        calculate_splits(total_amount, ctx.accounts.title_config.creator_bps)?;

    validate_unlock(
        &ctx.accounts.platform_config,
        &ctx.accounts.title_config,
        &ctx.accounts.user_vault,
        &ctx.accounts.delegate,
        &episode_id,
        total_amount,
    )?;

    route_payouts(
        &ctx.accounts.user_vault.to_account_info(),
        &ctx.accounts.creator_wallet.to_account_info(),
        &ctx.accounts.platform_wallet.to_account_info(),
        ctx.accounts.crowdfund.as_mut(),
        ctx.accounts
            .revenue_vault
            .as_ref()
            .map(|account| account.as_ref()),
        ctx.accounts
            .production_wallet
            .as_ref()
            .map(|account| account.as_ref()),
        &ctx.accounts.title_config.title_id,
        ctx.program_id,
        creator_amount,
        platform_amount,
    )?;

    let vault = &mut ctx.accounts.user_vault;
    vault.total_spent = vault
        .total_spent
        .checked_add(total_amount)
        .ok_or(VertaError::MathOverflow)?;

    let receipt = &mut ctx.accounts.unlock_receipt;
    receipt.series = ctx.accounts.title_config.key();
    receipt.user = vault.user;
    receipt.episode_id = episode_id;
    receipt.paid_amount = total_amount;
    receipt.unlocked_at = Clock::get()?.unix_timestamp;
    receipt.bump = ctx.bumps.unlock_receipt;

    Ok(())
}

/// New Frictionless Unlock: Payout only, no on-chain receipt account
pub fn direct_payout(ctx: Context<DirectPayout>, episode_id: String) -> Result<()> {
    let total_amount = ctx.accounts.platform_config.price_sol;
    let (creator_amount, platform_amount) =
        calculate_splits(total_amount, ctx.accounts.title_config.creator_bps)?;

    validate_unlock(
        &ctx.accounts.platform_config,
        &ctx.accounts.title_config,
        &ctx.accounts.user_vault,
        &ctx.accounts.delegate,
        &episode_id,
        total_amount,
    )?;

    route_payouts(
        &ctx.accounts.user_vault.to_account_info(),
        &ctx.accounts.creator_wallet.to_account_info(),
        &ctx.accounts.platform_wallet.to_account_info(),
        ctx.accounts.crowdfund.as_mut(),
        ctx.accounts
            .revenue_vault
            .as_ref()
            .map(|account| account.as_ref()),
        ctx.accounts
            .production_wallet
            .as_ref()
            .map(|account| account.as_ref()),
        &ctx.accounts.title_config.title_id,
        ctx.program_id,
        creator_amount,
        platform_amount,
    )?;

    let vault = &mut ctx.accounts.user_vault;
    vault.total_spent = vault
        .total_spent
        .checked_add(total_amount)
        .ok_or(VertaError::MathOverflow)?;

    Ok(())
}

// Helper functions to avoid duplication
fn calculate_splits(total_amount: u64, creator_bps: u16) -> Result<(u64, u64)> {
    let creator_amount = (total_amount as u128)
        .checked_mul(creator_bps as u128)
        .ok_or(VertaError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128)
        .ok_or(VertaError::MathOverflow)? as u64;

    let platform_amount = total_amount
        .checked_sub(creator_amount)
        .ok_or(VertaError::MathOverflow)?;
    Ok((creator_amount, platform_amount))
}

fn validate_unlock(
    platform_config: &Account<PlatformConfig>,
    title_config: &Account<TitleConfig>,
    vault: &Account<UserVault>,
    delegate: &Signer,
    episode_id: &String,
    total_amount: u64,
) -> Result<()> {
    require!(!platform_config.paused, VertaError::PlatformPaused);
    require!(title_config.active, VertaError::TitleInactive);
    require!(vault.auto_play_enabled, VertaError::AutoPlayDisabled);
    require!(delegate.key() == vault.delegate, VertaError::OnlyDelegate);
    require!(
        episode_id.len() > 0 && episode_id.len() <= MAX_EPISODE_ID_LEN,
        VertaError::InvalidEpisodeId
    );

    let vault_lamports = vault.to_account_info().lamports();
    require!(
        vault_lamports >= total_amount,
        VertaError::InsufficientVaultFunds
    );
    Ok(())
}

fn route_payouts<'info>(
    source: &AccountInfo<'info>,
    creator_wallet: &AccountInfo<'info>,
    platform_wallet: &AccountInfo<'info>,
    crowdfund: Option<&mut Account<'info, Crowdfund>>,
    revenue_vault: Option<&AccountInfo<'info>>,
    production_wallet: Option<&AccountInfo<'info>>,
    title_id: &String,
    program_id: &Pubkey,
    creator_amount: u64,
    platform_amount: u64,
) -> Result<()> {
    let expected_crowdfund_pda =
        Pubkey::find_program_address(&[b"crowdfund", title_id.as_bytes()], program_id).0;

    if creator_wallet.key() == expected_crowdfund_pda {
        let crowdfund = crowdfund.ok_or(VertaError::NotFinalized)?;
        let revenue_vault = revenue_vault.ok_or(VertaError::InvalidRecipient)?;
        let production_wallet = production_wallet.ok_or(VertaError::InvalidRecipient)?;
        require!(
            crowdfund.key() == expected_crowdfund_pda,
            VertaError::InvalidRecipient
        );

        record_revenue_from_source(
            crowdfund,
            revenue_vault,
            production_wallet,
            source,
            creator_amount,
        )?;
    } else {
        checked_lamport_transfer(source, creator_wallet, creator_amount)?;
    }

    checked_lamport_transfer(source, platform_wallet, platform_amount)?;
    Ok(())
}

fn checked_lamport_transfer<'info>(
    from: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }

    let from_balance = from.lamports();
    require!(from_balance >= amount, VertaError::InsufficientVaultFunds);
    let to_balance = to.lamports();

    **from.try_borrow_mut_lamports()? = from_balance
        .checked_sub(amount)
        .ok_or(VertaError::MathOverflow)?;
    **to.try_borrow_mut_lamports()? = to_balance
        .checked_add(amount)
        .ok_or(VertaError::MathOverflow)?;

    Ok(())
}

#[derive(Accounts)]
#[instruction(episode_id: String)]
pub struct AutoUnlockEpisode<'info> {
    #[account(
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_config.bump,
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(
        seeds = [TITLE_CONFIG_SEED, title_config.title_id.as_bytes()],
        bump = title_config.bump,
        has_one = creator_wallet @ VertaError::InvalidRecipient,
    )]
    pub title_config: Account<'info, TitleConfig>,

    #[account(
        mut,
        seeds = [USER_VAULT_SEED, user_vault.user.as_ref()],
        bump = user_vault.bump,
    )]
    pub user_vault: Account<'info, UserVault>,

    #[account(
        init,
        payer = delegate,
        space = EpisodeUnlockReceipt::MAX_SIZE,
        seeds = [
            UNLOCK_RECEIPT_SEED,
            user_vault.user.as_ref(),
            title_config.key().as_ref(),
            episode_id.as_bytes()
        ],
        bump
    )]
    pub unlock_receipt: Account<'info, EpisodeUnlockReceipt>,

    /// CHECK: Validated via title_config.has_one
    #[account(mut)]
    pub creator_wallet: UncheckedAccount<'info>,

    #[account(mut)]
    pub crowdfund: Option<Account<'info, Crowdfund>>,

    #[account(mut)]
    pub revenue_vault: Option<SystemAccount<'info>>,

    /// CHECK: Required and validated by crowdfund state only for graduated crowdfund titles.
    #[account(mut)]
    pub production_wallet: Option<UncheckedAccount<'info>>,

    /// CHECK: Validated via platform_config.platform_wallet
    #[account(
        mut,
        constraint = platform_wallet.key() == platform_config.platform_wallet @ VertaError::Unauthorized
    )]
    pub platform_wallet: UncheckedAccount<'info>,

    #[account(mut)]
    pub delegate: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(episode_id: String)]
pub struct DirectPayout<'info> {
    #[account(
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_config.bump,
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(
        seeds = [TITLE_CONFIG_SEED, title_config.title_id.as_bytes()],
        bump = title_config.bump,
        has_one = creator_wallet @ VertaError::InvalidRecipient,
    )]
    pub title_config: Account<'info, TitleConfig>,

    #[account(
        mut,
        seeds = [USER_VAULT_SEED, user_vault.user.as_ref()],
        bump = user_vault.bump,
    )]
    pub user_vault: Account<'info, UserVault>,

    /// CHECK: Validated via title_config.has_one
    #[account(mut)]
    pub creator_wallet: UncheckedAccount<'info>,

    #[account(mut)]
    pub crowdfund: Option<Account<'info, Crowdfund>>,

    #[account(mut)]
    pub revenue_vault: Option<SystemAccount<'info>>,

    /// CHECK: Required and validated by crowdfund state only for graduated crowdfund titles.
    #[account(mut)]
    pub production_wallet: Option<UncheckedAccount<'info>>,

    /// CHECK: Validated via platform_config.platform_wallet
    #[account(
        mut,
        constraint = platform_wallet.key() == platform_config.platform_wallet @ VertaError::Unauthorized
    )]
    pub platform_wallet: UncheckedAccount<'info>,

    #[account(mut)]
    pub delegate: Signer<'info>,
    pub system_program: Program<'info, System>,
}
