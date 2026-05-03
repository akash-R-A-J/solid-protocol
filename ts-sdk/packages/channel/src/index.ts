/**
 * @solid-protocol/credential-channel
 *
 * Secure ECIES credential delivery for SolID Protocol.
 *
 * Public API surface:
 *   - deriveChannelKeyFromWallet() — Holder derives X25519 key from wallet
 *   - encryptCredential()         — Issuer encrypts a credential bundle
 *   - decryptCredential()         — Holder decrypts a credential envelope
 *   - serializeEnvelope()         — Convert envelope to JSON string
 *   - parseEnvelope()             — Parse JSON string to envelope
 *
 * @example
 * ```ts
 * // ISSUER SIDE
 * import { encryptCredential } from '@solid-protocol/credential-channel';
 * const envelope = encryptCredential(bundle, holderChannelPubKey);
 *
 * // HOLDER SIDE
 * import { deriveChannelKeyFromWallet, decryptCredential } from '@solid-protocol/credential-channel';
 * const channelKeys = await deriveChannelKeyFromWallet(wallet.signMessage);
 * const bundle = decryptCredential(envelope, channelKeys.secretKey);
 * ```
 */

// Key derivation
export { deriveChannelKeyFromWallet, deriveChannelKeyFromSeed, CHANNEL_DERIVATION_MESSAGE } from './derive-key.js';

// Encryption (issuer side)
export { encryptCredential, serializeEnvelope } from './encrypt.js';

// Decryption (holder side)
export { decryptCredential, parseEnvelope } from './decrypt.js';

// Types
export type {
  CredentialBundle,
  EncryptedEnvelope,
  ChannelKeyPair,
  BJJSignature,
  BJJPublicKey,
  MerkleProof,
} from './types.js';
