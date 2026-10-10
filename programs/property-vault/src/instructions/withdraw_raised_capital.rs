use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id,
    token::{self, Mint, Token, TokenAccount},
};

use crate::{
    constants::{INVESTMENT_POOL_SEED, MOCK_USDC_DECIMALS, PROPERTY_SEED},
    error::PropertyVaultError as Error,
    logic,
    state::{InvestmentPool, Property},
};

#[derive(Accounts)]
pub struct WithdrawRaisedCapital<'info> {
    pub owner: Signer<'info>,
    #[account(
        seeds = [PROPERTY_SEED, property.owner.as_ref(), &property.property_id.to_le_bytes()],
        bump = property.bump,
        has_one = owner @ Error::Unauthorized
    )]
    pub property: Box<Account<'info, Property>>,
    #[account(
        mut,
        seeds = [INVESTMENT_POOL_SEED, property.key().as_ref()],
        bump = investment_pool.bump,
        has_one = property @ Error::InvalidProtocolAccount,
        constraint = investment_pool.vault == investment_vault.key()
            @ Error::InvalidProtocolAccount
    )]
    pub investment_pool: Box<Account<'info, InvestmentPool>>,
    #[account(
        mut,
        address = get_associated_token_address_with_program_id(
            &investment_pool.key(), &mock_usdc_mint.key(), &token::ID
        ) @ Error::InvalidProtocolAccount,
        constraint = investment_vault.owner == investment_pool.key()
            @ Error::InvalidProtocolAccount,
        constraint = investment_vault.mint == mock_usdc_mint.key()
            @ Error::InvalidMockUsdcMint
    )]
    pub investment_vault: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        address = get_associated_token_address_with_program_id(
            &owner.key(), &mock_usdc_mint.key(), &token::ID
        ) @ Error::InvalidProtocolAccount,
        constraint = owner_usdc.owner == owner.key() @ Error::Unauthorized,
        constraint = owner_usdc.mint == mock_usdc_mint.key() @ Error::InvalidMockUsdcMint
    )]
    pub owner_usdc: Box<Account<'info, TokenAccount>>,
    #[account(
        address = property.mock_usdc_mint @ Error::InvalidMockUsdcMint,
        owner = token::ID @ Error::InvalidMockUsdcMint,
        constraint = mock_usdc_mint.decimals == MOCK_USDC_DECIMALS
            @ Error::InvalidMockUsdcMint
    )]
    pub mock_usdc_mint: Box<Account<'info, Mint>>,
    pub legacy_token_program: Program<'info, Token>,
}

pub fn handle_withdraw_raised_capital(
    ctx: Context<WithdrawRaisedCapital>,
    amount: u64,
) -> Result<()> {
    let accounts = ctx.accounts;
    let next_withdrawn = logic::next_withdrawn_total(
        accounts.investment_pool.total_raised,
        accounts.investment_pool.total_withdrawn,
        accounts.investment_vault.amount,
        amount,
    )
    .map_err(|e| error!(e))?;

    let property_key = accounts.property.key();
    let bump = [accounts.investment_pool.bump];
    let seeds: &[&[u8]] = &[INVESTMENT_POOL_SEED, property_key.as_ref(), &bump];
    token::transfer_checked(
        CpiContext::new(
            accounts.legacy_token_program.key(),
            token::TransferChecked {
                from: accounts.investment_vault.to_account_info(),
                mint: accounts.mock_usdc_mint.to_account_info(),
                to: accounts.owner_usdc.to_account_info(),
                authority: accounts.investment_pool.to_account_info(),
            },
        )
        .with_signer(&[seeds]),
        amount,
        MOCK_USDC_DECIMALS,
    )?;
    accounts.investment_pool.total_withdrawn = next_withdrawn;
    Ok(())
}
