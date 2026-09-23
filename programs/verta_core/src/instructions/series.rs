use crate::constants::*;
use crate::errors::*;
use crate::state::*;
use anchor_lang::prelude::*;

pub fn create_title_config(
    ctx: Context<CreateTitleConfig>,
    title_id: String,
    creator_wallet: Pubkey,
    creator_bps: u16,
    platform_bps: u16,
    active: bool,
) -> Result<()> {
    require!(
        !title_id.is_empty() && title_id.len() <= MAX_TITLE_ID_LEN,
        VertaError::InvalidTitleId
    );
    require!(
        creator_bps
            .checked_add(platform_bps)
            .ok_or(VertaError::MathOverflow)?
            == BPS_DENOMINATOR,
        VertaError::InvalidBasisPoints
    );
    require!(
        creator_wallet != Pubkey::default(),
        VertaError::InvalidRecipient
    );

    let title_config = &mut ctx.accounts.title_config;
    title_config.title_id = title_id;
    title_config.creator_wallet = creator_wallet;
    title_config.creator_bps = creator_bps;
    title_config.platform_bps = platform_bps;
    title_config.active = active;
    title_config.bump = ctx.bumps.title_config;
    Ok(())
}

pub fn update_title_config(
    ctx: Context<UpdateTitleConfig>,
    new_creator_wallet: Option<Pubkey>,
    new_creator_bps: Option<u16>,
    new_platform_bps: Option<u16>,
    new_active: Option<bool>,
) -> Result<()> {
    let title_config = &mut ctx.accounts.title_config;

    if let Some(wallet) = new_creator_wallet {
        require!(wallet != Pubkey::default(), VertaError::InvalidRecipient);
        title_config.creator_wallet = wallet;
    }
    if let (Some(cbps), Some(pbps)) = (new_creator_bps, new_platform_bps) {
        require!(
            cbps.checked_add(pbps).ok_or(VertaError::MathOverflow)? == BPS_DENOMINATOR,
            VertaError::InvalidBasisPoints
        );
        title_config.creator_bps = cbps;
        title_config.platform_bps = pbps;
    }
    if let Some(active) = new_active {
        title_config.active = active;
    }
    Ok(())
}

#[derive(Accounts)]
#[instruction(title_id: String)]
pub struct CreateTitleConfig<'info> {
    #[account(
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_config.bump,
        has_one = operator @ VertaError::Unauthorized
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(
        init,
        payer = operator,
        space = TitleConfig::MAX_SIZE,
        seeds = [TITLE_CONFIG_SEED, title_id.as_bytes()],
        bump
    )]
    pub title_config: Account<'info, TitleConfig>,

    #[account(mut)]
    pub operator: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateTitleConfig<'info> {
    #[account(
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_config.bump,
        has_one = operator @ VertaError::Unauthorized
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(
        mut,
        seeds = [TITLE_CONFIG_SEED, title_config.title_id.as_bytes()],
        bump = title_config.bump,
    )]
    pub title_config: Account<'info, TitleConfig>,

    pub operator: Signer<'info>,
}
