use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id,
    token::{self, Mint, Token, TokenAccount},
};

use crate::{
    constants::{MOCK_USDC_DECIMALS, PROPERTY_SEED, RENT_POOL_SEED},
    error::PropertyVaultError as Error,
    state::{Property, RentPool},
};

#[derive(Accounts)]
pub struct ClaimOwnerRent<'info> {
    pub owner: Signer<'info>,
    #[account(
        seeds = [PROPERTY_SEED, property.owner.as_ref(), &property.property_id.to_le_bytes()],
        bump = property.bump,
        has_one = owner @ Error::Unauthorized
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
        address = property.mock_usdc_mint @ Error::InvalidMockUsdcMint,
        owner = token::ID @ Error::InvalidMockUsdcMint,
        constraint = mock_usdc_mint.decimals == MOCK_USDC_DECIMALS @ Error::InvalidMockUsdcMint
    )]
    pub mock_usdc_mint: Box<Account<'info, Mint>>,
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
            &owner.key(), &mock_usdc_mint.key(), &token::ID
        ) @ Error::InvalidProtocolAccount,
        constraint = owner_usdc.owner == owner.key() @ Error::Unauthorized,
        constraint = owner_usdc.mint == mock_usdc_mint.key() @ Error::InvalidMockUsdcMint
    )]
    pub owner_usdc: Box<Account<'info, TokenAccount>>,
    pub legacy_token_program: Program<'info, Token>,
}

pub fn handle_claim_owner_rent(ctx: Context<ClaimOwnerRent>) -> Result<()> {
    let accounts = ctx.accounts;
    let amount = accounts
        .rent_pool
        .owner_entitlement_total
        .checked_sub(accounts.rent_pool.owner_claimed_total)
        .ok_or_else(|| error!(Error::InvalidRentAccountingState))?;
    require!(amount > 0, Error::NoOwnerRentEntitlement);
    let required_backing = accounts
        .rent_pool
        .reserved_unclaimed
        .checked_add(accounts.rent_pool.current_deposits)
        .and_then(|sum| sum.checked_add(amount))
        .ok_or_else(|| error!(Error::ArithmeticOverflow))?;
    require!(
        accounts.rent_vault.amount >= required_backing,
        Error::InsufficientRentVaultBalance
    );

    let property_key = accounts.property.key();
    let bump = [accounts.rent_pool.bump];
    let seeds: &[&[u8]] = &[RENT_POOL_SEED, property_key.as_ref(), &bump];
    token::transfer_checked(
        CpiContext::new(
            accounts.legacy_token_program.key(),
            token::TransferChecked {
                from: accounts.rent_vault.to_account_info(),
                mint: accounts.mock_usdc_mint.to_account_info(),
                to: accounts.owner_usdc.to_account_info(),
                authority: accounts.rent_pool.to_account_info(),
            },
        )
        .with_signer(&[seeds]),
        amount,
        MOCK_USDC_DECIMALS,
    )?;
    accounts.rent_pool.owner_claimed_total = accounts.rent_pool.owner_entitlement_total;
    Ok(())
}
