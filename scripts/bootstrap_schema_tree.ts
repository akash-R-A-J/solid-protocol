/**
 * SolID Protocol -- per-schema credential-tree bootstrap.
 *
 * Companion to `backfill_issuer_tree.ts`.  Where that script creates the
 * singleton issuer-tree, this one creates the *per-schema credential
 * tree* that `issuer-registry::issue_credential` appends commitments to.
 *
 * Why this script exists
 * ----------------------
 * On mainnet (and on a fresh localnet) the schema-credential tree is a
 * one-shot bootstrap operation, identical in shape to the issuer-tree
 * bootstrap (ADR-0014):
 *
 *   1. Allocate an SPL Account Compression tree account, sized via
 *      `getConcurrentMerkleTreeAccountSize`.
 *   2. Call SPL AC `init_empty_merkle_tree` with the operator wallet as
 *      a *temporary* authority (a PDA cannot directly sign that ix from
 *      a client; SPL AC requires the authority key to be a transaction
 *      signer).
 *   3. Immediately call SPL AC `transfer_authority` to hand the tree
 *      over to the `[b"tree-authority", schema_hash]` PDA owned by
 *      `issuer-registry`.  This is the same PDA that
 *      `issue_credential` invoke_signed-CPIs as.
 *   4. Call `schema_registry::initialize_tree_binding(schema_hash,
 *      tree_pubkey)` so on-chain proof verification can prove
 *      `(schema_hash, tree_pubkey)` are bound.
 *
 * After (4), `issue_credential` works.  Before (4), it fails with
 * `TreeBindingMismatch` because the binding either doesn't exist or
 * was init'd with `Pubkey::default()` (and its `tree_pubkey` is
 * immutable -- programs/schema-registry/src/lib.rs:189).
 *
 * Why we don't fold this into `initialize.ts`
 * -------------------------------------------
 * `initialize.ts` is meant to be the program-bootstrap step (registry
 * config, schema, global binding, verifier config, VK).  Tree creation
 * is a different lifecycle event: on mainnet you may rotate or migrate
 * trees independently of the program initialisation.  Mirroring
 * `backfill_issuer_tree.ts`'s separation keeps every script
 * single-purpose and re-runnable.
 *
 * Re-run safety
 * -------------
 *   * If `state.merkleTreeAddress` already points at a live SPL AC
 *     account, reuses it.
 *   * If `init_empty_merkle_tree` errors with "already initialised",
 *     proceeds.
 *   * If `transfer_authority` errors with `IncorrectAuthority` (the
 *     wallet already handed off in a prior run), proceeds.
 *   * If the binding already exists, skips `initialize_tree_binding`.
 *
 * Required upstream state (`initialize.ts`):
 *   * `state.schemaHash`         hex-32
 *   * `state.schemaPda`          base58
 *   * `state.schemaTreeBindingPda` base58 (we re-derive but cross-check)
 *
 * Localnet E2E sequence (post-fix):
 *
 *   tsx scripts/initialize.ts             # skips schema + issuer bindings
 *   tsx scripts/backfill_issuer_tree.ts   # creates issuer tree
 *   tsx scripts/bootstrap_schema_tree.ts  # creates schema tree (this)
 *   tsx scripts/bootstrap_issuer.ts       # registers + approves + enrols
 *   tsx scripts/issue.ts                  # issues credential
 *   tsx scripts/prove.ts                  # generates proof + verifies
 *
 * SOLID-SEC-039 -- this script refuses to run against non-localnet RPCs
 * unless `SOLID_ALLOW_NON_LOCALNET=1` is explicitly set.
 */

import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  LAMPORTS_PER_SOL,
} from '@solana/web3.js';
import * as anchor from '@coral-xyz/anchor';
import {
  SPL_ACCOUNT_COMPRESSION_PROGRAM_ID,
  SPL_NOOP_PROGRAM_ID,
  deriveTreeAuthority,
  deriveSchemaTreeBinding,
} from '@solid-protocol/light';
import {
  getConcurrentMerkleTreeAccountSize,
} from '@solana/spl-account-compression';
import { initWasm, PROGRAM_IDS } from '@solid-protocol/core';
import * as fs from 'fs';
import * as path from 'path';
import { readState, writeState } from './lib/e2e_state';
import { loadKeypair } from './lib/keypair';

const SCHEMA_REGISTRY = new PublicKey(PROGRAM_IDS.schemaRegistry);

// Defaults match `backfill_issuer_tree.ts` (depth 16, buffer 64,
// canopy 0).  Override via env for production: depth 20, buffer 64
// or 256, canopy 10 are the recommended settings (DEFAULT_TREE_PARAMS
// in ts-sdk/packages/light/src/index.ts).  Note that the depth chosen
// here MUST match the `LEAF_DEPTH` constant in
// circuits/batch_credential_query.circom -- otherwise generated proofs
// will fail Merkle-path verification.
const SCHEMA_TREE_DEPTH = Number(process.env.SOLID_SCHEMA_TREE_DEPTH ?? '20');
const SCHEMA_TREE_BUFFER = Number(process.env.SOLID_SCHEMA_TREE_BUFFER ?? '64');
const SCHEMA_TREE_CANOPY = Number(process.env.SOLID_SCHEMA_TREE_CANOPY ?? '0');

const RPC_URL = process.env.SOLID_RPC_URL ?? 'http://127.0.0.1:8899';
const ALLOW_NON_LOCALNET = process.env.SOLID_ALLOW_NON_LOCALNET === '1';
const RPC_IS_LOCAL = /\b(localhost|127\.0\.0\.1|0\.0\.0\.0)\b/.test(RPC_URL);
if (!RPC_IS_LOCAL && !ALLOW_NON_LOCALNET) {
  console.error(
    `[bootstrap_schema_tree] Refusing to run against non-localnet RPC.\n` +
    `  SOLID_RPC_URL = ${RPC_URL}\n` +
    `Set SOLID_ALLOW_NON_LOCALNET=1 to override.\n` +
    `See sec/SECURITY_REGISTRY.md SOLID-SEC-039.`,
  );
  process.exit(2);
}

const wallet = loadKeypair();
const connection = new Connection(RPC_URL, 'confirmed');
const provider = new anchor.AnchorProvider(
  connection,
  new anchor.Wallet(wallet),
  { commitment: 'confirmed' },
);
anchor.setProvider(provider);

function loadIdl(name: string): any {
  const snake = name.replace(/[A-Z]/g, l => `_${l.toLowerCase()}`);
  for (const filename of [name, snake]) {
    for (const subdir of ['idl', 'types', 'deploy']) {
      const p = path.join('target', subdir, `${filename}.json`);
      if (fs.existsSync(p)) {
        const idl = JSON.parse(fs.readFileSync(p, 'utf-8'));
        if (!idl.address) idl.address = SCHEMA_REGISTRY.toBase58();
        return idl;
      }
    }
  }
  throw new Error(`IDL for ${name} not found in target/`);
}

function isAlreadyInitialised(e: unknown): boolean {
  const m = String((e as any)?.message ?? e);
  return m.includes('already in use') || m.includes('already initialized');
}

async function main() {
  await initWasm();
  console.log('SolID Protocol -- schema-credential tree bootstrap');
  console.log('===================================================');
  console.log(`RPC:    ${RPC_URL}`);
  console.log(`Wallet: ${wallet.publicKey.toBase58()}`);
  console.log(
    `Tree:   depth=${SCHEMA_TREE_DEPTH} buffer=${SCHEMA_TREE_BUFFER} canopy=${SCHEMA_TREE_CANOPY}`,
  );

  const state = readState('bootstrap_schema_tree');
  if (!state.schemaHash) {
    throw new Error(
      `state.schemaHash unset.  Run \`tsx scripts/initialize.ts\` first.`,
    );
  }
  const schemaHash = Uint8Array.from(Buffer.from(state.schemaHash, 'hex'));
  if (schemaHash.length !== 32) {
    throw new Error(`state.schemaHash must hex-decode to 32 bytes, got ${schemaHash.length}`);
  }

  const schemaIdl = loadIdl('schemaRegistry');
  const schemaProgram = new anchor.Program(schemaIdl, provider);

  // PDAs.  We re-derive locally and cross-check against state to catch
  // accidental schema-hash drift between initialize.ts and this run.
  const { pda: treeAuthorityPda } = deriveTreeAuthority(schemaHash);
  const { pda: bindingPda } = deriveSchemaTreeBinding(schemaHash);
  console.log(`SchemaHash:        ${state.schemaHash}`);
  console.log(`SchemaPda:         ${state.schemaPda}`);
  console.log(`Binding PDA:       ${bindingPda.toBase58()}`);
  console.log(`TreeAuthority PDA: ${treeAuthorityPda.toBase58()}`);
  if (state.schemaTreeBindingPda && state.schemaTreeBindingPda !== bindingPda.toBase58()) {
    throw new Error(
      `schemaTreeBindingPda mismatch.  state=${state.schemaTreeBindingPda}, ` +
      `derived=${bindingPda.toBase58()}.  Did the schema hash change?`,
    );
  }

  // ─── 1. Create / reuse the SPL AC tree account ────────────────────
  let treePubkey: PublicKey;
  let treeKeypair: Keypair | null = null;
  if (
    state.merkleTreeAddress &&
    state.merkleTreeAddress !== PublicKey.default.toBase58()
  ) {
    const existing = new PublicKey(state.merkleTreeAddress);
    const info = await connection.getAccountInfo(existing);
    if (info) {
      console.log(`\n[1/4] reuse tree ${existing.toBase58()}`);
      treePubkey = existing;
    } else {
      throw new Error(
        `state.merkleTreeAddress = ${existing.toBase58()} but the ` +
        `account does not exist on this RPC.  Clear the field and ` +
        `re-run to create anew, or point SOLID_RPC_URL at the right cluster.`,
      );
    }
  } else {
    console.log('\n[1/4] Creating SPL AC tree account');
    treeKeypair = Keypair.generate();
    treePubkey = treeKeypair.publicKey;
    // SPL AC's V1 header was 488b larger than the hand-rolled
    // computation we used to do for the issuer tree on 2026-04-25,
    // and `init_empty_merkle_tree` panicked with `assertion failed:
    // mid <= self.len()` (lib.rs:186).  Always trust the SDK.
    const size = getConcurrentMerkleTreeAccountSize(
      SCHEMA_TREE_DEPTH,
      SCHEMA_TREE_BUFFER,
      SCHEMA_TREE_CANOPY,
    );
    const rent = await connection.getMinimumBalanceForRentExemption(size);
    const tx = new Transaction().add(
      SystemProgram.createAccount({
        fromPubkey: wallet.publicKey,
        newAccountPubkey: treePubkey,
        lamports: rent,
        space: size,
        programId: SPL_ACCOUNT_COMPRESSION_PROGRAM_ID,
      }),
    );
    await anchor.web3.sendAndConfirmTransaction(connection, tx, [wallet, treeKeypair]);
    console.log(
      `   ok (tree=${treePubkey.toBase58()}, size=${size}b, rent=${(rent / LAMPORTS_PER_SOL).toFixed(4)} SOL)`,
    );
  }

  // ─── 2a. SPL AC init_empty_merkle_tree (wallet as temp authority) ─
  //
  // SPL AC's `init_empty_merkle_tree` requires the authority to be a
  // transaction signer; PDAs cannot sign directly from a client.  The
  // protocol's long-term answer is an `initialize_credential_tree`
  // wrapper inside `issuer-registry` that CPIs SPL AC under the
  // `(b"tree-authority", schema_hash)` PDA seeds (see SESSION_LOG_2026-04-25
  // §"Future work").  Until that lands, we use the same two-step
  // dance as `backfill_issuer_tree.ts`: init with the wallet as
  // temporary authority, then transfer to the PDA in the next ix.
  // The trade-off is one tx-window where the wallet is the authority.
  console.log('\n[2a/4] spl_account_compression::init_empty_merkle_tree');
  const INIT_EMPTY_DISC = Buffer.from([191, 11, 119, 7, 180, 107, 220, 110]);
  const TRANSFER_AUTH_DISC = Buffer.from([48, 169, 76, 72, 229, 180, 55, 161]);
  const initEmptyData = Buffer.alloc(8 + 4 + 4);
  INIT_EMPTY_DISC.copy(initEmptyData, 0);
  initEmptyData.writeUInt32LE(SCHEMA_TREE_DEPTH, 8);
  initEmptyData.writeUInt32LE(SCHEMA_TREE_BUFFER, 12);
  const initEmptyIx = new anchor.web3.TransactionInstruction({
    programId: SPL_ACCOUNT_COMPRESSION_PROGRAM_ID,
    keys: [
      { pubkey: treePubkey, isSigner: false, isWritable: true },
      { pubkey: wallet.publicKey, isSigner: true, isWritable: false },
      { pubkey: SPL_NOOP_PROGRAM_ID, isSigner: false, isWritable: false },
    ],
    data: initEmptyData,
  });
  try {
    await anchor.web3.sendAndConfirmTransaction(
      connection, new Transaction().add(initEmptyIx), [wallet],
    );
    console.log('   ok');
  } catch (e: any) {
    if (isAlreadyInitialised(e)) {
      console.log('   ok (already initialised)');
    } else {
      throw e;
    }
  }

  // ─── 2b. SPL AC transfer_authority -> (b"tree-authority", schema_hash) ─
  console.log('\n[2b/4] spl_account_compression::transfer_authority -> tree_authority_pda');
  const transferData = Buffer.concat([TRANSFER_AUTH_DISC, treeAuthorityPda.toBuffer()]);
  const transferIx = new anchor.web3.TransactionInstruction({
    programId: SPL_ACCOUNT_COMPRESSION_PROGRAM_ID,
    keys: [
      { pubkey: treePubkey, isSigner: false, isWritable: true },
      { pubkey: wallet.publicKey, isSigner: true, isWritable: false },
    ],
    data: transferData,
  });
  try {
    await anchor.web3.sendAndConfirmTransaction(
      connection, new Transaction().add(transferIx), [wallet],
    );
    console.log('   ok');
  } catch (e: any) {
    const m = String(e?.message ?? e);
    // Re-runs: the wallet is no longer authority and SPL AC will
    // reject; treat as idempotent success.
    if (m.includes('IncorrectAuthority') || m.includes('already')) {
      console.log('   ok (already transferred)');
    } else {
      throw e;
    }
  }

  // ─── 3. schema_registry::initialize_tree_binding ──────────────────
  //
  // INIT-ONLY: programs/schema-registry/src/lib.rs:189.  Once this PDA
  // is created with `tree_pubkey`, the field is immutable.  That is
  // why we cannot afford to call this with a placeholder in
  // `initialize.ts` -- if we did, the binding would be permanently
  // dead and we would have to use a different schema (or re-deploy
  // the schema-registry program) to recover.
  console.log('\n[3/4] schema_registry::initialize_tree_binding');
  try {
    await schemaProgram.methods.initializeTreeBinding(
      Array.from(schemaHash),
      treePubkey,
    ).accounts({
      schemaAccount: new PublicKey(state.schemaPda),
      schemaTreeBinding: bindingPda,
      authority: wallet.publicKey,
      systemProgram: SystemProgram.programId,
    }).rpc();
    console.log(`   ok (binding=${bindingPda.toBase58()}, tree=${treePubkey.toBase58()})`);
  } catch (e: any) {
    if (!isAlreadyInitialised(e)) throw e;
    // Re-run: confirm the existing binding actually points at our
    // tree.  If the binding was init'd against a different tree (e.g.
    // by a stale `initialize.ts` run), bail loudly -- the binding is
    // immutable and we cannot recover here.
    const info = await connection.getAccountInfo(bindingPda);
    if (!info) throw new Error('binding marked already-init but account missing');
    // Layout: 8 disc | 32 schemaHash | 32 tree_pubkey | ...
    const onchainTree = new PublicKey(info.data.subarray(40, 72));
    if (!onchainTree.equals(treePubkey)) {
      throw new Error(
        `SchemaTreeBinding already initialised but points at a different tree:\n` +
        `  on-chain:  ${onchainTree.toBase58()}\n` +
        `  expected:  ${treePubkey.toBase58()}\n` +
        `The binding is INIT-ONLY (programs/schema-registry/src/lib.rs:189).\n` +
        `Either delete this state and start over with a fresh validator, or\n` +
        `set SOLID_TREE_PUBKEY=${onchainTree.toBase58()} and use that tree.`,
      );
    }
    console.log(`   ok (already bound to ${onchainTree.toBase58()})`);
  }

  // ─── 4. Persist state ─────────────────────────────────────────────
  console.log('\n[4/4] persist state');
  state.merkleTreeAddress = treePubkey.toBase58();
  state.schemaTreeBindingPda = bindingPda.toBase58();
  state.schemaTreeAuthorityPda = treeAuthorityPda.toBase58();
  state.schemaTreeDepth = SCHEMA_TREE_DEPTH;
  writeState(state);
  console.log('   ok');

  console.log('\nDone.');
  console.log(`  schema_tree         : ${treePubkey.toBase58()}`);
  console.log(`  schema_tree_binding : ${bindingPda.toBase58()}`);
  console.log(`  tree_authority      : ${treeAuthorityPda.toBase58()}`);
  console.log(`  depth=${SCHEMA_TREE_DEPTH} buffer=${SCHEMA_TREE_BUFFER} canopy=${SCHEMA_TREE_CANOPY}`);
}

main().catch(e => {
  console.error(e);
  process.exit(1);
});
