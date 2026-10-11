use anchor_lang::solana_program::program_pack::Pack;
use anchor_lang::{AccountDeserialize, AccountSerialize, InstructionData, ToAccountMetas};
use anchor_spl::{
    associated_token::{
        get_associated_token_address_with_program_id, spl_associated_token_account,
    },
    token::{self, spl_token},
    token_2022::{self, spl_token_2022},
};
use property_vault::{accounts, constants::*, instruction, state::*, ID as VAULT_ID};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_keypair::{read_keypair_file, Keypair};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::{
    account::{Account, AccountSharedData},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_signer::Signer;
use solana_system_interface::instruction as system_instruction;
use solana_transaction::Transaction;
use spl_tlv_account_resolution::{account::ExtraAccountMeta, state::ExtraAccountMetaList};
use spl_token_2022::extension::{
    immutable_owner::ImmutableOwner,
    transfer_hook::{self, TransferHook, TransferHookAccount},
    BaseStateWithExtensions, StateWithExtensions, StateWithExtensionsMut,
};
use spl_transfer_hook_interface::instruction::ExecuteInstruction;
use spl_type_length_value::state::TlvStateBorrowed;

const HOOK_ID: Pubkey = Pubkey::from_str_const("3eKZN7MKpkU7gjaYFdR8zV6UCr8u7FeBTKBLe2yeM6J5");

struct ListingKeys {
    property: Pubkey,
    mint: Pubkey,
    inventory_authority: Pubkey,
    freeze_authority: Pubkey,
    inventory_vault: Pubkey,
    investment_pool: Pubkey,
    investment_vault: Pubkey,
    rent_pool: Pubkey,
    rent_vault: Pubkey,
    registry: Pubkey,
    meta_list: Pubkey,
}

fn listing_keys(owner: Pubkey, property_id: u64, mock_mint: Pubkey) -> ListingKeys {
    let property = Pubkey::find_program_address(
        &[PROPERTY_SEED, owner.as_ref(), &property_id.to_le_bytes()],
        &VAULT_ID,
    )
    .0;
    let mint = Pubkey::find_program_address(&[PROPERTY_MINT_SEED, property.as_ref()], &VAULT_ID).0;
    let inventory_authority =
        Pubkey::find_program_address(&[INVENTORY_AUTHORITY_SEED, property.as_ref()], &VAULT_ID).0;
    let freeze_authority =
        Pubkey::find_program_address(&[FREEZE_AUTHORITY_SEED, property.as_ref()], &VAULT_ID).0;
    let inventory_vault =
        get_associated_token_address_with_program_id(&inventory_authority, &mint, &token_2022::ID);
    let investment_pool =
        Pubkey::find_program_address(&[INVESTMENT_POOL_SEED, property.as_ref()], &VAULT_ID).0;
    let rent_pool = Pubkey::find_program_address(&[RENT_POOL_SEED, property.as_ref()], &VAULT_ID).0;
    let registry =
        Pubkey::find_program_address(&[HOLDER_REGISTRY_SEED, property.as_ref()], &VAULT_ID).0;
    ListingKeys {
        property,
        mint,
        inventory_authority,
        freeze_authority,
        inventory_vault,
        investment_pool,
        investment_vault: get_associated_token_address_with_program_id(
            &investment_pool,
            &mock_mint,
            &token::ID,
        ),
        rent_pool,
        rent_vault: get_associated_token_address_with_program_id(
            &rent_pool,
            &mock_mint,
            &token::ID,
        ),
        registry,
        meta_list: Pubkey::find_program_address(
            &[EXTRA_ACCOUNT_METAS_SEED, mint.as_ref()],
            &HOOK_ID,
        )
        .0,
    }
}

fn list_instruction(
    owner: Pubkey,
    config: Pubkey,
    mock_mint: Pubkey,
    property_id: u64,
    tenant: Pubkey,
    price: u64,
    supply: u64,
    duration: u64,
) -> (Instruction, ListingKeys) {
    let keys = listing_keys(owner, property_id, mock_mint);
    let ix = Instruction {
        program_id: VAULT_ID,
        accounts: accounts::ListAndTokenizeProperty {
            owner,
            config,
            property: keys.property,
            property_mint: keys.mint,
            inventory_authority: keys.inventory_authority,
            freeze_authority: keys.freeze_authority,
            inventory_vault: keys.inventory_vault,
            investment_pool: keys.investment_pool,
            investment_vault: keys.investment_vault,
            rent_pool: keys.rent_pool,
            rent_vault: keys.rent_vault,
            holder_registry: keys.registry,
            mock_usdc_mint: mock_mint,
            token_2022_program: token_2022::ID,
            legacy_token_program: token::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: instruction::ListAndTokenizeProperty {
            property_id,
            tenant,
            token_price: price,
            max_supply: supply,
            terms_hash: [7; 32],
            duration_seconds: duration,
            investor_allocation_bps: 8_000,
        }
        .data(),
    };
    (ix, keys)
}

fn hook_initialize_instruction(owner: Pubkey, keys: &ListingKeys) -> Instruction {
    Instruction {
        program_id: HOOK_ID,
        accounts: vec![
            AccountMeta::new(owner, true),
            AccountMeta::new(keys.meta_list, false),
            AccountMeta::new_readonly(keys.mint, false),
            AccountMeta::new_readonly(keys.property, false),
            AccountMeta::new_readonly(keys.rent_pool, false),
            AccountMeta::new_readonly(keys.registry, false),
            AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
        ],
        data: vec![92, 197, 174, 197, 41, 124, 19, 3],
    }
}

fn initialize_config_instruction(admin: Pubkey, mock_usdc_mint: Pubkey) -> Instruction {
    Instruction {
        program_id: VAULT_ID,
        accounts: accounts::InitializeConfig {
            admin,
            config: Pubkey::find_program_address(&[CONFIG_SEED], &VAULT_ID).0,
            mock_usdc_mint_account: mock_usdc_mint,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: instruction::InitializeConfig { mock_usdc_mint }.data(),
    }
}

#[derive(Debug)]
struct TxReceipt {
    compute_units: u64,
    logs: Vec<String>,
}

async fn send(
    context: &mut ProgramTestContext,
    instructions: Vec<Instruction>,
    extra: &[&Keypair],
) -> Result<TxReceipt, String> {
    let blockhash = context.banks_client.get_latest_blockhash().await.unwrap();
    let mut signers = vec![&context.payer];
    signers.extend_from_slice(extra);
    let tx = Transaction::new_signed_with_payer(
        &instructions,
        Some(&context.payer.pubkey()),
        &signers,
        blockhash,
    );
    let outcome = context
        .banks_client
        .process_transaction_with_metadata(tx)
        .await
        .map_err(|e| format!("{e:?}"))?;
    let metadata = outcome
        .metadata
        .ok_or_else(|| format!("transaction was not executed: {:?}", outcome.result))?;
    outcome
        .result
        .map_err(|e| format!("{e:?}; logs: {:?}", metadata.log_messages))?;
    Ok(TxReceipt {
        compute_units: metadata.compute_units_consumed,
        logs: metadata.log_messages,
    })
}

async fn account(context: &mut ProgramTestContext, key: Pubkey) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .unwrap()
        .unwrap()
}

fn decode<T: AccountDeserialize>(data: &[u8]) -> T {
    T::try_deserialize(&mut &data[..]).unwrap()
}

fn register_instruction(
    holder: Pubkey,
    keys: &ListingKeys,
    replace_index: Option<u8>,
    previous_holder_token: Option<Pubkey>,
) -> Instruction {
    Instruction {
        program_id: VAULT_ID,
        accounts: accounts::RegisterHolder {
            holder,
            property: keys.property,
            mint: keys.mint,
            registry: keys.registry,
            rent_pool: keys.rent_pool,
            holder_token: get_associated_token_address_with_program_id(
                &holder,
                &keys.mint,
                &token_2022::ID,
            ),
            freeze_authority: keys.freeze_authority,
            token_2022_program: token_2022::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: anchor_lang::system_program::ID,
            previous_holder_token,
        }
        .to_account_metas(None),
        data: instruction::RegisterHolder { replace_index }.data(),
    }
}

fn invest_instruction(
    investor: Pubkey,
    keys: &ListingKeys,
    mock_mint: Pubkey,
    amount: u64,
    accepted_terms_hash: [u8; 32],
) -> Instruction {
    let position = Pubkey::find_program_address(
        &[POSITION_SEED, keys.property.as_ref(), investor.as_ref()],
        &VAULT_ID,
    )
    .0;
    Instruction {
        program_id: VAULT_ID,
        accounts: accounts::Invest {
            investor,
            property: keys.property,
            mint: keys.mint,
            inventory_authority: keys.inventory_authority,
            inventory_vault: keys.inventory_vault,
            freeze_authority: keys.freeze_authority,
            investor_property_token: get_associated_token_address_with_program_id(
                &investor,
                &keys.mint,
                &token_2022::ID,
            ),
            registry: keys.registry,
            rent_pool: keys.rent_pool,
            investment_pool: keys.investment_pool,
            position,
            mock_usdc_mint: mock_mint,
            investor_usdc: get_associated_token_address_with_program_id(
                &investor,
                &mock_mint,
                &token::ID,
            ),
            investment_vault: keys.investment_vault,
            extra_account_meta_list: keys.meta_list,
            hook_program: HOOK_ID,
            token_2022_program: token_2022::ID,
            legacy_token_program: token::ID,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: instruction::Invest {
            amount,
            accepted_terms_hash,
            accepted_investor_allocation_bps: 8_000,
        }
        .data(),
    }
}

fn secondary_instruction(
    source: Pubkey,
    destination: Pubkey,
    keys: &ListingKeys,
    amount: u64,
) -> Instruction {
    Instruction {
        program_id: VAULT_ID,
        accounts: accounts::TransferPropertyTokens {
            source_holder: source,
            property: keys.property,
            mint: keys.mint,
            source_token: get_associated_token_address_with_program_id(
                &source,
                &keys.mint,
                &token_2022::ID,
            ),
            destination_token: get_associated_token_address_with_program_id(
                &destination,
                &keys.mint,
                &token_2022::ID,
            ),
            freeze_authority: keys.freeze_authority,
            registry: keys.registry,
            rent_pool: keys.rent_pool,
            extra_account_meta_list: keys.meta_list,
            hook_program: HOOK_ID,
            property_token_program: token_2022::ID,
        }
        .to_account_metas(None),
        data: instruction::TransferPropertyTokens { amount }.data(),
    }
}

fn withdraw_instruction(
    owner: Pubkey,
    keys: &ListingKeys,
    mock_mint: Pubkey,
    amount: u64,
) -> Instruction {
    Instruction {
        program_id: VAULT_ID,
        accounts: accounts::WithdrawRaisedCapital {
            owner,
            property: keys.property,
            investment_pool: keys.investment_pool,
            investment_vault: keys.investment_vault,
            owner_usdc: get_associated_token_address_with_program_id(
                &owner,
                &mock_mint,
                &token::ID,
            ),
            mock_usdc_mint: mock_mint,
            legacy_token_program: token::ID,
        }
        .to_account_metas(None),
        data: instruction::WithdrawRaisedCapital { amount }.data(),
    }
}

fn pay_rent_instruction(
    tenant: Pubkey,
    keys: &ListingKeys,
    mock_mint: Pubkey,
    amount: u64,
) -> Instruction {
    Instruction {
        program_id: VAULT_ID,
        accounts: accounts::PayRent {
            tenant,
            property: keys.property,
            rent_pool: keys.rent_pool,
            rent_vault: keys.rent_vault,
            tenant_usdc: get_associated_token_address_with_program_id(
                &tenant,
                &mock_mint,
                &token::ID,
            ),
            mock_usdc_mint: mock_mint,
            legacy_token_program: token::ID,
        }
        .to_account_metas(None),
        data: instruction::PayRent { amount }.data(),
    }
}

fn finalize_instruction(
    payer: Pubkey,
    keys: &ListingKeys,
    mock_mint: Pubkey,
    period_id: u64,
    holders: &[Pubkey],
) -> Instruction {
    let rent_period = Pubkey::find_program_address(
        &[
            RENT_PERIOD_SEED,
            keys.property.as_ref(),
            &period_id.to_le_bytes(),
        ],
        &VAULT_ID,
    )
    .0;
    let mut accounts = accounts::FinalizeRentPeriod {
        payer,
        property: keys.property,
        mint: keys.mint,
        inventory_vault: keys.inventory_vault,
        registry: keys.registry,
        rent_pool: keys.rent_pool,
        mock_usdc_mint: mock_mint,
        rent_vault: keys.rent_vault,
        rent_period,
        system_program: anchor_lang::system_program::ID,
        token_2022_program: token_2022::ID,
    }
    .to_account_metas(None);
    accounts.extend(holders.iter().map(|holder| {
        AccountMeta::new_readonly(
            get_associated_token_address_with_program_id(holder, &keys.mint, &token_2022::ID),
            false,
        )
    }));
    Instruction {
        program_id: VAULT_ID,
        accounts,
        data: instruction::FinalizeRentPeriod {}.data(),
    }
}

fn claim_instruction(
    holder: Pubkey,
    keys: &ListingKeys,
    mock_mint: Pubkey,
    period_id: u64,
) -> Instruction {
    let rent_period = Pubkey::find_program_address(
        &[
            RENT_PERIOD_SEED,
            keys.property.as_ref(),
            &period_id.to_le_bytes(),
        ],
        &VAULT_ID,
    )
    .0;
    let rent_claim = Pubkey::find_program_address(
        &[
            RENT_CLAIM_SEED,
            keys.property.as_ref(),
            &period_id.to_le_bytes(),
            holder.as_ref(),
        ],
        &VAULT_ID,
    )
    .0;
    Instruction {
        program_id: VAULT_ID,
        accounts: accounts::ClaimRent {
            holder,
            property: keys.property,
            rent_pool: keys.rent_pool,
            rent_period,
            rent_claim,
            mock_usdc_mint: mock_mint,
            rent_vault: keys.rent_vault,
            holder_usdc: get_associated_token_address_with_program_id(
                &holder,
                &mock_mint,
                &token::ID,
            ),
            legacy_token_program: token::ID,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: instruction::ClaimRent { period_id }.data(),
    }
}

fn owner_claim_instruction(owner: Pubkey, keys: &ListingKeys, mock_mint: Pubkey) -> Instruction {
    Instruction {
        program_id: VAULT_ID,
        accounts: accounts::ClaimOwnerRent {
            owner,
            property: keys.property,
            rent_pool: keys.rent_pool,
            mock_usdc_mint: mock_mint,
            rent_vault: keys.rent_vault,
            owner_usdc: get_associated_token_address_with_program_id(
                &owner,
                &mock_mint,
                &token::ID,
            ),
            legacy_token_program: token::ID,
        }
        .to_account_metas(None),
        data: instruction::ClaimOwnerRent {}.data(),
    }
}

async fn set_clock_to_cutoff(context: &mut ProgramTestContext, rent_pool: Pubkey) {
    let mut clock: anchor_lang::prelude::Clock = context.banks_client.get_sysvar().await.unwrap();
    let pool: RentPool = decode(&account(context, rent_pool).await.data);
    clock.unix_timestamp = pool.cutoff_ts;
    context.set_sysvar(&clock);
}

async fn usdc_balance(context: &mut ProgramTestContext, address: Pubkey) -> u64 {
    spl_token::state::Account::unpack(&account(context, address).await.data)
        .unwrap()
        .amount
}

fn with_transfer_budget(instruction: Instruction) -> Vec<Instruction> {
    vec![
        ComputeBudgetInstruction::set_compute_unit_limit(400_000),
        instruction,
    ]
}

async fn property_balance(context: &mut ProgramTestContext, token: Pubkey) -> (u64, bool) {
    let data = account(context, token).await.data;
    let state = StateWithExtensions::<spl_token_2022::state::Account>::unpack(&data).unwrap();
    (
        state.base.amount,
        state.base.state == spl_token_2022::state::AccountState::Frozen,
    )
}

fn temporarily_thaw_account(context: &mut ProgramTestContext, key: Pubkey, mut account: Account) {
    let mut state =
        StateWithExtensionsMut::<spl_token_2022::state::Account>::unpack(&mut account.data)
            .unwrap();
    state.base.state = spl_token_2022::state::AccountState::Initialized;
    state.pack_base();
    context.set_account(&key, &AccountSharedData::from(account));
}

async fn fund_investor(
    context: &mut ProgramTestContext,
    investor: &Keypair,
    mint: Pubkey,
    amount: u64,
) {
    let payer = context.payer.pubkey();
    let ata = get_associated_token_address_with_program_id(&investor.pubkey(), &mint, &token::ID);
    send(
        context,
        vec![
            system_instruction::transfer(&payer, &investor.pubkey(), 2_000_000_000),
            spl_associated_token_account::instruction::create_associated_token_account(
                &payer,
                &investor.pubkey(),
                &mint,
                &token::ID,
            ),
            spl_token::instruction::mint_to(&token::ID, &mint, &ata, &payer, &[], amount).unwrap(),
        ],
        &[],
    )
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "requires SBF artifacts and PROPERTY_VAULT_BOOTSTRAP_KEYPAIR"]
async fn config_initialization_requires_authorized_signer_and_canonical_legacy_mint() {
    let wallet_path = std::env::var("PROPERTY_VAULT_BOOTSTRAP_KEYPAIR")
        .expect("set PROPERTY_VAULT_BOOTSTRAP_KEYPAIR to the authorized wallet file path");
    let admin = read_keypair_file(wallet_path).expect("read bootstrap test wallet");
    assert_eq!(admin.pubkey(), BOOTSTRAP_ADMIN);

    let sbf_dir = format!("{}/../../target/deploy", env!("CARGO_MANIFEST_DIR"));
    std::env::set_var("BPF_OUT_DIR", &sbf_dir);
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("property_vault", VAULT_ID, None);
    let mut context = test.start_with_context().await;
    let payer = context.payer.pubkey();
    let unauthorized = Keypair::new();
    let valid_mint = Keypair::new();
    let wrong_decimals_mint = Keypair::new();
    let wrong_owner_mint = Keypair::new();
    let rent = context.banks_client.get_rent().await.unwrap();
    send(
        &mut context,
        vec![
            system_instruction::transfer(&payer, &admin.pubkey(), 1_000_000_000),
            system_instruction::transfer(&payer, &unauthorized.pubkey(), 10_000_000),
            system_instruction::create_account(
                &payer,
                &valid_mint.pubkey(),
                rent.minimum_balance(spl_token::state::Mint::LEN),
                spl_token::state::Mint::LEN as u64,
                &token::ID,
            ),
            spl_token::instruction::initialize_mint2(
                &token::ID,
                &valid_mint.pubkey(),
                &payer,
                None,
                MOCK_USDC_DECIMALS,
            )
            .unwrap(),
            system_instruction::create_account(
                &payer,
                &wrong_decimals_mint.pubkey(),
                rent.minimum_balance(spl_token::state::Mint::LEN),
                spl_token::state::Mint::LEN as u64,
                &token::ID,
            ),
            spl_token::instruction::initialize_mint2(
                &token::ID,
                &wrong_decimals_mint.pubkey(),
                &payer,
                None,
                9,
            )
            .unwrap(),
            system_instruction::create_account(
                &payer,
                &wrong_owner_mint.pubkey(),
                rent.minimum_balance(spl_token_2022::state::Mint::LEN),
                spl_token_2022::state::Mint::LEN as u64,
                &token_2022::ID,
            ),
            spl_token_2022::instruction::initialize_mint2(
                &token_2022::ID,
                &wrong_owner_mint.pubkey(),
                &payer,
                None,
                MOCK_USDC_DECIMALS,
            )
            .unwrap(),
        ],
        &[&valid_mint, &wrong_decimals_mint, &wrong_owner_mint],
    )
    .await
    .unwrap();

    let unauthorized_error = send(
        &mut context,
        vec![initialize_config_instruction(
            unauthorized.pubkey(),
            valid_mint.pubkey(),
        )],
        &[&unauthorized],
    )
    .await
    .unwrap_err();
    assert!(
        unauthorized_error.contains("Unauthorized"),
        "{unauthorized_error}"
    );
    let wrong_owner_error = send(
        &mut context,
        vec![initialize_config_instruction(
            admin.pubkey(),
            wrong_owner_mint.pubkey(),
        )],
        &[&admin],
    )
    .await
    .unwrap_err();
    assert!(
        wrong_owner_error.contains("AccountOwnedByWrongProgram"),
        "{wrong_owner_error}"
    );
    let decimals_error = send(
        &mut context,
        vec![initialize_config_instruction(
            admin.pubkey(),
            wrong_decimals_mint.pubkey(),
        )],
        &[&admin],
    )
    .await
    .unwrap_err();
    assert!(
        decimals_error.contains("InvalidMockUsdcMint"),
        "{decimals_error}"
    );
    let config = Pubkey::find_program_address(&[CONFIG_SEED], &VAULT_ID).0;
    assert!(context
        .banks_client
        .get_account(config)
        .await
        .unwrap()
        .is_none());

    let initialize = initialize_config_instruction(admin.pubkey(), valid_mint.pubkey());
    send(&mut context, vec![initialize.clone()], &[&admin])
        .await
        .unwrap();
    let state: Config = decode(&account(&mut context, config).await.data);
    assert_eq!(state.admin, admin.pubkey());
    assert_eq!(state.mock_usdc_mint, valid_mint.pubkey());
    assert!(send(&mut context, vec![initialize], &[&admin])
        .await
        .is_err());
}

// Run after `anchor build --skip-lint`: cargo test -p property-vault --test listing -- --ignored
#[tokio::test]
#[ignore = "requires both SBF artifacts from anchor build"]
async fn listing_initializes_fixed_supply_and_hook_metadata() {
    let sbf_dir = format!("{}/../../target/deploy", env!("CARGO_MANIFEST_DIR"));
    std::env::set_var("BPF_OUT_DIR", &sbf_dir);
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("property_vault", VAULT_ID, None);
    test.add_program("property_vault_transfer_hook", HOOK_ID, None);

    let mock_mint = Keypair::new();
    let config = Pubkey::find_program_address(&[CONFIG_SEED], &VAULT_ID).0;
    let mut config_data = vec![0; Config::SPACE];
    // ProgramTest's payer is generated at start; Config does not gate listing by admin.
    Config {
        admin: Pubkey::default(),
        mock_usdc_mint: mock_mint.pubkey(),
        bump: Pubkey::find_program_address(&[CONFIG_SEED], &VAULT_ID).1,
    }
    .try_serialize(&mut &mut config_data[..])
    .unwrap();
    test.add_account(
        config,
        Account {
            lamports: 10_000_000,
            data: config_data,
            owner: VAULT_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    let mut context = test.start_with_context().await;
    let payer = context.payer.pubkey();
    let mint_rent = context
        .banks_client
        .get_rent()
        .await
        .unwrap()
        .minimum_balance(spl_token::state::Mint::LEN);
    send(
        &mut context,
        vec![
            system_instruction::create_account(
                &payer,
                &mock_mint.pubkey(),
                mint_rent,
                spl_token::state::Mint::LEN as u64,
                &token::ID,
            ),
            spl_token::instruction::initialize_mint2(
                &token::ID,
                &mock_mint.pubkey(),
                &payer,
                None,
                MOCK_USDC_DECIMALS,
            )
            .unwrap(),
        ],
        &[&mock_mint],
    )
    .await
    .unwrap();

    let tenant = Pubkey::new_unique();
    let (list_ix, keys) = list_instruction(
        payer,
        config,
        mock_mint.pubkey(),
        17,
        tenant,
        100_000_000,
        100,
        3600,
    );
    // The hook sees serialized PropertyVault state in the following top-level
    // instruction, so both setup steps can be atomic.
    let hook_ix = hook_initialize_instruction(payer, &keys);
    let listing_units = send(&mut context, vec![list_ix.clone(), hook_ix], &[])
        .await
        .unwrap();
    println!(
        "atomic listing and hook setup compute units: {}",
        listing_units.compute_units
    );

    let property: Property = decode(&account(&mut context, keys.property).await.data);
    assert_eq!(property.owner, payer);
    assert_eq!(property.tenant, tenant);
    assert_eq!(property.mint, keys.mint);
    assert_eq!(property.inventory_vault, keys.inventory_vault);
    assert_eq!(property.max_supply, 100);
    assert_eq!(property.investor_allocation_bps, 8_000);
    assert_eq!(property.mock_usdc_mint, mock_mint.pubkey());

    let mint_account = account(&mut context, keys.mint).await;
    assert_eq!(mint_account.owner, token_2022::ID);
    let mint_state =
        StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_account.data).unwrap();
    assert_eq!(mint_state.base.decimals, 0);
    assert_eq!(mint_state.base.supply, 100);
    assert!(mint_state.base.mint_authority.is_none());
    assert_eq!(
        mint_state.base.freeze_authority,
        anchor_lang::solana_program::program_option::COption::Some(keys.freeze_authority)
    );
    let hook = mint_state.get_extension::<TransferHook>().unwrap();
    assert!(Option::<Pubkey>::from(hook.authority).is_none());
    assert_eq!(transfer_hook::get_program_id(&mint_state), Some(HOOK_ID));

    let inventory_account = account(&mut context, keys.inventory_vault).await;
    assert_eq!(inventory_account.owner, token_2022::ID);
    let inventory =
        StateWithExtensions::<spl_token_2022::state::Account>::unpack(&inventory_account.data)
            .unwrap();
    assert_eq!(inventory.base.amount, 100);
    assert_eq!(inventory.base.owner, keys.inventory_authority);
    assert_eq!(inventory.base.mint, keys.mint);
    assert!(inventory.get_extension::<ImmutableOwner>().is_ok());
    assert!(inventory.get_extension::<TransferHookAccount>().is_ok());
    assert_eq!(
        inventory.base.state,
        spl_token_2022::state::AccountState::Frozen
    );

    let investment_vault =
        spl_token::state::Account::unpack(&account(&mut context, keys.investment_vault).await.data)
            .unwrap();
    let rent_vault =
        spl_token::state::Account::unpack(&account(&mut context, keys.rent_vault).await.data)
            .unwrap();
    assert_ne!(keys.investment_vault, keys.rent_vault);
    assert_eq!(investment_vault.owner, keys.investment_pool);
    assert_eq!(rent_vault.owner, keys.rent_pool);
    assert_eq!(investment_vault.mint, mock_mint.pubkey());
    assert_eq!(rent_vault.mint, mock_mint.pubkey());
    let investment_pool: InvestmentPool =
        decode(&account(&mut context, keys.investment_pool).await.data);
    assert_eq!(investment_pool.vault, keys.investment_vault);
    assert_eq!(investment_pool.total_raised, 0);
    let rent_pool: RentPool = decode(&account(&mut context, keys.rent_pool).await.data);
    assert_eq!(rent_pool.vault, keys.rent_vault);
    assert_eq!(rent_pool.period_id, 0);
    assert_eq!(rent_pool.cutoff_ts - rent_pool.period_start_ts, 3600);
    let registry: HolderRegistry = decode(&account(&mut context, keys.registry).await.data);
    assert_eq!(registry.count, 0);
    assert_eq!(registry.holders, [Pubkey::default(); MAX_HOLDERS]);

    let metadata = account(&mut context, keys.meta_list).await;
    assert_eq!(metadata.owner, HOOK_ID);
    assert_eq!(
        metadata.data.len(),
        ExtraAccountMetaList::size_of(3).unwrap()
    );
    let tlv = TlvStateBorrowed::unpack(&metadata.data).unwrap();
    let metas = ExtraAccountMetaList::unpack_with_tlv_state::<ExecuteInstruction>(&tlv).unwrap();
    assert_eq!(metas.len(), 3);
    for (actual, expected) in metas
        .iter()
        .zip([keys.property, keys.rent_pool, keys.registry])
    {
        assert_eq!(
            *actual,
            ExtraAccountMeta::new_with_pubkey(&expected, false, false).unwrap()
        );
    }

    for (i, price, supply, duration, bad_tenant) in [
        (18, 0, 100, 3600, tenant),
        (19, 1, 0, 3600, tenant),
        (20, 1, 100, 0, tenant),
        (21, 1, 100, 3600, Pubkey::default()),
    ] {
        let (invalid_ix, invalid_keys) = list_instruction(
            payer,
            config,
            mock_mint.pubkey(),
            i,
            bad_tenant,
            price,
            supply,
            duration,
        );
        assert!(send(&mut context, vec![invalid_ix], &[]).await.is_err());
        assert!(context
            .banks_client
            .get_account(invalid_keys.property)
            .await
            .unwrap()
            .is_none());
    }
    for (id, allocation) in [(24, 0), (25, 10_000), (26, u16::MAX)] {
        let (mut invalid, keys) =
            list_instruction(payer, config, mock_mint.pubkey(), id, tenant, 1, 100, 3600);
        invalid.data = instruction::ListAndTokenizeProperty {
            property_id: id,
            tenant,
            token_price: 1,
            max_supply: 100,
            terms_hash: [7; 32],
            duration_seconds: 3600,
            investor_allocation_bps: allocation,
        }
        .data();
        let error = send(&mut context, vec![invalid], &[]).await.unwrap_err();
        assert!(error.contains("InvalidInvestorAllocation"), "{error}");
        assert!(context
            .banks_client
            .get_account(keys.property)
            .await
            .unwrap()
            .is_none());
    }
    assert!(send(&mut context, vec![list_ix], &[]).await.is_err());

    let (malformed_list, malformed_keys) = list_instruction(
        payer,
        config,
        mock_mint.pubkey(),
        22,
        tenant,
        100_000_000,
        100,
        3600,
    );
    let mut malformed_hook = hook_initialize_instruction(payer, &malformed_keys);
    malformed_hook.accounts[5].pubkey = keys.registry;
    assert!(
        send(&mut context, vec![malformed_list, malformed_hook], &[])
            .await
            .is_err()
    );
    assert!(context
        .banks_client
        .get_account(malformed_keys.property)
        .await
        .unwrap()
        .is_none());

    // A standalone listing succeeds, but it cannot be invested in until its
    // separate hook metadata instruction completes.
    let (partial_list, partial_keys) = list_instruction(
        payer,
        config,
        mock_mint.pubkey(),
        23,
        tenant,
        100_000_000,
        100,
        3600,
    );
    send(&mut context, vec![partial_list], &[]).await.unwrap();
    assert!(context
        .banks_client
        .get_account(partial_keys.property)
        .await
        .unwrap()
        .is_some());
    assert!(context
        .banks_client
        .get_account(partial_keys.meta_list)
        .await
        .unwrap()
        .is_none());

    let investor = Keypair::new();
    fund_investor(&mut context, &investor, mock_mint.pubkey(), 1_000_000_000).await;
    let investor_key = investor.pubkey();
    let investor_usdc_ata = get_associated_token_address_with_program_id(
        &investor_key,
        &mock_mint.pubkey(),
        &token::ID,
    );
    assert_eq!(
        spl_token::state::Account::unpack(&account(&mut context, investor_usdc_ata).await.data)
            .unwrap()
            .amount,
        1_000_000_000
    );
    let investor_property_ata =
        get_associated_token_address_with_program_id(&investor_key, &keys.mint, &token_2022::ID);
    let register_units = send(
        &mut context,
        vec![register_instruction(investor_key, &keys, None, None)],
        &[&investor],
    )
    .await
    .unwrap();
    println!(
        "holder registration compute units: {}",
        register_units.compute_units
    );
    assert_eq!(
        property_balance(&mut context, investor_property_ata).await,
        (0, true)
    );
    assert!(send(
        &mut context,
        vec![register_instruction(investor_key, &keys, None, None)],
        &[&investor],
    )
    .await
    .is_err());

    send(
        &mut context,
        vec![register_instruction(
            investor_key,
            &partial_keys,
            None,
            None,
        )],
        &[&investor],
    )
    .await
    .unwrap();
    let investor_usdc_before = usdc_balance(&mut context, investor_usdc_ata).await;
    let partial_error = send(
        &mut context,
        vec![invest_instruction(
            investor_key,
            &partial_keys,
            mock_mint.pubkey(),
            100_000_000,
            [7; 32],
        )],
        &[&investor],
    )
    .await
    .unwrap_err();
    assert!(
        partial_error.contains("InvalidHookMetadata"),
        "{partial_error}"
    );
    assert_eq!(
        usdc_balance(&mut context, investor_usdc_ata).await,
        investor_usdc_before
    );
    assert_eq!(
        usdc_balance(&mut context, partial_keys.investment_vault).await,
        0
    );

    // Force compute exhaustion after the legacy USDC payment CPI has run.
    // Solana must roll that payment and the new position back when the later
    // allocation path runs out of compute.
    let usdc_before = usdc_balance(&mut context, investor_usdc_ata).await;
    let investment_vault_before = usdc_balance(&mut context, keys.investment_vault).await;
    let inventory_before = property_balance(&mut context, keys.inventory_vault).await;
    let late_failure = send(
        &mut context,
        vec![
            ComputeBudgetInstruction::set_compute_unit_limit(100_000),
            invest_instruction(
                investor_key,
                &keys,
                mock_mint.pubkey(),
                100_000_000,
                [7; 32],
            ),
        ],
        &[&investor],
    )
    .await
    .unwrap_err();
    assert!(
        late_failure.contains(&format!("Program {} success", token::ID)),
        "payment CPI did not complete before failure: {late_failure}"
    );
    assert_eq!(
        usdc_balance(&mut context, investor_usdc_ata).await,
        usdc_before
    );
    assert_eq!(
        usdc_balance(&mut context, keys.investment_vault).await,
        investment_vault_before
    );
    assert_eq!(
        property_balance(&mut context, keys.inventory_vault).await,
        inventory_before
    );
    assert_eq!(
        property_balance(&mut context, investor_property_ata).await,
        (0, true)
    );

    let first = send(
        &mut context,
        with_transfer_budget(invest_instruction(
            investor_key,
            &keys,
            mock_mint.pubkey(),
            500_000_000,
            [7; 32],
        )),
        &[&investor],
    )
    .await
    .unwrap();
    println!("first investment compute units: {}", first.compute_units);
    assert!(
        first
            .logs
            .iter()
            .any(|line| line.contains(&format!("Program {HOOK_ID} invoke"))),
        "Token-2022 did not invoke the separate Transfer Hook: {:?}",
        first.logs
    );
    assert_eq!(
        property_balance(&mut context, keys.inventory_vault).await,
        (95, true)
    );
    assert_eq!(
        property_balance(&mut context, investor_property_ata).await,
        (5, true)
    );
    let usdc_vault =
        spl_token::state::Account::unpack(&account(&mut context, keys.investment_vault).await.data)
            .unwrap();
    assert_eq!(usdc_vault.amount, 500_000_000);
    assert_eq!(
        spl_token::state::Account::unpack(&account(&mut context, investor_usdc_ata).await.data)
            .unwrap()
            .amount,
        500_000_000
    );
    let position_key = Pubkey::find_program_address(
        &[POSITION_SEED, keys.property.as_ref(), investor_key.as_ref()],
        &VAULT_ID,
    )
    .0;
    let position: InvestorPosition = decode(&account(&mut context, position_key).await.data);
    assert_eq!(position.invested_amount, 500_000_000);
    assert_eq!(position.token_amount, 5);
    assert_eq!(position.accepted_terms_hash, [7; 32]);
    let pool: InvestmentPool = decode(&account(&mut context, keys.investment_pool).await.data);
    assert_eq!(pool.total_raised, 500_000_000);
    let mint_state = account(&mut context, keys.mint).await;
    let mint_state =
        StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_state.data).unwrap();
    assert_eq!(mint_state.base.supply, 100);

    for (amount, hash, expected_error) in [
        (100_000_000, [8; 32], "StaleTerms"),
        (550_000_000, [7; 32], "InvestmentAmountNotDivisible"),
        (0, [7; 32], "InvalidInvestmentAmount"),
        (600_000_000, [7; 32], "InvalidInvestmentAmount"),
        (10_100_000_000, [7; 32], "InsufficientPropertyTokenSupply"),
    ] {
        let error = send(
            &mut context,
            vec![invest_instruction(
                investor_key,
                &keys,
                mock_mint.pubkey(),
                amount,
                hash,
            )],
            &[&investor],
        )
        .await
        .unwrap_err();
        assert!(error.contains(expected_error), "{error}");
    }
    let mut wrong_vault = invest_instruction(
        investor_key,
        &keys,
        mock_mint.pubkey(),
        100_000_000,
        [7; 32],
    );
    wrong_vault.accounts[13].pubkey = keys.rent_vault;
    assert!(send(&mut context, vec![wrong_vault], &[&investor])
        .await
        .is_err());
    let mut wrong_mint = invest_instruction(
        investor_key,
        &keys,
        mock_mint.pubkey(),
        100_000_000,
        [7; 32],
    );
    wrong_mint.accounts[11].pubkey = keys.mint;
    assert!(send(&mut context, vec![wrong_mint], &[&investor])
        .await
        .is_err());
    let mut wrong_inventory = invest_instruction(
        investor_key,
        &keys,
        mock_mint.pubkey(),
        100_000_000,
        [7; 32],
    );
    wrong_inventory.accounts[4].pubkey = investor_property_ata;
    assert!(send(&mut context, vec![wrong_inventory], &[&investor])
        .await
        .is_err());
    assert_eq!(
        property_balance(&mut context, investor_property_ata).await,
        (5, true)
    );
    assert_eq!(
        spl_token::state::Account::unpack(
            &account(&mut context, keys.investment_vault).await.data,
        ).unwrap().amount,
        500_000_000
    );

    let repeated = send(
        &mut context,
        with_transfer_budget(invest_instruction(
            investor_key,
            &keys,
            mock_mint.pubkey(),
            300_000_000,
            [7; 32],
        )),
        &[&investor],
    )
    .await
    .unwrap();
    println!(
        "repeated investment compute units: {}",
        repeated.compute_units
    );
    assert_eq!(
        property_balance(&mut context, keys.inventory_vault).await,
        (92, true)
    );
    assert_eq!(
        property_balance(&mut context, investor_property_ata).await,
        (8, true)
    );
    let position: InvestorPosition = decode(&account(&mut context, position_key).await.data);
    assert_eq!(position.invested_amount, 800_000_000);
    assert_eq!(position.token_amount, 8);
    let pool: InvestmentPool = decode(&account(&mut context, keys.investment_pool).await.data);
    assert_eq!(pool.total_raised, 800_000_000);

    let recipient = Keypair::new();
    send(
        &mut context,
        vec![system_instruction::transfer(
            &payer,
            &recipient.pubkey(),
            2_000_000_000,
        )],
        &[],
    )
    .await
    .unwrap();
    send(
        &mut context,
        vec![register_instruction(recipient.pubkey(), &keys, None, None)],
        &[&recipient],
    )
    .await
    .unwrap();
    let recipient_ata = get_associated_token_address_with_program_id(
        &recipient.pubkey(),
        &keys.mint,
        &token_2022::ID,
    );
    let direct_transfer = spl_token_2022::instruction::transfer_checked(
        &token_2022::ID,
        &investor_property_ata,
        &keys.mint,
        &recipient_ata,
        &investor_key,
        &[],
        1,
        0,
    )
    .unwrap();
    assert!(send(&mut context, vec![direct_transfer], &[&investor])
        .await
        .is_err());
    assert_eq!(
        property_balance(&mut context, investor_property_ata).await,
        (8, true)
    );
    let secondary = send(
        &mut context,
        with_transfer_budget(secondary_instruction(
            investor_key,
            recipient.pubkey(),
            &keys,
            1,
        )),
        &[&investor],
    )
    .await
    .unwrap();
    println!(
        "secondary transfer compute units: {}",
        secondary.compute_units
    );
    assert!(secondary
        .logs
        .iter()
        .any(|line| line.contains(&format!("Program {HOOK_ID} invoke"))));
    assert_eq!(
        property_balance(&mut context, investor_property_ata).await,
        (7, true)
    );
    assert_eq!(
        property_balance(&mut context, recipient_ata).await,
        (1, true)
    );
    let position: InvestorPosition = decode(&account(&mut context, position_key).await.data);
    assert_eq!(position.token_amount, 8); // cumulative investment record is unchanged
    let mut unauthorized_transfer =
        secondary_instruction(investor_key, recipient.pubkey(), &keys, 1);
    unauthorized_transfer.accounts[0].pubkey = recipient.pubkey();
    let unauthorized_error = send(&mut context, vec![unauthorized_transfer], &[&recipient])
        .await
        .unwrap_err();
    assert!(
        unauthorized_error.contains("Unauthorized"),
        "{unauthorized_error}"
    );
    assert_eq!(
        property_balance(&mut context, investor_property_ata).await,
        (7, true)
    );
    let mut wrong_hook_account = secondary_instruction(investor_key, recipient.pubkey(), &keys, 1);
    wrong_hook_account.accounts[9].pubkey = keys.investment_pool;
    assert!(send(&mut context, vec![wrong_hook_account], &[&investor])
        .await
        .is_err());
    assert_eq!(
        property_balance(&mut context, investor_property_ata).await,
        (7, true)
    );

    // Test-only bank mutation makes the otherwise frozen accounts transferable.
    // A malformed hook account must then reject a real Token-2022 transfer.
    // The preceding USDC transfer in the same transaction must roll back.
    let saved_source = account(&mut context, investor_property_ata).await;
    let saved_destination = account(&mut context, recipient_ata).await;
    let saved_registry = account(&mut context, keys.registry).await;
    temporarily_thaw_account(&mut context, investor_property_ata, saved_source.clone());
    temporarily_thaw_account(&mut context, recipient_ata, saved_destination.clone());
    let mut malformed_registry = saved_registry.clone();
    let mut malformed_registry_state: HolderRegistry = decode(&malformed_registry.data);
    malformed_registry_state.count = 11;
    malformed_registry_state
        .try_serialize(&mut &mut malformed_registry.data[..])
        .unwrap();
    context.set_account(&keys.registry, &AccountSharedData::from(malformed_registry));
    let mut malformed_hook_transfer = spl_token_2022::instruction::transfer_checked(
        &token_2022::ID,
        &investor_property_ata,
        &keys.mint,
        &recipient_ata,
        &investor_key,
        &[],
        1,
        0,
    )
    .unwrap();
    malformed_hook_transfer.accounts.extend([
        AccountMeta::new_readonly(keys.property, false),
        AccountMeta::new_readonly(keys.rent_pool, false),
        AccountMeta::new_readonly(keys.registry, false), // malformed registry data
        AccountMeta::new_readonly(keys.meta_list, false),
        AccountMeta::new_readonly(HOOK_ID, false),
    ]);
    let usdc_before =
        spl_token::state::Account::unpack(&account(&mut context, keys.investment_vault).await.data)
            .unwrap()
            .amount;
    let malformed_error = send(
        &mut context,
        vec![
            spl_token::instruction::transfer(
                &token::ID,
                &investor_usdc_ata,
                &keys.investment_vault,
                &investor_key,
                &[],
                100_000_000,
            )
            .unwrap(),
            malformed_hook_transfer,
        ],
        &[&investor],
    )
    .await
    .unwrap_err();
    assert!(
        malformed_error.contains(&format!("Program {HOOK_ID} invoke")),
        "the malformed transfer did not reach the hook: {malformed_error}"
    );
    assert!(
        malformed_error.contains("MalformedHolderRegistry"),
        "the hook did not reject the malformed registry: {malformed_error}"
    );
    assert_eq!(
        spl_token::state::Account::unpack(
            &account(&mut context, keys.investment_vault).await.data,
        ).unwrap().amount,
        usdc_before
    );
    context.set_account(
        &investor_property_ata,
        &AccountSharedData::from(saved_source),
    );
    context.set_account(&recipient_ata, &AccountSharedData::from(saved_destination));
    context.set_account(&keys.registry, &AccountSharedData::from(saved_registry));
    assert_eq!(
        property_balance(&mut context, investor_property_ata).await,
        (7, true)
    );
    assert_eq!(
        property_balance(&mut context, recipient_ata).await,
        (1, true)
    );

    let mut empty_holders = Vec::new();
    for _ in 0..8 {
        let wallet = Keypair::new();
        send(
            &mut context,
            vec![system_instruction::transfer(
                &payer,
                &wallet.pubkey(),
                1_000_000_000,
            )],
            &[],
        )
        .await
        .unwrap();
        send(
            &mut context,
            vec![register_instruction(wallet.pubkey(), &keys, None, None)],
            &[&wallet],
        )
        .await
        .unwrap();
        empty_holders.push(wallet);
    }
    let registry: HolderRegistry = decode(&account(&mut context, keys.registry).await.data);
    assert_eq!(registry.count, 10);
    let overflow_wallet = Keypair::new();
    send(
        &mut context,
        vec![system_instruction::transfer(
            &payer,
            &overflow_wallet.pubkey(),
            1_000_000_000,
        )],
        &[],
    )
    .await
    .unwrap();
    let capacity_error = send(
        &mut context,
        vec![register_instruction(
            overflow_wallet.pubkey(),
            &keys,
            None,
            None,
        )],
        &[&overflow_wallet],
    )
    .await
    .unwrap_err();
    assert!(
        capacity_error.contains("HolderRegistryFull"),
        "{capacity_error}"
    );
    let overflow_ata = get_associated_token_address_with_program_id(
        &overflow_wallet.pubkey(),
        &keys.mint,
        &token_2022::ID,
    );
    send(
        &mut context,
        vec![
            spl_associated_token_account::instruction::create_associated_token_account(
                &payer,
                &overflow_wallet.pubkey(),
                &keys.mint,
                &token_2022::ID,
            ),
        ],
        &[],
    )
    .await
    .unwrap();
    let saved_source = account(&mut context, investor_property_ata).await;
    temporarily_thaw_account(&mut context, investor_property_ata, saved_source.clone());
    let mut unregistered_hook_transfer = spl_token_2022::instruction::transfer_checked(
        &token_2022::ID,
        &investor_property_ata,
        &keys.mint,
        &overflow_ata,
        &investor_key,
        &[],
        1,
        0,
    )
    .unwrap();
    unregistered_hook_transfer.accounts.extend([
        AccountMeta::new_readonly(keys.property, false),
        AccountMeta::new_readonly(keys.rent_pool, false),
        AccountMeta::new_readonly(keys.registry, false),
        AccountMeta::new_readonly(keys.meta_list, false),
        AccountMeta::new_readonly(HOOK_ID, false),
    ]);
    let hook_rejection = send(
        &mut context,
        with_transfer_budget(unregistered_hook_transfer),
        &[&investor],
    )
    .await
    .unwrap_err();
    assert!(
        hook_rejection.contains(&format!("Program {HOOK_ID} invoke")),
        "{hook_rejection}"
    );
    assert!(
        hook_rejection.contains("UnregisteredDestination"),
        "{hook_rejection}"
    );
    assert_eq!(property_balance(&mut context, overflow_ata).await.0, 0);
    let delegate = Keypair::new();
    send(
        &mut context,
        vec![spl_token_2022::instruction::approve_checked(
            &token_2022::ID,
            &investor_property_ata,
            &keys.mint,
            &delegate.pubkey(),
            &investor_key,
            &[],
            1,
            0,
        )
        .unwrap()],
        &[&investor],
    )
    .await
    .unwrap();
    let saved_destination = account(&mut context, recipient_ata).await;
    temporarily_thaw_account(&mut context, recipient_ata, saved_destination.clone());
    let mut delegated_transfer = spl_token_2022::instruction::transfer_checked(
        &token_2022::ID,
        &investor_property_ata,
        &keys.mint,
        &recipient_ata,
        &delegate.pubkey(),
        &[],
        1,
        0,
    )
    .unwrap();
    delegated_transfer.accounts.extend([
        AccountMeta::new_readonly(keys.property, false),
        AccountMeta::new_readonly(keys.rent_pool, false),
        AccountMeta::new_readonly(keys.registry, false),
        AccountMeta::new_readonly(keys.meta_list, false),
        AccountMeta::new_readonly(HOOK_ID, false),
    ]);
    let authority_rejection = send(
        &mut context,
        with_transfer_budget(delegated_transfer),
        &[&delegate],
    )
    .await
    .unwrap_err();
    assert!(
        authority_rejection.contains(&format!("Program {HOOK_ID} invoke")),
        "{authority_rejection}"
    );
    assert!(
        authority_rejection.contains("InvalidTransferAuthority"),
        "{authority_rejection}"
    );
    context.set_account(
        &investor_property_ata,
        &AccountSharedData::from(saved_source),
    );
    context.set_account(&recipient_ata, &AccountSharedData::from(saved_destination));

    let occupied_error = send(
        &mut context,
        vec![register_instruction(
            overflow_wallet.pubkey(),
            &keys,
            Some(0),
            Some(investor_property_ata),
        )],
        &[&overflow_wallet],
    )
    .await
    .unwrap_err();
    assert!(
        occupied_error.contains("HolderStillOwnsTokens"),
        "{occupied_error}"
    );
    let replaced_wallet = &empty_holders[0];
    let replaced_ata = get_associated_token_address_with_program_id(
        &replaced_wallet.pubkey(),
        &keys.mint,
        &token_2022::ID,
    );
    let replacement = Keypair::new();
    send(
        &mut context,
        vec![system_instruction::transfer(
            &payer,
            &replacement.pubkey(),
            1_000_000_000,
        )],
        &[],
    )
    .await
    .unwrap();
    send(
        &mut context,
        vec![register_instruction(
            replacement.pubkey(),
            &keys,
            Some(2),
            Some(replaced_ata),
        )],
        &[&replacement],
    )
    .await
    .unwrap();
    let registry: HolderRegistry = decode(&account(&mut context, keys.registry).await.data);
    assert_eq!(registry.count, 10);
    assert_eq!(registry.holders[2], replacement.pubkey());
    fund_investor(
        &mut context,
        replaced_wallet,
        mock_mint.pubkey(),
        100_000_000,
    )
    .await;
    let unregistered_error = send(
        &mut context,
        vec![invest_instruction(
            replaced_wallet.pubkey(),
            &keys,
            mock_mint.pubkey(),
            100_000_000,
            [7; 32],
        )],
        &[replaced_wallet],
    )
    .await
    .unwrap_err();
    assert!(
        unregistered_error.contains("HolderNotRegistered"),
        "{unregistered_error}"
    );

    let unregistered_transfer_error = send(
        &mut context,
        vec![secondary_instruction(
            investor_key,
            replaced_wallet.pubkey(),
            &keys,
            1,
        )],
        &[&investor],
    )
    .await
    .unwrap_err();
    assert!(
        unregistered_transfer_error.contains("HolderNotRegistered"),
        "{unregistered_transfer_error}"
    );

    let mut clock: anchor_lang::prelude::Clock = context.banks_client.get_sysvar().await.unwrap();
    let rent_pool: RentPool = decode(&account(&mut context, keys.rent_pool).await.data);
    clock.unix_timestamp = rent_pool.cutoff_ts;
    context.set_sysvar(&clock);
    let cutoff_error = send(
        &mut context,
        vec![secondary_instruction(
            investor_key,
            recipient.pubkey(),
            &keys,
            1,
        )],
        &[&investor],
    )
    .await
    .unwrap_err();
    assert!(
        cutoff_error.contains("RentCollectionClosed"),
        "{cutoff_error}"
    );
    assert_eq!(
        property_balance(&mut context, investor_property_ata).await,
        (7, true)
    );
    assert_eq!(
        property_balance(&mut context, recipient_ata).await,
        (1, true)
    );
}

#[derive(Debug, PartialEq, Eq)]
struct Phase5Snapshot {
    investment_vault: u64,
    rent_vault: u64,
    owner_usdc: u64,
    tenant_usdc: u64,
    total_raised: u64,
    total_withdrawn: u64,
    current_deposits: u64,
}

async fn phase5_snapshot(
    context: &mut ProgramTestContext,
    keys: &ListingKeys,
    owner_usdc: Pubkey,
    tenant_usdc: Pubkey,
) -> Phase5Snapshot {
    let investment_pool: InvestmentPool =
        decode(&account(context, keys.investment_pool).await.data);
    let rent_pool: RentPool = decode(&account(context, keys.rent_pool).await.data);
    Phase5Snapshot {
        investment_vault: usdc_balance(context, keys.investment_vault).await,
        rent_vault: usdc_balance(context, keys.rent_vault).await,
        owner_usdc: usdc_balance(context, owner_usdc).await,
        tenant_usdc: usdc_balance(context, tenant_usdc).await,
        total_raised: investment_pool.total_raised,
        total_withdrawn: investment_pool.total_withdrawn,
        current_deposits: rent_pool.current_deposits,
    }
}

async fn assert_phase5_rejected_unchanged(
    context: &mut ProgramTestContext,
    instructions: Vec<Instruction>,
    signers: &[&Keypair],
    expected_error: &str,
    keys: &ListingKeys,
    owner_usdc: Pubkey,
    tenant_usdc: Pubkey,
) {
    let before = phase5_snapshot(context, keys, owner_usdc, tenant_usdc).await;
    let error = send(context, instructions, signers).await.unwrap_err();
    assert!(error.contains(expected_error), "{error}");
    assert_eq!(
        phase5_snapshot(context, keys, owner_usdc, tenant_usdc).await,
        before
    );
}

async fn assert_claim_rejected_unchanged(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    signers: &[&Keypair],
    expected_error: &str,
    keys: &ListingKeys,
    period: Pubkey,
) {
    let vault_before = usdc_balance(context, keys.rent_vault).await;
    let pool_before: RentPool = decode(&account(context, keys.rent_pool).await.data);
    let period_before: RentPeriod = decode(&account(context, period).await.data);
    let error = send(context, vec![instruction], signers).await.unwrap_err();
    assert!(error.contains(expected_error), "{error}");
    assert_eq!(usdc_balance(context, keys.rent_vault).await, vault_before);
    let pool_after: RentPool = decode(&account(context, keys.rent_pool).await.data);
    let period_after: RentPeriod = decode(&account(context, period).await.data);
    assert_eq!(
        pool_after.reserved_unclaimed,
        pool_before.reserved_unclaimed
    );
    assert_eq!(period_after.claimed_amount, period_before.claimed_amount);
}

#[tokio::test]
#[ignore = "requires both SBF artifacts from anchor build"]
async fn phase5_withdrawals_and_rent_payments_use_separate_sbf_vaults() {
    let sbf_dir = format!("{}/../../target/deploy", env!("CARGO_MANIFEST_DIR"));
    std::env::set_var("BPF_OUT_DIR", &sbf_dir);
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("property_vault", VAULT_ID, None);
    test.add_program("property_vault_transfer_hook", HOOK_ID, None);

    let mock_mint = Keypair::new();
    let config = Pubkey::find_program_address(&[CONFIG_SEED], &VAULT_ID).0;
    let mut config_data = vec![0; Config::SPACE];
    Config {
        admin: Pubkey::default(),
        mock_usdc_mint: mock_mint.pubkey(),
        bump: Pubkey::find_program_address(&[CONFIG_SEED], &VAULT_ID).1,
    }
    .try_serialize(&mut &mut config_data[..])
    .unwrap();
    test.add_account(
        config,
        Account {
            lamports: 10_000_000,
            data: config_data,
            owner: VAULT_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    let mut context = test.start_with_context().await;
    let owner = context.payer.pubkey();
    let mint_rent = context
        .banks_client
        .get_rent()
        .await
        .unwrap()
        .minimum_balance(spl_token::state::Mint::LEN);
    send(
        &mut context,
        vec![
            system_instruction::create_account(
                &owner,
                &mock_mint.pubkey(),
                mint_rent,
                spl_token::state::Mint::LEN as u64,
                &token::ID,
            ),
            spl_token::instruction::initialize_mint2(
                &token::ID,
                &mock_mint.pubkey(),
                &owner,
                None,
                MOCK_USDC_DECIMALS,
            )
            .unwrap(),
        ],
        &[&mock_mint],
    )
    .await
    .unwrap();

    let tenant = Keypair::new();
    let (list_ix, keys) = list_instruction(
        owner,
        config,
        mock_mint.pubkey(),
        51,
        tenant.pubkey(),
        100_000_000,
        100,
        3600,
    );
    let hook_ix = Instruction {
        program_id: HOOK_ID,
        accounts: vec![
            AccountMeta::new(owner, true),
            AccountMeta::new(keys.meta_list, false),
            AccountMeta::new_readonly(keys.mint, false),
            AccountMeta::new_readonly(keys.property, false),
            AccountMeta::new_readonly(keys.rent_pool, false),
            AccountMeta::new_readonly(keys.registry, false),
            AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
        ],
        data: vec![92, 197, 174, 197, 41, 124, 19, 3],
    };
    send(&mut context, vec![list_ix, hook_ix], &[])
        .await
        .unwrap();

    let owner_usdc =
        get_associated_token_address_with_program_id(&owner, &mock_mint.pubkey(), &token::ID);
    send(
        &mut context,
        vec![
            spl_associated_token_account::instruction::create_associated_token_account(
                &owner,
                &owner,
                &mock_mint.pubkey(),
                &token::ID,
            ),
        ],
        &[],
    )
    .await
    .unwrap();
    let investor = Keypair::new();
    fund_investor(&mut context, &investor, mock_mint.pubkey(), 1_000_000_000).await;
    send(
        &mut context,
        vec![register_instruction(investor.pubkey(), &keys, None, None)],
        &[&investor],
    )
    .await
    .unwrap();
    let investment = send(
        &mut context,
        with_transfer_budget(invest_instruction(
            investor.pubkey(),
            &keys,
            mock_mint.pubkey(),
            500_000_000,
            [7; 32],
        )),
        &[&investor],
    )
    .await
    .unwrap();
    assert!(investment
        .logs
        .iter()
        .any(|line| line.contains(&format!("Program {HOOK_ID} invoke"))));

    fund_investor(&mut context, &tenant, mock_mint.pubkey(), 400_000_000).await;
    let tenant_usdc = get_associated_token_address_with_program_id(
        &tenant.pubkey(),
        &mock_mint.pubkey(),
        &token::ID,
    );
    assert_eq!(
        phase5_snapshot(&mut context, &keys, owner_usdc, tenant_usdc).await,
        Phase5Snapshot {
            investment_vault: 500_000_000,
            rent_vault: 0,
            owner_usdc: 0,
            tenant_usdc: 400_000_000,
            total_raised: 500_000_000,
            total_withdrawn: 0,
            current_deposits: 0,
        }
    );

    send(
        &mut context,
        vec![withdraw_instruction(
            owner,
            &keys,
            mock_mint.pubkey(),
            200_000_000,
        )],
        &[],
    )
    .await
    .unwrap();
    assert_eq!(
        phase5_snapshot(&mut context, &keys, owner_usdc, tenant_usdc).await,
        Phase5Snapshot {
            investment_vault: 300_000_000,
            rent_vault: 0,
            owner_usdc: 200_000_000,
            tenant_usdc: 400_000_000,
            total_raised: 500_000_000,
            total_withdrawn: 200_000_000,
            current_deposits: 0,
        }
    );
    let investor_property_ata = get_associated_token_address_with_program_id(
        &investor.pubkey(),
        &keys.mint,
        &token_2022::ID,
    );
    assert_eq!(
        property_balance(&mut context, investor_property_ata).await,
        (5, true)
    );
    assert_eq!(
        property_balance(&mut context, keys.inventory_vault).await,
        (95, true)
    );
    let position = Pubkey::find_program_address(
        &[
            POSITION_SEED,
            keys.property.as_ref(),
            investor.pubkey().as_ref(),
        ],
        &VAULT_ID,
    )
    .0;
    let position_state: InvestorPosition = decode(&account(&mut context, position).await.data);
    assert_eq!(position_state.invested_amount, 500_000_000);
    assert_eq!(position_state.token_amount, 5);
    let mint_state = account(&mut context, keys.mint).await;
    assert_eq!(
        StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_state.data)
            .unwrap()
            .base
            .supply,
        100
    );

    let attacker = Keypair::new();
    fund_investor(&mut context, &attacker, mock_mint.pubkey(), 1).await;
    for (instruction, expected) in [
        (
            withdraw_instruction(owner, &keys, mock_mint.pubkey(), 0),
            "InvalidWithdrawalAmount",
        ),
        (
            withdraw_instruction(owner, &keys, mock_mint.pubkey(), 301_000_000),
            "WithdrawalExceedsAvailableCapital",
        ),
    ] {
        assert_phase5_rejected_unchanged(
            &mut context,
            vec![instruction],
            &[],
            expected,
            &keys,
            owner_usdc,
            tenant_usdc,
        )
        .await;
    }
    assert_phase5_rejected_unchanged(
        &mut context,
        vec![withdraw_instruction(
            attacker.pubkey(),
            &keys,
            mock_mint.pubkey(),
            1,
        )],
        &[&attacker],
        "Unauthorized",
        &keys,
        owner_usdc,
        tenant_usdc,
    )
    .await;
    let mut wrong_vault = withdraw_instruction(owner, &keys, mock_mint.pubkey(), 1);
    wrong_vault.accounts[3].pubkey = keys.rent_vault;
    assert_phase5_rejected_unchanged(
        &mut context,
        vec![wrong_vault],
        &[],
        "InvalidProtocolAccount",
        &keys,
        owner_usdc,
        tenant_usdc,
    )
    .await;
    let mut wrong_destination = withdraw_instruction(owner, &keys, mock_mint.pubkey(), 1);
    wrong_destination.accounts[4].pubkey = tenant_usdc;
    assert_phase5_rejected_unchanged(
        &mut context,
        vec![wrong_destination],
        &[],
        "Unauthorized",
        &keys,
        owner_usdc,
        tenant_usdc,
    )
    .await;
    let mut wrong_mint = withdraw_instruction(owner, &keys, mock_mint.pubkey(), 1);
    wrong_mint.accounts[5].pubkey = keys.mint;
    assert_phase5_rejected_unchanged(
        &mut context,
        vec![wrong_mint],
        &[],
        "AccountOwnedByWrongProgram",
        &keys,
        owner_usdc,
        tenant_usdc,
    )
    .await;
    assert_phase5_rejected_unchanged(
        &mut context,
        vec![
            withdraw_instruction(owner, &keys, mock_mint.pubkey(), 1),
            withdraw_instruction(owner, &keys, mock_mint.pubkey(), 301_000_000),
        ],
        &[],
        "WithdrawalExceedsAvailableCapital",
        &keys,
        owner_usdc,
        tenant_usdc,
    )
    .await;

    send(
        &mut context,
        vec![withdraw_instruction(
            owner,
            &keys,
            mock_mint.pubkey(),
            300_000_000,
        )],
        &[],
    )
    .await
    .unwrap();
    assert_eq!(
        phase5_snapshot(&mut context, &keys, owner_usdc, tenant_usdc).await,
        Phase5Snapshot {
            investment_vault: 0,
            rent_vault: 0,
            owner_usdc: 500_000_000,
            tenant_usdc: 400_000_000,
            total_raised: 500_000_000,
            total_withdrawn: 500_000_000,
            current_deposits: 0,
        }
    );
    assert_phase5_rejected_unchanged(
        &mut context,
        vec![withdraw_instruction(owner, &keys, mock_mint.pubkey(), 1)],
        &[],
        "WithdrawalExceedsAvailableCapital",
        &keys,
        owner_usdc,
        tenant_usdc,
    )
    .await;

    send(
        &mut context,
        vec![pay_rent_instruction(
            tenant.pubkey(),
            &keys,
            mock_mint.pubkey(),
            120_000_000,
        )],
        &[&tenant],
    )
    .await
    .unwrap();
    assert_eq!(
        phase5_snapshot(&mut context, &keys, owner_usdc, tenant_usdc).await,
        Phase5Snapshot {
            investment_vault: 0,
            rent_vault: 120_000_000,
            owner_usdc: 500_000_000,
            tenant_usdc: 280_000_000,
            total_raised: 500_000_000,
            total_withdrawn: 500_000_000,
            current_deposits: 120_000_000,
        }
    );
    send(
        &mut context,
        vec![pay_rent_instruction(
            tenant.pubkey(),
            &keys,
            mock_mint.pubkey(),
            80_000_000,
        )],
        &[&tenant],
    )
    .await
    .unwrap();
    assert_eq!(
        phase5_snapshot(&mut context, &keys, owner_usdc, tenant_usdc).await,
        Phase5Snapshot {
            investment_vault: 0,
            rent_vault: 200_000_000,
            owner_usdc: 500_000_000,
            tenant_usdc: 200_000_000,
            total_raised: 500_000_000,
            total_withdrawn: 500_000_000,
            current_deposits: 200_000_000,
        }
    );

    assert_phase5_rejected_unchanged(
        &mut context,
        vec![pay_rent_instruction(
            attacker.pubkey(),
            &keys,
            mock_mint.pubkey(),
            1,
        )],
        &[&attacker],
        "Unauthorized",
        &keys,
        owner_usdc,
        tenant_usdc,
    )
    .await;
    let mut wrong_rent_vault = pay_rent_instruction(tenant.pubkey(), &keys, mock_mint.pubkey(), 1);
    wrong_rent_vault.accounts[3].pubkey = keys.investment_vault;
    assert_phase5_rejected_unchanged(
        &mut context,
        vec![wrong_rent_vault],
        &[&tenant],
        "InvalidProtocolAccount",
        &keys,
        owner_usdc,
        tenant_usdc,
    )
    .await;
    let mut wrong_source = pay_rent_instruction(tenant.pubkey(), &keys, mock_mint.pubkey(), 1);
    wrong_source.accounts[4].pubkey = owner_usdc;
    assert_phase5_rejected_unchanged(
        &mut context,
        vec![wrong_source],
        &[&tenant],
        "Unauthorized",
        &keys,
        owner_usdc,
        tenant_usdc,
    )
    .await;
    let mut wrong_rent_mint = pay_rent_instruction(tenant.pubkey(), &keys, mock_mint.pubkey(), 1);
    wrong_rent_mint.accounts[5].pubkey = keys.mint;
    assert_phase5_rejected_unchanged(
        &mut context,
        vec![wrong_rent_mint],
        &[&tenant],
        "AccountOwnedByWrongProgram",
        &keys,
        owner_usdc,
        tenant_usdc,
    )
    .await;
    for (amount, expected) in [
        (0, "InvalidRentPaymentAmount"),
        (201_000_000, "InsufficientTenantUsdcBalance"),
    ] {
        assert_phase5_rejected_unchanged(
            &mut context,
            vec![pay_rent_instruction(
                tenant.pubkey(),
                &keys,
                mock_mint.pubkey(),
                amount,
            )],
            &[&tenant],
            expected,
            &keys,
            owner_usdc,
            tenant_usdc,
        )
        .await;
    }
    assert_phase5_rejected_unchanged(
        &mut context,
        vec![
            pay_rent_instruction(tenant.pubkey(), &keys, mock_mint.pubkey(), 1),
            pay_rent_instruction(tenant.pubkey(), &keys, mock_mint.pubkey(), 300_000_000),
        ],
        &[&tenant],
        "InsufficientTenantUsdcBalance",
        &keys,
        owner_usdc,
        tenant_usdc,
    )
    .await;

    let mut clock: anchor_lang::prelude::Clock = context.banks_client.get_sysvar().await.unwrap();
    let rent_pool: RentPool = decode(&account(&mut context, keys.rent_pool).await.data);
    clock.unix_timestamp = rent_pool.cutoff_ts;
    context.set_sysvar(&clock);
    assert_phase5_rejected_unchanged(
        &mut context,
        vec![pay_rent_instruction(
            tenant.pubkey(),
            &keys,
            mock_mint.pubkey(),
            1,
        )],
        &[&tenant],
        "RentCollectionClosed",
        &keys,
        owner_usdc,
        tenant_usdc,
    )
    .await;
}

#[tokio::test]
#[ignore = "requires both SBF artifacts from anchor build"]
async fn phase6_cutoff_snapshots_and_historical_claims_use_real_sbf_balances() {
    let sbf_dir = format!("{}/../../target/deploy", env!("CARGO_MANIFEST_DIR"));
    std::env::set_var("BPF_OUT_DIR", &sbf_dir);
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("property_vault", VAULT_ID, None);
    test.add_program("property_vault_transfer_hook", HOOK_ID, None);
    let mock_mint = Keypair::new();
    let config = Pubkey::find_program_address(&[CONFIG_SEED], &VAULT_ID).0;
    let mut config_data = vec![0; Config::SPACE];
    Config {
        admin: Pubkey::default(),
        mock_usdc_mint: mock_mint.pubkey(),
        bump: Pubkey::find_program_address(&[CONFIG_SEED], &VAULT_ID).1,
    }
    .try_serialize(&mut &mut config_data[..])
    .unwrap();
    test.add_account(
        config,
        Account {
            lamports: 10_000_000,
            data: config_data,
            owner: VAULT_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    let mut context = test.start_with_context().await;
    let owner = context.payer.pubkey();
    let mint_rent = context
        .banks_client
        .get_rent()
        .await
        .unwrap()
        .minimum_balance(spl_token::state::Mint::LEN);
    send(
        &mut context,
        vec![
            system_instruction::create_account(
                &owner,
                &mock_mint.pubkey(),
                mint_rent,
                spl_token::state::Mint::LEN as u64,
                &token::ID,
            ),
            spl_token::instruction::initialize_mint2(
                &token::ID,
                &mock_mint.pubkey(),
                &owner,
                None,
                MOCK_USDC_DECIMALS,
            )
            .unwrap(),
        ],
        &[&mock_mint],
    )
    .await
    .unwrap();
    let tenant = Keypair::new();
    let (list_ix, keys) = list_instruction(
        owner,
        config,
        mock_mint.pubkey(),
        61,
        tenant.pubkey(),
        100_000_000,
        100,
        3600,
    );
    let hook_ix = Instruction {
        program_id: HOOK_ID,
        accounts: vec![
            AccountMeta::new(owner, true),
            AccountMeta::new(keys.meta_list, false),
            AccountMeta::new_readonly(keys.mint, false),
            AccountMeta::new_readonly(keys.property, false),
            AccountMeta::new_readonly(keys.rent_pool, false),
            AccountMeta::new_readonly(keys.registry, false),
            AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
        ],
        data: vec![92, 197, 174, 197, 41, 124, 19, 3],
    };
    send(&mut context, vec![list_ix, hook_ix], &[])
        .await
        .unwrap();

    let alice = Keypair::new();
    let bob = Keypair::new();
    fund_investor(&mut context, &alice, mock_mint.pubkey(), 1_000_000_000).await;
    fund_investor(&mut context, &bob, mock_mint.pubkey(), 1_000_000_000).await;
    fund_investor(&mut context, &tenant, mock_mint.pubkey(), 500_000_000).await;
    let mut holders = vec![alice.pubkey(), bob.pubkey()];
    for holder in [&alice, &bob] {
        send(
            &mut context,
            vec![register_instruction(holder.pubkey(), &keys, None, None)],
            &[holder],
        )
        .await
        .unwrap();
    }
    let mut zero_holders = Vec::new();
    for _ in 0..8 {
        let holder = Keypair::new();
        fund_investor(&mut context, &holder, mock_mint.pubkey(), 1).await;
        send(
            &mut context,
            vec![register_instruction(holder.pubkey(), &keys, None, None)],
            &[&holder],
        )
        .await
        .unwrap();
        holders.push(holder.pubkey());
        zero_holders.push(holder);
    }
    assert_eq!(
        decode::<HolderRegistry>(&account(&mut context, keys.registry).await.data).count,
        10
    );
    for (holder, amount) in [(&alice, 600_000_000), (&bob, 400_000_000)] {
        let receipt = send(
            &mut context,
            with_transfer_budget(invest_instruction(
                holder.pubkey(),
                &keys,
                mock_mint.pubkey(),
                amount,
                [7; 32],
            )),
            &[holder],
        )
        .await
        .unwrap();
        assert!(receipt
            .logs
            .iter()
            .any(|line| line.contains(&format!("Program {HOOK_ID} invoke"))));
    }
    for amount in [75_000_000, 125_000_000] {
        send(
            &mut context,
            vec![pay_rent_instruction(
                tenant.pubkey(),
                &keys,
                mock_mint.pubkey(),
                amount,
            )],
            &[&tenant],
        )
        .await
        .unwrap();
    }
    let transfer = send(
        &mut context,
        with_transfer_budget(secondary_instruction(
            alice.pubkey(),
            bob.pubkey(),
            &keys,
            2,
        )),
        &[&alice],
    )
    .await
    .unwrap();
    assert!(transfer
        .logs
        .iter()
        .any(|line| line.contains(&format!("Program {HOOK_ID} invoke"))));
    let alice_property =
        get_associated_token_address_with_program_id(&alice.pubkey(), &keys.mint, &token_2022::ID);
    let bob_property =
        get_associated_token_address_with_program_id(&bob.pubkey(), &keys.mint, &token_2022::ID);
    assert_eq!(
        property_balance(&mut context, alice_property).await,
        (4, true)
    );
    assert_eq!(
        property_balance(&mut context, bob_property).await,
        (6, true)
    );
    let original_rent = usdc_balance(&mut context, keys.rent_vault).await;
    assert_eq!(original_rent, 200_000_000);
    let premature = finalize_instruction(owner, &keys, mock_mint.pubkey(), 0, &holders);
    let premature_error = send(&mut context, vec![premature.clone()], &[])
        .await
        .unwrap_err();
    assert!(
        premature_error.contains("RentPeriodNotReady"),
        "{premature_error}"
    );
    let rollback_error = send(
        &mut context,
        vec![
            pay_rent_instruction(tenant.pubkey(), &keys, mock_mint.pubkey(), 1),
            premature,
        ],
        &[&tenant],
    )
    .await
    .unwrap_err();
    assert!(
        rollback_error.contains("RentPeriodNotReady"),
        "{rollback_error}"
    );
    assert_eq!(
        usdc_balance(&mut context, keys.rent_vault).await,
        original_rent
    );
    set_clock_to_cutoff(&mut context, keys.rent_pool).await;
    let mut missing = finalize_instruction(owner, &keys, mock_mint.pubkey(), 0, &holders);
    missing.accounts.pop();
    let missing_error = send(&mut context, vec![missing], &[]).await.unwrap_err();
    assert!(
        missing_error.contains("InvalidHolderSnapshot"),
        "{missing_error}"
    );
    let mut duplicate = finalize_instruction(owner, &keys, mock_mint.pubkey(), 0, &holders);
    let last = duplicate.accounts.len() - 1;
    duplicate.accounts[last].pubkey = duplicate.accounts[last - 1].pubkey;
    let duplicate_error = send(&mut context, vec![duplicate], &[]).await.unwrap_err();
    assert!(
        duplicate_error.contains("InvalidHolderTokenAccount"),
        "{duplicate_error}"
    );
    let mut substituted = finalize_instruction(owner, &keys, mock_mint.pubkey(), 0, &holders);
    substituted.accounts.swap(11, 12);
    let substituted_error = send(&mut context, vec![substituted], &[])
        .await
        .unwrap_err();
    assert!(
        substituted_error.contains("InvalidHolderTokenAccount"),
        "{substituted_error}"
    );
    let mut wrong_owner = finalize_instruction(owner, &keys, mock_mint.pubkey(), 0, &holders);
    let alice_usdc = get_associated_token_address_with_program_id(
        &alice.pubkey(),
        &mock_mint.pubkey(),
        &token::ID,
    );
    wrong_owner.accounts[11].pubkey = alice_usdc;
    let wrong_owner_error = send(&mut context, vec![wrong_owner], &[])
        .await
        .unwrap_err();
    assert!(
        wrong_owner_error.contains("InvalidHolderTokenAccount"),
        "{wrong_owner_error}"
    );
    assert_eq!(
        usdc_balance(&mut context, keys.rent_vault).await,
        original_rent
    );
    let pool_before: RentPool = decode(&account(&mut context, keys.rent_pool).await.data);
    assert_eq!(pool_before.period_id, 0);
    assert_eq!(pool_before.current_deposits, 200_000_000);

    let cutoff_transfer = send(
        &mut context,
        vec![secondary_instruction(
            alice.pubkey(),
            bob.pubkey(),
            &keys,
            1,
        )],
        &[&alice],
    )
    .await
    .unwrap_err();
    assert!(
        cutoff_transfer.contains("RentCollectionClosed"),
        "{cutoff_transfer}"
    );
    let cutoff_invest = send(
        &mut context,
        vec![invest_instruction(
            alice.pubkey(),
            &keys,
            mock_mint.pubkey(),
            100_000_000,
            [7; 32],
        )],
        &[&alice],
    )
    .await
    .unwrap_err();
    assert!(
        cutoff_invest.contains("RentCollectionClosed"),
        "{cutoff_invest}"
    );
    let finalize = finalize_instruction(owner, &keys, mock_mint.pubkey(), 0, &holders);
    let finalized = send(
        &mut context,
        vec![
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            finalize.clone(),
        ],
        &[],
    )
    .await
    .unwrap();
    println!(
        "ten-holder rent finalization compute units: {}",
        finalized.compute_units
    );
    let period0_key = Pubkey::find_program_address(
        &[
            RENT_PERIOD_SEED,
            keys.property.as_ref(),
            &0u64.to_le_bytes(),
        ],
        &VAULT_ID,
    )
    .0;
    let period0: RentPeriod = decode(&account(&mut context, period0_key).await.data);
    assert_eq!(period0.holder_count, 10);
    assert_eq!(period0.balances[0..2], [4, 6]);
    assert_eq!(period0.entitlements[0..2], [64_000_000, 96_000_000]);
    assert_eq!(period0.owner_entitlement, 40_000_000);
    assert!(period0.entitlements[2..].iter().all(|amount| *amount == 0));
    assert_eq!(period0.eligible_supply, 10);
    assert_eq!(period0.total_entitlement, 160_000_000);
    let pool: RentPool = decode(&account(&mut context, keys.rent_pool).await.data);
    assert_eq!(pool.period_id, 1);
    assert_eq!(pool.current_deposits, 0);
    assert_eq!(pool.reserved_unclaimed, 160_000_000);
    assert_eq!(pool.owner_entitlement_total, 40_000_000);
    assert_eq!(pool.period_start_ts, period0.cutoff_ts);
    assert_eq!(pool.cutoff_ts - pool.period_start_ts, 3600);
    let duplicate_finalize = send(&mut context, vec![finalize], &[]).await.unwrap_err();
    assert!(
        duplicate_finalize.contains("ConstraintSeeds"),
        "{duplicate_finalize}"
    );

    let alice_claim = claim_instruction(alice.pubkey(), &keys, mock_mint.pubkey(), 0);
    let mut forged = alice_claim.clone();
    forged.accounts[4].pubkey = Pubkey::new_unique();
    assert_claim_rejected_unchanged(
        &mut context,
        forged,
        &[&alice],
        "ConstraintSeeds",
        &keys,
        period0_key,
    )
    .await;
    let mut wrong_period = alice_claim.clone();
    wrong_period.data = instruction::ClaimRent { period_id: 1 }.data();
    assert_claim_rejected_unchanged(
        &mut context,
        wrong_period,
        &[&alice],
        "ConstraintSeeds",
        &keys,
        period0_key,
    )
    .await;
    let mut wrong_property = alice_claim.clone();
    wrong_property.accounts[1].pubkey = keys.registry;
    assert_claim_rejected_unchanged(
        &mut context,
        wrong_property,
        &[&alice],
        "AccountDiscriminatorMismatch",
        &keys,
        period0_key,
    )
    .await;
    let mut wrong_vault = alice_claim.clone();
    wrong_vault.accounts[6].pubkey = keys.investment_vault;
    assert_claim_rejected_unchanged(
        &mut context,
        wrong_vault,
        &[&alice],
        "InvalidProtocolAccount",
        &keys,
        period0_key,
    )
    .await;
    let mut wrong_mint = alice_claim.clone();
    wrong_mint.accounts[5].pubkey = keys.mint;
    assert_claim_rejected_unchanged(
        &mut context,
        wrong_mint,
        &[&alice],
        "AccountOwnedByWrongProgram",
        &keys,
        period0_key,
    )
    .await;
    let mut wrong_signer = alice_claim.clone();
    wrong_signer.accounts[0].pubkey = bob.pubkey();
    assert_claim_rejected_unchanged(
        &mut context,
        wrong_signer,
        &[&bob],
        "ConstraintSeeds",
        &keys,
        period0_key,
    )
    .await;
    let alice_usdc_before = usdc_balance(&mut context, alice_usdc).await;
    send(&mut context, vec![alice_claim.clone()], &[&alice])
        .await
        .unwrap();
    assert_eq!(
        usdc_balance(&mut context, alice_usdc).await,
        alice_usdc_before + 64_000_000
    );
    assert_eq!(
        usdc_balance(&mut context, keys.rent_vault).await,
        136_000_000
    );
    let repeat_claim = send(
        &mut context,
        vec![
            ComputeBudgetInstruction::set_compute_unit_limit(300_000),
            alice_claim,
        ],
        &[&alice],
    )
    .await
    .unwrap_err();
    assert!(
        repeat_claim.contains("RentAlreadyClaimed"),
        "{repeat_claim}"
    );

    let next_transfer = send(
        &mut context,
        with_transfer_budget(secondary_instruction(
            alice.pubkey(),
            bob.pubkey(),
            &keys,
            1,
        )),
        &[&alice],
    )
    .await
    .unwrap();
    assert!(next_transfer
        .logs
        .iter()
        .any(|line| line.contains(&format!("Program {HOOK_ID} invoke"))));
    assert_eq!(
        property_balance(&mut context, alice_property).await,
        (3, true)
    );
    assert_eq!(
        property_balance(&mut context, bob_property).await,
        (7, true)
    );
    send(
        &mut context,
        vec![pay_rent_instruction(
            tenant.pubkey(),
            &keys,
            mock_mint.pubkey(),
            50_000_000,
        )],
        &[&tenant],
    )
    .await
    .unwrap();
    assert_eq!(
        usdc_balance(&mut context, keys.rent_vault).await,
        186_000_000
    );
    let duplicate_old_claim = send(
        &mut context,
        vec![
            ComputeBudgetInstruction::set_compute_unit_limit(350_000),
            claim_instruction(alice.pubkey(), &keys, mock_mint.pubkey(), 0),
        ],
        &[&alice],
    )
    .await
    .unwrap_err();
    assert!(
        duplicate_old_claim.contains("RentAlreadyClaimed"),
        "{duplicate_old_claim}"
    );
    assert_eq!(
        usdc_balance(&mut context, keys.rent_vault).await,
        186_000_000
    );
    set_clock_to_cutoff(&mut context, keys.rent_pool).await;
    let finalize1 = finalize_instruction(owner, &keys, mock_mint.pubkey(), 1, &holders);
    send(
        &mut context,
        vec![
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            finalize1,
        ],
        &[],
    )
    .await
    .unwrap();
    let period1_key = Pubkey::find_program_address(
        &[
            RENT_PERIOD_SEED,
            keys.property.as_ref(),
            &1u64.to_le_bytes(),
        ],
        &VAULT_ID,
    )
    .0;
    let period1: RentPeriod = decode(&account(&mut context, period1_key).await.data);
    assert_eq!(period1.balances[0..2], [3, 7]);
    assert_eq!(period1.entitlements[0..2], [12_000_000, 28_000_000]);
    assert_eq!(period1.owner_entitlement, 10_000_000);
    let pool: RentPool = decode(&account(&mut context, keys.rent_pool).await.data);
    assert_eq!(pool.reserved_unclaimed, 136_000_000);
    assert_eq!(pool.owner_entitlement_total, 50_000_000);

    send(
        &mut context,
        with_transfer_budget(secondary_instruction(
            bob.pubkey(),
            alice.pubkey(),
            &keys,
            7,
        )),
        &[&bob],
    )
    .await
    .unwrap();
    assert_eq!(
        property_balance(&mut context, bob_property).await,
        (0, true)
    );
    let carol = Keypair::new();
    fund_investor(&mut context, &carol, mock_mint.pubkey(), 1).await;
    send(
        &mut context,
        vec![register_instruction(
            carol.pubkey(),
            &keys,
            Some(1),
            Some(bob_property),
        )],
        &[&carol],
    )
    .await
    .unwrap();
    holders[1] = carol.pubkey();
    assert_eq!(
        decode::<HolderRegistry>(&account(&mut context, keys.registry).await.data).holders[1],
        carol.pubkey()
    );
    let bob_usdc = get_associated_token_address_with_program_id(
        &bob.pubkey(),
        &mock_mint.pubkey(),
        &token::ID,
    );
    let bob_before = usdc_balance(&mut context, bob_usdc).await;
    send(
        &mut context,
        vec![claim_instruction(
            bob.pubkey(),
            &keys,
            mock_mint.pubkey(),
            0,
        )],
        &[&bob],
    )
    .await
    .unwrap();
    send(
        &mut context,
        vec![claim_instruction(
            bob.pubkey(),
            &keys,
            mock_mint.pubkey(),
            1,
        )],
        &[&bob],
    )
    .await
    .unwrap();
    assert_eq!(
        usdc_balance(&mut context, bob_usdc).await,
        bob_before + 124_000_000
    );
    assert_eq!(
        usdc_balance(&mut context, keys.rent_vault).await,
        62_000_000
    );
    let pool: RentPool = decode(&account(&mut context, keys.rent_pool).await.data);
    assert_eq!(pool.reserved_unclaimed, 12_000_000);
    let zero_holder_claim = send(
        &mut context,
        vec![claim_instruction(
            zero_holders[0].pubkey(),
            &keys,
            mock_mint.pubkey(),
            0,
        )],
        &[&zero_holders[0]],
    )
    .await
    .unwrap_err();
    assert!(
        zero_holder_claim.contains("NoRentEntitlement"),
        "{zero_holder_claim}"
    );

    // Period 2 has no deposits. It finalizes with no new claims and leaves
    // Alice's unclaimed period-1 entitlement reserved in the vault.
    set_clock_to_cutoff(&mut context, keys.rent_pool).await;
    send(
        &mut context,
        vec![
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            finalize_instruction(owner, &keys, mock_mint.pubkey(), 2, &holders),
        ],
        &[],
    )
    .await
    .unwrap();
    let period2_key = Pubkey::find_program_address(
        &[
            RENT_PERIOD_SEED,
            keys.property.as_ref(),
            &2u64.to_le_bytes(),
        ],
        &VAULT_ID,
    )
    .0;
    let period2: RentPeriod = decode(&account(&mut context, period2_key).await.data);
    assert_eq!(period2.distributable_rent, 0);
    assert_eq!(period2.total_entitlement, 0);
    let carol_zero_claim = send(
        &mut context,
        vec![claim_instruction(
            carol.pubkey(),
            &keys,
            mock_mint.pubkey(),
            2,
        )],
        &[&carol],
    )
    .await
    .unwrap_err();
    assert!(
        carol_zero_claim.contains("NoRentEntitlement"),
        "{carol_zero_claim}"
    );
    assert_eq!(
        usdc_balance(&mut context, keys.rent_vault).await,
        62_000_000
    );

    send(
        &mut context,
        with_transfer_budget(secondary_instruction(
            alice.pubkey(),
            carol.pubkey(),
            &keys,
            3,
        )),
        &[&alice],
    )
    .await
    .unwrap();
    assert_eq!(
        property_balance(&mut context, alice_property).await,
        (7, true)
    );
    let carol_property =
        get_associated_token_address_with_program_id(&carol.pubkey(), &keys.mint, &token_2022::ID);
    assert_eq!(
        property_balance(&mut context, carol_property).await,
        (3, true)
    );
    send(
        &mut context,
        vec![pay_rent_instruction(
            tenant.pubkey(),
            &keys,
            mock_mint.pubkey(),
            1,
        )],
        &[&tenant],
    )
    .await
    .unwrap();
    set_clock_to_cutoff(&mut context, keys.rent_pool).await;
    send(
        &mut context,
        vec![
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            finalize_instruction(owner, &keys, mock_mint.pubkey(), 3, &holders),
        ],
        &[],
    )
    .await
    .unwrap();
    let period3_key = Pubkey::find_program_address(
        &[
            RENT_PERIOD_SEED,
            keys.property.as_ref(),
            &3u64.to_le_bytes(),
        ],
        &VAULT_ID,
    )
    .0;
    let period3: RentPeriod = decode(&account(&mut context, period3_key).await.data);
    assert_eq!(period3.distributable_rent, 1);
    assert_eq!(period3.total_entitlement, 0);
    let pool: RentPool = decode(&account(&mut context, keys.rent_pool).await.data);
    assert_eq!(period3.owner_entitlement, 1);
    assert_eq!(pool.reserved_unclaimed, 12_000_000);

    send(
        &mut context,
        vec![pay_rent_instruction(
            tenant.pubkey(),
            &keys,
            mock_mint.pubkey(),
            9,
        )],
        &[&tenant],
    )
    .await
    .unwrap();
    set_clock_to_cutoff(&mut context, keys.rent_pool).await;
    send(
        &mut context,
        vec![
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            finalize_instruction(owner, &keys, mock_mint.pubkey(), 4, &holders),
        ],
        &[],
    )
    .await
    .unwrap();
    let period4_key = Pubkey::find_program_address(
        &[
            RENT_PERIOD_SEED,
            keys.property.as_ref(),
            &4u64.to_le_bytes(),
        ],
        &VAULT_ID,
    )
    .0;
    let period4: RentPeriod = decode(&account(&mut context, period4_key).await.data);
    assert_eq!(period4.distributable_rent, 9);
    assert_eq!(period4.balances[0..2], [7, 3]);
    assert_eq!(period4.entitlements[0..2], [5, 2]);
    assert_eq!(period4.total_entitlement, 7);
    assert_eq!(period4.owner_entitlement, 2);
    let pool: RentPool = decode(&account(&mut context, keys.rent_pool).await.data);
    assert_eq!(pool.reserved_unclaimed, 12_000_007);
    assert_eq!(pool.owner_entitlement_total, 50_000_003);
    send(
        &mut context,
        vec![claim_instruction(
            carol.pubkey(),
            &keys,
            mock_mint.pubkey(),
            4,
        )],
        &[&carol],
    )
    .await
    .unwrap();
    send(
        &mut context,
        vec![claim_instruction(
            alice.pubkey(),
            &keys,
            mock_mint.pubkey(),
            4,
        )],
        &[&alice],
    )
    .await
    .unwrap();
    assert_eq!(
        usdc_balance(&mut context, keys.rent_vault).await,
        62_000_003
    );
    assert_eq!(
        decode::<RentPool>(&account(&mut context, keys.rent_pool).await.data).reserved_unclaimed,
        12_000_000
    );

    let owner_usdc =
        get_associated_token_address_with_program_id(&owner, &mock_mint.pubkey(), &token::ID);
    send(
        &mut context,
        vec![
            spl_associated_token_account::instruction::create_associated_token_account(
                &owner,
                &owner,
                &mock_mint.pubkey(),
                &token::ID,
            ),
        ],
        &[],
    )
    .await
    .unwrap();
    let owner_claim = owner_claim_instruction(owner, &keys, mock_mint.pubkey());
    let mut attacker_claim = owner_claim.clone();
    attacker_claim.accounts[0].pubkey = alice.pubkey();
    let error = send(&mut context, vec![attacker_claim], &[&alice])
        .await
        .unwrap_err();
    assert!(error.contains("Unauthorized"), "{error}");
    let owner_before = usdc_balance(&mut context, owner_usdc).await;
    let investment_before_owner_claim = usdc_balance(&mut context, keys.investment_vault).await;
    send(&mut context, vec![owner_claim.clone()], &[])
        .await
        .unwrap();
    assert_eq!(
        usdc_balance(&mut context, owner_usdc).await,
        owner_before + 50_000_003
    );
    assert_eq!(
        usdc_balance(&mut context, keys.rent_vault).await,
        12_000_000
    );
    assert_eq!(
        usdc_balance(&mut context, keys.investment_vault).await,
        investment_before_owner_claim
    );
    let repeat_owner_claim = send(&mut context, vec![owner_claim], &[])
        .await
        .unwrap_err();
    assert!(
        repeat_owner_claim.contains("NoOwnerRentEntitlement"),
        "{repeat_owner_claim}"
    );
    assert_eq!(
        usdc_balance(&mut context, keys.rent_vault).await,
        12_000_000
    );

    // A second property remains wholly in inventory. Its rent belongs only to
    // the owner; no later investor can claim this period.
    let (empty_list, empty_keys) = list_instruction(
        owner,
        config,
        mock_mint.pubkey(),
        62,
        tenant.pubkey(),
        100_000_000,
        100,
        3600,
    );
    let empty_hook = Instruction {
        program_id: HOOK_ID,
        accounts: vec![
            AccountMeta::new(owner, true),
            AccountMeta::new(empty_keys.meta_list, false),
            AccountMeta::new_readonly(empty_keys.mint, false),
            AccountMeta::new_readonly(empty_keys.property, false),
            AccountMeta::new_readonly(empty_keys.rent_pool, false),
            AccountMeta::new_readonly(empty_keys.registry, false),
            AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
        ],
        data: vec![92, 197, 174, 197, 41, 124, 19, 3],
    };
    send(&mut context, vec![empty_list, empty_hook], &[])
        .await
        .unwrap();
    send(
        &mut context,
        vec![pay_rent_instruction(
            tenant.pubkey(),
            &empty_keys,
            mock_mint.pubkey(),
            7,
        )],
        &[&tenant],
    )
    .await
    .unwrap();
    set_clock_to_cutoff(&mut context, empty_keys.rent_pool).await;
    send(
        &mut context,
        vec![finalize_instruction(
            owner,
            &empty_keys,
            mock_mint.pubkey(),
            0,
            &[],
        )],
        &[],
    )
    .await
    .unwrap();
    let empty_period_key = Pubkey::find_program_address(
        &[
            RENT_PERIOD_SEED,
            empty_keys.property.as_ref(),
            &0u64.to_le_bytes(),
        ],
        &VAULT_ID,
    )
    .0;
    let empty_period: RentPeriod = decode(&account(&mut context, empty_period_key).await.data);
    assert_eq!(empty_period.eligible_supply, 0);
    assert_eq!(empty_period.holder_count, 0);
    assert_eq!(empty_period.total_entitlement, 0);
    let empty_pool: RentPool = decode(&account(&mut context, empty_keys.rent_pool).await.data);
    assert_eq!(empty_period.owner_entitlement, 7);
    assert_eq!(empty_pool.owner_entitlement_total, 7);
    assert_eq!(empty_pool.reserved_unclaimed, 0);
    assert_eq!(usdc_balance(&mut context, empty_keys.rent_vault).await, 7);
    send(
        &mut context,
        vec![owner_claim_instruction(
            owner,
            &empty_keys,
            mock_mint.pubkey(),
        )],
        &[],
    )
    .await
    .unwrap();
    assert_eq!(usdc_balance(&mut context, empty_keys.rent_vault).await, 0);
    send(
        &mut context,
        vec![register_instruction(
            alice.pubkey(),
            &empty_keys,
            None,
            None,
        )],
        &[&alice],
    )
    .await
    .unwrap();
    send(
        &mut context,
        with_transfer_budget(invest_instruction(
            alice.pubkey(),
            &empty_keys,
            mock_mint.pubkey(),
            100_000_000,
            [7; 32],
        )),
        &[&alice],
    )
    .await
    .unwrap();
    let old_claim = send(
        &mut context,
        vec![claim_instruction(
            alice.pubkey(),
            &empty_keys,
            mock_mint.pubkey(),
            0,
        )],
        &[&alice],
    )
    .await
    .unwrap_err();
    assert!(old_claim.contains("NoRentEntitlement"), "{old_claim}");

    // Both properties are valid; substituting their valid accounts must still
    // fail each property's PDA and account-relationship checks.
    let investment_before = usdc_balance(&mut context, keys.investment_vault).await;
    let rent_before = usdc_balance(&mut context, keys.rent_vault).await;
    let mut crossed_invest = invest_instruction(
        alice.pubkey(),
        &keys,
        mock_mint.pubkey(),
        100_000_000,
        [7; 32],
    );
    crossed_invest.accounts[9].pubkey = empty_keys.investment_pool;
    assert!(send(&mut context, vec![crossed_invest], &[&alice])
        .await
        .is_err());
    let mut crossed_withdraw = withdraw_instruction(owner, &keys, mock_mint.pubkey(), 1);
    crossed_withdraw.accounts[2].pubkey = empty_keys.investment_pool;
    assert!(send(&mut context, vec![crossed_withdraw], &[])
        .await
        .is_err());
    let mut crossed_transfer = secondary_instruction(alice.pubkey(), carol.pubkey(), &keys, 1);
    crossed_transfer.accounts[7].pubkey = empty_keys.registry;
    assert!(send(&mut context, vec![crossed_transfer], &[&alice])
        .await
        .is_err());
    let mut crossed_payment = pay_rent_instruction(tenant.pubkey(), &keys, mock_mint.pubkey(), 1);
    crossed_payment.accounts[2].pubkey = empty_keys.rent_pool;
    assert!(send(&mut context, vec![crossed_payment], &[&tenant])
        .await
        .is_err());
    let mut crossed_finalize = finalize_instruction(owner, &keys, mock_mint.pubkey(), 5, &holders);
    crossed_finalize.accounts[5].pubkey = empty_keys.rent_pool;
    assert!(send(&mut context, vec![crossed_finalize], &[])
        .await
        .is_err());
    let empty_period_key = Pubkey::find_program_address(
        &[
            RENT_PERIOD_SEED,
            empty_keys.property.as_ref(),
            &0u64.to_le_bytes(),
        ],
        &VAULT_ID,
    )
    .0;
    let mut crossed_claim = claim_instruction(alice.pubkey(), &keys, mock_mint.pubkey(), 1);
    crossed_claim.accounts[3].pubkey = empty_period_key;
    assert!(send(&mut context, vec![crossed_claim], &[&alice])
        .await
        .is_err());
    assert_eq!(
        usdc_balance(&mut context, keys.investment_vault).await,
        investment_before
    );
    assert_eq!(
        usdc_balance(&mut context, keys.rent_vault).await,
        rent_before
    );

    // All 1,000 tokens sold: a 10,000 USDC deposit gives 8 USDC per token.
    let (full_list, full_keys) = list_instruction(
        owner,
        config,
        mock_mint.pubkey(),
        63,
        tenant.pubkey(),
        1,
        1_000,
        3600,
    );
    send(
        &mut context,
        vec![full_list, hook_initialize_instruction(owner, &full_keys)],
        &[],
    )
    .await
    .unwrap();
    for holder in [&alice, &bob] {
        send(
            &mut context,
            vec![register_instruction(
                holder.pubkey(),
                &full_keys,
                None,
                None,
            )],
            &[holder],
        )
        .await
        .unwrap();
    }
    let mut stale_allocation =
        invest_instruction(alice.pubkey(), &full_keys, mock_mint.pubkey(), 400, [7; 32]);
    stale_allocation.data = instruction::Invest {
        amount: 400,
        accepted_terms_hash: [7; 32],
        accepted_investor_allocation_bps: 7_000,
    }
    .data();
    let error = send(&mut context, vec![stale_allocation], &[&alice])
        .await
        .unwrap_err();
    assert!(error.contains("StaleInvestorAllocation"), "{error}");
    for (holder, amount) in [(&alice, 400), (&bob, 600)] {
        send(
            &mut context,
            with_transfer_budget(invest_instruction(
                holder.pubkey(),
                &full_keys,
                mock_mint.pubkey(),
                amount,
                [7; 32],
            )),
            &[holder],
        )
        .await
        .unwrap();
    }
    assert_eq!(
        property_balance(&mut context, full_keys.inventory_vault)
            .await
            .0,
        0
    );
    let tenant_usdc = get_associated_token_address_with_program_id(
        &tenant.pubkey(),
        &mock_mint.pubkey(),
        &token::ID,
    );
    send(
        &mut context,
        vec![spl_token::instruction::mint_to(
            &token::ID,
            &mock_mint.pubkey(),
            &tenant_usdc,
            &owner,
            &[],
            10_000_000_000,
        )
        .unwrap()],
        &[],
    )
    .await
    .unwrap();
    send(
        &mut context,
        vec![pay_rent_instruction(
            tenant.pubkey(),
            &full_keys,
            mock_mint.pubkey(),
            10_000_000_000,
        )],
        &[&tenant],
    )
    .await
    .unwrap();
    set_clock_to_cutoff(&mut context, full_keys.rent_pool).await;
    send(
        &mut context,
        vec![
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            finalize_instruction(
                owner,
                &full_keys,
                mock_mint.pubkey(),
                0,
                &[alice.pubkey(), bob.pubkey()],
            ),
        ],
        &[],
    )
    .await
    .unwrap();
    let full_period_key = Pubkey::find_program_address(
        &[
            RENT_PERIOD_SEED,
            full_keys.property.as_ref(),
            &0u64.to_le_bytes(),
        ],
        &VAULT_ID,
    )
    .0;
    let full_period: RentPeriod = decode(&account(&mut context, full_period_key).await.data);
    assert_eq!(full_period.eligible_supply, 1_000);
    assert_eq!(
        full_period.entitlements[0..2],
        [3_200_000_000, 4_800_000_000]
    );
    assert_eq!(full_period.owner_entitlement, 2_000_000_000);
    assert_eq!(full_period.total_entitlement, 8_000_000_000);
}
