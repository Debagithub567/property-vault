pub mod constants;
pub mod error;
pub mod instructions;
pub mod logic;
pub mod rent_logic;
pub mod state;
pub mod token_transfer;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("DA1iVaPWd7epSAong3cGz8jX7rRJMT42zfrHWo3z5123");

#[program]
pub mod property_vault {
    use super::*;

    pub fn initialize_config(ctx: Context<InitializeConfig>, mock_usdc_mint: Pubkey) -> Result<()> {
        instructions::initialize_config::handle_initialize_config(ctx, mock_usdc_mint)
    }

    pub fn list_and_tokenize_property(
        ctx: Context<ListAndTokenizeProperty>,
        property_id: u64,
        tenant: Pubkey,
        token_price: u64,
        max_supply: u64,
        terms_hash: [u8; 32],
        duration_seconds: u64,
    ) -> Result<()> {
        instructions::list_and_tokenize_property::handle_list(
            ctx,
            property_id,
            tenant,
            token_price,
            max_supply,
            terms_hash,
            duration_seconds,
        )
    }

    pub fn register_holder<'info>(
        ctx: Context<'info, RegisterHolder<'info>>,
        replace_index: Option<u8>,
    ) -> Result<()> {
        instructions::register_holder::handle_register_holder(ctx, replace_index)
    }

    pub fn invest(ctx: Context<Invest>, amount: u64, accepted_terms_hash: [u8; 32]) -> Result<()> {
        instructions::invest::handle_invest(ctx, amount, accepted_terms_hash)
    }

    pub fn transfer_property_tokens(
        ctx: Context<TransferPropertyTokens>,
        amount: u64,
    ) -> Result<()> {
        instructions::transfer_property_tokens::handle_transfer_property_tokens(ctx, amount)
    }

    pub fn withdraw_raised_capital(ctx: Context<WithdrawRaisedCapital>, amount: u64) -> Result<()> {
        instructions::withdraw_raised_capital::handle_withdraw_raised_capital(ctx, amount)
    }

    pub fn pay_rent(ctx: Context<PayRent>, amount: u64) -> Result<()> {
        instructions::pay_rent::handle_pay_rent(ctx, amount)
    }

    pub fn finalize_rent_period(ctx: Context<FinalizeRentPeriod>) -> Result<()> {
        instructions::finalize_rent_period::handle_finalize_rent_period(ctx)
    }

    pub fn claim_rent(ctx: Context<ClaimRent>, period_id: u64) -> Result<()> {
        instructions::claim_rent::handle_claim_rent(ctx, period_id)
    }
}
