/**
 * SolID Protocol — on-chain initialization script.
 *
 * Bootstraps the three programs on a freshly deployed cluster. Safe to re-run:
 * every step is idempotent and ignores "already initialised" failures.
 *
 * Steps:
 *   1. initialize_registry (issuer-registry)
 *   2. register_schema (schema-registry)
 *   3. initialize_tree_binding (schema-registry) -- skipped if
 *      SOLID_TREE_PUBKEY unset; deferred to bootstrap_schema_tree.ts.
 *   4. initialize_global_binding (schema-registry)
 *   5. initialize_issuer_tree_binding (issuer-registry, ADR-0014) --
 *      skipped if SOLID_ISSUER_TREE_PUBKEY unset; deferred to
 *      backfill_issuer_tree.ts.
 *   6. initialize (zk-verifier)
 *   7. store_verification_key in chunks (zk-verifier)
 *
 * Init-only contracts (skip-when-unset rationale):
 *   - schema-tree-binding's `tree_pubkey` is immutable after init
 *     (programs/schema-registry/src/lib.rs:189).
 *   - issuer-tree-binding's `tree_pubkey` is immutable after init
 *     (ADR-0014).
 *   Hence we never call these instructions with `Pubkey::default()`
 *   as a placeholder -- doing so would permanently brick the
 *   binding.  Either supply the real pubkey via env, or skip and
 *   let the dedicated bootstrap script create-and-bind in one shot.
 *
 * Program IDs are sourced from @solid-protocol/core::PROGRAM_IDS, the single
 * source of truth mirrored in Anchor.toml and enforced by CI via
 * scripts/check_program_ids.py.
 */

import { Connection, Keypair, PublicKey, SystemProgram, ComputeBudgetProgram } from '@solana/web3.js';
import * as anchor from '@coral-xyz/anchor';
import * as crypto from 'crypto';
import { createMint, TOKEN_PROGRAM_ID } from '@solana/spl-token';
import { initWasm, computeSchemaHash, PROGRAM_IDS } from '@solid-protocol/core';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { invalidateState, readStateOrNull, stateFilePath, writeState } from './lib/e2e_state';

const PROGRAM_PUBKEYS = {
  schemaRegistry: new PublicKey(PROGRAM_IDS.schemaRegistry),
  zkVerifier: new PublicKey(PROGRAM_IDS.zkVerifier),
  issuerRegistry: new PublicKey(PROGRAM_IDS.issuerRegistry),
};

const RPC_URL = process.env.SOLID_RPC_URL ?? 'http://127.0.0.1:8899';
const SCHEMA_NAME = 'basic_identity_v1';
const SCHEMA_VERSION = 1;
const SCHEMA_FIELDS = [
  'age', 'country_code', 'region', 'id_type',
  'verification_level', 'issued_date', 'nationality', '_reserved',
];

const keypairPath = path.join(os.homedir(), '.config/solana/id.json');
const secretKey = JSON.parse(fs.readFileSync(keypairPath, 'utf-8'));
const wallet = Keypair.fromSecretKey(Uint8Array.from(secretKey));
const connection = new Connection(RPC_URL, 'confirmed');

const provider = new anchor.AnchorProvider(
  connection,
  new anchor.Wallet(wallet),
  { commitment: 'confirmed' },
);
anchor.setProvider(provider);

function fieldToBytesBE(s: string): Uint8Array {
  let n = BigInt(s);
  const bytes = new Uint8Array(32);
  for (let i = 31; i >= 0; i--) {
    bytes[i] = Number(n & 0xFFn);
    n >>= 8n;
  }
  return bytes;
}

function serializeG1(point: string[]): Uint8Array {
  const buf = new Uint8Array(64);
  buf.set(fieldToBytesBE(point[0]), 0);
  buf.set(fieldToBytesBE(point[1]), 32);
  return buf;
}

function serializeG2(point: string[][]): Uint8Array {
  // LB5 / SOLID-SEC-067 (closed 2026-04-30): snarkjs JSON dumps G2
  // points as `[[c0_real, c1_imag], [c0_real, c1_imag], [1, 0]]` --
  // i.e. (real, imag) ordering for each F_q² coefficient.  Solana's
  // alt_bn128 syscall (which groth16-solana wraps for verify) decodes
  // each F_q² element in (imag, real) order: bytes [0..32] = x_imag,
  // [32..64] = x_real, [64..96] = y_imag, [96..128] = y_real.  The
  // pre-fix serialiser used (real, imag) ordering -- 50% chance of
  // landing on a valid-but-different curve point, 50% chance of
  // off-curve, and either way the on-chain Groth16 pairing rejected
  // every proof.  Latent because no proof had reached the on-chain
  // verify path until SOLID-SEC-054 / B13 Option 2 closed the
  // wire-size cap.
  const buf = new Uint8Array(128);
  buf.set(fieldToBytesBE(point[0][1]), 0);   // x_imag
  buf.set(fieldToBytesBE(point[0][0]), 32);  // x_real
  buf.set(fieldToBytesBE(point[1][1]), 64);  // y_imag
  buf.set(fieldToBytesBE(point[1][0]), 96);  // y_real
  return buf;
}

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

function isAlreadyInitialised(e: any): boolean {
  const m = String(e?.message ?? e);
  return m.includes('already in use') || m.includes('already initialized');
}

async function main() {
  console.log('SolID Protocol on-chain initialization');
  console.log('======================================');
  console.log(`Wallet: ${wallet.publicKey.toBase58()}`);
  console.log(`RPC:    ${RPC_URL}`);

  const issuerIdl = loadIdl('issuerRegistry');
  const schemaIdl = loadIdl('schemaRegistry');
  const zkIdl = loadIdl('zkVerifier');

  const issuerProgram = new anchor.Program(issuerIdl, provider);
  const schemaProgram = new anchor.Program(schemaIdl, provider);
  const zkProgram = new anchor.Program(zkIdl, provider);

  await initWasm();

  // 1. Initialize issuer registry.
  //
  // Contract (post 2026-04-26 redesign — see programs/issuer-registry/src/lib.rs
  // and docs/E2E_BLOCKERS.md B10):
  //
  //   `initialize_registry` requires a typed SPL `Mint` account
  //   (`governance_mint`) and atomically births the singleton governance
  //   vault TokenAccount under the PDA `["governance-vault", registry_config]`.
  //   The previous shape — a free `governance_token_mint: Pubkey` parameter
  //   with no validation — let scripts pass `Pubkey::default()`, which then
  //   poisoned every downstream guard that compared a vault's mint against
  //   `registry.governance_token_mint`.  That bug is fixed at the contract
  //   layer; this script must respect the new contract.
  //
  // Mint resolution order:
  //   1. `SOLID_GOVERNANCE_MINT` env override (operator opt-in).
  //   2. Cached `state.governanceMint` from a prior run.
  //   3. Fresh SPL mint created here (decimals=6, mint authority = wallet).
  //
  // The chosen mint is persisted to the E2E state file so downstream
  // scripts (bootstrap_issuer, etc.) can pick it up without re-creating.
  console.log('\n[1/8] initialize_registry');
  const [registryPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('registry-config')],
    PROGRAM_PUBKEYS.issuerRegistry,
  );
  const [governanceVaultPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('governance-vault'), registryPda.toBuffer()],
    PROGRAM_PUBKEYS.issuerRegistry,
  );

  const existingState = readStateOrNull() ?? {};
  let governanceMint: PublicKey;
  if (process.env.SOLID_GOVERNANCE_MINT) {
    governanceMint = new PublicKey(process.env.SOLID_GOVERNANCE_MINT);
    console.log(`   mint (env override) ${governanceMint.toBase58()}`);
  } else if (existingState.governanceMint) {
    governanceMint = new PublicKey(existingState.governanceMint);
    console.log(`   mint (cached state) ${governanceMint.toBase58()}`);
  } else {
    governanceMint = await createMint(
      connection,
      wallet,
      wallet.publicKey,
      null,
      6,
    );
    console.log(`   mint (fresh)        ${governanceMint.toBase58()}`);
  }

  // The voting period is the same parameter `bootstrap_issuer.ts` reads
  // from `SOLID_VOTING_PERIOD_SECONDS` (default 20s).  Keeping these two
  // entry points aligned is a hard E2E invariant: `initialize_registry`
  // runs first and writes the period to the registry; `bootstrap-issuer`
  // is then idempotent (its own `initialize_registry` becomes a no-op).
  // If the two ever disagree, finalize_voting fails with
  // VotingPeriodNotEnded after a real-time wait that is bounded by the
  // *first* writer.  86400s (1 day) is a production default; localnet
  // E2E always overrides via env (see scripts/init.sh / npm run e2e).
  const initVotingPeriodSeconds = Number(
    process.env.SOLID_VOTING_PERIOD_SECONDS ?? '86400',
  );
  const initMinStakeLamports = new anchor.BN(
    process.env.SOLID_MIN_STAKE_LAMPORTS ?? '1000000000',
  );
  try {
    await issuerProgram.methods.initializeRegistry(
      initMinStakeLamports,
      new anchor.BN(initVotingPeriodSeconds),
      new anchor.BN(6000),
    ).accounts({
      registryConfig: registryPda,
      governanceMint,
      governanceVault: governanceVaultPda,
      authority: wallet.publicKey,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
      rent: anchor.web3.SYSVAR_RENT_PUBKEY,
    }).rpc();
    console.log('   ok (initialised)');
  } catch (e: any) {
    if (!isAlreadyInitialised(e)) throw e;
    console.log('   ok (already active)');
  }

  // Reconcile against on-chain truth: the program is the source of truth.
  // If the registry already existed and was bound to a different mint,
  // adopt the on-chain value (a corrupt `Pubkey::default()` is no longer
  // representable under the new contract — `initialize_registry` would
  // have failed at deserialise — but we still defensively check).
  //
  // Stale-state self-heal (NF / 2026-05-01): use `fetchNullable` so we can
  // surface a clear error when `isAlreadyInitialised` matched (line 228)
  // but the registry PDA does not actually exist on-chain.  That state is
  // unreachable on a healthy run; it indicates the validator was
  // `solana-test-validator --reset` since the local state cache was
  // written.  Wipe the cache so the next run auto-recovers.
  const onChainConfig =
    await issuerProgram.account.registryConfig.fetchNullable(registryPda);
  if (onChainConfig === null) {
    const cleared = invalidateState();
    throw new Error(
      `initialize_registry: registry PDA ${registryPda.toBase58()} does not ` +
      `exist on-chain even though the catch path matched "already initialised".  ` +
      `This is the stale state-cache surface: validator was --reset but ` +
      `${cleared} was not.  Cache has been cleared; please re-run npm run e2e.`,
    );
  }
  const onChainMint = onChainConfig.governanceTokenMint as PublicKey;
  if (!onChainMint.equals(governanceMint)) {
    console.log(`   reconciling: local ${governanceMint.toBase58()} -> on-chain ${onChainMint.toBase58()}`);
    governanceMint = onChainMint;
  }
  existingState.governanceMint = governanceMint.toBase58();
  writeState(existingState);

  // 2. Register schema.
  //
  // SOLID-SEC-002 + SOLID-SEC-063 / H5: the schema_hash is computed by
  // `computeSchemaHash(name, version, field_names, category)` which is
  // a TypeScript mirror of `solid_core::schema::compute_schema_hash_from_parts`.
  // The preimage now binds field_names + category in addition to
  // (name, version, field_count); see CRITs above for the
  // collision-vulnerability that motivated the widening.
  // Cross-language vector coverage tracked in SOLID-SEC-010.
  console.log('\n[2/8] register_schema');
  const schemaHash = computeSchemaHash(
    SCHEMA_NAME,
    SCHEMA_VERSION,
    SCHEMA_FIELDS,
    'Identity',
  );
  console.log(`   schema_hash: ${Buffer.from(schemaHash).toString('hex')}`);
  const [schemaPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('schema'), Buffer.from(SCHEMA_NAME), Buffer.from([SCHEMA_VERSION])],
    PROGRAM_PUBKEYS.schemaRegistry,
  );
  try {
    // SOLID-SEC-063 / H5: the widened compute_schema_hash_from_parts
    // does a Poseidon-Merkle-Damgard absorb over name + field_names +
    // category.  On BPF each absorb round costs ~10K CU
    // (bytes_le_to_fr canonicalisation + fr_to_bytes_le + sol_poseidon
    // syscall), and the 8-field schema lands ~10 absorb rounds
    // (~100K CU) on top of the ~50K CU baseline.  Default per-ix
    // budget is 200K -- not enough headroom.  Prepend a
    // setComputeUnitLimit to give the handler 400K which is
    // ~2x the measured cost.
    await schemaProgram.methods.registerSchema(
      SCHEMA_NAME,
      SCHEMA_VERSION,
      'Identity',
      SCHEMA_FIELDS,
      Array.from(schemaHash),
    ).accounts({
      schemaAccount: schemaPda,
      authority: wallet.publicKey,
      systemProgram: SystemProgram.programId,
    })
      .preInstructions([
        ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 }),
      ])
      .rpc();
    console.log('   ok (registered)');
  } catch (e: any) {
    if (!isAlreadyInitialised(e)) throw e;
    console.log('   ok (already active)');
  }

  // 3. SchemaTreeBinding.
  //
  // Contract: `schema-registry::initialize_tree_binding` is INIT-ONLY
  // (programs/schema-registry/src/lib.rs:189) -- once created, the
  // `tree_pubkey` field is immutable.  Therefore we MUST NOT call this
  // ix with `PublicKey.default` as a placeholder: the binding would
  // be permanently dead and `issue_credential` would never succeed
  // against it.
  //
  // Instead we mirror the same opt-in pattern step 5 uses for the
  // issuer tree binding: only init when the caller has supplied a
  // real tree pubkey via `SOLID_TREE_PUBKEY`.  Otherwise we skip and
  // defer to `scripts/bootstrap_schema_tree.ts`, which creates the
  // SPL AC tree, transfers authority to the
  // `(b"tree-authority", schema_hash)` PDA, and only then calls
  // `initialize_tree_binding` with the real pubkey.
  console.log('\n[3/8] initialize_tree_binding');
  const [bindingPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('schema-tree-binding'), Buffer.from(schemaHash)],
    PROGRAM_PUBKEYS.schemaRegistry,
  );
  let treePubkey = PublicKey.default;
  if (process.env.SOLID_TREE_PUBKEY) {
    treePubkey = new PublicKey(process.env.SOLID_TREE_PUBKEY);
    try {
      await schemaProgram.methods.initializeTreeBinding(
        Array.from(schemaHash),
        treePubkey,
      ).accounts({
        schemaAccount: schemaPda,
        schemaTreeBinding: bindingPda,
        authority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      }).rpc();
      console.log(`   ok (binding at ${bindingPda.toBase58()})`);
    } catch (e: any) {
      if (!isAlreadyInitialised(e)) throw e;
      console.log('   ok (already active)');
    }
  } else {
    console.log(
      '   SKIPPED (SOLID_TREE_PUBKEY unset).  Run\n' +
      '   `tsx scripts/bootstrap_schema_tree.ts` next to create the\n' +
      '   SPL AC tree and bind it before credentials can be issued.',
    );
  }

  // 4. GlobalStateBinding.
  console.log('\n[4/8] initialize_global_binding (singleton)');
  const [globalBindingPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('global-binding')],
    PROGRAM_PUBKEYS.schemaRegistry,
  );
  try {
    await schemaProgram.methods.initializeGlobalBinding().accounts({
      globalBinding: globalBindingPda,
      authority: wallet.publicKey,
      systemProgram: SystemProgram.programId,
    }).rpc();
    console.log('   ok (global binding created)');
  } catch (e: any) {
    if (!isAlreadyInitialised(e)) throw e;
    console.log('   ok (already active)');
  }

  // 4b. ADR-0014: IssuerTreeBinding.
  //
  // Creates the singleton `IssuerTreeBinding` PDA under
  // `issuer-registry`.  The binding stores `tree_pubkey` IMMUTABLY
  // once initialised, so we only call `initialize_issuer_tree_binding`
  // here if the caller has supplied a real tree pubkey via
  // `SOLID_ISSUER_TREE_PUBKEY`.  Otherwise we skip and defer to
  // `scripts/backfill_issuer_tree.ts`, which creates the SPL AC tree
  // and calls this ix with the real pubkey.
  console.log('\n[5/8] initialize_issuer_tree_binding (ADR-0014)');
  const [issuerTreeBindingPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('issuer-tree-binding')],
    PROGRAM_PUBKEYS.issuerRegistry,
  );
  let issuerTreePubkey = PublicKey.default;
  if (process.env.SOLID_ISSUER_TREE_PUBKEY) {
    issuerTreePubkey = new PublicKey(process.env.SOLID_ISSUER_TREE_PUBKEY);
    try {
      await issuerProgram.methods.initializeIssuerTreeBinding(
        issuerTreePubkey,
      ).accounts({
        registryConfig: registryPda,
        issuerTreeBinding: issuerTreeBindingPda,
        authority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      }).rpc();
      console.log(`   ok (binding at ${issuerTreeBindingPda.toBase58()})`);
    } catch (e: any) {
      if (!isAlreadyInitialised(e)) throw e;
      console.log('   ok (already active)');
    }
  } else {
    console.log(
      '   SKIPPED (SOLID_ISSUER_TREE_PUBKEY unset).  Run\n' +
      '   `tsx scripts/backfill_issuer_tree.ts` next to create the\n' +
      '   SPL AC tree and bind it before proofs can verify.',
    );
  }

  // 5. Verifier config.
  console.log('\n[6/8] zk_verifier.initialize');
  const [verifierConfigPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('verifier-config')],
    PROGRAM_PUBKEYS.zkVerifier,
  );
  try {
    await zkProgram.methods.initialize().accounts({
      verifierConfig: verifierConfigPda,
      authority: wallet.publicKey,
      systemProgram: SystemProgram.programId,
    }).rpc();
    console.log('   ok (initialised)');
  } catch (e: any) {
    if (!isAlreadyInitialised(e)) throw e;
    console.log('   ok (already active)');
  }

  // 6. Upload verification key.
  console.log('\n[7/8] store_verification_key');
  const [vkStoragePda] = PublicKey.findProgramAddressSync(
    [Buffer.from('vk-storage'), verifierConfigPda.toBuffer()],
    PROGRAM_PUBKEYS.zkVerifier,
  );
  const vkJsonPath = 'circuits/build/verification_key.json';
  const vkSha256Path = 'circuits/build/verification_key.sha256';
  if (!fs.existsSync(vkJsonPath)) {
    console.error(
      `   verification_key.json missing at ${vkJsonPath}. ` +
      `Run "cd circuits && node scripts/setup.js" first.`,
    );
    process.exit(1);
  }

  // SOLID-SEC-041.  Refuse to upload a VK whose sha256 does not match
  // either the `SOLID_VK_SHA256` env var or the `verification_key.sha256`
  // file written by `circuits/scripts/setup.js`.  Catches:
  //   - stale `circuits/build/` after a `git pull` that rev'd the circuit
  //   - someone dropping a foreign VK into the build dir
  //   - operator running initialize.ts against a different branch than the
  //     one whose trusted setup was sanctioned.
  // The env var form is the authoritative path for published releases:
  // the canonical hash lives in the release notes / ADR, and the operator
  // sets `SOLID_VK_SHA256=<hex>` before running `npm run e2e`.  The file
  // form is a developer-loop convenience.
  const vkJsonBytes = fs.readFileSync(vkJsonPath);
  const computedVkSha256 = crypto.createHash('sha256').update(vkJsonBytes).digest('hex');
  const envPin = (process.env.SOLID_VK_SHA256 ?? '').trim().toLowerCase();
  const filePin = fs.existsSync(vkSha256Path)
    ? fs.readFileSync(vkSha256Path, 'utf-8').trim().toLowerCase()
    : '';
  const expected = envPin || filePin;
  if (!expected) {
    console.error(
      `   verification_key.sha256 missing at ${vkSha256Path} and SOLID_VK_SHA256 ` +
      `env var is unset. SOLID-SEC-041 requires one of the two: either re-run ` +
      `"cd circuits && node scripts/setup.js" to regenerate the pinned hash, ` +
      `or export the canonical hash published in the circuit-revision ADR as ` +
      `SOLID_VK_SHA256=<hex>.`,
    );
    process.exit(1);
  }
  if (computedVkSha256 !== expected) {
    console.error(
      `   VK sha256 mismatch (SOLID-SEC-041 gate).\n` +
      `     computed : ${computedVkSha256}\n` +
      `     expected : ${expected}\n` +
      `     source   : ${envPin ? 'SOLID_VK_SHA256 env var' : vkSha256Path}\n` +
      `   Refusing to upload.  Either the verification_key.json on disk is ` +
      `stale relative to the sanctioned circuit revision, or the pinned ` +
      `hash is out of date.  Re-run the trusted setup or update the pin.`,
    );
    process.exit(1);
  }
  console.log(
    `   VK sha256 gate: ok (${computedVkSha256.slice(0, 12)}...; ` +
    `pin source: ${envPin ? 'SOLID_VK_SHA256 env' : vkSha256Path})`,
  );

  // Idempotency guard: read verifier_config, skip the chunked upload
  // entirely if the VK is already finalized (post-vk_initialized=true,
  // post `finalize_verification_key`).  Catches the common
  // `ChunkOutOfOrder` failure when the script is re-run against a
  // validator that already has the VK on-chain (ledger persists across
  // a state-file wipe).  If the upload is partially done
  // (`next_vk_chunk > 0` but `vk_initialized=false`), refuse with an
  // actionable error rather than corrupt the partial state -- the
  // operator must `solana-test-validator --reset` (clean validator) or
  // wait for the prior session to recover.
  // SOLID-SEC-006 Part 1 / NF-02: finalize-if-pending.  Pre-fix this branch
  // returned as soon as `vk_initialized==true`, but never asserted
  // `vk_finalized==true`.  Result: every deployment ended with the freeze-
  // gate dead code (authority could replace the VK without the 48h timelock
  // by re-uploading chunk 0).  The branch now finalizes before returning.
  let batchVkAlreadyFinalized = false;
  try {
    const cfg = await zkProgram.account.verifierConfig.fetch(verifierConfigPda);
    if (cfg.vkInitialized) {
      if (!cfg.vkFinalized) {
        console.log('   VK uploaded by prior run but not finalized; calling finalize_verification_key (SOLID-SEC-006 Part 1)...');
        await zkProgram.methods.finalizeVerificationKey().accounts({
          verifierConfig: verifierConfigPda,
          authority: wallet.publicKey,
        }).rpc();
        const post = await zkProgram.account.verifierConfig.fetch(verifierConfigPda);
        if (!post.vkFinalized) {
          throw new Error('SOLID-SEC-006 Part 1: finalize_verification_key returned but vk_finalized is still false.');
        }
        console.log('   ok (VK finalized; freeze-gate active)');
      } else {
        console.log('   ok (VK already finalized on this validator; skipping upload)');
      }
      // Don't return here -- we still need to upload the subgroup VK
      // ([8/8] below), which has its own idempotency path.
      batchVkAlreadyFinalized = true;
    }
    if (cfg.nextVkChunk && cfg.nextVkChunk > 0 && !cfg.vkInitialized) {
      throw new Error(
        `verifier_config has next_vk_chunk=${cfg.nextVkChunk} but vk_initialized=false; ` +
        `a prior partial upload is on-chain.  Either restart with ` +
        `solana-test-validator --reset, or call finalize_verification_key from the ` +
        `chunk index where the prior run stopped.`,
      );
    }
  } catch (e: any) {
    // First-run path: verifier_config exists (just initialized in step 6) but
    // `fetch` throws if the account isn't decodable.  Continue to upload below.
    if (e?.message?.includes('Account does not exist') || e?.message?.includes('next_vk_chunk')) {
      throw e;
    }
    // Otherwise treat as "freshly created; no chunks yet" -- proceed.
  }

  if (!batchVkAlreadyFinalized) {
    const vkJson = JSON.parse(vkJsonBytes.toString('utf-8'));
    const icLen = vkJson.IC.length;
    const vkBytes = Buffer.concat([
      Buffer.from(Uint32Array.from([icLen]).buffer),
      Buffer.from(serializeG1(vkJson.vk_alpha_1)),
      Buffer.from(serializeG2(vkJson.vk_beta_2)),
      Buffer.from(serializeG2(vkJson.vk_gamma_2)),
      Buffer.from(serializeG2(vkJson.vk_delta_2)),
      Buffer.concat((vkJson.IC as string[][]).map((p: string[]) => Buffer.from(serializeG1(p)))),
    ]);
    const CHUNK_SIZE = 900;
    const totalChunks = Math.ceil(vkBytes.length / CHUNK_SIZE);
    for (let i = 0; i < totalChunks; i++) {
      const chunk = vkBytes.subarray(i * CHUNK_SIZE, (i + 1) * CHUNK_SIZE);
      // Anchor 0.30 IDL declares `chunk_data` as the `bytes` type, which
      // the BorshInstructionCoder encodes via `byteVec` and expects a
      // `Buffer` (not `number[]`).  Passing `Array.from(chunk)` triggers
      // `Blob.encode[data] requires (length N) Buffer as src` — the
      // diagnostic is misleading but the root cause is just the type.
      await zkProgram.methods.storeVerificationKey(
        i,
        Buffer.from(chunk),
        i === totalChunks - 1,
      ).accounts({
        verifierConfig: verifierConfigPda,
        vkStorage: vkStoragePda,
        authority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      }).rpc();
      process.stdout.write(`   uploaded chunk ${i + 1}/${totalChunks}\r`);
    }
    console.log(`\n   ok (${vkBytes.length} bytes)`);

    // SOLID-SEC-006 Part 1 / NF-02 (closed 2026-05-01).  Freeze the VK so
    // any further write must go through the 48h-timelock path
    // (`request_vk_rotation` -> wait -> `rotate_verification_key`).
    // Pre-fix: this call was missing; every deployment ended with
    // `vk_finalized=false` and `store_verification_key` would happily
    // accept a fresh chunk-0 from the authority, bypassing the SEC-006
    // freeze-gate entirely.
    console.log('   finalizing VK (SOLID-SEC-006 Part 1)...');
    await zkProgram.methods.finalizeVerificationKey().accounts({
      verifierConfig: verifierConfigPda,
      authority: wallet.publicKey,
    }).rpc();
    const cfgAfterFinalize = await zkProgram.account.verifierConfig.fetch(verifierConfigPda);
    if (!cfgAfterFinalize.vkFinalized) {
      throw new Error(
        'SOLID-SEC-006 Part 1 post-condition failed: finalize_verification_key returned ' +
        'but verifier_config.vk_finalized is still false.  Aborting initialize.',
      );
    }
    console.log(`   ok (vk_finalized=true; rotation now requires request_vk_rotation + 48h timelock)`);
  }

  // ─── [8/8] subgroup VK upload (SEC-048 Phase E.4) ─────────────────────
  //
  // The subgroup verifier lives on the `issuer-registry` program (not
  // zk-verifier).  Mirrors the batch VK pipeline 1:1 with separate
  // PDAs (`subgroup-verifier-config` + `subgroup-vk-storage`) and the
  // 48h timelock from ADR-0015.
  console.log('\n[8/8] init_subgroup_verifier + store_subgroup_vk_chunk + finalize_subgroup_vk');
  const subgroupVkJsonPath = 'circuits/build/bjj_subgroup_verification_key.json';
  const subgroupVkSha256Path = 'circuits/build/bjj_subgroup_verification_key.sha256';
  if (!fs.existsSync(subgroupVkJsonPath)) {
    console.error(
      `   bjj_subgroup_verification_key.json missing at ${subgroupVkJsonPath}. ` +
      `Run "cd circuits && node scripts/setup.js --circuit bjj_subgroup_proof" first.`,
    );
    process.exit(1);
  }
  // SOLID-SEC-041 mirror for the subgroup VK.  Same env > sidecar
  // priority order; refusal mode is identical.
  const subgroupVkJsonBytes = fs.readFileSync(subgroupVkJsonPath);
  const computedSubgroupVkSha256 = crypto
    .createHash('sha256')
    .update(subgroupVkJsonBytes)
    .digest('hex');
  const envSubgroupPin = (process.env.SOLID_SUBGROUP_VK_SHA256 ?? '')
    .trim()
    .toLowerCase();
  const fileSubgroupPin = fs.existsSync(subgroupVkSha256Path)
    ? fs.readFileSync(subgroupVkSha256Path, 'utf-8').trim().toLowerCase()
    : '';
  const expectedSubgroup = envSubgroupPin || fileSubgroupPin;
  if (!expectedSubgroup) {
    console.error(
      `   bjj_subgroup_verification_key.sha256 missing at ${subgroupVkSha256Path} ` +
      `and SOLID_SUBGROUP_VK_SHA256 env var is unset.  SOLID-SEC-041 ` +
      `(SEC-048 Phase E.4 mirror) requires one of the two: re-run "cd ` +
      `circuits && node scripts/setup.js --circuit bjj_subgroup_proof", ` +
      `or export SOLID_SUBGROUP_VK_SHA256=<hex>.`,
    );
    process.exit(1);
  }
  if (computedSubgroupVkSha256 !== expectedSubgroup) {
    console.error(
      `   subgroup VK sha256 mismatch (SOLID-SEC-041 / SEC-048 Phase E.4 gate).\n` +
      `     computed : ${computedSubgroupVkSha256}\n` +
      `     expected : ${expectedSubgroup}\n` +
      `     source   : ${envSubgroupPin ? 'SOLID_SUBGROUP_VK_SHA256 env var' : subgroupVkSha256Path}\n` +
      `   Refusing to upload.  Either the bjj_subgroup_verification_key.json ` +
      `is stale, or the pinned hash is out of date.`,
    );
    process.exit(1);
  }
  console.log(
    `   subgroup VK sha256 gate: ok (${computedSubgroupVkSha256.slice(0, 12)}...; ` +
    `pin source: ${envSubgroupPin ? 'SOLID_SUBGROUP_VK_SHA256 env' : subgroupVkSha256Path})`,
  );

  const [subgroupVerifierConfigPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('subgroup-verifier-config')],
    PROGRAM_PUBKEYS.issuerRegistry,
  );
  const [subgroupVkStoragePda] = PublicKey.findProgramAddressSync(
    [Buffer.from('subgroup-vk-storage'), subgroupVerifierConfigPda.toBuffer()],
    PROGRAM_PUBKEYS.issuerRegistry,
  );

  // a) init_subgroup_verifier (idempotent on AccountAlreadyInUse).
  try {
    await issuerProgram.methods.initSubgroupVerifier().accounts({
      subgroupVerifierConfig: subgroupVerifierConfigPda,
      authority: wallet.publicKey,
      systemProgram: SystemProgram.programId,
    }).rpc();
    console.log('   init_subgroup_verifier ok (initialised)');
  } catch (e: any) {
    if (!isAlreadyInitialised(e)) throw e;
    console.log('   init_subgroup_verifier ok (already active)');
  }

  // b) Idempotency: if the subgroup VK is already finalized, skip the
  // upload entirely.  Matches the batch-VK skip pattern.
  let subgroupVkAlreadyFinalized = false;
  try {
    const cfg = await issuerProgram.account.subgroupVerifierConfig.fetch(subgroupVerifierConfigPda);
    if (cfg.vkInitialized) {
      if (!cfg.vkFinalized) {
        console.log('   subgroup VK uploaded by prior run but not finalized; calling finalize_subgroup_vk...');
        await issuerProgram.methods.finalizeSubgroupVk().accounts({
          subgroupVerifierConfig: subgroupVerifierConfigPda,
          authority: wallet.publicKey,
        }).rpc();
        const post = await issuerProgram.account.subgroupVerifierConfig.fetch(subgroupVerifierConfigPda);
        if (!post.vkFinalized) {
          throw new Error('SEC-048 Phase E.4 post-condition failed: finalize_subgroup_vk returned but vk_finalized is still false.');
        }
        console.log('   ok (subgroup VK finalized; freeze-gate active)');
      } else {
        console.log('   ok (subgroup VK already finalized on this validator; skipping upload)');
      }
      subgroupVkAlreadyFinalized = true;
    } else if (cfg.nextVkChunk && cfg.nextVkChunk > 0) {
      throw new Error(
        `subgroup_verifier_config has next_vk_chunk=${cfg.nextVkChunk} but vk_initialized=false; ` +
        `a prior partial upload is on-chain.  Restart with solana-test-validator --reset.`,
      );
    }
  } catch (e: any) {
    if (e?.message?.includes('Account does not exist') || e?.message?.includes('next_vk_chunk')) {
      throw e;
    }
  }

  if (!subgroupVkAlreadyFinalized) {
    // c) Upload subgroup VK chunks.
    const subgroupVkJson = JSON.parse(subgroupVkJsonBytes.toString('utf-8'));
    const subgroupIcLen = subgroupVkJson.IC.length;
    const subgroupVkBytes = Buffer.concat([
      Buffer.from(Uint32Array.from([subgroupIcLen]).buffer),
      Buffer.from(serializeG1(subgroupVkJson.vk_alpha_1)),
      Buffer.from(serializeG2(subgroupVkJson.vk_beta_2)),
      Buffer.from(serializeG2(subgroupVkJson.vk_gamma_2)),
      Buffer.from(serializeG2(subgroupVkJson.vk_delta_2)),
      Buffer.concat(
        (subgroupVkJson.IC as string[][]).map((p: string[]) => Buffer.from(serializeG1(p))),
      ),
    ]);
    const CHUNK_SIZE = 900;
    const totalSubgroupChunks = Math.ceil(subgroupVkBytes.length / CHUNK_SIZE);
    for (let i = 0; i < totalSubgroupChunks; i++) {
      const chunk = subgroupVkBytes.subarray(i * CHUNK_SIZE, (i + 1) * CHUNK_SIZE);
      await issuerProgram.methods.storeSubgroupVkChunk(
        i,
        Buffer.from(chunk),
        i === totalSubgroupChunks - 1,
      ).accounts({
        subgroupVerifierConfig: subgroupVerifierConfigPda,
        subgroupVkStorage: subgroupVkStoragePda,
        authority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      }).rpc();
      process.stdout.write(`   uploaded subgroup chunk ${i + 1}/${totalSubgroupChunks}\r`);
    }
    console.log(`\n   ok (${subgroupVkBytes.length} subgroup VK bytes)`);

    // d) Finalize subgroup VK (SOLID-SEC-006 mirror).
    console.log('   finalizing subgroup VK (SEC-048 Phase E.4)...');
    await issuerProgram.methods.finalizeSubgroupVk().accounts({
      subgroupVerifierConfig: subgroupVerifierConfigPda,
      authority: wallet.publicKey,
    }).rpc();
    const cfgAfterFinalize = await issuerProgram.account.subgroupVerifierConfig.fetch(subgroupVerifierConfigPda);
    if (!cfgAfterFinalize.vkFinalized) {
      throw new Error(
        'SEC-048 Phase E.4 post-condition failed: finalize_subgroup_vk returned ' +
        'but subgroup_verifier_config.vk_finalized is still false.  Aborting initialize.',
      );
    }
    console.log(
      `   ok (subgroup vk_finalized=true; rotation now requires ` +
      `request_subgroup_vk_rotation + 48h timelock)`,
    );
  }

  // Persist state for downstream scripts.  Written under
  // `$XDG_RUNTIME_DIR` / `$TMPDIR` with mode 0600 (SOLID-SEC-020);
  // never under the repo working tree.
  //
  // Idempotency rule: tree pubkeys are written only when this run
  // actually bound them.  When the env var is unset (the localnet
  // E2E default), we PRESERVE whatever a prior `bootstrap_*.ts` run
  // wrote.  This keeps `npm run init-onchain` re-runnable mid-pipeline
  // without bricking downstream `issue.ts` / `prove.ts` calls that
  // depend on `state.merkleTreeAddress` being a real SPL AC tree.
  const prior = readStateOrNull() ?? {};
  const state: Record<string, any> = {
    ...prior,
    schemaName: SCHEMA_NAME,
    schemaVersion: SCHEMA_VERSION,
    schemaHash: Buffer.from(schemaHash).toString('hex'),
    schemaPda: schemaPda.toBase58(),
    registryPda: registryPda.toBase58(),
    governanceMint: governanceMint.toBase58(),
    governanceVaultPda: governanceVaultPda.toBase58(),
    schemaTreeBindingPda: bindingPda.toBase58(),
    globalBindingPda: globalBindingPda.toBase58(),
    issuerTreeBindingPda: issuerTreeBindingPda.toBase58(),
    verifierConfigPda: verifierConfigPda.toBase58(),
    vkStoragePda: vkStoragePda.toBase58(),
    // SEC-048 Phase E.4: subgroup VK PDAs.  bootstrap_issuer reads
    // these to wire `register_issuer`'s `subgroup_verifier_config` and
    // `subgroup_vk_storage` accounts.
    subgroupVerifierConfigPda: subgroupVerifierConfigPda.toBase58(),
    subgroupVkStoragePda: subgroupVkStoragePda.toBase58(),
  };
  if (process.env.SOLID_TREE_PUBKEY) {
    state.merkleTreeAddress = treePubkey.toBase58();
  } else if (!prior.merkleTreeAddress) {
    state.merkleTreeAddress = PublicKey.default.toBase58();
  }
  if (process.env.SOLID_ISSUER_TREE_PUBKEY) {
    state.issuerMerkleTreeAddress = issuerTreePubkey.toBase58();
  } else if (!prior.issuerMerkleTreeAddress) {
    state.issuerMerkleTreeAddress = PublicKey.default.toBase58();
  }
  const written = writeState(state);
  console.log(`\nWrote ${written}`);
  console.log(`(SOLID_E2E_STATE_FILE can override; default is ${stateFilePath()})`);
  console.log('Done.');
}

main().catch(e => {
  console.error(e);
  process.exit(1);
});
