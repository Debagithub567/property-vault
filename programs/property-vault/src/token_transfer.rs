use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke_signed, program_option::COption};
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id,
    token_2022::{self, spl_token_2022},
    token_interface::{Mint, TokenAccount},
};
use spl_tlv_account_resolution::{account::ExtraAccountMeta, state::ExtraAccountMetaList};
use spl_token_2022::{
    extension::{
        immutable_owner::ImmutableOwner,
        transfer_hook::{self, TransferHook, TransferHookAccount},
        BaseStateWithExtensions, StateWithExtensions,
    },
    state::{Account as SplTokenAccount, AccountState, Mint as SplMint},
};
use spl_transfer_hook_interface::{
    instruction::ExecuteInstruction, onchain::add_extra_accounts_for_execute_cpi,
};
use spl_type_length_value::state::TlvStateBorrowed;

use crate::{
    constants::{
        EXTRA_ACCOUNT_METAS_SEED, FREEZE_AUTHORITY_SEED, HOLDER_REGISTRY_SEED,
        INVENTORY_AUTHORITY_SEED, MAX_HOLDERS, PROPERTY_MINT_SEED, PROPERTY_SEED, RENT_POOL_SEED,
        TRANSFER_HOOK_PROGRAM_ID,
    },
    error::PropertyVaultError as Error,
    state::{HolderRegistry, Property, RentPool},
};

pub fn validate_property(
    property: &Account<Property>,
    mint: &InterfaceAccount<Mint>,
    rent_pool: &Account<RentPool>,
    registry: &Account<HolderRegistry>,
) -> Result<()> {
    let property_id = property.property_id.to_le_bytes();
    let (property_key, property_bump) = Pubkey::find_program_address(
        &[PROPERTY_SEED, property.owner.as_ref(), &property_id],
        &crate::ID,
    );
    require_keys_eq!(property.key(), property_key, Error::InvalidProtocolAccount);
    require_eq!(property.bump, property_bump, Error::InvalidProtocolAccount);
    let mint_key =
        Pubkey::find_program_address(&[PROPERTY_MINT_SEED, property_key.as_ref()], &crate::ID).0;
    require_keys_eq!(property.mint, mint_key, Error::InvalidPropertyMint);
    require_keys_eq!(mint.key(), mint_key, Error::InvalidPropertyMint);
    require_keys_eq!(
        *mint.to_account_info().owner,
        token_2022::ID,
        Error::InvalidPropertyMint
    );
    require_eq!(mint.decimals, 0, Error::InvalidPropertyMint);

    let mint_info = mint.to_account_info();
    let mint_data = mint_info.try_borrow_data()?;
    let mint_state = StateWithExtensions::<SplMint>::unpack(&mint_data)
        .map_err(|_| error!(Error::InvalidPropertyMint))?;
    let hook = mint_state
        .get_extension::<TransferHook>()
        .map_err(|_| error!(Error::InvalidPropertyMint))?;
    require!(
        transfer_hook::get_program_id(&mint_state) == Some(TRANSFER_HOOK_PROGRAM_ID)
            && Option::<Pubkey>::from(hook.authority).is_none(),
        Error::InvalidPropertyMint
    );
    require_eq!(
        mint_state.base.supply,
        property.max_supply,
        Error::InvalidPropertyMint
    );
    require!(
        mint_state.base.mint_authority.is_none(),
        Error::InvalidPropertyMint
    );

    let (inventory_authority, inventory_bump) = Pubkey::find_program_address(
        &[INVENTORY_AUTHORITY_SEED, property_key.as_ref()],
        &crate::ID,
    );
    require_eq!(
        property.inventory_authority_bump,
        inventory_bump,
        Error::InvalidProtocolAccount
    );
    let expected_inventory = get_associated_token_address_with_program_id(
        &inventory_authority,
        &mint_key,
        &token_2022::ID,
    );
    require_keys_eq!(
        property.inventory_vault,
        expected_inventory,
        Error::InvalidProtocolAccount
    );
    let (freeze_authority, freeze_bump) =
        Pubkey::find_program_address(&[FREEZE_AUTHORITY_SEED, property_key.as_ref()], &crate::ID);
    require_eq!(
        property.freeze_authority_bump,
        freeze_bump,
        Error::InvalidProtocolAccount
    );
    require!(
        mint_state.base.freeze_authority == COption::Some(freeze_authority),
        Error::InvalidPropertyMint
    );

    let (rent_key, rent_bump) =
        Pubkey::find_program_address(&[RENT_POOL_SEED, property_key.as_ref()], &crate::ID);
    require_keys_eq!(rent_pool.key(), rent_key, Error::InvalidProtocolAccount);
    require_eq!(rent_pool.bump, rent_bump, Error::InvalidProtocolAccount);
    require_keys_eq!(
        rent_pool.property,
        property_key,
        Error::InvalidProtocolAccount
    );
    let expected_vault = get_associated_token_address_with_program_id(
        &rent_key,
        &property.mock_usdc_mint,
        &anchor_spl::token::ID,
    );
    require_keys_eq!(
        rent_pool.vault,
        expected_vault,
        Error::InvalidProtocolAccount
    );
    let duration = i64::try_from(rent_pool.duration_seconds)
        .map_err(|_| error!(Error::InvalidRentDuration))?;
    require!(duration > 0, Error::InvalidRentDuration);
    require_eq!(
        rent_pool.cutoff_ts,
        rent_pool
            .period_start_ts
            .checked_add(duration)
            .ok_or_else(|| error!(Error::ArithmeticOverflow))?,
        Error::InvalidProtocolAccount
    );

    let (registry_key, registry_bump) =
        Pubkey::find_program_address(&[HOLDER_REGISTRY_SEED, property_key.as_ref()], &crate::ID);
    require_keys_eq!(registry.key(), registry_key, Error::InvalidProtocolAccount);
    require_eq!(registry.bump, registry_bump, Error::InvalidProtocolAccount);
    require_keys_eq!(
        registry.property,
        property_key,
        Error::InvalidProtocolAccount
    );
    require_keys_eq!(registry.mint, mint_key, Error::InvalidProtocolAccount);
    let count = usize::from(registry.count);
    require!(count <= MAX_HOLDERS, Error::InvalidProtocolAccount);
    for (i, holder) in registry.holders[..count].iter().enumerate() {
        require!(
            *holder != Pubkey::default()
                && *holder != inventory_authority
                && !registry.holders[..i].contains(holder),
            Error::InvalidProtocolAccount
        );
    }
    Ok(())
}

pub fn require_collection_open(rent_pool: &RentPool) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        now >= rent_pool.period_start_ts,
        Error::RentCollectionClosed
    );
    require!(now < rent_pool.cutoff_ts, Error::RentCollectionClosed);
    Ok(())
}

pub fn is_registered(registry: &HolderRegistry, wallet: &Pubkey) -> bool {
    registry.holders[..usize::from(registry.count)].contains(wallet)
}

pub fn validate_token_account(
    token: &InterfaceAccount<TokenAccount>,
    mint: &Pubkey,
) -> Result<Pubkey> {
    require_keys_eq!(
        *token.to_account_info().owner,
        token_2022::ID,
        Error::InvalidHolderTokenAccount
    );
    require_keys_eq!(token.mint, *mint, Error::InvalidHolderTokenAccount);
    let wallet = token.owner;
    require_keys_eq!(
        token.key(),
        get_associated_token_address_with_program_id(&wallet, mint, &token_2022::ID),
        Error::InvalidHolderTokenAccount
    );
    let info = token.to_account_info();
    let data = info.try_borrow_data()?;
    let state = StateWithExtensions::<SplTokenAccount>::unpack(&data)
        .map_err(|_| error!(Error::InvalidHolderTokenAccount))?;
    state
        .get_extension::<ImmutableOwner>()
        .map_err(|_| error!(Error::InvalidHolderTokenAccount))?;
    state
        .get_extension::<TransferHookAccount>()
        .map_err(|_| error!(Error::InvalidHolderTokenAccount))?;
    require!(
        state.base.state == AccountState::Frozen,
        Error::TokenAccountNotFrozen
    );
    Ok(wallet)
}

pub fn validate_hook_metadata(
    metadata: &UncheckedAccount,
    mint: Pubkey,
    property: Pubkey,
    rent_pool: Pubkey,
    registry: Pubkey,
) -> Result<()> {
    let expected = Pubkey::find_program_address(
        &[EXTRA_ACCOUNT_METAS_SEED, mint.as_ref()],
        &TRANSFER_HOOK_PROGRAM_ID,
    )
    .0;
    require_keys_eq!(metadata.key(), expected, Error::InvalidHookMetadata);
    let info = metadata.to_account_info();
    require_keys_eq!(
        *info.owner,
        TRANSFER_HOOK_PROGRAM_ID,
        Error::InvalidHookMetadata
    );
    let data = info.try_borrow_data()?;
    let tlv = TlvStateBorrowed::unpack(&data).map_err(|_| error!(Error::InvalidHookMetadata))?;
    let metas = ExtraAccountMetaList::unpack_with_tlv_state::<ExecuteInstruction>(&tlv)
        .map_err(|_| error!(Error::InvalidHookMetadata))?;
    require_eq!(metas.len(), 3, Error::InvalidHookMetadata);
    for (actual, key) in metas.iter().zip([property, rent_pool, registry]) {
        let expected = ExtraAccountMeta::new_with_pubkey(&key, false, false)?;
        require!(*actual == expected, Error::InvalidHookMetadata);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn transfer_checked_with_hook<'info>(
    source: AccountInfo<'info>,
    mint: AccountInfo<'info>,
    destination: AccountInfo<'info>,
    transfer_authority: AccountInfo<'info>,
    freeze_authority: AccountInfo<'info>,
    property: AccountInfo<'info>,
    rent_pool: AccountInfo<'info>,
    registry: AccountInfo<'info>,
    metadata: AccountInfo<'info>,
    hook_program: AccountInfo<'info>,
    token_program: AccountInfo<'info>,
    amount: u64,
    source_signer_seeds: &[&[&[u8]]],
    property_key: Pubkey,
    freeze_bump: u8,
) -> Result<()> {
    require!(amount > 0, Error::InvalidTransferAmount);
    let freeze_bump_bytes = [freeze_bump];
    let freeze_seeds: &[&[u8]] = &[
        FREEZE_AUTHORITY_SEED,
        property_key.as_ref(),
        &freeze_bump_bytes,
    ];
    let freeze_signers = &[freeze_seeds];
    for account in [&source, &destination] {
        token_2022::thaw_account(
            CpiContext::new(
                token_2022::ID,
                token_2022::ThawAccount {
                    account: account.clone(),
                    mint: mint.clone(),
                    authority: freeze_authority.clone(),
                },
            )
            .with_signer(freeze_signers),
        )?;
    }

    let mut instruction = spl_token_2022::instruction::transfer_checked(
        &token_2022::ID,
        source.key,
        mint.key,
        destination.key,
        transfer_authority.key,
        &[],
        amount,
        0,
    )?;
    let mut infos = vec![
        source.clone(),
        mint.clone(),
        destination.clone(),
        transfer_authority.clone(),
    ];
    let additional = [property, rent_pool, registry, metadata, hook_program];
    add_extra_accounts_for_execute_cpi(
        &mut instruction,
        &mut infos,
        &TRANSFER_HOOK_PROGRAM_ID,
        source.clone(),
        mint.clone(),
        destination.clone(),
        transfer_authority,
        amount,
        &additional,
    )?;
    infos.push(token_program);
    invoke_signed(&instruction, &infos, source_signer_seeds)?;

    for account in [&source, &destination] {
        token_2022::freeze_account(
            CpiContext::new(
                token_2022::ID,
                token_2022::FreezeAccount {
                    account: account.clone(),
                    mint: mint.clone(),
                    authority: freeze_authority.clone(),
                },
            )
            .with_signer(freeze_signers),
        )?;
    }
    Ok(())
}
