//! Nullifier computation for anti-replay and unlinkability.
//!
//! The authoritative preimage is documented in ADR-0006.  The formula
//! evolved as:
//!
//!   v0.x (3-input, historical):
//!     Poseidon(holderPrivateKey, schemaHash, verifierNonce)
//!
//!   Phase 3.5 (5-input; currently implemented in `compute_nullifier`):
//!     Poseidon(masterKey, revocationNonce, verifierAddress,
//!              queryContextHash, verifierNonce)
//!
//!   ADR-0014 / Phase 2 impl 3 (6-input; target):
//!     Poseidon(masterKey, revocationNonce, verifierAddress,
//!              queryContextHash, verifierNonce, issuerTreeRoot)
//!
//! The circuit (`batch_credential_query.circom`) and the on-chain
//! verifier are already on the 6-input form as of the ADR-0014
//! circuit rev.  This host-side helper migrates together with the
//! holder SDK + cross-language vectors in the Phase 2 impl 3 commit;
//! until then, `compute_nullifier` below is the 5-input Phase 3.5
//! form used for off-chain vector sanity-checks ONLY -- do NOT
//! treat it as a mirror of what the circuit currently emits.
//!
//! Closes SOLID-SEC-036 on the docstring axis.

use crate::error::Result;
use crate::poseidon;

/// Compute a hardened nullifier hash (Phase 3.5).
///
/// `nullifier = Poseidon(masterKey, revocationNonce, verifierAddress, queryHash, verifierNonce)`
///
/// # Arguments
/// * `master_key` — Holder's BJJ private key (32 bytes LE)
/// * `rev_nonce` — Revocation nonce (8 bytes)
/// * `verifier_addr` — Verifier program address (32 bytes LE)
/// * `query_hash` — Hash of the specific query predicates (32 bytes LE)
/// * `verifier_nonce` — Verifier-scoped nonce (32 bytes LE)
pub fn compute_nullifier(
    master_key: &[u8; 32],
    rev_nonce: u64,
    verifier_addr: &[u8; 32],
    query_hash: &[u8; 32],
    verifier_nonce: &[u8; 32],
) -> Result<[u8; 32]> {
    let rev_nonce_fr = crate::poseidon::u64_to_fr(rev_nonce);
    let rev_nonce_bytes = crate::poseidon::fr_to_bytes_le(&rev_nonce_fr);
    
    poseidon::hash_bytes(&[
        *master_key, 
        rev_nonce_bytes, 
        *verifier_addr, 
        *query_hash, 
        *verifier_nonce
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::babyjubjub;

    #[test]
    fn test_nullifier_deterministic() {
        let kp = babyjubjub::generate_keypair().unwrap();
        let verifier = [1u8; 32];
        let query = [2u8; 32];
        let nonce = [3u8; 32];

        let n1 = compute_nullifier(&kp.private_key, 0, &verifier, &query, &nonce).unwrap();
        let n2 = compute_nullifier(&kp.private_key, 0, &verifier, &query, &nonce).unwrap();
        assert_eq!(n1, n2);
    }

    #[test]
    fn test_nullifier_different_nonce_different_output() {
        let kp = babyjubjub::generate_keypair().unwrap();
        let verifier = [1u8; 32];
        let query = [2u8; 32];

        let n1 = compute_nullifier(&kp.private_key, 0, &verifier, &query, &[1u8; 32]).unwrap();
        let n2 = compute_nullifier(&kp.private_key, 0, &verifier, &query, &[2u8; 32]).unwrap();
        assert_ne!(n1, n2, "Different nonces must produce different nullifiers");
    }

    #[test]
    fn test_nullifier_different_holder_different_output() {
        let kp1 = babyjubjub::generate_keypair().unwrap();
        let kp2 = babyjubjub::generate_keypair().unwrap();
        let verifier = [1u8; 32];
        let query = [2u8; 32];
        let nonce = [3u8; 32];

        let n1 = compute_nullifier(&kp1.private_key, 0, &verifier, &query, &nonce).unwrap();
        let n2 = compute_nullifier(&kp2.private_key, 0, &verifier, &query, &nonce).unwrap();
        assert_ne!(n1, n2);
    }

    #[test]
    fn test_nullifier_rotation_affects_output() {
        let kp = babyjubjub::generate_keypair().unwrap();
        let verifier = [1u8; 32];
        let query = [2u8; 32];
        let nonce = [3u8; 32];

        let n1 = compute_nullifier(&kp.private_key, 0, &verifier, &query, &nonce).unwrap();
        let n2 = compute_nullifier(&kp.private_key, 1, &verifier, &query, &nonce).unwrap();
        assert_ne!(n1, n2, "Revocation nonce must rotate nullifier");
    }
}
