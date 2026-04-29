use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, Transfer};
use crate::state::*;
use crate::errors::VertaError;

#[derive(Accounts)]
#[instruction(series_id: String, goal: u64, deadline: i64)]
pub struct InitializeCrowdfund<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + Crowdfund::INIT_SPACE,
        seeds = [b"crowdfund", series_id.as_bytes()],
        bump
    )]
    pub crowdfund: Account<'info, Crowdfund>,
    
    #[account(mut)]
    pub admin: Signer<'info>,
    
    #[account(
        seeds = [b"platform_config"],
        bump = platform_config.bump,
        has_one = admin @ VertaError::Unauthorized
    )]
    pub platform_config: Account<'info, PlatformConfig>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Contribute<'info> {
    #[account(
        mut,
        seeds = [b"crowdfund", crowdfund.series_id.as_bytes()],
        bump = crowdfund.bump,
        constraint = crowdfund.status == CrowdfundStatus::Active @ VertaError::CampaignInactive,
        constraint = Clock::get()?.unix_timestamp < crowdfund.deadline @ VertaError::CampaignEnded
    )]
    pub crowdfund: Account<'info, Crowdfund>,

    #[account(
        init_if_needed,
        payer = user,
        space = 8 + Contribution::INIT_SPACE,
        seeds = [b"contribution", crowdfund.key().as_ref(), user.key().as_ref()],
        bump
    )]
    pub contribution: Account<'info, Contribution>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FinalizeCampaign<'info> {
    #[account(
        mut,
        seeds = [b"crowdfund", crowdfund.series_id.as_bytes()],
        bump = crowdfund.bump,
        constraint = crowdfund.status == CrowdfundStatus::Active @ VertaError::CampaignInactive,
    )]
    pub crowdfund: Account<'info, Crowdfund>,

    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [b"platform_config"],
        bump = platform_config.bump,
        has_one = admin @ VertaError::Unauthorized
    )]
    pub platform_config: Account<'info, PlatformConfig>,
    
    /// CHECK: Recipient of funds (Production Wallet)
    #[account(mut, constraint = production_wallet.key() == crowdfund.production_wallet @ VertaError::InvalidRecipient)]
    pub production_wallet: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Refund<'info> {
    #[account(
        mut,
        seeds = [b"crowdfund", crowdfund.series_id.as_bytes()],
        bump = crowdfund.bump,
        constraint = crowdfund.status == CrowdfundStatus::Refunded @ VertaError::CampaignInactive,
    )]
    pub crowdfund: Account<'info, Crowdfund>,

    #[account(
        mut,
        seeds = [b"contribution", crowdfund.key().as_ref(), user.key().as_ref()],
        bump = contribution.bump,
        close = user
    )]
    pub contribution: Account<'info, Contribution>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimRevenue<'info> {
    #[account(
        mut,
        seeds = [b"crowdfund", crowdfund.series_id.as_bytes()],
        bump = crowdfund.bump,
        constraint = crowdfund.status == CrowdfundStatus::Succeeded @ VertaError::CampaignInactive,
    )]
    pub crowdfund: Account<'info, Crowdfund>,

    #[account(
        mut,
        seeds = [b"contribution", crowdfund.key().as_ref(), user.key().as_ref()],
        bump = contribution.bump,
    )]
    pub contribution: Account<'info, Contribution>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_crowdfund(
    ctx: Context<InitializeCrowdfund>,
    series_id: String,
    goal: u64,
    deadline: i64,
    production_wallet: Pubkey,
    investor_share_bps: u16,
) -> Result<()> {
    let crowdfund = &mut ctx.accounts.crowdfund;
    crowdfund.series_id = series_id;
    crowdfund.goal = goal;
    crowdfund.total_raised = 0;
    crowdfund.revenue_accumulated = 0;
    crowdfund.deadline = deadline;
    crowdfund.production_wallet = production_wallet;
    crowdfund.investor_share_bps = investor_share_bps;
    crowdfund.status = CrowdfundStatus::Active;
    crowdfund.bump = ctx.bumps.crowdfund;
    Ok(())
}

pub fn contribute(ctx: Context<Contribute>, amount: u64) -> Result<()> {
    let crowdfund = &mut ctx.accounts.crowdfund;
    let contribution = &mut ctx.accounts.contribution;

    // 1. Transfer $SOL from user to crowdfund PDA
    let cpi_context = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        Transfer {
            from: ctx.accounts.user.to_account_info(),
            to: crowdfund.to_account_info(),
        },
    );
    system_program::transfer(cpi_context, amount)?;

    // 2. Update accounts
    crowdfund.total_raised = crowdfund.total_raised.checked_add(amount).ok_or(VertaError::MathOverflow)?;
    
    if contribution.user == Pubkey::default() {
        contribution.user = ctx.accounts.user.key();
        contribution.amount_contributed = 0;
        contribution.amount_withdrawn = 0;
        contribution.bump = ctx.bumps.contribution;
    }
    
    contribution.amount_contributed = contribution.amount_contributed.checked_add(amount).ok_or(VertaError::MathOverflow)?;

    Ok(())
}

pub fn finalize_campaign(ctx: Context<FinalizeCampaign>) -> Result<()> {
    let crowdfund = &mut ctx.accounts.crowdfund;
    
    require!(crowdfund.total_raised >= crowdfund.goal, VertaError::GoalNotReached);
    
    crowdfund.status = CrowdfundStatus::Succeeded;
    
    // Transfer all funds to production wallet
    let amount = crowdfund.to_account_info().lamports();
    let rent = Rent::get()?;
    let min_rent = rent.minimum_balance(crowdfund.to_account_info().data_len());
    let withdrawable = amount.checked_sub(min_rent).ok_or(VertaError::MathOverflow)?;

    **crowdfund.to_account_info().try_borrow_mut_lamports()? -= withdrawable;
    **ctx.accounts.production_wallet.try_borrow_mut_lamports()? += withdrawable;

    Ok(())
}

pub fn fail_campaign(ctx: Context<FinalizeCampaign>) -> Result<()> {
    let crowdfund = &mut ctx.accounts.crowdfund;
    let now = Clock::get()?.unix_timestamp;
    
    require!(now >= crowdfund.deadline, VertaError::CampaignNotEnded);
    require!(crowdfund.total_raised < crowdfund.goal, VertaError::MathOverflow);
    
    crowdfund.status = CrowdfundStatus::Refunded;
    Ok(())
}

pub fn refund(ctx: Context<Refund>) -> Result<()> {
    let crowdfund = &mut ctx.accounts.crowdfund;
    let contribution = &mut ctx.accounts.contribution;
    
    let amount = contribution.amount_contributed;
    
    // Transfer funds from crowdfund PDA to user
    **crowdfund.to_account_info().try_borrow_mut_lamports()? -= amount;
    **ctx.accounts.user.try_borrow_mut_lamports()? += amount;

    Ok(())
}

pub fn claim_revenue(ctx: Context<ClaimRevenue>) -> Result<()> {
    let crowdfund = &mut ctx.accounts.crowdfund;
    let contribution = &mut ctx.accounts.contribution;

    // 1. Calculate the total pool dedicated to investors (using BPS)
    let investor_pool = (crowdfund.revenue_accumulated as u128)
        .checked_mul(crowdfund.investor_share_bps as u128)
        .ok_or(VertaError::MathOverflow)?
        .checked_div(10_000)
        .ok_or(VertaError::MathOverflow)? as u64;

    // 2. Calculate the user's share of THAT pool: (User Contribution / Goal) * Investor Pool
    let total_entitled = (contribution.amount_contributed as u128)
        .checked_mul(investor_pool as u128)
        .ok_or(VertaError::MathOverflow)?
        .checked_div(crowdfund.goal as u128)
        .ok_or(VertaError::MathOverflow)? as u64;

    let claimable = total_entitled.checked_sub(contribution.amount_withdrawn).ok_or(VertaError::MathOverflow)?;

    require!(claimable > 0, VertaError::NoRevenueToClaim);

    // Update state
    contribution.amount_withdrawn = contribution.amount_withdrawn.checked_add(claimable).ok_or(VertaError::MathOverflow)?;

    // Transfer funds from crowdfund PDA to user
    **crowdfund.to_account_info().try_borrow_mut_lamports()? -= claimable;
    **ctx.accounts.user.try_borrow_mut_lamports()? += claimable;

    Ok(())
}


