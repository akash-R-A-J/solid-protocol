import {
  initWasm,
  QueryBuilder,
  MultiCredentialQuery,
  BJJKeypair,
  generateKeypair as coreGenerateKeypair,
  PROGRAM_IDS,
} from '@solid-protocol/core';
import {
  generateBatchProof,
  type BatchProofResult,
  type StoredCredential,
  type MerkleProofSource,
} from '@solid-protocol/holder';
import {
  verifyOnChain as realVerifyOnChain,
  type SchemaTreeAccounts,
  type VerificationRequest,
  type VerificationResult,
  deriveNullifierPda,
  deriveVerifierConfigPda,
  deriveVkStoragePda,
  checkIssuerStatus,
  extractWirePublicInputs,
} from '@solid-protocol/verifier';
import { SOLID_CONFIG } from './config';
import { ResilientConnection } from './rpc';
import {
  verifyArtifactSha256,
  WASM_PIN,
  ZKEY_PIN,
  VK_PIN,
  type ArtifactPin,
} from './artifact_integrity';
import { Connection, Keypair, PublicKey } from '@solana/web3.js';
import { Buffer } from 'buffer';
import * as fs from 'fs';
import * as path from 'path';

/**
 * Read an `<artifact>.sha256` sidecar file if present.  Returns the
 * lowercase-hex content (trimmed) or `undefined` if the file is missing
 * or unreadable.  64-char validation is done by `verifyArtifactSha256`.
 */
function readSidecar(sidecarPath: string): string | undefined {
  try {
    if (!fs.existsSync(sidecarPath)) return undefined;
    return fs.readFileSync(sidecarPath, 'utf-8');
  } catch {
    return undefined;
  }
}

/**
 * Load an artifact from a URL or local path and verify its SHA-256
 * against the configured pin sources.  Throws on mismatch.  Returns the
 * raw bytes (suitable for passing to `snarkjs.groth16.fullProve`).
 *
 * SOLID-SEC-058 / CRIT-3.  See `artifact_integrity.ts` for the pin
 * resolution order and the `SOLID_CIRCUIT_ARTIFACT_INTEGRITY=skip`
 * dev escape hatch.
 */
async function loadAndVerifyArtifact(
  pathOrUrl: string,
  pin: ArtifactPin,
  name: string,
): Promise<Uint8Array> {
  const isHttp = /^https?:\/\//.test(pathOrUrl);
  const cameFromCdn =
    isHttp && pathOrUrl.startsWith(SOLID_CONFIG.ARTIFACT_BASE_URL);

  let bytes: Uint8Array;
  if (isHttp) {
    const res = await fetch(pathOrUrl);
    if (!res.ok) {
      throw new Error(
        `[artifact-integrity] ${name}: failed to fetch ${pathOrUrl} ` +
          `(HTTP ${res.status})`,
      );
    }
    const buf = await res.arrayBuffer();
    bytes = new Uint8Array(buf);
  } else {
    const resolved = path.isAbsolute(pathOrUrl)
      ? pathOrUrl
      : path.resolve(process.cwd(), pathOrUrl);
    bytes = new Uint8Array(fs.readFileSync(resolved));
  }

  verifyArtifactSha256(bytes, pin, readSidecar, name, cameFromCdn);
  return bytes;
}

/**
 * SolID Protocol SDK
 *
 * High-level facade over the lower-level @solid-protocol/{core,holder,verifier,light}
 * packages. The facade is thin: every method delegates to the package that
 * already owns the real logic. In particular, neither prove() nor verifyOnChain()
 * contain any stub output: they call the real Groth16 prover and submit a
 * real confirmed verify_batch_proof transaction.
 */
export class SolID {
  private static _initialized = false;
  private static _rpc: ResilientConnection;
  private static _connection: Connection;

  /**
   * Initialize the SDK. Must be called before any proving or verification.
   */
  static async initialize(): Promise<void> {
    if (this._initialized) return;
    await initWasm();

    this._rpc = new ResilientConnection(SOLID_CONFIG.SOLANA_RPC_URLS, 'confirmed');
    this._connection = this._rpc.connection;

    this._initialized = true;
  }

  /** Expose the resilient Solana RPC client so downstream packages
   *  (issuer/holder) can share the same endpoint pool and failover state. */
  static get rpc(): ResilientConnection {
    if (!this._initialized) {
      throw new Error('SolID SDK: call SolID.initialize() before accessing rpc.');
    }
    return this._rpc;
  }

  /**
   * Generate a new master identity keypair.
   */
  static generateMasterIdentity(): BJJKeypair {
    return coreGenerateKeypair();
  }

  /**
   * Prove a multi-credential compound query.
   *
   * Delegates to @solid-protocol/holder::generateBatchProof with circuit
   * artifact paths sourced from SOLID_CONFIG. The caller supplies the
   * credentials, the master key, the global-state tree, and a
   * MerkleProofAdapter.
   */
  static async prove(params: {
    query: MultiCredentialQuery;
    credentials: StoredCredential[];
    masterPrivateKey: Uint8Array;
    masterPublicKey: { x: Uint8Array; y: Uint8Array };
    revocationNonce: bigint;
    globalStateTree: PublicKey;
    /** ADR-0014: SPL AC account backing the singleton issuer tree. */
    issuerMerkleTree: PublicKey;
    /** ADR-0014: the singleton issuer-tree root the caller claims is current. */
    issuerTreeRoot: Uint8Array;
    merkleProofAdapter: MerkleProofSource['merkleProofAdapter'];
    circuitPaths?: { wasmPath: string; zkeyPath: string };
  }): Promise<BatchProofResult> {
    if (!this._initialized) await this.initialize();
    const sourcePaths = params.circuitPaths ?? {
      wasmPath: `${SOLID_CONFIG.ARTIFACT_BASE_URL}${SOLID_CONFIG.CIRCUIT_METADATA.BATCH_QUERY.WASM_PATH}`,
      zkeyPath: `${SOLID_CONFIG.ARTIFACT_BASE_URL}${SOLID_CONFIG.CIRCUIT_METADATA.BATCH_QUERY.ZKEY_PATH}`,
    };

    // SOLID-SEC-058 / CRIT-3: load each artifact, compute SHA-256, refuse
    // on mismatch / refuse on missing pin for CDN-loaded artifacts.  Pass
    // the verified bytes (Uint8Array) downstream -- snarkjs.groth16.fullProve
    // accepts Buffer/Uint8Array as well as path/URL strings.  This removes
    // the TOCTOU between hash check and snarkjs's own re-fetch.
    const wasmBytes = await loadAndVerifyArtifact(
      sourcePaths.wasmPath,
      WASM_PIN,
      'batch_credential_query.wasm',
    );
    const zkeyBytes = await loadAndVerifyArtifact(
      sourcePaths.zkeyPath,
      ZKEY_PIN,
      'batch_credential_query.zkey',
    );

    return generateBatchProof(
      params.query,
      params.credentials,
      params.masterPrivateKey,
      params.masterPublicKey,
      params.revocationNonce,
      { wasmPath: wasmBytes as unknown as string, zkeyPath: zkeyBytes as unknown as string },
      {
        merkleProofAdapter: params.merkleProofAdapter,
        globalStateTree: params.globalStateTree,
        issuerMerkleTree: params.issuerMerkleTree,
        issuerTreeRoot: params.issuerTreeRoot,
      },
    );
  }

  /**
   * Submit a proof to the on-chain ZK verifier.
   *
   * Delegates to @solid-protocol/verifier::verifyOnChain which builds and
   * confirms a real verify_batch_proof transaction. Returns the transaction
   * signature of the confirmed submission, not a placeholder string.
   */
  static async verifyOnChain(params: {
    payer: Keypair;
    proof: BatchProofResult;
    query: MultiCredentialQuery;
    trees: SchemaTreeAccounts;
  }): Promise<VerificationResult> {
    if (!this._initialized) await this.initialize();

    // SOLID-SEC-054 / B13: extract the 21-element wire subset from the
    // full 32-element snarkjs publicSignals output.  The on-chain handler
    // reconstructs the remaining 11 slots from accounts.
    const fullPublicInputs = params.proof.publicSignals.map(s =>
      bigintToBytes32BE(BigInt(s)),
    );
    const request: VerificationRequest = {
      query: params.query,
      proofData: {
        proof_a: params.proof.solanaProof.proofA,
        proof_b: params.proof.solanaProof.proofB,
        proof_c: params.proof.solanaProof.proofC,
        publicInputs: extractWirePublicInputs(fullPublicInputs),
        nullifier: params.proof.nullifier,
      },
    };

    return realVerifyOnChain(
      this._connection,
      params.payer,
      request,
      params.trees,
      new PublicKey(PROGRAM_IDS.zkVerifier),
    );
  }

  /**
   * Resolve a schema hash to its on-chain SchemaAccount PDA.
   *
   * The pre-remediation implementation filtered by a wrong memcmp offset and
   * always returned empty. The Borsh layout of SchemaAccount uses
   * len-prefixed Strings, so a static offset against name + category cannot
   * work. We do a cheap full scan of program accounts (the registry is
   * small; production integrators should layer a proper indexer on top).
   */
  static async resolveSchema(schemaHash: Uint8Array): Promise<PublicKey | null> {
    if (!this._initialized) await this.initialize();
    const schemaProgramId = new PublicKey(SOLID_CONFIG.PROGRAM_IDS.SCHEMA_REGISTRY);
    const accounts = await this._connection.getProgramAccounts(schemaProgramId);
    const target = Buffer.from(schemaHash);
    for (const entry of accounts) {
      // SchemaAccount layout relative to the 8-byte Anchor discriminator:
      //   [8..40)  authority
      //   [40..]   name (4-byte len + bytes)
      //   ...      version (u8)
      //   ...      category (4-byte len + bytes)
      //   ...      field_names (Vec<String>)
      //   [...+32) schema_hash
      // Parse schema_hash by walking length prefixes rather than a fixed
      // offset. The field ordering must match register_schema.
      const data = entry.account.data;
      try {
        let off = 8 + 32;
        // name
        const nameLen = data.readUInt32LE(off); off += 4 + nameLen;
        // version
        off += 1;
        // category
        const catLen = data.readUInt32LE(off); off += 4 + catLen;
        // field_names
        const fieldsLen = data.readUInt32LE(off); off += 4;
        for (let i = 0; i < fieldsLen; i++) {
          const sLen = data.readUInt32LE(off); off += 4 + sLen;
        }
        const hash = data.subarray(off, off + 32);
        if (hash.equals(target)) return entry.pubkey;
      } catch {
        continue;
      }
    }
    return null;
  }

  /**
   * List all IssuerAccount PDAs whose status is Approved (variant 1).
   *
   * Pre-remediation code filtered with an offset that ignored the
   * 4-byte Borsh length prefixes on name/metadata_uri. We walk the layout
   * here and keep only Approved entries.
   */
  static async listIssuers(): Promise<PublicKey[]> {
    if (!this._initialized) await this.initialize();
    const registryId = new PublicKey(SOLID_CONFIG.PROGRAM_IDS.ISSUER_REGISTRY);
    const accounts = await this._connection.getProgramAccounts(registryId);
    const approved: PublicKey[] = [];
    for (const entry of accounts) {
      const data = entry.account.data;
      try {
        let off = 8 + 32;
        const nameLen = data.readUInt32LE(off); off += 4 + nameLen;
        const metaLen = data.readUInt32LE(off); off += 4 + metaLen;
        off += 32 + 32; // bjj_pub_key_x, bjj_pub_key_y
        off += 1;       // tier
        const status = data.readUInt8(off);
        if (status === 1) approved.push(entry.pubkey); // IssuerStatus::Approved = 1
      } catch {
        continue;
      }
    }
    return approved;
  }
}

function bigintToBytes32BE(n: bigint): Uint8Array {
  const hex = n.toString(16).padStart(64, '0');
  const out = new Uint8Array(32);
  for (let i = 0; i < 32; i++) {
    out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

// Re-export core types and lower-level helpers for convenience.
export * from '@solid-protocol/core';
export {
  deriveNullifierPda,
  deriveVerifierConfigPda,
  deriveVkStoragePda,
  checkIssuerStatus,
};
export type {
  SchemaTreeAccounts,
  VerificationRequest,
  VerificationResult,
} from '@solid-protocol/verifier';
export { SOLID_CONFIG } from './config';
export * from './manifest';
export * from './indexer';

// SOLID-SEC-058 / CRIT-3: artifact integrity helpers re-exported so
// downstream callers (scripts/prove.ts, third-party integrators) can
// verify off-chain prover artifacts before handing them to snarkjs.
export {
  loadAndVerifyArtifact,
};
export {
  verifyArtifactSha256,
  resolveExpectedSha256,
  sha256Hex,
  WASM_PIN,
  ZKEY_PIN,
  VK_PIN,
  // SEC-048 Phase E.4 (2026-05-XX): subgroup-circuit pins.
  SUBGROUP_WASM_PIN,
  SUBGROUP_ZKEY_PIN,
  SUBGROUP_VK_PIN,
  type ArtifactPin,
  type ArtifactKind,
} from './artifact_integrity';
