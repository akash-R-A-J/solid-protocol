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
  computeIdentityState,
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
import { decodeIssuerTreeBinding } from '@solid-protocol/issuer/registry';
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
import * as path from 'path';
import { readState, writeState } from './lib/e2e_state';
import { loadKeypair } from './lib/keypair';
import {
  appendTrackedLeafHex,
  bytesToHex,
  flattenSiblings,
  hexToBytes32,
  planAppendToLiveRoot,
} from './lib/root_update_plan';

const PROGRAM_PUBKEYS = {
  zkVerifier: new PublicKey(PROGRAM_IDS.zkVerifier),
  schemaRegistry: new PublicKey(PROGRAM_IDS.schemaRegistry),
};

const RPC_URL = process.env.SOLID_RPC_URL ?? 'http://127.0.0.1:8899';
const GLOBAL_TREE_DEPTH = 20;
const CREDENTIAL_TREE_DEPTH = Number(process.env.SOLID_SCHEMA_TREE_DEPTH ?? '20');
/// ADR-0014: fixed issuer-tree depth; matches the circuit's main-component
/// ISSUER_TREE_DEPTH parameter.
const ISSUER_TREE_DEPTH = 16;
const ROOT_UPDATE_COMPUTE_UNITS = 1_200_000;

async function main() {
  await initWasm();
  const connection = new Connection(RPC_URL, 'confirmed');

  const wallet = loadKeypair();

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
  const credentialTreeDepth = Number(state.schemaTreeDepth ?? CREDENTIAL_TREE_DEPTH);
  const schemaIdl = JSON.parse(
    fs.readFileSync('target/idl/schema_registry.json', 'utf-8'),
  );
  if (!schemaIdl.address) schemaIdl.address = PROGRAM_PUBKEYS.schemaRegistry.toBase58();
  const schemaProgram = new anchor.Program(schemaIdl, provider);
  const schemaTreeBinding = new PublicKey(state.schemaTreeBindingPda);
  state.schemaCredentialLeaves = appendTrackedLeafHex(
    state.schemaCredentialLeaves,
    credential.commitment,
  );

  // SOLID-SEC-080 made root pushes live-root anchored.  Devnet runs are not
  // clean-slate: if this credential's root is already mirrored, skip the tx;
  // if the local state has pending leaves, append exactly the next one with
  // old_leaf=[0;32] at the correct live index.
  let liveSchemaRoot = await readAccountRoot(connection, schemaTreeBinding, 72, 'SchemaTreeBinding');
  for (;;) {
    const plan = await planAppendToLiveRoot({
      depth: credentialTreeDepth,
      trackedLeafHexes: state.schemaCredentialLeaves,
      currentRoot: liveSchemaRoot,
    });
    if (plan.kind === 'skip') {
      console.log(`   schema_tree_binding root already current (${bytesToHex(liveSchemaRoot).slice(0, 16)}...)`);
      break;
    }
    await schemaProgram.methods
      .updateTreeRoot(
        Array.from(hexToBytes32(state.schemaHash, 'schemaHash')),
        Array.from(plan.newRoot),
        Array.from(plan.oldLeaf),
        Array.from(plan.newLeaf),
        new anchor.BN(plan.leafIndex),
        Buffer.from(flattenSiblings(plan.siblings, credentialTreeDepth)),
      )
      .accounts({
        schemaTreeBinding,
        authority: wallet.publicKey,
      })
      .preInstructions([
        ComputeBudgetProgram.setComputeUnitLimit({ units: ROOT_UPDATE_COMPUTE_UNITS }),
      ])
      .rpc();
    liveSchemaRoot = plan.newRoot;
    console.log(`   schema_tree_binding root pushed: ${bytesToHex(plan.newRoot).slice(0, 16)}... (leaf ${plan.leafIndex})`);
  }
  const credReplica = replicaFromHexLeaves(credentialTreeDepth, state.schemaCredentialLeaves);

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
  // identity leaf = Poseidon(credPubX, credPubY, revocationNonce). At this
  // bootstrapping stage revocationNonce is 0.
  const identityLeaf = computeIdentityState(kp.public_key_x, kp.public_key_y, 0n);
  const globalBinding = new PublicKey(state.globalBindingPda);
  state.globalIdentityLeaves = appendTrackedLeafHex(state.globalIdentityLeaves, identityLeaf);
  let liveGlobalRoot = await readAccountRoot(connection, globalBinding, 8, 'GlobalStateBinding');
  for (;;) {
    const plan = await planAppendToLiveRoot({
      depth: GLOBAL_TREE_DEPTH,
      trackedLeafHexes: state.globalIdentityLeaves,
      currentRoot: liveGlobalRoot,
    });
    if (plan.kind === 'skip') {
      console.log(`   global_binding root already current (${bytesToHex(liveGlobalRoot).slice(0, 16)}...)`);
      break;
    }
    await schemaProgram.methods
      .updateGlobalRoot(
        Array.from(plan.newRoot),
        Array.from(plan.oldLeaf),
        Array.from(plan.newLeaf),
        new anchor.BN(plan.leafIndex),
        Buffer.from(flattenSiblings(plan.siblings, GLOBAL_TREE_DEPTH)),
      )
      .accounts({
        globalBinding,
        authority: wallet.publicKey,
      })
      .preInstructions([
        ComputeBudgetProgram.setComputeUnitLimit({ units: ROOT_UPDATE_COMPUTE_UNITS }),
      ])
      .rpc();
    liveGlobalRoot = plan.newRoot;
    console.log(`   global_binding root pushed: ${bytesToHex(plan.newRoot).slice(0, 16)}... (leaf ${plan.leafIndex})`);
  }
  const globalReplica = replicaFromHexLeaves(GLOBAL_TREE_DEPTH, state.globalIdentityLeaves);
  writeState(state);

  // ADR-0014: issuer-tree replica.  Seeded with the issuer's leaf
  // (Poseidon(5) over IssuerAccount preimage).  The resulting root
  // MUST match `IssuerTreeBinding.current_root` on-chain, otherwise
  // `verify_batch_proof` rejects with `IssuerTreeRootMismatch`.  On a
  // clean localnet E2E run the backfill script updates the binding
  // root right after `append_issuer_leaf`, and this replica matches.
  const issuerIdl = JSON.parse(
    fs.readFileSync('target/idl/issuer_registry.json', 'utf-8'),
  );
  if (!issuerIdl.address) issuerIdl.address = PROGRAM_IDS.issuerRegistry;
  const issuerProgram = new anchor.Program(issuerIdl, provider);
  const issuerTreeDepth = Number(state.issuerTreeDepth ?? ISSUER_TREE_DEPTH);
  const issuerTreeBinding = new PublicKey(state.issuerTreeBindingPda);
  const issuerTreeBindingInfo = await connection.getAccountInfo(issuerTreeBinding, 'confirmed');
  if (!issuerTreeBindingInfo) throw new Error(`IssuerTreeBinding not found: ${issuerTreeBinding.toBase58()}`);
  const liveIssuerBinding = decodeIssuerTreeBinding(issuerTreeBindingInfo.data);
  const { replica: issuerReplica, root: issuerTreeRoot, count: issuerLeafCount } =
    await buildIssuerReplicaFromChain(issuerProgram, issuerTreeDepth);
  if (bytesToHex(issuerTreeRoot) !== liveIssuerBinding.currentRoot) {
    throw new Error(
      `Issuer tree replica root ${bytesToHex(issuerTreeRoot)} does not match live binding root ${liveIssuerBinding.currentRoot}. ` +
      'Re-run bootstrap_issuer/backfill or rebuild issuer-tree history before proving.',
    );
  }
  const issuerLeaf = computeIssuerLeaf(
    credential.issuerAuthority.toBytes(),
    credential.issuerPubKeyX,
    credential.issuerPubKeyY,
    credential.issuerStatusEpoch,
    credential.issuerRevocationNonce,
  );
  await issuerReplica.fetch(new PublicKey(state.issuerMerkleTreeAddress), issuerLeaf);
  console.log(`[debug] witness issuerTreeRoot (poseidon-replica): ${Buffer.from(issuerTreeRoot).toString('hex')}`);
  console.log(`[debug] issuer tree leaves replayed:              ${issuerLeafCount}`);
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

function replicaFromHexLeaves(depth: number, leaves: string[]): LocalReplicaAdapter {
  const replica = new LocalReplicaAdapter(depth, poseidonHashPair);
  for (const [index, leaf] of leaves.entries()) {
    replica.appendLeaf(hexToBytes32(leaf, `trackedLeaf[${index}]`));
  }
  return replica;
}

async function readAccountRoot(
  connection: Connection,
  pubkey: PublicKey,
  offset: number,
  label: string,
): Promise<Uint8Array> {
  const account = await connection.getAccountInfo(pubkey, 'confirmed');
  if (!account) throw new Error(`${label} not found: ${pubkey.toBase58()}`);
  if (account.data.length < offset + 32) {
    throw new Error(`${label} account too small: ${account.data.length} bytes`);
  }
  return Uint8Array.from(account.data.subarray(offset, offset + 32));
}

async function buildIssuerReplicaFromChain(
  issuerProgram: anchor.Program,
  depth: number,
): Promise<{ replica: LocalReplicaAdapter; root: Uint8Array; count: number }> {
  const replica = new LocalReplicaAdapter(depth, poseidonHashPair);
  const allIssuers = await (issuerProgram.account as any).issuerAccount.all();
  const enrolled = allIssuers
    .map((entry: any) => entry.account)
    .filter((account: any) => account.isTreeEnrolled)
    .sort((a: any, b: any) =>
      Number(a.issuerTreeLeafIndex.toString()) - Number(b.issuerTreeLeafIndex.toString()),
    );
  for (const account of enrolled) {
    replica.appendLeaf(computeIssuerLeaf(
      account.authority.toBytes(),
      Uint8Array.from(account.bjjPubKeyX),
      Uint8Array.from(account.bjjPubKeyY),
      BigInt(account.statusEpoch.toString()),
      BigInt(account.revocationNonce.toString()),
    ));
  }
  return { replica, root: replica.getRoot(), count: enrolled.length };
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
