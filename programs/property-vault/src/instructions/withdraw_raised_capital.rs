use anchor_lang::prelude::*;
use anchor_spl::token_interface::TokenInterface;

use crate::{constants::*};

#[derive(Accounts)]
pub struct WithdrawRaisedCapital<'info> {
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl<'info> WithdrawRaisedCapital<'info> {
    pub fn handle_withdraw_raised_capital(&self) -> Result<()> {
        Ok(())
    }
}