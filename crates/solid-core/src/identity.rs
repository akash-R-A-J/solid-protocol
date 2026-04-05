use crate::babyjubjub::BJJPublicKey;
use crate::poseidon;
use crate::error::Result;

/// Phase 2.1: Identity State Commitment
/// Represents the unified state of a SolID identity anchored in the global tree.
pub struct IdentityState {
    pub public_key: BJJPublicKey,
    pub revocation_nonce: u64,
}

impl IdentityState {
    pub fn new(public_key: BJJPublicKey, revocation_nonce: u64) -> Self {
        Self {
            public_key,
            revocation_nonce,
        }
    }

    /// Compute the cryptographic commitment (leaf for Layer 2 Global Tree).
    /// commitment = Poseidon(pk_x, pk_y, revocation_nonce)
    pub fn commitment(&self) -> Result<[u8; 32]> {
        let x = self.public_key.x;
        let y = self.public_key.y;
        let nonce_bytes = self.revocation_nonce.to_le_bytes();
        let mut nonce_32 = [0u8; 32];
        nonce_32[0..8].copy_from_slice(&nonce_bytes);

        poseidon::hash_bytes(&[x, y, nonce_32])
    }
}
