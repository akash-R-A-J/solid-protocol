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

    pub fn new(
        commitment: [u8; 32],
        schema_hash: [u8; 32],
        issuer: [u8; 32],
    ) -> Self {
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
