/**
 * @solid-protocol/channel -- Decryption (Holder Side)
 *
 * Reverses the ECIES envelope:
 *   1. Decode the envelope fields from base64
 *   2. ECDH with holder's channel secret key + issuer's ephemeral pubkey
 *   3. Decrypt with NaCl box.open (XSalsa20-Poly1305)
 *   4. Parse and validate the CredentialBundle
 */
import nacl from 'tweetnacl';

import type { CredentialBundle, EncryptedEnvelope } from './types.js';
import {
  assertChannelSecretKey,
  assertCredentialBundle,
  assertEncryptedEnvelope,
  decodeBase64Field,
  NACL_BOX_NONCE_BYTES,
  X25519_PUBLIC_KEY_BYTES,
} from './validation.js';

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
  assertEncryptedEnvelope(envelope);
  assertChannelSecretKey(channelSecretKey);

  // 1. Decode base64 fields
  const ephemeralPubKey = decodeBase64Field(
    envelope.ephemeralPublicKey,
    X25519_PUBLIC_KEY_BYTES,
    'ephemeralPublicKey',
  );
  const nonce = decodeBase64Field(envelope.nonce, NACL_BOX_NONCE_BYTES, 'nonce');
  const ciphertext = decodeBase64Field(envelope.ciphertext, decodeCiphertextLength(envelope.ciphertext), 'ciphertext');

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

  // 4. Protocol-level structural validation. Cryptographic validity
  // (commitment recomputation, signature verification, on-chain roots) is
  // enforced by the wallet/holder/verifier packages.
  assertCredentialBundle(bundle);

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
  assertEncryptedEnvelope(envelope);
  return envelope;
}

function decodeCiphertextLength(base64: string): number {
  const padding = base64.endsWith('==') ? 2 : base64.endsWith('=') ? 1 : 0;
  return Math.floor((base64.length * 3) / 4) - padding;
}
