//! Compressed credential account type for Light Protocol state trees.
//!
//! Each credential is stored as a compressed account containing
//! the Poseidon commitment, schema hash, and issuer pubkey.
//! The actual credential data never touches the chain — only
//! the commitment leaf is stored in the Merkle tree.

use borsh::{BorshDeserialize, BorshSerialize};

/// A compressed credential leaf stored in a Light Protocol state tree.
///
/// This is the on-chain representation of an issued credential.
/// The full credential data is held off-chain by the holder.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CompressedCredential {
    /// Poseidon commitment = H(dataHash, schemaHash, holderPubX, holderPubY, salt)
    pub commitment: [u8; 32],

    /// Schema hash identifying the credential type
    pub schema_hash: [u8; 32],

    /// Issuer's Solana public key (for registry lookup)
    pub issuer: [u8; 32],

    /// Issuance timestamp
    pub created_at: i64,

    /// Whether this credential has been revoked
    pub revoked: bool,
}

impl CompressedCredential {
    pub const DISCRIMINATOR: [u8; 8] = *b"solidcrd";

    pub fn new(commitment: [u8; 32], schema_hash: [u8; 32], issuer: [u8; 32]) -> Self {
        Self {
            commitment,
            schema_hash,
            issuer,
            created_at: 0, // Set during CPI
            revoked: false,
        }
    }

    /// Serialized size for account allocation.
    pub const fn size() -> usize {
        8 + // discriminator
        32 + // commitment
        32 + // schema_hash
        32 + // issuer
        8 + // created_at
        1 // revoked
    }
}

/// A compressed nullifier leaf to prevent proof replay.
///
/// In the "Great Infra" design, nullifiers are stored in a dedicated Merkle tree.
/// Each proof "shields" a nullifier; if the leaf already exists, the creation fails.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CompressedNullifier {
    /// The unique nullifier = Poseidon(masterKey, verifierAddress, schemaHash)
    pub nullifier: [u8; 32],
    pub created_at: i64,
}

impl CompressedNullifier {
    pub const DISCRIMINATOR: [u8; 8] = *b"solidnul";
    pub const fn size() -> usize {
        8 + 32 + 8
    }
}

/// A compressed identity state representing a user's self-sovereign identity.
///
/// Revocation happens by incrementing the revocation_nonce, which rotates
/// the on-chain identityState and invalidates previous ZK proofs.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CompressedIdentity {
    /// The holder's Solana public key (authority)
    pub owner: [u8; 32],
    /// Incremented by the owner to revoke all previous credentials
    pub revocation_nonce: u64,
}

impl CompressedIdentity {
    pub const DISCRIMINATOR: [u8; 8] = *b"solidid_";
    pub const fn size() -> usize {
        8 + 32 + 8
    }
}

/// A compressed issuer account representing a registered Trust Anchor.
///
/// In the Phase 2 "Neutral" design, all issuers stake the same amount,
/// but their 'tier' is stored as metadata for dApp-layer filtering.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CompressedIssuer {
    pub authority: [u8; 32],
    pub bjj_pub_key_x: [u8; 32],
    pub bjj_pub_key_y: [u8; 32],
    pub tier: u8,              // 0=Community, 1=Enterprise, 2=Regulated, 3=Government
    pub status: u8,            // 0=Pending, 1=Approved, 2=Revoked
    pub revocation_nonce: u64, // Used for issuer-level schema revocation (Phase 2.4)
}

impl CompressedIssuer {
    pub const DISCRIMINATOR: [u8; 8] = *b"solidiss";
    pub const fn size() -> usize {
        8 + 32 + 32 + 32 + 1 + 1 + 8
    }
}
