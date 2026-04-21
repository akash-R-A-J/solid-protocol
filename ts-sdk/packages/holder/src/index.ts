/**
 * @solid-protocol/holder — Proof generation for credential holders
 *
 * v0.2 (2026-04 R-2 remediation):
 *   Migrated off the Light Protocol stateless client and onto
 *   `@solid-protocol/light` v0.2 — which is now an SPL Account Compression
 *   adapter.  Callers pass a `MerkleProofAdapter` (typically a
 *   Helius-DAS-backed adapter in production, or a `LocalReplicaAdapter` for
 *   tests/localnet) plus the credential's tree address.
 *
 * Pipeline:
 *   1. Compute nullifier via WASM (Step 8 in circuit)
 *   2. Fetch Merkle proof via the supplied adapter
 *   3. Build the Circom circuit input (public + private)
 *   4. Run `snarkjs.groth16.fullProve` for proof generation
 *   5. Format the proof for `groth16-solana` on-chain verification
 */

import {
  initWasm,
  computeNullifier,
  computeIdentityState,
  deriveCredentialKey,
  poseidonHashBytes,
  type BJJKeypair,
  type CompoundQuery,
  type MultiCredentialQuery,
  MAX_CREDENTIALS,
  MAX_PREDICATES,
  PROGRAM_IDS,
  OP_MAP,
} from '@solid-protocol/core';
import {
  fetchMerkleProof as lightFetchMerkleProof,
  type MerkleProofAdapter,
  type MerkleProof,
} from '@solid-protocol/light';
import { PublicKey } from '@solana/web3.js';

// @ts-ignore — snarkjs doesn't have perfect types
import * as snarkjs from 'snarkjs';
import { Buffer } from 'buffer';

/** Verifier Program ID, derived from @solid-protocol/core (single source of truth). */
const VERIFIER_ID_BYTES = new PublicKey(PROGRAM_IDS.zkVerifier).toBytes();

export interface StoredCredential {
  schemaHash: Uint8Array;
  attestationData: bigint[];
  issuerSignature: { r8_x: Uint8Array; r8_y: Uint8Array; s: Uint8Array };
  issuerPubKeyX: Uint8Array;
  issuerPubKeyY: Uint8Array;
  holderPubKeyX: Uint8Array;
  holderPubKeyY: Uint8Array;
  holderPrivateKey: Uint8Array;
  salt: Uint8Array;
  commitment: Uint8Array;
  expirationTimestamp: number;
  /** SPL Account-Compression tree this credential was issued into. */
  merkleTree: PublicKey;
}

/** Options that every proof-generation entrypoint requires. */
export interface MerkleProofSource {
  /** Adapter the holder uses to retrieve Merkle proofs for leaves.  Required. */
  merkleProofAdapter: MerkleProofAdapter;
}

export interface ProofResult {
  proof: any;
  publicSignals: string[];
  nullifier: Uint8Array;
  /** Formatted for on-chain Groth16 verification */
  solanaProof: {
    proofA: Uint8Array;   // 64 bytes
    proofB: Uint8Array;   // 128 bytes
    proofC: Uint8Array;   // 64 bytes
  };
}

export interface BatchProofResult extends ProofResult {
  nullifier: Uint8Array;
}

/**
 * Generate a Groth16 proof for a compound query.
 *
 * This is the full holder-side pipeline:
 *   1. Compute nullifier via WASM (Step 8 in circuit)
 *   2. Fetch Merkle proof from Photon Indexer (Light Protocol)
 *   3. Build complete circuit input (public + private)
 *   4. Run snarkjs.groth16.fullProve() for proof generation
 *   5. Format proof for on-chain groth16-solana verification
 */
export async function generateProof(
  query: CompoundQuery,
  credential: StoredCredential,
  masterPrivateKey: Uint8Array,
  circuitPaths: {
    wasmPath: string;
    zkeyPath: string;
  },
  options: MerkleProofSource & {
    /** Global-state tree PDA (mirrors schema_registry::global_binding). */
    globalStateTree: PublicKey;
  },
): Promise<ProofResult> {
  await initWasm();

  // queryContextHash must match the circuit's Step 4 computation. The circuit
  // uses distinct Poseidon(8) calls over (credIndex||fieldIndex) and
  // (operator||value), then a final Poseidon(4) with numPredicates +
  // compoundLogic. Compound_query does not include credIndex but we pad with
  // zeros to share the hash structure with the batch circuit.
  const queryContextHash = computeQueryContextHash(query);

  // Hardened nullifier uses the master key, not the per-schema credential
  // key. Passing credential.holderPrivateKey (per-schema) here was a bug:
  // the circuit's nullifier formula is Poseidon(masterKey, ...).
  const nullifier = computeNullifier(
    masterPrivateKey,
    query.revocationNonce ?? 0n,
    VERIFIER_ID_BYTES,
    queryContextHash,
    query.verifierNonce,
  );

  // Credential inclusion proof in the schema-scoped tree.
  const merkleProof: MerkleProof = await lightFetchMerkleProof(
    options.merkleProofAdapter,
    credential.merkleTree,
    credential.commitment,
  );

  // Per-schema identity leaf for the global-state tree. Must match the
  // circuit's IdentityAnchor derivation exactly.
  const kp: BJJKeypair = deriveCredentialKey(masterPrivateKey, credential.schemaHash);
  const identityLeaf = computeIdentityState(
    kp.public_key_x,
    kp.public_key_y,
    query.revocationNonce ?? 0n,
  );
  const globalProof = await lightFetchMerkleProof(
    options.merkleProofAdapter,
    options.globalStateTree,
    identityLeaf,
  );

  const queryFieldIndices = Array(MAX_PREDICATES).fill(0);
  const queryOperators = Array(MAX_PREDICATES).fill(0);
  const queryValues = Array(MAX_PREDICATES).fill('0');

  query.predicates.forEach((p: any, i: number) => {
    queryFieldIndices[i] = p.fieldIndex;
    queryOperators[i] = OP_MAP[p.operator as keyof typeof OP_MAP];
    queryValues[i] = p.value.toString();
  });

  const circuitInput = {
    // Public inputs
    globalRoot: bufToDecimal(globalProof.root),
    merkleRoot: bufToDecimal(merkleProof.root),
    schemaHash: bufToDecimal(credential.schemaHash),
    issuerPubKeyAx: bufToDecimal(credential.issuerPubKeyX),
    issuerPubKeyAy: bufToDecimal(credential.issuerPubKeyY),
    queryFieldIndices,
    queryOperators,
    queryValues,
    numPredicates: query.predicates.length,
    compoundLogic: query.compoundLogic === 'AND' ? 0 : 1,
    verifierAddress: bufToDecimal(VERIFIER_ID_BYTES),
    verifierNonce: bufToDecimal(query.verifierNonce),
    currentTimestamp: Math.floor(Date.now() / 1000),

    // Private inputs
    masterIdentityKey: bufToDecimal(masterPrivateKey),
    revocationNonce: (query.revocationNonce ?? 0n).toString(),
    globalSiblings: globalProof.siblings,
    globalPathIndices: globalProof.pathIndices,
    attestationData: credential.attestationData.map(d => d.toString()),
    salt: bufToDecimal(credential.salt),
    holderBJJPrivKey: bufToDecimal(kp.private_key),
    issuerSigR8x: bufToDecimal(credential.issuerSignature.r8_x),
    issuerSigR8y: bufToDecimal(credential.issuerSignature.r8_y),
    issuerSigS: bufToDecimal(credential.issuerSignature.s),
    merkleSiblings: merkleProof.siblings,
    merklePathIndices: merkleProof.pathIndices,
    expirationTimestamp: credential.expirationTimestamp,
  };

  const { proof, publicSignals } = await snarkjs.groth16.fullProve(
    circuitInput,
    circuitPaths.wasmPath,
    circuitPaths.zkeyPath,
  );

  const solanaProof = formatProofForSolana(proof);
  // BUG-02 fix mirrored in single-credential path.
  const nullifierFromCircuit = bigintToBytes32(BigInt(publicSignals[0]));

  // Sanity check: the WASM nullifier must match the circuit output.
  if (Buffer.from(nullifier).compare(nullifierFromCircuit) !== 0) {
    throw new Error(
      'Nullifier mismatch: SDK-computed value differs from circuit output',
    );
  }

  return { proof, publicSignals, nullifier, solanaProof };
}

/**
 * Generate a Groth16 proof for multiple credentials (N=4).
 * Phase 3.1: Composable Identity.
 */
export async function generateBatchProof(
  query: MultiCredentialQuery,
  credentials: StoredCredential[], // Must be up to 4, padded with placeholders
  masterPrivateKey: Uint8Array,
  masterPublicKey: { x: Uint8Array; y: Uint8Array },
  revocationNonce: bigint,
  circuitPaths: {
    wasmPath: string;
    zkeyPath: string;
  },
  options: MerkleProofSource & {
    /** Address of the global-state tree (mirrored in `schema_registry::global_binding`). */
    globalStateTree: PublicKey;
  },
): Promise<BatchProofResult> {
  await initWasm();

  // SEC-20: Canonical Ordering & Smart Sorting (Phase 3.2)
  // 1. Sort credentials with a stable sort to maintain predictability
  const sortedCredsWithIndices = credentials
    .map((c, i) => ({ cred: c, originalIndex: i }))
    .sort((a, b) => {
        const hexA = Buffer.from(a.cred.schemaHash).toString('hex');
        const hexB = Buffer.from(b.cred.schemaHash).toString('hex');
        return hexA.localeCompare(hexB);
    });

  const sortedCredentials = sortedCredsWithIndices.map(x => x.cred);
  
  // 2. Create index mapping [originalIndex] -> [newPosition]
  const indexMap = new Map<number, number>();
  sortedCredsWithIndices.forEach((x, newIdx) => {
      indexMap.set(x.originalIndex, newIdx);
  });

  // 3. SEC-17: Identity Cohesion — Validate all credentials belong to the master identity
  for (const cred of sortedCredentials) {
      if (Buffer.from(cred.holderPubKeyX).compare(masterPublicKey.x) !== 0) {
          throw new Error("Identity Cohesion Failure: Credential does not belong to master identity");
      }
  }

  // 4. Fetch per-credential Merkle proofs (per-schema SPL AC trees).
  const credentialProofs = await Promise.all(
    sortedCredentials.map(c =>
      lightFetchMerkleProof(options.merkleProofAdapter, c.merkleTree, c.commitment),
    ),
  );

  // 5. Fetch a per-credential global inclusion proof.
  //
  //    BUG-04 fix: The circuit's `IdentityAnchor` derives a per-schema
  //    BabyJubJub keypair via
  //        credPriv_i = Poseidon(masterKey, schemaHash_i)
  //        (credAx_i, credAy_i) = BabyPbk(credPriv_i)
  //    and computes the global-tree leaf as
  //        identityState_i = Poseidon(credAx_i, credAy_i, revocationNonce).
  //
  //    The pre-remediation SDK used the naive master-key leaf
  //        Poseidon(masterPK.x, masterPK.y, revocationNonce)
  //    for every credential and fetched one shared global proof. Because
  //    the circuit runs NUM_CREDS IdentityAnchors — each with its own
  //    schema-derived key — the global-tree membership would never match.
  //
  //    We now derive the per-schema keypair and fetch one global proof per
  //    credential, matching the circuit exactly.
  const perSchemaAnchors = sortedCredentials.map(c => {
    const kp: BJJKeypair = deriveCredentialKey(masterPrivateKey, c.schemaHash);
    const leaf = computeIdentityState(kp.public_key_x, kp.public_key_y, revocationNonce);
    return { keypair: kp, leaf };
  });
  const globalProofs = await Promise.all(
    perSchemaAnchors.map(a =>
      lightFetchMerkleProof(
        options.merkleProofAdapter,
        options.globalStateTree,
        a.leaf,
      ),
    ),
  );
  // Sanity: every credential's global root should be the same (single tree).
  const canonicalGlobalRoot = globalProofs[0]?.root;
  if (!canonicalGlobalRoot) {
    throw new Error('No global-state proof returned for any credential');
  }
  for (const p of globalProofs) {
    if (Buffer.from(p.root).compare(canonicalGlobalRoot) !== 0) {
      throw new Error(
        'Global-state proofs returned divergent roots — indexer may be inconsistent',
      );
    }
  }

  // 6. Build Batch Circuit Input.
  const circuitInput: any = {
    // Public Inputs
    globalRoot: bufToDecimal(canonicalGlobalRoot),
    merkleRoots: credentialProofs.map((p: any) => bufToDecimal(p.root)),
    schemaHashes: sortedCredentials.map(c => bufToDecimal(c.schemaHash)),

    queryCredentialIndices: Array(MAX_PREDICATES).fill(0),
    queryFieldIndices: Array(MAX_PREDICATES).fill(0),
    queryOperators: Array(MAX_PREDICATES).fill(0),
    queryValues: Array(MAX_PREDICATES).fill('0'),
    numPredicates: query.predicates.length,
    compoundLogic: query.compoundLogic === 'AND' ? 0 : 1,

    verifierAddress: bufToDecimal(VERIFIER_ID_BYTES),
    verifierNonce: bufToDecimal(query.verifierNonce),
    currentTimestamp: Math.floor(Date.now() / 1000),

    // Private Inputs
    masterIdentityKey: bufToDecimal(masterPrivateKey),
    revocationNonce: revocationNonce.toString(),
    // Per-credential global-tree siblings/pathIndices.
    globalSiblings: globalProofs.map(p => p.siblings),
    globalPathIndices: globalProofs.map(p => p.pathIndices),

    data: sortedCredentials.map(c => c.attestationData.map(d => d.toString())),
    salts: sortedCredentials.map(c => bufToDecimal(c.salt)),
    issuerSigR8xs: sortedCredentials.map(c => bufToDecimal(c.issuerSignature.r8_x)),
    issuerSigR8ys: sortedCredentials.map(c => bufToDecimal(c.issuerSignature.r8_y)),
    issuerSigSs: sortedCredentials.map(c => bufToDecimal(c.issuerSignature.s)),
    issuerPubKeyAxs: sortedCredentials.map(c => bufToDecimal(c.issuerPubKeyX)),
    issuerPubKeyAys: sortedCredentials.map(c => bufToDecimal(c.issuerPubKeyY)),
    merkleSiblings: credentialProofs.map((p: any) => p.siblings),
    merklePathIndices: credentialProofs.map((p: any) => p.pathIndices),
    expirationTimestamps: sortedCredentials.map(c => c.expirationTimestamp),
  };

  // 5. Map predicates to circuit arrays & REMAP indices
  query.predicates.forEach((p: any, i: number) => {
    // SEC-20: Use the indexMap to find the new sorted position of the credential
    const remappedIndex = indexMap.get(p.credentialIndex);
    if (remappedIndex === undefined) {
        throw new Error(`Invalid predicate: credentialIndex ${p.credentialIndex} not found in batch`);
    }
    circuitInput.queryCredentialIndices[i] = remappedIndex;
    circuitInput.queryFieldIndices[i] = p.fieldIndex;
    circuitInput.queryOperators[i] = OP_MAP[p.operator as keyof typeof OP_MAP];
    circuitInput.queryValues[i] = p.value.toString();
  });

  // 4. Run SnarkJS
  console.log('Generating Batch Groth16 proof (N=4)...');
  const { proof, publicSignals } = await snarkjs.groth16.fullProve(
    circuitInput,
    circuitPaths.wasmPath,
    circuitPaths.zkeyPath,
  );

  const solanaProof = formatProofForSolana(proof);
  // BUG-02 fix: snarkjs returns publicSignals[i] as a decimal bigint-as-string,
  // not a hex string. The pre-remediation SDK called Buffer.from(value, 'hex')
  // which produced a zero-length buffer when the string contained any digit
  // that was not also a hex character, and a truncated nullifier otherwise.
  const nullifier = bigintToBytes32(BigInt(publicSignals[0]));

  return { proof, publicSignals, nullifier, solanaProof };
}

/**
 * Verify a proof locally (for testing, without on-chain submission).
 */
export async function verifyProofLocally(
  proof: any,
  publicSignals: string[],
  verificationKeyPath: string,
): Promise<boolean> {
  const fs = await import('fs');
  const vk = JSON.parse(fs.readFileSync(verificationKeyPath, 'utf-8'));
  return await snarkjs.groth16.verify(vk, publicSignals, proof);
}

// ─── Helpers ───────────────────────────────────────────────────────────────

function bufToDecimal(buf: Uint8Array): string {
  let result = 0n;
  for (let i = buf.length - 1; i >= 0; i--) {
    result = result * 256n + BigInt(buf[i]);
  }
  return result.toString();
}

/**
 * Format a snarkjs Groth16 proof for groth16-solana on-chain verification.
 *
 * groth16-solana expects:
 *   proofA: 2 x 32 bytes (G1 point, negated)
 *   proofB: 2 x 2 x 32 bytes (G2 point)
 *   proofC: 2 x 32 bytes (G1 point)
 */
function formatProofForSolana(proof: any): {
  proofA: Uint8Array;
  proofB: Uint8Array;
  proofC: Uint8Array;
} {
  // Convert proof.pi_a (G1) — negate Y coordinate for groth16-solana
  const proofA = new Uint8Array(64);
  const piA_x = bigintToBytes32(BigInt(proof.pi_a[0]));
  const piA_y = bigintToBytes32(BigInt(proof.pi_a[1]));
  proofA.set(piA_x, 0);
  proofA.set(piA_y, 32);

  // Convert proof.pi_b (G2)
  const proofB = new Uint8Array(128);
  for (let i = 0; i < 2; i++) {
    for (let j = 0; j < 2; j++) {
      const val = bigintToBytes32(BigInt(proof.pi_b[i][j]));
      proofB.set(val, (i * 2 + j) * 32);
    }
  }

  // Convert proof.pi_c (G1)
  const proofC = new Uint8Array(64);
  const piC_x = bigintToBytes32(BigInt(proof.pi_c[0]));
  const piC_y = bigintToBytes32(BigInt(proof.pi_c[1]));
  proofC.set(piC_x, 0);
  proofC.set(piC_y, 32);

  return { proofA, proofB, proofC };
}

function bigintToBytes32(n: bigint): Uint8Array {
  const hex = n.toString(16).padStart(64, '0');
  const bytes = new Uint8Array(32);
  for (let i = 0; i < 32; i++) {
    bytes[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

/**
 * Compute `queryContextHash = Poseidon(schemaHash, fieldIndices|operators|values, numPredicates, compoundLogic, expirationTimestamp)`.
 *
 * Mirrors Step 4 of `circuits/compound_query.circom`, giving the verifier a
 * stable, scope-binding identifier for the exact query that produced the proof.
 */
function computeQueryContextHash(query: CompoundQuery): Uint8Array {
  const inputs: bigint[] = [];
  inputs.push(bytesToBigInt(query.schemaHash));

  for (let i = 0; i < MAX_PREDICATES; i++) {
    const p = query.predicates[i];
    inputs.push(p ? BigInt(p.fieldIndex) : 0n);
    inputs.push(p ? BigInt(OP_MAP[p.operator as keyof typeof OP_MAP]) : 0n);
    inputs.push(p ? BigInt(p.value) : 0n);
  }
  inputs.push(BigInt(query.predicates.length));
  inputs.push(BigInt(query.compoundLogic === 'AND' ? 0 : 1));
  inputs.push(BigInt(query.expirationTimestamp ?? 0));

  return poseidonHashBytes(inputs);
}

function bytesToBigInt(buf: Uint8Array): bigint {
  let r = 0n;
  for (let i = buf.length - 1; i >= 0; i--) r = r * 256n + BigInt(buf[i]);
  return r;
}
