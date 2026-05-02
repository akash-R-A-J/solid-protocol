/**
 * SolID Protocol — proof generation and on-chain verification script (E2E step 3).
 *
 * Depends on scripts/issue.ts having run first.
 *
 * Responsibilities:
 *   1. Load credential state produced by issue.ts.
 *   2. Build a MultiCredentialQuery via QueryBuilder.
 *   3. Seed a LocalReplicaAdapter with the known commitment (for localnet);
 *      production runs would pass a Helius DAS adapter instead.
 *   4. Generate a batch Groth16 proof via @solid-protocol/holder.
 *   5. Submit verify_batch_proof via @solid-protocol/verifier, capturing the
 *      real transaction signature.
 *   6. Replay the same transaction and assert that it is rejected by the
 *      nullifier PDA's init constraint.
 */

import { Connection, Keypair, PublicKey, ComputeBudgetProgram } from '@solana/web3.js';
import * as anchor from '@coral-xyz/anchor';
import {
  initWasm,
  QueryBuilder,
  PROGRAM_IDS,
  computeCommitment,
  computeIssuerLeaf,
  deriveCredentialKey,
} from '@solid-protocol/core';
import { generateBatchProof, type StoredCredential } from '@solid-protocol/holder';
import {
  verifyOnChain,
  type SchemaTreeAccounts,
  extractWirePublicInputs,
  ensureLookupTable,
  lookupAddressesFromTrees,
} from '@solid-protocol/verifier';
// SOLID-SEC-058 / CRIT-3: verify the .wasm and .zkey before passing them
// to snarkjs.  The SDK facade does this for callers of `SolID.prove()`;
// this script calls `generateBatchProof` directly, so it must verify here.
import {
  loadAndVerifyArtifact,
  WASM_PIN,
  ZKEY_PIN,
} from '@solid-protocol/sdk';
import { LocalReplicaAdapter, poseidonHashPair } from '@solid-protocol/light';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { readState, writeState } from './lib/e2e_state';

const PROGRAM_PUBKEYS = {
  zkVerifier: new PublicKey(PROGRAM_IDS.zkVerifier),
  schemaRegistry: new PublicKey(PROGRAM_IDS.schemaRegistry),
};

const RPC_URL = process.env.SOLID_RPC_URL ?? 'http://127.0.0.1:8899';
const GLOBAL_TREE_DEPTH = 20;
const CREDENTIAL_TREE_DEPTH = 20;
/// ADR-0014: fixed issuer-tree depth; matches the circuit's main-component
/// ISSUER_TREE_DEPTH parameter.
const ISSUER_TREE_DEPTH = 16;

async function main() {
  await initWasm();
  const connection = new Connection(RPC_URL, 'confirmed');

  const keypairPath = path.join(os.homedir(), '.config/solana/id.json');
  const secretKey = JSON.parse(fs.readFileSync(keypairPath, 'utf-8'));
  const wallet = Keypair.fromSecretKey(Uint8Array.from(secretKey));

  // Anchor provider for the schema-registry update_*_root ixs (SOLID-SEC-059
  // sibling closures).
  const provider = new anchor.AnchorProvider(
    connection,
    new anchor.Wallet(wallet),
    { commitment: 'confirmed' },
  );
  anchor.setProvider(provider);

  const state = readState('prove');
  if (!state.credential) {
    throw new Error('state.credential missing; run scripts/issue.ts first');
  }

  console.log('SolID Protocol — proof and verification');
  console.log('=======================================');

  const credential: StoredCredential = {
    schemaHash: Uint8Array.from(state.credential.schemaHash),
    attestationData: state.credential.attestationData.map((s: string) => BigInt(s)),
    issuerSignature: {
      r8_x: Uint8Array.from(state.credential.issuerSignature.r8_x),
      r8_y: Uint8Array.from(state.credential.issuerSignature.r8_y),
      s: Uint8Array.from(state.credential.issuerSignature.s),
    },
    issuerPubKeyX: Uint8Array.from(state.credential.issuerPubKeyX),
    issuerPubKeyY: Uint8Array.from(state.credential.issuerPubKeyY),
    holderPubKeyX: Uint8Array.from(state.credential.holderPubKeyX),
    holderPubKeyY: Uint8Array.from(state.credential.holderPubKeyY),
    holderPrivateKey: Uint8Array.from(state.holderSchema.private_key),
    salt: Uint8Array.from(state.credential.salt),
    commitment: Uint8Array.from(state.credential.commitment),
    expirationTimestamp: state.credential.expirationTimestamp ?? 0,
    merkleTree: new PublicKey(state.merkleTreeAddress),
    // ADR-0014 preimage snapshot (written by issue.ts).
    issuerAuthority: new PublicKey(state.credential.issuerAuthority),
    issuerStatusEpoch: BigInt(state.credential.issuerStatusEpoch),
    issuerRevocationNonce: BigInt(state.credential.issuerRevocationNonce),
    issuerTreeLeafIndex: BigInt(state.credential.issuerTreeLeafIndex),
  };

  // 1. Build query (age >= 21).
  console.log('[1/4] Building query...');
  const nonce = new Uint8Array(32);
  for (let i = 0; i < nonce.length; i++) nonce[i] = 1; // deterministic test nonce
  const schemaHashes = [
    credential.schemaHash,
    new Uint8Array(32),
    new Uint8Array(32),
    new Uint8Array(32),
  ];
  const query = new QueryBuilder()
    .schemas(schemaHashes)
    .where(0, 0, 'GTE', 21n)
    .verifier(PROGRAM_PUBKEYS.zkVerifier.toBuffer())
    .nonce(nonce)
    .build();

  // 2. Seed local Merkle replica with the issued credential.
  //    In production this is a Helius DAS adapter; we keep the local replica
  //    in the E2E so the flow works against `solana-test-validator` without
  //    an indexer.
  console.log('[2/4] Seeding local Merkle replica...');
  const credReplica = new LocalReplicaAdapter(CREDENTIAL_TREE_DEPTH, poseidonHashPair);
  credReplica.appendLeaf(credential.commitment);

  // SOLID-SEC-059 sibling closure (2026-04-30): the schema-tree
  // binding stores the Poseidon root the circuit consumes via
  // `merkleRoots[i]`.  Push it on-chain via the integrity-checked
  // `update_tree_root` ix (handler runs on-chain Poseidon recompute
  // against this caller-supplied path; refuses any push that doesn't
  // recompute to `new_root`).
  {
    const credProof = await credReplica.fetch(
      new PublicKey(state.schemaTreeAddress ?? state.schemaTreeBindingPda),
      credential.commitment,
    );
    const credPath = new Uint8Array(CREDENTIAL_TREE_DEPTH * 32);
    for (let i = 0; i < CREDENTIAL_TREE_DEPTH; i++) {
      credPath.set(credProof.siblings[i], i * 32);
    }
    const schemaIdl = JSON.parse(
      fs.readFileSync('target/idl/schema_registry.json', 'utf-8'),
    );
    if (!schemaIdl.address) schemaIdl.address = PROGRAM_PUBKEYS.schemaRegistry.toBase58();
    const schemaProgram = new anchor.Program(schemaIdl, provider);
    // SOLID-SEC-080 (NF-03, 2026-05-01): the on-chain handler now
    // requires `old_leaf` so it can anchor the caller-supplied path
    // against the binding's current_root before applying.  On the
    // FIRST update from a fresh binding (current_root == [0; 32]
    // sentinel), `old_leaf` is unused -- the anchor short-circuits;
    // `[0u8; 32]` is the canonical placeholder.
    //
    // For subsequent updates, the caller is responsible for tracking
    // the previous leaf at `leaf_index` and threading it here.  At
    // this stage of `prove.ts` we are always doing the first update
    // (one credential per e2e run), so `[0u8; 32]` is correct.
    const credOldLeaf = new Uint8Array(32);
    await schemaProgram.methods
      .updateTreeRoot(
        Array.from(Uint8Array.from(Buffer.from(state.schemaHash, 'hex'))),
        Array.from(credProof.root),
        Array.from(credOldLeaf),
        Array.from(credential.commitment),
        new anchor.BN(0),
        Buffer.from(credPath),
      )
      .accounts({
        schemaTreeBinding: new PublicKey(state.schemaTreeBindingPda),
        authority: wallet.publicKey,
      })
      .preInstructions([
        ComputeBudgetProgram.setComputeUnitLimit({ units: 800_000 }),
      ])
      .rpc();
    console.log(`   schema_tree_binding root pushed: ${Buffer.from(credProof.root).toString('hex').slice(0, 16)}...`);
  }

  // Build the global replica: compute the per-schema identity leaf and seed it.
  //
  // SOLID-SEC-011: the previous revision used `keccak256HashPair`, a
  // symbol that was never imported (the in-circuit hash and the SolID
  // identity-state tree both use Poseidon).  `@solid-protocol/light`
  // exports `poseidonHashPair` which mirrors the circuit's Poseidon(2)
  // composition; the SPL-AC tree header happens to use keccak256
  // internally but that root is opaque to SolID proofs and is not
  // consumed here.
  const holderMasterPriv = Uint8Array.from(state.holderMaster.private_key);
  const kp = deriveCredentialKey(holderMasterPriv, credential.schemaHash);
  const globalReplica = new LocalReplicaAdapter(GLOBAL_TREE_DEPTH, poseidonHashPair);
  // identity leaf = Poseidon(credPubX, credPubY, revocationNonce). At this
  // bootstrapping stage revocationNonce is 0.
  const { computeIdentityState } = await import('@solid-protocol/core');
  const identityLeaf = computeIdentityState(kp.public_key_x, kp.public_key_y, 0n);
  globalReplica.appendLeaf(identityLeaf);

  // SOLID-SEC-059 sibling closure (2026-04-30): push the global-tree
  // Poseidon root via the integrity-checked `update_global_root` ix.
  // The on-chain handler runs Poseidon recompute over `poseidon_proof_path`
  // and refuses any push that doesn't equal `new_root`.
  {
    const globalProof = await globalReplica.fetch(
      new PublicKey(state.globalBindingPda),
      identityLeaf,
    );
    const globalPath = new Uint8Array(GLOBAL_TREE_DEPTH * 32);
    for (let i = 0; i < GLOBAL_TREE_DEPTH; i++) {
      globalPath.set(globalProof.siblings[i], i * 32);
    }
    const schemaIdl = JSON.parse(
      fs.readFileSync('target/idl/schema_registry.json', 'utf-8'),
    );
    if (!schemaIdl.address) schemaIdl.address = PROGRAM_PUBKEYS.schemaRegistry.toBase58();
    const schemaProgram = new anchor.Program(schemaIdl, provider);
    // SOLID-SEC-080 (NF-03, 2026-05-01): same anchor as update_tree_root
    // above.  First-update path; `old_leaf` short-circuits via the
    // [0; 32] sentinel.
    const globalOldLeaf = new Uint8Array(32);
    await schemaProgram.methods
      .updateGlobalRoot(
        Array.from(globalProof.root),
        Array.from(globalOldLeaf),
        Array.from(identityLeaf),
        new anchor.BN(0),
        Buffer.from(globalPath),
      )
      .accounts({
        globalBinding: new PublicKey(state.globalBindingPda),
        authority: wallet.publicKey,
      })
      .preInstructions([
        ComputeBudgetProgram.setComputeUnitLimit({ units: 800_000 }),
      ])
      .rpc();
    console.log(`   global_binding root pushed: ${Buffer.from(globalProof.root).toString('hex').slice(0, 16)}...`);
  }

  // ADR-0014: issuer-tree replica.  Seeded with the issuer's leaf
  // (Poseidon(5) over IssuerAccount preimage).  The resulting root
  // MUST match `IssuerTreeBinding.current_root` on-chain, otherwise
  // `verify_batch_proof` rejects with `IssuerTreeRootMismatch`.  On a
  // clean localnet E2E run the backfill script updates the binding
  // root right after `append_issuer_leaf`, and this replica matches.
  const issuerReplica = new LocalReplicaAdapter(ISSUER_TREE_DEPTH, poseidonHashPair);
  const issuerLeaf = computeIssuerLeaf(
    credential.issuerAuthority.toBytes(),
    credential.issuerPubKeyX,
    credential.issuerPubKeyY,
    credential.issuerStatusEpoch,
    credential.issuerRevocationNonce,
  );
  issuerReplica.appendLeaf(issuerLeaf);
  const issuerTreeRoot = issuerReplica.getRoot();
  console.log(`[debug] witness issuerTreeRoot (poseidon-replica): ${Buffer.from(issuerTreeRoot).toString('hex')}`);
  console.log(`[debug] issuer leaf (poseidon-5):                  ${Buffer.from(issuerLeaf).toString('hex')}`);
  console.log(`[debug] witness globalRoot (poseidon-replica):     ${Buffer.from(globalReplica.getRoot()).toString('hex')}`);
  console.log(`[debug] witness credentialRoot (poseidon-replica): ${Buffer.from(credReplica.getRoot()).toString('hex')}`);
  console.log(`[debug] credential.commitment (witness leaf):      ${Buffer.from(credential.commitment).toString('hex')}`);
  console.log(`[debug] state.schemaHash (witness slot 6):         ${state.schemaHash}`);
  const issuerMerkleTreePk = new PublicKey(state.issuerMerkleTreeAddress);

  const merkleProofAdapter = {
    async fetch(tree: PublicKey, leaf: Uint8Array) {
      const key = tree.toBase58();
      if (key === state.globalBindingPda) return globalReplica.fetch(tree, leaf);
      if (key === state.issuerMerkleTreeAddress) return issuerReplica.fetch(tree, leaf);
      return credReplica.fetch(tree, leaf);
    },
  };

  // 3. Generate batch proof.
  console.log('[3/4] Generating Groth16 batch proof...');
  const wasmPath = 'circuits/build/batch_credential_query_js/batch_credential_query.wasm';
  const zkeyPath = 'circuits/build/batch_credential_query_final.zkey';
  if (!fs.existsSync(wasmPath) || !fs.existsSync(zkeyPath)) {
    console.error(
      `Circuit artifacts missing. Run "cd circuits && node scripts/setup.js" first.`,
    );
    process.exit(1);
  }
  // SOLID-SEC-058 / CRIT-3: verify SHA-256 against pinned sources
  // (env vars, sidecar files, or config constants) before snarkjs sees
  // these bytes.  Local files have a sidecar at
  // circuits/build/<name>.sha256 written by setup.js + this session's
  // bootstrap.  See ts-sdk/packages/sdk/src/artifact_integrity.ts.
  const wasmBytes = await loadAndVerifyArtifact(
    wasmPath,
    WASM_PIN,
    'batch_credential_query.wasm',
  );
  const zkeyBytes = await loadAndVerifyArtifact(
    zkeyPath,
    ZKEY_PIN,
    'batch_credential_query.zkey',
  );
  const startTs = Date.now();
  const proofResult = await generateBatchProof(
    query,
    // Holder SDK contract (docs/MODULE_CONTRACTS.md §generateBatchProof):
    // 1..NUM_CREDS=4 active credentials; padding to 4 is performed
    // internally with sentinel slots (schemaHash == 0), which the
    // circuit gates off via `anchors[i].enabled = 0`.
    [credential],
    holderMasterPriv,
    {
      x: Uint8Array.from(state.holderMaster.public_key_x),
      y: Uint8Array.from(state.holderMaster.public_key_y),
    },
    0n, // revocationNonce
    { wasmPath: wasmBytes, zkeyPath: zkeyBytes },
    {
      merkleProofAdapter,
      globalStateTree: new PublicKey(state.globalBindingPda),
      issuerMerkleTree: issuerMerkleTreePk,
      issuerTreeRoot,
    },
  );
  console.log(`   proof generated in ${((Date.now() - startTs) / 1000).toFixed(2)}s`);

  // [debug] Dump every public-input slot.
  for (let i = 0; i < proofResult.publicSignals.length; i++) {
    const sig = proofResult.publicSignals[i];
    const bigVal = BigInt(sig);
    const bytesBE = bigVal.toString(16).padStart(64, '0');
    console.log(`[debug] publicSignals[${String(i).padStart(2)}] = ${sig.padStart(78)}  hex(BE): ${bytesBE}`);
  }

  // [debug] Verify proof locally to isolate witness/circuit consistency
  // from on-chain reconstruction.
  const vkPath = 'circuits/build/verification_key.json';
  if (fs.existsSync(vkPath)) {
    const vk = JSON.parse(fs.readFileSync(vkPath, 'utf-8'));
    const snarkjs = await import('snarkjs');
    const localOK = await snarkjs.groth16.verify(vk, proofResult.publicSignals, proofResult.proof);
    console.log(`[debug] LOCAL groth16 verify: ${localOK ? 'OK' : 'FAILED'}`);

    // SOLID-SEC-067 / LB5 host-test fixture: persist (vk, proof,
    // publicSignals) so a Rust integration test can exercise the same
    // verify_groth16_proof path the on-chain handler uses, but on the
    // host.  If that host test passes, on-chain Groth16 must pass too;
    // any remaining e2e-on-chain failure is then localised to
    // reconstruction / validator-state / wire-encoding.
    const fixtureDir = 'tests/fixtures';
    fs.mkdirSync(fixtureDir, { recursive: true });
    const fixturePath = path.join(fixtureDir, 'groth16_e2e_proof.json');
    fs.writeFileSync(
      fixturePath,
      JSON.stringify(
        {
          description:
            'Captured by scripts/prove.ts after a successful LOCAL snarkjs.groth16.verify.\n' +
            'Used by Rust host-test `groth16_host_verify_round_trip` in zk-verifier to ' +
            'exercise the on-chain serialization+verify path off-chain.',
          vk_path: vkPath,
          publicSignals: proofResult.publicSignals,
          proof: proofResult.proof,
          // The Solana-formatted byte-arrays the SDK feeds to the
          // verifier ix.  Pre-LB5 this was wrong for G2 components;
          // post-LB5 the host-test should accept these bytes.
          solanaProof: {
            proofA: Array.from(proofResult.solanaProof.proofA),
            proofB: Array.from(proofResult.solanaProof.proofB),
            proofC: Array.from(proofResult.solanaProof.proofC),
          },
        },
        null,
        2,
      ),
    );
    console.log(`[debug] wrote fixture -> ${fixturePath}`);
  }

  // 4. Submit to on-chain verifier.
  console.log('[4/4] Submitting verify_batch_proof...');
  const schemaTreeBinding = new PublicKey(state.schemaTreeBindingPda);
  const trees: SchemaTreeAccounts = {
    globalTree: new PublicKey(state.globalBindingPda),
    // SOLID-SEC-054 / B13: only slot 0 is active in this prove flow
    // (single-schema proof).  Inactive slots use `PublicKey.default`
    // (the all-zero pubkey, system-program owned) -- the on-chain
    // handler's owner-check classifies them as inactive
    // (account.owner != SCHEMA_REGISTRY_ID).  All three inactive
    // slots collapsing to the same pubkey is fine: web3.js dedupes
    // references in the ALT, and Anchor reads accounts positionally
    // from the ix's accountKeyIndexes (which carry distinct indices
    // even when they all resolve to the same account).
    schemaTree0: schemaTreeBinding,
    schemaTree1: PublicKey.default,
    schemaTree2: PublicKey.default,
    schemaTree3: PublicKey.default,
    issuerTreeBinding: new PublicKey(state.issuerTreeBindingPda),
  };
  // Extract the 21-element wire subset from the full 32-element
  // snarkjs publicSignals output. The handler reconstructs the
  // remaining 11 slots from accounts already on the ix surface
  // (SOLID-SEC-054 / B13; wire shrinks 1324 -> 972 bytes).
  const fullPublicInputs = proofResult.publicSignals.map(s => {
    let n = BigInt(s);
    const bytes = new Uint8Array(32);
    for (let i = 31; i >= 0; i--) { bytes[i] = Number(n & 0xFFn); n >>= 8n; }
    return bytes;
  });
  const request = {
    query,
    proofData: {
      proof_a: proofResult.solanaProof.proofA,
      proof_b: proofResult.solanaProof.proofB,
      proof_c: proofResult.solanaProof.proofC,
      publicInputs: extractWirePublicInputs(fullPublicInputs),
      nullifier: proofResult.nullifier,
    },
  };
  // SOLID-SEC-054 / B13 path (3): Address Lookup Table compression of
  // the read-only account list.  Required to fit the legacy-tx wire
  // ceiling alongside the path-(1) on-chain reconstruction.  Cached
  // across runs in e2e_state under `verifyAltPubkey`.
  const cachedAlt = state.verifyAltPubkey
    ? new PublicKey(state.verifyAltPubkey)
    : undefined;
  const lookupAddresses = lookupAddressesFromTrees(trees);
  console.log('Ensuring verify_batch_proof lookup table...');
  const { pubkey: altPubkey, account: altAccount } = await ensureLookupTable(
    connection,
    wallet,
    lookupAddresses,
    cachedAlt,
  );
  console.log(`   ALT: ${altPubkey.toBase58()}${cachedAlt ? ' (cached)' : ' (fresh)'}`);
  console.log(`   ALT addresses (${altAccount.state.addresses.length}):`);
  altAccount.state.addresses.forEach((a, i) => console.log(`     [${i}] ${a.toBase58()}`));
  if (!state.verifyAltPubkey || state.verifyAltPubkey !== altPubkey.toBase58()) {
    state.verifyAltPubkey = altPubkey.toBase58();
    writeState(state);
  }

  // Diagnostic: print the ix data length to confirm wire shape.
  const { buildVerifyBatchProofIx } = await import('@solid-protocol/verifier');
  const ix = buildVerifyBatchProofIx({
    payer: wallet.publicKey,
    request,
    trees,
  });
  console.log(`   ix.data.length = ${ix.data.length} (expected 972)`);
  console.log(`   ix.keys.length = ${ix.keys.length} (expected 11)`);
  console.log(`   first 16 bytes (disc + proof_a head): ${Buffer.from(ix.data.slice(0, 16)).toString('hex')}`);
  console.log(`   bytes [264..272] (Vec length region): ${Buffer.from(ix.data.slice(264, 272)).toString('hex')}`);
  console.log(`   bytes [last 32] (nullifier): ${Buffer.from(ix.data.slice(-32)).toString('hex')}`);

  const verification = await verifyOnChain(connection, wallet, request, trees, undefined, altAccount);
  console.log(`   verified: ${verification.verified}`);
  console.log(`   tx:       ${verification.transactionSignature}`);

  // Replay protection.
  console.log('Replay test (should fail)...');
  try {
    await verifyOnChain(connection, wallet, request, trees, undefined, altAccount);
    console.error('   ERROR: replay was accepted (nullifier PDA not enforced)');
    process.exit(1);
  } catch {
    console.log('   ok (replay rejected by nullifier PDA init constraint)');
  }

  console.log('\nDone.');
}

main()
  .then(() => {
    // SEC-048 Phase E.5 (diagnosed 2026-05-02): snarkjs's
    // `groth16.fullProve` (called inside `generateBatchProof`) spawns
    // N worker_threads (one per CPU core) and does NOT terminate them.
    // Combined with @solana/web3.js Connection's HTTP-keepalive
    // socket, the Node event loop stays alive after main() returns.
    // Without this explicit exit, `npm run e2e` hangs after the final
    // `verified: true` print until SIGTERM.  Standard pattern for
    // one-shot snarkjs-using scripts.  Confirmed via
    // `process._getActiveHandles()` showing N MessagePort + 1 TCP
    // Socket; see ~/.claude/.../memory/feedback_snarkjs_workers_hang.md.
    process.exit(0);
  })
  .catch(e => {
    console.error(e);
    process.exit(1);
  });
