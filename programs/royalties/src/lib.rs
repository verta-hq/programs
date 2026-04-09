use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, TokenAccount, TransferChecked};

declare_id!("HU9MebBjowZV3Hyr3GagXM2Pzx3xNdwbQbZqsJK3ZMcC");

#[program]
pub mod royalties {
    use super::*;

    /// Initializes the global platform authority.
    pub fn initialize_platform(ctx: Context<InitializePlatform>, admin: Pubkey) -> Result<()> {
        let platform_authority = &mut ctx.accounts.platform_authority;
        platform_authority.admin = admin;
        platform_authority.paused = false;
        platform_authority.bump = ctx.bumps.platform_authority;
        Ok(())
    }

    /// Toggles the platform's global paused state. Only the admin can call this.
    pub fn set_platform_paused(ctx: Context<SetPlatformPaused>, paused: bool) -> Result<()> {
        let platform_authority = &mut ctx.accounts.platform_authority;
        platform_authority.paused = paused;
        Ok(())
    }

    /// Creates a new title configuration. Only the platform admin can call this.
    pub fn create_title_config(
        ctx: Context<CreateTitleConfig>,
        title_id: String,
        authority: Pubkey,
        creator_wallet: Pubkey,
        platform_wallet: Pubkey,
        price_amount: u64,
        creator_bps: u16,
        platform_bps: u16,
        active: bool,
    ) -> Result<()> {
        // Validation
        require!(title_id.len() > 0 && title_id.len() <= MAX_TITLE_ID_LEN, VertaError::InvalidTitleId);
        require!(creator_bps + platform_bps == BPS_DENOMINATOR, VertaError::InvalidBasisPoints);
        require!(price_amount > 0, VertaError::InvalidPrice);
        require!(creator_wallet != Pubkey::default(), VertaError::InvalidRecipient);
        require!(platform_wallet != Pubkey::default(), VertaError::InvalidRecipient);

        let title_config = &mut ctx.accounts.title_config;
        title_config.title_id = title_id;
        title_config.authority = authority;
        title_config.creator_wallet = creator_wallet;
        title_config.platform_wallet = platform_wallet;
        title_config.payment_mint = ctx.accounts.payment_mint.key();
        title_config.price_amount = price_amount;
        title_config.creator_bps = creator_bps;
        title_config.platform_bps = platform_bps;
        title_config.active = active;
        title_config.bump = ctx.bumps.title_config;

        emit!(TitleConfigured {
            title_id: title_config.title_id.clone(),
            authority: title_config.authority,
            creator_wallet: title_config.creator_wallet,
            platform_wallet: title_config.platform_wallet,
            price_amount: title_config.price_amount,
        });

        Ok(())
    }

    /// Updates an existing title config. Only the title's authority can call this.
    pub fn update_title_config(
        ctx: Context<UpdateTitleConfig>,
        new_authority: Option<Pubkey>,
        new_creator_wallet: Option<Pubkey>,
        new_platform_wallet: Option<Pubkey>,
        new_price: Option<u64>,
        new_creator_bps: Option<u16>,
        new_platform_bps: Option<u16>,
        new_active: Option<bool>,
    ) -> Result<()> {
        let title_config = &mut ctx.accounts.title_config;

        if let Some(auth) = new_authority {
            title_config.authority = auth;
        }
        if let Some(cw) = new_creator_wallet {
            require!(cw != Pubkey::default(), VertaError::InvalidRecipient);
            title_config.creator_wallet = cw;
        }
        if let Some(pw) = new_platform_wallet {
            require!(pw != Pubkey::default(), VertaError::InvalidRecipient);
            title_config.platform_wallet = pw;
        }
        if let Some(price) = new_price {
            require!(price > 0, VertaError::InvalidPrice);
            title_config.price_amount = price;
        }
        if let (Some(cbps), Some(pbps)) = (new_creator_bps, new_platform_bps) {
            require!(cbps + pbps == BPS_DENOMINATOR, VertaError::InvalidBasisPoints);
            title_config.creator_bps = cbps;
            title_config.platform_bps = pbps;
        } else if let Some(cbps) = new_creator_bps {
            require!(cbps + title_config.platform_bps == BPS_DENOMINATOR, VertaError::InvalidBasisPoints);
            title_config.creator_bps = cbps;
        } else if let Some(pbps) = new_platform_bps {
            require!(title_config.creator_bps + pbps == BPS_DENOMINATOR, VertaError::InvalidBasisPoints);
            title_config.platform_bps = pbps;
        }

        if let Some(active) = new_active {
            title_config.active = active;
        }

        emit!(TitleConfigured {
            title_id: title_config.title_id.clone(),
            authority: title_config.authority,
            creator_wallet: title_config.creator_wallet,
            platform_wallet: title_config.platform_wallet,
            price_amount: title_config.price_amount,
        });

        Ok(())
    }

    /// Unlocks an episode by transferring tokens from the user to the creator and platform.
    pub fn unlock_episode(ctx: Context<UnlockEpisode>, episode_id: String) -> Result<()> {
        let platform_authority = &ctx.accounts.platform_authority;
        let title_config = &ctx.accounts.title_config;

        // 1. Validation
        require!(!platform_authority.paused, VertaError::PlatformPaused);
        require!(title_config.active, VertaError::TitleInactive);
        require!(episode_id.len() > 0 && episode_id.len() <= MAX_EPISODE_ID_LEN, VertaError::InvalidEpisodeId);
        
        // 2. Calculation
        let total_amount = title_config.price_amount;
        let creator_amount = (total_amount as u128)
            .checked_mul(title_config.creator_bps as u128)
            .ok_or(VertaError::MathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(VertaError::MathOverflow)? as u64;
        
        // Platform gets the remainder to avoid dust or loss due to rounding
        let platform_amount = total_amount.checked_sub(creator_amount).ok_or(VertaError::MathOverflow)?;

        // 3. Transfers
        // Transfer creator's share
        if creator_amount > 0 {
            token::transfer_checked(
                ctx.accounts.transfer_creator_ctx(),
                creator_amount,
                ctx.accounts.payment_mint.decimals,
            )?;
        }

        // Transfer platform's share
        if platform_amount > 0 {
            token::transfer_checked(
                ctx.accounts.transfer_platform_ctx(),
                platform_amount,
                ctx.accounts.payment_mint.decimals,
            )?;
        }

        // 4. Record Receipt
        let receipt = &mut ctx.accounts.unlock_receipt;
        receipt.title_config = title_config.key();
        receipt.user = ctx.accounts.user.key();
        receipt.episode_id = episode_id.clone();
        receipt.paid_amount = total_amount;
        receipt.payment_mint = title_config.payment_mint;
        receipt.unlocked_at = Clock::get()?.unix_timestamp;
        receipt.bump = ctx.bumps.unlock_receipt;

        // 5. Emit event
        emit!(EpisodeUnlocked {
            user: receipt.user,
            title_config: receipt.title_config,
            episode_id,
            payment_mint: receipt.payment_mint,
            total_amount,
            creator_amount,
            platform_amount,
            timestamp: receipt.unlocked_at,
        });

        Ok(())
    }
}

// --- Account Structures ---

#[account]
#[derive(InitSpace)]
pub struct PlatformAuthority {
    pub admin: Pubkey,
    pub paused: bool,
    pub bump: u8,
}

#[account]
pub struct TitleConfig {
    pub title_id: String, // Dynamic length string
    pub authority: Pubkey,
    pub creator_wallet: Pubkey,
    pub platform_wallet: Pubkey,
    pub payment_mint: Pubkey,
    pub price_amount: u64,
    pub creator_bps: u16,
    pub platform_bps: u16,
    pub active: bool,
    pub bump: u8,
}

impl TitleConfig {
    // Max length estimation for String + other fields
    pub const MAX_SIZE: usize = 8 + // discriminator
        (4 + MAX_TITLE_ID_LEN) + // title_id string
        32 + // authority
        32 + // creator_wallet
        32 + // platform_wallet
        32 + // payment_mint
        8 + // price_amount
        2 + // creator_bps
        2 + // platform_bps
        1 + // active
        1; // bump
}

#[account]
pub struct EpisodeUnlockReceipt {
    pub title_config: Pubkey,
    pub user: Pubkey,
    pub episode_id: String,
    pub paid_amount: u64,
    pub payment_mint: Pubkey,
    pub unlocked_at: i64,
    pub bump: u8,
}

impl EpisodeUnlockReceipt {
    pub const MAX_SIZE: usize = 8 + // discriminator
        32 + // title_config
        32 + // user
        (4 + MAX_EPISODE_ID_LEN) + // episode_id string
        8 + // paid_amount
        32 + // payment_mint
        8 + // unlocked_at
        1; // bump
}

// --- Contexts ---

#[derive(Accounts)]
pub struct InitializePlatform<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + PlatformAuthority::INIT_SPACE,
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump
    )]
    pub platform_authority: Account<'info, PlatformAuthority>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetPlatformPaused<'info> {
    #[account(
        mut,
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_authority.bump,
        has_one = admin @ VertaError::Unauthorized
    )]
    pub platform_authority: Account<'info, PlatformAuthority>,
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(title_id: String)]
pub struct CreateTitleConfig<'info> {
    #[account(
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_authority.bump,
        has_one = admin @ VertaError::Unauthorized
    )]
    pub platform_authority: Account<'info, PlatformAuthority>,

    #[account(
        init,
        payer = admin,
        space = TitleConfig::MAX_SIZE,
        seeds = [TITLE_CONFIG_SEED, title_id.as_bytes()],
        bump
    )]
    pub title_config: Account<'info, TitleConfig>,

    pub payment_mint: Account<'info, Mint>,

    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateTitleConfig<'info> {
    #[account(
        mut,
        seeds = [TITLE_CONFIG_SEED, title_config.title_id.as_bytes()],
        bump = title_config.bump,
        has_one = authority @ VertaError::Unauthorized
    )]
    pub title_config: Account<'info, TitleConfig>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(episode_id: String)]
pub struct UnlockEpisode<'info> {
    #[account(
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_authority.bump,
    )]
    pub platform_authority: Account<'info, PlatformAuthority>,

    #[account(
        seeds = [TITLE_CONFIG_SEED, title_config.title_id.as_bytes()],
        bump = title_config.bump,
    )]
    pub title_config: Account<'info, TitleConfig>,

    #[account(
        init,
        payer = user,
        space = EpisodeUnlockReceipt::MAX_SIZE,
        seeds = [
            UNLOCK_RECEIPT_SEED, 
            user.key().as_ref(), 
            title_config.key().as_ref(), 
            episode_id.as_bytes()
        ],
        bump
    )]
    pub unlock_receipt: Account<'info, EpisodeUnlockReceipt>,

    pub payment_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = user_token_account.owner == user.key() @ VertaError::InvalidTokenOwner,
        constraint = user_token_account.mint == payment_mint.key() @ VertaError::InvalidMint
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = creator_token_account.owner == title_config.creator_wallet @ VertaError::InvalidTokenOwner,
        constraint = creator_token_account.mint == payment_mint.key() @ VertaError::InvalidMint
    )]
    pub creator_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = platform_token_account.owner == title_config.platform_wallet @ VertaError::InvalidTokenOwner,
        constraint = platform_token_account.mint == payment_mint.key() @ VertaError::InvalidMint
    )]
    pub platform_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, token::Token>,
    pub system_program: Program<'info, System>,
}

impl<'info> UnlockEpisode<'info> {
    pub fn transfer_creator_ctx(&self) -> CpiContext<'_, '_, '_, 'info, TransferChecked<'info>> {
        let cpi_program = self.token_program.to_account_info();
        let cpi_accounts = TransferChecked {
            from: self.user_token_account.to_account_info(),
            mint: self.payment_mint.to_account_info(),
            to: self.creator_token_account.to_account_info(),
            authority: self.user.to_account_info(),
        };
        CpiContext::new(cpi_program, cpi_accounts)
    }

    pub fn transfer_platform_ctx(&self) -> CpiContext<'_, '_, '_, 'info, TransferChecked<'info>> {
        let cpi_program = self.token_program.to_account_info();
        let cpi_accounts = TransferChecked {
            from: self.user_token_account.to_account_info(),
            mint: self.payment_mint.to_account_info(),
            to: self.platform_token_account.to_account_info(),
            authority: self.user.to_account_info(),
        };
        CpiContext::new(cpi_program, cpi_accounts)
    }
}

// --- Events ---

#[event]
pub struct TitleConfigured {
    pub title_id: String,
    pub authority: Pubkey,
    pub creator_wallet: Pubkey,
    pub platform_wallet: Pubkey,
    pub price_amount: u64,
}

#[event]
pub struct EpisodeUnlocked {
    pub user: Pubkey,
    pub title_config: Pubkey,
    pub episode_id: String,
    pub payment_mint: Pubkey,
    pub total_amount: u64,
    pub creator_amount: u64,
    pub platform_amount: u64,
    pub timestamp: i64,
}

// --- Errors ---

#[error_code]
pub enum VertaError {
    #[msg("You are not authorized to perform this action.")]
    Unauthorized,
    #[msg("Basis points must sum to 10,000.")]
    InvalidBasisPoints,
    #[msg("Invalid title ID length.")]
    InvalidTitleId,
    #[msg("Invalid episode ID length.")]
    InvalidEpisodeId,
    #[msg("Price must be greater than zero.")]
    InvalidPrice,
    #[msg("Invalid recipient address.")]
    InvalidRecipient,
    #[msg("The platform is currently paused.")]
    PlatformPaused,
    #[msg("This title is currently inactive.")]
    TitleInactive,
    #[msg("The provided mint does not match the configuration.")]
    InvalidMint,
    #[msg("The provided token account does not belong to the expected owner.")]
    InvalidTokenOwner,
    #[msg("A math overflow occurred.")]
    MathOverflow,
}

// --- Constants ---

pub const PLATFORM_AUTHORITY_SEED: &[u8] = b"platform_authority";
pub const TITLE_CONFIG_SEED: &[u8] = b"title_config";
pub const UNLOCK_RECEIPT_SEED: &[u8] = b"unlock_receipt";

pub const BPS_DENOMINATOR: u16 = 10_000;
pub const MAX_TITLE_ID_LEN: usize = 32;
pub const MAX_EPISODE_ID_LEN: usize = 32;
