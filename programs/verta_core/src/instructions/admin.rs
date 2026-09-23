use crate::constants::*;
use crate::errors::*;
use crate::program::VertaCore;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

pub fn initialize_platform(
    ctx: Context<InitializePlatform>,
    admin: Pubkey,
    operator: Pubkey,
) -> Result<()> {
    require!(admin != Pubkey::default(), VertaError::InvalidRecipient);
    require!(operator != Pubkey::default(), VertaError::InvalidRecipient);

    let config = &mut ctx.accounts.platform_config;
    config.admin = admin;
    config.pending_admin = Pubkey::default();
    config.operator = operator;
    config.settlement_mint = ctx.accounts.settlement_mint.key();
    config.revenue_share_collection = Pubkey::default();
    config.paused = false;
    config.bump = ctx.bumps.platform_config;
    Ok(())
}

pub fn update_platform_config(
    ctx: Context<UpdatePlatformConfig>,
    new_operator: Option<Pubkey>,
    paused: Option<bool>,
) -> Result<()> {
    let config = &mut ctx.accounts.platform_config;

    if let Some(operator) = new_operator {
        require!(operator != Pubkey::default(), VertaError::InvalidRecipient);
        config.operator = operator;
    }
    if let Some(mint) = &ctx.accounts.new_settlement_mint {
        config.settlement_mint = mint.key();
    }
    if let Some(value) = paused {
        config.paused = value;
    }
    Ok(())
}

pub fn pause(ctx: Context<Pause>) -> Result<()> {
    ctx.accounts.platform_config.paused = true;
    Ok(())
}

pub fn propose_admin(ctx: Context<ProposeAdmin>, new_admin: Pubkey) -> Result<()> {
    require!(new_admin != Pubkey::default(), VertaError::InvalidRecipient);
    ctx.accounts.platform_config.pending_admin = new_admin;
    Ok(())
}

pub fn accept_admin(ctx: Context<AcceptAdmin>) -> Result<()> {
    let config = &mut ctx.accounts.platform_config;
    config.admin = ctx.accounts.pending_admin.key();
    config.pending_admin = Pubkey::default();
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

    pub settlement_mint: InterfaceAccount<'info, Mint>,

    #[account(
        constraint = program.programdata_address()? == Some(program_data.key()) @ VertaError::NotUpgradeAuthority
    )]
    pub program: Program<'info, VertaCore>,

    #[account(
        constraint = program_data.upgrade_authority_address == Some(payer.key()) @ VertaError::NotUpgradeAuthority
    )]
    pub program_data: Account<'info, ProgramData>,

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

    pub new_settlement_mint: Option<InterfaceAccount<'info, Mint>>,
}

#[derive(Accounts)]
pub struct Pause<'info> {
    #[account(
        mut,
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_config.bump,
        constraint = authority.key() == platform_config.operator
            || authority.key() == platform_config.admin @ VertaError::Unauthorized
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct ProposeAdmin<'info> {
    #[account(
        mut,
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_config.bump,
        has_one = admin @ VertaError::Unauthorized
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct AcceptAdmin<'info> {
    #[account(
        mut,
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_config.bump,
        constraint = platform_config.pending_admin != Pubkey::default() @ VertaError::NoPendingAdmin,
        constraint = platform_config.pending_admin == pending_admin.key() @ VertaError::Unauthorized
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    pub pending_admin: Signer<'info>,
}
