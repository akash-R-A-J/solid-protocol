/**
 * @solid-protocol/credential-channel -- Decryption (Holder Side)
 *
 * Reverses the ECIES envelope:
 *   1. Decode the envelope fields from base64
 *   2. ECDH with holder's channel secret key + issuer's ephemeral pubkey
 *   3. Decrypt with NaCl box.open (XSalsa20-Poly1305)
 *   4. Parse and validate the CredentialBundle
 */
import nacl from 'tweetnacl';
import { decodeBase64 } from 'tweetnacl-util';

import type { CredentialBundle, EncryptedEnvelope } from './types.js';

/**
 * Decrypt an encrypted credential envelope.
 *
 * @param envelope - The EncryptedEnvelope received from the issuer
 * @param channelSecretKey - The holder's X25519 secret key (32 bytes)
 * @returns The decrypted CredentialBundle
 * @throws If decryption fails (wrong key, tampered data, etc.)
 *
 * @example
 * ```ts
 * const bundle = decryptCredential(envelope, channelKeys.secretKey);
 * console.log(bundle.schemaName); // "basic_identity_v1"
 * ```
 */
export function decryptCredential(
  envelope: EncryptedEnvelope,
  channelSecretKey: Uint8Array,
): CredentialBundle {
  validateEnvelope(envelope);
  validateSecretKey(channelSecretKey);

  // 1. Decode base64 fields
  const ephemeralPubKey = decodeBase64(envelope.ephemeralPublicKey);
  const nonce = decodeBase64(envelope.nonce);
  const ciphertext = decodeBase64(envelope.ciphertext);

  // 2. Decrypt with NaCl box.open
  const plaintext = nacl.box.open(ciphertext, nonce, ephemeralPubKey, channelSecretKey);

  if (!plaintext) {
    throw new Error(
      'Decryption failed — either the channel key is wrong or the envelope was tampered with',
    );
  }

  // 3. Parse JSON
  const json = new TextDecoder().decode(plaintext);
  let bundle: CredentialBundle;
  try {
    bundle = JSON.parse(json) as CredentialBundle;
  } catch {
    throw new Error('Decrypted data is not valid JSON — envelope may be corrupted');
  }

  // 4. Basic structural validation
  validateBundle(bundle);

  return bundle;
}

/**
 * Parse a JSON string into an EncryptedEnvelope with validation.
 */
export function parseEnvelope(json: string): EncryptedEnvelope {
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch {
    throw new Error('Invalid envelope JSON');
  }

  const envelope = parsed as EncryptedEnvelope;
  validateEnvelope(envelope);
  return envelope;
}

/**
 * Validate envelope structure.
 */
function validateEnvelope(envelope: EncryptedEnvelope): void {
  if (!envelope || typeof envelope !== 'object') {
    throw new Error('Envelope must be a non-null object');
  }
  if (envelope.version !== 1) {
    throw new Error(`Unsupported envelope version: ${envelope.version}`);
  }
  if (typeof envelope.ephemeralPublicKey !== 'string' || !envelope.ephemeralPublicKey) {
    throw new Error('Missing or invalid ephemeralPublicKey');
  }
  if (typeof envelope.nonce !== 'string' || !envelope.nonce) {
    throw new Error('Missing or invalid nonce');
  }
  if (typeof envelope.ciphertext !== 'string' || !envelope.ciphertext) {
    throw new Error('Missing or invalid ciphertext');
  }
}

/**
 * Validate secret key shape.
 */
function validateSecretKey(key: Uint8Array): void {
  if (!(key instanceof Uint8Array) || key.length !== 32) {
    throw new Error(`Invalid secret key: expected 32-byte Uint8Array, got ${key?.length ?? 'null'}`);
  }
}

/**
 * Structural validation of a decrypted CredentialBundle.
 * Does NOT verify cryptographic integrity — that's the circuit's job.
 */
function validateBundle(bundle: CredentialBundle): void {
  const required: (keyof CredentialBundle)[] = [
    'version',
    'schemaHash',
    'attestationData',
    'issuerSignature',
    'issuerPublicKey',
    'holderPublicKey',
    'salt',
    'commitment',
    'treeAddress',
    'merkleProof',
  ];

  for (const field of required) {
    if (bundle[field] === undefined || bundle[field] === null) {
      throw new Error(`CredentialBundle is missing required field: ${field}`);
    }
  }

  if (!Array.isArray(bundle.attestationData) || bundle.attestationData.length === 0) {
    throw new Error('attestationData must be a non-empty array');
  }
}
