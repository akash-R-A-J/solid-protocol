//! Nullifier computation for anti-replay and unlinkability.
//!
//! `nullifier = Poseidon(holderPrivateKey, schemaHash, verifierNonce)`
//!
//! Properties:
//! - Same holder + same schema + same verifier nonce → same nullifier (detect double-use)
//! - Different verifier nonce → different nullifier (unlinkable across verifiers)
//! - Private key is never revealed — only the hash output is public

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
