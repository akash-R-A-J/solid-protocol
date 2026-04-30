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
  AddressLookupTableProgram,
  AddressLookupTableAccount,
  VersionedTransaction,
  TransactionMessage,
  ComputeBudgetProgram,
} from '@solana/web3.js';

/** Per-tx CU limit for `verify_batch_proof`.  The handler itself runs
 *  ~285K-345K CU per the cu_budget audit (Groth16 verify dominates at
 *  ~250K-300K via the alt_bn128 syscalls).  800K leaves ~2.5x headroom
 *  for run-to-run variance and is the same value SOLID-SEC-046 (CU
 *  regression gate) uses as its catch-all submit cap. */
const VERIFY_BATCH_PROOF_CU_LIMIT = 800_000;
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

/** SOLID-SEC-054 / B13: number of public-input slots transmitted on the
 *  wire. The remaining `NR_PUBLIC_INPUTS - NR_WIRE_INPUTS = 11` slots
 *  are reconstructed on-chain from accounts the ix already takes
 *  (global_tree, schema_tree_0..3, issuer_tree_binding, program_id).
 *  Wire shrinks 1324 -> 972 bytes, under Solana's 1232-byte legacy-tx
 *  packet ceiling. Must match `NR_WIRE_INPUTS` in
 *  `programs/zk-verifier/src/lib.rs`. */
export const NR_WIRE_INPUTS = 21;

/** Slots transmitted on the wire, in canonical (ascending) order.
 *  publicSignals[WIRE_INPUT_SLOTS[i]] is sent as wire element `i`.
 *  Mirror of `WIRE_INPUT_SLOTS` in `programs/zk-verifier/src/lib.rs`;
 *  any divergence breaks Groth16 verification at runtime. */
export const WIRE_INPUT_SLOTS: readonly number[] = [
  0, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 30, 31,
];

/** Extract the 21-element wire subset from a full 32-element publicSignals
 *  array. Caller-friendly helper around `WIRE_INPUT_SLOTS`. Throws if the
 *  input length is not exactly `NR_PUBLIC_INPUTS`. */
export function extractWirePublicInputs(publicSignals: Uint8Array[]): Uint8Array[] {
  if (publicSignals.length !== NR_PUBLIC_INPUTS) {
    throw new Error(
      `extractWirePublicInputs expects exactly ${NR_PUBLIC_INPUTS} entries, got ${publicSignals.length}`,
    );
  }
  return WIRE_INPUT_SLOTS.map(i => publicSignals[i]);
}

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
    /**
     * Exactly `NR_WIRE_INPUTS` (21) of 32-byte slices, in
     * `WIRE_INPUT_SLOTS` order.  SOLID-SEC-054 / B13: the on-chain
     * handler reconstructs the remaining 11 slots from the accounts
     * already on the ix surface, so the wire only carries the
     * witness-bound subset.  Use `extractWirePublicInputs` to derive
     * this from the full 32-element snarkjs `publicSignals` output.
     */
    publicInputs: Uint8Array[];
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
  /** 4 PDAs managed by schema-registry, one per active schema slot.
   *  SOLID-SEC-054 / B13: the on-chain handler distinguishes active from
   *  inactive slots by checking `account.owner == SCHEMA_REGISTRY_ID`,
   *  so callers MUST pass `PublicKey.default` (a property, not a call --
   *  the all-zero pubkey whose owner resolves to NativeLoader / system)
   *  for inactive slots.  Passing a real schema-tree PDA in an inactive
   *  slot causes the handler to reconstruct that slot's
   *  (merkleRoot, schemaHash) into the public-input array, which then
   *  fails Groth16 because the witness committed to zero for that slot. */
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
  if (publicInputs.length !== NR_WIRE_INPUTS) {
    throw new Error(
      `publicInputs must have exactly ${NR_WIRE_INPUTS} entries (the wire subset), got ${publicInputs.length}. ` +
        `If you have the full 32-element snarkjs publicSignals, pipe it through extractWirePublicInputs() first.`,
    );
  }
  for (const pi of publicInputs) {
    if (pi.length !== 32) throw new Error('each public input must be 32 bytes');
  }
  if (nullifier.length !== 32) throw new Error('nullifier must be 32 bytes');

  // Borsh layout for the instruction args.  `public_inputs` on-chain
  // is `Vec<u8>` (a flat byte vector); the handler chunks the payload
  // into 32-byte slices internally.  `Vec<[u8; 32]>` would have been
  // the natural type, but Anchor 0.30.1's per-element BorshDeserialize
  // for that type fails on BPF at runtime (`InstructionDidNotDeserialize`,
  // ~15K CU consumed).  See the comment block on `verify_batch_proof`
  // in `programs/zk-verifier/src/lib.rs` for the durable-fix reasoning.
  //
  //   discriminator      (8)
  // + proof_a            (64)
  // + proof_b            (128)
  // + proof_c            (64)
  // + public_inputs len  (4)            <-- borsh Vec length prefix
  //                                         (BYTE count = 672)
  // + public_inputs body (NR_WIRE_INPUTS * 32 = 21 * 32 = 672)
  // + nullifier          (32)
  // = 972 bytes  (post SOLID-SEC-054 / B13 reconstruction; was 1324
  //               pre-reconstruction with all 32 slots on the wire)
  const disc = anchorDiscriminator('verify_batch_proof');
  const wireBodyLen = NR_WIRE_INPUTS * 32;
  const data = Buffer.alloc(8 + 64 + 128 + 64 + 4 + wireBodyLen + 32);
  let offset = 0;
  disc.copy(data, offset); offset += 8;
  Buffer.from(proof_a).copy(data, offset); offset += 64;
  Buffer.from(proof_b).copy(data, offset); offset += 128;
  Buffer.from(proof_c).copy(data, offset); offset += 64;
  data.writeUInt32LE(wireBodyLen, offset); offset += 4;
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

// ─── B13 Option 2: buffer-account chunked-upload SDK ──────────────────────
//
// Companion to the on-chain trio (`init_proof_buffer`,
// `upload_proof_chunk`, `verify_batch_proof_v2`).  Caller flow:
//
//   1. ensureLookupTable(...)
//   2. init_proof_buffer (1 small tx)
//   3. upload_proof_chunk * N  (~2 chunks of 480 bytes each)
//   4. verify_batch_proof_v2 (1 small tx with cuIx + ALT)
//
// Wire-size: every step fits well under the 1232-byte legacy-tx cap.

/** Total payload staged in `proof_buffer.data`; mirrors
 *  `PROOF_BUFFER_PAYLOAD_SIZE` in the on-chain program.
 *  Layout: proof_a (64) + proof_b (128) + proof_c (64) + nullifier (32) +
 *  wire_inputs (NR_WIRE_INPUTS * 32 = 672) = 960. */
export const PROOF_BUFFER_PAYLOAD_SIZE: number = 64 + 128 + 64 + 32 + NR_WIRE_INPUTS * 32;

/** Default chunk size for `upload_proof_chunk`.  480 splits the 960-byte
 *  payload into exactly two chunks of equal size and each ix data is
 *  ~492 bytes (well under the 1232-byte cap). */
export const DEFAULT_PROOF_CHUNK_SIZE: number = 480;

export function deriveProofBufferPda(
  payer: PublicKey,
  programId: PublicKey = verifierProgramId(),
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from('proof-buffer'), payer.toBuffer()],
    programId,
  );
}

/** Encode the full 960-byte `proof_buffer.data` payload.  Mirrors the
 *  on-chain layout in `verify_batch_proof_v2`. */
export function encodeProofBufferPayload(
  proof_a: Uint8Array,
  proof_b: Uint8Array,
  proof_c: Uint8Array,
  nullifier: Uint8Array,
  wireInputs: Uint8Array[],
): Uint8Array {
  if (proof_a.length !== 64) throw new Error('proof_a must be 64 bytes');
  if (proof_b.length !== 128) throw new Error('proof_b must be 128 bytes');
  if (proof_c.length !== 64) throw new Error('proof_c must be 64 bytes');
  if (nullifier.length !== 32) throw new Error('nullifier must be 32 bytes');
  if (wireInputs.length !== NR_WIRE_INPUTS) {
    throw new Error(
      `wireInputs must have ${NR_WIRE_INPUTS} entries (the wire subset), got ${wireInputs.length}`,
    );
  }
  for (const w of wireInputs) {
    if (w.length !== 32) throw new Error('each wire input must be 32 bytes');
  }
  const buf = new Uint8Array(PROOF_BUFFER_PAYLOAD_SIZE);
  let off = 0;
  buf.set(proof_a, off); off += 64;
  buf.set(proof_b, off); off += 128;
  buf.set(proof_c, off); off += 64;
  buf.set(nullifier, off); off += 32;
  for (const w of wireInputs) {
    buf.set(w, off); off += 32;
  }
  return buf;
}

export function buildInitProofBufferIx(params: {
  payer: PublicKey;
  programId?: PublicKey;
}): TransactionInstruction {
  const programId = params.programId ?? verifierProgramId();
  const [bufferPda] = deriveProofBufferPda(params.payer, programId);
  const disc = anchorDiscriminator('init_proof_buffer');
  return new TransactionInstruction({
    programId,
    keys: [
      { pubkey: bufferPda, isSigner: false, isWritable: true },
      { pubkey: params.payer, isSigner: true, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: disc,
  });
}

export function buildUploadProofChunkIx(params: {
  payer: PublicKey;
  offset: number;
  bytes: Uint8Array;
  programId?: PublicKey;
}): TransactionInstruction {
  const programId = params.programId ?? verifierProgramId();
  const [bufferPda] = deriveProofBufferPda(params.payer, programId);
  const disc = anchorDiscriminator('upload_proof_chunk');
  // Layout: discriminator(8) + offset u32 LE(4) + Vec<u8> length(4) + bytes
  const data = Buffer.alloc(8 + 4 + 4 + params.bytes.length);
  let off = 0;
  disc.copy(data, off); off += 8;
  data.writeUInt32LE(params.offset, off); off += 4;
  data.writeUInt32LE(params.bytes.length, off); off += 4;
  Buffer.from(params.bytes).copy(data, off);
  return new TransactionInstruction({
    programId,
    keys: [
      { pubkey: bufferPda, isSigner: false, isWritable: true },
      { pubkey: params.payer, isSigner: true, isWritable: false },
    ],
    data,
  });
}

export function buildVerifyBatchProofV2Ix(params: {
  payer: PublicKey;
  nullifier: Uint8Array;
  trees: SchemaTreeAccounts;
  programId?: PublicKey;
}): TransactionInstruction {
  const programId = params.programId ?? verifierProgramId();
  if (params.nullifier.length !== 32) throw new Error('nullifier must be 32 bytes');
  const disc = anchorDiscriminator('verify_batch_proof_v2');
  // Layout: discriminator(8) + nullifier_seed [u8;32]
  const data = Buffer.alloc(8 + 32);
  disc.copy(data, 0);
  Buffer.from(params.nullifier).copy(data, 8);

  const [configPda] = deriveVerifierConfigPda(programId);
  const [vkPda] = deriveVkStoragePda(configPda, programId);
  const [nullifierPda] = deriveNullifierPda(params.nullifier, programId);
  const [bufferPda] = deriveProofBufferPda(params.payer, programId);

  // Account order matches `VerifyBatchProofV2` in the on-chain program.
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
    { pubkey: bufferPda, isSigner: false, isWritable: true },
    { pubkey: params.payer, isSigner: true, isWritable: true },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
  ];

  return new TransactionInstruction({ programId, keys, data });
}

// ─── Address Lookup Table helpers (SOLID-SEC-054 / B13 path 3) ───────────
//
// SOLID-SEC-054 has two parts:
//   (1) on-chain reconstruction shrinks `verify_batch_proof` ix data
//       1324 -> 972 bytes (handled in this SDK by NR_WIRE_INPUTS = 21).
//   (3) Address Lookup Table (ALT) collapses the ~11-account list in the
//       tx message header from `11 * 32 = 352` bytes of pubkeys down to
//       11 single-byte indices (~341 bytes saved).
// Both are required: post-(1)-only the legacy-tx wire is still ~1377
// bytes, over the 1232-byte protocol cap.  Post-(1)+(3) the message
// fits with ~135 bytes of headroom.  Doc reference:
// `docs/E2E_BLOCKERS.md` B13 "Recommended path: (1) + (3)".

/** Static accounts that show up in every `verify_batch_proof` ix and
 *  benefit from ALT compression.  Excludes the signer (`payer`) and the
 *  per-call dynamic writable PDA (`nullifier_record`); both must stay
 *  inline.  `verifier_config` is writable but static across calls
 *  (its mutation is just the `proof_count` bump), so it CAN go in the
 *  ALT -- the table's `addresses` array is direction-agnostic, the
 *  writable/readonly split is encoded per-message in the v0 header. */
export interface VerifyBatchProofLookupAddresses {
  verifierConfig: PublicKey;
  vkStorage: PublicKey;
  globalTree: PublicKey;
  schemaTree0: PublicKey;
  schemaTree1: PublicKey;
  schemaTree2: PublicKey;
  schemaTree3: PublicKey;
  issuerTreeBinding: PublicKey;
  systemProgram: PublicKey;
}

/** Collect the lookup-eligible addresses from a `SchemaTreeAccounts`
 *  bundle.  Excludes signer (payer) and the per-proof dynamic
 *  writable PDA (`nullifier_record`).  `verifier_config` and
 *  `vk_storage` are PDAs derived from `programId`; both are static
 *  across all calls and benefit from ALT compression. */
export function lookupAddressesFromTrees(
  trees: SchemaTreeAccounts,
  programId: PublicKey = verifierProgramId(),
): VerifyBatchProofLookupAddresses {
  const [configPda] = deriveVerifierConfigPda(programId);
  const [vkPda] = deriveVkStoragePda(configPda, programId);
  return {
    verifierConfig: configPda,
    vkStorage: vkPda,
    globalTree: trees.globalTree,
    schemaTree0: trees.schemaTree0,
    schemaTree1: trees.schemaTree1,
    schemaTree2: trees.schemaTree2,
    schemaTree3: trees.schemaTree3,
    issuerTreeBinding: trees.issuerTreeBinding,
    systemProgram: SystemProgram.programId,
  };
}

/** Create-or-reuse an Address Lookup Table populated with the given
 *  static addresses.  Idempotent at the "ALT pubkey already valid"
 *  level: callers pass `existing` to short-circuit when an ALT is
 *  already cached in state.  Returns the table's pubkey and the
 *  resolved `AddressLookupTableAccount` shape needed by
 *  `MessageV0.compileToV0Message`.
 *
 *  Solana requires a one-slot warmup between extending an ALT and
 *  using it in a message; this helper enforces a `confirmed`
 *  commitment on the extend tx, which is past warmup by the time the
 *  caller's next tx lands. */
export async function ensureLookupTable(
  connection: Connection,
  payer: Keypair,
  addresses: VerifyBatchProofLookupAddresses,
  existing?: PublicKey,
): Promise<{ pubkey: PublicKey; account: AddressLookupTableAccount }> {
  // Try the existing ALT first.  Reject if it doesn't carry every
  // address we need (defensive: an old ALT from a prior schema set
  // would silently miss accounts and cause `AccountNotFound` at submit).
  const want = [
    addresses.verifierConfig,
    addresses.vkStorage,
    addresses.globalTree,
    addresses.schemaTree0,
    addresses.schemaTree1,
    addresses.schemaTree2,
    addresses.schemaTree3,
    addresses.issuerTreeBinding,
    addresses.systemProgram,
  ];

  if (existing) {
    const got = await connection.getAddressLookupTable(existing);
    if (got.value !== null) {
      const inTable = new Set(got.value.state.addresses.map(a => a.toBase58()));
      const missing = want.filter(a => !inTable.has(a.toBase58()));
      if (missing.length === 0) {
        return { pubkey: existing, account: got.value };
      }
    }
    // Else: cache stale; fall through and create a fresh one.
  }

  // `recentSlot` must be a slot already present in the SlotHistory
  // sysvar at submit time.  Using `'finalized'` commitment guarantees
  // the slot has been rooted; a `'confirmed'` slot can race ahead of
  // the actual chain tip and be rejected with "N is not a recent
  // slot".  This matches the Solana CLI's
  // `program create-address-lookup-table` behavior.
  const slot = await connection.getSlot('finalized');
  const [createIx, lutPubkey] = AddressLookupTableProgram.createLookupTable({
    authority: payer.publicKey,
    payer: payer.publicKey,
    recentSlot: slot,
  });
  const extendIx = AddressLookupTableProgram.extendLookupTable({
    payer: payer.publicKey,
    authority: payer.publicKey,
    lookupTable: lutPubkey,
    addresses: want,
  });
  const tx = new Transaction().add(createIx, extendIx);
  tx.feePayer = payer.publicKey;
  await sendAndConfirmTransaction(connection, tx, [payer], { commitment: 'confirmed' });

  // Capture the slot the extend tx landed at, then wait until the
  // chain advances strictly past it before returning.  Using a v0
  // transaction with `recentBlockhash` from a slot equal to or
  // earlier than the extend slot makes the runtime see the table as
  // empty; the message then references indices that don't yet exist
  // in that slot's view.  Localnet finalization is too slow to use
  // here (single-validator), so we use `confirmed` + an explicit
  // slot-advance gate as the warmup signal.
  const extendSlot = await connection.getSlot('confirmed');
  for (let attempt = 0; attempt < 30; attempt++) {
    const cur = await connection.getSlot('confirmed');
    const fetched = await connection.getAddressLookupTable(lutPubkey, {
      commitment: 'confirmed',
    });
    if (
      cur > extendSlot &&
      fetched.value !== null &&
      fetched.value.state.addresses.length >= want.length
    ) {
      return { pubkey: lutPubkey, account: fetched.value };
    }
    await new Promise(r => setTimeout(r, 400));
  }
  throw new Error(
    `ALT ${lutPubkey.toBase58()} did not warm up with ${want.length} addresses within 12s`,
  );
}

// ─── High-level entry point ───────────────────────────────────────────────

/**
 * Submit a proof to the on-chain ZK verifier program.
 *
 * Returns the real transaction signature from a confirmed `verify_batch_proof`
 * invocation. Throws if verification fails on-chain or the nullifier PDA
 * already exists (replay).
 *
 * `lookupTable` is the SOLID-SEC-054 / B13 path-3 compression: with the
 * ALT account passed, the function builds a `VersionedTransaction` with
 * a `MessageV0` that references the ALT instead of inlining the 11
 * account pubkeys.  Without it the call falls back to a legacy
 * `Transaction`, which only fits when the account list is small (the
 * `verify_batch_proof` shape exceeds the 1232-byte legacy-tx ceiling
 * even after the wire-side reconstruction; see `docs/E2E_BLOCKERS.md`
 * B13).  Production callers must pass an ALT.
 */
export async function verifyOnChain(
  connection: Connection,
  payer: Keypair,
  request: VerificationRequest,
  trees: SchemaTreeAccounts,
  programId: PublicKey = verifierProgramId(),
  lookupTable?: AddressLookupTableAccount,
): Promise<VerificationResult> {
  // SOLID-SEC-054 / B13 Option 2: the legacy `verify_batch_proof`
  // wire is 1269 bytes once cuIx is prepended -- 37 bytes over the
  // 1232-byte legacy-tx cap.  v0+ALT compression doesn't recover that
  // delta because the cuIx itself adds ComputeBudget program key (32)
  // + cuIx data (~8) which must stay in `staticAccountKeys`.  The
  // buffer-account flow (this branch) is the documented escape valve.
  //
  // Set `SOLID_VERIFY_USE_LEGACY=1` to force the pre-Option-2 single-tx
  // path (only useful for the never-shrinks-below-1232 stress case).
  const useLegacy = process.env.SOLID_VERIFY_USE_LEGACY === '1';
  if (!useLegacy) {
    return verifyOnChainV2(connection, payer, request, trees, programId, lookupTable);
  }
  const ix = buildVerifyBatchProofIx({
    payer: payer.publicKey,
    request,
    trees,
    programId,
  });
  // Default per-ix CU budget (200K) is below verify_batch_proof's
  // ~285K-345K baseline.  Prepend a ComputeBudget ix so the
  // verifier has room to run.  See VERIFY_BATCH_PROOF_CU_LIMIT.
  const cuIx = ComputeBudgetProgram.setComputeUnitLimit({
    units: VERIFY_BATCH_PROOF_CU_LIMIT,
  });
  // Optional CU bump.  We attempt v0+ALT first WITHOUT the cuIx --
  // for the small-proof case the actual CU may stay under 200K.  If
  // the validator returns "Computational budget exceeded" we retry
  // with the cuIx prepended (which adds ~40 bytes; the v0 tx surface
  // is already at the ceiling so this MAY exceed 1232 bytes for
  // larger account sets, in which case the buffer-account fallback
  // (B13 Option 2) is required).
  const includeCuIx = process.env.SOLID_VERIFY_INCLUDE_CU_IX === '1';

  let signature: string;
  if (lookupTable) {
    // Versioned-tx + ALT path: the canonical SOLID-SEC-054 / B13 (1)+(3) flow.
    //
    // SOLID-SEC-064 / H6: pre-fix, this path returned `verified: true` even
    // when the on-chain tx reverted, because `connection.sendTransaction`
    // returns the signature regardless of execution result and we did not
    // inspect `confirmTransaction(...).value.err`.  A reverted
    // verify_batch_proof would therefore be reported as success to the
    // caller.  Fix: cache one blockhash; check `value.err` on confirm;
    // throw with the underlying error so the SDK is honest.
    const latest = await connection.getLatestBlockhash('confirmed');
    const blockhash = latest.blockhash;
    const lastValidBlockHeight = latest.lastValidBlockHeight;
    const message = new TransactionMessage({
      payerKey: payer.publicKey,
      recentBlockhash: blockhash,
      instructions: includeCuIx ? [cuIx, ix] : [ix],
    }).compileToV0Message([lookupTable]);
    const vtx = new VersionedTransaction(message);
    vtx.sign([payer]);
    signature = await connection.sendTransaction(vtx, { skipPreflight: false });
    const confirmation = await connection.confirmTransaction(
      { signature, blockhash, lastValidBlockHeight },
      'confirmed',
    );
    if (confirmation.value.err != null) {
      throw new Error(
        `verify_batch_proof v0/ALT tx ${signature} reverted on-chain: ` +
          `${JSON.stringify(confirmation.value.err)} (SOLID-SEC-064 / H6)`,
      );
    }
  } else {
    // Legacy path; only fits for ix surfaces small enough to clear the
    // 1232-byte legacy-tx ceiling without ALT compression.
    // sendAndConfirmTransaction throws on `value.err` already, so this
    // branch is honest by construction.
    const tx = new Transaction();
    if (includeCuIx) tx.add(cuIx);
    tx.add(ix);
    tx.feePayer = payer.publicKey;
    signature = await sendAndConfirmTransaction(connection, tx, [payer], {
      commitment: 'confirmed',
    });
  }

  // Defense-in-depth (SOLID-SEC-064 / H6): post-fetch the nullifier PDA
  // and assert it exists.  An on-chain verify_batch_proof success ALWAYS
  // initialises the nullifier PDA via #[account(init, ...)]; if the PDA
  // is absent post-confirm, something between client and chain lied.
  const [nullifierPda] = deriveNullifierPda(request.proofData.nullifier);
  const nullifierInfo = await connection.getAccountInfo(nullifierPda);
  if (nullifierInfo == null) {
    throw new Error(
      `verify_batch_proof tx ${signature} confirmed but nullifier PDA ` +
        `${nullifierPda.toBase58()} was not initialised; treating as ` +
        `unverified (SOLID-SEC-064 / H6 defense-in-depth).`,
    );
  }

  return {
    verified: true,
    nullifier: request.proofData.nullifier,
    transactionSignature: signature,
    timestamp: Math.floor(Date.now() / 1000),
  };
}

/**
 * SOLID-SEC-054 / B13 Option 2: orchestrate the 3-tx (or 4-tx) flow
 * that uploads the proof to a per-payer scratch PDA and verifies from
 * it.  Each tx fits well under the 1232-byte legacy-tx cap.
 *
 * Lifecycle:
 *   1. init_proof_buffer        (allocates ProofBuffer; rent-funded by payer)
 *   2. upload_proof_chunk * N   (default: 2 chunks of 480 bytes)
 *   3. verify_batch_proof_v2    (verifies + closes buffer; refunds rent)
 *
 * Idempotent re-uploads: if an init exists from a prior failed attempt,
 * we treat it as reusable; if any chunk upload fails we throw.
 */
async function verifyOnChainV2(
  connection: Connection,
  payer: Keypair,
  request: VerificationRequest,
  trees: SchemaTreeAccounts,
  programId: PublicKey = verifierProgramId(),
  lookupTable?: AddressLookupTableAccount,
): Promise<VerificationResult> {
  const { proof_a, proof_b, proof_c, publicInputs, nullifier } =
    request.proofData;
  if (publicInputs.length !== NR_WIRE_INPUTS) {
    throw new Error(
      `verifyOnChainV2: publicInputs must have ${NR_WIRE_INPUTS} entries (the wire subset), got ${publicInputs.length}.`,
    );
  }
  const payload = encodeProofBufferPayload(
    proof_a,
    proof_b,
    proof_c,
    nullifier,
    publicInputs,
  );

  const [bufferPda] = deriveProofBufferPda(payer.publicKey, programId);

  // Step 1: init_proof_buffer (only if not already allocated for this payer).
  const existing = await connection.getAccountInfo(bufferPda);
  if (existing == null) {
    const initIx = buildInitProofBufferIx({ payer: payer.publicKey, programId });
    const tx = new Transaction().add(initIx);
    tx.feePayer = payer.publicKey;
    await sendAndConfirmTransaction(connection, tx, [payer], {
      commitment: 'confirmed',
    });
  }

  // Step 2: upload_proof_chunk * N.  Slice payload by DEFAULT_PROOF_CHUNK_SIZE.
  const chunkSize = Number(
    process.env.SOLID_PROOF_CHUNK_SIZE ?? String(DEFAULT_PROOF_CHUNK_SIZE),
  );
  for (let off = 0; off < payload.length; off += chunkSize) {
    const slice = payload.subarray(off, Math.min(off + chunkSize, payload.length));
    const ix = buildUploadProofChunkIx({
      payer: payer.publicKey,
      offset: off,
      bytes: slice,
      programId,
    });
    const tx = new Transaction().add(ix);
    tx.feePayer = payer.publicKey;
    await sendAndConfirmTransaction(connection, tx, [payer], {
      commitment: 'confirmed',
    });
  }

  // Step 3: verify_batch_proof_v2 (with cuIx + ALT).
  const cuIx = ComputeBudgetProgram.setComputeUnitLimit({
    units: VERIFY_BATCH_PROOF_CU_LIMIT,
  });
  const verifyIx = buildVerifyBatchProofV2Ix({
    payer: payer.publicKey,
    nullifier,
    trees,
    programId,
  });

  let signature: string;
  if (lookupTable) {
    const latest = await connection.getLatestBlockhash('confirmed');
    const message = new TransactionMessage({
      payerKey: payer.publicKey,
      recentBlockhash: latest.blockhash,
      instructions: [cuIx, verifyIx],
    }).compileToV0Message([lookupTable]);
    const vtx = new VersionedTransaction(message);
    vtx.sign([payer]);
    signature = await connection.sendTransaction(vtx, { skipPreflight: false });
    const confirmation = await connection.confirmTransaction(
      {
        signature,
        blockhash: latest.blockhash,
        lastValidBlockHeight: latest.lastValidBlockHeight,
      },
      'confirmed',
    );
    if (confirmation.value.err != null) {
      throw new Error(
        `verify_batch_proof_v2 v0/ALT tx ${signature} reverted on-chain: ` +
          `${JSON.stringify(confirmation.value.err)} (SOLID-SEC-054 / B13 Option 2)`,
      );
    }
  } else {
    const tx = new Transaction().add(cuIx).add(verifyIx);
    tx.feePayer = payer.publicKey;
    signature = await sendAndConfirmTransaction(connection, tx, [payer], {
      commitment: 'confirmed',
    });
  }

  // Defense-in-depth (mirrors the legacy path): assert the nullifier PDA
  // got initialised.  An on-chain verify_batch_proof_v2 success ALWAYS
  // does this; absence post-confirm is an integration anomaly.
  const [nullifierPda] = deriveNullifierPda(nullifier, programId);
  const nullifierInfo = await connection.getAccountInfo(nullifierPda);
  if (nullifierInfo == null) {
    throw new Error(
      `verify_batch_proof_v2 tx ${signature} confirmed but nullifier PDA ` +
        `${nullifierPda.toBase58()} was not initialised (SOLID-SEC-064 / H6 defense-in-depth).`,
    );
  }

  return {
    verified: true,
    nullifier,
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
