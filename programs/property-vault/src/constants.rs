use anchor_lang::prelude::*;

#[constant]
pub const PROPERTY_SEED: &[u8] = b"property";

#[constant]
pub const PROPERTY_MINT_SEED: &[u8] = b"property_mint";

#[constant]
pub const INVESTMENT_POOL_SEED: &[u8] = b"investment_pool";

#[constant]
pub const POSITION_SEED: &[u8] = b"position";

#[constant]
pub const RENT_POOL_SEED: &[u8] = b"rent_pool";

#[constant]
pub const RENT_CLAIM_SEED: &[u8] = b"rent_claim";

#[constant]
pub const CONFIG_SEED: &[u8] = b"config";

#[constant]
pub const INVENTORY_AUTHORITY_SEED: &[u8] = b"inventory_authority";

#[constant]
pub const FREEZE_AUTHORITY_SEED: &[u8] = b"freeze_authority";

#[constant]
pub const HOLDER_REGISTRY_SEED: &[u8] = b"holder_registry";

#[constant]
pub const RENT_PERIOD_SEED: &[u8] = b"rent_period";

#[constant]
pub const EXTRA_ACCOUNT_METAS_SEED: &[u8] = b"extra-account-metas";

pub const MAX_HOLDERS: usize = 10;

pub const PROPERTY_TOKEN_DECIMALS: u8 = 0;
pub const MOCK_USDC_DECIMALS: u8 = 6;

// POC bootstrap authority: the existing local/Devnet deployment wallet.
pub const BOOTSTRAP_ADMIN: Pubkey =
    Pubkey::from_str_const("HEDAri8JV8DXDU2eN7EnJ9ELkXJ5GLBGiFemnQwx8vQA");
pub const TRANSFER_HOOK_PROGRAM_ID: Pubkey =
    Pubkey::from_str_const("3eKZN7MKpkU7gjaYFdR8zV6UCr8u7FeBTKBLe2yeM6J5");
