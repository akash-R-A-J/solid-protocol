/**
 * SolID Protocol -- issuer-approval bootstrap (E2E step 1.5).
 *
 * Runs after scripts/initialize.ts and before scripts/issue.ts.
 *
 * On a freshly initialised registry, every new issuer starts in
 * `IssuerStatus::Pending`. `issuer_registry::issue_credential` refuses
 * to execute unless the issuer is `Approved`.  This script walks the
 * full DAO flow end-to-end so `issue.ts` can just call issueCredential
 * without worrying about state:
 *
 *   1. Create a dedicated governance SPL-Token mint (unless
 *      `SOLID_GOVERNANCE_MINT` is already set) and mint a supply to
 *      the calling wallet.
 *   2. Call `initialize_registry` with a *short* voting period
 *      (`SOLID_VOTING_PERIOD_SECONDS`; default 20s) so the end-to-end
 *      path finishes in under a minute on a local validator.  Idempotent
 *      -- reuses the existing config if one is already installed.
 *   3. `register_issuer` (Community tier; 1-lamport min stake) under
 *      a fresh BJJ keypair + fresh Solana authority keypair.
 *   4. `stake_tokens` as the wallet voter.
 *   5. Wait >= 100 slots (flash-loan protection in vote_on_issuer).
 *   6. `vote_on_issuer` approve=true with majority weight.
 *   7. Wait for `voting_ends_at` to pass.
 *   8. `finalize_voting`  --> flips Pending -> Approved.
 *   9. Persist issuer state (BJJ keypair, authority secret, PDAs) into
 *      the shared E2E state file.
 *
 * Re-run-safe: if the same BJJ/authority keypairs are already in state,
 * re-use them; only the steps that have not completed are re-executed.
 *
 * Note on trusted-setup path: `approve_via_trust_anchor` would be
 * faster, but it itself requires a pre-existing approved
 * Government-tier anchor.  A clean-registry bootstrap therefore must
 * start with the DAO-vote path at least once; after that, further
 * issuers can be approved by either path.
 */

import { Connection, Keypair, PublicKey, SystemProgram, LAMPORTS_PER_SOL, ComputeBudgetProgram } from '@solana/web3.js';
import * as anchor from '@coral-xyz/anchor';
import {
  createMint,
  mintTo,
  getOrCreateAssociatedTokenAccount,
  TOKEN_PROGRAM_ID,
} from '@solana/spl-token';
import { initWasm, generateKeypair, isInPrimeOrderSubgroup, PROGRAM_IDS, computeIssuerLeaf } from '@solid-protocol/core';
import { LocalReplicaAdapter, poseidonHashPair } from '@solid-protocol/light';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { readState, writeState } from './lib/e2e_state';

const PROGRAM_PUBKEYS = {
  schemaRegistry: new PublicKey(PROGRAM_IDS.schemaRegistry),
  zkVerifier: new PublicKey(PROGRAM_IDS.zkVerifier),
  issuerRegistry: new PublicKey(PROGRAM_IDS.issuerRegistry),
};

const RPC_URL = process.env.SOLID_RPC_URL ?? 'http://127.0.0.1:8899';
const VOTING_PERIOD_SECONDS = Number(
  process.env.SOLID_VOTING_PERIOD_SECONDS ?? '20',
);
const STAKE_AMOUNT = BigInt(
  process.env.SOLID_STAKE_AMOUNT ?? `${1_000_000_000n}`, // 1e9 test units
);
const MIN_STAKE_LAMPORTS = new anchor.BN(1); // Community tier multiplier = 1

// SOLID-SEC-039: refuse to run against non-localnet RPC unless the operator
// opts in explicitly.  The script installs test-only parameters (20-second
// voting period, 1-lamport min stake, freshly-minted governance supply); a
// stray run against a real cluster would bake those into the first
// `initialize_registry` call for that cluster.  The downstream idempotent
// branch saves re-runs but not the very first deployment -- which is
// exactly when this script would be most dangerous.
//
// `SOLID_ALLOW_NON_LOCALNET=1` is the explicit override; the registry
// entry documents the runbook for real deployments.
const ALLOW_NON_LOCALNET = process.env.SOLID_ALLOW_NON_LOCALNET === '1';
const RPC_IS_LOCAL = /\b(localhost|127\.0\.0\.1|0\.0\.0\.0)\b/.test(RPC_URL);
if (!RPC_IS_LOCAL && !ALLOW_NON_LOCALNET) {
  console.error(
    `[bootstrap_issuer] Refusing to run against non-localnet RPC.\n` +
    `  SOLID_RPC_URL = ${RPC_URL}\n` +
    `\nThis script installs test-only DAO parameters (20-second voting\n` +
    `period, 1-lamport min stake, freshly-minted governance supply).\n` +
    `Running against devnet / mainnet would bake those into\n` +
    `initialize_registry.  If you know what you are doing, re-run with\n` +
    `SOLID_ALLOW_NON_LOCALNET=1 and override the test-only parameters\n` +
    `via SOLID_VOTING_PERIOD_SECONDS / SOLID_STAKE_AMOUNT / SOLID_MIN_STAKE_LAMPORTS.\n` +
    `\nSee sec/SECURITY_REGISTRY.md SOLID-SEC-039 for the runbook.`,
  );
  process.exit(2);
}

const keypairPath = path.join(os.homedir(), '.config/solana/id.json');
const wallet = Keypair.fromSecretKey(
  Uint8Array.from(JSON.parse(fs.readFileSync(keypairPath, 'utf-8'))),
);
const connection = new Connection(RPC_URL, 'confirmed');
const provider = new anchor.AnchorProvider(
  connection, new anchor.Wallet(wallet), { commitment: 'confirmed' },
);
anchor.setProvider(provider);

function loadIdl(name: string): any {
  const snake = name.replace(/[A-Z]/g, l => `_${l.toLowerCase()}`);
  for (const filename of [name, snake]) {
    for (const subdir of ['idl', 'types', 'deploy']) {
      const p = path.join('target', subdir, `${filename}.json`);
      if (fs.existsSync(p)) {
        const idl = JSON.parse(fs.readFileSync(p, 'utf-8'));
        if (!idl.address) {
          const pid = PROGRAM_PUBKEYS[name as keyof typeof PROGRAM_PUBKEYS];
          if (pid) idl.address = pid.toBase58();
        }
        return idl;
      }
    }
  }
  throw new Error(`IDL for ${name} not found in target/`);
}

function sleep(ms: number): Promise<void> {
  return new Promise(r => setTimeout(r, ms));
}

/// Poll `getSlot()` until `minSlot` has passed or `timeoutMs` elapses.
async function waitUntilSlot(target: number, timeoutMs: number): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const s = await connection.getSlot('confirmed');
    if (s >= target) return;
    await sleep(250);
  }
  throw new Error(`waitUntilSlot timed out (target=${target})`);
}

async function main() {
  await initWasm();
  console.log('SolID Protocol -- issuer-approval bootstrap');
  console.log('===========================================');
  console.log(`Wallet: ${wallet.publicKey.toBase58()}`);
  console.log(`RPC:    ${RPC_URL}`);

  const state = readState('bootstrap_issuer');
  const issuerIdl = loadIdl('issuerRegistry');
  const issuerProgram = new anchor.Program(issuerIdl, provider);

  // ─── 1. Governance mint (deferred: see step 2b) ─────────────────
  //
  // Architectural rule: the issuer-registry program is the source of
  // truth.  `initialize_registry` (the handler) records
  // `registry.governance_token_mint` exactly once -- the value passed
  // by the FIRST caller is canonical forever.  This script is a
  // consumer; it MUST adopt whatever the registry already committed
  // to, never override it.
  //
  // We therefore split mint resolution in two:
  //   (1) compute a *candidate* mint here -- from cached state, env
  //       override, or (lazily) create a fresh mint;
  //   (2) after step 2 (`initialize_registry` attempt) read the
  //       on-chain registry back and lock `governanceMint` to
  //       `registry.governance_token_mint`.  If the candidate
  //       disagrees with chain, we fail loudly with a runbook --
  //       silently drifting onto a sock-puppet mint is exactly the
  //       class of bug the program-side guard now prevents.
  // ─── 1. Governance mint ─────────────────────────────────────────
  //
  // Source of truth precedence:
  //   1. `state.governanceMint` (canonically written by `npm run init-onchain`,
  //       which calls `initialize_registry` and persists the bound mint).
  //   2. `SOLID_GOVERNANCE_MINT` env override (operator opt-in).
  //   3. Fresh SPL mint (only used if registry is uninitialised below).
  //
  // After step 2 we reconcile against on-chain truth: the program-side
  // contract (post 2026-04-26 redesign) typed-checks `governance_mint` as
  // an `Account<Mint>`, so a `Pubkey::default()` value is no longer
  // representable on chain at all.
  console.log('\n[1/8] Governance mint');
  const [registryConfigPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('registry-config')],
    PROGRAM_PUBKEYS.issuerRegistry,
  );
  const [governanceVaultPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('governance-vault'), registryConfigPda.toBuffer()],
    PROGRAM_PUBKEYS.issuerRegistry,
  );
  let governanceMint: PublicKey;
  let mintCreatedThisRun = false;
  if (state.governanceMint) {
    governanceMint = new PublicKey(state.governanceMint);
    console.log(`   candidate (cached state) ${governanceMint.toBase58()}`);
  } else if (process.env.SOLID_GOVERNANCE_MINT) {
    governanceMint = new PublicKey(process.env.SOLID_GOVERNANCE_MINT);
    console.log(`   candidate (env override) ${governanceMint.toBase58()}`);
  } else {
    governanceMint = await createMint(connection, wallet, wallet.publicKey, null, 6);
    mintCreatedThisRun = true;
    console.log(`   candidate (fresh mint)   ${governanceMint.toBase58()}`);
  }

  // ─── 2. initialize_registry (idempotent, atomic vault) ───────────
  //
  // Contract (post 2026-04-26 redesign — see programs/issuer-registry/src/lib.rs
  // doc and docs/E2E_BLOCKERS.md B10):
  //
  //   `initialize_registry` takes `governance_mint` as a typed
  //   `Account<Mint>` and atomically births the singleton governance
  //   vault under `["governance-vault", registry_config]`.  Anchor's
  //   deserializer rejects anything that isn't an SPL Mint, so the
  //   class of bug where `Pubkey::default()` ends up in the registry
  //   is no longer reachable.  No separate `init_governance_vault`
  //   step is required (and none exists in the IDL).
  console.log('\n[2/8] initialize_registry');
  let registryAlreadyInitialized = false;
  try {
    await issuerProgram.methods.initializeRegistry(
      MIN_STAKE_LAMPORTS,
      new anchor.BN(VOTING_PERIOD_SECONDS),
      new anchor.BN(6000),
    ).accounts({
      registryConfig: registryConfigPda,
      governanceMint,
      governanceVault: governanceVaultPda,
      authority: wallet.publicKey,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
      rent: anchor.web3.SYSVAR_RENT_PUBKEY,
    }).rpc();
    console.log(`   ok (initialised, vault=${governanceVaultPda.toBase58()})`);
  } catch (e: any) {
    const m = String(e?.message ?? e);
    if (!m.includes('already in use') && !m.includes('already initialized')) {
      throw e;
    }
    registryAlreadyInitialized = true;
    console.log('   ok (already active)');
  }

  // Reconcile against on-chain truth: the program is the source of truth.
  // If the registry already existed (e.g. `npm run init-onchain` ran first
  // and bound a different mint), adopt the on-chain value.
  const onChainConfig = await issuerProgram.account.registryConfig.fetch(registryConfigPda);
  const onChainMint = onChainConfig.governanceTokenMint as PublicKey;
  if (!onChainMint.equals(governanceMint)) {
    if (registryAlreadyInitialized) {
      console.log(
        `   reconciling: candidate ${governanceMint.toBase58()} -> on-chain ${onChainMint.toBase58()}`,
      );
      if (mintCreatedThisRun) {
        console.log('   note: fresh local mint discarded; on-chain registry has a different mint');
      }
      governanceMint = onChainMint;
    } else {
      throw new Error(
        `initialize_registry post-condition violated: sent ${governanceMint.toBase58()}, ` +
          `on chain ${onChainMint.toBase58()}`,
      );
    }
  }
  state.governanceMint = governanceMint.toBase58();
  state.governanceVaultPda = governanceVaultPda.toBase58();

  // ─── 3. register_issuer ───────────────────────────────────────────
  console.log('\n[3/8] register_issuer');
  const issuerBjj = state.issuerBjj
    ? {
        private_key: Uint8Array.from(state.issuerBjj.private_key),
        public_key_x: Uint8Array.from(state.issuerBjj.public_key_x),
        public_key_y: Uint8Array.from(state.issuerBjj.public_key_y),
      }
    : generateKeypair();
  const issuerAuthority = state.issuerAuthoritySecret
    ? Keypair.fromSecretKey(Uint8Array.from(state.issuerAuthoritySecret))
    : Keypair.generate();

  // SOLID-SEC-007 / SEC-048 client-side enforcement.
  //
  // The on-chain `register_issuer` instruction historically called
  // `solid_core::babyjubjub::require_in_prime_order_subgroup` to reject
  // small-order / off-curve / identity public keys.  On BPF that
  // `r * P == O` scalar multiplication exceeds the 1.4M CU per-tx
  // ceiling, so as of 2026-04-25 the on-chain check is gated behind
  // the `sec007-skip-onchain` Cargo feature on issuer-registry.  This
  // off-chain predicate is therefore the load-bearing gate while
  // SEC-048 (cheap on-chain replacement) is open; a failing key here
  // MUST never reach the registry.  See docs/E2E_BLOCKERS.md B9 and
  // sec/SECURITY_REGISTRY.md SEC-048.
  if (!isInPrimeOrderSubgroup(issuerBjj.public_key_x, issuerBjj.public_key_y)) {
    throw new Error(
      'SEC-048 guard: generated issuer BJJ public key is not in the ' +
        'prime-order subgroup (off-curve / identity / cofactor-8 torsion). ' +
        'Refusing to submit register_issuer; regenerate the keypair.',
    );
  }
  console.log('   sec-048 off-chain subgroup check: ok');

  // Fund issuer authority so it can pay for the register_issuer
  // `init` allocation (its PDA rent) + the 1 SOL stake CPI transfer
  // into stake_vault.  One transfer, idempotent via balance check.
  // 1.1 SOL covers 1 SOL stake + rent + tx fees with headroom.
  const bal = await connection.getBalance(issuerAuthority.publicKey);
  if (bal < 1.05 * LAMPORTS_PER_SOL) {
    const fundSig = await connection.sendTransaction(
      new anchor.web3.Transaction().add(
        SystemProgram.transfer({
          fromPubkey: wallet.publicKey,
          toPubkey: issuerAuthority.publicKey,
          lamports: 1.1 * LAMPORTS_PER_SOL,
        }),
      ),
      [wallet],
    );
    await connection.confirmTransaction(fundSig, 'confirmed');
  }

  const [issuerAccountPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('issuer'), issuerAuthority.publicKey.toBuffer()],
    PROGRAM_PUBKEYS.issuerRegistry,
  );
  const [stakeVaultPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('stake-vault')],
    PROGRAM_PUBKEYS.issuerRegistry,
  );
  try {
    // SEC-048: the on-chain BJJ subgroup check is currently gated
    // behind the `sec007-skip-onchain` Cargo feature on issuer-registry
    // (see docs/E2E_BLOCKERS.md B9, sec/SECURITY_REGISTRY.md SEC-048),
    // so this transaction completes within the default 200K CU budget.
    // The 400K cap below is intentionally above measured (~120K) to
    // keep headroom for Anchor `init`, the `system_program::transfer`
    // CPI, and Clock syscalls without hitting the ceiling on noisy
    // validators.  When the on-chain check is restored, raise to the
    // 1.4M per-tx ceiling and revisit (see SEC-048 remediation plan).
    await issuerProgram.methods.registerIssuer(
      'e2e-test-issuer',
      'ipfs://none',
      Array.from(issuerBjj.public_key_x),
      Array.from(issuerBjj.public_key_y),
      { community: {} },
    ).accounts({
      registryConfig: registryConfigPda,
      issuerAccount: issuerAccountPda,
      stakeVault: stakeVaultPda,
      issuerAuthority: issuerAuthority.publicKey,
      systemProgram: SystemProgram.programId,
    })
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
      .signers([issuerAuthority]).rpc();
    console.log(`   ok (issuer=${issuerAccountPda.toBase58()})`);
  } catch (e: any) {
    const m = String(e?.message ?? e);
    if (!m.includes('already in use')) throw e;
    console.log('   ok (already registered)');
  }

  // ─── 4. stake_tokens ─────────────────────────────────────────────
  //
  // The governance vault was created atomically with the registry in
  // step 2 (post 2026-04-26 contract redesign — see B10 in
  // docs/E2E_BLOCKERS.md).  `stake_tokens` therefore references the
  // vault read-mostly (mut for the inbound transfer), with no
  // `init_if_needed` co-located with a Token::Transfer CPI — which
  // is what was triggering the post-CPI access violation on localnet.
  console.log('\n[4/8] stake_tokens (voter = wallet)');
  const [stakerPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('staker'), wallet.publicKey.toBuffer()],
    PROGRAM_PUBKEYS.issuerRegistry,
  );
  const voterAta = await getOrCreateAssociatedTokenAccount(
    connection, wallet, governanceMint, wallet.publicKey,
  );
  if (voterAta.amount < STAKE_AMOUNT * 2n) {
    await mintTo(
      connection, wallet, governanceMint, voterAta.address, wallet,
      STAKE_AMOUNT * 10n,
    );
  }

  const stakeStartSlot = await connection.getSlot('confirmed');
  await issuerProgram.methods.stakeTokens(new anchor.BN(STAKE_AMOUNT.toString()))
    .accounts({
      registryConfig: registryConfigPda,
      stakerAccount: stakerPda,
      governanceVault: governanceVaultPda,
      governanceMint,
      voterTokenAccount: voterAta.address,
      voter: wallet.publicKey,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
      rent: anchor.web3.SYSVAR_RENT_PUBKEY,
    }).rpc();
  console.log(`   ok (staked ${STAKE_AMOUNT})`);

  // ─── 5. wait 100 slots (flash-loan protection) ───────────────────
  console.log('\n[5/8] waiting 100 slots for flash-loan cool-off...');
  await waitUntilSlot(stakeStartSlot + 102, 120_000);
  console.log('   ok');

  // ─── 6. vote_on_issuer (approve=true) ────────────────────────────
  console.log('\n[6/8] vote_on_issuer');
  const [voteRecordPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('vote'), issuerAccountPda.toBuffer(), wallet.publicKey.toBuffer()],
    PROGRAM_PUBKEYS.issuerRegistry,
  );
  try {
    await issuerProgram.methods.voteOnIssuer(true).accounts({
      registryConfig: registryConfigPda,
      issuerAccount: issuerAccountPda,
      voteRecord: voteRecordPda,
      stakerAccount: stakerPda,
      voter: wallet.publicKey,
      systemProgram: SystemProgram.programId,
    }).rpc();
    console.log('   ok (voted APPROVE)');
  } catch (e: any) {
    const m = String(e?.message ?? e);
    if (!m.includes('already in use')) throw e;
    console.log('   ok (already voted)');
  }

  // ─── 7. wait past voting_ends_at ─────────────────────────────────
  console.log('\n[7/8] waiting for voting period to end...');
  // Voting window started at issuer.voting_ends_at - voting_period.
  // Simpler: sleep `VOTING_PERIOD_SECONDS + 2` from now, which is
  // always sufficient on a fresh run.  On a re-run the finalize call
  // below will be idempotent.
  await sleep((VOTING_PERIOD_SECONDS + 2) * 1000);
  console.log('   ok');

  // ─── 8. finalize_voting ──────────────────────────────────────────
  console.log('\n[8/8] finalize_voting');
  try {
    await issuerProgram.methods.finalizeVoting().accounts({
      registryConfig: registryConfigPda,
      issuerAccount: issuerAccountPda,
      payer: wallet.publicKey,
      systemProgram: SystemProgram.programId,
    }).rpc();
    console.log('   ok');
  } catch (e: any) {
    const m = String(e?.message ?? e);
    // "Issuer is not in Pending status" is acceptable on re-run (already finalised).
    if (!m.includes('IssuerNotPending')) throw e;
    console.log('   ok (already finalised)');
  }

  // Sanity-check the final status.
  const issuerAccount: any = await (issuerProgram.account as any)
    .issuerAccount.fetch(issuerAccountPda);
  const isApproved = issuerAccount.status && typeof issuerAccount.status === 'object'
    ? Object.keys(issuerAccount.status)[0].toLowerCase() === 'approved'
    : String(issuerAccount.status).toLowerCase().includes('approved');
  if (!isApproved) {
    throw new Error(
      `issuer did not reach Approved status; final=${JSON.stringify(issuerAccount.status)}`,
    );
  }

  // ─── 8b. ADR-0014: append_issuer_leaf ──────────────────────────────
  //
  // Enrols the approved issuer into the singleton issuer tree so their
  // leaf is present at the root `verify_batch_proof` will read from
  // `IssuerTreeBinding`.  Requires the SPL AC tree itself to exist
  // (created by `scripts/backfill_issuer_tree.ts` or by an operator
  // before running this script); `SOLID_ISSUER_TREE_PUBKEY` must be
  // set.
  const issuerTreePkRaw = state.issuerMerkleTreeAddress
    ? new PublicKey(state.issuerMerkleTreeAddress)
    : null;
  if (!issuerTreePkRaw || issuerTreePkRaw.equals(PublicKey.default)) {
    console.log(
      '\n[8b/10] append_issuer_leaf: SKIPPED -- issuer tree not yet bound.\n' +
      '         Run scripts/backfill_issuer_tree.ts first (or set\n' +
      '         SOLID_ISSUER_TREE_PUBKEY and re-run initialize.ts).',
    );
  } else if (issuerAccount.isTreeEnrolled) {
    console.log('\n[8b/10] append_issuer_leaf: SKIPPED -- already enrolled');
  } else {
    console.log('\n[8b/10] append_issuer_leaf');
    const [issuerTreeBindingPda] = PublicKey.findProgramAddressSync(
      [Buffer.from('issuer-tree-binding')],
      PROGRAM_PUBKEYS.issuerRegistry,
    );
    const [issuerTreeAuthorityPda] = PublicKey.findProgramAddressSync(
      [Buffer.from('issuer-tree-authority')],
      PROGRAM_PUBKEYS.issuerRegistry,
    );
    // SOLID-SEC-059 / H1 (2026-04-30, atomic + integrity-checked):
    // `appendIssuerLeaf` updates the binding atomically via on-chain
    // Poseidon-Merkle recompute against `poseidon_proof_path`.  Caller
    // must supply the Poseidon siblings the new leaf would have AT its
    // assigned index.  Off-chain we replay the issuer tree locally
    // (LocalReplicaAdapter is Poseidon-hashed, matching the in-circuit
    // MerkleInclusion template).
    const refreshed: any = await (issuerProgram.account as any)
      .issuerAccount.fetch(issuerAccountPda);
    const newIssuerLeaf = computeIssuerLeaf(
      issuerAuthority.publicKey.toBytes(),
      issuerBjj.public_key_x,
      issuerBjj.public_key_y,
      BigInt(refreshed.statusEpoch.toString()),
      BigInt(refreshed.revocationNonce.toString()),
    );
    const treeDepth = Number(state.issuerTreeDepth ?? 16);
    const replica = new LocalReplicaAdapter(treeDepth, poseidonHashPair);
    // Replay every currently-enrolled issuer in leaf-index order so the
    // replica matches on-chain SPL AC's leaf set; then append the new
    // leaf; then fetch the path against the new leaf.
    const allIssuers = await (issuerProgram.account as any).issuerAccount.all();
    const enrolled = allIssuers
      .map((e: any) => ({ pda: e.publicKey, acc: e.account }))
      .filter((e: any) => e.acc.isTreeEnrolled)
      .sort((a: any, b: any) =>
        Number(a.acc.issuerTreeLeafIndex) - Number(b.acc.issuerTreeLeafIndex),
      );
    for (const e of enrolled) {
      const l = computeIssuerLeaf(
        e.acc.authority.toBytes(),
        Uint8Array.from(e.acc.bjjPubKeyX),
        Uint8Array.from(e.acc.bjjPubKeyY),
        BigInt(e.acc.statusEpoch.toString()),
        BigInt(e.acc.revocationNonce.toString()),
      );
      replica.appendLeaf(l);
    }
    replica.appendLeaf(newIssuerLeaf);
    const proof = await replica.fetch(issuerTreePkRaw, newIssuerLeaf);
    // Flat Vec<u8> matching the on-chain expectation (16 * 32 bytes).
    const flatPath = new Uint8Array(treeDepth * 32);
    for (let i = 0; i < treeDepth; i++) {
      flatPath.set(proof.siblings[i], i * 32);
    }

    await issuerProgram.methods
      .appendIssuerLeaf(Buffer.from(flatPath))
      .accounts({
        registryConfig: registryConfigPda,
        issuerAccount: issuerAccountPda,
        issuerTreeAuthority: issuerTreeAuthorityPda,
        merkleTree: issuerTreePkRaw,
        issuerTreeBinding: issuerTreeBindingPda,
        logWrapper: new PublicKey('noopb9bkMVfRPU8AsbpTUg8AQkHtKwMYZiFUjNRtMmV'),
        compressionProgram: new PublicKey('cmtDvXumGCrqC1Age74AVPhSRVXJMd8PJS91L8KbNCK'),
        authority: wallet.publicKey,
      })
      .preInstructions([
        // SOLID-SEC-059 / H1: on-chain Poseidon-Merkle recompute over a
        // depth-16 path costs ~380K CU on BPF (each
        // `Fr::from_le_bytes_mod_order` canonicalisation is ~10-20K CU
        // and we do 32 of them).  800K leaves comfortable headroom
        // under the 1.4M per-tx ceiling.
        ComputeBudgetProgram.setComputeUnitLimit({ units: 800_000 }),
      ])
      .rpc();
    console.log(`   ok (leaf appended + binding root atomically updated; binding=${issuerTreeBindingPda.toBase58()})`);
  }

  // ─── 9. persist state ─────────────────────────────────────────────
  console.log('\n[9/10] persisting state');
  state.governanceMint = governanceMint.toBase58();
  state.registryConfigPda = registryConfigPda.toBase58();
  state.issuerAccountPda = issuerAccountPda.toBase58();
  state.stakeVaultPda = stakeVaultPda.toBase58();
  state.issuerBjj = {
    private_key: Array.from(issuerBjj.private_key),
    public_key_x: Array.from(issuerBjj.public_key_x),
    public_key_y: Array.from(issuerBjj.public_key_y),
  };
  state.issuerAuthoritySecret = Array.from(issuerAuthority.secretKey);
  state.issuerAuthorityPubkey = issuerAuthority.publicKey.toBase58();
  writeState(state);

  console.log('\nDone. Summary:');
  console.log(`  issuer_account     : ${issuerAccountPda.toBase58()}`);
  console.log(`  issuer_authority   : ${issuerAuthority.publicKey.toBase58()}`);
  console.log(`  stake_vault        : ${stakeVaultPda.toBase58()}`);
  console.log(`  governance_mint    : ${governanceMint.toBase58()}`);
}

main().catch(e => {
  console.error(e);
  process.exit(1);
});
