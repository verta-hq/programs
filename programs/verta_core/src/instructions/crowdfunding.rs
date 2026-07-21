use crate::constants::{
    BPS_DENOMINATOR, ESCROW_VAULT_SEED, PLATFORM_AUTHORITY_SEED, REVENUE_VAULT_SEED, SCALE,
    TITLE_CONFIG_SEED, USER_VAULT_SEED,
};
use crate::errors::VertaError;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer as TokenTransfer};
use mpl_core::{
    accounts::BaseAssetV1,
    instructions::{CreateCollectionV1CpiBuilder, CreateV1CpiBuilder},
    types::{Attribute, Attributes, Plugin, PluginAuthorityPair},
};

const REVENUE_SHARE_COLLECTION_NAME: &str = "Verta Revenue Shares";
const REVENUE_SHARE_COLLECTION_URI: &str =
    "https://verta.app/metadata/revenue-shares-collection.json";
const REVENUE_SHARE_PASS_URI: &str = "https://verta.app/metadata/revenue-share-pass.json";

#[derive(Accounts)]
pub struct CreateRevenueShareCollection<'info> {
    #[account(
        mut,
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_config.bump,
        has_one = admin @ VertaError::Unauthorized
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(mut)]
    pub collection: Signer<'info>,

    /// CHECK: Metaplex Core program
    #[account(address = mpl_core::ID)]
    pub core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

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

    /// Settlement mint (USDC). Admin-trusted at init; all campaign token
    /// accounts are validated against `crowdfund.mint` thereafter.
    pub mint: Account<'info, Mint>,

    /// Escrow: holds contributions until finalize (→ production ATA) or
    /// refunds. Authority = crowdfund PDA.
    #[account(
        init,
        payer = admin,
        seeds = [ESCROW_VAULT_SEED, crowdfund.key().as_ref()],
        bump,
        token::mint = mint,
        token::authority = crowdfund,
    )]
    pub escrow_vault: Account<'info, TokenAccount>,

    /// Backer revenue pool fed by deposit_revenue; drained by claim_revenue.
    #[account(
        init,
        payer = admin,
        seeds = [REVENUE_VAULT_SEED, crowdfund.key().as_ref()],
        bump,
        token::mint = mint,
        token::authority = crowdfund,
    )]
    pub revenue_vault: Account<'info, TokenAccount>,

    #[account(
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_config.bump,
        has_one = admin @ VertaError::Unauthorized,
        constraint = platform_config.revenue_share_collection != Pubkey::default() @ VertaError::WrongCollection
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    pub token_program: Program<'info, Token>,
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
        payer = payer,
        space = 8 + Contribution::INIT_SPACE,
        seeds = [b"contribution", crowdfund.key().as_ref(), user.key().as_ref()],
        bump
    )]
    pub contribution: Account<'info, Contribution>,

    pub user: Signer<'info>,

    /// Rent payer for the contribution PDA — the platform in the sponsored
    /// flow (users hold no SOL), or the user themselves when self-paying.
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        constraint = user_ata.owner == user.key() @ VertaError::Unauthorized,
        constraint = user_ata.mint == crowdfund.mint @ VertaError::InvalidMint,
    )]
    pub user_ata: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [ESCROW_VAULT_SEED, crowdfund.key().as_ref()],
        bump,
    )]
    pub escrow_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
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

    #[account(
        mut,
        seeds = [ESCROW_VAULT_SEED, crowdfund.key().as_ref()],
        bump,
    )]
    pub escrow_vault: Account<'info, TokenAccount>,

    /// Production wallet's token account (any account owned by the stored
    /// production wallet, in the campaign mint).
    #[account(
        mut,
        constraint = production_ata.owner == crowdfund.production_wallet @ VertaError::InvalidRecipient,
        constraint = production_ata.mint == crowdfund.mint @ VertaError::InvalidMint,
    )]
    pub production_ata: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct HaltCampaign<'info> {
    #[account(
        mut,
        seeds = [b"crowdfund", crowdfund.series_id.as_bytes()],
        bump = crowdfund.bump,
        constraint = crowdfund.status == CrowdfundStatus::Active @ VertaError::CampaignInactive,
    )]
    pub crowdfund: Account<'info, Crowdfund>,

    pub admin: Signer<'info>,

    #[account(
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_config.bump,
        has_one = admin @ VertaError::Unauthorized
    )]
    pub platform_config: Account<'info, PlatformConfig>,
}

#[derive(Accounts)]
pub struct Refund<'info> {
    #[account(
        mut,
        seeds = [b"crowdfund", crowdfund.series_id.as_bytes()],
        bump = crowdfund.bump,
        constraint = crowdfund.status == CrowdfundStatus::Refunded
            || crowdfund.status == CrowdfundStatus::Cancelled @ VertaError::CampaignInactive,
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

    #[account(
        mut,
        constraint = user_ata.owner == user.key() @ VertaError::Unauthorized,
        constraint = user_ata.mint == crowdfund.mint @ VertaError::InvalidMint,
    )]
    pub user_ata: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [ESCROW_VAULT_SEED, crowdfund.key().as_ref()],
        bump,
    )]
    pub escrow_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MintRevenueShare<'info> {
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
        constraint = !contribution.minted @ VertaError::AlreadyMinted,
        constraint = contribution.user == user.key() @ VertaError::Unauthorized,
    )]
    pub contribution: Account<'info, Contribution>,

    #[account(
        init,
        payer = payer,
        space = 8 + SharePosition::INIT_SPACE,
        seeds = [b"position", asset.key().as_ref()],
        bump
    )]
    pub position: Account<'info, SharePosition>,

    pub user: Signer<'info>,

    /// Rent payer for the position PDA + Core asset — the platform in the
    /// sponsored flow, or the user themselves when self-paying.
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(mut)]
    pub asset: Signer<'info>,

    /// CHECK: Metaplex Core collection validated by stored campaign/platform address and owner.
    #[account(
        mut,
        address = crowdfund.collection @ VertaError::WrongCollection,
        owner = mpl_core::ID @ VertaError::InvalidAsset
    )]
    pub collection: UncheckedAccount<'info>,

    #[account(
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_config.bump,
        constraint = platform_config.revenue_share_collection == collection.key() @ VertaError::WrongCollection
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    /// CHECK: Metaplex Core program
    #[account(address = mpl_core::ID)]
    pub core_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RecordRevenue<'info> {
    #[account(
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_config.bump,
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(
        seeds = [TITLE_CONFIG_SEED, title_config.title_id.as_bytes()],
        bump = title_config.bump,
        constraint = title_config.active @ VertaError::TitleInactive,
        constraint = title_config.creator_wallet == crowdfund.key() @ VertaError::InvalidRecipient,
    )]
    pub title_config: Account<'info, TitleConfig>,

    #[account(
        mut,
        seeds = [USER_VAULT_SEED, user_vault.user.as_ref()],
        bump = user_vault.bump,
    )]
    pub user_vault: Account<'info, UserVault>,

    #[account(
        mut,
        seeds = [b"crowdfund", crowdfund.series_id.as_bytes()],
        bump = crowdfund.bump,
        constraint = crowdfund.status == CrowdfundStatus::Succeeded @ VertaError::CampaignInactive,
    )]
    pub crowdfund: Account<'info, Crowdfund>,

    #[account(
        mut,
        seeds = [REVENUE_VAULT_SEED, crowdfund.key().as_ref()],
        bump
    )]
    pub revenue_vault: SystemAccount<'info>,

    /// CHECK: Recipient of production share; validated against crowdfund state.
    #[account(
        mut,
        address = crowdfund.production_wallet @ VertaError::InvalidRecipient
    )]
    pub production_wallet: UncheckedAccount<'info>,

    pub delegate: Signer<'info>,
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
        seeds = [REVENUE_VAULT_SEED, crowdfund.key().as_ref()],
        bump
    )]
    pub revenue_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"position", asset.key().as_ref()],
        bump = position.bump,
        constraint = position.crowdfund == crowdfund.key() @ VertaError::WrongCollection,
        constraint = position.asset == asset.key() @ VertaError::InvalidAsset,
    )]
    pub position: Account<'info, SharePosition>,

    /// CHECK: Metaplex Core asset; owner and bytes verified in handler.
    #[account(owner = mpl_core::ID @ VertaError::InvalidAsset)]
    pub asset: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = user_ata.owner == user.key() @ VertaError::Unauthorized,
        constraint = user_ata.mint == crowdfund.mint @ VertaError::InvalidMint,
    )]
    pub user_ata: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

/// Platform admin deposits episode-revenue creator shares (already collected
/// in the USDC treasury by the settlement sweep) into a graduated campaign:
/// the backer portion lands in the revenue vault and advances the
/// accumulator, the remainder goes straight to the production wallet.
#[derive(Accounts)]
pub struct DepositRevenue<'info> {
    #[account(
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_config.bump,
        has_one = admin @ VertaError::Unauthorized,
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        constraint = admin_ata.owner == admin.key() @ VertaError::Unauthorized,
        constraint = admin_ata.mint == crowdfund.mint @ VertaError::InvalidMint,
    )]
    pub admin_ata: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"crowdfund", crowdfund.series_id.as_bytes()],
        bump = crowdfund.bump,
        constraint = crowdfund.status == CrowdfundStatus::Succeeded @ VertaError::CampaignInactive,
    )]
    pub crowdfund: Account<'info, Crowdfund>,

    #[account(
        mut,
        seeds = [REVENUE_VAULT_SEED, crowdfund.key().as_ref()],
        bump
    )]
    pub revenue_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = production_ata.owner == crowdfund.production_wallet @ VertaError::InvalidRecipient,
        constraint = production_ata.mint == crowdfund.mint @ VertaError::InvalidMint,
    )]
    pub production_ata: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn initialize_crowdfund(
    ctx: Context<InitializeCrowdfund>,
    series_id: String,
    goal: u64,
    deadline: i64,
    production_wallet: Pubkey,
    backer_revenue_bps: u16,
) -> Result<()> {
    let crowdfund = &mut ctx.accounts.crowdfund;
    crowdfund.series_id = series_id;
    crowdfund.goal = goal;
    crowdfund.total_raised = 0;
    crowdfund.deadline = deadline;
    crowdfund.production_wallet = production_wallet;
    crowdfund.backer_revenue_bps = backer_revenue_bps;
    crowdfund.status = CrowdfundStatus::Active;
    crowdfund.bump = ctx.bumps.crowdfund;
    crowdfund.cumulative_revenue_per_bps = 0;
    crowdfund.total_issued_bps = 0;
    crowdfund.accumulator_at_finalize = 0;
    crowdfund.collection = ctx.accounts.platform_config.revenue_share_collection;
    crowdfund.mint = ctx.accounts.mint.key();
    Ok(())
}

pub fn contribute(ctx: Context<Contribute>, amount: u64) -> Result<()> {
    let crowdfund = &mut ctx.accounts.crowdfund;
    let contribution = &mut ctx.accounts.contribution;

    // 1. Transfer USDC from the user's token account into escrow
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TokenTransfer {
                from: ctx.accounts.user_ata.to_account_info(),
                to: ctx.accounts.escrow_vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        amount,
    )?;

    // 2. Update accounts
    crowdfund.total_raised = crowdfund
        .total_raised
        .checked_add(amount)
        .ok_or(VertaError::MathOverflow)?;

    if contribution.user == Pubkey::default() {
        contribution.user = ctx.accounts.user.key();
        contribution.amount_contributed = 0;
        contribution.minted = false;
        contribution.bump = ctx.bumps.contribution;
    }

    contribution.amount_contributed = contribution
        .amount_contributed
        .checked_add(amount)
        .ok_or(VertaError::MathOverflow)?;

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

    // Ensure the goal was actually met. Early finalize (before the deadline) is
    // an explicit admin policy: the Succeeded status atomically freezes
    // contributions, so total_raised is fixed from this point for share math.
    require!(
        crowdfund.total_raised >= crowdfund.goal,
        VertaError::GoalNotReached
    );

    // Mark as succeeded (this blocks further contributions)
    crowdfund.status = CrowdfundStatus::Succeeded;

    // Freeze the accumulator denominators for the revenue engine
    crowdfund.total_issued_bps = crowdfund.backer_revenue_bps;
    crowdfund.accumulator_at_finalize = crowdfund.cumulative_revenue_per_bps;

    // Release the full escrow to the production wallet, signed by the
    // crowdfund PDA (token accounts carry no rent-reserve concern).
    let withdrawable = ctx.accounts.escrow_vault.amount;
    if withdrawable > 0 {
        let series_id = crowdfund.series_id.clone();
        let bump = [crowdfund.bump];
        let seeds: &[&[u8]] = &[b"crowdfund", series_id.as_bytes(), &bump];
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TokenTransfer {
                    from: ctx.accounts.escrow_vault.to_account_info(),
                    to: ctx.accounts.production_ata.to_account_info(),
                    authority: crowdfund.to_account_info(),
                },
                &[seeds],
            ),
            withdrawable,
        )?;
    }

    msg!("Campaign Finalized Successfully. Funds released to production wallet.");
    Ok(())
}

pub fn fail_campaign(ctx: Context<HaltCampaign>) -> Result<()> {
    let crowdfund = &mut ctx.accounts.crowdfund;
    let now = Clock::get()?.unix_timestamp;

    // Ensure the deadline has passed
    require!(now >= crowdfund.deadline, VertaError::CampaignNotEnded);

    // Ensure the goal was NOT met
    require!(
        crowdfund.total_raised < crowdfund.goal,
        VertaError::GoalAlreadyReached
    );

    crowdfund.status = CrowdfundStatus::Refunded;
    msg!("Campaign Failed. Refunds are now enabled for backers.");
    Ok(())
}

pub fn cancel_campaign(ctx: Context<HaltCampaign>) -> Result<()> {
    let crowdfund = &mut ctx.accounts.crowdfund;
    crowdfund.status = CrowdfundStatus::Cancelled;
    msg!("Campaign Cancelled by admin. Refunds are now enabled for backers.");
    Ok(())
}

pub fn refund(ctx: Context<Refund>) -> Result<()> {
    let crowdfund = &ctx.accounts.crowdfund;
    let contribution = &ctx.accounts.contribution;

    let amount = contribution.amount_contributed;
    require!(
        ctx.accounts.escrow_vault.amount >= amount,
        VertaError::InsufficientVaultBalance
    );

    let series_id = crowdfund.series_id.clone();
    let bump = [crowdfund.bump];
    let seeds: &[&[u8]] = &[b"crowdfund", series_id.as_bytes(), &bump];
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TokenTransfer {
                from: ctx.accounts.escrow_vault.to_account_info(),
                to: ctx.accounts.user_ata.to_account_info(),
                authority: crowdfund.to_account_info(),
            },
            &[seeds],
        ),
        amount,
    )?;

    Ok(())
}

pub fn create_revenue_share_collection(ctx: Context<CreateRevenueShareCollection>) -> Result<()> {
    let platform_config = &mut ctx.accounts.platform_config;

    require!(
        platform_config.revenue_share_collection == Pubkey::default()
            || platform_config.revenue_share_collection == ctx.accounts.collection.key(),
        VertaError::WrongCollection
    );

    CreateCollectionV1CpiBuilder::new(&ctx.accounts.core_program.to_account_info())
        .collection(&ctx.accounts.collection.to_account_info())
        .update_authority(Some(&platform_config.to_account_info()))
        .payer(&ctx.accounts.admin.to_account_info())
        .system_program(&ctx.accounts.system_program.to_account_info())
        .name(REVENUE_SHARE_COLLECTION_NAME.to_string())
        .uri(REVENUE_SHARE_COLLECTION_URI.to_string())
        .plugins(vec![])
        .invoke()?;

    platform_config.revenue_share_collection = ctx.accounts.collection.key();

    msg!(
        "Revenue Share Pass collection created: {}",
        platform_config.revenue_share_collection
    );
    Ok(())
}

pub fn mint_revenue_share(ctx: Context<MintRevenueShare>) -> Result<()> {
    let crowdfund = &ctx.accounts.crowdfund;
    let contribution = &mut ctx.accounts.contribution;
    let position = &mut ctx.accounts.position;

    // Pro-rata over final total_raised; overfunding dilutes instead of over-allocating.
    require!(crowdfund.total_raised > 0, VertaError::MathOverflow);
    let share_bps_u128 = (contribution.amount_contributed as u128)
        .checked_mul(crowdfund.backer_revenue_bps as u128)
        .ok_or(VertaError::MathOverflow)?
        .checked_div(crowdfund.total_raised as u128)
        .ok_or(VertaError::MathOverflow)?;

    require!(share_bps_u128 > 0, VertaError::NoRevenueToClaim);
    require!(share_bps_u128 <= u16::MAX as u128, VertaError::MathOverflow);
    let share_bps = share_bps_u128 as u16;

    let name = format!("{} Revenue Share Pass", crowdfund.series_id);

    let mut attributes = Vec::new();
    attributes.push(Attribute {
        key: "share_bps".to_string(),
        value: share_bps.to_string(),
    });
    attributes.push(Attribute {
        key: "series_id".to_string(),
        value: crowdfund.series_id.clone(),
    });

    let platform_bump = [ctx.accounts.platform_config.bump];
    let platform_seeds: &[&[u8]] = &[PLATFORM_AUTHORITY_SEED, &platform_bump];
    let signer_seeds = &[platform_seeds];

    CreateV1CpiBuilder::new(&ctx.accounts.core_program.to_account_info())
        .asset(&ctx.accounts.asset.to_account_info())
        .collection(Some(&ctx.accounts.collection.to_account_info()))
        .authority(Some(&ctx.accounts.platform_config.to_account_info()))
        .payer(&ctx.accounts.payer.to_account_info())
        .owner(Some(&ctx.accounts.user.to_account_info()))
        // Core collection assets derive update authority from the collection.
        // The collection itself is program-owned through the PlatformConfig PDA.
        .system_program(&ctx.accounts.system_program.to_account_info())
        .name(name)
        .uri(REVENUE_SHARE_PASS_URI.to_string())
        .plugins(vec![PluginAuthorityPair {
            plugin: Plugin::Attributes(Attributes {
                attribute_list: attributes,
            }),
            authority: None,
        }])
        .invoke_signed(signer_seeds)?;

    position.crowdfund = crowdfund.key();
    position.asset = ctx.accounts.asset.key();
    position.share_bps = share_bps;
    position.claimed_checkpoint = crowdfund.accumulator_at_finalize;
    position.bump = ctx.bumps.position;
    contribution.minted = true;

    msg!(
        "Revenue Share Pass minted successfully. Share: {} BPS",
        share_bps
    );
    Ok(())
}

pub fn record_revenue(ctx: Context<RecordRevenue>, amount: u64) -> Result<()> {
    require!(
        !ctx.accounts.platform_config.paused,
        VertaError::PlatformPaused
    );
    require!(
        ctx.accounts.user_vault.auto_play_enabled,
        VertaError::AutoPlayDisabled
    );
    require!(
        ctx.accounts.delegate.key() == ctx.accounts.user_vault.delegate,
        VertaError::OnlyDelegate
    );

    record_revenue_from_source(
        &mut ctx.accounts.crowdfund,
        &ctx.accounts.revenue_vault.to_account_info(),
        &ctx.accounts.production_wallet.to_account_info(),
        &ctx.accounts.user_vault.to_account_info(),
        amount,
    )
}

pub fn record_revenue_from_source<'info>(
    crowdfund: &mut Account<'info, Crowdfund>,
    revenue_vault: &AccountInfo<'info>,
    production_wallet: &AccountInfo<'info>,
    source: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    require!(
        crowdfund.status == CrowdfundStatus::Succeeded,
        VertaError::CampaignInactive
    );
    require!(
        production_wallet.key() == crowdfund.production_wallet,
        VertaError::InvalidRecipient
    );

    let expected_revenue_vault =
        Pubkey::find_program_address(&[REVENUE_VAULT_SEED, crowdfund.key().as_ref()], &crate::ID).0;
    require!(
        revenue_vault.key() == expected_revenue_vault,
        VertaError::InvalidRecipient
    );

    let backer_portion = (amount as u128)
        .checked_mul(crowdfund.backer_revenue_bps as u128)
        .ok_or(VertaError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128)
        .ok_or(VertaError::MathOverflow)? as u64;
    let production_portion = amount
        .checked_sub(backer_portion)
        .ok_or(VertaError::MathOverflow)?;

    if backer_portion > 0 {
        require!(crowdfund.total_issued_bps > 0, VertaError::MathOverflow);
        checked_lamport_transfer(source, revenue_vault, backer_portion)?;

        let accumulator_delta = (backer_portion as u128)
            .checked_mul(SCALE)
            .ok_or(VertaError::MathOverflow)?
            .checked_div(crowdfund.total_issued_bps as u128)
            .ok_or(VertaError::MathOverflow)?;
        crowdfund.cumulative_revenue_per_bps = crowdfund
            .cumulative_revenue_per_bps
            .checked_add(accumulator_delta)
            .ok_or(VertaError::MathOverflow)?;
    }

    if production_portion > 0 {
        checked_lamport_transfer(source, production_wallet, production_portion)?;
    }

    Ok(())
}

pub fn claim_revenue(ctx: Context<ClaimRevenue>) -> Result<()> {
    let crowdfund = &mut ctx.accounts.crowdfund;
    let position = &mut ctx.accounts.position;

    let asset_binding = ctx.accounts.asset.to_account_info();
    let asset_data = asset_binding.try_borrow_data()?;
    let asset_info = BaseAssetV1::from_bytes(&asset_data).map_err(|_| VertaError::InvalidAsset)?;
    require!(
        asset_info.owner == ctx.accounts.user.key(),
        VertaError::NotOwner
    );

    let checkpoint_delta = crowdfund
        .cumulative_revenue_per_bps
        .checked_sub(position.claimed_checkpoint)
        .ok_or(VertaError::MathOverflow)?;
    let claimable_u128 = (position.share_bps as u128)
        .checked_mul(checkpoint_delta)
        .ok_or(VertaError::MathOverflow)?
        .checked_div(SCALE)
        .ok_or(VertaError::MathOverflow)?;
    require!(claimable_u128 > 0, VertaError::NoRevenueToClaim);
    require!(claimable_u128 <= u64::MAX as u128, VertaError::MathOverflow);
    let claimable = claimable_u128 as u64;

    require!(
        ctx.accounts.revenue_vault.amount >= claimable,
        VertaError::InsufficientVaultBalance
    );

    position.claimed_checkpoint = crowdfund.cumulative_revenue_per_bps;
    let series_id = crowdfund.series_id.clone();
    let bump = [crowdfund.bump];
    let seeds: &[&[u8]] = &[b"crowdfund", series_id.as_bytes(), &bump];

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TokenTransfer {
                from: ctx.accounts.revenue_vault.to_account_info(),
                to: ctx.accounts.user_ata.to_account_info(),
                authority: crowdfund.to_account_info(),
            },
            &[seeds],
        ),
        claimable,
    )?;

    msg!("Revenue Claimed Successfully: {} micro-USDC", claimable);
    Ok(())
}

/// Settlement entry point: splits an episode-revenue creator share between the
/// backer revenue vault (advancing the accumulator) and the production wallet,
/// funded from the admin's USDC token account.
pub fn deposit_revenue(ctx: Context<DepositRevenue>, amount: u64) -> Result<()> {
    require!(
        !ctx.accounts.platform_config.paused,
        VertaError::PlatformPaused
    );
    require!(amount > 0, VertaError::InvalidPrice);

    let crowdfund = &mut ctx.accounts.crowdfund;

    let backer_portion = (amount as u128)
        .checked_mul(crowdfund.backer_revenue_bps as u128)
        .ok_or(VertaError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128)
        .ok_or(VertaError::MathOverflow)? as u64;
    let production_portion = amount
        .checked_sub(backer_portion)
        .ok_or(VertaError::MathOverflow)?;

    if backer_portion > 0 {
        require!(crowdfund.total_issued_bps > 0, VertaError::MathOverflow);
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TokenTransfer {
                    from: ctx.accounts.admin_ata.to_account_info(),
                    to: ctx.accounts.revenue_vault.to_account_info(),
                    authority: ctx.accounts.admin.to_account_info(),
                },
            ),
            backer_portion,
        )?;

        let accumulator_delta = (backer_portion as u128)
            .checked_mul(SCALE)
            .ok_or(VertaError::MathOverflow)?
            .checked_div(crowdfund.total_issued_bps as u128)
            .ok_or(VertaError::MathOverflow)?;
        crowdfund.cumulative_revenue_per_bps = crowdfund
            .cumulative_revenue_per_bps
            .checked_add(accumulator_delta)
            .ok_or(VertaError::MathOverflow)?;
    }

    if production_portion > 0 {
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TokenTransfer {
                    from: ctx.accounts.admin_ata.to_account_info(),
                    to: ctx.accounts.production_ata.to_account_info(),
                    authority: ctx.accounts.admin.to_account_info(),
                },
            ),
            production_portion,
        )?;
    }

    msg!(
        "Revenue deposited: {} micro-USDC ({} backers / {} production)",
        amount,
        backer_portion,
        production_portion
    );
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
    require!(from_balance >= amount, VertaError::InsufficientVaultBalance);
    let to_balance = to.lamports();

    **from.try_borrow_mut_lamports()? = from_balance
        .checked_sub(amount)
        .ok_or(VertaError::MathOverflow)?;
    **to.try_borrow_mut_lamports()? = to_balance
        .checked_add(amount)
        .ok_or(VertaError::MathOverflow)?;

    Ok(())
}

#[event]
pub struct ContributionEvent {
    pub series_id: String,
    pub user: Pubkey,
    pub amount: u64,
    pub total_raised: u64,
}
