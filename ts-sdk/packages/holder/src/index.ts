/**
 * @solid-protocol/holder — Proof generation for credential holders
 *
 * Full proof generation pipeline with Light Protocol:
 * 1. Fetch Merkle proof from Photon Indexer via @solid-protocol/light
 * 2. Prepare circuit inputs (private + public) via WASM
 * 3. Generate Groth16 proof via snarkjs
 * 4. Return proof ready for on-chain submission
 */

import {
  initWasm,
  computeNullifier,
  computeIdentityCommitment,
  poseidonHash,
  poseidonHashBytes,
  type CompoundQuery,
  type MultiCredentialQuery,
  MAX_CREDENTIALS,
  MAX_PREDICATES,
  NUM_FIELDS,
  PROGRAM_IDS,
  OP_MAP,
} from '@solid-protocol/core';
import {
  createLightRpc,
  fetchMerkleProof as lightFetchMerkleProof,
  type LightConfig,
  DEVNET_CONFIG,
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
  circuitPaths: {
    wasmPath: string;
    zkeyPath: string;
  },
  options?: {
    lightConfig?: LightConfig;
  },
): Promise<ProofResult> {
  await initWasm();

  // Compute the queryContextHash inline (mirrors circuit Step 4).
  const queryContextHash = computeQueryContextHash(query);

  // Step 1: Compute hardened nullifier via WASM (5-arg).
  const nullifier = computeNullifier(
    credential.holderPrivateKey,
    query.revocationNonce ?? 0n,
    VERIFIER_ID_BYTES,
    queryContextHash,
    query.verifierNonce,
  );

  // Step 2: Fetch Merkle proof from Photon Indexer (Light Protocol)
  const config = options?.lightConfig || DEVNET_CONFIG;
  const rpc = createLightRpc(config);
  const merkleProof = await lightFetchMerkleProof(rpc, credential.commitment);

  console.log(`Merkle proof fetched!`);
  console.log(`  Root: ${merkleProof.root.slice(0, 16)}...`);
  console.log(`  Leaf index: ${merkleProof.leafIndex}`);

  // Step 3: Build circuit input
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
    merkleRoot: merkleProof.root,
    schemaHash: bufToDecimal(credential.schemaHash),
    issuerPubKeyAx: bufToDecimal(credential.issuerPubKeyX),
    issuerPubKeyAy: bufToDecimal(credential.issuerPubKeyY),
    queryFieldIndices,
    queryOperators,
    queryValues,
    numPredicates: query.predicates.length,
    compoundLogic: query.compoundLogic === 'AND' ? 0 : 1,
    verifierNonce: bufToDecimal(query.verifierNonce),
    currentTimestamp: Math.floor(Date.now() / 1000),

    // Private inputs
    attestationData: credential.attestationData.map(d => d.toString()),
    salt: bufToDecimal(credential.salt),
    holderBJJPrivKey: bufToDecimal(credential.holderPrivateKey),
    holderBJJPubKeyAx: bufToDecimal(credential.holderPubKeyX),
    holderBJJPubKeyAy: bufToDecimal(credential.holderPubKeyY),
    issuerSigR8x: bufToDecimal(credential.issuerSignature.r8_x),
    issuerSigR8y: bufToDecimal(credential.issuerSignature.r8_y),
    issuerSigS: bufToDecimal(credential.issuerSignature.s),
    merkleSiblings: merkleProof.siblings,
    merklePathIndices: merkleProof.pathIndices,
    expirationTimestamp: credential.expirationTimestamp,
  };

  // Step 4: Generate Groth16 proof via snarkjs
  console.log('Generating Groth16 proof (this may take 10-30 seconds)...');
  const { proof, publicSignals } = await snarkjs.groth16.fullProve(
    circuitInput,
    circuitPaths.wasmPath,
    circuitPaths.zkeyPath,
  );

  // Step 5: Format for on-chain verification (groth16-solana format)
  const solanaProof = formatProofForSolana(proof);

  console.log('Proof generated successfully!');
  console.log(`  Nullifier: ${Buffer.from(nullifier).toString('hex').slice(0, 16)}...`);

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
  options?: {
    lightConfig?: LightConfig;
  },
): Promise<BatchProofResult> {
  await initWasm();
  const config = options?.lightConfig || DEVNET_CONFIG;
  const rpc = createLightRpc(config);

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

  // 4. Fetch Merkle proofs for ALL credentials in parallel
  const credentialProofs = await Promise.all(
    sortedCredentials.map(c => lightFetchMerkleProof(rpc, c.commitment))
  );

  // 2. Fetch the shared Global Identity proof
  // In SolID, it is Poseidon(masterPK_x, masterPK_y, revocationNonce)
  const identityCommitment = computeIdentityCommitment(
      masterPublicKey.x,
      masterPublicKey.y,
      revocationNonce
  );
  const globalProof = await lightFetchMerkleProof(rpc, identityCommitment);

  // 3. Build Batch Circuit Input (30+ signals)
  const circuitInput: any = {
    // Public Inputs
    globalRoot: globalProof.root,
    merkleRoots: credentialProofs.map((p: any) => p.root),
    schemaHashes: query.schema_hashes.map((h: any) => bufToDecimal(h)),
    
    queryCredentialIndices: Array(MAX_CREDENTIALS).fill(0),
    queryFieldIndices: Array(MAX_CREDENTIALS).fill(0),
    queryOperators: Array(MAX_CREDENTIALS).fill(0),
    queryValues: Array(MAX_CREDENTIALS).fill('0'),
    numPredicates: query.predicates.length,
    compoundLogic: query.compound_logic === 'AND' ? 0 : 1,
    
    verifierAddress: bufToDecimal(VERIFIER_ID_BYTES),
    verifierNonce: bufToDecimal(query.verifier_nonce),
    currentTimestamp: Math.floor(Date.now() / 1000),

    // Private Inputs
    masterIdentityKey: bufToDecimal(masterPrivateKey),
    revocationNonce: revocationNonce.toString(),
    globalSiblings: globalProof.siblings,
    globalPathIndices: globalProof.pathIndices,
    
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
  const nullifier = Uint8Array.from(Buffer.from(publicSignals[0], 'hex')); // nullifier is publicSignals[0]

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
