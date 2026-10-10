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

pub fn distributable_rent(current_deposits: u64, carry: u64) -> Result<u64, PropertyVaultError> {
    current_deposits
        .checked_add(carry)
        .ok_or(PropertyVaultError::ArithmeticOverflow)
}

pub fn holder_entitlement(
    distributable: u64,
    holder_balance: u64,
    eligible_supply: u64,
) -> Result<u64, PropertyVaultError> {
    if eligible_supply == 0 {
        return if holder_balance == 0 {
            Ok(0)
        } else {
            Err(PropertyVaultError::InvalidRentAccountingState)
        };
    }
    if holder_balance > eligible_supply {
        return Err(PropertyVaultError::InvalidRentAccountingState);
    }
    let product = u128::from(distributable)
        .checked_mul(u128::from(holder_balance))
        .ok_or(PropertyVaultError::ArithmeticOverflow)?;
    u64::try_from(product / u128::from(eligible_supply))
        .map_err(|_| PropertyVaultError::ArithmeticOverflow)
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
        assert_eq!(holder_entitlement(0, 2, 3).unwrap(), 0);
        assert_eq!(holder_entitlement(100, 2, 3).unwrap(), 66);
        assert_eq!(holder_entitlement(100, 0, 0).unwrap(), 0);
        assert!(matches!(
            holder_entitlement(100, 1, 0),
            Err(PropertyVaultError::InvalidRentAccountingState)
        ));
        assert!(matches!(
            holder_entitlement(100, 4, 3),
            Err(PropertyVaultError::InvalidRentAccountingState)
        ));
    }
}
