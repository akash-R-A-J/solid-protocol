/**
 * @solid-protocol/channel -- Encryption (Issuer Side)
 *
 * Implements ECIES (Elliptic Curve Integrated Encryption Scheme):
 *   1. Generate ephemeral X25519 keypair
 *   2. ECDH with holder's channel public key -> shared secret
 *   3. Encrypt credential bundle with NaCl secretbox (XSalsa20-Poly1305)
 *   4. Package into self-contained EncryptedEnvelope
 */
import nacl from 'tweetnacl';
import { encodeBase64 } from 'tweetnacl-util';

import type { CredentialBundle, EncryptedEnvelope } from './types.js';
import { assertChannelPublicKey, assertCredentialBundle } from './validation.js';

/**
 * Encrypt a credential bundle for a specific holder.
 *
 * @param bundle - The cleartext credential bundle
 * @param holderChannelPublicKey - The holder's X25519 public key (32 bytes)
 * @returns An EncryptedEnvelope safe for any transport
 *
 * @example
 * ```ts
 * const envelope = encryptCredential(bundle, holderPubKey);
 * // Send `envelope` over HTTPS, QR, email, etc.
 * ```
 */
export function encryptCredential(
  bundle: CredentialBundle,
  holderChannelPublicKey: Uint8Array,
): EncryptedEnvelope {
  assertCredentialBundle(bundle);
  assertChannelPublicKey(holderChannelPublicKey);

  // 1. Generate ephemeral keypair (fresh per envelope = forward secrecy)
  const ephemeral = nacl.box.keyPair();

  // 2. Generate random nonce
  const nonce = nacl.randomBytes(nacl.box.nonceLength);

  // 3. Serialize the bundle to JSON bytes
  const plaintext = new TextEncoder().encode(JSON.stringify(bundle));

  // 4. Encrypt with NaCl box (ECDH + XSalsa20-Poly1305)
  const ciphertext = nacl.box(plaintext, nonce, holderChannelPublicKey, ephemeral.secretKey);

  if (!ciphertext) {
    throw new Error('Encryption failed — this should never happen with valid inputs');
  }

  // 5. Package into envelope
  return {
    version: 1,
    ephemeralPublicKey: encodeBase64(ephemeral.publicKey),
    nonce: encodeBase64(nonce),
    ciphertext: encodeBase64(ciphertext),
  };
}

/**
 * Serialize an EncryptedEnvelope to a JSON string for transport.
 */
export function serializeEnvelope(envelope: EncryptedEnvelope): string {
  return JSON.stringify(envelope);
}
