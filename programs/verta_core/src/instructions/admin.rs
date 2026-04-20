use anchor_lang::prelude::*;
use crate::state::*;
use crate::constants::*;
use crate::errors::*;

pub fn initialize_platform(
    ctx: Context<InitializePlatform>, 
    admin: Pubkey, 
    platform_wallet: Pubkey,
    price_sol: u64,
    creator_bps: u16,
    platform_bps: u16,
) -> Result<()> {
    require!(creator_bps + platform_bps == BPS_DENOMINATOR, VertaError::InvalidBasisPoints);
    
    let config = &mut ctx.accounts.platform_config;
    config.admin = admin;
    config.platform_wallet = platform_wallet;
    config.paused = false;
    config.price_sol = price_sol;
    config.creator_bps = creator_bps;
    config.platform_bps = platform_bps;
    config.bump = ctx.bumps.platform_config;
    Ok(())
}

pub fn update_platform_config(
    ctx: Context<UpdatePlatformConfig>,
    new_admin: Option<Pubkey>,
    new_wallet: Option<Pubkey>,
    new_price: Option<u64>,
    new_creator_bps: Option<u16>,
    new_platform_bps: Option<u16>,
    paused: Option<bool>,
) -> Result<()> {
    let config = &mut ctx.accounts.platform_config;
    
    if let Some(val) = new_admin { config.admin = val; }
    if let Some(val) = new_wallet { config.platform_wallet = val; }
    if let Some(val) = new_price { config.price_sol = val; }
    if let Some(p) = paused { config.paused = p; }
    
    if let (Some(cbps), Some(pbps)) = (new_creator_bps, new_platform_bps) {
        require!(cbps + pbps == BPS_DENOMINATOR, VertaError::InvalidBasisPoints);
        config.creator_bps = cbps;
        config.platform_bps = pbps;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct InitializePlatform<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + PlatformConfig::INIT_SPACE,
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump
    )]
    pub platform_config: Account<'info, PlatformConfig>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdatePlatformConfig<'info> {
    #[account(
        mut,
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_config.bump,
        has_one = admin @ VertaError::Unauthorized
    )]
    pub platform_config: Account<'info, PlatformConfig>,
    pub admin: Signer<'info>,
}
