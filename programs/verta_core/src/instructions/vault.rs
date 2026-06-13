use crate::constants::*;
use crate::errors::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_lang::system_program;

pub fn initialize_user_vault(ctx: Context<InitializeUserVault>) -> Result<()> {
    let vault = &mut ctx.accounts.user_vault;
    vault.user = ctx.accounts.user.key();
    vault.delegate = Pubkey::default();
    vault.auto_play_enabled = false;
    vault.total_spent = 0;
    vault.bump = ctx.bumps.user_vault;
    Ok(())
}

pub fn deposit_sol(ctx: Context<DepositSol>, amount: u64) -> Result<()> {
    let cpi_context = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        system_program::Transfer {
            from: ctx.accounts.user.to_account_info(),
            to: ctx.accounts.user_vault.to_account_info(),
        },
    );
    system_program::transfer(cpi_context, amount)?;
    Ok(())
}

pub fn withdraw_sol(ctx: Context<WithdrawSol>, amount: u64) -> Result<()> {
    let vault_lamports = ctx.accounts.user_vault.to_account_info().lamports();
    require!(vault_lamports >= amount, VertaError::InsufficientVaultFunds);

    **ctx
        .accounts
        .user_vault
        .to_account_info()
        .try_borrow_mut_lamports()? -= amount;
    **ctx
        .accounts
        .user
        .to_account_info()
        .try_borrow_mut_lamports()? += amount;

    Ok(())
}

pub fn set_vault_delegate(
    ctx: Context<SetVaultDelegate>,
    delegate: Pubkey,
    enabled: bool,
) -> Result<()> {
    let vault = &mut ctx.accounts.user_vault;
    vault.delegate = delegate;
    vault.auto_play_enabled = enabled;
    Ok(())
}

#[derive(Accounts)]
pub struct InitializeUserVault<'info> {
    #[account(
        init,
        payer = user,
        space = 8 + UserVault::INIT_SPACE,
        seeds = [USER_VAULT_SEED, user.key().as_ref()],
        bump
    )]
    pub user_vault: Account<'info, UserVault>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DepositSol<'info> {
    #[account(
        mut,
        seeds = [USER_VAULT_SEED, user.key().as_ref()],
        bump = user_vault.bump,
    )]
    pub user_vault: Account<'info, UserVault>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct WithdrawSol<'info> {
    #[account(
        mut,
        seeds = [USER_VAULT_SEED, user.key().as_ref()],
        bump = user_vault.bump,
        has_one = user @ VertaError::Unauthorized
    )]
    pub user_vault: Account<'info, UserVault>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetVaultDelegate<'info> {
    #[account(
        mut,
        seeds = [USER_VAULT_SEED, user.key().as_ref()],
        bump = user_vault.bump,
        has_one = user @ VertaError::Unauthorized
    )]
    pub user_vault: Account<'info, UserVault>,
    pub user: Signer<'info>,
}
