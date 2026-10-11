use anchor_lang::prelude::*;
use anchor_spl::{
    token::{self, Mint as LegacyMint, TokenAccount as LegacyTokenAccount},
    token_2022::{self, Token2022},
    token_interface::{Mint, TokenAccount},
};

use crate::{
    constants::{
        HOLDER_REGISTRY_SEED, INVENTORY_AUTHORITY_SEED, MAX_HOLDERS, MOCK_USDC_DECIMALS,
        PROPERTY_SEED, RENT_PERIOD_SEED, RENT_POOL_SEED,
    },
    error::PropertyVaultError as Error,
    rent_logic,
    state::{HolderRegistry, Property, RentPeriod, RentPool},
    token_transfer,
};

#[derive(Accounts)]
pub struct FinalizeRentPeriod<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        seeds = [PROPERTY_SEED, property.owner.as_ref(), &property.property_id.to_le_bytes()],
        bump = property.bump
    )]
    pub property: Box<Account<'info, Property>>,
    #[account(address = property.mint @ Error::InvalidPropertyMint)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(
        address = property.inventory_vault @ Error::InvalidProtocolAccount
    )]
    pub inventory_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        seeds = [HOLDER_REGISTRY_SEED, property.key().as_ref()],
        bump = registry.bump
    )]
    pub registry: Box<Account<'info, HolderRegistry>>,
    #[account(
        mut,
        seeds = [RENT_POOL_SEED, property.key().as_ref()],
        bump = rent_pool.bump
    )]
    pub rent_pool: Box<Account<'info, RentPool>>,
    #[account(
        address = property.mock_usdc_mint @ Error::InvalidMockUsdcMint,
        owner = token::ID @ Error::InvalidMockUsdcMint,
        constraint = mock_usdc_mint.decimals == MOCK_USDC_DECIMALS
            @ Error::InvalidMockUsdcMint
    )]
    pub mock_usdc_mint: Box<Account<'info, LegacyMint>>,
    #[account(
        address = rent_pool.vault @ Error::InvalidProtocolAccount,
        constraint = rent_vault.owner == rent_pool.key() @ Error::InvalidProtocolAccount,
        constraint = rent_vault.mint == mock_usdc_mint.key() @ Error::InvalidMockUsdcMint
    )]
    pub rent_vault: Box<Account<'info, LegacyTokenAccount>>,
    #[account(
        init, payer = payer, space = RentPeriod::SPACE,
        seeds = [RENT_PERIOD_SEED, property.key().as_ref(), &rent_pool.period_id.to_le_bytes()],
        bump
    )]
    pub rent_period: Box<Account<'info, RentPeriod>>,
    pub system_program: Program<'info, System>,
    pub token_2022_program: Program<'info, Token2022>,
}

// remaining_accounts contains exactly one canonical Token-2022 ATA for every
// active registry entry, in registry order, including zero-balance holders.
pub fn handle_finalize_rent_period(ctx: Context<FinalizeRentPeriod>) -> Result<()> {
    let accounts = ctx.accounts;
    token_transfer::validate_property(
        &accounts.property,
        &accounts.mint,
        &accounts.rent_pool,
        &accounts.registry,
    )?;
    let now = Clock::get()?.unix_timestamp;
    rent_logic::require_finalizable(now, accounts.rent_pool.cutoff_ts).map_err(|e| error!(e))?;
    let count = usize::from(accounts.registry.count);
    require_eq!(
        ctx.remaining_accounts.len(),
        count,
        Error::InvalidHolderSnapshot
    );
    require!(count <= MAX_HOLDERS, Error::InvalidHolderSnapshot);

    let inventory_owner =
        token_transfer::validate_token_account(&accounts.inventory_vault, &accounts.mint.key())?;
    let inventory_authority = Pubkey::find_program_address(
        &[INVENTORY_AUTHORITY_SEED, accounts.property.key().as_ref()],
        &crate::ID,
    )
    .0;
    require_keys_eq!(
        inventory_owner,
        inventory_authority,
        Error::InvalidProtocolAccount
    );
    let inventory_balance = accounts.inventory_vault.amount;
    require!(
        inventory_balance <= accounts.property.max_supply,
        Error::InvalidHolderSnapshot
    );
    let mut balances = [0u64; MAX_HOLDERS];
    let mut eligible_supply = 0u64;
    for (index, info) in ctx.remaining_accounts.iter().enumerate() {
        let holder = accounts.registry.holders[index];
        require_keys_eq!(
            *info.owner,
            token_2022::ID,
            Error::InvalidHolderTokenAccount
        );
        let token = InterfaceAccount::<TokenAccount>::try_from(info)?;
        let wallet = token_transfer::validate_token_account(&token, &accounts.mint.key())?;
        require_keys_eq!(wallet, holder, Error::InvalidHolderTokenAccount);
        require_keys_neq!(
            token.key(),
            accounts.inventory_vault.key(),
            Error::InvalidHolderSnapshot
        );
        balances[index] = token.amount;
        eligible_supply = eligible_supply
            .checked_add(token.amount)
            .ok_or_else(|| error!(Error::ArithmeticOverflow))?;
    }

    require_eq!(
        eligible_supply
            .checked_add(inventory_balance)
            .ok_or_else(|| error!(Error::ArithmeticOverflow))?,
        accounts.property.max_supply,
        Error::InvalidHolderSnapshot
    );
    let distributable = accounts.rent_pool.current_deposits;
    let (investor_amount, owner_amount) = rent_logic::split_rent(
        distributable,
        accounts.property.investor_allocation_bps,
        eligible_supply,
    )
    .map_err(|e| error!(e))?;
    let owner_outstanding = accounts
        .rent_pool
        .owner_entitlement_total
        .checked_sub(accounts.rent_pool.owner_claimed_total)
        .ok_or_else(|| error!(Error::InvalidRentAccountingState))?;
    let rent_backing = accounts
        .rent_pool
        .reserved_unclaimed
        .checked_add(owner_outstanding)
        .and_then(|sum| sum.checked_add(distributable))
        .ok_or_else(|| error!(Error::ArithmeticOverflow))?;
    require!(
        accounts.rent_vault.amount >= rent_backing,
        Error::InsufficientRentVaultBalance
    );

    let entitlements =
        rent_logic::holder_entitlements(investor_amount, &balances, count, eligible_supply)
            .map_err(|e| error!(e))?;
    let total_entitlement = investor_amount;
    let next_reserved = accounts
        .rent_pool
        .reserved_unclaimed
        .checked_add(total_entitlement)
        .ok_or_else(|| error!(Error::ArithmeticOverflow))?;
    let next_owner_total = accounts
        .rent_pool
        .owner_entitlement_total
        .checked_add(owner_amount)
        .ok_or_else(|| error!(Error::ArithmeticOverflow))?;
    let next_id = accounts
        .rent_pool
        .period_id
        .checked_add(1)
        .ok_or_else(|| error!(Error::ArithmeticOverflow))?;
    let next_cutoff = rent_logic::checked_cutoff(now, accounts.rent_pool.duration_seconds)
        .map_err(|e| error!(e))?;

    let period = &mut accounts.rent_period;
    period.property = accounts.property.key();
    period.period_id = accounts.rent_pool.period_id;
    period.start_ts = accounts.rent_pool.period_start_ts;
    period.cutoff_ts = accounts.rent_pool.cutoff_ts;
    period.distributable_rent = distributable;
    period.owner_entitlement = owner_amount;
    period.eligible_supply = eligible_supply;
    period.total_entitlement = total_entitlement;
    period.claimed_amount = 0;
    period.holder_count = accounts.registry.count;
    period.holders = accounts.registry.holders;
    period.balances = balances;
    period.entitlements = entitlements;
    period.bump = ctx.bumps.rent_period;

    let pool = &mut accounts.rent_pool;
    pool.period_id = next_id;
    pool.period_start_ts = now;
    pool.cutoff_ts = next_cutoff;
    pool.current_deposits = 0;
    pool.reserved_unclaimed = next_reserved;
    pool.owner_entitlement_total = next_owner_total;
    Ok(())
}
