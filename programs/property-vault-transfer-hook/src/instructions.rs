use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount};
use property_vault::state::{HolderRegistry, Property, RentPool};

#[derive(Accounts)]
pub struct InitializeExtraAccountMetaList<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: This hook-owned PDA is created and initialized by the handler.
    #[account(mut, seeds = [b"extra-account-metas", mint.key().as_ref()], bump)]
    pub extra_account_meta_list: UncheckedAccount<'info>,
    pub mint: InterfaceAccount<'info, Mint>,
    pub property: Account<'info, Property>,
    pub rent_pool: Account<'info, RentPool>,
    pub holder_registry: Account<'info, HolderRegistry>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Execute<'info> {
    pub source_token: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    pub destination_token: InterfaceAccount<'info, TokenAccount>,
    /// CHECK: Token-2022 verifies transfer authority; the handler checks its key against the
    /// source token account owner. Signer privilege is unavailable in Execute.
    pub authority: UncheckedAccount<'info>,
    /// CHECK: The handler verifies this hook-owned PDA and its TLV account list.
    #[account(seeds = [b"extra-account-metas", mint.key().as_ref()], bump)]
    pub extra_account_meta_list: UncheckedAccount<'info>,
    pub property: Account<'info, Property>,
    pub rent_pool: Account<'info, RentPool>,
    pub holder_registry: Account<'info, HolderRegistry>,
}
