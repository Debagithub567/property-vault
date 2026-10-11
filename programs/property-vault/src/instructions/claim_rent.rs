use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id,
    token::{self, Mint, Token, TokenAccount},
};

use crate::{
    constants::{
        MOCK_USDC_DECIMALS, PROPERTY_SEED, RENT_CLAIM_SEED, RENT_PERIOD_SEED, RENT_POOL_SEED,
    },
    error::PropertyVaultError as Error,
    state::{Property, RentClaim, RentPeriod, RentPool},
};

#[derive(Accounts)]
#[instruction(period_id: u64)]
pub struct ClaimRent<'info> {
    #[account(mut)]
    pub holder: Signer<'info>,
    #[account(
        seeds = [PROPERTY_SEED, property.owner.as_ref(), &property.property_id.to_le_bytes()],
        bump = property.bump
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
        seeds = [RENT_PERIOD_SEED, property.key().as_ref(), &period_id.to_le_bytes()],
        bump = rent_period.bump,
        has_one = property @ Error::InvalidProtocolAccount,
        constraint = rent_period.period_id == period_id @ Error::InvalidProtocolAccount
    )]
    pub rent_period: Box<Account<'info, RentPeriod>>,
    #[account(
        init_if_needed, payer = holder, space = RentClaim::SPACE,
        seeds = [RENT_CLAIM_SEED, property.key().as_ref(), &period_id.to_le_bytes(), holder.key().as_ref()],
        bump
    )]
    pub rent_claim: Box<Account<'info, RentClaim>>,
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
        constraint = rent_pool.vault == rent_vault.key() @ Error::InvalidProtocolAccount,
        constraint = rent_vault.owner == rent_pool.key() @ Error::InvalidProtocolAccount,
        constraint = rent_vault.mint == mock_usdc_mint.key() @ Error::InvalidMockUsdcMint
    )]
    pub rent_vault: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        address = get_associated_token_address_with_program_id(
            &holder.key(), &mock_usdc_mint.key(), &token::ID
        ) @ Error::InvalidProtocolAccount,
        constraint = holder_usdc.owner == holder.key() @ Error::Unauthorized,
        constraint = holder_usdc.mint == mock_usdc_mint.key() @ Error::InvalidMockUsdcMint
    )]
    pub holder_usdc: Box<Account<'info, TokenAccount>>,
    pub legacy_token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handle_claim_rent(ctx: Context<ClaimRent>, period_id: u64) -> Result<()> {
    let accounts = ctx.accounts;
    require!(
        period_id < accounts.rent_pool.period_id,
        Error::RentPeriodNotReady
    );
    require!(
        accounts.rent_claim.property == Pubkey::default(),
        Error::RentAlreadyClaimed
    );
    require!(
        usize::from(accounts.rent_period.holder_count) <= crate::constants::MAX_HOLDERS,
        Error::InvalidRentAccountingState
    );
    let count = usize::from(accounts.rent_period.holder_count);
    let holder = accounts.holder.key();
    let index = accounts.rent_period.holders[..count]
        .iter()
        .position(|wallet| *wallet == holder)
        .ok_or_else(|| error!(Error::NoRentEntitlement))?;
    let amount = accounts.rent_period.entitlements[index];
    require!(amount > 0, Error::NoRentEntitlement);
    let next_claimed = accounts
        .rent_period
        .claimed_amount
        .checked_add(amount)
        .ok_or_else(|| error!(Error::ArithmeticOverflow))?;
    require!(
        next_claimed <= accounts.rent_period.total_entitlement,
        Error::InvalidRentAccountingState
    );
    require!(
        accounts.rent_pool.reserved_unclaimed >= amount,
        Error::InvalidRentAccountingState
    );
    let required_backing = accounts
        .rent_pool
        .reserved_unclaimed
        .checked_add(accounts.rent_pool.current_deposits)
        .and_then(|sum| {
            sum.checked_add(
                accounts
                    .rent_pool
                    .owner_entitlement_total
                    .checked_sub(accounts.rent_pool.owner_claimed_total)?,
            )
        })
        .ok_or_else(|| error!(Error::ArithmeticOverflow))?;
    require!(
        accounts.rent_vault.amount >= required_backing,
        Error::InsufficientRentVaultBalance
    );
    let next_reserved = accounts
        .rent_pool
        .reserved_unclaimed
        .checked_sub(amount)
        .ok_or_else(|| error!(Error::ArithmeticOverflow))?;

    let property_key = accounts.property.key();
    let bump = [accounts.rent_pool.bump];
    let seeds: &[&[u8]] = &[RENT_POOL_SEED, property_key.as_ref(), &bump];
    token::transfer_checked(
        CpiContext::new(
            accounts.legacy_token_program.key(),
            token::TransferChecked {
                from: accounts.rent_vault.to_account_info(),
                mint: accounts.mock_usdc_mint.to_account_info(),
                to: accounts.holder_usdc.to_account_info(),
                authority: accounts.rent_pool.to_account_info(),
            },
        )
        .with_signer(&[seeds]),
        amount,
        MOCK_USDC_DECIMALS,
    )?;
    accounts.rent_period.claimed_amount = next_claimed;
    accounts.rent_pool.reserved_unclaimed = next_reserved;
    let claim = &mut accounts.rent_claim;
    claim.property = property_key;
    claim.period_id = period_id;
    claim.holder = holder;
    claim.amount = amount;
    claim.bump = ctx.bumps.rent_claim;
    Ok(())
}
