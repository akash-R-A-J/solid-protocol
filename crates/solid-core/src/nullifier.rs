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

/// Compute a nullifier hash.
///
/// Matches the Circom circuit's Step 8:
///   `nullifier = Poseidon(holderPrivateKey, schemaHash, verifierNonce)`
///
/// # Arguments
/// * `holder_private_key` — Holder's BJJ private key (32 bytes LE)
/// * `schema_hash` — Schema identifier (32 bytes LE)
/// * `verifier_nonce` — Verifier-scoped nonce for context binding (32 bytes LE)
pub fn compute_nullifier(
    holder_private_key: &[u8; 32],
    schema_hash: &[u8; 32],
    verifier_nonce: &[u8; 32],
) -> Result<[u8; 32]> {
    poseidon::hash_bytes(&[*holder_private_key, *schema_hash, *verifier_nonce])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::babyjubjub;

    #[test]
    fn test_nullifier_deterministic() {
        let kp = babyjubjub::generate_keypair().unwrap();
        let schema = [1u8; 32];
        let nonce = [2u8; 32];

        let n1 = compute_nullifier(&kp.private_key, &schema, &nonce).unwrap();
        let n2 = compute_nullifier(&kp.private_key, &schema, &nonce).unwrap();
        assert_eq!(n1, n2);
    }

    #[test]
    fn test_nullifier_different_nonce_different_output() {
        let kp = babyjubjub::generate_keypair().unwrap();
        let schema = [1u8; 32];

        let n1 = compute_nullifier(&kp.private_key, &schema, &[1u8; 32]).unwrap();
        let n2 = compute_nullifier(&kp.private_key, &schema, &[2u8; 32]).unwrap();
        assert_ne!(n1, n2, "Different nonces must produce different nullifiers (unlinkability)");
    }

    #[test]
    fn test_nullifier_different_holder_different_output() {
        let kp1 = babyjubjub::generate_keypair().unwrap();
        let kp2 = babyjubjub::generate_keypair().unwrap();
        let schema = [1u8; 32];
        let nonce = [2u8; 32];

        let n1 = compute_nullifier(&kp1.private_key, &schema, &nonce).unwrap();
        let n2 = compute_nullifier(&kp2.private_key, &schema, &nonce).unwrap();
        assert_ne!(n1, n2);
    }

    #[test]
    fn test_nullifier_different_schema_different_output() {
        let kp = babyjubjub::generate_keypair().unwrap();
        let nonce = [2u8; 32];

        let n1 = compute_nullifier(&kp.private_key, &[1u8; 32], &nonce).unwrap();
        let n2 = compute_nullifier(&kp.private_key, &[2u8; 32], &nonce).unwrap();
        assert_ne!(n1, n2);
    }
}
