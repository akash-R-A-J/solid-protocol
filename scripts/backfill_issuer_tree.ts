/**
 * SolID Protocol -- issuer-tree backfill (ADR-0014).
 *
 * Creates the singleton SPL Account Compression tree that backs the
 * issuer tree, binds it via `initialize_issuer_tree_binding`, and
 * enrolls every currently-Approved issuer by calling
 * `append_issuer_leaf` + `update_issuer_tree_root` for each one.
 *
 * Re-run safe:
 *   * If the SPL AC tree already exists at `state.issuerMerkleTreeAddress`,
 *     reuses it.
 *   * If the binding already exists at the derived PDA, skips init.
 *   * If an issuer is already enrolled (`is_tree_enrolled == true`),
 *     skips their append.
 *
 * Requires the full on-chain pipeline (initialize.ts) to have run.
 * Does not register new issuers -- `bootstrap_issuer.ts` handles
 * that.  For localnet E2E, run:
 *
 *   tsx scripts/initialize.ts            # skips binding if unset
 *   tsx scripts/backfill_issuer_tree.ts  # creates tree + binds it
 *   tsx scripts/bootstrap_issuer.ts      # registers + approves + enrols
 *   tsx scripts/issue.ts                 # issues credential
 *   tsx scripts/prove.ts                 # generates proof + verifies
 *
 * ADR-0014 defers SPL AC tree parameters to the operator; defaults
 * below match the ADR (depth 16, buffer 64).  Override via env.
 */

import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  LAMPORTS_PER_SOL,
  ComputeBudgetProgram,
} from '@solana/web3.js';
import * as anchor from '@coral-xyz/anchor';
import {
  SPL_ACCOUNT_COMPRESSION_PROGRAM_ID,
  SPL_NOOP_PROGRAM_ID,
  LocalReplicaAdapter,
  poseidonHashPair,
  deriveIssuerTreeAuthority,
  deriveIssuerTreeBinding,
} from '@solid-protocol/light';
// SPL AC SDK 0.2.1 — used here to (a) compute the canonical account size
// (`getConcurrentMerkleTreeAccountSize`), and (b) build the `transfer_authority`
// instruction so we don't hand-roll discriminators.  `createInitEmptyMerkleTreeIx`
// can't be used directly because it marks the authority as a signer and we
// need a temporary wallet-as-authority init followed by a PDA hand-off
// (see comment block at "init_empty_merkle_tree" below).
import {
  getConcurrentMerkleTreeAccountSize,
  createTransferAuthorityIx,
} from '@solana/spl-account-compression';
import { initWasm, PROGRAM_IDS, computeIssuerLeaf } from '@solid-protocol/core';
import * as fs from 'fs';
import * as path from 'path';
import { readState, writeState } from './lib/e2e_state';
import { loadKeypair } from './lib/keypair';

const ISSUER_REGISTRY = new PublicKey(PROGRAM_IDS.issuerRegistry);
const ISSUER_TREE_DEPTH = Number(process.env.SOLID_ISSUER_TREE_DEPTH ?? '16');
const ISSUER_TREE_BUFFER = Number(process.env.SOLID_ISSUER_TREE_BUFFER ?? '64');
const ISSUER_TREE_CANOPY = Number(process.env.SOLID_ISSUER_TREE_CANOPY ?? '0');

const RPC_URL = process.env.SOLID_RPC_URL ?? 'http://127.0.0.1:8899';
const ALLOW_NON_LOCALNET = process.env.SOLID_ALLOW_NON_LOCALNET === '1';
const RPC_IS_LOCAL = /\b(localhost|127\.0\.0\.1|0\.0\.0\.0)\b/.test(RPC_URL);
if (!RPC_IS_LOCAL && !ALLOW_NON_LOCALNET) {
  console.error(
    `[backfill_issuer_tree] Refusing to run against non-localnet RPC.\n` +
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
        if (!idl.address) idl.address = ISSUER_REGISTRY.toBase58();
        return idl;
      }
    }
  }
  throw new Error(`IDL for ${name} not found in target/`);
}

/// Compute SPL AC's CreateTree account size via the documented formula:
///   8 (discriminator)
/// + 136 (tree header)
/// + ( (1 << maxDepth) * 2 - 1 ) * 32   (canonical tree bytes; we only
///   actually pay for maxBufferSize rolling roots, but the account has
///   to be large enough for the `ChangeLogEvent` ring).
///
/// Simpler: SPL AC exposes `getConcurrentMerkleTreeAccountSize` in its
/// JS SDK.  We avoid that dependency by computing the known-good size
/// inline.  For (depth=16, buffer=64, canopy=0) the size is 12_152 bytes.
function concurrentMerkleTreeAccountSize(
  maxDepth: number,
  maxBufferSize: number,
  canopyDepth: number,
): number {
  // Constants from spl-account-compression/src/state/concurrent_merkle_tree_header.rs
  // and spl-account-compression/src/state/path_node.rs.
  const HEADER_BYTES = 136;
  // ChangeLog path = maxDepth nodes x 32 bytes.
  const CHANGE_LOG_BYTES_PER_ENTRY = 32 * maxDepth + 32 + 4 + 4;
  const changelogBytes = CHANGE_LOG_BYTES_PER_ENTRY * maxBufferSize;
  // Canopy is 2^(canopyDepth+1) - 2 nodes at 32 bytes each.
  const canopyBytes = canopyDepth > 0
    ? (Math.pow(2, canopyDepth + 1) - 2) * 32
    : 0;
  return 8 + HEADER_BYTES + changelogBytes + canopyBytes;
}

async function main() {
  await initWasm();
  console.log('SolID Protocol -- issuer-tree backfill (ADR-0014)');
  console.log('==================================================');
  console.log(`RPC:    ${RPC_URL}`);
  console.log(`Wallet: ${wallet.publicKey.toBase58()}`);
  console.log(
    `Tree:   depth=${ISSUER_TREE_DEPTH} buffer=${ISSUER_TREE_BUFFER} canopy=${ISSUER_TREE_CANOPY}`,
  );

  const state = readState('backfill_issuer_tree');
  const issuerIdl = loadIdl('issuerRegistry');
  const issuerProgram = new anchor.Program(issuerIdl, provider);

  // `derive*` returns `{ pda, bump }` (NOT a tuple).  Tracked under the
  // SDK return-shape consistency note in SESSION_LOG_2026-04-25.
  const { pda: bindingPda } = deriveIssuerTreeBinding();
  const { pda: treeAuthorityPda } = deriveIssuerTreeAuthority();
  console.log(`Binding PDA:        ${bindingPda.toBase58()}`);
  console.log(`TreeAuthority PDA:  ${treeAuthorityPda.toBase58()}`);

  // ─── 1. Create the SPL AC tree account (if absent) ─────────────────
  let treeKeypair: Keypair;
  let createdTree = false;
  if (state.issuerMerkleTreeAddress && state.issuerMerkleTreeAddress !== PublicKey.default.toBase58()) {
    const existing = new PublicKey(state.issuerMerkleTreeAddress);
    const info = await connection.getAccountInfo(existing);
    if (info) {
      if (!info.owner.equals(SPL_ACCOUNT_COMPRESSION_PROGRAM_ID)) {
        throw new Error(
          `state.issuerMerkleTreeAddress = ${existing.toBase58()} exists but is ` +
          `owned by ${info.owner.toBase58()}, expected ${SPL_ACCOUNT_COMPRESSION_PROGRAM_ID.toBase58()}.`,
        );
      }
      console.log(`\n[1/4] reuse tree ${existing.toBase58()}`);
      treeKeypair = Keypair.generate(); // unused
      treeKeypair = { publicKey: existing, secretKey: new Uint8Array(64) } as any;
    } else {
      throw new Error(
        `state.issuerMerkleTreeAddress = ${existing.toBase58()} but the ` +
        `account does not exist.  Delete the field and re-run to create anew.`,
      );
    }
  } else {
    console.log('\n[1/4] Creating SPL AC tree account');
    treeKeypair = Keypair.generate();
    // Use the SDK's canonical size; our prior hand-rolled
    // `concurrentMerkleTreeAccountSize` returned 35472b for (16,64,0)
    // but the deployed mainnet SPL AC expects 35960b and panicked
    // (`assertion failed: mid <= self.len()` at lib.rs:186) on init.
    // 2026-04-25 -- mismatch was 488b in the V1 header layout.
    const size = getConcurrentMerkleTreeAccountSize(
      ISSUER_TREE_DEPTH,
      ISSUER_TREE_BUFFER,
      ISSUER_TREE_CANOPY,
    );
    const rent = await connection.getMinimumBalanceForRentExemption(size);
    const tx = new Transaction().add(
      SystemProgram.createAccount({
        fromPubkey: wallet.publicKey,
        newAccountPubkey: treeKeypair.publicKey,
        lamports: rent,
        space: size,
        programId: SPL_ACCOUNT_COMPRESSION_PROGRAM_ID,
      }),
    );
    await anchor.web3.sendAndConfirmTransaction(connection, tx, [wallet, treeKeypair]);
    createdTree = true;
    console.log(
      `   ok (tree=${treeKeypair.publicKey.toBase58()}, size=${size}b, rent=${(rent / LAMPORTS_PER_SOL).toFixed(4)} SOL)`,
    );
  }

  // ─── 2. SPL AC init_empty_merkle_tree ──────────────────────────────
  //
  // SPL AC's `init_empty_merkle_tree` accounts (program v0.4+):
  //   [0] merkle_tree     (writable)
  //   [1] authority       (signer)            <-- becomes tree authority
  //   [2] noop            (program)
  //
  // Our protocol needs the issuer-registry's `tree_authority_pda` to be
  // the on-chain authority (because `append_issuer_leaf` /
  // `revoke_issuer_atomic` invoke_signed CPI as that PDA).  But a PDA
  // can't sign `init_empty_merkle_tree` directly here; only an in-program
  // `invoke_signed` can do that, and the issuer-registry currently
  // exposes no `initialize_issuer_tree` wrapper (gap noted below).
  //
  // Two workable paths:
  //   (a) Add an `initialize_issuer_tree` instruction to issuer-registry
  //       that CPIs SPL AC under the PDA seeds.  Architecturally the
  //       cleanest, but requires a program change + redeploy.
  //   (b) Init with `wallet` as the temp authority, then call SPL AC's
  //       `transfer_authority` to hand off to the PDA.  No program
  //       change; one extra tx.
  //
  // We pick (b) here for the e2e because it requires no program
  // changes.  The trade-off: one tx-window where the tree authority
  // is the operator wallet, immediately closed by `transfer_authority`.
  // Long-term, `initialize_issuer_tree` (option a) is the correct fix
  // and is tracked in SESSION_LOG_2026-04-25 §"Future work".
  console.log('\n[2a/4] spl_account_compression::init_empty_merkle_tree');
  const INIT_EMPTY_DISC = Buffer.from([191, 11, 119, 7, 180, 107, 220, 110]);
  const TRANSFER_AUTH_DISC = Buffer.from([48, 169, 76, 72, 229, 180, 55, 161]);
  if (createdTree) {
    const initEmptyData = Buffer.alloc(8 + 4 + 4);
    INIT_EMPTY_DISC.copy(initEmptyData, 0);
    initEmptyData.writeUInt32LE(ISSUER_TREE_DEPTH, 8);
    initEmptyData.writeUInt32LE(ISSUER_TREE_BUFFER, 12);
    const initEmptyIx = new anchor.web3.TransactionInstruction({
      programId: SPL_ACCOUNT_COMPRESSION_PROGRAM_ID,
      keys: [
        { pubkey: treeKeypair.publicKey, isSigner: false, isWritable: true },
        { pubkey: wallet.publicKey, isSigner: true, isWritable: false },
        { pubkey: SPL_NOOP_PROGRAM_ID, isSigner: false, isWritable: false },
      ],
      data: initEmptyData,
    });
    await anchor.web3.sendAndConfirmTransaction(
      connection, new Transaction().add(initEmptyIx), [wallet],
    );
    console.log('   ok');
  } else {
    console.log('   ok (existing tree; init skipped)');
  }

  // ─── 2b. SPL AC transfer_authority -> issuer-registry PDA ──────────
  //
  // SPL AC `transfer_authority` accounts:
  //   [0] merkle_tree          (writable)
  //   [1] authority            (signer, current authority)
  //   [2] new_authority        (readonly, no-sign)
  // Data: discriminator(8) | new_authority(32)
  console.log('\n[2b/4] spl_account_compression::transfer_authority -> tree_authority_pda');
  if (createdTree) {
    const transferData = Buffer.concat([TRANSFER_AUTH_DISC, treeAuthorityPda.toBuffer()]);
    const transferIx = new anchor.web3.TransactionInstruction({
      programId: SPL_ACCOUNT_COMPRESSION_PROGRAM_ID,
      keys: [
        { pubkey: treeKeypair.publicKey, isSigner: false, isWritable: true },
        { pubkey: wallet.publicKey, isSigner: true, isWritable: false },
      ],
      data: transferData,
    });
    await anchor.web3.sendAndConfirmTransaction(
      connection, new Transaction().add(transferIx), [wallet],
    );
    console.log('   ok');
  } else {
    console.log('   ok (existing tree; authority transfer skipped)');
  }

  // ─── 3. initialize_issuer_tree_binding ────────────────────────────
  console.log('\n[3/4] initialize_issuer_tree_binding');
  try {
    await issuerProgram.methods.initializeIssuerTreeBinding(
      treeKeypair.publicKey,
    ).accounts({
      registryConfig: new PublicKey(state.registryPda),
      issuerTreeBinding: bindingPda,
      authority: wallet.publicKey,
      systemProgram: SystemProgram.programId,
    }).rpc();
    console.log('   ok');
  } catch (e: any) {
    const m = String(e?.message ?? e);
    if (!m.includes('already in use') && !m.includes('already initialized')) throw e;
    console.log('   ok (already bound)');
  }

  // ─── 4. Enroll existing approved issuers ─────────────────────────
  console.log('\n[4/4] enrolling approved issuers');
  const approved: any[] = [];
  // `scripts/build_idls.mjs` denamespaces account names so Anchor's TS
  // client exposes them under their unqualified camelCased form (e.g.
  // `issuerAccount` rather than `issuerRegistry::issuerAccount`).
  // See plan/SESSION_LOG_2026-04-25_PART2.md for the historical context.
  const issuerNs = (issuerProgram.account as any).issuerAccount;
  if (!issuerNs) {
    throw new Error(
      `Could not find IssuerAccount in IDL namespace. Available: ${Object.keys(issuerProgram.account).join(', ')}`,
    );
  }
  const all = await issuerNs.all();
  for (const entry of all) {
    const acc = entry.account;
    const status = acc.status && typeof acc.status === 'object'
      ? Object.keys(acc.status)[0].toLowerCase()
      : String(acc.status).toLowerCase();
    if (status === 'approved') approved.push({ pda: entry.publicKey, account: acc });
  }
  console.log(`   ${approved.length} approved issuer(s)`);

  // SOLID-SEC-059 / H1 (2026-04-30, atomic + integrity-checked):
  // `appendIssuerLeaf` updates the binding atomically via on-chain
  // Poseidon-Merkle recompute against `poseidon_proof_path`.  Caller
  // supplies the Poseidon siblings the new leaf would have at its
  // assigned index; the on-chain handler verifies via Poseidon-recompute.
  const replica = new LocalReplicaAdapter(ISSUER_TREE_DEPTH, poseidonHashPair);
  const issuerLeaf = (acc: any): Uint8Array => computeIssuerLeaf(
    acc.authority.toBytes(),
    Uint8Array.from(acc.bjjPubKeyX),
    Uint8Array.from(acc.bjjPubKeyY),
    BigInt(acc.statusEpoch.toString()),
    BigInt(acc.revocationNonce.toString()),
  );
  const enrolled = approved
    .filter(({ account: acc }) => acc.isTreeEnrolled)
    .sort((a, b) =>
      Number(a.account.issuerTreeLeafIndex.toString()) -
      Number(b.account.issuerTreeLeafIndex.toString()),
    );
  const notEnrolled = approved.filter(({ account: acc }) => !acc.isTreeEnrolled);

  for (const { pda, account: acc } of enrolled) {
    replica.appendLeaf(issuerLeaf(acc));
    console.log(`   skip ${pda.toBase58()} (already enrolled)`);
  }

  for (const { pda, account: acc } of notEnrolled) {
    if (acc.isTreeEnrolled) {
      console.log(`   skip ${pda.toBase58()} (already enrolled)`);
      continue;
    }
    const leaf = issuerLeaf(acc);
    replica.appendLeaf(leaf);
    const proof = await replica.fetch(treeKeypair.publicKey, leaf);
    const flatPath = new Uint8Array(ISSUER_TREE_DEPTH * 32);
    for (let i = 0; i < ISSUER_TREE_DEPTH; i++) {
      flatPath.set(proof.siblings[i], i * 32);
    }
    await issuerProgram.methods
      .appendIssuerLeaf(Buffer.from(flatPath))
      .accounts({
        registryConfig: new PublicKey(state.registryPda),
        issuerAccount: pda,
        issuerTreeAuthority: treeAuthorityPda,
        merkleTree: treeKeypair.publicKey,
        issuerTreeBinding: bindingPda,
        logWrapper: SPL_NOOP_PROGRAM_ID,
        compressionProgram: SPL_ACCOUNT_COMPRESSION_PROGRAM_ID,
        authority: wallet.publicKey,
      })
      .preInstructions([
        // SOLID-SEC-059 / H1: 800K covers the on-chain Poseidon-Merkle
        // recompute cost (~380K CU at depth 16) plus the SPL AC append
        // CPI (~20K CU) plus the binding write + bookkeeping (~5K CU),
        // with headroom under the 1.4M per-tx ceiling.
        ComputeBudgetProgram.setComputeUnitLimit({ units: 800_000 }),
      ])
      .rpc();
    console.log(`   enrolled ${pda.toBase58()} -> leaf_index ${(await issuerNs.fetch(pda)).issuerTreeLeafIndex}`);
  }

  const finalRoot = replica.getRoot();
  const bindingInfo = await connection.getAccountInfo(bindingPda, 'confirmed');
  if (!bindingInfo || bindingInfo.data.length < 72) {
    throw new Error(`IssuerTreeBinding not readable: ${bindingPda.toBase58()}`);
  }
  const liveRoot = Buffer.from(bindingInfo.data.subarray(40, 72));
  if (!liveRoot.equals(Buffer.from(finalRoot)) && enrolled.length + notEnrolled.length > 0) {
    const finalLeafEntry = [...enrolled, ...notEnrolled]
      .sort((a, b) =>
        Number(a.account.issuerTreeLeafIndex.toString()) -
        Number(b.account.issuerTreeLeafIndex.toString()),
      )
      .at(-1);
    if (!finalLeafEntry) throw new Error('issuer tree root mismatch but no issuer leaf is available');
    const finalLeaf = issuerLeaf(finalLeafEntry.account);
    const finalProof = await replica.fetch(treeKeypair.publicKey, finalLeaf);
    const flatPath = new Uint8Array(ISSUER_TREE_DEPTH * 32);
    for (let i = 0; i < ISSUER_TREE_DEPTH; i++) {
      flatPath.set(finalProof.siblings[i], i * 32);
    }
    await issuerProgram.methods
      .updateIssuerTreeRoot(
        Array.from(finalRoot),
        Array.from(finalLeaf),
        new anchor.BN(finalLeafEntry.account.issuerTreeLeafIndex.toString()),
        Buffer.from(flatPath),
      )
      .accounts({
        issuerTreeBinding: bindingPda,
        authority: wallet.publicKey,
      })
      .preInstructions([
        ComputeBudgetProgram.setComputeUnitLimit({ units: 800_000 }),
      ])
      .rpc();
    console.log('   repaired issuer-tree binding root from replayed enrolled leaves');
  }

  // ─── Persist state ────────────────────────────────────────────────
  state.issuerMerkleTreeAddress = treeKeypair.publicKey.toBase58();
  state.issuerTreeBindingPda = bindingPda.toBase58();
  state.issuerTreeAuthorityPda = treeAuthorityPda.toBase58();
  state.issuerTreeDepth = ISSUER_TREE_DEPTH;
  writeState(state);

  console.log('\nDone.');
  console.log(`  issuer_tree         : ${treeKeypair.publicKey.toBase58()}`);
  console.log(`  issuer_tree_binding : ${bindingPda.toBase58()}`);
  console.log(`  enrolled issuers    : ${approved.length}`);
}

main().catch(e => {
  console.error(e);
  process.exit(1);
});
