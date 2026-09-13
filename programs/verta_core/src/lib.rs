use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod instructions;
pub mod state;

use instructions::*;

declare_id!("D2FTBiHZkkaGAQzLcivACqEuZnSAKatZzCg8ej5bVzUT");

#[program]
pub mod verta_core {
    use super::*;

    /// Platform Admin: Initialize the Verta Protocol
    pub fn initialize_platform(
        ctx: Context<InitializePlatform>,
        admin: Pubkey,
        platform_wallet: Pubkey,
        price_sol: u64,
        creator_bps: u16,
        platform_bps: u16,
    ) -> Result<()> {
        instructions::admin::initialize_platform(
            ctx,
            admin,
            platform_wallet,
            price_sol,
            creator_bps,
            platform_bps,
        )
    }

    /// Platform Admin: Update global configuration (Prices, Splits, etc)
    pub fn update_platform_config(
        ctx: Context<UpdatePlatformConfig>,
        new_admin: Option<Pubkey>,
        new_wallet: Option<Pubkey>,
        new_price: Option<u64>,
        new_creator_bps: Option<u16>,
        new_platform_bps: Option<u16>,
        paused: Option<bool>,
    ) -> Result<()> {
        instructions::admin::update_platform_config(
            ctx,
            new_admin,
            new_wallet,
            new_price,
            new_creator_bps,
            new_platform_bps,
            paused,
        )
    }

    /// Platform Admin: Record (or re-record) the daily coin-economy checkpoint
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

    /// Platform Admin: Register a new series and set its specific royalty split
    pub fn create_title_config(
        ctx: Context<CreateTitleConfig>,
        title_id: String,
        authority: Pubkey,
        creator_wallet: Pubkey,
        creator_bps: u16,
        platform_bps: u16,
        active: bool,
    ) -> Result<()> {
        instructions::series::create_title_config(
            ctx,
            title_id,
            authority,
            creator_wallet,
            creator_bps,
            platform_bps,
            active,
        )
    }

    /// Platform Admin: Update series configuration
    pub fn update_title_config(
        ctx: Context<UpdateTitleConfig>,
        new_authority: Option<Pubkey>,
        new_creator_wallet: Option<Pubkey>,
        new_creator_bps: Option<u16>,
        new_platform_bps: Option<u16>,
        new_active: Option<bool>,
    ) -> Result<()> {
        instructions::series::update_title_config(
            ctx,
            new_authority,
            new_creator_wallet,
            new_creator_bps,
            new_platform_bps,
            new_active,
        )
    }

    /// User: Initialize personal SOL vault
    pub fn initialize_user_vault(ctx: Context<InitializeUserVault>) -> Result<()> {
        instructions::vault::initialize_user_vault(ctx)
    }

    /// User: Top-up vault with native SOL
    pub fn deposit_sol(ctx: Context<DepositSol>, amount: u64) -> Result<()> {
        instructions::vault::deposit_sol(ctx, amount)
    }

    /// User: Withdraw native SOL from vault
    pub fn withdraw_sol(ctx: Context<WithdrawSol>, amount: u64) -> Result<()> {
        instructions::vault::withdraw_sol(ctx, amount)
    }

    /// User: Authorize/Deauthorize a backend delegate
    pub fn set_vault_delegate(
        ctx: Context<SetVaultDelegate>,
        delegate: Pubkey,
        enabled: bool,
    ) -> Result<()> {
        instructions::vault::set_vault_delegate(ctx, delegate, enabled)
    }

    /// Backend: Trigger an automatic episode unlock from user vault
    pub fn auto_unlock_episode(ctx: Context<AutoUnlockEpisode>, episode_id: String) -> Result<()> {
        instructions::autoplay::auto_unlock_episode(ctx, episode_id)
    }

    /// Frictionless Backend Unlock: Payout only, no on-chain receipt
    pub fn direct_payout(ctx: Context<DirectPayout>, episode_id: String) -> Result<()> {
        instructions::autoplay::direct_payout(ctx, episode_id)
    }

    /// Admin: Initialize a new crowdfunding campaign
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

    /// Admin: Create the global Metaplex Core collection for Revenue Share Passes
    pub fn create_revenue_share_collection(
        ctx: Context<CreateRevenueShareCollection>,
    ) -> Result<()> {
        instructions::crowdfunding::create_revenue_share_collection(ctx)
    }

    /// User: Contribute $SOL to a crowdfunding campaign
    pub fn contribute(ctx: Context<Contribute>, amount: u64) -> Result<()> {
        instructions::crowdfunding::contribute(ctx, amount)
    }

    /// Admin: Finalize a successful campaign and release funds
    pub fn finalize_campaign(ctx: Context<FinalizeCampaign>) -> Result<()> {
        instructions::crowdfunding::finalize_campaign(ctx)
    }

    /// Admin: Mark a campaign as failed after its deadline (enabling refunds)
    pub fn fail_campaign(ctx: Context<HaltCampaign>) -> Result<()> {
        instructions::crowdfunding::fail_campaign(ctx)
    }

    /// Admin: Cancel an active campaign (enabling refunds)
    pub fn cancel_campaign(ctx: Context<HaltCampaign>) -> Result<()> {
        instructions::crowdfunding::cancel_campaign(ctx)
    }

    /// User: Claim refund from a failed campaign
    pub fn refund(ctx: Context<Refund>) -> Result<()> {
        instructions::crowdfunding::refund(ctx)
    }

    /// User: Mint their Revenue Share Pass after a successful campaign
    pub fn mint_revenue_share(ctx: Context<MintRevenueShare>) -> Result<()> {
        instructions::crowdfunding::mint_revenue_share(ctx)
    }

    /// Backend: Record graduated crowdfund revenue through the delegated unlock path
    pub fn record_revenue(ctx: Context<RecordRevenue>, amount: u64) -> Result<()> {
        instructions::crowdfunding::record_revenue(ctx, amount)
    }

    /// Admin: Deposit settled episode revenue (USDC) into a graduated
    /// campaign — backer share into the revenue vault + accumulator bump,
    /// remainder to the production wallet
    pub fn deposit_revenue(ctx: Context<DepositRevenue>, amount: u64) -> Result<()> {
        instructions::crowdfunding::deposit_revenue(ctx, amount)
    }

    /// User: Claim their share of accumulated revenue using their Revenue Share Pass
    pub fn claim_revenue(ctx: Context<ClaimRevenue>) -> Result<()> {
        instructions::crowdfunding::claim_revenue(ctx)
    }
}
