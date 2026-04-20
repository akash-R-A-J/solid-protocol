/**
 * @solid-protocol/issuer — Credential issuance SDK
 *
 * v0.2 (2026-04 R-2 remediation):
 *   Rewritten to target the on-chain `issuer-registry::issue_credential`
 *   instruction, which publishes the commitment to an SPL Account
 *   Compression tree with a PDA-signed tree authority.  The package no
 *   longer imports `@lightprotocol/stateless.js`.
 *
 * Pipeline:
 *   1. `computeCommitment` (WASM, BN254/Poseidon)
 *   2. `sign` (WASM, EdDSA-Poseidon over BabyJubJub)
 *   3. `issueCredentialOnChain` — build and send
 *      `issuer_registry::issue_credential` ix
 *   4. Return the full `IssuedCredential` bundle for holder-side storage.
 */

import {
  initWasm,
  sign,
  computeCommitment,
} from '@solid-protocol/core';
import {
  ISSUER_REGISTRY_PROGRAM_ID,
  SPL_ACCOUNT_COMPRESSION_PROGRAM_ID,
  SPL_NOOP_PROGRAM_ID,
  deriveTreeAuthority,
} from '@solid-protocol/light';
import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
} from '@solana/web3.js';

// ─── Instruction discriminator ────────────────────────────────────────────

/**
 * Anchor discriminator for `issuer_registry::issue_credential`.
 *
 * Derived by `sha256("global:issue_credential")[..8]`.  Pinning the bytes
 * here (rather than recomputing) keeps the SDK free of a sha256 runtime
 * dependency and makes the byte-level contract auditable.
 */
export const ISSUE_CREDENTIAL_DISCRIMINATOR = Uint8Array.from([
  0xff, 0xc1, 0xab, 0xe0, 0x44, 0xab, 0xc2, 0x57,
]);

// ─── Types ────────────────────────────────────────────────────────────────

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
  expirationTimestamp: number;
  issuedAt: number;
  /** tx signature of the `issue_credential` call. */
  signature: string;
  /** Merkle tree that received the leaf. */
  merkleTree: string;
}

export interface IssueOptions {
  connection: Connection;
  /** Keypair authorized by the issuer's `issuer_account` (matches
   *  `issuer_account.authority`). */
  issuerAuthority: Keypair;
  /** The SPL AC tree that has been bound to `schemaHash`.  Must already
   *  exist and have its authority set to `deriveTreeAuthority(schemaHash)`.  */
  merkleTree: PublicKey;
  /** Optional extra signers (e.g. fee payer ≠ issuer authority). */
  extraSigners?: Keypair[];
  /** Override to avoid sending (returns the assembled tx for inspection). */
  dryRun?: boolean;
}

// ─── Instruction construction ─────────────────────────────────────────────

/** `(b"issuer", authority)` under `issuer-registry`. */
export function deriveIssuerAccount(authority: PublicKey): { pda: PublicKey; bump: number } {
  const [pda, bump] = PublicKey.findProgramAddressSync(
    [Buffer.from('issuer'), authority.toBuffer()],
    ISSUER_REGISTRY_PROGRAM_ID,
  );
  return { pda, bump };
}

/**
 * Build a raw `issuer_registry::issue_credential` TransactionInstruction.
 *
 * Account order (MUST match the on-chain `IssueCredential` context):
 *   0. issuer_account          (writable, PDA)
 *   1. issuer_authority        (signer)
 *   2. tree_authority          (PDA; SPL AC sees it as signer via CPI)
 *   3. merkle_tree             (writable)
 *   4. log_wrapper             (spl-noop)
 *   5. compression_program     (SPL AC)
 */
export function buildIssueCredentialIx(
  issuerAuthority: PublicKey,
  schemaHash: Uint8Array,
  commitment: Uint8Array,
  merkleTree: PublicKey,
): TransactionInstruction {
  if (schemaHash.length !== 32) {
    throw new Error(`schemaHash must be 32 bytes, got ${schemaHash.length}`);
  }
  if (commitment.length !== 32) {
    throw new Error(`commitment must be 32 bytes, got ${commitment.length}`);
  }

  const { pda: issuerAccount } = deriveIssuerAccount(issuerAuthority);
  const { pda: treeAuthority } = deriveTreeAuthority(schemaHash);

  const data = Buffer.concat([
    Buffer.from(ISSUE_CREDENTIAL_DISCRIMINATOR),
    Buffer.from(schemaHash),
    Buffer.from(commitment),
  ]);

  return new TransactionInstruction({
    programId: ISSUER_REGISTRY_PROGRAM_ID,
    keys: [
      { pubkey: issuerAccount, isSigner: false, isWritable: true },
      { pubkey: issuerAuthority, isSigner: true, isWritable: false },
      { pubkey: treeAuthority, isSigner: false, isWritable: false },
      { pubkey: merkleTree, isSigner: false, isWritable: true },
      { pubkey: SPL_NOOP_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: SPL_ACCOUNT_COMPRESSION_PROGRAM_ID, isSigner: false, isWritable: false },
    ],
    data,
  });
}

// ─── High-level pipeline ──────────────────────────────────────────────────

/**
 * End-to-end issuance: compute commitment, sign, publish on-chain.
 *
 * The `issuerPrivateKey` is the BabyJubJub private key (for EdDSA).  The
 * `issuerAuthority` Solana keypair, passed via `options`, is a separate
 * identity that owns the `issuer_account` PDA and authorizes on-chain
 * publication.  These two identities are distinct by design — the BJJ key
 * is what the holder cryptographically trusts; the Solana key is merely the
 * SPL account custodian.
 */
export async function issueCredential(
  issuerPrivateKey: Uint8Array,
  issuerPubKeyX: Uint8Array,
  issuerPubKeyY: Uint8Array,
  request: IssuanceRequest,
  options: IssueOptions,
): Promise<IssuedCredential> {
  await initWasm();

  // Random salt.
  const salt = new Uint8Array(32);
  crypto.getRandomValues(salt);

  const commitment = computeCommitment(
    request.attestationData,
    request.schemaHash,
    request.holderPubKeyX,
    request.holderPubKeyY,
    salt,
  );

  const signature = sign(issuerPrivateKey, commitment);

  const ix = buildIssueCredentialIx(
    options.issuerAuthority.publicKey,
    request.schemaHash,
    commitment,
    options.merkleTree,
  );

  const tx = new Transaction().add(ix);
  let txSig = '';
  if (!options.dryRun) {
    const signers = [options.issuerAuthority, ...(options.extraSigners ?? [])];
    txSig = await sendAndConfirmTransaction(options.connection, tx, signers, {
      commitment: 'confirmed',
      skipPreflight: false,
    });
  }

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
    expirationTimestamp: request.expirationTimestamp ?? 0,
    issuedAt: Math.floor(Date.now() / 1000),
    signature: txSig,
    merkleTree: options.merkleTree.toBase58(),
  };
}

/**
 * Batch-issue multiple credentials.  Each issuance is a separate
 * transaction — the SPL AC append instruction is not batchable within a
 * single tx because the tree's sequence number advances on every append.
 */
export async function batchIssueCredentials(
  issuerPrivateKey: Uint8Array,
  issuerPubKeyX: Uint8Array,
  issuerPubKeyY: Uint8Array,
  requests: IssuanceRequest[],
  options: IssueOptions,
): Promise<IssuedCredential[]> {
  const results: IssuedCredential[] = [];
  for (const request of requests) {
    results.push(
      await issueCredential(
        issuerPrivateKey,
        issuerPubKeyX,
        issuerPubKeyY,
        request,
        options,
      ),
    );
  }
  return results;
}
