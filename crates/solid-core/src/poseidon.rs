//! Circomlib-compatible Poseidon hash using `light-poseidon`.
//!
//! All outputs are byte-for-byte identical to circomlib's `poseidon.circom`.
//! This is critical — any divergence means proofs generated off-chain won't
//! verify in the on-chain Groth16 verifier.
//!
//! Internally operates on `ark_bn254::Fr` (BN254 scalar field), which is the
//! same field as `ark_ed_on_bn254::Fq` (BabyJubJub base field / point coordinates).

use ark_bn254::Fr;
use ark_ff::{BigInteger, PrimeField};
use light_poseidon::{Poseidon, PoseidonHasher};

use crate::error::{Result, SolidError};

// ─── Field Element ↔ Bytes Conversion ──────────────────────────────────────

/// Convert an `Fr` field element to 32 bytes (little-endian).
///
/// This matches circomlib's internal representation.
pub fn fr_to_bytes_le(fr: &Fr) -> [u8; 32] {
    let bigint = fr.into_bigint();
    let le_vec = bigint.to_bytes_le();
    let mut out = [0u8; 32];
    let len = le_vec.len().min(32);
    out[..len].copy_from_slice(&le_vec[..len]);
    out
}

/// Convert 32 bytes (little-endian) to an `Fr` field element.
///
/// Automatically reduces modulo the BN254 scalar field prime.
pub fn bytes_le_to_fr(bytes: &[u8; 32]) -> Fr {
    Fr::from_le_bytes_mod_order(bytes)
}

/// Convert a `u64` to an `Fr` field element.
pub fn u64_to_fr(val: u64) -> Fr {
    Fr::from(val)
}

// ─── Poseidon Hash Functions ───────────────────────────────────────────────

/// Hash an arbitrary number of `Fr` field elements using Poseidon.
///
/// Matches `circomlib/circuits/poseidon.circom` output exactly.
/// Supports 1–16 inputs (circomlib limitation).
pub fn hash_fr(inputs: &[Fr]) -> Result<Fr> {
    if inputs.is_empty() || inputs.len() > 12 {
        return Err(SolidError::PoseidonHash(format!(
            "Poseidon supports 1-12 inputs, got {}",
            inputs.len()
        )));
    }

    let mut hasher = Poseidon::<Fr>::new_circom(inputs.len())
        .map_err(|e| SolidError::PoseidonHash(format!("Hasher init: {}", e)))?;

    hasher
        .hash(inputs)
        .map_err(|e| SolidError::PoseidonHash(format!("Hash: {}", e)))
}

/// Hash `u64` values using Poseidon.
///
/// Convenience wrapper that converts u64 → Fr before hashing.
/// Used for hashing attestation data fields.
pub fn hash_fields(fields: &[u64]) -> Result<Fr> {
    let fr_inputs: Vec<Fr> = fields.iter().map(|&f| Fr::from(f)).collect();
    hash_fr(&fr_inputs)
}

/// Hash `u64` values and return the result as 32 bytes (little-endian).
pub fn hash_fields_to_bytes(fields: &[u64]) -> Result<[u8; 32]> {
    let result = hash_fields(fields)?;
    Ok(fr_to_bytes_le(&result))
}

/// Hash byte arrays (each 32 bytes, little-endian Fr representation) using Poseidon.
pub fn hash_bytes(inputs: &[[u8; 32]]) -> Result<[u8; 32]> {
    let fr_inputs: Vec<Fr> = inputs.iter().map(|b| bytes_le_to_fr(b)).collect();
    let result = hash_fr(&fr_inputs)?;
    Ok(fr_to_bytes_le(&result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_single_element() {
        // Poseidon(0) should produce a deterministic, non-zero output
        let result = hash_fields(&[0]).unwrap();
        assert_ne!(result, Fr::from(0));
    }

    #[test]
    fn test_hash_deterministic() {
        let a = hash_fields(&[1, 2, 3]).unwrap();
        let b = hash_fields(&[1, 2, 3]).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_hash_different_inputs_different_outputs() {
        let a = hash_fields(&[1, 2, 3]).unwrap();
        let b = hash_fields(&[1, 2, 4]).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn test_hash_order_matters() {
        let a = hash_fields(&[1, 2]).unwrap();
        let b = hash_fields(&[2, 1]).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn test_bytes_roundtrip() {
        let original = hash_fields(&[42, 99]).unwrap();
        let bytes = fr_to_bytes_le(&original);
        let recovered = bytes_le_to_fr(&bytes);
        assert_eq!(original, recovered);
    }

    #[test]
    fn test_hash_bytes_matches_hash_fields() {
        let fields = [10u64, 20u64, 30u64];
        let result_fields = hash_fields_to_bytes(&fields).unwrap();

        let fr_inputs: Vec<[u8; 32]> = fields
            .iter()
            .map(|&f| fr_to_bytes_le(&Fr::from(f)))
            .collect();
        let result_bytes = hash_bytes(&fr_inputs).unwrap();

        assert_eq!(result_fields, result_bytes);
    }

    #[test]
    fn test_reject_empty_input() {
        assert!(hash_fields(&[]).is_err());
    }

    #[test]
    fn test_reject_too_many_inputs() {
        let inputs: Vec<u64> = (0..13).collect();
        assert!(hash_fields(&inputs).is_err());
    }

    #[test]
    fn test_max_inputs() {
        let inputs: Vec<u64> = (0..12).collect();
        assert!(hash_fields(&inputs).is_ok());
    }
}
