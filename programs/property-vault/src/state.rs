use anchor_lang::prelude::*;

use crate::constants::MAX_HOLDERS;

#[account]
#[derive(InitSpace)]
pub struct InvestmentPool {
    // The canonical Property account represented by this pool.
    pub property: Pubkey,
    // The mock-USDC investment vault; rent uses a separate vault.
    pub vault: Pubkey,
    // Cumulative capital accepted through successful investments.
    pub total_raised: u64,
    // Cumulative capital paid to the property owner.
    pub total_withdrawn: u64,
    pub bump: u8,
}

impl InvestmentPool {
    pub const SPACE: usize = Self::DISCRIMINATOR.len() + Self::INIT_SPACE;
}

#[account]
#[derive(InitSpace)]
pub struct InvestorPosition {
    pub property: Pubkey,
    pub investor: Pubkey,
    // Cumulative mock-USDC accepted from this investor for this property.
    pub invested_amount: u64,
    // Cumulative property tokens allocated to this position.
    pub token_amount: u64,
    // Currently recorded accepted terms hash; this account stores no acceptance history.
    pub accepted_terms_hash: [u8; 32],
    pub bump: u8,
}

impl InvestorPosition {
    pub const SPACE: usize = Self::DISCRIMINATOR.len() + Self::INIT_SPACE;
}

#[account]
#[derive(InitSpace)]
pub struct Config {
    pub admin: Pubkey,
    pub mock_usdc_mint: Pubkey,
    pub bump: u8,
}

impl Config {
    pub const SPACE: usize = Self::DISCRIMINATOR.len() + Self::INIT_SPACE;
}

#[account]
#[derive(InitSpace)]
pub struct Property {
    pub owner: Pubkey,
    pub tenant: Pubkey,
    pub property_id: u64,
    pub mint: Pubkey,
    pub inventory_vault: Pubkey,
    pub mock_usdc_mint: Pubkey,
    // Mock-USDC base units per zero-decimal property token.
    pub token_price: u64,
    pub max_supply: u64,
    pub terms_hash: [u8; 32],
    pub inventory_authority_bump: u8,
    pub freeze_authority_bump: u8,
    pub bump: u8,
}

impl Property {
    pub const SPACE: usize = Self::DISCRIMINATOR.len() + Self::INIT_SPACE;
}

#[account]
#[derive(InitSpace)]
pub struct HolderRegistry {
    pub property: Pubkey,
    pub mint: Pubkey,
    // Wallets are enumerated here; live balances must be read from canonical token accounts.
    pub holders: [Pubkey; MAX_HOLDERS],
    pub count: u8,
    pub bump: u8,
}

impl HolderRegistry {
    pub const SPACE: usize = Self::DISCRIMINATOR.len() + Self::INIT_SPACE;
}

#[account]
#[derive(InitSpace)]
pub struct RentPool {
    pub property: Pubkey,
    pub vault: Pubkey,
    pub period_id: u64,
    pub period_start_ts: i64,
    pub cutoff_ts: i64,
    pub duration_seconds: u64,
    pub current_deposits: u64,
    pub reserved_unclaimed: u64,
    pub carry: u64,
    pub bump: u8,
}

impl RentPool {
    pub const SPACE: usize = Self::DISCRIMINATOR.len() + Self::INIT_SPACE;
}

#[account]
#[derive(InitSpace)]
pub struct RentPeriod {
    pub property: Pubkey,
    pub period_id: u64,
    pub start_ts: i64,
    pub cutoff_ts: i64,
    pub distributable_rent: u64,
    pub eligible_supply: u64,
    pub total_entitlement: u64,
    pub claimed_amount: u64,
    pub holder_count: u8,
    pub holders: [Pubkey; MAX_HOLDERS],
    pub balances: [u64; MAX_HOLDERS],
    pub entitlements: [u64; MAX_HOLDERS],
    pub bump: u8,
}

impl RentPeriod {
    pub const SPACE: usize = Self::DISCRIMINATOR.len() + Self::INIT_SPACE;
}

#[account]
#[derive(InitSpace)]
pub struct RentClaim {
    pub property: Pubkey,
    pub period_id: u64,
    pub holder: Pubkey,
    pub amount: u64,
    pub bump: u8,
}

impl RentClaim {
    pub const SPACE: usize = Self::DISCRIMINATOR.len() + Self::INIT_SPACE;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_allocations_match_canonical_fixed_layouts() {
        assert_eq!(Config::SPACE, 73);
        assert_eq!(Property::SPACE, 227);
        assert_eq!(InvestmentPool::SPACE, 89);
        assert_eq!(InvestorPosition::SPACE, 121);
        assert_eq!(HolderRegistry::SPACE, 394);
        assert_eq!(RentPool::SPACE, 129);
        assert_eq!(RentPeriod::SPACE, 578);
        assert_eq!(RentClaim::SPACE, 89);
    }
}
