import assert from "node:assert/strict";
import { createHash, randomBytes } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { AnchorProvider, BN, Program, Wallet } from "@anchor-lang/core";
import bs58 from "bs58";
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  type TransactionInstruction,
} from "@solana/web3.js";
import * as spl from "../frontend/node_modules/@solana/spl-token/lib/cjs/index.js";

// These tests deliberately use saved Devnet IDLs. target/ is ignored by Git,
// so exact copies are kept as test fixtures for clean-checkout reproduction.
// Never use target/idl/property_vault.json: it may come from a local build.
function savedIdl(name: string): any {
  const fixture = JSON.parse(readFileSync(join(process.cwd(), "tests/fixtures", name), "utf8"));
  const target = join(process.cwd(), "target/idl", name);
  if (existsSync(target)) assert.deepEqual(JSON.parse(readFileSync(target, "utf8")), fixture, `${name} differs from the verified test fixture`);
  return fixture;
}
const idl = savedIdl("property_vault_devnet_compat.json");
const hookIdl = savedIdl("property_vault_transfer_hook.json");
const envLine = readFileSync(join(process.cwd(), "frontend/.env"), "utf8")
  .split(/\r?\n/).find((line) => line.startsWith("VITE_SOLANA_RPC_URL="));
assert.ok(envLine, "frontend/.env must define VITE_SOLANA_RPC_URL");
const rpcUrl = envLine.slice("VITE_SOLANA_RPC_URL=".length).replace(/^['"]|['"]$/g, "");
const connection = new Connection(rpcUrl, "confirmed");
const owner = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(join(homedir(), ".config/solana/id.json"), "utf8"))));
const investor = Keypair.generate();
const provider = new AnchorProvider(connection, new Wallet(owner), { commitment: "confirmed", preflightCommitment: "confirmed" });
const program = new Program(idl, provider) as any;
const hookProgram = new Program(hookIdl, provider) as any;

const MAIN = new PublicKey("DA1iVaPWd7epSAong3cGz8jX7rRJMT42zfrHWo3z5123");
const HOOK = new PublicKey("3eKZN7MKpkU7gjaYFdR8zV6UCr8u7FeBTKBLe2yeM6J5");
const USDC = new PublicKey("BoEucuYbF4nRNQqq35K7Fr9d7T5kRfJA32is5NiijGAZ");
const CONFIG = pda(MAIN, "config");
const TOKEN_PRICE = 1_000_000n; // 1 mock USDC, in six-decimal base units.
const RENT = 1_000_000n;
const SUPPLY = 10n;
const ALLOCATION_BPS = 8_000;
const DURATION_SECONDS = 180;
const TERMS = createHash("sha256").update("PropertyVault Devnet functional test agreement v1").digest();
const explorer = (signature: string) => `https://explorer.solana.com/tx/${signature}?cluster=devnet`;
const bn = (value: bigint | number) => new BN(value.toString());
const u64 = (value: bigint) => { const out = Buffer.alloc(8); out.writeBigUInt64LE(value); return out; };
const eq = (a: PublicKey, b: PublicKey) => assert.equal(a.toBase58(), b.toBase58());
const amount = (value: any) => BigInt(value.toString());
const usdcAta = (wallet: PublicKey) => spl.getAssociatedTokenAddressSync(USDC, wallet, false, spl.TOKEN_PROGRAM_ID);
const propertyAta = (wallet: PublicKey, mint: PublicKey) => spl.getAssociatedTokenAddressSync(mint, wallet, true, spl.TOKEN_2022_PROGRAM_ID);

function pda(programId: PublicKey, seed: string, ...parts: Buffer[]): PublicKey {
  return PublicKey.findProgramAddressSync([Buffer.from(seed), ...parts], programId)[0];
}

async function tokenBalance(address: PublicKey, tokenProgram = spl.TOKEN_PROGRAM_ID): Promise<bigint> {
  return BigInt((await spl.getAccount(connection, address, "confirmed", tokenProgram)).amount.toString());
}

async function accountBytes(addresses: PublicKey[]): Promise<(string | null)[]> {
  return (await connection.getMultipleAccountsInfo(addresses, "confirmed"))
    .map((info) => info?.data.toString("base64") ?? null);
}

async function confirmedTransaction(signature: string): Promise<any> {
  for (let attempt = 0; attempt < 12; attempt++) {
    const result = await connection.getTransaction(signature, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
    if (result) return result;
    await delay(1_000);
  }
  throw new Error(`Confirmed transaction ${signature} is unavailable from RPC`);
}

async function send(label: string, instructions: TransactionInstruction[], signers: Keypair[], expectedError?: string): Promise<string> {
  const latest = await connection.getLatestBlockhash("confirmed");
  const tx = new Transaction({ feePayer: signers[0].publicKey, recentBlockhash: latest.blockhash }).add(...instructions);
  tx.sign(...signers);
  let signature: string;
  try {
    signature = await connection.sendRawTransaction(tx.serialize(), { skipPreflight: Boolean(expectedError), maxRetries: 3 });
  } catch (error: any) {
    throw new Error(`${label}: ${String(error?.message ?? error).replaceAll(rpcUrl, "[configured RPC]")}`);
  }
  let confirmation: any;
  let rejectedStatus: any;
  try {
    confirmation = await connection.confirmTransaction({ signature, ...latest }, "confirmed");
  } catch (error) {
    // web3.js may reject with the plain InstructionError status object when a
    // failed signature is already visible through getSignatureStatus().
    if (!expectedError || error instanceof Error) throw error;
    rejectedStatus = error;
  }
  const receipt = await confirmedTransaction(signature);
  if (expectedError) {
    const expectedCode = idl.errors.find((entry: any) => entry.name === expectedError)?.code;
    assert.ok(expectedCode !== undefined, `${label}: expected error is absent from the compatible IDL`);
    const statusError = rejectedStatus ?? confirmation?.value.err;
    assert.equal(statusError?.InstructionError?.[0], 0, `${label}: expected the PropertyVault instruction to fail`);
    assert.equal(statusError?.InstructionError?.[1]?.Custom, expectedCode, `${label}: wrong confirmation error`);
    assert.equal(receipt.meta?.err?.InstructionError?.[0], 0, `${label}: expected a confirmed on-chain rejection`);
    assert.equal(receipt.meta?.err?.InstructionError?.[1]?.Custom, expectedCode, `${label}: wrong on-chain error`);
    const logs = (receipt.meta?.logMessages ?? []).join("\n");
    assert.match(logs, new RegExp(`Error Code: ${expectedError}\\b`), `${label}: wrong contract error`);
    console.log(`CONFIRMED REJECTION ${label}: ${signature} ${explorer(signature)}`);
  } else {
    assert.equal(confirmation.value.err, null, `${label}: confirmation failed`);
    assert.equal(receipt.meta?.err, null, `${label}: transaction failed on chain`);
    console.log(`CONFIRMED ${label}: ${signature} ${explorer(signature)}`);
  }
  return signature;
}

type Fixture = {
  id: bigint; property: PublicKey; mint: PublicKey; inventoryAuthority: PublicKey;
  freezeAuthority: PublicKey; inventoryVault: PublicKey; investmentPool: PublicKey;
  investmentVault: PublicKey; rentPool: PublicKey; rentVault: PublicKey;
  registry: PublicKey; metadata: PublicKey; position: PublicKey;
  investorUsdc: PublicKey; ownerUsdc: PublicKey; investorToken: PublicKey;
};
let f: Fixture;

function investInstruction(hash = TERMS, allocation = ALLOCATION_BPS, signer = investor.publicKey): Promise<TransactionInstruction> {
  return program.methods.invest(bn(TOKEN_PRICE), Array.from(hash), allocation).accountsStrict({
    investor: signer, property: f.property, mint: f.mint,
    inventoryAuthority: f.inventoryAuthority, inventoryVault: f.inventoryVault,
    freezeAuthority: f.freezeAuthority, investorPropertyToken: f.investorToken,
    registry: f.registry, rentPool: f.rentPool, investmentPool: f.investmentPool,
    position: f.position, mockUsdcMint: USDC, investorUsdc: f.investorUsdc,
    investmentVault: f.investmentVault, extraAccountMetaList: f.metadata,
    hookProgram: HOOK, token2022Program: spl.TOKEN_2022_PROGRAM_ID,
    legacyTokenProgram: spl.TOKEN_PROGRAM_ID, systemProgram: SystemProgram.programId,
  }).instruction();
}

describe("PropertyVault confirmed Devnet lifecycle", function () {
  this.timeout(120_000);

  it("verifies deployed ABI, existing account layouts, and funding authority", async function () {
    eq(new PublicKey(idl.address), MAIN);
    eq(new PublicKey(hookIdl.address), HOOK);
    eq(owner.publicKey, new PublicKey("HEDAri8JV8DXDU2eN7EnJ9ELkXJ5GLBGiFemnQwx8vQA"));
    const mainInfo = await connection.getAccountInfo(MAIN, "confirmed");
    const hookInfo = await connection.getAccountInfo(HOOK, "confirmed");
    assert.ok(mainInfo?.executable && hookInfo?.executable);
    const config: any = await program.account.config.fetch(CONFIG);
    eq(config.admin, owner.publicKey);
    eq(config.mockUsdcMint, USDC);
    const mint = await spl.getMint(connection, USDC, "confirmed", spl.TOKEN_PROGRAM_ID);
    eq(mint.mintAuthority!, owner.publicKey);
    assert.equal(mint.decimals, 6);
    assert.ok((await connection.getBalance(owner.publicKey, "confirmed")) > 80_000_000, "owner needs at least 0.08 SOL");
    for (const key of ["62AgUH4jRQ7py28KsZj4LJMgnbdwCgrY6PaNPGBwsk11", "9H3DuhZwEdUeRmHaAQNBY7etnnv96tiTs5UeaPApidWw"]) {
      const address = new PublicKey(key);
      const info = await connection.getAccountInfo(address, "confirmed");
      assert.equal(info?.data.length, 229);
      eq(info!.owner, MAIN);
      const property: any = await program.account.property.fetch(address);
      eq(property.mockUsdcMint, USDC);
      eq(pda(MAIN, "property", property.owner.toBuffer(), u64(amount(property.propertyId))), address);
      assert.equal(property.termsHash.length, 32);
    }
    const listing = await connection.getTransaction("2fd9Jb57QBMDRPRSjXTFA3MfWW1qCGTjsS9YiSVFi6xocLRSGYWQDiZhGPtrbm1ajLicQPoqeEpSA7GmdJWyunHH", { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
    const purchase = await connection.getTransaction("5jgMU22V9JHcaVs3p8WrYUV8yarmqUmPc7DDUrXwNyTAz8QNRBeztuSZqYdNv7JyHMZr9hbgiLZsH9jYQQ7MRkE6", { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
    assert.ok(listing?.meta && purchase?.meta, "historical confirmed transactions must be readable");
    const instructionLengths = (tx: any) => tx.transaction.message.instructions
      .filter((ix: any) => tx.transaction.message.accountKeys[ix.programIdIndex].equals(MAIN))
      .map((ix: any) => {
        const data = bs58.decode(ix.data);
        return { discriminator: Buffer.from(data.subarray(0, 8)).toString("hex"), size: data.length, accounts: ix.accounts.length };
      });
    const historical = [...instructionLengths(listing), ...instructionLengths(purchase)];
    for (const [name, size, accounts] of [["list_and_tokenize_property", 106, 17], ["invest", 50, 19]] as const) {
      const instruction = idl.instructions.find((entry: any) => entry.name === name);
      assert.ok(instruction);
      assert.ok(historical.some((entry) => entry.discriminator === Buffer.from(instruction.discriminator).toString("hex") && entry.size === size && entry.accounts === accounts), `${name} ABI differs from confirmed Devnet transaction`);
    }
    assert.deepEqual(idl.instructions.find((entry: any) => entry.name === "list_and_tokenize_property").args.map((arg: any) => arg.name), ["property_id", "tenant", "token_price", "max_supply", "terms_hash", "duration_seconds", "investor_allocation_bps"]);
    assert.deepEqual(idl.instructions.find((entry: any) => entry.name === "invest").args.map((arg: any) => arg.name), ["amount", "accepted_terms_hash", "accepted_investor_allocation_bps"]);
  });

  it("funds an isolated investor and tenant with controlled mock USDC", async function () {
    f = {} as Fixture;
    f.investorUsdc = usdcAta(investor.publicKey);
    f.ownerUsdc = usdcAta(owner.publicKey);
    const instructions = [
      SystemProgram.transfer({ fromPubkey: owner.publicKey, toPubkey: investor.publicKey, lamports: 20_000_000 }),
      spl.createAssociatedTokenAccountIdempotentInstruction(owner.publicKey, f.investorUsdc, investor.publicKey, USDC, spl.TOKEN_PROGRAM_ID),
      spl.createAssociatedTokenAccountIdempotentInstruction(owner.publicKey, f.ownerUsdc, owner.publicKey, USDC, spl.TOKEN_PROGRAM_ID),
      spl.createMintToInstruction(USDC, f.investorUsdc, owner.publicKey, 3_000_000, [], spl.TOKEN_PROGRAM_ID),
      spl.createMintToInstruction(USDC, f.ownerUsdc, owner.publicKey, 1_000_000, [], spl.TOKEN_PROGRAM_ID),
    ];
    const beforeInvestor = (await connection.getAccountInfo(f.investorUsdc)) ? await tokenBalance(f.investorUsdc) : 0n;
    const beforeOwner = (await connection.getAccountInfo(f.ownerUsdc)) ? await tokenBalance(f.ownerUsdc) : 0n;
    await send("test wallet funding", instructions, [owner]);
    assert.equal(await tokenBalance(f.investorUsdc), beforeInvestor + 3_000_000n);
    assert.equal(await tokenBalance(f.ownerUsdc), beforeOwner + 1_000_000n);
    assert.ok(await connection.getBalance(investor.publicKey) >= 20_000_000);
  });

  it("creates and verifies an isolated property, Token-2022 mint, pools, vaults, and registry", async function () {
    for (let attempt = 0; attempt < 5; attempt++) {
      f.id = randomBytes(8).readBigUInt64LE();
      f.property = pda(MAIN, "property", owner.publicKey.toBuffer(), u64(f.id));
      if (!(await connection.getAccountInfo(f.property))) break;
      if (attempt === 4) throw new Error("Could not generate an unused property ID");
    }
    f.mint = pda(MAIN, "property_mint", f.property.toBuffer());
    f.inventoryAuthority = pda(MAIN, "inventory_authority", f.property.toBuffer());
    f.freezeAuthority = pda(MAIN, "freeze_authority", f.property.toBuffer());
    f.investmentPool = pda(MAIN, "investment_pool", f.property.toBuffer());
    f.rentPool = pda(MAIN, "rent_pool", f.property.toBuffer());
    f.registry = pda(MAIN, "holder_registry", f.property.toBuffer());
    f.inventoryVault = propertyAta(f.inventoryAuthority, f.mint);
    f.investmentVault = spl.getAssociatedTokenAddressSync(USDC, f.investmentPool, true, spl.TOKEN_PROGRAM_ID);
    f.rentVault = spl.getAssociatedTokenAddressSync(USDC, f.rentPool, true, spl.TOKEN_PROGRAM_ID);
    f.metadata = pda(HOOK, "extra-account-metas", f.mint.toBuffer());
    f.position = pda(MAIN, "position", f.property.toBuffer(), investor.publicKey.toBuffer());
    f.investorToken = propertyAta(investor.publicKey, f.mint);
    const listing = await program.methods.listAndTokenizeProperty(bn(f.id), owner.publicKey, bn(TOKEN_PRICE), bn(SUPPLY), Array.from(TERMS), bn(DURATION_SECONDS), ALLOCATION_BPS).accountsStrict({
      owner: owner.publicKey, config: CONFIG, property: f.property, propertyMint: f.mint,
      inventoryAuthority: f.inventoryAuthority, freezeAuthority: f.freezeAuthority,
      inventoryVault: f.inventoryVault, investmentPool: f.investmentPool,
      investmentVault: f.investmentVault, rentPool: f.rentPool, rentVault: f.rentVault,
      holderRegistry: f.registry, mockUsdcMint: USDC, token2022Program: spl.TOKEN_2022_PROGRAM_ID,
      legacyTokenProgram: spl.TOKEN_PROGRAM_ID, associatedTokenProgram: spl.ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    }).instruction();
    const hook = await hookProgram.methods.initializeExtraAccountMetaList().accountsStrict({
      payer: owner.publicKey, extraAccountMetaList: f.metadata, mint: f.mint,
      property: f.property, rentPool: f.rentPool, holderRegistry: f.registry,
      systemProgram: SystemProgram.programId,
    }).instruction();
    await send("property listing and hook initialization", [listing, hook], [owner]);
    const propertyInfo = await connection.getAccountInfo(f.property, "confirmed");
    assert.equal(propertyInfo?.data.length, 229);
    eq(propertyInfo!.owner, MAIN);
    const property: any = await program.account.property.fetch(f.property);
    eq(property.owner, owner.publicKey);
    eq(property.mint, f.mint);
    eq(property.inventoryVault, f.inventoryVault);
    eq(property.mockUsdcMint, USDC);
    assert.equal(amount(property.tokenPrice), TOKEN_PRICE);
    assert.equal(amount(property.maxSupply), SUPPLY);
    assert.equal(property.investorAllocationBps, ALLOCATION_BPS);
    assert.deepEqual(Buffer.from(property.termsHash), TERMS);
    const mint = await spl.getMint(connection, f.mint, "confirmed", spl.TOKEN_2022_PROGRAM_ID);
    assert.equal(mint.decimals, 0);
    assert.equal(mint.supply, SUPPLY);
    assert.equal(mint.mintAuthority, null);
    eq(spl.getTransferHook(mint).programId, HOOK);
    assert.equal(await tokenBalance(f.inventoryVault, spl.TOKEN_2022_PROGRAM_ID), SUPPLY);
    const investment: any = await program.account.investmentPool.fetch(f.investmentPool);
    const rent: any = await program.account.rentPool.fetch(f.rentPool);
    const registry: any = await program.account.holderRegistry.fetch(f.registry);
    eq(investment.property, f.property);
    eq(investment.vault, f.investmentVault);
    assert.equal(amount(investment.totalRaised), 0n);
    assert.equal(amount(investment.totalWithdrawn), 0n);
    eq(rent.property, f.property);
    eq(rent.vault, f.rentVault);
    assert.equal(amount(rent.periodId), 0n);
    assert.equal(amount(rent.currentDeposits), 0n);
    assert.equal(amount(rent.cutoffTs) - amount(rent.periodStartTs), BigInt(DURATION_SECONDS));
    assert.equal(registry.count, 0);
    assert.equal(await tokenBalance(f.investmentVault), 0n);
    assert.equal(await tokenBalance(f.rentVault), 0n);
    eq((await connection.getAccountInfo(f.metadata, "confirmed"))!.owner, HOOK);
    console.log(`TEST PROPERTY ${f.property.toBase58()} ID ${f.id.toString()}`);
  });

  it("registers the investor and initializes a frozen Token-2022 account", async function () {
    const instruction = await program.methods.registerHolder(null).accountsStrict({
      holder: investor.publicKey, property: f.property, mint: f.mint, registry: f.registry,
      rentPool: f.rentPool, holderToken: f.investorToken, freezeAuthority: f.freezeAuthority,
      token2022Program: spl.TOKEN_2022_PROGRAM_ID, associatedTokenProgram: spl.ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId, previousHolderToken: null,
    }).instruction();
    await send("investor registration", [instruction], [investor]);
    const registry: any = await program.account.holderRegistry.fetch(f.registry);
    assert.equal(registry.count, 1);
    eq(registry.holders[0], investor.publicKey);
    const token = await spl.getAccount(connection, f.investorToken, "confirmed", spl.TOKEN_2022_PROGRAM_ID);
    eq(token.owner, investor.publicKey);
    eq(token.mint, f.mint);
    assert.equal(token.amount, 0n);
    assert.equal(token.isFrozen, true);
  });

  it("rejects an incorrect accepted terms hash without changing state", async function () {
    const watched = [f.investmentPool, f.investmentVault, f.inventoryVault, f.investorUsdc, f.investorToken, f.position];
    const before = await accountBytes(watched);
    const wrong = Buffer.from(TERMS); wrong[0] ^= 1;
    await send("wrong accepted terms hash", [await investInstruction(wrong)], [investor], "StaleTerms");
    assert.deepEqual(await accountBytes(watched), before);
  });

  it("rejects an incorrect investor allocation without changing state", async function () {
    const watched = [f.investmentPool, f.investmentVault, f.inventoryVault, f.investorUsdc, f.investorToken, f.position];
    const before = await accountBytes(watched);
    await send("wrong investor allocation", [await investInstruction(TERMS, 7_000)], [investor], "StaleInvestorAllocation");
    assert.deepEqual(await accountBytes(watched), before);
  });

  it("invests one token and verifies payment, inventory, and InvestorPosition", async function () {
    const beforeUsdc = await tokenBalance(f.investorUsdc);
    const beforeVault = await tokenBalance(f.investmentVault);
    const beforeInventory = await tokenBalance(f.inventoryVault, spl.TOKEN_2022_PROGRAM_ID);
    await send("one-token investment", [await investInstruction()], [investor]);
    assert.equal(await tokenBalance(f.investorUsdc), beforeUsdc - TOKEN_PRICE);
    assert.equal(await tokenBalance(f.investmentVault), beforeVault + TOKEN_PRICE);
    assert.equal(await tokenBalance(f.inventoryVault, spl.TOKEN_2022_PROGRAM_ID), beforeInventory - 1n);
    const token = await spl.getAccount(connection, f.investorToken, "confirmed", spl.TOKEN_2022_PROGRAM_ID);
    assert.equal(token.amount, 1n);
    assert.equal(token.isFrozen, true);
    const position: any = await program.account.investorPosition.fetch(f.position);
    const pool: any = await program.account.investmentPool.fetch(f.investmentPool);
    eq(position.investor, investor.publicKey);
    eq(position.property, f.property);
    assert.equal(amount(position.investedAmount), TOKEN_PRICE);
    assert.equal(amount(position.tokenAmount), 1n);
    assert.deepEqual(Buffer.from(position.acceptedTermsHash), TERMS);
    assert.equal(amount(pool.totalRaised), TOKEN_PRICE);
  });

  it("rejects unauthorized capital withdrawal without changing accounting", async function () {
    const watched = [f.investmentPool, f.investmentVault, f.investorUsdc];
    const before = await accountBytes(watched);
    const instruction = await program.methods.withdrawRaisedCapital(bn(TOKEN_PRICE)).accountsStrict({
      owner: investor.publicKey, property: f.property, investmentPool: f.investmentPool,
      investmentVault: f.investmentVault, ownerUsdc: f.investorUsdc,
      mockUsdcMint: USDC, legacyTokenProgram: spl.TOKEN_PROGRAM_ID,
    }).instruction();
    // The has_one constraint is annotated with the program's Unauthorized error.
    await send("unauthorized capital withdrawal", [instruction], [investor], "Unauthorized");
    assert.deepEqual(await accountBytes(watched), before);
  });

  it("withdraws raised capital and verifies both balances and accounting", async function () {
    const beforeOwner = await tokenBalance(f.ownerUsdc);
    const beforeVault = await tokenBalance(f.investmentVault);
    const instruction = await program.methods.withdrawRaisedCapital(bn(TOKEN_PRICE)).accountsStrict({
      owner: owner.publicKey, property: f.property, investmentPool: f.investmentPool,
      investmentVault: f.investmentVault, ownerUsdc: f.ownerUsdc,
      mockUsdcMint: USDC, legacyTokenProgram: spl.TOKEN_PROGRAM_ID,
    }).instruction();
    await send("owner capital withdrawal", [instruction], [owner]);
    assert.equal(await tokenBalance(f.ownerUsdc), beforeOwner + TOKEN_PRICE);
    assert.equal(await tokenBalance(f.investmentVault), beforeVault - TOKEN_PRICE);
    const pool: any = await program.account.investmentPool.fetch(f.investmentPool);
    assert.equal(amount(pool.totalRaised), TOKEN_PRICE);
    assert.equal(amount(pool.totalWithdrawn), TOKEN_PRICE);
    assert.equal(await tokenBalance(f.investorToken, spl.TOKEN_2022_PROGRAM_ID), 1n);
  });

  it("pays tenant rent and verifies rent-vault accounting", async function () {
    const beforeTenant = await tokenBalance(f.ownerUsdc);
    const beforeVault = await tokenBalance(f.rentVault);
    const instruction = await program.methods.payRent(bn(RENT)).accountsStrict({
      tenant: owner.publicKey, property: f.property, rentPool: f.rentPool,
      rentVault: f.rentVault, tenantUsdc: f.ownerUsdc, mockUsdcMint: USDC,
      legacyTokenProgram: spl.TOKEN_PROGRAM_ID,
    }).instruction();
    await send("tenant rent payment", [instruction], [owner]);
    assert.equal(await tokenBalance(f.ownerUsdc), beforeTenant - RENT);
    assert.equal(await tokenBalance(f.rentVault), beforeVault + RENT);
    const pool: any = await program.account.rentPool.fetch(f.rentPool);
    assert.equal(amount(pool.currentDeposits), RENT);
  });

  it("finalizes after cutoff and verifies the holder snapshot and rent split", async function () {
    this.timeout(300_000);
    const poolBefore: any = await program.account.rentPool.fetch(f.rentPool);
    const waitMs = Math.max(0, Number(poolBefore.cutoffTs.toString()) * 1_000 - Date.now() + 3_000);
    if (waitMs) await delay(waitMs);
    const period = pda(MAIN, "rent_period", f.property.toBuffer(), u64(amount(poolBefore.periodId)));
    const instruction = await program.methods.finalizeRentPeriod().accountsStrict({
      payer: owner.publicKey, property: f.property, mint: f.mint,
      inventoryVault: f.inventoryVault, registry: f.registry, rentPool: f.rentPool,
      mockUsdcMint: USDC, rentVault: f.rentVault, rentPeriod: period,
      systemProgram: SystemProgram.programId, token2022Program: spl.TOKEN_2022_PROGRAM_ID,
    }).remainingAccounts([{ pubkey: f.investorToken, isSigner: false, isWritable: false }]).instruction();
    await send("rent period finalization", [instruction], [owner]);
    const finalized: any = await program.account.rentPeriod.fetch(period);
    const poolAfter: any = await program.account.rentPool.fetch(f.rentPool);
    assert.equal(amount(finalized.periodId), 0n);
    assert.equal(amount(finalized.distributableRent), RENT);
    assert.equal(amount(finalized.totalEntitlement), 800_000n);
    assert.equal(amount(finalized.ownerEntitlement), 200_000n);
    assert.equal(amount(finalized.eligibleSupply), 1n);
    assert.equal(finalized.holderCount, 1);
    eq(finalized.holders[0], investor.publicKey);
    assert.equal(amount(finalized.balances[0]), 1n);
    assert.equal(amount(finalized.entitlements[0]), 800_000n);
    assert.equal(amount(poolAfter.periodId), 1n);
    assert.equal(amount(poolAfter.currentDeposits), 0n);
    assert.equal(amount(poolAfter.reservedUnclaimed), 800_000n);
    assert.equal(amount(poolAfter.ownerEntitlementTotal), 200_000n);
    assert.equal(await tokenBalance(f.rentVault), RENT);
  });

  it("claims investor rent and verifies claim PDA, balances, and reserved accounting", async function () {
    const claim = pda(MAIN, "rent_claim", f.property.toBuffer(), u64(0n), investor.publicKey.toBuffer());
    const period = pda(MAIN, "rent_period", f.property.toBuffer(), u64(0n));
    const beforeInvestor = await tokenBalance(f.investorUsdc);
    const beforeVault = await tokenBalance(f.rentVault);
    const instruction = await program.methods.claimRent(bn(0)).accountsStrict({
      holder: investor.publicKey, property: f.property, rentPool: f.rentPool,
      rentPeriod: period, rentClaim: claim, mockUsdcMint: USDC,
      rentVault: f.rentVault, holderUsdc: f.investorUsdc,
      legacyTokenProgram: spl.TOKEN_PROGRAM_ID, systemProgram: SystemProgram.programId,
    }).instruction();
    await send("investor rent claim", [instruction], [investor]);
    assert.equal(await tokenBalance(f.investorUsdc), beforeInvestor + 800_000n);
    assert.equal(await tokenBalance(f.rentVault), beforeVault - 800_000n);
    const claimed: any = await program.account.rentClaim.fetch(claim);
    eq(claimed.holder, investor.publicKey);
    assert.equal(amount(claimed.amount), 800_000n);
    assert.equal(amount((await program.account.rentPeriod.fetch(period) as any).claimedAmount), 800_000n);
    assert.equal(amount((await program.account.rentPool.fetch(f.rentPool) as any).reservedUnclaimed), 0n);
  });

  it("rejects a duplicate investor claim without moving rent", async function () {
    const claim = pda(MAIN, "rent_claim", f.property.toBuffer(), u64(0n), investor.publicKey.toBuffer());
    const period = pda(MAIN, "rent_period", f.property.toBuffer(), u64(0n));
    const watched = [claim, period, f.rentPool, f.rentVault, f.investorUsdc];
    const before = await accountBytes(watched);
    const instruction = await program.methods.claimRent(bn(0)).accountsStrict({
      holder: investor.publicKey, property: f.property, rentPool: f.rentPool,
      rentPeriod: period, rentClaim: claim, mockUsdcMint: USDC,
      rentVault: f.rentVault, holderUsdc: f.investorUsdc,
      legacyTokenProgram: spl.TOKEN_PROGRAM_ID, systemProgram: SystemProgram.programId,
    }).instruction();
    await send("duplicate investor rent claim", [instruction], [investor], "RentAlreadyClaimed");
    assert.deepEqual(await accountBytes(watched), before);
  });

  it("claims owner rent and verifies balances and final owner accounting", async function () {
    const beforeOwner = await tokenBalance(f.ownerUsdc);
    const beforeVault = await tokenBalance(f.rentVault);
    const instruction = await program.methods.claimOwnerRent().accountsStrict({
      owner: owner.publicKey, property: f.property, rentPool: f.rentPool,
      mockUsdcMint: USDC, rentVault: f.rentVault, ownerUsdc: f.ownerUsdc,
      legacyTokenProgram: spl.TOKEN_PROGRAM_ID,
    }).instruction();
    await send("owner rent claim", [instruction], [owner]);
    assert.equal(await tokenBalance(f.ownerUsdc), beforeOwner + 200_000n);
    assert.equal(await tokenBalance(f.rentVault), beforeVault - 200_000n);
    const pool: any = await program.account.rentPool.fetch(f.rentPool);
    assert.equal(amount(pool.ownerEntitlementTotal), 200_000n);
    assert.equal(amount(pool.ownerClaimedTotal), 200_000n);
    assert.equal(amount(pool.reservedUnclaimed), 0n);
  });
});
