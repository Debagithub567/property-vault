use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::{
    associated_token::{self, get_associated_token_address_with_program_id, AssociatedToken},
    token::{self, Mint as LegacyMint, Token},
    token_2022::{self, spl_token_2022, Token2022},
    token_2022_extensions::{self, transfer_hook::TransferHookInitialize},
};
use spl_token_2022::extension::ExtensionType;
use spl_token_2022::instruction::AuthorityType;

use crate::{
    constants::{
        CONFIG_SEED, FREEZE_AUTHORITY_SEED, HOLDER_REGISTRY_SEED, INVENTORY_AUTHORITY_SEED,
        INVESTMENT_POOL_SEED, MAX_HOLDERS, MOCK_USDC_DECIMALS, PROPERTY_MINT_SEED, PROPERTY_SEED,
        PROPERTY_TOKEN_DECIMALS, RENT_POOL_SEED, TRANSFER_HOOK_PROGRAM_ID,
    },
    error::PropertyVaultError,
    rent_logic,
    state::{Config, HolderRegistry, InvestmentPool, Property, RentPool},
};

#[derive(Accounts)]
#[instruction(property_id: u64)]
pub struct ListAndTokenizeProperty<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = mock_usdc_mint @ PropertyVaultError::InvalidMockUsdcMint
    )]
    pub config: Box<Account<'info, Config>>,
    #[account(
        init, payer = owner, space = Property::SPACE,
        seeds = [PROPERTY_SEED, owner.key().as_ref(), &property_id.to_le_bytes()], bump
    )]
    pub property: Box<Account<'info, Property>>,
    /// CHECK: Created by the handler as a Token-2022 mint after extension sizing.
    #[account(mut, seeds = [PROPERTY_MINT_SEED, property.key().as_ref()], bump)]
    pub property_mint: UncheckedAccount<'info>,
    /// CHECK: Canonical PDA controls the inventory ATA; it has no data account.
    #[account(seeds = [INVENTORY_AUTHORITY_SEED, property.key().as_ref()], bump)]
    pub inventory_authority: UncheckedAccount<'info>,
    /// CHECK: Canonical PDA remains the mint's freeze authority.
    #[account(seeds = [FREEZE_AUTHORITY_SEED, property.key().as_ref()], bump)]
    pub freeze_authority: UncheckedAccount<'info>,
    /// CHECK: Canonical Token-2022 ATA, created by the associated token program.
    #[account(
        mut,
        address = get_associated_token_address_with_program_id(
            &inventory_authority.key(), &property_mint.key(), &token_2022::ID
        ) @ PropertyVaultError::InvalidProtocolAccount
    )]
    pub inventory_vault: UncheckedAccount<'info>,
    #[account(
        init, payer = owner, space = InvestmentPool::SPACE,
        seeds = [INVESTMENT_POOL_SEED, property.key().as_ref()], bump
    )]
    pub investment_pool: Box<Account<'info, InvestmentPool>>,
    /// CHECK: Canonical legacy SPL ATA, created by the associated token program.
    #[account(
        mut,
        address = get_associated_token_address_with_program_id(
            &investment_pool.key(), &mock_usdc_mint.key(), &token::ID
        ) @ PropertyVaultError::InvalidProtocolAccount
    )]
    pub investment_vault: UncheckedAccount<'info>,
    #[account(
        init, payer = owner, space = RentPool::SPACE,
        seeds = [RENT_POOL_SEED, property.key().as_ref()], bump
    )]
    pub rent_pool: Box<Account<'info, RentPool>>,
    /// CHECK: Canonical legacy SPL ATA, created by the associated token program.
    #[account(
        mut,
        address = get_associated_token_address_with_program_id(
            &rent_pool.key(), &mock_usdc_mint.key(), &token::ID
        ) @ PropertyVaultError::InvalidProtocolAccount
    )]
    pub rent_vault: UncheckedAccount<'info>,
    #[account(
        init, payer = owner, space = HolderRegistry::SPACE,
        seeds = [HOLDER_REGISTRY_SEED, property.key().as_ref()], bump
    )]
    pub holder_registry: Box<Account<'info, HolderRegistry>>,
    #[account(
        owner = token::ID @ PropertyVaultError::InvalidMockUsdcMint,
        constraint = mock_usdc_mint.decimals == MOCK_USDC_DECIMALS
            @ PropertyVaultError::InvalidMockUsdcMint
    )]
    pub mock_usdc_mint: Box<Account<'info, LegacyMint>>,
    pub token_2022_program: Program<'info, Token2022>,
    pub legacy_token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

// Pair this instruction with the hook's owner-signed metadata initializer as
// the next top-level instruction. Anchor serializes these accounts on return.
pub fn handle_list(
    mut ctx: Context<ListAndTokenizeProperty>,
    property_id: u64,
    tenant: Pubkey,
    token_price: u64,
    max_supply: u64,
    terms_hash: [u8; 32],
    duration_seconds: u64,
    investor_allocation_bps: u16,
) -> Result<()> {
    require!(
        tenant != Pubkey::default(),
        PropertyVaultError::InvalidTenant
    );
    require!(token_price > 0, PropertyVaultError::InvalidTokenPrice);
    require!(max_supply > 0, PropertyVaultError::InvalidMaxSupply);
    require!(
        (1..10_000).contains(&investor_allocation_bps),
        PropertyVaultError::InvalidInvestorAllocation
    );
    let start_ts = Clock::get()?.unix_timestamp;
    let cutoff_ts =
        rent_logic::checked_cutoff(start_ts, duration_seconds).map_err(|e| error!(e))?;

    let accounts = &mut ctx.accounts;
    let property_key = accounts.property.key();
    let owner_info = accounts.owner.to_account_info();
    let mint_info = accounts.property_mint.to_account_info();
    let mint_key = accounts.property_mint.key();
    let token_2022_id = accounts.token_2022_program.key();
    let system_program_id = accounts.system_program.key();
    let associated_token_id = accounts.associated_token_program.key();

    // The TransferHook extension must exist before InitializeMint2.
    let mint_space = ExtensionType::try_calculate_account_len::<spl_token_2022::state::Mint>(&[
        ExtensionType::TransferHook,
    ])?;
    let property_bump = [ctx.bumps.property];
    let property_id_bytes = property_id.to_le_bytes();
    let owner_key = accounts.owner.key();
    let property_seeds: &[&[u8]] = &[
        PROPERTY_SEED,
        owner_key.as_ref(),
        &property_id_bytes,
        &property_bump,
    ];
    let mint_bump = [ctx.bumps.property_mint];
    let mint_seeds: &[&[u8]] = &[PROPERTY_MINT_SEED, property_key.as_ref(), &mint_bump];
    system_program::create_account(
        CpiContext::new(
            system_program_id,
            system_program::CreateAccount {
                from: owner_info.clone(),
                to: mint_info.clone(),
            },
        )
        .with_signer(&[mint_seeds]),
        Rent::get()?.minimum_balance(mint_space),
        u64::try_from(mint_space).map_err(|_| error!(PropertyVaultError::ArithmeticOverflow))?,
        &token_2022_id,
    )?;
    token_2022_extensions::transfer_hook_initialize(
        CpiContext::new(
            token_2022_id,
            TransferHookInitialize {
                token_program_id: accounts.token_2022_program.to_account_info(),
                mint: mint_info.clone(),
            },
        ),
        None,
        Some(TRANSFER_HOOK_PROGRAM_ID),
    )?;
    token_2022::initialize_mint2(
        CpiContext::new(
            token_2022_id,
            token_2022::InitializeMint2 {
                mint: mint_info.clone(),
            },
        ),
        PROPERTY_TOKEN_DECIMALS,
        &property_key,
        Some(&accounts.freeze_authority.key()),
    )?;

    create_ata(
        associated_token_id,
        owner_info.clone(),
        accounts.inventory_vault.to_account_info(),
        accounts.inventory_authority.to_account_info(),
        mint_info.clone(),
        accounts.system_program.to_account_info(),
        accounts.token_2022_program.to_account_info(),
    )?;
    create_ata(
        associated_token_id,
        owner_info.clone(),
        accounts.investment_vault.to_account_info(),
        accounts.investment_pool.to_account_info(),
        accounts.mock_usdc_mint.to_account_info(),
        accounts.system_program.to_account_info(),
        accounts.legacy_token_program.to_account_info(),
    )?;
    create_ata(
        associated_token_id,
        owner_info.clone(),
        accounts.rent_vault.to_account_info(),
        accounts.rent_pool.to_account_info(),
        accounts.mock_usdc_mint.to_account_info(),
        accounts.system_program.to_account_info(),
        accounts.legacy_token_program.to_account_info(),
    )?;

    token_2022::mint_to(
        CpiContext::new(
            token_2022_id,
            token_2022::MintTo {
                mint: mint_info.clone(),
                to: accounts.inventory_vault.to_account_info(),
                authority: accounts.property.to_account_info(),
            },
        )
        .with_signer(&[property_seeds]),
        max_supply,
    )?;
    token_2022::set_authority(
        CpiContext::new(
            token_2022_id,
            token_2022::SetAuthority {
                current_authority: accounts.property.to_account_info(),
                account_or_mint: mint_info.clone(),
            },
        )
        .with_signer(&[property_seeds]),
        AuthorityType::MintTokens,
        None,
    )?;
    let freeze_bump = [ctx.bumps.freeze_authority];
    let freeze_seeds: &[&[u8]] = &[FREEZE_AUTHORITY_SEED, property_key.as_ref(), &freeze_bump];
    token_2022::freeze_account(
        CpiContext::new(
            token_2022_id,
            token_2022::FreezeAccount {
                account: accounts.inventory_vault.to_account_info(),
                mint: mint_info.clone(),
                authority: accounts.freeze_authority.to_account_info(),
            },
        )
        .with_signer(&[freeze_seeds]),
    )?;

    let property = &mut accounts.property;
    property.owner = accounts.owner.key();
    property.tenant = tenant;
    property.property_id = property_id;
    property.mint = mint_key;
    property.inventory_vault = accounts.inventory_vault.key();
    property.mock_usdc_mint = accounts.mock_usdc_mint.key();
    property.token_price = token_price;
    property.max_supply = max_supply;
    property.investor_allocation_bps = investor_allocation_bps;
    property.terms_hash = terms_hash;
    property.inventory_authority_bump = ctx.bumps.inventory_authority;
    property.freeze_authority_bump = ctx.bumps.freeze_authority;
    property.bump = ctx.bumps.property;

    let investment_pool = &mut accounts.investment_pool;
    investment_pool.property = property_key;
    investment_pool.vault = accounts.investment_vault.key();
    investment_pool.total_raised = 0;
    investment_pool.total_withdrawn = 0;
    investment_pool.bump = ctx.bumps.investment_pool;

    let rent_pool = &mut accounts.rent_pool;
    rent_pool.property = property_key;
    rent_pool.vault = accounts.rent_vault.key();
    rent_pool.period_id = 0;
    rent_pool.period_start_ts = start_ts;
    rent_pool.cutoff_ts = cutoff_ts;
    rent_pool.duration_seconds = duration_seconds;
    rent_pool.current_deposits = 0;
    rent_pool.reserved_unclaimed = 0;
    rent_pool.owner_entitlement_total = 0;
    rent_pool.owner_claimed_total = 0;
    rent_pool.bump = ctx.bumps.rent_pool;

    let registry = &mut accounts.holder_registry;
    registry.property = property_key;
    registry.mint = mint_key;
    registry.holders = [Pubkey::default(); MAX_HOLDERS];
    registry.count = 0;
    registry.bump = ctx.bumps.holder_registry;
    Ok(())
}

fn create_ata<'info>(
    associated_token_program: Pubkey,
    payer: AccountInfo<'info>,
    associated_token: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    mint: AccountInfo<'info>,
    system_program: AccountInfo<'info>,
    token_program: AccountInfo<'info>,
) -> Result<()> {
    associated_token::create(CpiContext::new(
        associated_token_program,
        associated_token::Create {
            payer,
            associated_token,
            authority,
            mint,
            system_program,
            token_program,
        },
    ))
}
