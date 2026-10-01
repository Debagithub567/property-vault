use anchor_lang::prelude::*;
use anchor_spl::token_interface::TokenInterface;

use crate::{constants::*};

#[derive(Accounts)]
pub struct ListAndTokenizeProperty<'info> {
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl<'info> ListAndTokenizeProperty<'info> {
    pub fn handle_list_and_tokenize_property(&self) -> Result<()> {
        Ok(())
    }
}