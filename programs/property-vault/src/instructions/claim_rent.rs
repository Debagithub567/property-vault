use anchor_lang::prelude::*;
use anchor_spl::token_interface::TokenInterface;

use crate::{constants::*};

#[derive(Accounts)]
pub struct ClaimRent<'info> {
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl<'info> ClaimRent<'info> {
    pub fn handle_claim_rent(&self) -> Result<()> {
        Ok(())
    }
}