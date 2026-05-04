/**
 * @solid-protocol/channel
 *
 * Type definitions for the secure ECIES credential delivery channel.
 *
 * The CredentialBundle is the cleartext payload an issuer hands to a holder
 * after issuing a credential on-chain.  The EncryptedEnvelope is the
 * wire-safe wrapper that protects it in transit.
 */

/** Raw BJJ EdDSA signature components. */
export interface BJJSignature {
  /** R8 x-coordinate (decimal string of BN254 field element). */
  R8x: string;
  /** R8 y-coordinate (decimal string of BN254 field element). */
  R8y: string;
  /** Scalar S (decimal string). */
  S: string;
}

/** Issuer's BabyJubJub public key. */
export interface BJJPublicKey {
  /** x-coordinate in circomlib-native form (decimal string). */
  x: string;
  /** y-coordinate in circomlib-native form (decimal string). */
  y: string;
}

/** SPL Account Compression Merkle proof for a credential leaf. */
export interface MerkleProof {
  /** Sibling hashes, one per tree level (hex strings, bottom-up). */
  siblings: string[];
  /** Path direction per level: 0 = left, 1 = right. */
  pathIndices: number[];
  /** Leaf index in the tree. */
  leafIndex: number;
}

/**
 * The cleartext credential bundle delivered from issuer to holder.
 *
 * Contains everything the holder needs to generate a Groth16 proof.
 * Does NOT contain any private keys -- only the holder's BJJ pubkey
 * (which is public) and the issuer's signature over the commitment.
 */
export interface CredentialBundle {
  /** Protocol version for forward compatibility. */
  version: number;

  /** Schema hash (hex, 32 bytes). */
  schemaHash: string;

  /** Human-readable schema name (e.g. "basic_identity_v1"). */
  schemaName: string;

  /** Attestation field values (decimal strings of BN254 field elements). */
  attestationData: string[];

  /** Issuer's EdDSA-Poseidon signature over the credential commitment. */
  issuerSignature: BJJSignature;

  /** Issuer's BabyJubJub public key. */
  issuerPublicKey: BJJPublicKey;

  /** Holder's BabyJubJub public key (bound to the credential). */
  holderPublicKey: BJJPublicKey;

  /** Salt used in the commitment hash (decimal string). */
  salt: string;

  /** The Poseidon commitment stored on-chain (hex, 32 bytes). */
  commitment: string;

  /** Unix timestamp (seconds) after which the credential expires.  0 = no expiry. */
  expirationTimestamp: number;

  /** SPL Account Compression tree address (base58). */
  treeAddress: string;

  /** Merkle proof for the credential leaf in the schema tree. */
  merkleProof: MerkleProof;

  /** Issuer metadata for display purposes. */
  issuerName: string;

  /** Issuer's Solana authority pubkey (base58). */
  issuerAuthority: string;

  /** Issuer status epoch at time of issuance. */
  issuerStatusEpoch: string;

  /** Issuer revocation nonce at time of issuance. */
  issuerRevocationNonce: string;

  /**
   * Issuer's leaf index in the issuer tree at time of issuance (decimal string).
   *
   * Captured at issuance so the holder can request the correct Merkle path
   * for the issuer-tree membership proof at proof-generation time.  May be
   * stale if the issuer was revoked-and-readmitted; the holder should
   * refresh from the on-chain IssuerAccount before relying on it.
   */
  issuerTreeLeafIndex: string;
}

/**
 * ECIES encrypted envelope -- the wire format for credential delivery.
 *
 * Self-contained: any transport (HTTPS, QR, email, Dialect) can carry it.
 * Decryption requires the holder's channel private key (derived from wallet).
 */
export interface EncryptedEnvelope {
  /** Envelope format version.  Always 1 for this release. */
  version: 1;

  /** Issuer's ephemeral X25519 public key (base64, 32 bytes). */
  ephemeralPublicKey: string;

  /** Unique nonce for this envelope (base64, 24 bytes). */
  nonce: string;

  /** NaCl box ciphertext of the JSON-serialized CredentialBundle (base64). */
  ciphertext: string;
}

/**
 * A holder's channel keypair, derived deterministically from a wallet signature.
 */
export interface ChannelKeyPair {
  /** X25519 public key (Uint8Array, 32 bytes). */
  publicKey: Uint8Array;

  /** X25519 secret key (Uint8Array, 32 bytes). */
  secretKey: Uint8Array;
}
