use crate::constants::*;
use crate::errors::VertaError;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{
    self, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use mpl_core::{
    accounts::BaseAssetV1,
    instructions::{CreateCollectionV1CpiBuilder, CreateV1CpiBuilder},
    types::{Attribute, Attributes, Plugin, PluginAuthorityPair},
};

const REVENUE_SHARE_COLLECTION_NAME: &str = "Verta Revenue Shares";
const REVENUE_SHARE_COLLECTION_URI: &str =
    "https://www.verta.xyz/metadata/revenue-shares-collection.json";
const REVENUE_SHARE_PASS_URI: &str = "https://www.verta.xyz/metadata/revenue-share-pass.json";

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
#[instruction(series_id: String)]
pub struct InitializeCrowdfund<'info> {
    #[account(
        init,
        payer = operator,
        space = 8 + Crowdfund::INIT_SPACE,
        seeds = [CROWDFUND_SEED, series_id.as_bytes()],
        bump
    )]
    pub crowdfund: Account<'info, Crowdfund>,

    #[account(mut)]
    pub operator: Signer<'info>,

    #[account(
        constraint = mint.key() == platform_config.settlement_mint @ VertaError::InvalidSettlementMint,
        mint::token_program = token_program,
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init,
        payer = operator,
        seeds = [ESCROW_VAULT_SEED, crowdfund.key().as_ref()],
        bump,
        token::mint = mint,
        token::authority = crowdfund,
        token::token_program = token_program,
    )]
    pub escrow_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init,
        payer = operator,
        seeds = [REVENUE_VAULT_SEED, crowdfund.key().as_ref()],
        bump,
        token::mint = mint,
        token::authority = crowdfund,
        token::token_program = token_program,
    )]
    pub revenue_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_config.bump,
        has_one = operator @ VertaError::Unauthorized,
        constraint = !platform_config.paused @ VertaError::PlatformPaused,
        constraint = platform_config.revenue_share_collection != Pubkey::default() @ VertaError::WrongCollection
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Contribute<'info> {
    #[account(
        mut,
        seeds = [CROWDFUND_SEED, crowdfund.series_id.as_bytes()],
        bump = crowdfund.bump,
        constraint = crowdfund.status == CrowdfundStatus::Active @ VertaError::CampaignInactive,
        constraint = Clock::get()?.unix_timestamp < crowdfund.deadline @ VertaError::CampaignEnded
    )]
    pub crowdfund: Account<'info, Crowdfund>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + Contribution::INIT_SPACE,
        seeds = [CONTRIBUTION_SEED, crowdfund.key().as_ref(), user.key().as_ref()],
        bump
    )]
    pub contribution: Account<'info, Contribution>,

    pub user: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(address = crowdfund.mint @ VertaError::InvalidMint)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        token::mint = mint,
        token::authority = user,
        token::token_program = token_program,
    )]
    pub user_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [ESCROW_VAULT_SEED, crowdfund.key().as_ref()],
        bump,
        token::mint = mint,
        token::token_program = token_program,
    )]
    pub escrow_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_config.bump,
        constraint = !platform_config.paused @ VertaError::PlatformPaused
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FinalizeCampaign<'info> {
    #[account(
        mut,
        seeds = [CROWDFUND_SEED, crowdfund.series_id.as_bytes()],
        bump = crowdfund.bump,
        constraint = crowdfund.status == CrowdfundStatus::Active @ VertaError::CampaignInactive,
    )]
    pub crowdfund: Account<'info, Crowdfund>,

    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_config.bump,
        has_one = admin @ VertaError::Unauthorized,
        constraint = !platform_config.paused @ VertaError::PlatformPaused
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(address = crowdfund.mint @ VertaError::InvalidMint)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        seeds = [ESCROW_VAULT_SEED, crowdfund.key().as_ref()],
        bump,
        token::mint = mint,
        token::token_program = token_program,
    )]
    pub escrow_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Production wallet owner; validated against campaign state.
    #[account(address = crowdfund.production_wallet @ VertaError::InvalidRecipient)]
    pub production_wallet: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = admin,
        associated_token::mint = mint,
        associated_token::authority = production_wallet,
        associated_token::token_program = token_program,
    )]
    pub production_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct HaltCampaign<'info> {
    #[account(
        mut,
        seeds = [CROWDFUND_SEED, crowdfund.series_id.as_bytes()],
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
        seeds = [CROWDFUND_SEED, crowdfund.series_id.as_bytes()],
        bump = crowdfund.bump,
        constraint = crowdfund.status == CrowdfundStatus::Refunded
            || crowdfund.status == CrowdfundStatus::Cancelled @ VertaError::CampaignInactive,
    )]
    pub crowdfund: Account<'info, Crowdfund>,

    #[account(
        mut,
        seeds = [CONTRIBUTION_SEED, crowdfund.key().as_ref(), user.key().as_ref()],
        bump = contribution.bump,
        has_one = user @ VertaError::Unauthorized,
        has_one = rent_payer @ VertaError::InvalidRecipient,
        close = rent_payer
    )]
    pub contribution: Account<'info, Contribution>,

    pub user: Signer<'info>,

    /// CHECK: Receives the closed contribution rent; bound by has_one.
    #[account(mut)]
    pub rent_payer: UncheckedAccount<'info>,

    #[account(address = crowdfund.mint @ VertaError::InvalidMint)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        token::mint = mint,
        token::authority = user,
        token::token_program = token_program,
    )]
    pub user_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [ESCROW_VAULT_SEED, crowdfund.key().as_ref()],
        bump,
        token::mint = mint,
        token::token_program = token_program,
    )]
    pub escrow_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_config.bump,
        constraint = !platform_config.paused @ VertaError::PlatformPaused
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MintRevenueShare<'info> {
    #[account(
        mut,
        seeds = [CROWDFUND_SEED, crowdfund.series_id.as_bytes()],
        bump = crowdfund.bump,
        constraint = crowdfund.status == CrowdfundStatus::Succeeded @ VertaError::CampaignInactive,
    )]
    pub crowdfund: Account<'info, Crowdfund>,

    #[account(
        mut,
        seeds = [CONTRIBUTION_SEED, crowdfund.key().as_ref(), user.key().as_ref()],
        bump = contribution.bump,
        constraint = !contribution.minted @ VertaError::AlreadyMinted,
        has_one = user @ VertaError::Unauthorized,
    )]
    pub contribution: Account<'info, Contribution>,

    #[account(
        init,
        payer = payer,
        space = 8 + SharePosition::INIT_SPACE,
        seeds = [POSITION_SEED, asset.key().as_ref()],
        bump
    )]
    pub position: Account<'info, SharePosition>,

    pub user: Signer<'info>,

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
        constraint = !platform_config.paused @ VertaError::PlatformPaused,
        constraint = platform_config.revenue_share_collection == collection.key() @ VertaError::WrongCollection
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    /// CHECK: Metaplex Core program
    #[account(address = mpl_core::ID)]
    pub core_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimRevenue<'info> {
    #[account(
        mut,
        seeds = [CROWDFUND_SEED, crowdfund.series_id.as_bytes()],
        bump = crowdfund.bump,
        constraint = crowdfund.status == CrowdfundStatus::Succeeded @ VertaError::CampaignInactive,
    )]
    pub crowdfund: Account<'info, Crowdfund>,

    #[account(address = crowdfund.mint @ VertaError::InvalidMint)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        seeds = [REVENUE_VAULT_SEED, crowdfund.key().as_ref()],
        bump,
        token::mint = mint,
        token::token_program = token_program,
    )]
    pub revenue_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [POSITION_SEED, asset.key().as_ref()],
        bump = position.bump,
        constraint = position.crowdfund == crowdfund.key() @ VertaError::WrongCollection,
        constraint = position.asset == asset.key() @ VertaError::InvalidAsset,
    )]
    pub position: Account<'info, SharePosition>,

    /// CHECK: Metaplex Core asset; owner and bytes verified in handler.
    #[account(owner = mpl_core::ID @ VertaError::InvalidAsset)]
    pub asset: UncheckedAccount<'info>,

    pub user: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = user,
        associated_token::token_program = token_program,
    )]
    pub user_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_config.bump,
        constraint = !platform_config.paused @ VertaError::PlatformPaused
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DepositRevenue<'info> {
    #[account(
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_config.bump,
        has_one = operator @ VertaError::Unauthorized,
        constraint = !platform_config.paused @ VertaError::PlatformPaused
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(mut)]
    pub operator: Signer<'info>,

    #[account(
        mut,
        seeds = [CROWDFUND_SEED, crowdfund.series_id.as_bytes()],
        bump = crowdfund.bump,
        constraint = crowdfund.status == CrowdfundStatus::Succeeded @ VertaError::CampaignInactive,
    )]
    pub crowdfund: Account<'info, Crowdfund>,

    #[account(address = crowdfund.mint @ VertaError::InvalidMint)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        token::mint = mint,
        token::authority = operator,
        token::token_program = token_program,
    )]
    pub operator_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [REVENUE_VAULT_SEED, crowdfund.key().as_ref()],
        bump,
        token::mint = mint,
        token::token_program = token_program,
    )]
    pub revenue_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Production wallet owner; validated against campaign state.
    #[account(address = crowdfund.production_wallet @ VertaError::InvalidRecipient)]
    pub production_wallet: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = operator,
        associated_token::mint = mint,
        associated_token::authority = production_wallet,
        associated_token::token_program = token_program,
    )]
    pub production_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_crowdfund(
    ctx: Context<InitializeCrowdfund>,
    series_id: String,
    goal: u64,
    deadline: i64,
    production_wallet: Pubkey,
    backer_revenue_bps: u16,
) -> Result<()> {
    require!(
        !series_id.is_empty() && series_id.len() <= MAX_TITLE_ID_LEN,
        VertaError::InvalidTitleId
    );
    require!(goal > 0, VertaError::InvalidAmount);
    require!(
        deadline > Clock::get()?.unix_timestamp,
        VertaError::InvalidDeadline
    );
    require!(
        backer_revenue_bps > 0 && backer_revenue_bps <= BPS_DENOMINATOR,
        VertaError::InvalidBasisPoints
    );
    require!(
        production_wallet != Pubkey::default(),
        VertaError::InvalidRecipient
    );

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
    require!(amount > 0, VertaError::InvalidAmount);

    let escrow_before = ctx.accounts.escrow_vault.amount;
    transfer_tokens(
        &ctx.accounts.token_program,
        &ctx.accounts.user_ata,
        &ctx.accounts.escrow_vault,
        &ctx.accounts.mint,
        ctx.accounts.user.to_account_info(),
        amount,
        None,
    )?;
    assert_received(&mut ctx.accounts.escrow_vault, escrow_before, amount)?;

    let crowdfund = &mut ctx.accounts.crowdfund;
    let contribution = &mut ctx.accounts.contribution;

    crowdfund.total_raised = crowdfund
        .total_raised
        .checked_add(amount)
        .ok_or(VertaError::MathOverflow)?;

    if contribution.user == Pubkey::default() {
        contribution.user = ctx.accounts.user.key();
        contribution.rent_payer = ctx.accounts.payer.key();
        contribution.amount_contributed = 0;
        contribution.minted = false;
        contribution.bump = ctx.bumps.contribution;
    }

    contribution.amount_contributed = contribution
        .amount_contributed
        .checked_add(amount)
        .ok_or(VertaError::MathOverflow)?;

    emit!(ContributionEvent {
        series_id: crowdfund.series_id.clone(),
        user: ctx.accounts.user.key(),
        amount,
        total_raised: crowdfund.total_raised,
    });

    Ok(())
}

pub fn finalize_campaign(ctx: Context<FinalizeCampaign>) -> Result<()> {
    require!(
        ctx.accounts.crowdfund.total_raised >= ctx.accounts.crowdfund.goal,
        VertaError::GoalNotReached
    );

    let series_id = ctx.accounts.crowdfund.series_id.clone();
    let bump = [ctx.accounts.crowdfund.bump];
    let seeds: &[&[u8]] = &[CROWDFUND_SEED, series_id.as_bytes(), &bump];

    let withdrawable = ctx.accounts.escrow_vault.amount;
    if withdrawable > 0 {
        let production_before = ctx.accounts.production_ata.amount;
        transfer_tokens(
            &ctx.accounts.token_program,
            &ctx.accounts.escrow_vault,
            &ctx.accounts.production_ata,
            &ctx.accounts.mint,
            ctx.accounts.crowdfund.to_account_info(),
            withdrawable,
            Some(&[seeds]),
        )?;
        assert_received(
            &mut ctx.accounts.production_ata,
            production_before,
            withdrawable,
        )?;
    }

    let crowdfund = &mut ctx.accounts.crowdfund;
    crowdfund.status = CrowdfundStatus::Succeeded;
    crowdfund.total_issued_bps = crowdfund.backer_revenue_bps;
    crowdfund.accumulator_at_finalize = crowdfund.cumulative_revenue_per_bps;
    Ok(())
}

pub fn fail_campaign(ctx: Context<HaltCampaign>) -> Result<()> {
    let crowdfund = &mut ctx.accounts.crowdfund;
    let now = Clock::get()?.unix_timestamp;
    require!(now >= crowdfund.deadline, VertaError::CampaignNotEnded);
    require!(
        crowdfund.total_raised < crowdfund.goal,
        VertaError::GoalAlreadyReached
    );
    crowdfund.status = CrowdfundStatus::Refunded;
    Ok(())
}

pub fn cancel_campaign(ctx: Context<HaltCampaign>) -> Result<()> {
    let crowdfund = &mut ctx.accounts.crowdfund;
    let now = Clock::get()?.unix_timestamp;
    require!(now < crowdfund.deadline, VertaError::CampaignDeadlinePassed);
    crowdfund.status = CrowdfundStatus::Cancelled;
    Ok(())
}

pub fn refund(ctx: Context<Refund>) -> Result<()> {
    let amount = ctx.accounts.contribution.amount_contributed;
    require!(
        ctx.accounts.escrow_vault.amount >= amount,
        VertaError::InsufficientVaultBalance
    );

    let series_id = ctx.accounts.crowdfund.series_id.clone();
    let bump = [ctx.accounts.crowdfund.bump];
    let seeds: &[&[u8]] = &[CROWDFUND_SEED, series_id.as_bytes(), &bump];

    let user_before = ctx.accounts.user_ata.amount;
    transfer_tokens(
        &ctx.accounts.token_program,
        &ctx.accounts.escrow_vault,
        &ctx.accounts.user_ata,
        &ctx.accounts.mint,
        ctx.accounts.crowdfund.to_account_info(),
        amount,
        Some(&[seeds]),
    )?;
    assert_received(&mut ctx.accounts.user_ata, user_before, amount)?;

    let crowdfund = &mut ctx.accounts.crowdfund;
    crowdfund.total_raised = crowdfund
        .total_raised
        .checked_sub(amount)
        .ok_or(VertaError::MathOverflow)?;
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
    Ok(())
}

pub fn mint_revenue_share(ctx: Context<MintRevenueShare>) -> Result<()> {
    let crowdfund = &ctx.accounts.crowdfund;
    let contribution = &mut ctx.accounts.contribution;
    let position = &mut ctx.accounts.position;

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
    let attributes = vec![
        Attribute {
            key: "share_bps".to_string(),
            value: share_bps.to_string(),
        },
        Attribute {
            key: "series_id".to_string(),
            value: crowdfund.series_id.clone(),
        },
    ];

    let platform_bump = [ctx.accounts.platform_config.bump];
    let platform_seeds: &[&[u8]] = &[PLATFORM_AUTHORITY_SEED, &platform_bump];
    let signer_seeds = &[platform_seeds];

    CreateV1CpiBuilder::new(&ctx.accounts.core_program.to_account_info())
        .asset(&ctx.accounts.asset.to_account_info())
        .collection(Some(&ctx.accounts.collection.to_account_info()))
        .authority(Some(&ctx.accounts.platform_config.to_account_info()))
        .payer(&ctx.accounts.payer.to_account_info())
        .owner(Some(&ctx.accounts.user.to_account_info()))
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
    Ok(())
}

pub fn claim_revenue(ctx: Context<ClaimRevenue>) -> Result<()> {
    let asset_binding = ctx.accounts.asset.to_account_info();
    let asset_data = asset_binding.try_borrow_data()?;
    let asset_info = BaseAssetV1::from_bytes(&asset_data).map_err(|_| VertaError::InvalidAsset)?;
    require!(
        asset_info.owner == ctx.accounts.user.key(),
        VertaError::NotOwner
    );
    drop(asset_data);

    let crowdfund = &ctx.accounts.crowdfund;
    let position = &ctx.accounts.position;
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

    let series_id = crowdfund.series_id.clone();
    let bump = [crowdfund.bump];
    let seeds: &[&[u8]] = &[CROWDFUND_SEED, series_id.as_bytes(), &bump];
    let new_checkpoint = crowdfund.cumulative_revenue_per_bps;

    let user_before = ctx.accounts.user_ata.amount;
    transfer_tokens(
        &ctx.accounts.token_program,
        &ctx.accounts.revenue_vault,
        &ctx.accounts.user_ata,
        &ctx.accounts.mint,
        ctx.accounts.crowdfund.to_account_info(),
        claimable,
        Some(&[seeds]),
    )?;
    assert_received(&mut ctx.accounts.user_ata, user_before, claimable)?;

    ctx.accounts.position.claimed_checkpoint = new_checkpoint;
    Ok(())
}

pub fn deposit_revenue(ctx: Context<DepositRevenue>, amount: u64) -> Result<()> {
    require!(amount > 0, VertaError::InvalidAmount);

    let backer_revenue_bps = ctx.accounts.crowdfund.backer_revenue_bps;
    let total_issued_bps = ctx.accounts.crowdfund.total_issued_bps;

    let backer_portion = (amount as u128)
        .checked_mul(backer_revenue_bps as u128)
        .ok_or(VertaError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128)
        .ok_or(VertaError::MathOverflow)? as u64;
    let production_portion = amount
        .checked_sub(backer_portion)
        .ok_or(VertaError::MathOverflow)?;

    if backer_portion > 0 {
        require!(total_issued_bps > 0, VertaError::MathOverflow);
        let vault_before = ctx.accounts.revenue_vault.amount;
        transfer_tokens(
            &ctx.accounts.token_program,
            &ctx.accounts.operator_ata,
            &ctx.accounts.revenue_vault,
            &ctx.accounts.mint,
            ctx.accounts.operator.to_account_info(),
            backer_portion,
            None,
        )?;
        assert_received(&mut ctx.accounts.revenue_vault, vault_before, backer_portion)?;

        let accumulator_delta = (backer_portion as u128)
            .checked_mul(SCALE)
            .ok_or(VertaError::MathOverflow)?
            .checked_div(total_issued_bps as u128)
            .ok_or(VertaError::MathOverflow)?;
        let crowdfund = &mut ctx.accounts.crowdfund;
        crowdfund.cumulative_revenue_per_bps = crowdfund
            .cumulative_revenue_per_bps
            .checked_add(accumulator_delta)
            .ok_or(VertaError::MathOverflow)?;
    }

    if production_portion > 0 {
        let production_before = ctx.accounts.production_ata.amount;
        transfer_tokens(
            &ctx.accounts.token_program,
            &ctx.accounts.operator_ata,
            &ctx.accounts.production_ata,
            &ctx.accounts.mint,
            ctx.accounts.operator.to_account_info(),
            production_portion,
            None,
        )?;
        assert_received(
            &mut ctx.accounts.production_ata,
            production_before,
            production_portion,
        )?;
    }

    Ok(())
}

fn transfer_tokens<'info>(
    token_program: &Interface<'info, TokenInterface>,
    from: &InterfaceAccount<'info, TokenAccount>,
    to: &InterfaceAccount<'info, TokenAccount>,
    mint: &InterfaceAccount<'info, Mint>,
    authority: AccountInfo<'info>,
    amount: u64,
    signer_seeds: Option<&[&[&[u8]]]>,
) -> Result<()> {
    let accounts = TransferChecked {
        from: from.to_account_info(),
        mint: mint.to_account_info(),
        to: to.to_account_info(),
        authority,
    };
    let program = token_program.to_account_info();
    let cpi = match signer_seeds {
        Some(seeds) => CpiContext::new_with_signer(program, accounts, seeds),
        None => CpiContext::new(program, accounts),
    };
    token_interface::transfer_checked(cpi, amount, mint.decimals)
}

fn assert_received<'info>(
    account: &mut InterfaceAccount<'info, TokenAccount>,
    before: u64,
    amount: u64,
) -> Result<()> {
    account.reload()?;
    let expected = before
        .checked_add(amount)
        .ok_or(VertaError::MathOverflow)?;
    require!(account.amount == expected, VertaError::TransferAmountMismatch);
    Ok(())
}

#[event]
pub struct ContributionEvent {
    pub series_id: String,
    pub user: Pubkey,
    pub amount: u64,
    pub total_raised: u64,
}
