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
  clusterApiUrl,
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
  MAX_PREDICATES,
  NUM_FIELDS,
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
 * Anchor account-discriminator for `IssuerAccount`.
 * Equals `sha256("account:IssuerAccount")[..8]`.  M7 / SOLID-SEC-075
 * (closed 2026-05-01) regression gate: pin at module load + reject
 * mismatched bytes before the parser walks the body.
 */
const ISSUER_ACCOUNT_DISCRIMINATOR: Buffer = createHash('sha256')
  .update('account:IssuerAccount')
  .digest()
  .subarray(0, 8);

/**
 * Check if an issuer is approved in the DAO registry.
 *
 * Reads the `IssuerAccount` PDA directly.  Validates the 8-byte Anchor
 * discriminator before decoding to refuse arbitrary same-size accounts
 * (M7 / SOLID-SEC-075, sibling class to M11 SOLID-SEC-073).  Body
 * decoded as: 32-byte authority + (4+n) name + (4+m) metadata_uri +
 * Ax/Ay + tier + status + stake -- enough to answer the common
 * question without pulling in `@coral-xyz/anchor`'s full
 * BorshAccountsCoder for one read.
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

  // M7 / SOLID-SEC-075: validate Anchor discriminator BEFORE walking the
  // body.  Pre-fix the parser blindly trusted the first 8 bytes were
  // an IssuerAccount discriminator and decoded the body regardless;
  // an attacker could plant a system-owned account at the same
  // address (impossible if owner-checks land elsewhere, but the gate
  // belongs at the SDK boundary too) with arbitrary bytes and the
  // SDK would happily report a synthetic "approved" status.
  if (info.data.length < 8 || !ISSUER_ACCOUNT_DISCRIMINATOR.equals(info.data.subarray(0, 8))) {
    return { approved: false, name: '', stakedAmount: 0n, tier: 0, status: 0 };
  }
  // Defence-in-depth: also assert the program owner.
  if (!info.owner.equals(registryProgramId)) {
    return { approved: false, name: '', stakedAmount: 0n, tier: 0, status: 0 };
  }

  // Body: skip 8-byte discriminator + 32-byte authority.
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

// ─── Product-level verifier SDK ─────────────────────────────────────────────

export type SolidCluster = 'devnet' | 'mainnet-beta' | 'mainnet' | 'localnet';
export type SchemaRef = string | { name: string; version: number } | { hash: string; name?: string; version?: number };
export type PredicateOp = '==' | '!=' | '>' | '>=' | '<' | '<=' | 'EQ' | 'NE' | 'GT' | 'GTE' | 'LT' | 'LTE';
export type PredicateValue = number | bigint | boolean;

export interface RequirementPredicate {
  field: string;
  op: PredicateOp;
  value: PredicateValue;
}

export interface ActionScope {
  appId: string;
  action: string;
  nonce?: Uint8Array | string;
}

export interface RequirementSpec {
  schema: SchemaRef;
  predicates: RequirementPredicate[];
  compoundLogic?: 'AND' | 'OR';
  action: ActionScope | string;
  expiresAt?: number;
  trustedIssuers?: PublicKey[];
}

export interface SchemaFieldDescriptor {
  name: string;
  index: number;
  type?: string;
  description?: string;
  rangeQueryable?: boolean;
}

export interface SchemaDescriptor {
  name: string;
  version: number;
  category?: string;
  hash: string;
  fields: SchemaFieldDescriptor[];
}

export interface RequirementPredicateEncoding {
  field: string;
  fieldIndex: number;
  op: Exclude<PredicateOp, '==' | '!=' | '>=' | '<=' | '>' | '<'>;
  value: string;
}

export interface PublicInputBlueprint {
  schemaHash: string;
  predicateEncodings: RequirementPredicateEncoding[];
  compoundLogic: 'AND' | 'OR';
  verifierAddress: string;
  verifierNonce: string;
  expiresAt: number;
}

export interface Requirement {
  readonly spec: RequirementSpec;
  readonly fingerprint: string;
  readonly schema: SchemaDescriptor;
  readonly schemaHash: string;
  readonly verifierAddress: PublicKey;
  readonly publicInputs: PublicInputBlueprint;
}

export type VerificationError =
  | 'MISSING_CREDENTIAL'
  | 'EXPIRED_CREDENTIAL'
  | 'REVOKED_CREDENTIAL'
  | 'REVOKED_ISSUER'
  | 'UNSUPPORTED_SCHEMA'
  | 'PREDICATE_NOT_SATISFIED'
  | 'USER_REJECTED'
  | 'PROOF_GENERATION_FAILED'
  | 'PROOF_REPLAYED'
  | 'ARTIFACT_PIN_MISMATCH'
  | 'VK_FROZEN'
  | 'INVALID_PUBLIC_INPUTS'
  | 'RPC_UNAVAILABLE'
  | 'TRANSACTION_TIMEOUT'
  | 'INSUFFICIENT_PAYER_BALANCE'
  | 'UNKNOWN';

export interface VerificationFailure {
  verified: false;
  reason: VerificationError;
  detail?: string;
  signature?: string;
}

export interface VerificationSuccess {
  verified: true;
  signature: string;
  nullifier: string;
  slot?: number;
  issuer?: { pubkey: PublicKey; status: number; name?: string };
  schema: { hash: string; ref: SchemaRef };
}

export type SolidVerificationResult = VerificationSuccess | VerificationFailure;

export class SolidVerificationError extends Error {
  readonly reason: VerificationError;
  readonly detail?: string;

  constructor(reason: VerificationError, detail?: string) {
    super(detail == null ? reason : `${reason}: ${detail}`);
    this.name = 'SolidVerificationError';
    this.reason = reason;
    this.detail = detail;
  }
}

export interface SerializedProof {
  proof?: unknown;
  publicSignals: Array<string | number | bigint | Uint8Array>;
  nullifier?: string | Uint8Array;
  solanaProof?: {
    proofA?: Uint8Array | string | number[];
    proofB?: Uint8Array | string | number[];
    proofC?: Uint8Array | string | number[];
    proof_a?: Uint8Array | string | number[];
    proof_b?: Uint8Array | string | number[];
    proof_c?: Uint8Array | string | number[];
  };
  proofA?: Uint8Array | string | number[];
  proofB?: Uint8Array | string | number[];
  proofC?: Uint8Array | string | number[];
  proof_a?: Uint8Array | string | number[];
  proof_b?: Uint8Array | string | number[];
  proof_c?: Uint8Array | string | number[];
}

export interface ProofRequestEnvelope {
  version: 1;
  requirement: Requirement;
  artifactPins: ResolvedArtifactPins;
  expiresAt: number;
}

export interface ProofRequestTransport {
  send(request: ProofRequestEnvelope): Promise<SerializedProof>;
}

export interface ProofRequestHandle {
  request: ProofRequestEnvelope;
  proof: Promise<SerializedProof>;
  cancel(): void;
}

export interface ResolvedArtifactPins {
  batchWasm: string;
  batchZkey: string;
  batchVk: string;
  subgroupWasm: string;
  subgroupZkey: string;
  subgroupVk: string;
}

export interface HealthReport {
  ok: boolean;
  cluster: SolidCluster;
  programs: {
    zkVerifier: boolean;
    issuerRegistry: boolean;
    schemaRegistry: boolean;
  };
  verifierConfig?: {
    exists: boolean;
    vkFinalized?: boolean;
    vkGeneration?: number;
  };
  artifactPins: ResolvedArtifactPins;
  artifactHost?: { ok: boolean; url: string; detail?: string };
  indexer?: { ok: boolean; url: string; detail?: string };
  errors: string[];
}

export interface SolidVerifierConfig {
  cluster: SolidCluster;
  connection?: Connection;
  programIds?: Partial<{
    zkVerifier: PublicKey;
    issuerRegistry: PublicKey;
    schemaRegistry: PublicKey;
  }>;
  artifactPins?: Partial<ResolvedArtifactPins>;
  artifactHostUrl?: string;
  indexerUrl?: string;
  requestTimeoutMs?: number;
  proofRequestTtlMs?: number;
  trustedIssuers?: PublicKey[];
  trustedSchemas?: SchemaRef[];
  schemaCatalog?: SchemaDescriptor[];
}

export interface VerifyProofArgs {
  proof: SerializedProof;
  requirement: Requirement;
  payer: Keypair;
  lookupTable?: AddressLookupTableAccount;
}

export interface RequestProofArgs {
  wallet: PublicKey;
  requirement: Requirement;
  transport: ProofRequestTransport;
  timeoutMs?: number;
}

export interface VerifyRequirementArgs {
  wallet: PublicKey;
  payer: Keypair;
  requirement?: Requirement;
  spec?: RequirementSpec;
  transport: ProofRequestTransport;
  timeoutMs?: number;
  lookupTable?: AddressLookupTableAccount;
}

const DEFAULT_ARTIFACT_HOST = '';
const DEFAULT_INDEXER_URL = undefined;

const DEFAULT_ARTIFACT_PINS: ResolvedArtifactPins = {
  batchVk: '8385b82b032f65e505c784b28486ca8bec7da3f3d4b97b82724e697734565146',
  batchWasm: 'add8cb0390511405faf2ffb1213d3792b858c7b2082d5b4b92a84dcd627e61b4',
  batchZkey: '7f43bbac249e8c913ac384ff4b007138d1ffb5bc489a16be42737598d96395e8',
  subgroupVk: '938ab39020f31156fa7e8fc230fc458adba5f13e08c64d41d9dbffdbc3643ce9',
  subgroupWasm: '2c00e5a455a3b6fe1dc2a8761737acf2911baf396aab70165de3abd2673608b0',
  subgroupZkey: 'ea401ea9cbeb9ef829be30e0080b83a3c82544af53367b95a17f24a75845bd4a',
};

const DEFAULT_SCHEMA_CATALOG: SchemaDescriptor[] = [
  {
    name: 'basic_identity_v1',
    version: 1,
    category: 'Hospitality',
    hash: 'c27b4c5f3e75c3f0320d02298d06d689a2d3c7e973fbbd23022c567e37bb8306',
    fields: [
      { index: 0, name: 'age', type: 'uint64', rangeQueryable: true, description: 'Age in years' },
      { index: 1, name: 'country_code', type: 'uint64', description: 'ISO 3166-1 numeric country code' },
      { index: 2, name: 'resident_region', type: 'uint64', description: 'Region code' },
      { index: 3, name: 'id_type', type: 'enum', description: 'Identity document type' },
      { index: 4, name: 'verification_level', type: 'uint64', rangeQueryable: true, description: '1=Self, 2=KYC, 3=InPerson' },
      { index: 5, name: 'issued_date', type: 'timestamp', rangeQueryable: true, description: 'Issuance timestamp' },
      { index: 6, name: 'nationality', type: 'uint64', description: 'Nationality code' },
      { index: 7, name: '_reserved', type: 'uint64', description: 'Reserved for future use' },
    ],
  },
];

const ARTIFACT_PIN_ENV: Record<keyof ResolvedArtifactPins, string> = {
  batchWasm: 'SOLID_CIRCUIT_WASM_SHA256',
  batchZkey: 'SOLID_CIRCUIT_ZKEY_SHA256',
  batchVk: 'SOLID_VK_SHA256',
  subgroupWasm: 'SOLID_SUBGROUP_WASM_SHA256',
  subgroupZkey: 'SOLID_SUBGROUP_ZKEY_SHA256',
  subgroupVk: 'SOLID_SUBGROUP_VK_SHA256',
};

const ARTIFACT_SIDECARS: Record<keyof ResolvedArtifactPins, string> = {
  batchWasm: 'circuits/build/batch_credential_query.wasm.sha256',
  batchZkey: 'circuits/build/batch_credential_query.zkey.sha256',
  batchVk: 'circuits/build/verification_key.sha256',
  subgroupWasm: 'circuits/build/bjj_subgroup_proof.wasm.sha256',
  subgroupZkey: 'circuits/build/bjj_subgroup_proof.zkey.sha256',
  subgroupVk: 'circuits/build/bjj_subgroup_verification_key.sha256',
};

const ARTIFACT_FILENAMES: Record<keyof ResolvedArtifactPins, string> = {
  batchWasm: 'batch_credential_query.wasm',
  batchZkey: 'batch_credential_query.zkey',
  batchVk: 'verification_key.json',
  subgroupWasm: 'bjj_subgroup_proof.wasm',
  subgroupZkey: 'bjj_subgroup_proof.zkey',
  subgroupVk: 'bjj_subgroup_verification_key.json',
};

export class SolidVerifier {
  readonly connection: Connection;
  readonly cluster: SolidCluster;
  readonly programIds: {
    zkVerifier: PublicKey;
    issuerRegistry: PublicKey;
    schemaRegistry: PublicKey;
  };
  readonly artifactHostUrl: string;
  readonly indexerUrl?: string;
  readonly artifactPins: ResolvedArtifactPins;
  private readonly schemaCatalog: SchemaDescriptor[];
  private readonly requestTimeoutMs: number;
  private readonly proofRequestTtlMs: number;

  constructor(config: SolidVerifierConfig) {
    this.cluster = config.cluster;
    this.connection = config.connection ?? new Connection(endpointForCluster(config.cluster), 'confirmed');
    this.programIds = {
      zkVerifier: config.programIds?.zkVerifier ?? new PublicKey(PROGRAM_IDS.zkVerifier),
      issuerRegistry: config.programIds?.issuerRegistry ?? new PublicKey(PROGRAM_IDS.issuerRegistry),
      schemaRegistry: config.programIds?.schemaRegistry ?? new PublicKey(PROGRAM_IDS.schemaRegistry),
    };
    this.artifactHostUrl = config.artifactHostUrl
      ? normalizeBaseUrl(config.artifactHostUrl)
      : DEFAULT_ARTIFACT_HOST;
    this.indexerUrl = config.indexerUrl ?? DEFAULT_INDEXER_URL;
    this.artifactPins = resolveArtifactPins(config.artifactPins);
    this.schemaCatalog = [...DEFAULT_SCHEMA_CATALOG, ...(config.schemaCatalog ?? [])];
    this.requestTimeoutMs = config.requestTimeoutMs ?? 5 * 60 * 1000;
    this.proofRequestTtlMs = config.proofRequestTtlMs ?? 5 * 60 * 1000;
  }

  defineRequirement(spec: RequirementSpec): Requirement {
    const schema = this.resolveSchema(spec.schema);
    const predicates = normalizePredicates(spec.predicates, schema);
    const verifierNonce = normalizeActionNonce(spec.action);
    const action = normalizeAction(spec.action);
    const canonical = {
      schemaHash: schema.hash,
      predicates,
      compoundLogic: spec.compoundLogic ?? 'AND',
      action: {
        appId: action.appId,
        action: action.action,
        nonce: bytesToHex(verifierNonce),
      },
      expiresAt: spec.expiresAt ?? 0,
      trustedIssuers: (spec.trustedIssuers ?? []).map(k => k.toBase58()).sort(),
    };
    const fingerprint = createHash('sha256')
      .update(stableJson(canonical))
      .digest('hex');

    return {
      spec,
      fingerprint,
      schema,
      schemaHash: schema.hash,
      verifierAddress: this.programIds.zkVerifier,
      publicInputs: {
        schemaHash: schema.hash,
        predicateEncodings: predicates,
        compoundLogic: spec.compoundLogic ?? 'AND',
        verifierAddress: this.programIds.zkVerifier.toBase58(),
        verifierNonce: bytesToHex(verifierNonce),
        expiresAt: spec.expiresAt ?? 0,
      },
    };
  }

  async requestProof(args: RequestProofArgs): Promise<SerializedProof> {
    const proof = this.requestProofHandle(args).proof;
    return proof;
  }

  requestProofHandle(args: RequestProofArgs): ProofRequestHandle {
    const controller = new AbortController();
    const request: ProofRequestEnvelope = {
      version: 1,
      requirement: args.requirement,
      artifactPins: this.artifactPins,
      expiresAt: Date.now() + this.proofRequestTtlMs,
    };
    const timeoutMs = args.timeoutMs ?? this.requestTimeoutMs;
    const timer = setTimeout(() => controller.abort(), timeoutMs);
    const proof = Promise.race([
      args.transport.send(request),
      new Promise<SerializedProof>((_, reject) => {
        controller.signal.addEventListener('abort', () => {
          reject(new SolidVerificationError('TRANSACTION_TIMEOUT', `Proof request timed out after ${timeoutMs}ms`));
        });
      }),
    ]).finally(() => clearTimeout(timer));

    return {
      request,
      proof,
      cancel: () => controller.abort(),
    };
  }

  async verifyProof(args: VerifyProofArgs): Promise<SolidVerificationResult> {
    try {
      const normalized = normalizeSerializedProof(args.proof);
      validatePublicSignals(args.requirement, normalized.fullPublicInputs);
      const [nullifierPda] = deriveNullifierPda(normalized.nullifier, this.programIds.zkVerifier);
      const existingNullifier = await this.connection.getAccountInfo(nullifierPda);
      if (existingNullifier != null) {
        return {
          verified: false,
          reason: 'PROOF_REPLAYED',
          detail: `Nullifier ${bytesToHex(normalized.nullifier)} has already been used.`,
        };
      }

      const trees = deriveTreesFromPublicInputs(
        normalized.fullPublicInputs,
        this.programIds.schemaRegistry,
        this.programIds.issuerRegistry,
      );
      const request: VerificationRequest = {
        query: buildQueryFromRequirement(args.requirement, normalized.fullPublicInputs),
        proofData: {
          proof_a: normalized.proofA,
          proof_b: normalized.proofB,
          proof_c: normalized.proofC,
          publicInputs: extractWirePublicInputs(normalized.fullPublicInputs),
          nullifier: normalized.nullifier,
        },
      };
      const lowLevel = await verifyOnChain(
        this.connection,
        args.payer,
        request,
        trees,
        this.programIds.zkVerifier,
        args.lookupTable,
      );
      const status = await this.connection.getSignatureStatuses([lowLevel.transactionSignature]);
      return {
        verified: true,
        signature: lowLevel.transactionSignature,
        nullifier: bytesToHex(lowLevel.nullifier),
        slot: status.value[0]?.slot,
        schema: { hash: args.requirement.schemaHash, ref: args.requirement.spec.schema },
      };
    } catch (err) {
      return normalizeVerificationFailure(err);
    }
  }

  async verifyRequirement(args: VerifyRequirementArgs): Promise<SolidVerificationResult> {
    const requirement = args.requirement ?? this.defineRequirement(requiredSpec(args.spec));
    try {
      const proof = await this.requestProof({
        wallet: args.wallet,
        requirement,
        transport: args.transport,
        timeoutMs: args.timeoutMs,
      });
      return this.verifyProof({
        proof,
        requirement,
        payer: args.payer,
        lookupTable: args.lookupTable,
      });
    } catch (err) {
      return normalizeVerificationFailure(err);
    }
  }

  async health(): Promise<HealthReport> {
    const errors: string[] = [];
    const [zk, issuer, schema] = await Promise.all([
      this.connection.getAccountInfo(this.programIds.zkVerifier).catch((err) => {
        errors.push(`zkVerifier RPC error: ${String(err)}`);
        return null;
      }),
      this.connection.getAccountInfo(this.programIds.issuerRegistry).catch((err) => {
        errors.push(`issuerRegistry RPC error: ${String(err)}`);
        return null;
      }),
      this.connection.getAccountInfo(this.programIds.schemaRegistry).catch((err) => {
        errors.push(`schemaRegistry RPC error: ${String(err)}`);
        return null;
      }),
    ]);

    const [configPda] = deriveVerifierConfigPda(this.programIds.zkVerifier);
    const configInfo = await this.connection.getAccountInfo(configPda).catch((err) => {
      errors.push(`verifier config RPC error: ${String(err)}`);
      return null;
    });
    const verifierConfig = configInfo == null
      ? { exists: false }
      : {
          exists: true,
          vkFinalized: configInfo.data[50] === 1,
          vkGeneration: configInfo.data.length >= 53 ? configInfo.data.readUInt16LE(51) : undefined,
        };

    const artifactHost = await pingJson(`${this.artifactHostUrl}.well-known/solid-protocol.json`);
    if (!artifactHost.ok) errors.push(`artifact host unavailable: ${artifactHost.detail ?? artifactHost.url}`);
    const indexer = this.indexerUrl ? await pingJson(`${trimTrailingSlash(this.indexerUrl)}/v1/health`) : undefined;
    if (indexer && !indexer.ok) errors.push(`indexer unavailable: ${indexer.detail ?? indexer.url}`);

    const programs = {
      zkVerifier: zk?.executable === true,
      issuerRegistry: issuer?.executable === true,
      schemaRegistry: schema?.executable === true,
    };
    return {
      ok: programs.zkVerifier && programs.issuerRegistry && programs.schemaRegistry && verifierConfig.exists && errors.length === 0,
      cluster: this.cluster,
      programs,
      verifierConfig,
      artifactPins: this.artifactPins,
      artifactHost,
      indexer,
      errors,
    };
  }

  async loadArtifact(kind: keyof ResolvedArtifactPins): Promise<Uint8Array> {
    const url = `${this.artifactHostUrl}${ARTIFACT_FILENAMES[kind]}`;
    const res = await fetch(url);
    if (!res.ok) {
      throw new SolidVerificationError('RPC_UNAVAILABLE', `Failed to fetch ${url}: HTTP ${res.status}`);
    }
    const bytes = new Uint8Array(await res.arrayBuffer());
    const actual = createHash('sha256').update(bytes).digest('hex');
    const expected = this.artifactPins[kind];
    if (actual !== expected) {
      throw new SolidVerificationError(
        'ARTIFACT_PIN_MISMATCH',
        `${kind} SHA-256 mismatch: expected ${expected}, got ${actual}`,
      );
    }
    return bytes;
  }

  private resolveSchema(ref: SchemaRef): SchemaDescriptor {
    if (typeof ref === 'object' && 'hash' in ref) {
      const hash = normalizeHex32(ref.hash, 'schema.hash');
      return this.schemaCatalog.find(s => s.hash === hash) ?? {
        name: ref.name ?? hash,
        version: ref.version ?? 0,
        hash,
        fields: [],
      };
    }
    const name = typeof ref === 'string' ? ref : ref.name;
    const version = typeof ref === 'string' ? undefined : ref.version;
    const schema = this.schemaCatalog.find(s =>
      s.name === name && (version == null || s.version === version),
    );
    if (!schema) {
      throw new SolidVerificationError(
        'UNSUPPORTED_SCHEMA',
        `Schema ${typeof ref === 'string' ? ref : `${ref.name}@${ref.version}`} is not in this verifier's schema catalog. ` +
          `Pass schema: { hash } or add schemaCatalog to SolidVerifier config.`,
      );
    }
    return schema;
  }
}

export function walletAdapterTransport(wallet: unknown): ProofRequestTransport {
  return {
    async send(request: ProofRequestEnvelope): Promise<SerializedProof> {
      const maybeWallet = wallet as {
        requestProof?: (request: ProofRequestEnvelope) => Promise<SerializedProof>;
        solid?: { requestProof?: (request: ProofRequestEnvelope) => Promise<SerializedProof> };
      };
      if (typeof maybeWallet.requestProof === 'function') {
        return maybeWallet.requestProof(request);
      }
      if (typeof maybeWallet.solid?.requestProof === 'function') {
        return maybeWallet.solid.requestProof(request);
      }
      const solid = (globalThis as typeof globalThis & {
        window?: { solid?: { requestProof?: (request: ProofRequestEnvelope) => Promise<SerializedProof> } };
      }).window?.solid;
      if (typeof solid?.requestProof === 'function') {
        return solid.requestProof(request);
      }
      throw new SolidVerificationError(
        'MISSING_CREDENTIAL',
        'No SolID proof transport is available. Pass walletAdapterTransport(walletWithSolID), httpTransport(...), or a custom transport.',
      );
    },
  };
}

export function httpTransport(endpoint: string, init?: RequestInit): ProofRequestTransport {
  return {
    async send(request: ProofRequestEnvelope): Promise<SerializedProof> {
      const res = await fetch(endpoint, {
        ...init,
        method: init?.method ?? 'POST',
        headers: {
          'content-type': 'application/json',
          ...(init?.headers ?? {}),
        },
        body: JSON.stringify(request),
      });
      if (!res.ok) {
        throw new SolidVerificationError('RPC_UNAVAILABLE', `Holder endpoint ${endpoint} returned HTTP ${res.status}`);
      }
      return res.json() as Promise<SerializedProof>;
    },
  };
}

export function explainVerificationError(
  err: VerificationError,
  detail?: string,
): { humanReadable: string; suggestedAction: string } {
  const table: Record<VerificationError, { humanReadable: string; suggestedAction: string }> = {
    MISSING_CREDENTIAL: {
      humanReadable: 'This wallet does not have a credential that can satisfy the requirement.',
      suggestedAction: 'Ask the user to claim the required credential, then try again.',
    },
    EXPIRED_CREDENTIAL: {
      humanReadable: 'The matching credential is expired.',
      suggestedAction: 'Ask the user to refresh or re-issue the credential.',
    },
    REVOKED_CREDENTIAL: {
      humanReadable: 'The credential has been revoked.',
      suggestedAction: 'Ask the user to contact the issuer or claim a fresh credential.',
    },
    REVOKED_ISSUER: {
      humanReadable: 'The credential issuer is no longer trusted by the registry.',
      suggestedAction: 'Use a credential from an approved issuer.',
    },
    UNSUPPORTED_SCHEMA: {
      humanReadable: 'The verifier does not recognize this credential schema.',
      suggestedAction: 'Add the schema to the verifier catalog or choose a supported requirement.',
    },
    PREDICATE_NOT_SATISFIED: {
      humanReadable: 'The wallet has a credential, but it does not satisfy the requested predicates.',
      suggestedAction: 'Show the user which eligibility condition failed.',
    },
    USER_REJECTED: {
      humanReadable: 'The user rejected the proof request.',
      suggestedAction: 'Let the user retry when they are ready.',
    },
    PROOF_GENERATION_FAILED: {
      humanReadable: 'The holder could not generate a proof.',
      suggestedAction: 'Check holder artifacts, credentials, and Merkle proof availability.',
    },
    PROOF_REPLAYED: {
      humanReadable: 'This proof has already been used.',
      suggestedAction: 'Request a fresh proof with a fresh verifier nonce.',
    },
    ARTIFACT_PIN_MISMATCH: {
      humanReadable: 'A prover or verifier artifact failed its SHA-256 integrity check.',
      suggestedAction: 'Do not continue. Refresh artifacts from the canonical SolID release.',
    },
    VK_FROZEN: {
      humanReadable: 'The verifier is in a verification-key rotation window.',
      suggestedAction: 'Retry after the rotation finalizes.',
    },
    INVALID_PUBLIC_INPUTS: {
      humanReadable: 'The proof public inputs do not match the requirement.',
      suggestedAction: 'Regenerate the proof from the holder using the exact requirement envelope.',
    },
    RPC_UNAVAILABLE: {
      humanReadable: 'The Solana RPC, holder endpoint, artifact host, or indexer is unavailable.',
      suggestedAction: 'Retry with a healthy endpoint or check SolID devnet status.',
    },
    TRANSACTION_TIMEOUT: {
      humanReadable: 'The proof request or verification transaction timed out.',
      suggestedAction: 'Retry, or increase the timeout for slow wallets/RPCs.',
    },
    INSUFFICIENT_PAYER_BALANCE: {
      humanReadable: 'The verifier payer does not have enough SOL for fees and rent.',
      suggestedAction: 'Fund the payer and retry.',
    },
    UNKNOWN: {
      humanReadable: 'Verification failed for an unexpected reason.',
      suggestedAction: 'Log the detail string and report it if it persists.',
    },
  };
  const value = table[err];
  return detail == null ? value : {
    humanReadable: value.humanReadable,
    suggestedAction: `${value.suggestedAction} Detail: ${detail}`,
  };
}

function endpointForCluster(cluster: SolidCluster): string {
  if (cluster === 'localnet') return 'http://127.0.0.1:8899';
  if (cluster === 'mainnet') return clusterApiUrl('mainnet-beta');
  return clusterApiUrl(cluster);
}

function normalizeBaseUrl(url: string): string {
  return url.endsWith('/') ? url : `${url}/`;
}

function trimTrailingSlash(url: string): string {
  return url.endsWith('/') ? url.slice(0, -1) : url;
}

function readSidecar(pathName: string): string | undefined {
  try {
    // Optional Node-only sidecar support. Browser builds fall back to env/default pins.
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const fs = require('fs') as typeof import('fs');
    if (!fs.existsSync(pathName)) return undefined;
    return fs.readFileSync(pathName, 'utf8').trim().toLowerCase();
  } catch {
    return undefined;
  }
}

function resolveArtifactPins(overrides: Partial<ResolvedArtifactPins> | undefined): ResolvedArtifactPins {
  const result = { ...DEFAULT_ARTIFACT_PINS, ...(overrides ?? {}) };
  for (const key of Object.keys(result) as Array<keyof ResolvedArtifactPins>) {
    const env = process.env[ARTIFACT_PIN_ENV[key]]?.trim().toLowerCase();
    const sidecar = readSidecar(ARTIFACT_SIDECARS[key]);
    result[key] = normalizeHex32(env || sidecar || result[key], key);
  }
  return result;
}

function normalizePredicates(predicates: RequirementPredicate[], schema: SchemaDescriptor): RequirementPredicateEncoding[] {
  if (predicates.length === 0 || predicates.length > MAX_PREDICATES) {
    throw new SolidVerificationError('INVALID_PUBLIC_INPUTS', `Requirement must have 1-${MAX_PREDICATES} predicates.`);
  }
  return predicates.map((pred) => {
    const field = schema.fields.find(f => f.name === pred.field);
    if (!field) {
      throw new SolidVerificationError('UNSUPPORTED_SCHEMA', `Field "${pred.field}" is not present in schema ${schema.name}.`);
    }
    if (field.index < 0 || field.index >= NUM_FIELDS) {
      throw new SolidVerificationError('UNSUPPORTED_SCHEMA', `Field "${pred.field}" has invalid index ${field.index}.`);
    }
    return {
      field: pred.field,
      fieldIndex: field.index,
      op: normalizeOp(pred.op),
      value: predicateValueToString(pred.value),
    };
  });
}

function normalizeOp(op: PredicateOp): RequirementPredicateEncoding['op'] {
  const mapped = ({
    '==': 'EQ',
    '!=': 'NE',
    '>': 'GT',
    '>=': 'GTE',
    '<': 'LT',
    '<=': 'LTE',
    EQ: 'EQ',
    NE: 'NE',
    GT: 'GT',
    GTE: 'GTE',
    LT: 'LT',
    LTE: 'LTE',
  } as const)[op];
  if (!mapped) throw new SolidVerificationError('INVALID_PUBLIC_INPUTS', `Unsupported predicate operator: ${op}`);
  return mapped;
}

function predicateValueToString(value: PredicateValue): string {
  if (typeof value === 'boolean') return value ? '1' : '0';
  return BigInt(value).toString();
}

function normalizeAction(action: ActionScope | string): ActionScope {
  return typeof action === 'string' ? { appId: 'solid-app', action } : action;
}

function normalizeActionNonce(action: ActionScope | string): Uint8Array {
  const normalized = normalizeAction(action);
  if (normalized.nonce instanceof Uint8Array) {
    if (normalized.nonce.length !== 32) throw new SolidVerificationError('INVALID_PUBLIC_INPUTS', 'action.nonce must be 32 bytes.');
    return normalized.nonce;
  }
  if (typeof normalized.nonce === 'string') {
    return hexToBytes(normalizeHex32(normalized.nonce, 'action.nonce'));
  }
  return generateVerifierNonce();
}

function requiredSpec(spec: RequirementSpec | undefined): RequirementSpec {
  if (spec == null) {
    throw new SolidVerificationError('INVALID_PUBLIC_INPUTS', 'verifyRequirement requires either requirement or spec.');
  }
  return spec;
}

function normalizeHex32(value: string, label: string): string {
  const hex = value.startsWith('0x') ? value.slice(2) : value;
  const lower = hex.toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(lower)) {
    throw new SolidVerificationError('INVALID_PUBLIC_INPUTS', `${label} must be a 32-byte lowercase hex string.`);
  }
  return lower;
}

function hexToBytes(hex: string): Uint8Array {
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  return out;
}

function bytesToHex(bytes: Uint8Array): string {
  return Buffer.from(bytes).toString('hex');
}

function stableJson(value: unknown): string {
  if (value == null || typeof value !== 'object') return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(stableJson).join(',')}]`;
  const entries = Object.entries(value as Record<string, unknown>).sort(([a], [b]) => a.localeCompare(b));
  return `{${entries.map(([k, v]) => `${JSON.stringify(k)}:${stableJson(v)}`).join(',')}}`;
}

interface NormalizedProof {
  proofA: Uint8Array;
  proofB: Uint8Array;
  proofC: Uint8Array;
  fullPublicInputs: Uint8Array[];
  nullifier: Uint8Array;
}

function normalizeSerializedProof(proof: SerializedProof): NormalizedProof {
  const publicSignals = proof.publicSignals.map(publicSignalToBytes32);
  if (publicSignals.length !== NR_PUBLIC_INPUTS) {
    throw new SolidVerificationError(
      'INVALID_PUBLIC_INPUTS',
      `Expected ${NR_PUBLIC_INPUTS} public signals, got ${publicSignals.length}.`,
    );
  }
  const solanaProof = proof.solanaProof ?? {};
  const proofA = bytesLikeToU8(proof.proofA ?? proof.proof_a ?? solanaProof.proofA ?? solanaProof.proof_a, 64, 'proofA');
  const proofB = bytesLikeToU8(proof.proofB ?? proof.proof_b ?? solanaProof.proofB ?? solanaProof.proof_b, 128, 'proofB');
  const proofC = bytesLikeToU8(proof.proofC ?? proof.proof_c ?? solanaProof.proofC ?? solanaProof.proof_c, 64, 'proofC');
  const nullifier = proof.nullifier == null
    ? publicSignals[0]
    : (typeof proof.nullifier === 'string'
        ? hexToBytes(normalizeHex32(proof.nullifier, 'nullifier'))
        : bytesLikeToU8(proof.nullifier, 32, 'nullifier'));
  return { proofA, proofB, proofC, fullPublicInputs: publicSignals, nullifier };
}

function bytesLikeToU8(value: Uint8Array | string | number[] | undefined, length: number, label: string): Uint8Array {
  if (value == null) throw new SolidVerificationError('INVALID_PUBLIC_INPUTS', `${label} is required.`);
  const bytes = typeof value === 'string'
    ? hexToBytes(normalizeFixedHex(value, length, label))
    : value instanceof Uint8Array
      ? value
      : Uint8Array.from(value);
  if (bytes.length !== length) {
    throw new SolidVerificationError('INVALID_PUBLIC_INPUTS', `${label} must be ${length} bytes, got ${bytes.length}.`);
  }
  return bytes;
}

function normalizeFixedHex(value: string, length: number, label: string): string {
  const hex = value.startsWith('0x') ? value.slice(2) : value;
  const lower = hex.toLowerCase();
  if (!/^[0-9a-f]+$/.test(lower) || lower.length !== length * 2) {
    throw new SolidVerificationError('INVALID_PUBLIC_INPUTS', `${label} must be ${length} bytes of hex.`);
  }
  return lower;
}

function publicSignalToBytes32(value: string | number | bigint | Uint8Array): Uint8Array {
  if (value instanceof Uint8Array) {
    if (value.length !== 32) throw new SolidVerificationError('INVALID_PUBLIC_INPUTS', 'public signal bytes must be 32 bytes.');
    return value;
  }
  const bigintValue = typeof value === 'bigint' ? value : BigInt(value);
  return bigintToBytes32BE(bigintValue);
}

function bigintToBytes32BE(n: bigint): Uint8Array {
  if (n < 0n) {
    throw new SolidVerificationError('INVALID_PUBLIC_INPUTS', 'public signal cannot be negative.');
  }
  const hex = n.toString(16).padStart(64, '0');
  if (hex.length > 64) {
    throw new SolidVerificationError('INVALID_PUBLIC_INPUTS', 'public signal exceeds 32 bytes.');
  }
  return hexToBytes(hex);
}

function bytes32BEToBigInt(bytes: Uint8Array): bigint {
  return BigInt(`0x${bytesToHex(bytes)}`);
}

function validatePublicSignals(requirement: Requirement, fullPublicInputs: Uint8Array[]): void {
  const activeSchemaHashes = fullPublicInputs.slice(6, 10).map(bytesToHex).filter(h => h !== ZERO32_HEX);
  if (!activeSchemaHashes.includes(requirement.schemaHash)) {
    throw new SolidVerificationError(
      'INVALID_PUBLIC_INPUTS',
      `Proof schema hashes do not include requirement schema ${requirement.schemaHash}.`,
    );
  }
  const numPredicates = Number(bytes32BEToBigInt(fullPublicInputs[27]));
  if (numPredicates !== requirement.publicInputs.predicateEncodings.length) {
    throw new SolidVerificationError('INVALID_PUBLIC_INPUTS', 'Proof predicate count does not match requirement.');
  }
  const compoundLogic = Number(bytes32BEToBigInt(fullPublicInputs[28]));
  const expectedLogic = requirement.publicInputs.compoundLogic === 'AND' ? 0 : 1;
  if (compoundLogic !== expectedLogic) {
    throw new SolidVerificationError('INVALID_PUBLIC_INPUTS', 'Proof compound logic does not match requirement.');
  }
  requirement.publicInputs.predicateEncodings.forEach((pred, i) => {
    const field = Number(bytes32BEToBigInt(fullPublicInputs[15 + i]));
    const op = Number(bytes32BEToBigInt(fullPublicInputs[19 + i]));
    const value = bytes32BEToBigInt(fullPublicInputs[23 + i]).toString();
    if (field !== pred.fieldIndex || op !== OP_CODE[pred.op] || value !== pred.value) {
      throw new SolidVerificationError('INVALID_PUBLIC_INPUTS', `Predicate ${i} does not match requirement.`);
    }
  });
}

const ZERO32_HEX = '0'.repeat(64);
const OP_CODE: Record<RequirementPredicateEncoding['op'], number> = {
  EQ: 1,
  NE: 2,
  GT: 3,
  GTE: 4,
  LT: 5,
  LTE: 6,
};

function deriveTreesFromPublicInputs(
  fullPublicInputs: Uint8Array[],
  schemaRegistryProgramId: PublicKey,
  issuerRegistryProgramId: PublicKey,
): SchemaTreeAccounts {
  const schemaTrees = fullPublicInputs.slice(6, 10).map((schemaHash) => {
    if (bytesToHex(schemaHash) === ZERO32_HEX) return PublicKey.default;
    return PublicKey.findProgramAddressSync(
      [Buffer.from('schema-tree-binding'), Buffer.from(schemaHash)],
      schemaRegistryProgramId,
    )[0];
  });
  return {
    globalTree: PublicKey.findProgramAddressSync([Buffer.from('global-binding')], schemaRegistryProgramId)[0],
    schemaTree0: schemaTrees[0],
    schemaTree1: schemaTrees[1],
    schemaTree2: schemaTrees[2],
    schemaTree3: schemaTrees[3],
    issuerTreeBinding: PublicKey.findProgramAddressSync([Buffer.from('issuer-tree-binding')], issuerRegistryProgramId)[0],
  };
}

function buildQueryFromRequirement(requirement: Requirement, fullPublicInputs: Uint8Array[]): MultiCredentialQuery {
  return {
    schemaHashes: fullPublicInputs.slice(6, 10),
    predicates: requirement.publicInputs.predicateEncodings.map((pred) => ({
      credentialIndex: 0,
      fieldIndex: pred.fieldIndex,
      operator: pred.op,
      value: BigInt(pred.value),
    })),
    compoundLogic: requirement.publicInputs.compoundLogic,
    verifierAddress: requirement.verifierAddress.toBytes(),
    verifierNonce: hexToBytes(requirement.publicInputs.verifierNonce),
    expirationTimestamp: requirement.publicInputs.expiresAt,
    globalRoot: fullPublicInputs[1],
    queryContextHash: new Uint8Array(32),
  };
}

function normalizeVerificationFailure(err: unknown): VerificationFailure {
  if (err instanceof SolidVerificationError) {
    return { verified: false, reason: err.reason, detail: err.detail };
  }
  const message = err instanceof Error ? err.message : String(err);
  const lower = message.toLowerCase();
  if (lower.includes('insufficient') || lower.includes('0x1')) {
    return { verified: false, reason: 'INSUFFICIENT_PAYER_BALANCE', detail: message };
  }
  if (lower.includes('already in use') || lower.includes('nullifier') && lower.includes('already')) {
    return { verified: false, reason: 'PROOF_REPLAYED', detail: message };
  }
  if (lower.includes('vk') && (lower.includes('frozen') || lower.includes('finalized'))) {
    return { verified: false, reason: 'VK_FROZEN', detail: message };
  }
  if (lower.includes('sha-256') || lower.includes('artifact') && lower.includes('mismatch')) {
    return { verified: false, reason: 'ARTIFACT_PIN_MISMATCH', detail: message };
  }
  if (lower.includes('timeout') || lower.includes('block height exceeded')) {
    return { verified: false, reason: 'TRANSACTION_TIMEOUT', detail: message };
  }
  if (lower.includes('fetch') || lower.includes('network') || lower.includes('rpc')) {
    return { verified: false, reason: 'RPC_UNAVAILABLE', detail: message };
  }
  if (lower.includes('public input') || lower.includes('proof_a') || lower.includes('proofb') || lower.includes('predicate')) {
    return { verified: false, reason: 'INVALID_PUBLIC_INPUTS', detail: message };
  }
  return { verified: false, reason: 'UNKNOWN', detail: message };
}

async function pingJson(url: string): Promise<{ ok: boolean; url: string; detail?: string }> {
  try {
    const res = await fetch(url, { method: 'GET' });
    return res.ok ? { ok: true, url } : { ok: false, url, detail: `HTTP ${res.status}` };
  } catch (err) {
    return { ok: false, url, detail: err instanceof Error ? err.message : String(err) };
  }
}
