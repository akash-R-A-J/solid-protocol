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
import { initWasm, PROGRAM_IDS, computeIssuerLeaf } from '@solid-protocol/core';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { readState, writeState } from './lib/e2e_state';

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

const keypairPath = path.join(os.homedir(), '.config/solana/id.json');
const wallet = Keypair.fromSecretKey(
  Uint8Array.from(JSON.parse(fs.readFileSync(keypairPath, 'utf-8'))),
);
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

  const [bindingPda] = deriveIssuerTreeBinding();
  const [treeAuthorityPda] = deriveIssuerTreeAuthority();
  console.log(`Binding PDA:        ${bindingPda.toBase58()}`);
  console.log(`TreeAuthority PDA:  ${treeAuthorityPda.toBase58()}`);

  // ─── 1. Create the SPL AC tree account (if absent) ─────────────────
  let treeKeypair: Keypair;
  if (state.issuerMerkleTreeAddress && state.issuerMerkleTreeAddress !== PublicKey.default.toBase58()) {
    const existing = new PublicKey(state.issuerMerkleTreeAddress);
    const info = await connection.getAccountInfo(existing);
    if (info) {
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
    const size = concurrentMerkleTreeAccountSize(
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
    console.log(
      `   ok (tree=${treeKeypair.publicKey.toBase58()}, size=${size}b, rent=${(rent / LAMPORTS_PER_SOL).toFixed(4)} SOL)`,
    );
  }

  // ─── 2. SPL AC init_empty_merkle_tree ──────────────────────────────
  // Discriminator for `init_empty_merkle_tree`: sha256("global:init_empty_merkle_tree")[..8]
  // = [191, 26, 167, 158, 149, 82, 224, 161]
  console.log('\n[2/4] spl_account_compression::init_empty_merkle_tree');
  const initEmptyDisc = Buffer.from([191, 26, 167, 158, 149, 82, 224, 161]);
  const initEmptyData = Buffer.alloc(8 + 4 + 4);
  initEmptyDisc.copy(initEmptyData, 0);
  initEmptyData.writeUInt32LE(ISSUER_TREE_DEPTH, 8);
  initEmptyData.writeUInt32LE(ISSUER_TREE_BUFFER, 12);
  const initEmptyIx = new anchor.web3.TransactionInstruction({
    programId: SPL_ACCOUNT_COMPRESSION_PROGRAM_ID,
    keys: [
      { pubkey: treeKeypair.publicKey, isSigner: false, isWritable: true },
      { pubkey: treeAuthorityPda, isSigner: false, isWritable: false },
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
    const m = String(e?.message ?? e);
    if (m.includes('already in use') || m.includes('already initialized')) {
      console.log('   ok (already initialised)');
    } else {
      throw e;
    }
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
  const all = await (issuerProgram.account as any).issuerAccount.all();
  for (const entry of all) {
    const acc = entry.account;
    const status = acc.status && typeof acc.status === 'object'
      ? Object.keys(acc.status)[0].toLowerCase()
      : String(acc.status).toLowerCase();
    if (status === 'approved') approved.push({ pda: entry.publicKey, account: acc });
  }
  console.log(`   ${approved.length} approved issuer(s)`);

  // Seed a local replica so we can push `update_issuer_tree_root` after
  // each append (the on-chain SPL AC tree root is expensive to read
  // back; the replica's root matches as long as no one else mutates
  // the tree).
  const replica = new LocalReplicaAdapter(ISSUER_TREE_DEPTH, poseidonHashPair);

  for (const { pda, account: acc } of approved) {
    if (acc.isTreeEnrolled) {
      console.log(`   skip ${pda.toBase58()} (already enrolled)`);
      continue;
    }
    const leaf = computeIssuerLeaf(
      acc.authority.toBytes(),
      Uint8Array.from(acc.bjjPubKeyX),
      Uint8Array.from(acc.bjjPubKeyY),
      BigInt(acc.statusEpoch.toString()),
      BigInt(acc.revocationNonce.toString()),
    );
    await issuerProgram.methods.appendIssuerLeaf().accounts({
      registryConfig: new PublicKey(state.registryPda),
      issuerAccount: pda,
      issuerTreeAuthority: treeAuthorityPda,
      merkleTree: treeKeypair.publicKey,
      logWrapper: SPL_NOOP_PROGRAM_ID,
      compressionProgram: SPL_ACCOUNT_COMPRESSION_PROGRAM_ID,
      authority: wallet.publicKey,
    }).rpc();
    replica.appendLeaf(leaf);
    const newRoot = replica.getRoot();
    await issuerProgram.methods.updateIssuerTreeRoot(
      Array.from(newRoot),
    ).accounts({
      issuerTreeBinding: bindingPda,
      authority: wallet.publicKey,
    }).rpc();
    console.log(`   enrolled ${pda.toBase58()} -> leaf_index ${(await (issuerProgram.account as any).issuerAccount.fetch(pda)).issuerTreeLeafIndex}`);
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
