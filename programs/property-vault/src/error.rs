use anchor_lang::prelude::*;

#[error_code]
pub enum PropertyVaultError {
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Investment amount must be positive")]
    InvalidInvestmentAmount,
    #[msg("Accepted terms do not match the current property terms")]
    StaleTerms,
    #[msg("Property token price must be positive")]
    InvalidTokenPrice,
    #[msg("Investment amount must purchase a whole number of property tokens")]
    InvestmentAmountNotDivisible,
    #[msg("Property token allocation must be positive")]
    InvalidTokenAllocation,
    #[msg("Insufficient property token inventory")]
    InsufficientPropertyTokenSupply,
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,
    #[msg("Investment pool accounting is inconsistent")]
    InvalidAccountingState,
    #[msg("Withdrawal amount must be positive")]
    InvalidWithdrawalAmount,
    #[msg("Withdrawal exceeds available raised capital")]
    WithdrawalExceedsAvailableCapital,
    #[msg("Investment vault balance is insufficient for withdrawal")]
    InsufficientInvestmentVaultBalance,
    #[msg("Rent payment amount must be positive")]
    InvalidRentPaymentAmount,
    #[msg("Tenant mock-USDC balance is insufficient")]
    InsufficientTenantUsdcBalance,
    #[msg("This instruction is not implemented yet")]
    NotImplemented,
    #[msg("Invalid property supply")]
    InvalidMaxSupply,
    #[msg("Investor rental allocation must be between 1 and 9,999 basis points")]
    InvalidInvestorAllocation,
    #[msg("Accepted investor rental allocation does not match the property")]
    StaleInvestorAllocation,
    #[msg("Invalid tenant")]
    InvalidTenant,
    #[msg("Invalid rent collection duration")]
    InvalidRentDuration,
    #[msg("Rent collection has ended for this period")]
    RentCollectionClosed,
    #[msg("Rent collection is still active")]
    RentPeriodNotReady,
    #[msg("Holder registry is full")]
    HolderRegistryFull,
    #[msg("Holder is not registered")]
    HolderNotRegistered,
    #[msg("Holder is already registered")]
    HolderAlreadyRegistered,
    #[msg("Token account does not match the holder's canonical associated token account")]
    InvalidHolderTokenAccount,
    #[msg("Token account is not frozen at rest")]
    TokenAccountNotFrozen,
    #[msg("Mock USDC mint must use the legacy token program and six decimals")]
    InvalidMockUsdcMint,
    #[msg("Invalid property token mint")]
    InvalidPropertyMint,
    #[msg("Invalid protocol account or PDA")]
    InvalidProtocolAccount,
    #[msg("Rent pool accounting is inconsistent")]
    InvalidRentAccountingState,
    #[msg("Registered holder token accounts do not form a complete valid snapshot")]
    InvalidHolderSnapshot,
    #[msg("Rent vault cannot cover reserved and current-period rent")]
    InsufficientRentVaultBalance,
    #[msg("Rent entitlement has already been claimed")]
    RentAlreadyClaimed,
    #[msg("No rent entitlement exists for this holder")]
    NoRentEntitlement,
    #[msg("No finalized owner rent is available to claim")]
    NoOwnerRentEntitlement,
    #[msg("Transfer amount must be positive")]
    InvalidTransferAmount,
    #[msg("Property token balance is insufficient")]
    InsufficientPropertyTokenBalance,
    #[msg("Transfer Hook metadata is missing or invalid")]
    InvalidHookMetadata,
    #[msg("Holder registry replacement index is invalid")]
    InvalidRegistryIndex,
    #[msg("The replaced holder still owns property tokens")]
    HolderStillOwnsTokens,
}
