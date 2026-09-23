use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod instructions;
pub mod state;

use instructions::*;

declare_id!("A3CuYy9r2tpU4BafDSEbasuvFJAG3Nvet8oKAv6J1Pzr");

#[program]
pub mod verta_core {
    use super::*;

    pub fn initialize_platform(
        ctx: Context<InitializePlatform>,
        admin: Pubkey,
        operator: Pubkey,
    ) -> Result<()> {
        instructions::admin::initialize_platform(ctx, admin, operator)
    }

    pub fn update_platform_config(
        ctx: Context<UpdatePlatformConfig>,
        new_operator: Option<Pubkey>,
        paused: Option<bool>,
    ) -> Result<()> {
        instructions::admin::update_platform_config(ctx, new_operator, paused)
    }

    pub fn pause(ctx: Context<Pause>) -> Result<()> {
        instructions::admin::pause(ctx)
    }

    pub fn propose_admin(ctx: Context<ProposeAdmin>, new_admin: Pubkey) -> Result<()> {
        instructions::admin::propose_admin(ctx, new_admin)
    }

    pub fn accept_admin(ctx: Context<AcceptAdmin>) -> Result<()> {
        instructions::admin::accept_admin(ctx)
    }

    pub fn record_credit_epoch(
        ctx: Context<RecordCreditEpoch>,
        epoch_day: u32,
        coins_sold: u64,
        coins_spent: u64,
        coins_refunded: u64,
        usd_liability_micro: u64,
        balances_root: [u8; 32],
    ) -> Result<()> {
        instructions::credits::record_credit_epoch(
            ctx,
            epoch_day,
            coins_sold,
            coins_spent,
            coins_refunded,
            usd_liability_micro,
            balances_root,
        )
    }

    pub fn create_title_config(
        ctx: Context<CreateTitleConfig>,
        title_id: String,
        creator_wallet: Pubkey,
        creator_bps: u16,
        platform_bps: u16,
        active: bool,
    ) -> Result<()> {
        instructions::series::create_title_config(
            ctx,
            title_id,
            creator_wallet,
            creator_bps,
            platform_bps,
            active,
        )
    }

    pub fn update_title_config(
        ctx: Context<UpdateTitleConfig>,
        new_creator_wallet: Option<Pubkey>,
        new_creator_bps: Option<u16>,
        new_platform_bps: Option<u16>,
        new_active: Option<bool>,
    ) -> Result<()> {
        instructions::series::update_title_config(
            ctx,
            new_creator_wallet,
            new_creator_bps,
            new_platform_bps,
            new_active,
        )
    }

    pub fn create_revenue_share_collection(
        ctx: Context<CreateRevenueShareCollection>,
    ) -> Result<()> {
        instructions::crowdfunding::create_revenue_share_collection(ctx)
    }

    pub fn initialize_crowdfund(
        ctx: Context<InitializeCrowdfund>,
        series_id: String,
        goal: u64,
        deadline: i64,
        production_wallet: Pubkey,
        backer_revenue_bps: u16,
    ) -> Result<()> {
        instructions::crowdfunding::initialize_crowdfund(
            ctx,
            series_id,
            goal,
            deadline,
            production_wallet,
            backer_revenue_bps,
        )
    }

    pub fn contribute(ctx: Context<Contribute>, amount: u64) -> Result<()> {
        instructions::crowdfunding::contribute(ctx, amount)
    }

    pub fn finalize_campaign(ctx: Context<FinalizeCampaign>) -> Result<()> {
        instructions::crowdfunding::finalize_campaign(ctx)
    }

    pub fn fail_campaign(ctx: Context<HaltCampaign>) -> Result<()> {
        instructions::crowdfunding::fail_campaign(ctx)
    }

    pub fn cancel_campaign(ctx: Context<HaltCampaign>) -> Result<()> {
        instructions::crowdfunding::cancel_campaign(ctx)
    }

    pub fn refund(ctx: Context<Refund>) -> Result<()> {
        instructions::crowdfunding::refund(ctx)
    }

    pub fn mint_revenue_share(ctx: Context<MintRevenueShare>) -> Result<()> {
        instructions::crowdfunding::mint_revenue_share(ctx)
    }

    pub fn deposit_revenue(ctx: Context<DepositRevenue>, amount: u64) -> Result<()> {
        instructions::crowdfunding::deposit_revenue(ctx, amount)
    }

    pub fn claim_revenue(ctx: Context<ClaimRevenue>) -> Result<()> {
        instructions::crowdfunding::claim_revenue(ctx)
    }
}
