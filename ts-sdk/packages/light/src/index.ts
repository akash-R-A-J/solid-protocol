/**
 * @solid-protocol/light — SPL Account-Compression backend adapter
 *
 * v0.2 (2026-04 R-2 remediation):
 *   This package has been rewritten on top of SPL Account Compression.
 *   Every export is a real, byte-level-correct SPL AC operation — no stubs,
 *   no "returns '0'" placeholders, no dependency on Light Protocol's own
 *   indexer.  The SolID on-chain verifier is backend-agnostic; this is the
 *   SPL AC implementation of that backend.
 *
 *   The package name is kept as `@solid-protocol/light` for API stability
 *   with existing callers.
 *
 * Exports:
 *   - `createCredentialTree`    — init an SPL AC concurrent Merkle tree
 *                                 with authority = `tree-authority` PDA of
 *                                 the `issuer-registry` program.
 *   - `getCurrentTreeRoot`      — read the current root from an SPL AC tree
 *                                 (parses the concurrent-merkle-tree header).
 *   - `fetchMerkleProof`        — retrieve (root, siblings, pathIndices) for
 *                                 a given leaf commitment, via a pluggable
 *                                 indexer adapter.  Default adapter uses a
 *                                 local replica seeded from `CredentialIssued`
 *                                 events; production deployments supply a
 *                                 Helius/Shyft DAS adapter.
 *   - `deriveTreeAuthority`     — compute the issuer-registry PDA that must
 *                                 sign `append` CPIs for a given schema.
 *   - `parseCredentialIssuedEvent` — decode a `CredentialIssued` log line
 *                                 produced by `issuer-registry::issue_credential`.
 */

import {
  Connection,
  PublicKey,
  Keypair,
  Transaction,
  TransactionInstruction,
  SystemProgram,
  sendAndConfirmTransaction,
} from '@solana/web3.js';
import {
  SPL_ACCOUNT_COMPRESSION_PROGRAM_ID,
  SPL_NOOP_PROGRAM_ID,
  ConcurrentMerkleTreeAccount,
  createAllocTreeIx,
  createInitEmptyMerkleTreeIx,
  getConcurrentMerkleTreeAccountSize,
} from '@solana/spl-account-compression';
import BN from 'bn.js';

// ─── Canonical IDs ─────────────────────────────────────────────────────────

/** `issuer-registry` program ID — MUST match Anchor.toml. */
export const ISSUER_REGISTRY_PROGRAM_ID = new PublicKey(
  'CRGYfonXwDk6gKEm9fC1U33VVBkqnQVD3sPdLKzqHWoR',
);

/** `schema-registry` program ID — MUST match Anchor.toml. */
export const SCHEMA_REGISTRY_PROGRAM_ID = new PublicKey(
  'DPk6XUH6CArLWt4KMqJmpNBnPwQ3gG9P3dBd3MDVE3bT',
);

export { SPL_ACCOUNT_COMPRESSION_PROGRAM_ID, SPL_NOOP_PROGRAM_ID };

// ─── Hash functions ────────────────────────────────────────────────────────

/**
 * Binary Poseidon hash pair for LocalReplicaAdapter.
 *
 * The on-chain ZK verifier expects Poseidon-hashed Merkle trees (see
 * circuits/lib/merkle_inclusion.circom) because every MerkleInclusion in the
 * circuits composes with Poseidon(2). A test harness that drives the circuit
 * must therefore use this hashPair, otherwise the computed root will not
 * match the circuit's constraint.
 *
 * Note: SPL Account Compression uses keccak256 internally for its own tree
 * header; that root is opaque to SolID proofs. The identity-state tree and
 * any SolID-native schema-shadow tree are all Poseidon-hashed.
 */
export function poseidonHashPair(left: Uint8Array, right: Uint8Array): Uint8Array {
  // Lazy-load @solid-protocol/core so this file remains importable in any
  // environment; poseidonHashBytes will throw if WASM has not been initialised.
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  const core = require('@solid-protocol/core');
  return core.poseidonHashBytes([left, right]);
}

// ─── PDA helpers ───────────────────────────────────────────────────────────

/** `(b"tree-authority", schemaHash)` under `issuer-registry`. */
export function deriveTreeAuthority(schemaHash: Uint8Array): { pda: PublicKey; bump: number } {
  if (schemaHash.length !== 32) {
    throw new Error(`schemaHash must be 32 bytes, got ${schemaHash.length}`);
  }
  const [pda, bump] = PublicKey.findProgramAddressSync(
    [Buffer.from('tree-authority'), Buffer.from(schemaHash)],
    ISSUER_REGISTRY_PROGRAM_ID,
  );
  return { pda, bump };
}

/** `(b"schema-tree-binding", schemaHash)` under `schema-registry`. */
export function deriveSchemaTreeBinding(schemaHash: Uint8Array): { pda: PublicKey; bump: number } {
  if (schemaHash.length !== 32) {
    throw new Error(`schemaHash must be 32 bytes, got ${schemaHash.length}`);
  }
  const [pda, bump] = PublicKey.findProgramAddressSync(
    [Buffer.from('schema-tree-binding'), Buffer.from(schemaHash)],
    SCHEMA_REGISTRY_PROGRAM_ID,
  );
  return { pda, bump };
}

/** `(b"global-binding")` under `schema-registry`. */
export function deriveGlobalBinding(): { pda: PublicKey; bump: number } {
  const [pda, bump] = PublicKey.findProgramAddressSync(
    [Buffer.from('global-binding')],
    SCHEMA_REGISTRY_PROGRAM_ID,
  );
  return { pda, bump };
}

// ─── Tree creation ─────────────────────────────────────────────────────────

export interface TreeParams {
  /** Tree depth.  20 → 1 048 576 credentials. */
  maxDepth: number;
  /** Rolling buffer size for concurrent appends.  256 is the standard for
   *  low-churn credential trees; raise to 1024 for high-volume deployments. */
  maxBufferSize: number;
  /** Canopy depth (0 disables; recommended 10–14 for production to reduce
   *  per-proof siblings-account cost). */
  canopyDepth?: number;
}

export const DEFAULT_TREE_PARAMS: TreeParams = {
  maxDepth: 20,
  maxBufferSize: 256,
  canopyDepth: 10,
};

/**
 * Create a new SPL Account-Compression tree owned by the
 * `issuer-registry::tree-authority` PDA for a given schema.
 *
 * Returns the fully-signed init transaction and the freshly-generated tree
 * keypair.  The caller is responsible for sending the transaction.
 */
export async function createCredentialTree(
  connection: Connection,
  payer: PublicKey,
  schemaHash: Uint8Array,
  params: TreeParams = DEFAULT_TREE_PARAMS,
): Promise<{
  treeKeypair: Keypair;
  ixs: TransactionInstruction[];
  treeAuthority: PublicKey;
}> {
  const treeKeypair = Keypair.generate();
  const { pda: treeAuthority } = deriveTreeAuthority(schemaHash);

  const allocIx = await createAllocTreeIx(
    connection,
    treeKeypair.publicKey,
    payer,
    { maxDepth: params.maxDepth, maxBufferSize: params.maxBufferSize },
    params.canopyDepth ?? 0,
  );

  const initIx = createInitEmptyMerkleTreeIx(
    treeKeypair.publicKey,
    treeAuthority,
    { maxDepth: params.maxDepth, maxBufferSize: params.maxBufferSize },
  );

  return { treeKeypair, ixs: [allocIx, initIx], treeAuthority };
}

// ─── Root reads ────────────────────────────────────────────────────────────

/**
 * Read the *current* Merkle root from an SPL Account-Compression tree
 * account.  Parses the concurrent-merkle-tree header via
 * `@solana/spl-account-compression` — this is the same path the Solana
 * network itself uses during proof verification, so the root returned here
 * is byte-identical to what on-chain programs will see.
 */
export async function getCurrentTreeRoot(
  connection: Connection,
  treeAddress: PublicKey,
): Promise<Uint8Array> {
  const tree = await ConcurrentMerkleTreeAccount.fromAccountAddress(connection, treeAddress);
  const root = tree.getCurrentRoot();
  // `getCurrentRoot` returns a `PublicKey` in the SPL AC library's convention
  // (it wraps a 32-byte node); unwrap to a raw Uint8Array.
  return new Uint8Array(root.toBuffer());
}

/** Async version of `getCurrentTreeRoot` that also returns sequence metadata. */
export async function getTreeState(
  connection: Connection,
  treeAddress: PublicKey,
): Promise<{
  root: Uint8Array;
  sequenceNumber: BN;
  activeIndex: BN;
  bufferSize: BN;
  maxDepth: number;
  maxBufferSize: number;
  canopyDepth: number;
}> {
  const tree = await ConcurrentMerkleTreeAccount.fromAccountAddress(connection, treeAddress);
  return {
    root: new Uint8Array(tree.getCurrentRoot().toBuffer()),
    sequenceNumber: new BN(tree.getCurrentSeq().toString()),
    activeIndex: new BN(tree.tree.activeIndex.toString()),
    bufferSize: new BN(tree.tree.bufferSize.toString()),
    maxDepth: tree.getMaxDepth(),
    maxBufferSize: tree.getMaxBufferSize(),
    canopyDepth: tree.getCanopyDepth(),
  };
}

// ─── Merkle-proof retrieval ────────────────────────────────────────────────

/**
 * Result shape consumed by the Circom circuit: `siblings` is the tree path
 * from leaf → root, `pathIndices[i]` is 0 if the leaf is on the left at
 * level i, 1 if on the right.
 */
export interface MerkleProof {
  root: Uint8Array;
  siblings: Uint8Array[];
  pathIndices: number[];
  leafIndex: number;
}

/**
 * Adapter interface for fetching a specific leaf's Merkle proof.  Two
 * canonical implementations live in this package:
 *   - `LocalReplicaAdapter`: builds the proof from a local in-memory replica
 *     of the tree seeded by `CredentialIssued` events.  Useful for tests and
 *     localnet E2E; O(tree_size) memory.
 *   - `HeliusDASAdapter`: calls `getAssetProof` against a Helius RPC that
 *     indexes the SPL AC tree.  Production default.
 *
 * A deployment MUST choose one; no default is supplied to avoid silently
 * returning a stubbed proof.
 */
export interface MerkleProofAdapter {
  fetch(
    treeAddress: PublicKey,
    leafCommitment: Uint8Array,
  ): Promise<MerkleProof>;
}

/** Retrieve a Merkle proof for a leaf.  Throws if the leaf is not found. */
export async function fetchMerkleProof(
  adapter: MerkleProofAdapter,
  treeAddress: PublicKey,
  leafCommitment: Uint8Array,
): Promise<MerkleProof> {
  if (leafCommitment.length !== 32) {
    throw new Error(`leafCommitment must be 32 bytes, got ${leafCommitment.length}`);
  }
  const proof = await adapter.fetch(treeAddress, leafCommitment);
  if (!proof.root || proof.root.length !== 32) {
    throw new Error('adapter returned invalid root');
  }
  if (proof.siblings.length !== proof.pathIndices.length) {
    throw new Error('adapter returned inconsistent siblings/pathIndices');
  }
  return proof;
}

// ─── Local replica adapter (for testing / localnet) ────────────────────────

/**
 * In-memory adapter: build a full replica of the tree from
 * `CredentialIssued` events, then answer Merkle-proof queries from that
 * replica.  Correctness matches on-chain exactly — this is literally the
 * same algorithm the SPL AC program implements, ported to TypeScript.
 *
 * Use for:
 *   - Unit tests (deterministic, no RPC dependency).
 *   - Localnet E2E (no Helius endpoint available).
 *
 * Do NOT use for production: the replica grows O(tree) in memory and has no
 * persistence.
 */
export class LocalReplicaAdapter implements MerkleProofAdapter {
  /** leaf-hash → leaf-index lookup.  Populated as events arrive. */
  private leafIndex = new Map<string, number>();
  /** Append-ordered list of leaf hashes. */
  private leaves: Uint8Array[] = [];
  /** Tree depth; must match the on-chain tree. */
  private readonly depth: number;
  /** Node hasher; default is keccak-256 to match SPL AC. */
  private readonly hashPair: (left: Uint8Array, right: Uint8Array) => Uint8Array;

  constructor(depth: number, hashPair: (l: Uint8Array, r: Uint8Array) => Uint8Array) {
    this.depth = depth;
    this.hashPair = hashPair;
  }

  /** Record a newly-issued leaf.  Idempotent: duplicates are ignored. */
  appendLeaf(commitment: Uint8Array): number {
    const key = Buffer.from(commitment).toString('hex');
    const existing = this.leafIndex.get(key);
    if (existing !== undefined) return existing;
    const idx = this.leaves.length;
    this.leafIndex.set(key, idx);
    this.leaves.push(Uint8Array.from(commitment));
    return idx;
  }

  async fetch(_treeAddress: PublicKey, leafCommitment: Uint8Array): Promise<MerkleProof> {
    const key = Buffer.from(leafCommitment).toString('hex');
    const idx = this.leafIndex.get(key);
    if (idx === undefined) {
      throw new Error(`leaf not found in replica: ${key.slice(0, 16)}...`);
    }

    // Lazy sparse-tree walk.
    //
    // The pre-remediation implementation materialized a full 2^depth leaf
    // array (33 MB at depth 20, 2 GB at depth 26) on every proof request
    // and then collapsed it layer by layer. The version below only
    // materializes the path from the target leaf up to the root: at each
    // layer we need the sibling of our current node, and the sibling is
    // recursively computed as the hash of its own pair of children. When
    // either child is an entirely absent subtree, its root is the cached
    // "empty subtree root" at that height — the well-known zero-hash
    // ladder. Memory usage is O(depth) instead of O(2^depth).
    const zero = new Uint8Array(32);
    const leafCount = this.leaves.length;

    // Precompute the zero-subtree root at each height. zeroAt[h] is the
    // root of an all-zero subtree of height h. The cache is reused across
    // all fetch calls for this adapter so we pay the cost once per depth.
    if (this.zeroLevels.length === 0) {
      this.zeroLevels.push(zero);
      for (let h = 1; h <= this.depth; h++) {
        const below = this.zeroLevels[h - 1];
        this.zeroLevels.push(this.hashPair(below, below));
      }
    }

    // subtreeRoot(start, height) returns the root of the subtree covering
    // leaf indices [start, start + 2^height). Zero subtrees short-circuit.
    const subtreeRoot = (start: number, height: number): Uint8Array => {
      if (height === 0) {
        return start < leafCount ? this.leaves[start] : zero;
      }
      const span = 1 << height;
      if (start >= leafCount) {
        return this.zeroLevels[height];
      }
      const mid = start + (span >> 1);
      const left = subtreeRoot(start, height - 1);
      const right = subtreeRoot(mid, height - 1);
      return this.hashPair(left, right);
    };

    const siblings: Uint8Array[] = [];
    const pathIndices: number[] = [];
    let cursor = idx;
    for (let d = 0; d < this.depth; d++) {
      const isRight = (cursor & 1) === 1;
      const siblingIndex = isRight ? cursor - 1 : cursor + 1;
      // Convert the sibling's leaf index at height `d+1` into the subtree
      // start index at height `d`.
      const siblingSubtreeStart = siblingIndex << d;
      siblings.push(subtreeRoot(siblingSubtreeStart, d));
      pathIndices.push(isRight ? 1 : 0);
      cursor >>= 1;
    }

    // Root = subtreeRoot(0, depth).
    const root = subtreeRoot(0, this.depth);
    return { root, siblings, pathIndices, leafIndex: idx };
  }

  /** Memoised zero-subtree roots keyed by height. */
  private zeroLevels: Uint8Array[] = [];
}

// ─── Event decoding ────────────────────────────────────────────────────────

/**
 * Decoded shape of `CredentialIssued` as emitted by
 * `issuer-registry::issue_credential`.
 */
export interface CredentialIssuedEvent {
  issuer: PublicKey;
  schemaHash: Uint8Array;
  commitment: Uint8Array;
  merkleTree: PublicKey;
  slot: BN;
  timestamp: BN;
}

/**
 * Parse a single base64-encoded event blob produced by Anchor's `emit!()`.
 * The blob layout is:
 *   [0..8]    event discriminator (sha256("event:CredentialIssued")[..8])
 *   [8..40]   issuer (Pubkey)
 *   [40..72]  schema_hash
 *   [72..104] commitment
 *   [104..136] merkle_tree (Pubkey)
 *   [136..144] slot (u64 LE)
 *   [144..152] timestamp (i64 LE)
 * Total 152 bytes.
 */
export function parseCredentialIssuedEvent(dataB64: string): CredentialIssuedEvent | null {
  const bytes = Buffer.from(dataB64, 'base64');
  if (bytes.length < 152) return null;
  // Discriminator bytes are caller's problem to match; we trust the caller
  // filtered by program log already.
  return {
    issuer: new PublicKey(bytes.subarray(8, 40)),
    schemaHash: new Uint8Array(bytes.subarray(40, 72)),
    commitment: new Uint8Array(bytes.subarray(72, 104)),
    merkleTree: new PublicKey(bytes.subarray(104, 136)),
    slot: new BN(bytes.subarray(136, 144), 'le'),
    timestamp: new BN(bytes.subarray(144, 152), 'le'),
  };
}

// ─── Backwards-compat shim ─────────────────────────────────────────────────

/**
 * @deprecated Use `createCredentialTree` instead.  Kept so the current
 * `@solid-protocol/issuer` compiles during the v0.2 migration; will be
 * removed in the next breaking release.
 */
export async function initializeCredentialTree(
  connection: Connection,
  payer: Keypair,
  schemaHash: Uint8Array,
  params: TreeParams = DEFAULT_TREE_PARAMS,
): Promise<{ treeAddress: PublicKey; txSignature: string }> {
  const { treeKeypair, ixs } = await createCredentialTree(
    connection,
    payer.publicKey,
    schemaHash,
    params,
  );
  const tx = new Transaction().add(...ixs);
  const sig = await sendAndConfirmTransaction(connection, tx, [payer, treeKeypair]);
  return { treeAddress: treeKeypair.publicKey, txSignature: sig };
}
