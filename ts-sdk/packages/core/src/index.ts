/**
 * @solid-protocol/core — WASM loader + typed re-exports
 *
 * This is the foundation of the TS SDK. It loads the WASM module
 * and provides typed wrappers around the raw WASM functions.
 * NO crypto is reimplemented in TypeScript — everything calls WASM.
 */

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
 */
export async function initWasm(): Promise<void> {
  if (wasmModule) return;
  // Dynamic import of the WASM package
  wasmModule = await import('@solid-protocol/wasm');
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
  const totalLen = inputs.length * 32;
  
  // SEC-18: High-Performance Zero-Copy Path (Phase 3)
  // We write directly into the shared WASM buffer to avoid overhead.
  wasmModule.resizeSharedBuffer(totalLen);
  const ptr = wasmModule.getSharedBufferPointer();
  const memory = wasmModule.memory.buffer; // The underlying WebAssembly.Memory
  
  const bufferView = new Uint8Array(memory, ptr, totalLen);
  inputs.forEach((inp, i) => bufferView.set(inp, i * 32));
  
  return new Uint8Array(wasmModule.poseidonHashShared(totalLen));
}

// ─── BabyJubJub ────────────────────────────────────────────────────────────

export function generateKeypair(): BJJKeypair {
  ensureInit();
  return wasmModule.generateBJJKeypair();
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

export function computeNullifier(
  masterKey: Uint8Array,
  revocationNonce: bigint,
  verifierAddress: Uint8Array,
  queryContextHash: Uint8Array,
  verifierNonce: Uint8Array,
): Uint8Array {
  ensureInit();
  return new Uint8Array(wasmModule.computeHardenedNullifier(
    masterKey, revocationNonce, verifierAddress, queryContextHash, verifierNonce,
  ));
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

export function computeIdentityState(pubKeyX: Uint8Array, pubKeyY: Uint8Array, revocationNonce: bigint): Uint8Array {
  ensureInit();
  return new Uint8Array(wasmModule.computeIdentityState(pubKeyX, pubKeyY, revocationNonce));
}

// ─── Query Builder ─────────────────────────────────────────────────────────

const OP_MAP = { 'NOOP': 0, 'EQ': 1, 'NE': 2, 'GT': 3, 'GTE': 4, 'LT': 5, 'LTE': 6 } as const;

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
    
    return poseidonHashBytes([
        hIndices, 
        hOps, 
        new Uint8Array(new BigUint64Array([BigInt(this._predicates.length)]).buffer),
        new Uint8Array(new BigUint64Array([this._logic === 'AND' ? 0n : 1n]).buffer)
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

  /** Convert to circuit public inputs (Solana verifier expects 31 inputs) */
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
