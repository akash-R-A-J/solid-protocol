//! Nullifier computation for anti-replay and unlinkability.
//!
//! The authoritative preimage is ADR-0006.  History:
//!
//!   v0.x (3-input, pre-Phase-3; historical only):
//!     Poseidon(holderPrivateKey, schemaHash, verifierNonce)
//!
//!   Phase 3.5 (5-input; superseded 2026-04-24):
//!     Poseidon(masterKey, revocationNonce, verifierAddress,
//!              queryContextHash, verifierNonce)
//!
//!   ADR-0014 / Phase 2 (6-input; **current**, matches circuit + on-chain
//!   verifier):
//!     Poseidon(masterKey, revocationNonce, verifierAddress,
//!              queryContextHash, verifierNonce, issuerTreeRoot)
//!
//! The 6th input ties every proof to a specific issuer-tree epoch.
//! Revoking any issuer bumps their leaf's revocation_nonce -> the
//! issuer-tree root changes -> the nullifier universe shifts.  A
//! pre-revocation proof submitted afterwards (a) cannot collide
//! nullifier-wise with a post-revocation proof, and (b) fails the
//! in-circuit Merkle-membership check against the new root anyway.
//! Both directions of the epoch boundary closed; SOLID-SEC-008.
//!
//! SOLID-SEC-036 closed in the same commit as the docstring update.

use crate::error::Result;
use crate::poseidon;

/// Compute the hardened 6-input nullifier hash (ADR-0006 Phase 2
/// revision; matches `batch_credential_query.circom` STEP 5).
///
/// # Arguments
/// * `master_key`       -- holder's BJJ master private key (32 bytes LE)
/// * `rev_nonce`        -- holder's revocation nonce (u64)
/// * `verifier_addr`    -- verifier program address (32 bytes LE)
/// * `query_hash`       -- hash of the specific query predicates (32 bytes LE)
/// * `verifier_nonce`   -- verifier-scoped per-session nonce (32 bytes LE)
/// * `issuer_tree_root` -- singleton issuer-tree root at proof time
///                         (32 bytes LE); ADR-0014 / SOLID-SEC-008.
///
/// Every argument is load-bearing.  The order here MUST match the
/// `nullifier.inputs[0..6]` wiring in the batch circuit; a swap
/// silently makes proofs fail on-chain with no clear error signal.
pub fn compute_nullifier(
    master_key: &[u8; 32],
    rev_nonce: u64,
    verifier_addr: &[u8; 32],
    query_hash: &[u8; 32],
    verifier_nonce: &[u8; 32],
    issuer_tree_root: &[u8; 32],
) -> Result<[u8; 32]> {
    let rev_nonce_fr = crate::poseidon::u64_to_fr(rev_nonce);
    let rev_nonce_bytes = crate::poseidon::fr_to_bytes_le(&rev_nonce_fr);

    poseidon::hash_bytes(&[
        *master_key,
        rev_nonce_bytes,
        *verifier_addr,
        *query_hash,
        *verifier_nonce,
        *issuer_tree_root,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::babyjubjub;

    fn itr() -> [u8; 32] {
        [0x99u8; 32]
    }

    #[test]
    fn test_nullifier_deterministic() {
        let kp = babyjubjub::generate_keypair().unwrap();
        let verifier = [1u8; 32];
        let query = [2u8; 32];
        let nonce = [3u8; 32];
        let root = itr();

        let n1 = compute_nullifier(&kp.private_key, 0, &verifier, &query, &nonce, &root).unwrap();
        let n2 = compute_nullifier(&kp.private_key, 0, &verifier, &query, &nonce, &root).unwrap();
        assert_eq!(n1, n2);
    }

    #[test]
    fn test_nullifier_different_nonce_different_output() {
        let kp = babyjubjub::generate_keypair().unwrap();
        let verifier = [1u8; 32];
        let query = [2u8; 32];
        let root = itr();

        let n1 = compute_nullifier(&kp.private_key, 0, &verifier, &query, &[1u8; 32], &root).unwrap();
        let n2 = compute_nullifier(&kp.private_key, 0, &verifier, &query, &[2u8; 32], &root).unwrap();
        assert_ne!(n1, n2, "Different verifier nonces must produce different nullifiers");
    }

    #[test]
    fn test_nullifier_different_holder_different_output() {
        let kp1 = babyjubjub::generate_keypair().unwrap();
        let kp2 = babyjubjub::generate_keypair().unwrap();
        let verifier = [1u8; 32];
        let query = [2u8; 32];
        let nonce = [3u8; 32];
        let root = itr();

        let n1 = compute_nullifier(&kp1.private_key, 0, &verifier, &query, &nonce, &root).unwrap();
        let n2 = compute_nullifier(&kp2.private_key, 0, &verifier, &query, &nonce, &root).unwrap();
        assert_ne!(n1, n2);
    }

    #[test]
    fn test_nullifier_rotation_affects_output() {
        let kp = babyjubjub::generate_keypair().unwrap();
        let verifier = [1u8; 32];
        let query = [2u8; 32];
        let nonce = [3u8; 32];
        let root = itr();

        let n1 = compute_nullifier(&kp.private_key, 0, &verifier, &query, &nonce, &root).unwrap();
        let n2 = compute_nullifier(&kp.private_key, 1, &verifier, &query, &nonce, &root).unwrap();
        assert_ne!(n1, n2, "Revocation nonce must rotate nullifier");
    }

    /// SOLID-SEC-008 regression gate (host side).  Same holder, same
    /// verifier, same query, same nonce, same rev_nonce -- but the
    /// issuer-tree root differs by one byte.  The nullifier MUST
    /// differ; otherwise the epoch bind is a lie.
    #[test]
    fn test_nullifier_changes_with_issuer_tree_root() {
        let kp = babyjubjub::generate_keypair().unwrap();
        let verifier = [1u8; 32];
        let query = [2u8; 32];
        let nonce = [3u8; 32];

        let n1 = compute_nullifier(&kp.private_key, 0, &verifier, &query, &nonce, &[0x99u8; 32]).unwrap();
        let mut alt = [0x99u8; 32];
        alt[0] = 0x98; // single-bit-ish change
        let n2 = compute_nullifier(&kp.private_key, 0, &verifier, &query, &nonce, &alt).unwrap();
        assert_ne!(
            n1, n2,
            "Issuer-tree root must rotate the nullifier (SOLID-SEC-008 epoch bind)"
        );
    }
}
