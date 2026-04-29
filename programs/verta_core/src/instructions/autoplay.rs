use anchor_lang::prelude::*;
use anchor_lang::system_program;
use crate::state::*;
use crate::constants::*;
use crate::errors::*;

pub fn auto_unlock_episode(ctx: Context<AutoUnlockEpisode>, episode_id: String) -> Result<()> {
    let total_amount;
    let creator_amount;
    let platform_amount;
    let vault_user;
    let vault_bump;

    {
        let platform_config = &ctx.accounts.platform_config;
        let title_config = &ctx.accounts.title_config;
        let vault = &ctx.accounts.user_vault;

        require!(!platform_config.paused, VertaError::PlatformPaused);
        require!(title_config.active, VertaError::TitleInactive);
        require!(vault.auto_play_enabled, VertaError::AutoPlayDisabled);
        require!(ctx.accounts.delegate.key() == vault.delegate, VertaError::OnlyDelegate);
        require!(episode_id.len() > 0 && episode_id.len() <= MAX_EPISODE_ID_LEN, VertaError::InvalidEpisodeId);

        total_amount = platform_config.price_sol;
        
        creator_amount = (total_amount as u128)
            .checked_mul(title_config.creator_bps as u128)
            .ok_or(VertaError::MathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(VertaError::MathOverflow)? as u64;
        
        platform_amount = total_amount.checked_sub(creator_amount).ok_or(VertaError::MathOverflow)?;

        vault_user = vault.user;
        vault_bump = vault.bump;

        let vault_lamports = ctx.accounts.user_vault.to_account_info().lamports();
        require!(vault_lamports >= total_amount, VertaError::InsufficientVaultFunds);
    }

    // 2. Transers
    if creator_amount > 0 {
        **ctx.accounts.user_vault.to_account_info().try_borrow_mut_lamports()? -= creator_amount;
        **ctx.accounts.creator_wallet.to_account_info().try_borrow_mut_lamports()? += creator_amount;
    }

    if platform_amount > 0 {
        **ctx.accounts.user_vault.to_account_info().try_borrow_mut_lamports()? -= platform_amount;
        **ctx.accounts.platform_wallet.to_account_info().try_borrow_mut_lamports()? += platform_amount;
    }

    // 3. Post-transfer state updates
    
    // [NEW] If the creator_wallet is actually a Crowdfund PDA, update its accumulated revenue
    let (expected_crowdfund_pda, _bump) = Pubkey::find_program_address(
        &[b"crowdfund", ctx.accounts.title_config.title_id.as_bytes()],
        ctx.program_id
    );

    if ctx.accounts.creator_wallet.key() == expected_crowdfund_pda {
        let mut data = ctx.accounts.creator_wallet.try_borrow_mut_data()?;
        let mut crowdfund = Crowdfund::try_deserialize(&mut &data[..])?;
        
        crowdfund.revenue_accumulated = crowdfund.revenue_accumulated
            .checked_add(creator_amount)
            .ok_or(VertaError::MathOverflow)?;
        
        crowdfund.try_serialize(&mut &mut data[..])?;
    }

    let vault = &mut ctx.accounts.user_vault;
    vault.total_spent = vault.total_spent.checked_add(total_amount).ok_or(VertaError::MathOverflow)?;


    let receipt = &mut ctx.accounts.unlock_receipt;
    receipt.series = ctx.accounts.title_config.key();
    receipt.user = vault.user;
    receipt.episode_id = episode_id;
    receipt.paid_amount = total_amount;
    receipt.unlocked_at = Clock::get()?.unix_timestamp;
    receipt.bump = ctx.bumps.unlock_receipt;

    Ok(())
}

#[derive(Accounts)]
#[instruction(episode_id: String)]
pub struct AutoUnlockEpisode<'info> {
    #[account(
        seeds = [PLATFORM_AUTHORITY_SEED],
        bump = platform_config.bump,
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(
        seeds = [TITLE_CONFIG_SEED, title_config.title_id.as_bytes()],
        bump = title_config.bump,
        has_one = creator_wallet @ VertaError::InvalidRecipient,
    )]
    pub title_config: Account<'info, TitleConfig>,

    #[account(
        mut,
        seeds = [USER_VAULT_SEED, user_vault.user.as_ref()],
        bump = user_vault.bump,
    )]
    pub user_vault: Account<'info, UserVault>,

    #[account(
        init,
        payer = delegate,
        space = EpisodeUnlockReceipt::MAX_SIZE,
        seeds = [
            UNLOCK_RECEIPT_SEED, 
            user_vault.user.as_ref(), 
            title_config.key().as_ref(), 
            episode_id.as_bytes()
        ],
        bump
    )]
    pub unlock_receipt: Account<'info, EpisodeUnlockReceipt>,

    /// CHECK: Validated via title_config.has_one
    #[account(mut)]
    pub creator_wallet: UncheckedAccount<'info>,

    /// CHECK: Validated via platform_config.platform_wallet
    #[account(
        mut,
        constraint = platform_wallet.key() == platform_config.platform_wallet @ VertaError::Unauthorized
    )]
    pub platform_wallet: UncheckedAccount<'info>,

    #[account(mut)]
    pub delegate: Signer<'info>,
    pub system_program: Program<'info, System>,
}
