use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::Token2022,
    token_interface::{Mint, TokenAccount},
};

use crate::{
    constants::{
        EXTRA_ACCOUNT_METAS_SEED, FREEZE_AUTHORITY_SEED, HOLDER_REGISTRY_SEED, RENT_POOL_SEED,
        TRANSFER_HOOK_PROGRAM_ID,
    },
    error::PropertyVaultError as Error,
    state::{HolderRegistry, Property, RentPool},
    token_transfer,
};

#[derive(Accounts)]
pub struct TransferPropertyTokens<'info> {
    pub source_holder: Signer<'info>,
    pub property: Box<Account<'info, Property>>,
    #[account(address = property.mint @ Error::InvalidPropertyMint)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut)]
    pub source_token: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub destination_token: Box<InterfaceAccount<'info, TokenAccount>>,
    /// CHECK: Canonical freeze authority PDA.
    #[account(
        seeds = [FREEZE_AUTHORITY_SEED, property.key().as_ref()],
        bump = property.freeze_authority_bump
    )]
    pub freeze_authority: UncheckedAccount<'info>,
    #[account(
        seeds = [HOLDER_REGISTRY_SEED, property.key().as_ref()],
        bump = registry.bump
    )]
    pub registry: Box<Account<'info, HolderRegistry>>,
    #[account(seeds = [RENT_POOL_SEED, property.key().as_ref()], bump = rent_pool.bump)]
    pub rent_pool: Box<Account<'info, RentPool>>,
    /// CHECK: Hook-owned TLV PDA is parsed and validated by the handler.
    #[account(
        seeds = [EXTRA_ACCOUNT_METAS_SEED, mint.key().as_ref()],
        seeds::program = TRANSFER_HOOK_PROGRAM_ID,
        bump,
        owner = TRANSFER_HOOK_PROGRAM_ID @ Error::InvalidHookMetadata
    )]
    pub extra_account_meta_list: UncheckedAccount<'info>,
    /// CHECK: Fixed executable Transfer Hook program ID.
    #[account(address = TRANSFER_HOOK_PROGRAM_ID @ Error::InvalidHookMetadata, executable)]
    pub hook_program: UncheckedAccount<'info>,
    pub property_token_program: Program<'info, Token2022>,
}

pub fn handle_transfer_property_tokens(
    ctx: Context<TransferPropertyTokens>,
    amount: u64,
) -> Result<()> {
    let accounts = ctx.accounts;
    require!(amount > 0, Error::InvalidTransferAmount);
    token_transfer::validate_property(
        &accounts.property,
        &accounts.mint,
        &accounts.rent_pool,
        &accounts.registry,
    )?;
    token_transfer::require_collection_open(&accounts.rent_pool)?;
    token_transfer::validate_hook_metadata(
        &accounts.extra_account_meta_list,
        accounts.mint.key(),
        accounts.property.key(),
        accounts.rent_pool.key(),
        accounts.registry.key(),
    )?;
    let source_wallet =
        token_transfer::validate_token_account(&accounts.source_token, &accounts.mint.key())?;
    let destination_wallet =
        token_transfer::validate_token_account(&accounts.destination_token, &accounts.mint.key())?;
    require_keys_eq!(
        source_wallet,
        accounts.source_holder.key(),
        Error::Unauthorized
    );
    require_keys_neq!(
        source_wallet,
        destination_wallet,
        Error::InvalidHolderTokenAccount
    );
    require!(
        token_transfer::is_registered(&accounts.registry, &source_wallet),
        Error::HolderNotRegistered
    );
    require!(
        token_transfer::is_registered(&accounts.registry, &destination_wallet),
        Error::HolderNotRegistered
    );
    require!(
        accounts.source_token.amount >= amount,
        Error::InsufficientPropertyTokenBalance
    );
    token_transfer::transfer_checked_with_hook(
        accounts.source_token.to_account_info(),
        accounts.mint.to_account_info(),
        accounts.destination_token.to_account_info(),
        accounts.source_holder.to_account_info(),
        accounts.freeze_authority.to_account_info(),
        accounts.property.to_account_info(),
        accounts.rent_pool.to_account_info(),
        accounts.registry.to_account_info(),
        accounts.extra_account_meta_list.to_account_info(),
        accounts.hook_program.to_account_info(),
        accounts.property_token_program.to_account_info(),
        amount,
        &[],
        accounts.property.key(),
        accounts.property.freeze_authority_bump,
    )
}
