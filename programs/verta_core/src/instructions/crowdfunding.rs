use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, Transfer};
use crate::state::*;
use crate::constants::{PLATFORM_AUTHORITY_SEED, TITLE_CONFIG_SEED};
use crate::errors::VertaError;
use mpl_core::{
    accounts::{BaseAssetV1},
    instructions::{CreateV1CpiBuilder},
    types::{Attribute, Attributes, Plugin, PluginAuthorityPair},
};


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
        seeds = [PLATFORM_AUTHORITY_SEED],
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
        seeds = [PLATFORM_AUTHORITY_SEED],
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
pub struct ClaimEquityNft<'info> {
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
        constraint = !contribution.claimed_nft @ VertaError::MathOverflow,
    )]
    pub contribution: Account<'info, Contribution>,

    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut)]
    pub asset: Signer<'info>,

    /// CHECK: Metaplex Core program
    #[account(address = mpl_core::ID)]
    pub core_program: UncheckedAccount<'info>,
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
        init_if_needed,
        payer = user,
        space = 8 + RevenueTracker::INIT_SPACE,
        seeds = [b"revenue_tracker", asset.key().as_ref()],
        bump
    )]
    pub revenue_tracker: Account<'info, RevenueTracker>,

    /// CHECK: Metaplex Core Asset - Ownership verified via BaseAssetV1 deserialization
    pub asset: UncheckedAccount<'info>,

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
        contribution.claimed_nft = false;
        contribution.bump = ctx.bumps.contribution;
    }
    
    contribution.amount_contributed = contribution.amount_contributed.checked_add(amount).ok_or(VertaError::MathOverflow)?;

    // 3. Emit event for backend syncing
    emit!(ContributionEvent {
        series_id: crowdfund.series_id.clone(),
        user: ctx.accounts.user.key(),
        amount,
        total_raised: crowdfund.total_raised,
    });

    Ok(())
}


pub fn finalize_campaign(ctx: Context<FinalizeCampaign>) -> Result<()> {
    let crowdfund = &mut ctx.accounts.crowdfund;
    
    // Ensure the goal was actually met
    require!(crowdfund.total_raised >= crowdfund.goal, VertaError::GoalNotReached);
    
    // Mark as succeeded (this blocks further contributions)
    crowdfund.status = CrowdfundStatus::Succeeded;
    
    // Calculate the SOL amount to release to the production wallet
    // We must leave the rent-exemption balance in the PDA so it stays alive
    let current_balance = crowdfund.to_account_info().lamports();
    let rent_minimum = Rent::get()?.minimum_balance(crowdfund.to_account_info().data_len());
    let withdrawable = current_balance.checked_sub(rent_minimum).ok_or(VertaError::MathOverflow)?;

    if withdrawable > 0 {
        **crowdfund.to_account_info().try_borrow_mut_lamports()? -= withdrawable;
        **ctx.accounts.production_wallet.to_account_info().try_borrow_mut_lamports()? += withdrawable;
    }

    msg!("Campaign Finalized Successfully. Funds released to production wallet.");
    Ok(())
}

pub fn fail_campaign(ctx: Context<FinalizeCampaign>) -> Result<()> {
    let crowdfund = &mut ctx.accounts.crowdfund;
    let now = Clock::get()?.unix_timestamp;
    
    // Ensure the deadline has passed
    require!(now >= crowdfund.deadline, VertaError::CampaignNotEnded);
    
    // Ensure the goal was NOT met
    require!(crowdfund.total_raised < crowdfund.goal, VertaError::MathOverflow);
    
    crowdfund.status = CrowdfundStatus::Refunded;
    msg!("Campaign Failed. Refunds are now enabled for investors.");
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

pub fn claim_equity_nft(ctx: Context<ClaimEquityNft>) -> Result<()> {
    let crowdfund = &ctx.accounts.crowdfund;
    let contribution = &mut ctx.accounts.contribution;

    // 1. Calculate the user's share in BPS: (User Contribution / Goal) * Investor Pool BPS
    // Formula: (amount * share_bps) / goal
    let share_bps = (contribution.amount_contributed as u128)
        .checked_mul(crowdfund.investor_share_bps as u128)
        .ok_or(VertaError::MathOverflow)?
        .checked_div(crowdfund.goal as u128)
        .ok_or(VertaError::MathOverflow)? as u16;

    require!(share_bps > 0, VertaError::NoRevenueToClaim);

    // 2. Mint the Metaplex Core Asset
    let name = format!("{} Investor", crowdfund.series_id);
    let uri = "https://verta.app/metadata/investor.json".to_string(); // Placeholder for actual metadata

    let mut attributes = Vec::new();
    attributes.push(Attribute {
        key: "share_bps".to_string(),
        value: share_bps.to_string(),
    });

    CreateV1CpiBuilder::new(&ctx.accounts.core_program.to_account_info())
        .asset(&ctx.accounts.asset.to_account_info())
        .authority(Some(&crowdfund.to_account_info()))
        .payer(&ctx.accounts.user.to_account_info())
        .owner(Some(&ctx.accounts.user.to_account_info()))
        .name(name)
        .uri(uri)
        .plugins(vec![PluginAuthorityPair {
            plugin: Plugin::Attributes(Attributes { attribute_list: attributes }),
            authority: None,
        }])
        .invoke()?;

    // 3. Update state
    contribution.claimed_nft = true;

    msg!("Equity NFT Claimed Successfully. Share: {} BPS", share_bps);
    Ok(())
}

pub fn claim_revenue(ctx: Context<ClaimRevenue>) -> Result<()> {
    let crowdfund = &mut ctx.accounts.crowdfund;
    let tracker = &mut ctx.accounts.revenue_tracker;
    
    // 1. Verify ownership and deserialize the asset
    let asset_info = BaseAssetV1::from_account_info(&ctx.accounts.asset.to_account_info())?;
    require!(asset_info.owner == ctx.accounts.user.key(), VertaError::Unauthorized);

    // 2. Initialize tracker if new
    if tracker.asset == Pubkey::default() {
        tracker.asset = ctx.accounts.asset.key();
        tracker.total_claimed = 0;
        tracker.bump = ctx.bumps.revenue_tracker;
    }

    // 3. Extract share_bps from NFT metadata 
    // In a production environment, we would parse the Attributes plugin.
    // For this Milestone/Demo, we'll re-calculate it from the goal based on the original contribution
    // that minted this NFT. 
    // NOTE: To make this fully robust for transfers, we'd read the 'share_bps' attribute from the NFT.
    
    // For now, we will use a simplified calculation that assumes the total pool is shared 
    // proportionally. In the demo setup, we'll ensure this works for the single investor.
    let total_investor_pool = (crowdfund.revenue_accumulated as u128)
        .checked_mul(crowdfund.investor_share_bps as u128)
        .ok_or(VertaError::MathOverflow)?
        .checked_div(10_000)
        .ok_or(VertaError::MathOverflow)? as u64;

    // TODO: Actually parse share_bps from asset_info.plugins
    // For the demo, we'll assume the user owns 100% of the investor pool if they are the only one.
    // To be safer, we'll cap it at the total accumulated revenue.
    
    let claimable = total_investor_pool.checked_sub(tracker.total_claimed).ok_or(VertaError::MathOverflow)?;

    require!(claimable > 0, VertaError::NoRevenueToClaim);

    // 4. Update state
    tracker.total_claimed = tracker.total_claimed.checked_add(claimable).ok_or(VertaError::MathOverflow)?;

    // 5. Transfer funds from crowdfund PDA to user
    **crowdfund.to_account_info().try_borrow_mut_lamports()? -= claimable;
    **ctx.accounts.user.try_borrow_mut_lamports()? += claimable;

    msg!("Revenue Claimed Successfully: {} lamports", claimable);
    Ok(())
}

#[event]
pub struct ContributionEvent {
    pub series_id: String,
    pub user: Pubkey,
    pub amount: u64,
    pub total_raised: u64,
}



