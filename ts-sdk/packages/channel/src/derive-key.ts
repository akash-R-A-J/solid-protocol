/**
 * @solid-protocol/channel -- Key Derivation
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
 *
 * Security note: only call this inside a trusted wallet/extension context.
 * A normal dApp page must never receive the returned signature bytes; if a
 * malicious page can ask an external wallet to sign this fixed message, it
 * can derive the same channel secret.  Self-custodial SolID holder wallets
 * should prefer deriveChannelKeyFromMasterSeed().
 */
import nacl from 'tweetnacl';

import type { ChannelKeyPair } from './types.js';

/** The fixed message signed by the wallet to derive the channel key. */
export const CHANNEL_DERIVATION_MESSAGE = 'solid:channel:v1';

/**
 * Derive a deterministic X25519 keypair from a wallet signature.
 *
 * Intended for wallet implementations, not arbitrary web pages.  The
 * signMessage callback must be trusted not to expose the signature bytes to
 * the requesting dApp.
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
  // The buffer.slice copy is required for TS 5.9's Uint8Array<ArrayBufferLike>
  // -> BufferSource narrowing on crypto.subtle.digest.
  const sigBuf = signature.buffer.slice(
    signature.byteOffset,
    signature.byteOffset + signature.byteLength,
  ) as ArrayBuffer;
  const hashBuffer = await crypto.subtle.digest('SHA-256', sigBuf);
  const seed = new Uint8Array(hashBuffer);

  return deriveChannelKeyFromSeed(seed);
}

/**
 * Derive X25519 keypair from a raw 32-byte seed.
 * Lower-level building block; most callers want one of the higher-level
 * helpers below.
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

/**
 * Derive a channel keypair from a wallet that holds its own master seed.
 *
 * This is the path for wallets that *are* the identity (e.g. solid-wallet's
 * encrypted vault holds an identity master seed) rather than wallets that
 * *sign* on behalf of an external identity.  Two derivation paths exist
 * deliberately:
 *
 *   - `deriveChannelKeyFromWallet(signMessage)` -- for external wallets
 *     (Phantom, Solflare, Ledger).  The wallet signs a fixed domain message
 *     and the signature is hashed into the seed.  The wallet itself never
 *     learns the X25519 secret.
 *
 *   - `deriveChannelKeyFromMasterSeed(masterSeed)` -- for self-custodial
 *     wallets that already hold a high-entropy identity seed.  No signature
 *     is required; the master seed is mixed with a domain label so that
 *     channel keys are isolated from any other use of the same seed.
 *
 * Both paths must produce keys that are interoperable with
 * `encryptCredential` / `decryptCredential`.  Within a single wallet, the
 * choice of derivation must be stable -- switching paths invalidates every
 * envelope the holder previously gave their issuers.
 *
 * @param masterSeed - The wallet's identity seed, 32 bytes.
 * @param domain - Domain-separation label.  Defaults to the v1 constant.
 *   Override only if you are isolating multiple channels off the same seed.
 *
 * @example
 * ```ts
 * const channelKeys = await deriveChannelKeyFromMasterSeed(identitySeed);
 * postMessageToIssuer({ holderChannelPubKey: encodeBase64(channelKeys.publicKey) });
 * ```
 */
export async function deriveChannelKeyFromMasterSeed(
  masterSeed: Uint8Array,
  domain: string = CHANNEL_DERIVATION_MESSAGE,
): Promise<ChannelKeyPair> {
  if (masterSeed.length !== 32) {
    throw new Error(`Master seed must be 32 bytes, got ${masterSeed.length}`);
  }

  const domainBytes = new TextEncoder().encode(domain);
  const material = new Uint8Array(domainBytes.length + masterSeed.length);
  material.set(domainBytes, 0);
  material.set(masterSeed, domainBytes.length);

  const seedBuffer = await crypto.subtle.digest(
    'SHA-256',
    material.buffer.slice(
      material.byteOffset,
      material.byteOffset + material.byteLength,
    ) as ArrayBuffer,
  );
  return deriveChannelKeyFromSeed(new Uint8Array(seedBuffer));
}
