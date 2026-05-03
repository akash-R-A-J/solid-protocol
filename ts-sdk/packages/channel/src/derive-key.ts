/**
 * @solid-protocol/credential-channel -- Key Derivation
 *
 * Derives a deterministic X25519 keypair from a Solana wallet signature.
 *
 * The derivation path is:
 *   wallet.signMessage("solid:channel:v1")
 *     -> SHA-256(signature_bytes)
 *     -> X25519 seed (first 32 bytes)
 *     -> nacl.box.keyPair.fromSecretKey(seed)
 *
 * This means:
 *   - Same wallet = same channel key (deterministic, recoverable)
 *   - Different wallets = different keys (isolated)
 *   - No extra key backup needed -- wallet seed recovers everything
 */
import nacl from 'tweetnacl';

import type { ChannelKeyPair } from './types.js';

/** The fixed message signed by the wallet to derive the channel key. */
export const CHANNEL_DERIVATION_MESSAGE = 'solid:channel:v1';

/**
 * Derive a deterministic X25519 keypair from a wallet signature.
 *
 * @param signMessage - The wallet's signMessage function
 *   (e.g. `phantom.solana.signMessage` or `wallet.signMessage`)
 * @returns A stable X25519 keypair for this wallet
 *
 * @example
 * ```ts
 * const channelKeys = await deriveChannelKeyFromWallet(
 *   (msg) => wallet.signMessage(new TextEncoder().encode(msg))
 * );
 * ```
 */
export async function deriveChannelKeyFromWallet(
  signMessage: (message: Uint8Array) => Promise<Uint8Array>,
): Promise<ChannelKeyPair> {
  const messageBytes = new TextEncoder().encode(CHANNEL_DERIVATION_MESSAGE);
  const signature = await signMessage(messageBytes);

  // SHA-256 the signature to get a uniform 32-byte seed.
  // This is important: raw Ed25519 signatures are 64 bytes and have
  // internal structure that could leak wallet info if used directly.
  const hashBuffer = await crypto.subtle.digest('SHA-256', signature);
  const seed = new Uint8Array(hashBuffer);

  return deriveChannelKeyFromSeed(seed);
}

/**
 * Derive X25519 keypair from a raw 32-byte seed.
 * Exposed for testing; production code should use `deriveChannelKeyFromWallet`.
 */
export function deriveChannelKeyFromSeed(seed: Uint8Array): ChannelKeyPair {
  if (seed.length !== 32) {
    throw new Error(`Channel key seed must be 32 bytes, got ${seed.length}`);
  }

  const keyPair = nacl.box.keyPair.fromSecretKey(seed);
  return {
    publicKey: keyPair.publicKey,
    secretKey: keyPair.secretKey,
  };
}
