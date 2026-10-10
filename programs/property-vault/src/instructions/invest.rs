use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id,
    token::{self, Mint as LegacyMint, Token, TokenAccount as LegacyTokenAccount},
    token_2022::{self, Token2022},
    token_interface::{Mint, TokenAccount},
};

use crate::{
    constants::{
        EXTRA_ACCOUNT_METAS_SEED, FREEZE_AUTHORITY_SEED, HOLDER_REGISTRY_SEED,
        INVENTORY_AUTHORITY_SEED, INVESTMENT_POOL_SEED, MOCK_USDC_DECIMALS, POSITION_SEED,
        RENT_POOL_SEED, TRANSFER_HOOK_PROGRAM_ID,
    },
    error::PropertyVaultError as Error,
    logic,
    state::{HolderRegistry, InvestmentPool, InvestorPosition, Property, RentPool},
    token_transfer,
};

#[derive(Accounts)]
pub struct Invest<'info> {
    #[account(mut)]
    pub investor: Signer<'info>,
    pub property: Box<Account<'info, Property>>,
    #[account(address = property.mint @ Error::InvalidPropertyMint)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    /// CHECK: Canonical inventory authority PDA; checked against property state.
    #[account(
        seeds = [INVENTORY_AUTHORITY_SEED, property.key().as_ref()],
        bump = property.inventory_authority_bump
    )]
    pub inventory_authority: UncheckedAccount<'info>,
    #[account(mut, address = property.inventory_vault @ Error::InvalidProtocolAccount)]
    pub inventory_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    /// CHECK: Canonical freeze authority PDA; checked against property state.
    #[account(
        seeds = [FREEZE_AUTHORITY_SEED, property.key().as_ref()],
        bump = property.freeze_authority_bump
    )]
    pub freeze_authority: UncheckedAccount<'info>,
    #[account(
        mut,
        address = get_associated_token_address_with_program_id(
            &investor.key(), &mint.key(), &token_2022::ID
        ) @ Error::InvalidHolderTokenAccount
    )]
    pub investor_property_token: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        seeds = [HOLDER_REGISTRY_SEED, property.key().as_ref()],
        bump = registry.bump
    )]
    pub registry: Box<Account<'info, HolderRegistry>>,
    #[account(seeds = [RENT_POOL_SEED, property.key().as_ref()], bump = rent_pool.bump)]
    pub rent_pool: Box<Account<'info, RentPool>>,
    #[account(
        mut, seeds = [INVESTMENT_POOL_SEED, property.key().as_ref()],
        bump = investment_pool.bump,
        has_one = property @ Error::InvalidProtocolAccount,
        constraint = investment_pool.vault == investment_vault.key()
            @ Error::InvalidProtocolAccount
    )]
    pub investment_pool: Box<Account<'info, InvestmentPool>>,
    #[account(
        init_if_needed,
        payer = investor,
        space = InvestorPosition::SPACE,
        seeds = [POSITION_SEED, property.key().as_ref(), investor.key().as_ref()],
        bump
    )]
    pub position: Box<Account<'info, InvestorPosition>>,
    #[account(
        address = property.mock_usdc_mint @ Error::InvalidMockUsdcMint,
        owner = token::ID @ Error::InvalidMockUsdcMint,
        constraint = mock_usdc_mint.decimals == MOCK_USDC_DECIMALS
            @ Error::InvalidMockUsdcMint
    )]
    pub mock_usdc_mint: Box<Account<'info, LegacyMint>>,
    #[account(
        mut,
        address = get_associated_token_address_with_program_id(
            &investor.key(), &mock_usdc_mint.key(), &token::ID
        ) @ Error::InvalidProtocolAccount,
        constraint = investor_usdc.owner == investor.key() @ Error::Unauthorized,
        constraint = investor_usdc.mint == mock_usdc_mint.key() @ Error::InvalidMockUsdcMint
    )]
    pub investor_usdc: Box<Account<'info, LegacyTokenAccount>>,
    #[account(
        mut,
        constraint = investment_vault.owner == investment_pool.key()
            @ Error::InvalidProtocolAccount,
        constraint = investment_vault.mint == mock_usdc_mint.key()
            @ Error::InvalidMockUsdcMint
    )]
    pub investment_vault: Box<Account<'info, LegacyTokenAccount>>,
    /// CHECK: Hook-owned TLV PDA is validated and parsed by the handler.
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
    pub token_2022_program: Program<'info, Token2022>,
    pub legacy_token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[event]
pub struct InvestmentMade {
    pub property: Pubkey,
    pub investor: Pubkey,
    pub usdc_amount: u64,
    pub token_amount: u64,
}

pub fn handle_invest(
    ctx: Context<Invest>,
    amount: u64,
    accepted_terms_hash: [u8; 32],
) -> Result<()> {
    let accounts = ctx.accounts;
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
    logic::validate_investment_request(amount, &accepted_terms_hash, &accounts.property.terms_hash)
        .map_err(|e| error!(e))?;
    let token_amount = logic::calculate_token_amount(amount, accounts.property.token_price)
        .map_err(|e| error!(e))?;
    logic::validate_inventory(accounts.inventory_vault.amount, token_amount)
        .map_err(|e| error!(e))?;
    require!(
        token_transfer::is_registered(&accounts.registry, &accounts.investor.key()),
        Error::HolderNotRegistered
    );
    let inventory_owner =
        token_transfer::validate_token_account(&accounts.inventory_vault, &accounts.mint.key())?;
    require_keys_eq!(
        inventory_owner,
        accounts.inventory_authority.key(),
        Error::InvalidProtocolAccount
    );
    let investor_owner = token_transfer::validate_token_account(
        &accounts.investor_property_token,
        &accounts.mint.key(),
    )?;
    require_keys_eq!(
        investor_owner,
        accounts.investor.key(),
        Error::InvalidHolderTokenAccount
    );
    require_keys_neq!(
        accounts.inventory_vault.key(),
        accounts.investor_property_token.key(),
        Error::InvalidHolderTokenAccount
    );

    let first_investment = accounts.position.property == Pubkey::default();
    if first_investment {
        require_keys_eq!(
            accounts.position.investor,
            Pubkey::default(),
            Error::InvalidProtocolAccount
        );
        require_eq!(
            accounts.position.invested_amount,
            0,
            Error::InvalidAccountingState
        );
        require_eq!(
            accounts.position.token_amount,
            0,
            Error::InvalidAccountingState
        );
        require!(
            accounts.position.accepted_terms_hash == [0; 32],
            Error::InvalidAccountingState
        );
        require_eq!(accounts.position.bump, 0, Error::InvalidProtocolAccount);
    } else {
        require_keys_eq!(
            accounts.position.property,
            accounts.property.key(),
            Error::InvalidProtocolAccount
        );
        require_keys_eq!(
            accounts.position.investor,
            accounts.investor.key(),
            Error::InvalidProtocolAccount
        );
        require_eq!(
            accounts.position.bump,
            ctx.bumps.position,
            Error::InvalidProtocolAccount
        );
    }
    require!(
        accounts.investment_pool.total_withdrawn <= accounts.investment_pool.total_raised,
        Error::InvalidAccountingState
    );
    let totals = logic::next_investment_totals(
        accounts.investment_pool.total_raised,
        accounts.position.invested_amount,
        accounts.position.token_amount,
        amount,
        token_amount,
    )
    .map_err(|e| error!(e))?;
    require!(
        accounts.investor_usdc.amount >= amount,
        Error::InvalidInvestmentAmount
    );

    token::transfer(
        CpiContext::new(
            accounts.legacy_token_program.key(),
            token::Transfer {
                from: accounts.investor_usdc.to_account_info(),
                to: accounts.investment_vault.to_account_info(),
                authority: accounts.investor.to_account_info(),
            },
        ),
        amount,
    )?;
    let property_key = accounts.property.key();
    let inventory_bump = [accounts.property.inventory_authority_bump];
    let inventory_seeds: &[&[u8]] = &[
        INVENTORY_AUTHORITY_SEED,
        property_key.as_ref(),
        &inventory_bump,
    ];
    token_transfer::transfer_checked_with_hook(
        accounts.inventory_vault.to_account_info(),
        accounts.mint.to_account_info(),
        accounts.investor_property_token.to_account_info(),
        accounts.inventory_authority.to_account_info(),
        accounts.freeze_authority.to_account_info(),
        accounts.property.to_account_info(),
        accounts.rent_pool.to_account_info(),
        accounts.registry.to_account_info(),
        accounts.extra_account_meta_list.to_account_info(),
        accounts.hook_program.to_account_info(),
        accounts.token_2022_program.to_account_info(),
        token_amount,
        &[inventory_seeds],
        property_key,
        accounts.property.freeze_authority_bump,
    )?;

    accounts.investment_pool.total_raised = totals.total_raised;
    accounts.position.property = property_key;
    accounts.position.investor = accounts.investor.key();
    accounts.position.invested_amount = totals.invested_amount;
    accounts.position.token_amount = totals.token_amount;
    accounts.position.accepted_terms_hash = accepted_terms_hash;
    accounts.position.bump = ctx.bumps.position;
    emit!(InvestmentMade {
        property: property_key,
        investor: accounts.investor.key(),
        usdc_amount: amount,
        token_amount,
    });
    Ok(())
}
