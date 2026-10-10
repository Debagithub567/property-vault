use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::{self, get_associated_token_address_with_program_id, AssociatedToken},
    token_2022::{self, Token2022},
    token_interface::{Mint, TokenAccount},
};

use crate::{
    constants::{
        FREEZE_AUTHORITY_SEED, HOLDER_REGISTRY_SEED, INVENTORY_AUTHORITY_SEED, MAX_HOLDERS,
        RENT_POOL_SEED,
    },
    error::PropertyVaultError as Error,
    state::{HolderRegistry, Property, RentPool},
    token_transfer,
};

#[derive(Accounts)]
pub struct RegisterHolder<'info> {
    #[account(mut)]
    pub holder: Signer<'info>,
    pub property: Box<Account<'info, Property>>,
    #[account(address = property.mint @ Error::InvalidPropertyMint)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(
        mut, seeds = [HOLDER_REGISTRY_SEED, property.key().as_ref()],
        bump = registry.bump
    )]
    pub registry: Box<Account<'info, HolderRegistry>>,
    #[account(seeds = [RENT_POOL_SEED, property.key().as_ref()], bump = rent_pool.bump)]
    pub rent_pool: Box<Account<'info, RentPool>>,
    /// CHECK: Canonical Token-2022 ATA; created if absent, otherwise checked in the handler.
    #[account(
        mut,
        address = get_associated_token_address_with_program_id(
            &holder.key(), &mint.key(), &token_2022::ID
        ) @ Error::InvalidHolderTokenAccount
    )]
    pub holder_token: UncheckedAccount<'info>,
    /// CHECK: Canonical PDA controls mint freezing; checked by seeds and property state.
    #[account(
        seeds = [FREEZE_AUTHORITY_SEED, property.key().as_ref()],
        bump = property.freeze_authority_bump
    )]
    pub freeze_authority: UncheckedAccount<'info>,
    pub token_2022_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    /// CHECK: Only used for slot reuse; its address, owner, mint, and zero balance are checked.
    pub previous_holder_token: Option<UncheckedAccount<'info>>,
}

pub fn handle_register_holder<'info>(
    ctx: Context<'info, RegisterHolder<'info>>,
    replace_index: Option<u8>,
) -> Result<()> {
    let accounts = ctx.accounts;
    token_transfer::validate_property(
        &accounts.property,
        &accounts.mint,
        &accounts.rent_pool,
        &accounts.registry,
    )?;
    token_transfer::require_collection_open(&accounts.rent_pool)?;
    let holder_key = accounts.holder.key();
    let inventory_authority = Pubkey::find_program_address(
        &[INVENTORY_AUTHORITY_SEED, accounts.property.key().as_ref()],
        &crate::ID,
    )
    .0;
    require_keys_neq!(holder_key, inventory_authority, Error::Unauthorized);
    require!(
        !token_transfer::is_registered(&accounts.registry, &holder_key),
        Error::HolderAlreadyRegistered
    );

    let slot = if let Some(index) = replace_index {
        let index = usize::from(index);
        require!(
            index < usize::from(accounts.registry.count),
            Error::InvalidRegistryIndex
        );
        let previous = accounts.registry.holders[index];
        let expected = get_associated_token_address_with_program_id(
            &previous,
            &accounts.mint.key(),
            &token_2022::ID,
        );
        let previous_token = accounts
            .previous_holder_token
            .as_ref()
            .ok_or_else(|| error!(Error::InvalidHolderTokenAccount))?;
        require_keys_eq!(
            previous_token.key(),
            expected,
            Error::InvalidHolderTokenAccount
        );
        let previous_info = previous_token.to_account_info();
        if previous_info.data_is_empty() {
            require_keys_eq!(
                *previous_info.owner,
                System::id(),
                Error::InvalidHolderTokenAccount
            );
        } else {
            require_keys_eq!(
                *previous_info.owner,
                token_2022::ID,
                Error::InvalidHolderTokenAccount
            );
            let token = InterfaceAccount::<TokenAccount>::try_from(previous_token.as_ref())?;
            require_keys_eq!(token.owner, previous, Error::InvalidHolderTokenAccount);
            require_keys_eq!(
                token.mint,
                accounts.mint.key(),
                Error::InvalidHolderTokenAccount
            );
            require_eq!(token.amount, 0, Error::HolderStillOwnsTokens);
        }
        index
    } else {
        let index = usize::from(accounts.registry.count);
        require!(index < MAX_HOLDERS, Error::HolderRegistryFull);
        require!(
            accounts.previous_holder_token.is_none(),
            Error::InvalidRegistryIndex
        );
        index
    };

    let holder_info = accounts.holder_token.to_account_info();
    if holder_info.data_is_empty() {
        require_keys_eq!(
            *holder_info.owner,
            System::id(),
            Error::InvalidHolderTokenAccount
        );
        associated_token::create(CpiContext::new(
            accounts.associated_token_program.key(),
            associated_token::Create {
                payer: accounts.holder.to_account_info(),
                associated_token: holder_info.clone(),
                authority: accounts.holder.to_account_info(),
                mint: accounts.mint.to_account_info(),
                system_program: accounts.system_program.to_account_info(),
                token_program: accounts.token_2022_program.to_account_info(),
            },
        ))?;
    }
    let holder_token = InterfaceAccount::<TokenAccount>::try_from(accounts.holder_token.as_ref())?;
    require_keys_eq!(
        *holder_info.owner,
        token_2022::ID,
        Error::InvalidHolderTokenAccount
    );
    require_keys_eq!(
        holder_token.owner,
        holder_key,
        Error::InvalidHolderTokenAccount
    );
    require_keys_eq!(
        holder_token.mint,
        accounts.mint.key(),
        Error::InvalidHolderTokenAccount
    );
    require_eq!(holder_token.amount, 0, Error::InvalidHolderTokenAccount);
    if holder_token.state != token_2022::spl_token_2022::state::AccountState::Frozen {
        require!(
            holder_token.state == token_2022::spl_token_2022::state::AccountState::Initialized,
            Error::InvalidHolderTokenAccount
        );
        let property_key = accounts.property.key();
        let freeze_bump = [accounts.property.freeze_authority_bump];
        let freeze_seeds: &[&[u8]] = &[FREEZE_AUTHORITY_SEED, property_key.as_ref(), &freeze_bump];
        token_2022::freeze_account(
            CpiContext::new(
                accounts.token_2022_program.key(),
                token_2022::FreezeAccount {
                    account: holder_info.clone(),
                    mint: accounts.mint.to_account_info(),
                    authority: accounts.freeze_authority.to_account_info(),
                },
            )
            .with_signer(&[freeze_seeds]),
        )?;
    }
    let frozen = InterfaceAccount::<TokenAccount>::try_from(accounts.holder_token.as_ref())?;
    token_transfer::validate_token_account(&frozen, &accounts.mint.key())?;

    let registry = &mut accounts.registry;
    registry.holders[slot] = holder_key;
    if replace_index.is_none() {
        registry.count = registry
            .count
            .checked_add(1)
            .ok_or_else(|| error!(Error::ArithmeticOverflow))?;
    }
    Ok(())
}
