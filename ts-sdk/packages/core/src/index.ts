/**
 * @solid-protocol/core — WASM loader + typed re-exports
 *
 * This is the foundation of the TS SDK. It loads the WASM module
 * and provides typed wrappers around the raw WASM functions.
 * NO crypto is reimplemented in TypeScript — everything calls WASM.
 */

/** Protocol-wide constants mirrored from `crates/solid-core/src/lib.rs`. */
export const MAX_CREDENTIALS = 4;
export const MAX_PREDICATES = 4;
export const NUM_FIELDS = 8;
export const TREE_DEPTH = 20;

/** Canonical SolID program IDs. Kept as base58 strings so JS doesn't
 *  need the Solana web3 package just to import types. */
export const PROGRAM_IDS = {
  zkVerifier: 'DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb',
  issuerRegistry: '5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx',
  schemaRegistry: '4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1',
} as const;

/** Operator encoding matches `crates/solid-core/src/query.rs#Operator`. */
export const OP_MAP = {
  NOOP: 0, EQ: 1, NE: 2, GT: 3, GTE: 4, LT: 5, LTE: 6,
} as const;

// Re-export types
export interface BJJKeypair {
  private_key: Uint8Array;
  public_key_x: Uint8Array;
  public_key_y: Uint8Array;
}

export interface EdDSASignature {
  r8_x: Uint8Array;
  r8_y: Uint8Array;
  s: Uint8Array;
}

export interface CompoundQuery {
  schemaHash: Uint8Array;
  predicates: Predicate[];
  compoundLogic: 'AND' | 'OR';
  verifierNonce: Uint8Array;
  expirationTimestamp: number;
  // Phase 3.1: Global State Signals
  globalRoot?: Uint8Array;
  revocationNonce?: bigint;
}

export interface Predicate {
  credentialIndex: number;
  fieldIndex: number;
  operator: 'NOOP' | 'EQ' | 'NE' | 'GT' | 'GTE' | 'LT' | 'LTE';
  value: bigint;
}

// WASM module reference
let wasmModule: any = null;

/**
 * Initialize the WASM module. Must be called before any crypto operations.
 *
 * The bridge is built from `wasm/src/lib.rs` (crate `solid-wasm`) by
 * running `wasm-pack build wasm/ --target nodejs --out-dir
 * ts-sdk/packages/core/wasm --release` from the repo root. At runtime
 * the compiled entry point is at `dist/index.js`, so `../wasm`
 * resolves to `ts-sdk/packages/core/wasm/` where wasm-pack emits its
 * package.json + `solid_wasm.js`. See SOLID-SEC-009 / ADR-0002.
 */
export async function initWasm(): Promise<void> {
  if (wasmModule) return;
  // The module is built out-of-band; path is resolved at runtime.
  // eslint-disable-next-line @typescript-eslint/ban-ts-comment
  // @ts-ignore — ambient types come from wasm/solid_wasm.d.ts once built
  wasmModule = await import('../wasm/solid_wasm.js');
}

function ensureInit() {
  if (!wasmModule) throw new Error('WASM not initialized. Call initWasm() first.');
}

// ─── Poseidon Hash ─────────────────────────────────────────────────────────

export function poseidonHash(fields: bigint[]): Uint8Array {
  ensureInit();
  return new Uint8Array(wasmModule.poseidonHash(new BigUint64Array(fields)));
}

export function poseidonHashBytes(inputs: Uint8Array[]): Uint8Array {
  ensureInit();

  // Concatenate the chunks into a single flat buffer; wasm-bindgen marshals
  // the slice into wasm linear memory in a single copy via passArray8ToWasm0.
  //
  // A previous implementation tried to write directly into a Rust-side
  // `static SHARED_BUFFER` via `wasm.memory.buffer`. That path was removed
  // because (a) `--target nodejs` does not re-export `memory` on the module
  // exports, so it crashed in node, (b) the captured pointer could dangle if
  // an unrelated wasm allocation reallocated the static `Vec<u8>` between
  // `resizeSharedBuffer()` and `poseidonHashShared()`, and (c) at our
  // payload sizes (≤ 4 × 32 B) the marshaling cost is sub-microsecond, so
  // the "zero-copy" path saved nothing while doubling API surface.
  // See ADR-0002 / SOLID-SEC-009.
  const flat = new Uint8Array(inputs.length * 32);
  inputs.forEach((inp, i) => {
    if (inp.length !== 32) {
      throw new Error(
        `poseidonHashBytes: each input must be 32 bytes, input[${i}] is ${inp.length}`,
      );
    }
    flat.set(inp, i * 32);
  });
  return new Uint8Array(wasmModule.poseidonHashBytes(flat));
}

// ─── Schema hash (SOLID-SEC-002 + SOLID-SEC-063 / H5) ──────────────────────
//
// Mirror of `solid_core::schema::compute_schema_hash_from_parts` and
// `poseidon_compress_bytes`.  Produces byte-identical output so the
// SDK-computed schema_hash passes `register_schema`'s on-chain check.
//
// Derivation:
//   name_h    = poseidonCompressBytes(utf8(name))
//   version_e = [version, 0, 0, ..., 0]                     // 32 bytes, byte 0 = version
//   count_e   = [u64 LE field_names.length, 0, 0, ..., 0]  // 32 bytes
//   fnames_h  = poseidonCompressBytes(canonical(field_names))
//                where canonical = (u32 LE count) || (u32 LE len || bytes)*
//   cat_h     = poseidonCompressBytes(utf8(category))
//   schema_hash = poseidonHashBytes([name_h, version_e, count_e, fnames_h, cat_h])

export function poseidonCompressBytes(data: Uint8Array): Uint8Array {
  let state: Uint8Array = new Uint8Array(32);
  for (let off = 0; off < data.length; off += 31) {
    const elem = new Uint8Array(32);
    const slice = data.subarray(off, Math.min(off + 31, data.length));
    elem.set(slice, 0);
    elem[31] = slice.length; // canonical chunk-length tag at byte 31
    state = new Uint8Array(poseidonHashBytes([state, elem]));
  }
  const lenTag = new Uint8Array(32);
  new DataView(lenTag.buffer).setBigUint64(0, BigInt(data.length), true);
  state = new Uint8Array(poseidonHashBytes([state, lenTag]));
  return state;
}

export function computeSchemaHash(
  name: string,
  version: number,
  fieldNames: string[],
  category: string,
): Uint8Array {
  const enc = new TextEncoder();
  const nameH = poseidonCompressBytes(enc.encode(name));

  // Canonical field-names byte stream: count-prefix + per-name length-prefix + bytes.
  const fbParts: Uint8Array[] = [];
  const countBuf = new Uint8Array(4);
  new DataView(countBuf.buffer).setUint32(0, fieldNames.length, true);
  fbParts.push(countBuf);
  for (const n of fieldNames) {
    const nb = enc.encode(n);
    const lenBuf = new Uint8Array(4);
    new DataView(lenBuf.buffer).setUint32(0, nb.length, true);
    fbParts.push(lenBuf);
    fbParts.push(nb);
  }
  let totalLen = 0;
  for (const p of fbParts) totalLen += p.length;
  const fb = new Uint8Array(totalLen);
  let off = 0;
  for (const p of fbParts) {
    fb.set(p, off);
    off += p.length;
  }
  const fnamesH = poseidonCompressBytes(fb);
  const catH = poseidonCompressBytes(enc.encode(category));

  const versionE = new Uint8Array(32);
  versionE[0] = version & 0xff;

  const countE = new Uint8Array(32);
  new DataView(countE.buffer).setBigUint64(0, BigInt(fieldNames.length), true);

  return poseidonHashBytes([nameH, versionE, countE, fnamesH, catH]);
}

// ─── BabyJubJub ────────────────────────────────────────────────────────────

// The WASM bridge serialises BJJ keypairs with `#[serde(rename_all =
// "camelCase")]`, so the raw object keys are `privateKey / publicKeyX /
// publicKeyY`.  The SDK's public contract (BJJKeypair) is snake_case
// (`private_key / public_key_x / public_key_y`) and every TS consumer
// (scripts/, holder, e2e_state.json) has been written against that
// shape since v0.1.  Rather than break that contract, this helper
// re-keys the WASM output once so callers always see snake_case.
// SOLID-SEC-010 cross-language vector coverage will eventually freeze
// the wire shape, after which we can collapse the rename.
function snakeCaseBjj(raw: any): BJJKeypair {
  // wasm-bindgen + serde returns these as plain `number[]` (JSON arrays),
  // not `Uint8Array`. The `BJJKeypair` public contract promises
  // `Uint8Array`, and downstream consumers (e.g. holder cohesion check
  // calling `Buffer.from(...).compare(derivedX)`) require a typed array
  // -- `Buffer.compare` rejects raw arrays with ERR_INVALID_ARG_TYPE.
  // Coerce here once so every consumer sees the documented type.
  const toU8 = (v: any): Uint8Array =>
    v instanceof Uint8Array ? v : Uint8Array.from(v);
  return {
    private_key: toU8(raw.privateKey ?? raw.private_key),
    public_key_x: toU8(raw.publicKeyX ?? raw.public_key_x),
    public_key_y: toU8(raw.publicKeyY ?? raw.public_key_y),
  };
}

export function generateKeypair(): BJJKeypair {
  ensureInit();
  return snakeCaseBjj(wasmModule.generateBJJKeypair());
}

/**
 * SOLID-SEC-007 / SEC-048 client-side predicate.
 *
 * Returns true iff (`publicKeyX`, `publicKeyY`) is on the BabyJubJub
 * curve, is not the Edwards neutral element, and lies in the
 * prime-order subgroup (cofactor-8 torsion absent).  The full cost is
 * a host-side `r * P == O` scalar mul (~ms on x86), and this is the
 * canonical pre-submit gate every off-chain `register_issuer` caller
 * MUST run.
 *
 * Background: the on-chain `register_issuer` instruction in
 * `programs/issuer-registry/src/lib.rs` historically called the
 * matching Rust helper, but on BPF the `r * P == O` scalar mul costs
 * > 1.4M CU (the per-tx ceiling) and could not land.  As of
 * 2026-04-25 the on-chain check is gated behind a `sec007-skip-onchain`
 * Cargo feature on issuer-registry to unblock e2e; the off-chain
 * check (this function) is the load-bearing enforcement point until
 * SEC-048 is closed (see docs/E2E_BLOCKERS.md B9 and
 * sec/SECURITY_REGISTRY.md SEC-048).
 */
export function isInPrimeOrderSubgroup(
  publicKeyX: Uint8Array,
  publicKeyY: Uint8Array,
): boolean {
  ensureInit();
  return wasmModule.isBjjInPrimeOrderSubgroup(publicKeyX, publicKeyY);
}

export function sign(privateKey: Uint8Array, message: Uint8Array): EdDSASignature {
  ensureInit();
  return wasmModule.signMessage(privateKey, message);
}

export function verify(
  publicKeyX: Uint8Array, publicKeyY: Uint8Array,
  message: Uint8Array,
  signature: EdDSASignature,
): boolean {
  ensureInit();
  return wasmModule.verifySignature(
    publicKeyX, publicKeyY, message,
    signature.r8_x, signature.r8_y, signature.s,
  );
}

// ─── Commitment & Nullifier ────────────────────────────────────────────────

export function computeCommitment(
  dataFields: bigint[],
  schemaHash: Uint8Array,
  holderPubKeyX: Uint8Array,
  holderPubKeyY: Uint8Array,
  salt: Uint8Array,
): Uint8Array {
  ensureInit();
  return new Uint8Array(wasmModule.computeCommitment(
    new BigUint64Array(dataFields),
    schemaHash, holderPubKeyX, holderPubKeyY, salt,
  ));
}

/**
 * Compute the hardened 6-input nullifier (ADR-0006 Phase 2 revision):
 *
 *   nullifier = Poseidon(
 *     masterKey,
 *     revocationNonce,
 *     verifierAddress,
 *     queryContextHash,
 *     verifierNonce,
 *     issuerTreeRoot,         // ADR-0014 / SOLID-SEC-008 epoch bind
 *   )
 *
 * Produces bytes bit-compatible with `solid_core::nullifier::compute_nullifier`
 * and the Circom `batch_credential_query.circom` STEP 5.  A callsite using
 * the old 5-argument signature fails to compile -- intentional, since the
 * 5-input value no longer matches the on-chain verifier.
 */
export function computeNullifier(
  masterKey: Uint8Array,
  revocationNonce: bigint,
  verifierAddress: Uint8Array,
  queryContextHash: Uint8Array,
  verifierNonce: Uint8Array,
  issuerTreeRoot: Uint8Array,
): Uint8Array {
  ensureInit();
  return new Uint8Array(wasmModule.computeHardenedNullifier(
    masterKey, revocationNonce, verifierAddress,
    queryContextHash, verifierNonce, issuerTreeRoot,
  ));
}

/**
 * Compute the identity-state commitment that anchors a master identity in the
 * global state tree:
 *   `identityState = Poseidon(pubKeyX, pubKeyY, revocationNonce)`
 */
export function computeIdentityCommitment(
  pubKeyX: Uint8Array,
  pubKeyY: Uint8Array,
  revocationNonce: bigint,
): Uint8Array {
  return computeIdentityState(pubKeyX, pubKeyY, revocationNonce);
}

/**
 * Compute the ADR-0014 issuer-tree leaf:
 *   `leaf = Poseidon(issuerAuthority, bjjPubKeyX, bjjPubKeyY,
 *                    statusEpoch, revocationNonce)`
 *
 * MUST match the on-chain `compute_issuer_leaf_bytes` in
 * `programs/issuer-registry/src/lib.rs`.  Used by the holder SDK to
 * precompute the leaf for `fetchMerkleProof`, and by any indexer that
 * replays `IssuerLeafAppended` / `IssuerLeafReplaced` events into an
 * issuer-tree replica.
 *
 * `issuerAuthority` is a 32-byte Solana pubkey (the authority field
 * of the issuer's on-chain `IssuerAccount`).  The byte-order contract
 * mirrors the on-chain helper: raw 32-byte arrays are fed directly
 * into Poseidon, so callers MUST pass `authority.toBytes()` (not a
 * BE-encoded BigInt).
 */
export function computeIssuerLeaf(
  issuerAuthority: Uint8Array,
  bjjPubKeyX: Uint8Array,
  bjjPubKeyY: Uint8Array,
  statusEpoch: bigint,
  revocationNonce: bigint,
): Uint8Array {
  ensureInit();
  // Convert u64s to 32-byte LE (matches `solid_core::poseidon::u64_to_fr`
  // then `fr_to_bytes_le`).  Low 8 bytes carry the value; remaining 24
  // bytes are zero (fields below 2^64 don't wrap the modulus).
  const statusEpochBytes = new Uint8Array(32);
  const nonceBytes = new Uint8Array(32);
  let se = statusEpoch;
  let rn = revocationNonce;
  for (let i = 0; i < 8; i++) {
    statusEpochBytes[i] = Number(se & 0xffn);
    nonceBytes[i] = Number(rn & 0xffn);
    se >>= 8n;
    rn >>= 8n;
  }
  return poseidonHashBytes([
    issuerAuthority,
    bjjPubKeyX,
    bjjPubKeyY,
    statusEpochBytes,
    nonceBytes,
  ]);
}

// ─── Identity ──────────────────────────────────────────────────────────────

export function generateIdentity(passphrase: string): string {
  ensureInit();
  return wasmModule.generateIdentity(passphrase);
}

export function unlockIdentity(identityJson: string, passphrase: string): Uint8Array {
  ensureInit();
  return new Uint8Array(wasmModule.unlockIdentity(identityJson, passphrase));
}

export function deriveKey(masterKey: Uint8Array, context: Uint8Array): Uint8Array {
  ensureInit();
  return new Uint8Array(wasmModule.deriveKey(masterKey, context));
}

/**
 * Derive the per-schema BabyJubJub keypair that the circuit's `IdentityAnchor`
 * expects.
 *
 *   credentialPrivKey = Poseidon(masterKey, schemaHash)
 *   (credentialPubKeyAx, credentialPubKeyAy) = BabyPbk(credentialPrivKey)
 *
 * Wraps the WASM `deriveCredentialKey` binding. The holder SDK must use this
 * to build the per-schema identity leaf
 *   Poseidon(credentialPubKeyAx, credentialPubKeyAy, revocationNonce)
 * that sits in the global-state Merkle tree, not the naive master-key
 * commitment (BUG-04).
 */
export function deriveCredentialKey(
  masterKey: Uint8Array,
  schemaHash: Uint8Array,
): BJJKeypair {
  ensureInit();
  return snakeCaseBjj(wasmModule.deriveCredentialKey(masterKey, schemaHash));
}

export function computeIdentityState(pubKeyX: Uint8Array, pubKeyY: Uint8Array, revocationNonce: bigint): Uint8Array {
  ensureInit();
  return new Uint8Array(wasmModule.computeIdentityState(pubKeyX, pubKeyY, revocationNonce));
}

// ─── Query Builder ─────────────────────────────────────────────────────────

export interface MultiCredentialQuery {
  schemaHashes: Uint8Array[]; // Exactly 4
  predicates: Predicate[];    // 1-4
  compoundLogic: 'AND' | 'OR';
  verifierAddress: Uint8Array;
  verifierNonce: Uint8Array;
  expirationTimestamp: number;
  globalRoot: Uint8Array;
  queryContextHash: Uint8Array;
}

export class QueryBuilder {
  private _schemaHashes: Uint8Array[] = Array(4).fill(new Uint8Array(32));
  private _predicates: Predicate[] = [];
  private _logic: 'AND' | 'OR' = 'AND';
  private _verifierAddr: Uint8Array = new Uint8Array(32);
  private _nonce: Uint8Array = new Uint8Array(32);
  private _expiration: number = 0;
  private _globalRoot: Uint8Array = new Uint8Array(32);
  private _revocationNonce: bigint = 0n;

  schemas(hashes: Uint8Array[]): this { 
    hashes.forEach((h, i) => { if (i < 4) this._schemaHashes[i] = h; });
    return this; 
  }

  where(credIndex: number, fieldIndex: number, op: Predicate['operator'], value: bigint): this {
    this._predicates.push({ credentialIndex: credIndex, fieldIndex, operator: op, value });
    return this;
  }

  verifier(addr: Uint8Array): this { this._verifierAddr = addr; return this; }
  globalRoot(root: Uint8Array): this { this._globalRoot = root; return this; }
  revocationNonce(nonce: bigint): this { this._revocationNonce = nonce; return this; }
  nonce(n: Uint8Array): this { this._nonce = n; return this; }
  expiration(ts: number): this { this._expiration = ts; return this; }

  /**
   * SEC-13: Compute Query Context Hash
   * Prevents "Query Malleability" by binding the proof to specific predicates.
   */
  private _computeContextHash(): Uint8Array {
    const credIndices = new BigUint64Array(4).fill(0n);
    const fieldIndices = new BigUint64Array(4).fill(0n);
    const ops = new BigUint64Array(4).fill(0n);
    const vals = new BigUint64Array(4).fill(0n);

    this._predicates.forEach((p, i) => {
      credIndices[i] = BigInt(p.credentialIndex);
      fieldIndices[i] = BigInt(p.fieldIndex);
      ops[i] = BigInt(OP_MAP[p.operator]);
      vals[i] = p.value;
    });

    const hIndices = poseidonHash([...credIndices, ...fieldIndices]);
    const hOps = poseidonHash([...ops, ...vals]);

    // u64 -> 32-byte LE (low 8 bytes carry the value, remaining 24 are
    // zero).  Matches `solid_core::poseidon::u64_to_fr` /
    // `fr_to_bytes_le` and the canonical pattern in computeIssuerLeaf
    // above.  The circuit consumes these as field elements directly, so
    // 8-byte LE and 32-byte LE encode the same field value (numPredicates
    // and compoundLogic are both < 2^64).  poseidonHashBytes requires
    // 32-byte chunks at the wasm boundary; passing 8 bytes throws
    // "input[i] is 8".
    const u64Le32 = (v: bigint): Uint8Array => {
      const out = new Uint8Array(32);
      let x = v;
      for (let i = 0; i < 8; i++) { out[i] = Number(x & 0xffn); x >>= 8n; }
      return out;
    };
    return poseidonHashBytes([
      hIndices,
      hOps,
      u64Le32(BigInt(this._predicates.length)),
      u64Le32(this._logic === 'AND' ? 0n : 1n),
    ]);
  }

  build(): MultiCredentialQuery {
    if (this._predicates.length === 0 || this._predicates.length > 4) {
      throw new Error('Query must have 1-4 predicates');
    }
    // SEC-20: Enforce canonical ordering in SDK
    for (let i = 0; i < 3; i++) {
        const h1 = this._schemaHashes[i];
        const h2 = this._schemaHashes[i+1];
        if (h2.some(b => b !== 0)) {
            let isSmaller = false;
            for (let j = 0; j < 32; j++) {
                if (h1[j] < h2[j]) { isSmaller = true; break; }
                if (h1[j] > h2[j]) throw new Error('Schema hashes must be strictly ascending (SEC-20)');
            }
            if (!isSmaller) throw new Error('Schema hashes must be strictly ascending (SEC-20)');
        }
    }

    return {
      schemaHashes: this._schemaHashes,
      predicates: this._predicates,
      compoundLogic: this._logic,
      verifierAddress: this._verifierAddr,
      verifierNonce: this._nonce,
      expirationTimestamp: this._expiration,
      globalRoot: this._globalRoot,
      queryContextHash: this._computeContextHash(),
    };
  }

  /** Convert to circuit public inputs.
   *
   *  The circuit's public-input arity is `NR_PUBLIC_INPUTS = 32` post
   *  ADR-0014 (issuerTreeRoot inserted at slot [10]).  Pre-ADR-0014 it
   *  was 31; the comment here used to say "31" and was stale.
   *
   *  The on-chain `verify_batch_proof` ix transmits only 21 of those
   *  slots on the wire (SOLID-SEC-054 / B13); the other 11 are
   *  reconstructed from accounts.  But the witness still uses the
   *  full 32-element array, which is what this method returns. */
  toCircuitInputs() {
    const q = this.build();
    
    const queryCredIndices = Array(4).fill(0);
    const queryFieldIndices = Array(4).fill(0);
    const queryOperators = Array(4).fill(0);
    const queryValues = Array(4).fill(0n);

    q.predicates.forEach((p, i) => {
      queryCredIndices[i] = p.credentialIndex || 0;
      queryFieldIndices[i] = p.fieldIndex;
      queryOperators[i] = OP_MAP[p.operator];
      queryValues[i] = p.value;
    });

    return {
      // Index 0: Nullifier
      nullifierHash: new Uint8Array(32), 
      // Indices 1-6: State & Roots
      globalRoot: q.globalRoot,
      merkleRoots: Array(4).fill(new Uint8Array(32)),
      schemaHashes: q.schemaHashes,
      // Indices 7-30: Query Context
      queryCredentialIndices: queryCredIndices,
      queryFieldIndices,
      queryOperators,
      queryValues,
      numPredicates: q.predicates.length,
      compoundLogic: q.compoundLogic === 'AND' ? 0 : 1,
      // Index 31-33: Contextual Nonces
      verifierAddress: q.verifierAddress,
      verifierNonce: q.verifierNonce,
      currentTimestamp: Math.floor(Date.now() / 1000),
    };
  }
}
