use anchor_lang::prelude::*;
use anchor_lang::solana_program::program_option::COption;
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id,
    token_2022::{self, spl_token_2022},
    token_interface::{Mint, TokenAccount},
};
use property_vault::{
    constants::{
        FREEZE_AUTHORITY_SEED, HOLDER_REGISTRY_SEED, INVENTORY_AUTHORITY_SEED, MAX_HOLDERS,
        PROPERTY_MINT_SEED, PROPERTY_SEED, RENT_POOL_SEED,
    },
    state::{HolderRegistry, Property, RentPool},
};
use spl_token_2022::{
    extension::{
        immutable_owner::ImmutableOwner,
        transfer_hook::{self, TransferHook, TransferHookAccount},
        BaseStateWithExtensions, StateWithExtensions,
    },
    state::{Account as SplTokenAccount, Mint as SplMint},
};

use crate::error::HookError;

pub fn validate_property_accounts(
    property: &Account<Property>,
    mint: &InterfaceAccount<Mint>,
    rent_pool: &Account<RentPool>,
    holder_registry: &Account<HolderRegistry>,
) -> Result<Pubkey> {
    let property_id = property.property_id.to_le_bytes();
    let (property_key, property_bump) = Pubkey::find_program_address(
        &[PROPERTY_SEED, property.owner.as_ref(), &property_id],
        &property_vault::ID,
    );
    require_keys_eq!(property.key(), property_key, HookError::InvalidPropertyPda);
    require_eq!(property.bump, property_bump, HookError::InvalidPropertyPda);

    let (mint_key, _) = Pubkey::find_program_address(
        &[PROPERTY_MINT_SEED, property_key.as_ref()],
        &property_vault::ID,
    );
    require_keys_eq!(mint.key(), mint_key, HookError::InvalidPropertyMint);
    require_keys_eq!(property.mint, mint_key, HookError::InvalidPropertyMint);
    require_keys_eq!(
        *mint.to_account_info().owner,
        token_2022::ID,
        HookError::InvalidPropertyMint
    );
    require_eq!(mint.decimals, 0, HookError::InvalidPropertyMint);

    let mint_info = mint.to_account_info();
    let mint_data = mint_info.try_borrow_data()?;
    let mint_state = StateWithExtensions::<SplMint>::unpack(&mint_data)
        .map_err(|_| error!(HookError::InvalidPropertyMint))?;
    let hook_extension = mint_state
        .get_extension::<TransferHook>()
        .map_err(|_| error!(HookError::InvalidTransferHookProgram))?;
    require!(
        transfer_hook::get_program_id(&mint_state) == Some(crate::ID),
        HookError::InvalidTransferHookProgram
    );
    require!(
        Option::<Pubkey>::from(hook_extension.authority).is_none(),
        HookError::InvalidTransferHookProgram
    );
    require_eq!(
        mint_state.base.supply,
        property.max_supply,
        HookError::InvalidPropertyMint
    );
    require!(
        mint_state.base.mint_authority.is_none(),
        HookError::InvalidPropertyMint
    );

    let (inventory_authority, inventory_bump) = Pubkey::find_program_address(
        &[INVENTORY_AUTHORITY_SEED, property_key.as_ref()],
        &property_vault::ID,
    );
    require_eq!(
        property.inventory_authority_bump,
        inventory_bump,
        HookError::InvalidPropertyPda
    );
    let inventory_vault = get_associated_token_address_with_program_id(
        &inventory_authority,
        &mint_key,
        &token_2022::ID,
    );
    require_keys_eq!(
        property.inventory_vault,
        inventory_vault,
        HookError::InvalidPropertyPda
    );
    let (freeze_authority, freeze_bump) = Pubkey::find_program_address(
        &[FREEZE_AUTHORITY_SEED, property_key.as_ref()],
        &property_vault::ID,
    );
    require_eq!(
        property.freeze_authority_bump,
        freeze_bump,
        HookError::InvalidPropertyPda
    );
    require!(
        mint_state.base.freeze_authority.as_ref() == COption::Some(&freeze_authority),
        HookError::InvalidPropertyMint
    );

    let (rent_pool_key, rent_pool_bump) = Pubkey::find_program_address(
        &[RENT_POOL_SEED, property_key.as_ref()],
        &property_vault::ID,
    );
    require_keys_eq!(rent_pool.key(), rent_pool_key, HookError::InvalidRentPool);
    require_eq!(rent_pool.bump, rent_pool_bump, HookError::InvalidRentPool);
    require_keys_eq!(rent_pool.property, property_key, HookError::InvalidRentPool);
    let rent_vault = get_associated_token_address_with_program_id(
        &rent_pool_key,
        &property.mock_usdc_mint,
        &anchor_spl::token::ID,
    );
    require_keys_eq!(rent_pool.vault, rent_vault, HookError::InvalidRentPool);
    require!(rent_pool.duration_seconds > 0, HookError::InvalidRentPool);
    let duration = i64::try_from(rent_pool.duration_seconds)
        .map_err(|_| error!(HookError::InvalidRentPool))?;
    let expected_cutoff = rent_pool
        .period_start_ts
        .checked_add(duration)
        .ok_or_else(|| error!(HookError::InvalidRentPool))?;
    require_eq!(
        rent_pool.cutoff_ts,
        expected_cutoff,
        HookError::InvalidRentPool
    );

    let (registry_key, registry_bump) = Pubkey::find_program_address(
        &[HOLDER_REGISTRY_SEED, property_key.as_ref()],
        &property_vault::ID,
    );
    require_keys_eq!(
        holder_registry.key(),
        registry_key,
        HookError::InvalidHolderRegistry
    );
    require_eq!(
        holder_registry.bump,
        registry_bump,
        HookError::InvalidHolderRegistry
    );
    require_keys_eq!(
        holder_registry.property,
        property_key,
        HookError::InvalidHolderRegistry
    );
    require_keys_eq!(
        holder_registry.mint,
        mint_key,
        HookError::InvalidHolderRegistry
    );
    validate_registry(&holder_registry.holders, holder_registry.count)?;

    Ok(inventory_authority)
}

pub fn validate_token_account(
    token: &InterfaceAccount<TokenAccount>,
    mint: &Pubkey,
) -> Result<Pubkey> {
    require_keys_eq!(
        *token.to_account_info().owner,
        token_2022::ID,
        HookError::InvalidTokenAccount
    );
    require_keys_eq!(token.mint, *mint, HookError::InvalidTokenAccount);
    let wallet = token.owner;
    let expected = get_associated_token_address_with_program_id(&wallet, mint, &token_2022::ID);
    require_keys_eq!(token.key(), expected, HookError::InvalidTokenAccount);
    let token_info = token.to_account_info();
    let token_data = token_info.try_borrow_data()?;
    let token_state = StateWithExtensions::<SplTokenAccount>::unpack(&token_data)
        .map_err(|_| error!(HookError::InvalidTokenAccount))?;
    token_state
        .get_extension::<ImmutableOwner>()
        .map_err(|_| error!(HookError::InvalidTokenAccount))?;
    Ok(wallet)
}

pub fn require_real_transfer(source: &InterfaceAccount<TokenAccount>) -> Result<()> {
    let source_info = source.to_account_info();
    let source_data = source_info.try_borrow_data()?;
    let source_state = StateWithExtensions::<SplTokenAccount>::unpack(&source_data)
        .map_err(|_| error!(HookError::InvalidTokenAccount))?;
    let transfer_state = source_state
        .get_extension::<TransferHookAccount>()
        .map_err(|_| error!(HookError::NotTransferring))?;
    require!(
        bool::from(transfer_state.transferring),
        HookError::NotTransferring
    );
    Ok(())
}

pub fn validate_registry(holders: &[Pubkey; MAX_HOLDERS], count: u8) -> Result<()> {
    let count = usize::from(count);
    require!(count <= MAX_HOLDERS, HookError::MalformedHolderRegistry);
    for (i, holder) in holders[..count].iter().enumerate() {
        require!(
            *holder != Pubkey::default(),
            HookError::MalformedHolderRegistry
        );
        require!(
            !holders[..i].contains(holder),
            HookError::MalformedHolderRegistry
        );
    }
    Ok(())
}

pub fn is_registered(holder_registry: &HolderRegistry, wallet: &Pubkey) -> bool {
    holder_registry.holders[..usize::from(holder_registry.count)].contains(wallet)
}

pub fn require_collection_open(now: i64, start_ts: i64, cutoff_ts: i64) -> Result<()> {
    require!(now >= start_ts, HookError::CollectionNotStarted);
    require!(now < cutoff_ts, HookError::CollectionClosed);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_rejects_overflow_and_duplicate_wallets() {
        let mut holders = [Pubkey::default(); MAX_HOLDERS];
        let first = Pubkey::new_unique();
        holders[0] = first;
        assert!(validate_registry(&holders, 1).is_ok());
        assert!(validate_registry(&holders, 11).is_err());
        holders[1] = first;
        assert!(validate_registry(&holders, 2).is_err());
    }

    #[test]
    fn cutoff_boundary_closes_transfers_immediately() {
        assert!(require_collection_open(99, 100, 200).is_err());
        assert!(require_collection_open(100, 100, 200).is_ok());
        assert!(require_collection_open(199, 100, 200).is_ok());
        assert!(require_collection_open(200, 100, 200).is_err());
    }
}
