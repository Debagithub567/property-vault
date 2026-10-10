use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id,
    token::{self, Mint, Token, TokenAccount},
};

use crate::{
    constants::{MOCK_USDC_DECIMALS, PROPERTY_SEED, RENT_POOL_SEED},
    error::PropertyVaultError as Error,
    rent_logic,
    state::{Property, RentPool},
    token_transfer,
};

#[derive(Accounts)]
pub struct PayRent<'info> {
    pub tenant: Signer<'info>,
    #[account(
        seeds = [PROPERTY_SEED, property.owner.as_ref(), &property.property_id.to_le_bytes()],
        bump = property.bump,
        has_one = tenant @ Error::Unauthorized
    )]
    pub property: Box<Account<'info, Property>>,
    #[account(
        mut,
        seeds = [RENT_POOL_SEED, property.key().as_ref()],
        bump = rent_pool.bump,
        has_one = property @ Error::InvalidProtocolAccount,
        constraint = rent_pool.vault == rent_vault.key() @ Error::InvalidProtocolAccount
    )]
    pub rent_pool: Box<Account<'info, RentPool>>,
    #[account(
        mut,
        address = get_associated_token_address_with_program_id(
            &rent_pool.key(), &mock_usdc_mint.key(), &token::ID
        ) @ Error::InvalidProtocolAccount,
        constraint = rent_vault.owner == rent_pool.key() @ Error::InvalidProtocolAccount,
        constraint = rent_vault.mint == mock_usdc_mint.key() @ Error::InvalidMockUsdcMint
    )]
    pub rent_vault: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        address = get_associated_token_address_with_program_id(
            &tenant.key(), &mock_usdc_mint.key(), &token::ID
        ) @ Error::InvalidProtocolAccount,
        constraint = tenant_usdc.owner == tenant.key() @ Error::Unauthorized,
        constraint = tenant_usdc.mint == mock_usdc_mint.key() @ Error::InvalidMockUsdcMint
    )]
    pub tenant_usdc: Box<Account<'info, TokenAccount>>,
    #[account(
        address = property.mock_usdc_mint @ Error::InvalidMockUsdcMint,
        owner = token::ID @ Error::InvalidMockUsdcMint,
        constraint = mock_usdc_mint.decimals == MOCK_USDC_DECIMALS
            @ Error::InvalidMockUsdcMint
    )]
    pub mock_usdc_mint: Box<Account<'info, Mint>>,
    pub legacy_token_program: Program<'info, Token>,
}

pub fn handle_pay_rent(ctx: Context<PayRent>, amount: u64) -> Result<()> {
    let accounts = ctx.accounts;
    require!(amount > 0, Error::InvalidRentPaymentAmount);
    let expected_cutoff = rent_logic::checked_cutoff(
        accounts.rent_pool.period_start_ts,
        accounts.rent_pool.duration_seconds,
    )
    .map_err(|e| error!(e))?;
    require_eq!(
        accounts.rent_pool.cutoff_ts,
        expected_cutoff,
        Error::InvalidRentAccountingState
    );
    token_transfer::require_collection_open(&accounts.rent_pool)?;
    require!(
        accounts.tenant_usdc.amount >= amount,
        Error::InsufficientTenantUsdcBalance
    );
    let next_deposits = accounts
        .rent_pool
        .current_deposits
        .checked_add(amount)
        .ok_or_else(|| error!(Error::ArithmeticOverflow))?;
    token::transfer_checked(
        CpiContext::new(
            accounts.legacy_token_program.key(),
            token::TransferChecked {
                from: accounts.tenant_usdc.to_account_info(),
                mint: accounts.mock_usdc_mint.to_account_info(),
                to: accounts.rent_vault.to_account_info(),
                authority: accounts.tenant.to_account_info(),
            },
        ),
        amount,
        MOCK_USDC_DECIMALS,
    )?;
    accounts.rent_pool.current_deposits = next_deposits;
    Ok(())
}
