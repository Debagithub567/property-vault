use crate::error::PropertyVaultError;

pub fn validate_investment_request(
    amount: u64,
    accepted_hash: &[u8; 32],
    current_hash: &[u8; 32],
) -> Result<(), PropertyVaultError> {
    if amount == 0 {
        return Err(PropertyVaultError::InvalidInvestmentAmount);
    }
    if accepted_hash != current_hash {
        return Err(PropertyVaultError::StaleTerms);
    }
    Ok(())
}

// Amount and price are mock-USDC base units; each property token has zero decimals.
pub fn calculate_token_amount(amount: u64, token_price: u64) -> Result<u64, PropertyVaultError> {
    if amount == 0 {
        return Err(PropertyVaultError::InvalidInvestmentAmount);
    }
    if token_price == 0 {
        return Err(PropertyVaultError::InvalidTokenPrice);
    }
    if amount % token_price != 0 {
        return Err(PropertyVaultError::InvestmentAmountNotDivisible);
    }

    let token_amount = amount / token_price;
    if token_amount == 0 {
        return Err(PropertyVaultError::InvalidTokenAllocation);
    }
    Ok(token_amount)
}

// The caller must read available_inventory from the canonical property inventory vault.
pub fn validate_inventory(
    available_inventory: u64,
    requested_tokens: u64,
) -> Result<(), PropertyVaultError> {
    if requested_tokens == 0 {
        return Err(PropertyVaultError::InvalidTokenAllocation);
    }
    if available_inventory < requested_tokens {
        return Err(PropertyVaultError::InsufficientPropertyTokenSupply);
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub struct InvestmentTotals {
    pub total_raised: u64,
    pub invested_amount: u64,
    pub token_amount: u64,
}

pub fn next_investment_totals(
    pool_raised: u64,
    position_invested: u64,
    position_tokens: u64,
    amount: u64,
    allocated_tokens: u64,
) -> Result<InvestmentTotals, PropertyVaultError> {
    if amount == 0 {
        return Err(PropertyVaultError::InvalidInvestmentAmount);
    }
    if allocated_tokens == 0 {
        return Err(PropertyVaultError::InvalidTokenAllocation);
    }

    Ok(InvestmentTotals {
        total_raised: pool_raised
            .checked_add(amount)
            .ok_or(PropertyVaultError::ArithmeticOverflow)?,
        invested_amount: position_invested
            .checked_add(amount)
            .ok_or(PropertyVaultError::ArithmeticOverflow)?,
        token_amount: position_tokens
            .checked_add(allocated_tokens)
            .ok_or(PropertyVaultError::ArithmeticOverflow)?,
    })
}

pub fn next_withdrawn_total(
    total_raised: u64,
    total_withdrawn: u64,
    investment_vault_balance: u64,
    amount: u64,
) -> Result<u64, PropertyVaultError> {
    if amount == 0 {
        return Err(PropertyVaultError::InvalidWithdrawalAmount);
    }

    let available = total_raised
        .checked_sub(total_withdrawn)
        .ok_or(PropertyVaultError::InvalidAccountingState)?;
    if amount > available {
        return Err(PropertyVaultError::WithdrawalExceedsAvailableCapital);
    }
    if amount > investment_vault_balance {
        return Err(PropertyVaultError::InsufficientInvestmentVaultBalance);
    }

    total_withdrawn
        .checked_add(amount)
        .ok_or(PropertyVaultError::ArithmeticOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn investment_request_checks_amount_and_terms() {
        let current_hash = [7; 32];
        assert!(validate_investment_request(1, &current_hash, &current_hash).is_ok());
        assert!(matches!(
            validate_investment_request(0, &current_hash, &current_hash),
            Err(PropertyVaultError::InvalidInvestmentAmount)
        ));
        assert!(matches!(
            validate_investment_request(1, &[8; 32], &current_hash),
            Err(PropertyVaultError::StaleTerms)
        ));
    }

    #[test]
    fn price_calculation_returns_whole_tokens() {
        assert_eq!(calculate_token_amount(500_000_000, 100_000_000).unwrap(), 5);
        assert_eq!(calculate_token_amount(100_000_000, 100_000_000).unwrap(), 1);
    }

    #[test]
    fn price_calculation_rejects_invalid_inputs() {
        assert!(matches!(
            calculate_token_amount(0, 100_000_000),
            Err(PropertyVaultError::InvalidInvestmentAmount)
        ));
        assert!(matches!(
            calculate_token_amount(550_000_000, 100_000_000),
            Err(PropertyVaultError::InvestmentAmountNotDivisible)
        ));
        assert!(matches!(
            calculate_token_amount(100_000_000, 0),
            Err(PropertyVaultError::InvalidTokenPrice)
        ));
        assert!(matches!(
            calculate_token_amount(50_000_000, 100_000_000),
            Err(PropertyVaultError::InvestmentAmountNotDivisible)
        ));
    }

    #[test]
    fn inventory_checks_remaining_tokens() {
        assert!(validate_inventory(10, 5).is_ok());
        assert!(validate_inventory(5, 5).is_ok());
        assert!(matches!(
            validate_inventory(4, 5),
            Err(PropertyVaultError::InsufficientPropertyTokenSupply)
        ));
        assert!(matches!(
            validate_inventory(10, 0),
            Err(PropertyVaultError::InvalidTokenAllocation)
        ));
    }

    #[test]
    fn investment_totals_support_first_and_repeated_investments() {
        assert_eq!(
            next_investment_totals(0, 0, 0, 500_000_000, 5).unwrap(),
            InvestmentTotals {
                total_raised: 500_000_000,
                invested_amount: 500_000_000,
                token_amount: 5,
            }
        );
        assert_eq!(
            next_investment_totals(500_000_000, 500_000_000, 5, 300_000_000, 3).unwrap(),
            InvestmentTotals {
                total_raised: 800_000_000,
                invested_amount: 800_000_000,
                token_amount: 8,
            }
        );
    }

    #[test]
    fn investment_totals_reject_invalid_allocations_and_overflow() {
        assert!(matches!(
            next_investment_totals(0, 0, 0, 0, 1),
            Err(PropertyVaultError::InvalidInvestmentAmount)
        ));
        assert!(matches!(
            next_investment_totals(0, 0, 0, 1, 0),
            Err(PropertyVaultError::InvalidTokenAllocation)
        ));
        assert!(matches!(
            next_investment_totals(u64::MAX, 0, 0, 1, 1),
            Err(PropertyVaultError::ArithmeticOverflow)
        ));
        assert!(matches!(
            next_investment_totals(0, u64::MAX, 0, 1, 1),
            Err(PropertyVaultError::ArithmeticOverflow)
        ));
        assert!(matches!(
            next_investment_totals(0, 0, u64::MAX, 1, 1),
            Err(PropertyVaultError::ArithmeticOverflow)
        ));
    }

    #[test]
    fn withdrawal_returns_new_cumulative_total() {
        assert_eq!(
            next_withdrawn_total(1_000_000_000, 200_000_000, 800_000_000, 300_000_000).unwrap(),
            500_000_000
        );
    }

    #[test]
    fn withdrawal_rejects_invalid_amount_or_accounting() {
        assert!(matches!(
            next_withdrawn_total(100, 0, 100, 0),
            Err(PropertyVaultError::InvalidWithdrawalAmount)
        ));
        assert!(matches!(
            next_withdrawn_total(100, 101, 100, 1),
            Err(PropertyVaultError::InvalidAccountingState)
        ));
        assert!(matches!(
            next_withdrawn_total(100, 20, 100, 81),
            Err(PropertyVaultError::WithdrawalExceedsAvailableCapital)
        ));
        assert!(matches!(
            next_withdrawn_total(100, 20, 50, 60),
            Err(PropertyVaultError::InsufficientInvestmentVaultBalance)
        ));
    }
}
