# PropertyVault Anchor handoff for the Devnet demo

This describes the current Anchor protocol and the four-phase demo. It is an
integration contract, not evidence of a Devnet deployment or a security audit.
The newer PropertyVault POC Requirements PDF calls for one combined listing
action; the older team plan describes separate creation and tokenization. The
program implements the combined action. Rent claims use finalized balance
snapshots, as required by the newer PDF and the finalized demo, rather than a
claim-time token balance.

## Protocol decisions

- The main program ID is `DA1iVaPWd7epSAong3cGz8jX7rRJMT42zfrHWo3z5123`.
  The Transfer Hook program ID is `3eKZN7MKpkU7gjaYFdR8zV6UCr8u7FeBTKBLe2yeM6J5`.
  These are configured IDs in `Anchor.toml` and `declare_id!`; verify the
  deployed IDs before using Devnet.
- The generated IDLs are `target/idl/property_vault.json` and
  `target/idl/property_vault_transfer_hook.json`. Rebuild them with the SBF
  artifacts and use the IDLs from that build.
- The property mint is Token-2022 with zero decimals, fixed supply pre-minted
  into frozen inventory, no mint authority, and a PDA freeze authority.
  Investor positions are cumulative purchase records. Current economic rights
  come from token balances at a finalized rent cutoff.
- `Property` has no valuation, name, image, monthly-rent amount, or explicit
  status. Those are descriptive data for this MVP. The tenant may pay any
  positive amount during the collection window. A valid investment requires
  the canonical initialized Property, mint, inventory ATA, investment pool and
  vault, rent pool, registry, hook program, and hook metadata; the mint's supply,
  authorities, decimals, and hook configuration are checked. These conditions
  provide the current investment-ready lifecycle gate. There is no owner pause
  or pool-open switch.
- Holder eligibility means open, wallet-signed self-registration in one of ten
  property slots. It is **not** legal or KYC approval. An empty-balance slot
  may be replaced. Finalized claims use stored historical holder keys, so
  replacement does not change earlier entitlements.
- The current rent formula allocates `current_deposits + carry` across sold
  tokens only. Each holder receives the floor of its proportional share;
  undivided base units carry to the next period. Unsold inventory receives no
  claim. There is no owner/investor percentage split and no owner rent
  withdrawal. The split is a separate, unresolved business decision. It does
  not block a demo described as claiming the current protocol's rent share;
  do not promise an owner share in the demo.
- `claim_rent` is manual and wallet-signed. A separate property can be
  prepared with a finalized, unclaimed entitlement before the live demo.

## Required listing transaction

**One click must submit one transaction containing two top-level program
instructions in this order:**

1. Main program `list_and_tokenize_property`.
2. Hook program `initialize_extra_account_meta_list` for that new mint.

The owner signs the transaction and pays account rent. A failure of the second
instruction rolls back the first under Solana transaction atomicity. The main
listing handler itself does **not** initialize the hook metadata; sending it
alone can create a property that cannot be invested in or transferred through
the protocol. The owner can later initialize the missing metadata, but the
frontend must never treat a standalone listing signature as completion. Before
showing “ready,” confirm the transaction and read the hook-owned metadata PDA,
including its three metas in order: Property, RentPool, HolderRegistry. The
main program also parses and validates this account on investment and
protocol transfers. Simulate the complete transaction and set a compute-unit
limit with margin if the selected RPC/runtime requires it.

The implementation keeps this two-instruction design because direct CPI from
listing would require additional hook accounts and early serialization of the
new Property, RentPool, and HolderRegistry before Anchor's normal account
write-back. It would materially change the tested initialization path. Do not
describe the current composition as protocol-enforced atomic initialization;
it is an atomic **client transaction requirement**.

## Addresses and amount encoding

Derive PDAs with the indicated program ID, byte prefixes, and little-endian
`u64` integers. The owner's public key and `property_id` determine a property;
the frontend may fetch all `Property` accounts from the main program and
filter by owner for the demo. It should verify the canonical PDA for each
decoded account. No event index or transaction-history index is included.

| Account | Seeds | Program ID |
| --- | --- | --- |
| Config | `config` | Main |
| Property | `property`, owner, `property_id` as LE `u64` | Main |
| Property mint | `property_mint`, Property | Main |
| Inventory authority | `inventory_authority`, Property | Main |
| Freeze authority | `freeze_authority`, Property | Main |
| InvestmentPool | `investment_pool`, Property | Main |
| InvestorPosition | `position`, Property, investor | Main |
| RentPool | `rent_pool`, Property | Main |
| HolderRegistry | `holder_registry`, Property | Main |
| RentPeriod | `rent_period`, Property, period ID as LE `u64` | Main |
| RentClaim | `rent_claim`, Property, period ID as LE `u64`, holder | Main |
| ExtraAccountMetaList | `extra-account-metas`, property mint | Hook |

The inventory vault is the Token-2022 ATA of the Inventory authority for the
property mint. Holder token accounts are Token-2022 ATAs for the property mint.
Investment and rent vaults are distinct **legacy SPL Token** ATAs of their
respective pool PDAs for the configured mock-USDC mint. Owner, investor, tenant,
and claimant USDC accounts are their legacy SPL Token ATAs.

Property tokens have no fractions: `quantity` is a whole positive integer.
`Property.token_price` is already **raw six-decimal mock-USDC units per token**.
For `invest`, compute `amount = quantity * token_price` with integer/BigInt
arithmetic, check that the result fits `u64`, and pass that raw amount with the
current `terms_hash`. For example, a price of `100_000_000` and quantity `5`
means `500_000_000` base units, displayed as 500.000000 mock USDC. Do not use
floating-point arithmetic for the instruction amount.

## Demo instruction accounts and signers

Names and order below come from the generated IDLs and Rust account structs.
Program accounts must use the correct deployed IDs. The instruction builder
should derive canonical PDAs and ATAs; it should not accept arbitrary vaults
from UI input.

| Instruction | Required signer | Accounts in IDL order |
| --- | --- | --- |
| `initialize_config(mock_usdc_mint)` | Designated bootstrap admin | `admin`, `config`, `mock_usdc_mint_account`, `system_program` |
| `list_and_tokenize_property(property_id, tenant, token_price, max_supply, terms_hash, duration_seconds)` | Owner Wallet A | `owner`, `config`, `property`, `property_mint`, `inventory_authority`, `freeze_authority`, `inventory_vault`, `investment_pool`, `investment_vault`, `rent_pool`, `rent_vault`, `holder_registry`, `mock_usdc_mint`, `token_2022_program`, `legacy_token_program`, `associated_token_program`, `system_program` |
| Hook `initialize_extra_account_meta_list()` | Same Owner Wallet A, same transaction as listing | `payer`, `extra_account_meta_list`, `mint`, `property`, `rent_pool`, `holder_registry`, `system_program` |
| `register_holder(replace_index)` | Holder Wallet B | `holder`, `property`, `mint`, `registry`, `rent_pool`, `holder_token`, `freeze_authority`, `token_2022_program`, `associated_token_program`, `system_program`, optional `previous_holder_token` |
| `invest(amount, accepted_terms_hash)` | Investor Wallet B | `investor`, `property`, `mint`, `inventory_authority`, `inventory_vault`, `freeze_authority`, `investor_property_token`, `registry`, `rent_pool`, `investment_pool`, `position`, `mock_usdc_mint`, `investor_usdc`, `investment_vault`, `extra_account_meta_list`, `hook_program`, `token_2022_program`, `legacy_token_program`, `system_program` |
| `withdraw_raised_capital(amount)` | Property owner Wallet A | `owner`, `property`, `investment_pool`, `investment_vault`, `owner_usdc`, `mock_usdc_mint`, `legacy_token_program` |
| `pay_rent(amount)` | Stored tenant wallet | `tenant`, `property`, `rent_pool`, `rent_vault`, `tenant_usdc`, `mock_usdc_mint`, `legacy_token_program` |
| `finalize_rent_period()` | Any funded payer; preparation phase | `payer`, `property`, `mint`, `inventory_vault`, `registry`, `rent_pool`, `mock_usdc_mint`, `rent_vault`, `rent_period`, `system_program`, `token_2022_program`, followed by exactly one read-only canonical Token-2022 ATA for every active registry entry **in registry order**, including zero-balance accounts |
| `claim_rent(period_id)` | Historical holder Wallet B | `holder`, `property`, `rent_pool`, `rent_period`, `rent_claim`, `mock_usdc_mint`, `rent_vault`, `holder_usdc`, `legacy_token_program`, `system_program` |

The `transfer_property_tokens` instruction is available for protocol-controlled
secondary transfers but is not required by the four-phase demo. Its signer is
the source holder; its IDL names are `source_holder`, `property`, `mint`,
`source_token`, `destination_token`, `freeze_authority`, `registry`, `rent_pool`,
`extra_account_meta_list`, `hook_program`, `property_token_program`.

The mock-USDC mint is fixed once in the singleton Config PDA by
`initialize_config`. It must be a six-decimal legacy SPL Token mint. The
listing instruction checks it against Config, and later payment instructions
check it against the Property. The bootstrap signer is the public key in
`BOOTSTRAP_ADMIN` in `constants.rs`; keep its private key out of the repo.

## Reads and transaction states

- **Property discovery:** fetch main-program `Property` accounts, verify each
  PDA from its stored owner and property ID, and use its mint and price. The
  image and name are off-chain descriptive frontend data, not Property fields.
- **Available investment capital:** read `InvestmentPool.total_raised` and
  `total_withdrawn`, plus the actual investment-vault token balance. The
  withdrawable amount is bounded by both `raised - withdrawn` and vault
  balance. Display the pool totals separately from its balance.
- **Claimable rent:** for each finalized `RentPeriod` with
  `period_id < RentPool.period_id`, find Wallet B in `holders[0..holder_count]`
  and read the matching `entitlements[index]`. A positive amount is unclaimed
  if the corresponding `RentClaim` PDA has not been initialized. Use the
  period's stored entitlement even when Wallet B now holds zero tokens. Build
  `claim_rent(period_id)` with Wallet B's USDC ATA as the destination.
- **Pending/confirmed/failed:** show pending after the user signs and the RPC
  accepts a signature. Query signature status at the selected commitment and
  verify `err == null` before showing confirmed. A wallet rejection, RPC
  simulation error, expired blockhash, or status with non-null `err` is failed.
  Read the resulting accounts after confirmation; keep the signature with the
  action and render a Devnet Solscan link using that signature. This does not
  require a transaction-history index.

## Reproducible Devnet preparation — do not run as part of this handoff

1. Run the verification command below and build fresh artifacts for both
   programs. Compare IDs in `Anchor.toml`, both `declare_id!` macros, generated
   IDLs, and `solana-keygen pubkey target/deploy/<program>-keypair.json`. Keep
   keypair files private. The IDs must match before deployment.
2. Point a dedicated CLI profile at Devnet. Fund its fee payer with Devnet SOL.
   Deploy the **Transfer Hook first**, then the main program, using the two
   corresponding `.so` files and exact program-ID keypairs from `target/deploy`.
   `solana program deploy --url devnet --program-id <hook-keypair-path>
   target/deploy/property_vault_transfer_hook.so` is the hook form; repeat with
   the main keypair and `property_vault.so`. Confirm both with
   `solana program show --url devnet <program-id>`.
3. Create a **legacy SPL Token** mock-USDC mint with six decimals, for example
   `spl-token --url devnet --program-id TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA
   create-token --decimals 6`. Record the returned mint public key. Verify
   token program ownership and decimals before proceeding.
4. Using the designated bootstrap wallet (public key `BOOTSTRAP_ADMIN`), call
   `initialize_config(mock_usdc_mint)` once through the freshly generated main
   IDL. Verify the Config PDA stores that mint. Do not put wallet key material
   in source, logs, documentation, or frontend environment variables.
5. Fund owner Wallet A, investor Wallet B, and the configured tenant wallet
   with Devnet SOL for fees/account rent. Create their legacy mock-USDC ATAs;
   mint mock USDC to Wallet B and tenant using the mint authority. Owner ATA
   must exist before withdrawal. Use demo-only balances.
6. For the purchase property, submit the **two-instruction listing transaction**
   above from Wallet A. Confirm its signature, mint supply/authorities,
   inventory balance, separate vault owners, frozen state, hook program ID, and
   canonical hook metadata. Register Wallet B, then invest with raw USDC
   amount and matching terms hash. Confirm the investment vault and investor
   property-token balances, then call owner withdrawal and confirm its vault
   and `total_withdrawn` changes.
7. Prepare a second property for the rent-claim demo: list with both setup
   instructions, register Wallet B, invest, pay positive rent from the stored
   tenant during its collection window, wait until its configurable cutoff,
   and call `finalize_rent_period` with every registered holder ATA in order.
   Confirm Wallet B's stored entitlement is positive and leave its RentClaim
   PDA uninitialized. The live demo then calls `claim_rent` once from Wallet B.
8. Record program IDs, IDL hashes, mint address, PDA addresses, transaction
   signatures, account states, and Devnet Solscan links in a private demo
   checklist. Verify all links use Devnet. A local SBF test pass is not evidence
   that these Devnet transactions occurred.

## Verification command

Run from the workspace root after any Rust changes. The integration tests are
intentionally ignored by default and must be invoked explicitly. The bootstrap
test needs the **path** to the authorized local keypair file, not its contents.

```sh
export PROPERTY_VAULT_BOOTSTRAP_KEYPAIR=/absolute/path/to/authorized-wallet.json
cargo fmt --all -- --check
cargo test --workspace --lib --offline
anchor build --skip-lint
cargo test -p property-vault --test listing --offline -- --ignored --test-threads=1
```

Record the printed pass/fail/ignored counts and ensure the BPF loader reports
the freshly built program artifacts. An ordinary `cargo test` omits the SBF
integration tests and is not sufficient for this handoff. The currently
installed Anchor CLI is 1.1.2 while the Rust and TypeScript Anchor packages
are 1.2.0. The local build succeeds but warns about this mismatch; use a
matching CLI for the final deployment build and compare its IDLs and program
IDs again.
