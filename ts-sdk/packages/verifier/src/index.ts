/**
 * @solid-protocol/verifier — Proof verification and submission SDK
 *
 * Builds and submits the real `verify_batch_proof` instruction directly.
 * No IDL is required: we hand-roll the 8-byte Anchor discriminator +
 * Borsh argument layout to match `programs/zk-verifier/src/lib.rs`.
 *
 * This keeps the verifier SDK deployable without a pre-built Anchor client.
 */

import {
  Connection,
  PublicKey,
  Keypair,
  Transaction,
  TransactionInstruction,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  sendAndConfirmTransaction,
} from '@solana/web3.js';
import { createHash } from 'crypto';
import {
  PROGRAM_IDS,
  QueryBuilder,
  type CompoundQuery,
  type MultiCredentialQuery,
} from '@solid-protocol/core';

export { QueryBuilder } from '@solid-protocol/core';

// ─── Constants ─────────────────────────────────────────────────────────────

/** Must match `NR_PUBLIC_INPUTS` in `programs/zk-verifier/src/lib.rs`.
 *  Bumped 31 -> 32 by ADR-0014 (issuerTreeRoot added at slot [10]). */
export const NR_PUBLIC_INPUTS = 32;

const NULLIFIER_SEED = Buffer.from('null');
const VERIFIER_CONFIG_SEED = Buffer.from('verifier-config');
const VK_STORAGE_SEED = Buffer.from('vk-storage');

/** Anchor global instruction discriminator = first 8 bytes of
 *  `sha256("global:<snake_case_name>")`. */
function anchorDiscriminator(name: string): Buffer {
  const h = createHash('sha256').update(`global:${name}`).digest();
  return h.subarray(0, 8);
}

// ─── Types ─────────────────────────────────────────────────────────────────

export interface VerificationRequest {
  query: CompoundQuery | MultiCredentialQuery;
  proofData: {
    proof_a: Uint8Array; // 64
    proof_b: Uint8Array; // 128
    proof_c: Uint8Array; // 64
    publicInputs: Uint8Array[]; // exactly NR_PUBLIC_INPUTS of 32-byte slices
    nullifier: Uint8Array; // 32
  };
}

export interface VerificationResult {
  verified: boolean;
  nullifier: Uint8Array;
  transactionSignature: string;
  timestamp: number;
}

export interface SchemaTreeAccounts {
  /** 4 PDAs managed by schema-registry, one per active schema slot (pad with
   *  a zero / dummy `PublicKey` for inactive slots). */
  schemaTree0: PublicKey;
  schemaTree1: PublicKey;
  schemaTree2: PublicKey;
  schemaTree3: PublicKey;
  /** Global state-root PDA. */
  globalTree: PublicKey;
  /** ADR-0014: singleton `IssuerTreeBinding` PDA under issuer-registry.
   *  Derived from `[b"issuer-tree-binding"]` (see
   *  `@solid-protocol/light::deriveIssuerTreeBinding`).  Required for
   *  every verify_batch_proof call post ADR-0014. */
  issuerTreeBinding: PublicKey;
}

// ─── PDA helpers ───────────────────────────────────────────────────────────

export function verifierProgramId(): PublicKey {
  return new PublicKey(PROGRAM_IDS.zkVerifier);
}

export function deriveVerifierConfigPda(programId = verifierProgramId()): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([VERIFIER_CONFIG_SEED], programId);
}

export function deriveVkStoragePda(
  configPda: PublicKey,
  programId = verifierProgramId(),
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [VK_STORAGE_SEED, configPda.toBuffer()],
    programId,
  );
}

export function deriveNullifierPda(
  nullifier: Uint8Array,
  programId = verifierProgramId(),
): [PublicKey, number] {
  if (nullifier.length !== 32) throw new Error('nullifier must be 32 bytes');
  return PublicKey.findProgramAddressSync(
    [NULLIFIER_SEED, Buffer.from(nullifier)],
    programId,
  );
}

// ─── Instruction builders ─────────────────────────────────────────────────

/** Build the raw `verify_batch_proof` instruction. */
export function buildVerifyBatchProofIx(params: {
  payer: PublicKey;
  request: VerificationRequest;
  trees: SchemaTreeAccounts;
  programId?: PublicKey;
}): TransactionInstruction {
  const programId = params.programId ?? verifierProgramId();
  const { proof_a, proof_b, proof_c, publicInputs, nullifier } =
    params.request.proofData;

  if (proof_a.length !== 64) throw new Error('proof_a must be 64 bytes');
  if (proof_b.length !== 128) throw new Error('proof_b must be 128 bytes');
  if (proof_c.length !== 64) throw new Error('proof_c must be 64 bytes');
  if (publicInputs.length !== NR_PUBLIC_INPUTS) {
    throw new Error(
      `publicInputs must have exactly ${NR_PUBLIC_INPUTS} entries, got ${publicInputs.length}`,
    );
  }
  for (const pi of publicInputs) {
    if (pi.length !== 32) throw new Error('each public input must be 32 bytes');
  }
  if (nullifier.length !== 32) throw new Error('nullifier must be 32 bytes');

  // Borsh layout for the instruction args.  `public_inputs` is a Rust
  // `Vec<[u8; 32]>` on-chain (heap-resident; see the comment block on
  // `verify_batch_proof` in `programs/zk-verifier/src/lib.rs` for why
  // this isn't a fixed-size array), so its borsh wire layout is a
  // 4-byte LE length prefix followed by the flat 32-byte slices.
  // Fixed-size `[u8; N]` fields stay as a flat concatenation.
  //
  //   discriminator      (8)
  // + proof_a            (64)
  // + proof_b            (128)
  // + proof_c            (64)
  // + public_inputs len  (4)            <-- borsh Vec length prefix
  // + public_inputs body (32 * 32 = 1024)
  // + nullifier          (32)
  // = 1324 bytes
  const disc = anchorDiscriminator('verify_batch_proof');
  const data = Buffer.alloc(8 + 64 + 128 + 64 + 4 + NR_PUBLIC_INPUTS * 32 + 32);
  let offset = 0;
  disc.copy(data, offset); offset += 8;
  Buffer.from(proof_a).copy(data, offset); offset += 64;
  Buffer.from(proof_b).copy(data, offset); offset += 128;
  Buffer.from(proof_c).copy(data, offset); offset += 64;
  data.writeUInt32LE(NR_PUBLIC_INPUTS, offset); offset += 4;
  for (const pi of publicInputs) {
    Buffer.from(pi).copy(data, offset); offset += 32;
  }
  Buffer.from(nullifier).copy(data, offset);

  const [configPda] = deriveVerifierConfigPda(programId);
  const [vkPda] = deriveVkStoragePda(configPda, programId);
  const [nullifierPda] = deriveNullifierPda(nullifier, programId);

  // Account order must match `VerifyBatchProof` in the on-chain program.
  // ADR-0014: `issuer_tree_binding` inserted AFTER the four schema
  // trees, BEFORE `payer`.  A mismatch here silently sends accounts
  // to the wrong slots and makes every proof fail with an unhelpful
  // error; the shape is pinned in the on-chain Accounts derive.
  const keys = [
    { pubkey: configPda, isSigner: false, isWritable: true },
    { pubkey: vkPda, isSigner: false, isWritable: false },
    { pubkey: nullifierPda, isSigner: false, isWritable: true },
    { pubkey: params.trees.globalTree, isSigner: false, isWritable: false },
    { pubkey: params.trees.schemaTree0, isSigner: false, isWritable: false },
    { pubkey: params.trees.schemaTree1, isSigner: false, isWritable: false },
    { pubkey: params.trees.schemaTree2, isSigner: false, isWritable: false },
    { pubkey: params.trees.schemaTree3, isSigner: false, isWritable: false },
    { pubkey: params.trees.issuerTreeBinding, isSigner: false, isWritable: false },
    { pubkey: params.payer, isSigner: true, isWritable: true },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
  ];

  return new TransactionInstruction({ programId, keys, data });
}

// ─── High-level entry point ───────────────────────────────────────────────

/**
 * Submit a proof to the on-chain ZK verifier program.
 *
 * Returns the real transaction signature from a confirmed `verify_batch_proof`
 * invocation. Throws if verification fails on-chain or the nullifier PDA
 * already exists (replay).
 */
export async function verifyOnChain(
  connection: Connection,
  payer: Keypair,
  request: VerificationRequest,
  trees: SchemaTreeAccounts,
  programId: PublicKey = verifierProgramId(),
): Promise<VerificationResult> {
  const ix = buildVerifyBatchProofIx({
    payer: payer.publicKey,
    request,
    trees,
    programId,
  });
  const tx = new Transaction().add(ix);
  tx.feePayer = payer.publicKey;

  const signature = await sendAndConfirmTransaction(connection, tx, [payer], {
    commitment: 'confirmed',
  });

  return {
    verified: true,
    nullifier: request.proofData.nullifier,
    transactionSignature: signature,
    timestamp: Math.floor(Date.now() / 1000),
  };
}

// ─── Issuer status check ──────────────────────────────────────────────────

/**
 * Check if an issuer is approved in the DAO registry.
 *
 * Reads the `IssuerAccount` PDA directly. Deserializes the first 8-byte Anchor
 * discriminator + the 32-byte authority + (4+n) name + (4+m) metadata_uri +
 * Ax/Ay + tier + status, which is enough to answer the common question.
 */
export async function checkIssuerStatus(
  connection: Connection,
  issuerAuthority: PublicKey,
  registryProgramId: PublicKey = new PublicKey(PROGRAM_IDS.issuerRegistry),
): Promise<{ approved: boolean; name: string; stakedAmount: bigint; tier: number; status: number }> {
  const [issuerPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('issuer'), issuerAuthority.toBuffer()],
    registryProgramId,
  );

  const info = await connection.getAccountInfo(issuerPda);
  if (!info) return { approved: false, name: '', stakedAmount: 0n, tier: 0, status: 0 };

  // Skip 8-byte Anchor discriminator + 32 byte authority.
  let cur = 8 + 32;
  const nameLen = info.data.readUInt32LE(cur); cur += 4;
  const name = info.data.slice(cur, cur + nameLen).toString('utf-8'); cur += nameLen;
  const metaLen = info.data.readUInt32LE(cur); cur += 4;
  cur += metaLen; // skip metadata_uri
  cur += 32 + 32; // Ax, Ay
  const tier = info.data.readUInt8(cur); cur += 1;
  const status = info.data.readUInt8(cur); cur += 1;
  const staked = info.data.readBigUInt64LE(cur); cur += 8;

  return {
    approved: status === 1,  // IssuerStatus::Approved = 1
    name,
    stakedAmount: staked,
    tier,
    status,
  };
}

/**
 * Generate a cryptographically secure verifier nonce.
 * Uses Web Crypto if available (browser / Node >=18), falls back to `node:crypto`.
 */
export function generateVerifierNonce(): Uint8Array {
  const nonce = new Uint8Array(32);
  if (typeof globalThis.crypto?.getRandomValues === 'function') {
    globalThis.crypto.getRandomValues(nonce);
    return nonce;
  }
  const nodeCrypto = require('crypto');
  nodeCrypto.randomFillSync(nonce);
  return nonce;
}
