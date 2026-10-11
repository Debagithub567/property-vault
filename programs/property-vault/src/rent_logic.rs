use crate::constants::MAX_HOLDERS;
use crate::error::PropertyVaultError;

pub fn checked_cutoff(start_ts: i64, duration_seconds: u64) -> Result<i64, PropertyVaultError> {
    if duration_seconds == 0 {
        return Err(PropertyVaultError::InvalidRentDuration);
    }
    let duration =
        i64::try_from(duration_seconds).map_err(|_| PropertyVaultError::InvalidRentDuration)?;
    start_ts
        .checked_add(duration)
        .ok_or(PropertyVaultError::ArithmeticOverflow)
}

pub fn require_collection_open(now: i64, cutoff_ts: i64) -> Result<(), PropertyVaultError> {
    if now >= cutoff_ts {
        return Err(PropertyVaultError::RentCollectionClosed);
    }
    Ok(())
}

pub fn require_finalizable(now: i64, cutoff_ts: i64) -> Result<(), PropertyVaultError> {
    if now < cutoff_ts {
        return Err(PropertyVaultError::RentPeriodNotReady);
    }
    Ok(())
}

pub fn split_rent(
    deposits: u64,
    investor_bps: u16,
    eligible_supply: u64,
) -> Result<(u64, u64), PropertyVaultError> {
    if !(1..10_000).contains(&investor_bps) {
        return Err(PropertyVaultError::InvalidInvestorAllocation);
    }
    if eligible_supply == 0 {
        return Ok((0, deposits));
    }
    let investor = u64::try_from(
        u128::from(deposits)
            .checked_mul(u128::from(investor_bps))
            .ok_or(PropertyVaultError::ArithmeticOverflow)?
            / 10_000,
    )
    .map_err(|_| PropertyVaultError::ArithmeticOverflow)?;
    let owner = deposits
        .checked_sub(investor)
        .ok_or(PropertyVaultError::ArithmeticOverflow)?;
    Ok((investor, owner))
}

// Largest remainder: floor every holder's exact share, then give leftover base
// units to the largest fractional remainders, breaking ties by registry index.
pub fn holder_entitlements(
    investor_amount: u64,
    balances: &[u64; MAX_HOLDERS],
    count: usize,
    eligible_supply: u64,
) -> Result<[u64; MAX_HOLDERS], PropertyVaultError> {
    if count > MAX_HOLDERS {
        return Err(PropertyVaultError::InvalidRentAccountingState);
    }
    let mut entitlements = [0u64; MAX_HOLDERS];
    if eligible_supply == 0 {
        return if investor_amount == 0 && balances[..count].iter().all(|balance| *balance == 0) {
            Ok(entitlements)
        } else {
            Err(PropertyVaultError::InvalidRentAccountingState)
        };
    }
    if balances[..count]
        .iter()
        .try_fold(0u64, |sum, balance| sum.checked_add(*balance))
        != Some(eligible_supply)
    {
        return Err(PropertyVaultError::InvalidRentAccountingState);
    }
    let mut remainders = [0u64; MAX_HOLDERS];
    let mut assigned = 0u64;
    for index in 0..count {
        let product = u128::from(investor_amount)
            .checked_mul(u128::from(balances[index]))
            .ok_or(PropertyVaultError::ArithmeticOverflow)?;
        entitlements[index] = u64::try_from(product / u128::from(eligible_supply))
            .map_err(|_| PropertyVaultError::ArithmeticOverflow)?;
        remainders[index] = u64::try_from(product % u128::from(eligible_supply))
            .map_err(|_| PropertyVaultError::ArithmeticOverflow)?;
        assigned = assigned
            .checked_add(entitlements[index])
            .ok_or(PropertyVaultError::ArithmeticOverflow)?;
    }
    let leftover = investor_amount
        .checked_sub(assigned)
        .ok_or(PropertyVaultError::InvalidRentAccountingState)?;
    if leftover > u64::try_from(count).map_err(|_| PropertyVaultError::ArithmeticOverflow)? {
        return Err(PropertyVaultError::InvalidRentAccountingState);
    }
    for _ in 0..leftover {
        let index = (0..count)
            .filter(|index| balances[*index] > 0)
            .max_by_key(|index| (remainders[*index], MAX_HOLDERS - *index))
            .ok_or(PropertyVaultError::InvalidRentAccountingState)?;
        entitlements[index] = entitlements[index]
            .checked_add(1)
            .ok_or(PropertyVaultError::ArithmeticOverflow)?;
        remainders[index] = 0;
    }
    Ok(entitlements)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cutoff_boundary_is_exclusive_for_collection() {
        assert!(require_collection_open(99, 100).is_ok());
        assert!(matches!(
            require_collection_open(100, 100),
            Err(PropertyVaultError::RentCollectionClosed)
        ));
        assert!(matches!(
            require_finalizable(99, 100),
            Err(PropertyVaultError::RentPeriodNotReady)
        ));
        assert!(require_finalizable(100, 100).is_ok());
    }

    #[test]
    fn cutoff_rejects_zero_and_overflow() {
        assert_eq!(checked_cutoff(100, 60).unwrap(), 160);
        assert!(matches!(
            checked_cutoff(100, 0),
            Err(PropertyVaultError::InvalidRentDuration)
        ));
        assert!(matches!(
            checked_cutoff(i64::MAX, 1),
            Err(PropertyVaultError::ArithmeticOverflow)
        ));
    }

    #[test]
    fn entitlement_handles_zero_rent_and_rounding() {
        let mut balances = [0; MAX_HOLDERS];
        balances[0..3].copy_from_slice(&[1, 1, 1]);
        assert_eq!(
            holder_entitlements(2, &balances, 3, 3).unwrap()[..3],
            [1, 1, 0]
        );
        assert_eq!(
            holder_entitlements(0, &balances, 3, 3).unwrap()[..3],
            [0, 0, 0]
        );
        assert_eq!(
            split_rent(10_000_000_000, 8_000, 400).unwrap(),
            (8_000_000_000, 2_000_000_000)
        );
        assert_eq!(split_rent(7, 8_000, 0).unwrap(), (0, 7));
        assert_eq!(split_rent(1, 8_000, 1).unwrap(), (0, 1));
        assert_eq!(
            split_rent(u64::MAX, 9_999, 1).unwrap().0,
            18_444_899_399_302_180_659
        );
        assert!(matches!(
            split_rent(100, 10_000, 1),
            Err(PropertyVaultError::InvalidInvestorAllocation)
        ));
        assert!(matches!(
            holder_entitlements(100, &balances, 3, 0),
            Err(PropertyVaultError::InvalidRentAccountingState)
        ));
        assert!(matches!(
            holder_entitlements(100, &balances, 3, 4),
            Err(PropertyVaultError::InvalidRentAccountingState)
        ));
    }
}
