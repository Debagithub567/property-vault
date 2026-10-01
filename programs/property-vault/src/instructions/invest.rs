use anchor_lang::prelude::*;
use anchor_spl::token_interface::TokenInterface;

use crate::{constants::*};

#[derive(Accounts)]
pub struct Invest<'info> {
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl<'info> Invest<'info> {
    pub fn handle_invest(&self) -> Result<()> {
        Ok(())
    }
}