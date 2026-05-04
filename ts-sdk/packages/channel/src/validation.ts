import { decodeBase64 } from 'tweetnacl-util';

import type { CredentialBundle, EncryptedEnvelope } from './types.js';

export const ENVELOPE_VERSION = 1;
export const CREDENTIAL_BUNDLE_VERSION = 1;
export const X25519_PUBLIC_KEY_BYTES = 32;
export const X25519_SECRET_KEY_BYTES = 32;
export const NACL_BOX_NONCE_BYTES = 24;
export const NUM_FIELDS = 8;
export const MAX_MERKLE_PROOF_DEPTH = 64;

const BN254_FIELD_PRIME =
  21888242871839275222246405745257275088548364400416034343698204186575808495617n;
const MAX_U64 = (1n << 64n) - 1n;
const HEX_32 = /^[0-9a-f]{64}$/i;
const BASE58 = /^[1-9A-HJ-NP-Za-km-z]+$/;
const DECIMAL = /^(0|[1-9][0-9]*)$/;

export interface ValidationResult {
  ok: boolean;
  errors: string[];
}

export function validateEncryptedEnvelope(envelope: unknown): ValidationResult {
  const errors: string[] = [];
  if (!isRecord(envelope)) {
    return { ok: false, errors: ['Envelope must be a non-null object'] };
  }

  if (envelope.version !== ENVELOPE_VERSION) {
    errors.push(`Unsupported envelope version: ${String(envelope.version)}`);
  }
  expectBase64Length(envelope.ephemeralPublicKey, X25519_PUBLIC_KEY_BYTES, 'ephemeralPublicKey', errors);
  expectBase64Length(envelope.nonce, NACL_BOX_NONCE_BYTES, 'nonce', errors);
  expectBase64MinLength(envelope.ciphertext, 17, 'ciphertext', errors);

  return { ok: errors.length === 0, errors };
}

export function assertEncryptedEnvelope(envelope: unknown): asserts envelope is EncryptedEnvelope {
  const result = validateEncryptedEnvelope(envelope);
  if (!result.ok) {
    throw new Error(result.errors.join('; '));
  }
}

export function validateCredentialBundle(bundle: unknown): ValidationResult {
  const errors: string[] = [];
  if (!isRecord(bundle)) {
    return { ok: false, errors: ['CredentialBundle must be a non-null object'] };
  }

  if (bundle.version !== CREDENTIAL_BUNDLE_VERSION) {
    errors.push(`CredentialBundle version must be ${CREDENTIAL_BUNDLE_VERSION}`);
  }
  expectHex32(bundle.schemaHash, 'schemaHash', errors, { nonZero: true });
  expectNonEmptyString(bundle.schemaName, 'schemaName', errors);
  expectAttestationData(bundle.attestationData, errors);
  expectSignature(bundle.issuerSignature, errors);
  expectPublicKey(bundle.issuerPublicKey, 'issuerPublicKey', errors);
  expectPublicKey(bundle.holderPublicKey, 'holderPublicKey', errors);
  expectFieldElement(bundle.salt, 'salt', errors);
  expectHex32(bundle.commitment, 'commitment', errors, { nonZero: true });
  expectUnixTimestamp(bundle.expirationTimestamp, 'expirationTimestamp', errors);
  expectBase58Pubkey(bundle.treeAddress, 'treeAddress', errors);
  expectMerkleProof(bundle.merkleProof, errors);
  expectNonEmptyString(bundle.issuerName, 'issuerName', errors);
  expectBase58Pubkey(bundle.issuerAuthority, 'issuerAuthority', errors);
  expectDecimal(bundle.issuerStatusEpoch, 'issuerStatusEpoch', errors);
  expectDecimal(bundle.issuerRevocationNonce, 'issuerRevocationNonce', errors);
  expectDecimal(bundle.issuerTreeLeafIndex, 'issuerTreeLeafIndex', errors);

  return { ok: errors.length === 0, errors };
}

export function assertCredentialBundle(bundle: unknown): asserts bundle is CredentialBundle {
  const result = validateCredentialBundle(bundle);
  if (!result.ok) {
    throw new Error(result.errors.join('; '));
  }
}

export function decodeBase64Field(value: string, expectedBytes: number, name: string): Uint8Array {
  const bytes = decodeBase64(value);
  if (bytes.length !== expectedBytes) {
    throw new Error(`${name} must decode to ${expectedBytes} bytes, got ${bytes.length}`);
  }
  return bytes;
}

export function assertChannelSecretKey(key: Uint8Array): void {
  if (!(key instanceof Uint8Array) || key.length !== X25519_SECRET_KEY_BYTES) {
    throw new Error(
      `Invalid secret key: expected ${X25519_SECRET_KEY_BYTES}-byte Uint8Array, got ${key?.length ?? 'null'}`,
    );
  }
}

export function assertChannelPublicKey(key: Uint8Array): void {
  if (!(key instanceof Uint8Array) || key.length !== X25519_PUBLIC_KEY_BYTES) {
    throw new Error(
      `Invalid public key: expected ${X25519_PUBLIC_KEY_BYTES}-byte Uint8Array, got ${key?.length ?? 'null'}`,
    );
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}

function expectBase64Length(
  value: unknown,
  expectedBytes: number,
  name: string,
  errors: string[],
): void {
  if (typeof value !== 'string' || value.length === 0) {
    errors.push(`${name} must be a non-empty base64 string`);
    return;
  }
  try {
    const bytes = decodeBase64(value);
    if (bytes.length !== expectedBytes) {
      errors.push(`${name} must decode to ${expectedBytes} bytes, got ${bytes.length}`);
    }
  } catch {
    errors.push(`${name} must be valid base64`);
  }
}

function expectBase64MinLength(
  value: unknown,
  minBytes: number,
  name: string,
  errors: string[],
): void {
  if (typeof value !== 'string' || value.length === 0) {
    errors.push(`${name} must be a non-empty base64 string`);
    return;
  }
  try {
    const bytes = decodeBase64(value);
    if (bytes.length < minBytes) {
      errors.push(`${name} must decode to at least ${minBytes} bytes, got ${bytes.length}`);
    }
  } catch {
    errors.push(`${name} must be valid base64`);
  }
}

function expectNonEmptyString(value: unknown, name: string, errors: string[]): void {
  if (typeof value !== 'string' || value.trim().length === 0) {
    errors.push(`${name} must be a non-empty string`);
  }
}

function expectHex32(
  value: unknown,
  name: string,
  errors: string[],
  options: { nonZero?: boolean } = {},
): void {
  if (typeof value !== 'string' || !HEX_32.test(strip0x(value))) {
    errors.push(`${name} must be a 32-byte hex string`);
    return;
  }
  if (options.nonZero && /^0{64}$/i.test(strip0x(value))) {
    errors.push(`${name} must not be the reserved all-zero value`);
  }
}

function expectAttestationData(value: unknown, errors: string[]): void {
  if (!Array.isArray(value)) {
    errors.push('attestationData must be an array');
    return;
  }
  if (value.length !== NUM_FIELDS) {
    errors.push(`attestationData must contain exactly ${NUM_FIELDS} fields`);
  }
  value.forEach((field, index) => {
    expectDecimal(field, `attestationData[${index}]`, errors, MAX_U64);
  });
}

function expectSignature(value: unknown, errors: string[]): void {
  if (!isRecord(value)) {
    errors.push('issuerSignature must be an object');
    return;
  }
  expectFieldElement(value.R8x, 'issuerSignature.R8x', errors);
  expectFieldElement(value.R8y, 'issuerSignature.R8y', errors);
  expectFieldElement(value.S, 'issuerSignature.S', errors);
}

function expectPublicKey(value: unknown, name: string, errors: string[]): void {
  if (!isRecord(value)) {
    errors.push(`${name} must be an object`);
    return;
  }
  expectFieldElement(value.x, `${name}.x`, errors);
  expectFieldElement(value.y, `${name}.y`, errors);
}

function expectMerkleProof(value: unknown, errors: string[]): void {
  if (!isRecord(value)) {
    errors.push('merkleProof must be an object');
    return;
  }
  const siblings = value.siblings;
  const pathIndices = value.pathIndices;
  if (!Array.isArray(siblings) || !Array.isArray(pathIndices)) {
    errors.push('merkleProof.siblings and merkleProof.pathIndices must be arrays');
    return;
  }
  if (siblings.length !== pathIndices.length) {
    errors.push('merkleProof.siblings and merkleProof.pathIndices must have the same length');
  }
  if (siblings.length > MAX_MERKLE_PROOF_DEPTH) {
    errors.push(`merkleProof depth must be <= ${MAX_MERKLE_PROOF_DEPTH}`);
  }
  siblings.forEach((sibling, index) => {
    expectHex32(sibling, `merkleProof.siblings[${index}]`, errors);
  });
  pathIndices.forEach((pathIndex, index) => {
    if (pathIndex !== 0 && pathIndex !== 1) {
      errors.push(`merkleProof.pathIndices[${index}] must be 0 or 1`);
    }
  });
  if (typeof value.leafIndex !== 'number' || !Number.isSafeInteger(value.leafIndex) || value.leafIndex < 0) {
    errors.push('merkleProof.leafIndex must be a non-negative safe integer');
  }
}

function expectUnixTimestamp(value: unknown, name: string, errors: string[]): void {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) {
    errors.push(`${name} must be a non-negative Unix timestamp in seconds`);
  }
}

function expectBase58Pubkey(value: unknown, name: string, errors: string[]): void {
  if (typeof value !== 'string' || value.length < 32 || value.length > 44 || !BASE58.test(value)) {
    errors.push(`${name} must be a base58 Solana public key string`);
  }
}

function expectFieldElement(value: unknown, name: string, errors: string[]): void {
  expectDecimal(value, name, errors, BN254_FIELD_PRIME - 1n);
}

function expectDecimal(
  value: unknown,
  name: string,
  errors: string[],
  max?: bigint,
): void {
  if (typeof value !== 'string' || !DECIMAL.test(value)) {
    errors.push(`${name} must be a canonical unsigned decimal string`);
    return;
  }
  if (max != null && BigInt(value) > max) {
    errors.push(`${name} exceeds the allowed range`);
  }
}

function strip0x(value: string): string {
  return value.startsWith('0x') ? value.slice(2) : value;
}
