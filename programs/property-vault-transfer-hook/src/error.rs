use anchor_lang::prelude::*;

#[error_code]
pub enum HookError {
    #[msg("Invalid or already initialized ExtraAccountMetaList")]
    InvalidExtraAccountMetaList,
    #[msg("Invalid canonical Property PDA or bump")]
    InvalidPropertyPda,
    #[msg("Invalid property mint PDA or account")]
    InvalidPropertyMint,
    #[msg("The mint does not use this Transfer Hook program")]
    InvalidTransferHookProgram,
    #[msg("Invalid RentPool PDA, period, or property relationship")]
    InvalidRentPool,
    #[msg("Invalid HolderRegistry PDA or property relationship")]
    InvalidHolderRegistry,
    #[msg("The holder registry exceeds its limit or contains duplicate wallets")]
    MalformedHolderRegistry,
    #[msg("The source or destination is not a canonical Token-2022 associated token account")]
    InvalidTokenAccount,
    #[msg("Transfer authority does not own the source token account")]
    InvalidTransferAuthority,
    #[msg("The token account is not in a real Token-2022 transfer")]
    NotTransferring,
    #[msg("The source wallet is not registered")]
    UnregisteredSource,
    #[msg("The destination wallet is not registered")]
    UnregisteredDestination,
    #[msg("Inventory may only be the source of a property token transfer")]
    InvalidInventoryTransfer,
    #[msg("Property-token transfers are closed at this period's cutoff")]
    CollectionClosed,
    #[msg("The rental collection period has not started")]
    CollectionNotStarted,
    #[msg("Transfer amount must be positive")]
    InvalidAmount,
}
