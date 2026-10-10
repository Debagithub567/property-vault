pub mod error;
pub mod instructions;
pub mod validation;

use anchor_lang::prelude::*;
use anchor_lang::{system_program, ToAccountInfos};
use spl_discriminator::SplDiscriminate;
use spl_tlv_account_resolution::{account::ExtraAccountMeta, state::ExtraAccountMetaList};
use spl_transfer_hook_interface::instruction::{ExecuteInstruction, TransferHookInstruction};

pub use instructions::*;

declare_id!("3eKZN7MKpkU7gjaYFdR8zV6UCr8u7FeBTKBLe2yeM6J5");

#[program]
pub mod property_vault_transfer_hook {
    use super::*;

    pub fn initialize_extra_account_meta_list(
        ctx: Context<InitializeExtraAccountMetaList>,
    ) -> Result<()> {
        validation::validate_property_accounts(
            &ctx.accounts.property,
            &ctx.accounts.mint,
            &ctx.accounts.rent_pool,
            &ctx.accounts.holder_registry,
        )?;
        require_keys_eq!(
            ctx.accounts.payer.key(),
            ctx.accounts.property.owner,
            error::HookError::InvalidPropertyPda
        );

        let extra_accounts = [
            ExtraAccountMeta::new_with_pubkey(&ctx.accounts.property.key(), false, false)?,
            ExtraAccountMeta::new_with_pubkey(&ctx.accounts.rent_pool.key(), false, false)?,
            ExtraAccountMeta::new_with_pubkey(&ctx.accounts.holder_registry.key(), false, false)?,
        ];
        let space = ExtraAccountMetaList::size_of(extra_accounts.len())?;
        let meta_info = ctx.accounts.extra_account_meta_list.to_account_info();
        require_keys_eq!(
            *meta_info.owner,
            System::id(),
            error::HookError::InvalidExtraAccountMetaList
        );
        require!(
            meta_info.data_is_empty(),
            error::HookError::InvalidExtraAccountMetaList
        );

        let mint_key = ctx.accounts.mint.key();
        let bump = [ctx.bumps.extra_account_meta_list];
        let signer_seeds: &[&[u8]] = &[b"extra-account-metas", mint_key.as_ref(), &bump];
        let signer = &[signer_seeds];
        let payer_info = ctx.accounts.payer.to_account_info();
        let system_program_id = ctx.accounts.system_program.key();
        let rent = Rent::get()?.minimum_balance(space);

        if meta_info.lamports() == 0 {
            system_program::create_account(
                CpiContext::new(
                    system_program_id,
                    system_program::CreateAccount {
                        from: payer_info,
                        to: meta_info.clone(),
                    },
                )
                .with_signer(signer),
                rent,
                u64::try_from(space)
                    .map_err(|_| error!(error::HookError::InvalidExtraAccountMetaList))?,
                &crate::ID,
            )?;
        } else {
            let shortfall = rent.saturating_sub(meta_info.lamports());
            if shortfall > 0 {
                system_program::transfer(
                    CpiContext::new(
                        system_program_id,
                        system_program::Transfer {
                            from: payer_info,
                            to: meta_info.clone(),
                        },
                    ),
                    shortfall,
                )?;
            }
            system_program::allocate(
                CpiContext::new(
                    system_program_id,
                    system_program::Allocate {
                        account_to_allocate: meta_info.clone(),
                    },
                )
                .with_signer(signer),
                u64::try_from(space)
                    .map_err(|_| error!(error::HookError::InvalidExtraAccountMetaList))?,
            )?;
            system_program::assign(
                CpiContext::new(
                    system_program_id,
                    system_program::Assign {
                        account_to_assign: meta_info.clone(),
                    },
                )
                .with_signer(signer),
                &crate::ID,
            )?;
        }

        ExtraAccountMetaList::init::<ExecuteInstruction>(
            &mut meta_info.try_borrow_mut_data()?,
            &extra_accounts,
        )?;
        Ok(())
    }

    #[instruction(discriminator = ExecuteInstruction::SPL_DISCRIMINATOR_SLICE)]
    pub fn execute(ctx: Context<Execute>, amount: u64) -> Result<()> {
        require!(amount > 0, error::HookError::InvalidAmount);
        let inventory_authority = validation::validate_property_accounts(
            &ctx.accounts.property,
            &ctx.accounts.mint,
            &ctx.accounts.rent_pool,
            &ctx.accounts.holder_registry,
        )?;
        validation::require_real_transfer(&ctx.accounts.source_token)?;

        let meta_info = ctx.accounts.extra_account_meta_list.to_account_info();
        require_keys_eq!(
            *meta_info.owner,
            crate::ID,
            error::HookError::InvalidExtraAccountMetaList
        );
        let execute_data = TransferHookInstruction::Execute { amount }.pack();
        ExtraAccountMetaList::check_account_infos::<ExecuteInstruction>(
            &ctx.accounts.to_account_infos(),
            &execute_data,
            &crate::ID,
            &meta_info.try_borrow_data()?,
        )?;

        let mint_key = ctx.accounts.mint.key();
        let source_wallet =
            validation::validate_token_account(&ctx.accounts.source_token, &mint_key)?;
        let destination_wallet =
            validation::validate_token_account(&ctx.accounts.destination_token, &mint_key)?;
        require_keys_eq!(
            ctx.accounts.authority.key(),
            source_wallet,
            error::HookError::InvalidTransferAuthority
        );

        validation::require_collection_open(
            Clock::get()?.unix_timestamp,
            ctx.accounts.rent_pool.period_start_ts,
            ctx.accounts.rent_pool.cutoff_ts,
        )?;
        require!(
            destination_wallet != inventory_authority,
            error::HookError::InvalidInventoryTransfer
        );
        if source_wallet == inventory_authority {
            require_keys_eq!(
                ctx.accounts.source_token.key(),
                ctx.accounts.property.inventory_vault,
                error::HookError::InvalidInventoryTransfer
            );
        } else {
            require!(
                validation::is_registered(&ctx.accounts.holder_registry, &source_wallet),
                error::HookError::UnregisteredSource
            );
        }
        require!(
            validation::is_registered(&ctx.accounts.holder_registry, &destination_wallet),
            error::HookError::UnregisteredDestination
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::Discriminator;

    #[test]
    fn execute_uses_the_spl_interface_discriminator() {
        assert_eq!(
            crate::instruction::Execute::DISCRIMINATOR,
            ExecuteInstruction::SPL_DISCRIMINATOR_SLICE
        );
        let packed = TransferHookInstruction::Execute { amount: 42 }.pack();
        assert_eq!(&packed[..8], crate::instruction::Execute::DISCRIMINATOR);
        assert_eq!(&packed[8..], &42_u64.to_le_bytes());
    }

    #[test]
    fn three_extra_accounts_fit_the_tlv_allocation() {
        let keys = [
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        ];
        let metas = keys
            .iter()
            .map(|key| ExtraAccountMeta::new_with_pubkey(key, false, false).unwrap())
            .collect::<Vec<_>>();
        let space = ExtraAccountMetaList::size_of(metas.len()).unwrap();
        assert_eq!(space, 121);
        let mut data = vec![0; space];
        ExtraAccountMetaList::init::<ExecuteInstruction>(&mut data, &metas).unwrap();
    }
}
