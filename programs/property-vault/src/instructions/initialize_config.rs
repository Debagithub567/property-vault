use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint};

use crate::{
    constants::{BOOTSTRAP_ADMIN, CONFIG_SEED, MOCK_USDC_DECIMALS},
    error::PropertyVaultError,
    state::Config,
};

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(mut, address = BOOTSTRAP_ADMIN @ PropertyVaultError::Unauthorized)]
    pub admin: Signer<'info>,
    #[account(init, payer = admin, space = Config::SPACE, seeds = [CONFIG_SEED], bump)]
    pub config: Account<'info, Config>,
    #[account(owner = token::ID @ PropertyVaultError::InvalidMockUsdcMint)]
    pub mock_usdc_mint_account: Account<'info, Mint>,
    pub system_program: Program<'info, System>,
}

pub fn handle_initialize_config(
    ctx: Context<InitializeConfig>,
    mock_usdc_mint: Pubkey,
) -> Result<()> {
    require_keys_eq!(
        mock_usdc_mint,
        ctx.accounts.mock_usdc_mint_account.key(),
        PropertyVaultError::InvalidMockUsdcMint
    );
    require_eq!(
        ctx.accounts.mock_usdc_mint_account.decimals,
        MOCK_USDC_DECIMALS,
        PropertyVaultError::InvalidMockUsdcMint
    );
    let config = &mut ctx.accounts.config;
    config.admin = ctx.accounts.admin.key();
    config.mock_usdc_mint = mock_usdc_mint;
    config.bump = ctx.bumps.config;
    Ok(())
}
