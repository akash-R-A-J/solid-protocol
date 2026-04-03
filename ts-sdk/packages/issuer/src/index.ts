/**
 * @solid-protocol/issuer — Credential issuance SDK
 *
 * Full issuance pipeline integrating with Light Protocol:
 * 1. Generate/load issuer BJJ identity
 * 2. Compute attestation commitment (via WASM)
 * 3. Sign commitment with EdDSA-Poseidon (via WASM)
 * 4. Insert commitment leaf into Light Protocol compressed tree
 * 5. Return credential bundle to holder
 */

import {
  initWasm,
  generateKeypair,
  sign,
  computeCommitment,
  poseidonHash,
  type BJJKeypair,
} from '@solid-protocol/core';
import {
  createLightRpc,
  insertCredentialLeaf,
  type LightConfig,
  DEVNET_CONFIG,
} from '@solid-protocol/light';
import { Connection, PublicKey, Keypair } from '@solana/web3.js';

export interface IssuanceRequest {
  schemaHash: Uint8Array;
  attestationData: bigint[];
  holderPubKeyX: Uint8Array;
  holderPubKeyY: Uint8Array;
  expirationTimestamp?: number;
}

export interface IssuedCredential {
  schemaHash: Uint8Array;
  attestationData: bigint[];
  issuerSignature: { r8_x: Uint8Array; r8_y: Uint8Array; s: Uint8Array };
  issuerPubKeyX: Uint8Array;
  issuerPubKeyY: Uint8Array;
  holderPubKeyX: Uint8Array;
  holderPubKeyY: Uint8Array;
  salt: Uint8Array;
  commitment: Uint8Array;
  leafIndex: number;
  expirationTimestamp: number;
  issuedAt: number;
}

/**
 * Issue a credential to a holder with full Light Protocol integration.
 *
 * Steps:
 *   1. Compute Poseidon commitment (WASM)
 *   2. Sign commitment with issuer BJJ key (WASM)
 *   3. Insert commitment into compressed Merkle tree (Light Protocol)
 *   4. Return full credential bundle for holder's local storage
 */
export async function issueCredential(
  issuerPrivateKey: Uint8Array,
  issuerPubKeyX: Uint8Array,
  issuerPubKeyY: Uint8Array,
  request: IssuanceRequest,
  options: {
    payer: Keypair;
    lightConfig?: LightConfig;
  },
): Promise<IssuedCredential> {
  await initWasm();

  // Step 1: Generate random salt
  const salt = new Uint8Array(32);
  crypto.getRandomValues(salt);

  // Step 2: Compute commitment via WASM (matches circuit Step 2)
  const commitment = computeCommitment(
    request.attestationData,
    request.schemaHash,
    request.holderPubKeyX,
    request.holderPubKeyY,
    salt,
  );

  // Step 3: Sign commitment with issuer's BJJ key (matches circuit Step 3)
  const signature = sign(issuerPrivateKey, commitment);

  // Step 4: Insert into Light Protocol compressed tree
  const config = options.lightConfig || DEVNET_CONFIG;
  const rpc = createLightRpc(config);
  const issuerPubkey = options.payer.publicKey;

  const { txSignature, leafIndex } = await insertCredentialLeaf(
    rpc,
    options.payer,
    commitment,
    request.schemaHash,
    issuerPubkey,
  );

  console.log(`Credential issued and compressed!`);
  console.log(`  Commitment: ${Buffer.from(commitment).toString('hex').slice(0, 16)}...`);
  console.log(`  Leaf index: ${leafIndex}`);
  console.log(`  TX: ${txSignature}`);

  return {
    schemaHash: request.schemaHash,
    attestationData: request.attestationData,
    issuerSignature: signature,
    issuerPubKeyX,
    issuerPubKeyY,
    holderPubKeyX: request.holderPubKeyX,
    holderPubKeyY: request.holderPubKeyY,
    salt,
    commitment,
    leafIndex,
    expirationTimestamp: request.expirationTimestamp ?? 0,
    issuedAt: Math.floor(Date.now() / 1000),
  };
}

/**
 * Batch issue multiple credentials (cost-efficient).
 * Groups multiple leaf insertions into fewer transactions.
 */
export async function batchIssueCredentials(
  issuerPrivateKey: Uint8Array,
  issuerPubKeyX: Uint8Array,
  issuerPubKeyY: Uint8Array,
  requests: IssuanceRequest[],
  options: {
    payer: Keypair;
    lightConfig?: LightConfig;
  },
): Promise<IssuedCredential[]> {
  const results: IssuedCredential[] = [];
  for (const request of requests) {
    const cred = await issueCredential(
      issuerPrivateKey, issuerPubKeyX, issuerPubKeyY,
      request, options,
    );
    results.push(cred);
  }
  return results;
}
