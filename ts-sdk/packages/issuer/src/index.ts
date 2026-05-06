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
// SEC-048 Phase E.4 (2026-05-XX): static import matches the holder
// package's pattern (`@solid-protocol/holder`'s generateBatchProof).
// A dynamic `await import('snarkjs')` inside generateSubgroupProof
// caused `bootstrap_issuer.ts` to hang after Summary print on Node 24
// -- the dynamically-loaded snarkjs left worker_threads alive in a
// way that held the event loop open even after the script body
// completed.  Static import + identical loading semantics to the
// holder package fixes the hang.
// @ts-ignore -- snarkjs doesn't ship perfect types
import * as snarkjs from 'snarkjs';
import {
  ISSUER_REGISTRY_PROGRAM_ID,
  SPL_ACCOUNT_COMPRESSION_PROGRAM_ID,
  SPL_NOOP_PROGRAM_ID,
  deriveIssuerAccountPda,
  deriveIssuerSchemaPermissionPda,
  deriveSchemaAccountPda,
  deriveSchemaTreeBindingPda,
  deriveTreeAuthorityPda,
  type IssuerRegistryProgramIdOverrides,
} from './registry.js';
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
  /** Schema name, as passed to `schema_registry::register_schema`.
   *  Required since SOLID-SEC-003 to derive the `SchemaAccount` PDA. */
  schemaName: string;
  /** Schema version byte (u8), as passed to
   *  `schema_registry::register_schema`. */
  schemaVersion: number;
  /**
   * Optional fee payer.  When set, this keypair is bound to `tx.feePayer`
   * and prepended to the signer list.  Use when the `issuerAuthority` is
   * an unfunded BJJ-anchored identity and a separate Solana wallet
   * fronts the lamports.  When unset, the `issuerAuthority` pays its
   * own fees.
   */
  feePayer?: Keypair;
  /** Optional extra signers beyond `issuerAuthority` and `feePayer`.
   *  Each must appear as a signer in the instruction's account list,
   *  otherwise web3.js raises `unknown signer` at compile time. */
  extraSigners?: Keypair[];
  /** Override to avoid sending (returns the assembled tx for inspection). */
  dryRun?: boolean;
  /** Program IDs sourced from a deployment manifest. Defaults to canonical SolID devnet IDs. */
  programIds?: IssuerRegistryProgramIdOverrides;
}

// ─── Instruction construction ─────────────────────────────────────────────

/** `(b"issuer", authority)` under `issuer-registry`. */
export function deriveIssuerAccount(
  authority: PublicKey,
  programIds?: IssuerRegistryProgramIdOverrides,
): { pda: PublicKey; bump: number } {
  const programId = programIds?.issuerRegistry ?? ISSUER_REGISTRY_PROGRAM_ID;
  const [, bump] = PublicKey.findProgramAddressSync(
    [Buffer.from('issuer'), authority.toBuffer()],
    programId,
  );
  return {
    pda: deriveIssuerAccountPda(authority, programIds),
    bump,
  };
}

/** `(b"issuer-schema", issuer_account, schemaHash)` under `issuer-registry`. */
export function deriveIssuerSchemaPermission(
  issuerAccount: PublicKey,
  schemaHash: Uint8Array,
  programIds?: IssuerRegistryProgramIdOverrides,
): { pda: PublicKey; bump: number } {
  if (schemaHash.length !== 32) {
    throw new Error(`schemaHash must be 32 bytes, got ${schemaHash.length}`);
  }
  const programId = programIds?.issuerRegistry ?? ISSUER_REGISTRY_PROGRAM_ID;
  const [, bump] = PublicKey.findProgramAddressSync(
    [Buffer.from('issuer-schema'), issuerAccount.toBuffer(), Buffer.from(schemaHash)],
    programId,
  );
  return {
    pda: deriveIssuerSchemaPermissionPda(issuerAccount, schemaHash, programIds),
    bump,
  };
}

/**
 * Build a raw `issuer_registry::issue_credential` TransactionInstruction.
 *
 * Account order (MUST match the on-chain `IssueCredential` context in
 * `programs/issuer-registry/src/lib.rs`, post-SOLID-SEC-003):
 *   0. issuer_account             (writable, PDA)
 *   1. issuer_authority           (signer)
 *   2. issuer_schema_permission   (read-only, PDA)
 *   3. schema_account             (read-only, PDA under schema-registry)
 *   4. schema_tree_binding        (read-only, PDA under schema-registry)
 *   5. tree_authority             (PDA; SPL AC sees it as signer via CPI)
 *   6. merkle_tree                (writable)
 *   7. log_wrapper                (spl-noop)
 *   8. compression_program        (SPL AC)
 *
 * `schemaName` + `schemaVersion` are required to derive the
 * `SchemaAccount` PDA.  These should match exactly what was passed to
 * `schema_registry::register_schema`; the on-chain handler will
 * re-derive the same seeds and reject a mismatch with `ConstraintSeeds`.
 */
export function buildIssueCredentialIx(
  issuerAuthority: PublicKey,
  schemaName: string,
  schemaVersion: number,
  schemaHash: Uint8Array,
  commitment: Uint8Array,
  merkleTree: PublicKey,
  programIds?: IssuerRegistryProgramIdOverrides,
): TransactionInstruction {
  if (schemaHash.length !== 32) {
    throw new Error(`schemaHash must be 32 bytes, got ${schemaHash.length}`);
  }
  if (commitment.length !== 32) {
    throw new Error(`commitment must be 32 bytes, got ${commitment.length}`);
  }

  const issuerAccount = deriveIssuerAccountPda(issuerAuthority, programIds);
  const issuerSchemaPermission = deriveIssuerSchemaPermissionPda(issuerAccount, schemaHash, programIds);
  const schemaAccount = deriveSchemaAccountPda(schemaName, schemaVersion, programIds);
  const schemaTreeBinding = deriveSchemaTreeBindingPda(schemaHash, programIds);
  const treeAuthority = deriveTreeAuthorityPda(schemaHash, programIds);

  const data = Buffer.concat([
    Buffer.from(ISSUE_CREDENTIAL_DISCRIMINATOR),
    Buffer.from(schemaHash),
    Buffer.from(commitment),
  ]);

  return new TransactionInstruction({
    programId: programIds?.issuerRegistry ?? ISSUER_REGISTRY_PROGRAM_ID,
    keys: [
      { pubkey: issuerAccount, isSigner: false, isWritable: true },
      { pubkey: issuerAuthority, isSigner: true, isWritable: false },
      { pubkey: issuerSchemaPermission, isSigner: false, isWritable: false },
      { pubkey: schemaAccount, isSigner: false, isWritable: false },
      { pubkey: schemaTreeBinding, isSigner: false, isWritable: false },
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
    options.schemaName,
    options.schemaVersion,
    request.schemaHash,
    commitment,
    options.merkleTree,
    options.programIds,
  );

  const tx = new Transaction().add(ix);
  // Bind feePayer explicitly so web3.js adds it to the message
  // accountKeys; otherwise passing a keypair via signers that isn't
  // referenced by the instruction triggers "unknown signer" at
  // compileMessage time.
  const feePayer = options.feePayer ?? options.issuerAuthority;
  tx.feePayer = feePayer.publicKey;
  let txSig = '';
  if (!options.dryRun) {
    // De-dupe by pubkey: feePayer + issuerAuthority may be the same key.
    const seen = new Set<string>();
    const signers: Keypair[] = [];
    for (const kp of [feePayer, options.issuerAuthority, ...(options.extraSigners ?? [])]) {
      const k = kp.publicKey.toBase58();
      if (seen.has(k)) continue;
      seen.add(k);
      signers.push(kp);
    }
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

// ─── SEC-048 Phase E.4: subgroup-proof generation ─────────────────────────
//
// `register_issuer` (post Phase E.3) takes a 256-byte
// `subgroup_proof: Vec<u8>` argument that is a Groth16 proof of the
// prime-order subgroup invariant for the supplied BJJ pubkey.  This
// helper produces those bytes from a candidate pubkey + the subgroup
// circuit's WASM/zkey artifacts.
//
// The returned bytes are in the SDK-encoded form expected by the
// on-chain handler (LB5 / SOLID-SEC-067 G2 (imag, real) swap applied;
// proof_a NOT pre-negated -- on-chain
// `solid_light::groth16::verify_groth16_proof::<2>` negates internally).

/**
 * Generate a Groth16 proof that `(pubKeyX, pubKeyY)` is in the BJJ
 * prime-order subgroup, suitable for passing to `register_issuer` as
 * the `subgroup_proof` argument (256-byte SDK-encoded form).
 *
 * Inputs:
 *   - `pubKeyX`, `pubKeyY`: 32-byte LE buffers (circomlib-native form)
 *     of the candidate BJJ pubkey.  These are the same bytes that
 *     `register_issuer` takes as `bjj_pub_key_x` / `_y`.
 *   - `wasmBytes`, `zkeyBytes`: the subgroup circuit's `.wasm` and
 *     `.zkey` artifacts, integrity-verified by the caller (see
 *     `SUBGROUP_WASM_PIN` / `SUBGROUP_ZKEY_PIN` in
 *     `@solid-protocol/sdk`).
 *
 * Returns the 256-byte serialized proof (`proofA[64] || proofB[128] ||
 * proofC[64]`) plus the snarkjs `publicSignals` for caller-side
 * sanity-checking against the input pubkey.
 *
 * Throws if snarkjs witness generation rejects the input (which
 * happens for off-curve points: the in-circuit `BabyCheck` is the
 * first constraint).  The caller can use this as a UX-grade
 * pre-submit gate; the load-bearing soundness check is the on-chain
 * Groth16 verify.
 */
export async function generateSubgroupProof(
  pubKeyX: Uint8Array,
  pubKeyY: Uint8Array,
  wasmBytes: Uint8Array,
  zkeyBytes: Uint8Array,
): Promise<{ proofBytes: Uint8Array; publicSignals: [string, string] }> {
  if (pubKeyX.length !== 32) {
    throw new Error(
      `generateSubgroupProof: pubKeyX must be 32 bytes (got ${pubKeyX.length})`,
    );
  }
  if (pubKeyY.length !== 32) {
    throw new Error(
      `generateSubgroupProof: pubKeyY must be 32 bytes (got ${pubKeyY.length})`,
    );
  }

  // Convert LE bytes -> decimal string for snarkjs witness input.
  // BabyJubJub Ax / Ay are field elements in `Fq` (BN254 base
  // prime), so the LE byte-encoding directly decodes to the integer
  // value the circuit expects (no mod-reduction necessary -- callers
  // upstream MUST gate via `is_canonical_bn254_le`, which
  // `register_issuer`'s SOLID-SEC-062 check enforces on-chain).
  const ax = leBytesToDecimal(pubKeyX);
  const ay = leBytesToDecimal(pubKeyY);

  const { proof, publicSignals } = await snarkjs.groth16.fullProve(
    { Ax: ax, Ay: ay },
    wasmBytes,
    zkeyBytes,
  );

  if (publicSignals.length !== 2) {
    throw new Error(
      `generateSubgroupProof: subgroup circuit produced ${publicSignals.length} public ` +
        `signals; expected exactly 2 (Ax, Ay).`,
    );
  }

  // Encode proof for groth16-solana on-chain verify.  Mirrors holder
  // SDK's `formatProofForSolana`: G2 (real, imag) -> (imag, real)
  // swap (LB5 / SOLID-SEC-067); proof_a NOT pre-negated (the
  // on-chain helper negates internally).
  const proofA = new Uint8Array(64);
  proofA.set(bigintDecToBe32(proof.pi_a[0]), 0);
  proofA.set(bigintDecToBe32(proof.pi_a[1]), 32);

  const proofB = new Uint8Array(128);
  proofB.set(bigintDecToBe32(proof.pi_b[0][1]), 0); // x_imag
  proofB.set(bigintDecToBe32(proof.pi_b[0][0]), 32); // x_real
  proofB.set(bigintDecToBe32(proof.pi_b[1][1]), 64); // y_imag
  proofB.set(bigintDecToBe32(proof.pi_b[1][0]), 96); // y_real

  const proofC = new Uint8Array(64);
  proofC.set(bigintDecToBe32(proof.pi_c[0]), 0);
  proofC.set(bigintDecToBe32(proof.pi_c[1]), 32);

  const proofBytes = new Uint8Array(256);
  proofBytes.set(proofA, 0);
  proofBytes.set(proofB, 64);
  proofBytes.set(proofC, 192);

  return {
    proofBytes,
    publicSignals: [publicSignals[0] as string, publicSignals[1] as string],
  };
}

/** LE 32-byte buffer -> decimal string (BigInt). */
function leBytesToDecimal(le: Uint8Array): string {
  let n = 0n;
  for (let i = le.length - 1; i >= 0; i--) {
    n = n * 256n + BigInt(le[i]);
  }
  return n.toString(10);
}

/** Decimal string -> 32-byte BE buffer (matches groth16-solana
 *  expected encoding for public-input field elements). */
function bigintDecToBe32(dec: string): Uint8Array {
  const n = BigInt(dec);
  const hex = n.toString(16).padStart(64, '0');
  if (hex.length > 64) {
    throw new Error(`bigintDecToBe32: value exceeds 32 bytes (${dec})`);
  }
  const out = new Uint8Array(32);
  for (let i = 0; i < 32; i++) {
    out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}
