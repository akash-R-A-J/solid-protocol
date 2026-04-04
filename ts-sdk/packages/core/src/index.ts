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
}

export interface Predicate {
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
  const flat = new Uint8Array(inputs.length * 32);
  inputs.forEach((inp, i) => flat.set(inp, i * 32));
  return new Uint8Array(wasmModule.poseidonHashBytes(flat));
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
  holderPrivateKey: Uint8Array,
  schemaHash: Uint8Array,
  verifierNonce: Uint8Array,
): Uint8Array {
  ensureInit();
  return new Uint8Array(wasmModule.computeNullifier(
    holderPrivateKey, schemaHash, verifierNonce,
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

// ─── Query Builder ─────────────────────────────────────────────────────────

const OP_MAP = { 'NOOP': 0, 'EQ': 1, 'NE': 2, 'GT': 3, 'GTE': 4, 'LT': 5, 'LTE': 6 } as const;

export class QueryBuilder {
  private _schemaHash: Uint8Array = new Uint8Array(32);
  private _predicates: Predicate[] = [];
  private _logic: 'AND' | 'OR' = 'AND';
  private _nonce: Uint8Array = new Uint8Array(32);
  private _expiration: number = 0;

  schema(hash: Uint8Array): this { this._schemaHash = hash; return this; }

  where(fieldIndex: number, op: Predicate['operator'], value: bigint): this {
    this._predicates.push({ fieldIndex, operator: op, value });
    return this;
  }

  and(fieldIndex: number, op: Predicate['operator'], value: bigint): this {
    this._logic = 'AND';
    return this.where(fieldIndex, op, value);
  }

  or(fieldIndex: number, op: Predicate['operator'], value: bigint): this {
    this._logic = 'OR';
    return this.where(fieldIndex, op, value);
  }

  nonce(n: Uint8Array): this { this._nonce = n; return this; }
  expiration(ts: number): this { this._expiration = ts; return this; }

  build(): CompoundQuery {
    if (this._predicates.length === 0 || this._predicates.length > 4) {
      throw new Error('Query must have 1-4 predicates');
    }
    return {
      schemaHash: this._schemaHash,
      predicates: this._predicates,
      compoundLogic: this._logic,
      verifierNonce: this._nonce,
      expirationTimestamp: this._expiration,
    };
  }

  /** Convert to circuit public input format */
  toCircuitInputs() {
    const q = this.build();
    const fieldIndices = Array(4).fill(0);
    const operators = Array(4).fill(0);
    const values = Array(4).fill(0n);

    q.predicates.forEach((p, i) => {
      fieldIndices[i] = p.fieldIndex;
      operators[i] = OP_MAP[p.operator];
      values[i] = p.value;
    });

    return {
      schemaHash: q.schemaHash,
      queryFieldIndices: fieldIndices,
      queryOperators: operators,
      queryValues: values,
      numPredicates: q.predicates.length,
      compoundLogic: q.compoundLogic === 'AND' ? 0 : 1,
      verifierNonce: q.verifierNonce,
      expirationTimestamp: q.expirationTimestamp,
    };
  }
}
