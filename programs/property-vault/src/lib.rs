pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("5dMnfwj9iPrxpuFMnbWw3DnPi8S1h2axN6XxEv9VqdF5");

#[program]
pub mod property_vault {
    use super::*;

    pub fn handle_list_and_tokenize_property(ctx: Context<ListAndTokenizeProperty>) -> Result<()> {
        ctx.accounts.handle_list_and_tokenize_property()
    }

    pub fn invest(ctx: Context<Invest>) -> Result<()> {
        ctx.accounts.handle_invest()
    }

    pub fn withdraw_raised_capital(ctx: Context<WithdrawRaisedCapital>) -> Result<()> {
        ctx.accounts.handle_withdraw_raised_capital()
    }

    pub fn pay_rent(ctx: Context<PayRent>) -> Result<()> {
        ctx.accounts.handle_pay_rent()
    }

    pub fn claim_rent(ctx: Context<ClaimRent>) -> Result<()> {
        ctx.accounts.handle_claim_rent()
    }
}