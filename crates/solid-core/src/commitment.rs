//! Attestation commitment computation.
//!
//! Matches the Circom circuit's commitment computation exactly:
//!   dataHash = Poseidon(attestationData[0], ..., attestationData[N-1])
//!   commitment = Poseidon(dataHash, schemaHash, holderPubKeyX, holderPubKeyY, salt)
//!
//! This commitment becomes a leaf in the Light Protocol compressed Merkle tree.

use ark_bn254::Fr;

use crate::babyjubjub::BJJPublicKey;
use crate::error::{Result, SolidError};
use crate::poseidon;

/// Compute the attestation data hash.
///
/// `dataHash = Poseidon(fields[0], fields[1], ..., fields[N-1])`
///
/// This is Step 2a of the circuit. Supports up to 16 fields (Poseidon limit),
/// but the circuit is instantiated with 8 fields.
pub fn hash_attestation_data(fields: &[u64]) -> Result<[u8; 32]> {
    if fields.is_empty() || fields.len() > 16 {
        return Err(SolidError::Commitment(format!(
            "Attestation data must have 1-16 fields, got {}",
            fields.len()
        )));
    }
    poseidon::hash_fields_to_bytes(fields)
}

/// Compute the full attestation commitment.
///
/// `commitment = Poseidon(dataHash, schemaHash, holderPubKeyX, holderPubKeyY, salt)`
///
/// This is Step 2b of the circuit. The output is the Merkle tree leaf value.
///
/// # Arguments
/// * `data_fields` — Raw attestation field values (e.g., [age, country_code, ...])
/// * `schema_hash` — Poseidon hash of the schema PDA (32 bytes LE)
/// * `holder_pub_key` — Holder's BabyJubJub public key
/// * `salt` — Random salt for commitment uniqueness (32 bytes)
pub fn compute_attestation_commitment(
    data_fields: &[u64],
    schema_hash: &[u8; 32],
    holder_pub_key: &BJJPublicKey,
    salt: &[u8; 32],
) -> Result<[u8; 32]> {
    // Step 2a: Hash attestation data fields
    let data_hash_fr = poseidon::hash_fields(data_fields)?;

    // Step 2b: Combine with metadata
    let schema_fr = poseidon::bytes_le_to_fr(schema_hash);
    let holder_x_fr = poseidon::bytes_le_to_fr(&holder_pub_key.x);
    let holder_y_fr = poseidon::bytes_le_to_fr(&holder_pub_key.y);
    let salt_fr = poseidon::bytes_le_to_fr(salt);

    let commitment = poseidon::hash_fr(&[
        data_hash_fr,
        schema_fr,
        holder_x_fr,
        holder_y_fr,
        salt_fr,
    ])?;

    Ok(poseidon::fr_to_bytes_le(&commitment))
}

/// Compute the attestation commitment from a pre-computed data hash.
///
/// Use this when you already have the data hash (e.g., from a previous computation).
pub fn compute_commitment_from_data_hash(
    data_hash: &[u8; 32],
    schema_hash: &[u8; 32],
    holder_pub_key: &BJJPublicKey,
    salt: &[u8; 32],
) -> Result<[u8; 32]> {
    let inputs: Vec<Fr> = [data_hash, schema_hash, &holder_pub_key.x, &holder_pub_key.y, salt]
        .iter()
        .map(|b| poseidon::bytes_le_to_fr(b))
        .collect();

    let commitment = poseidon::hash_fr(&inputs)?;
    Ok(poseidon::fr_to_bytes_le(&commitment))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::babyjubjub;

    #[test]
    fn test_commitment_deterministic() {
        let kp = babyjubjub::generate_keypair().unwrap();
        let schema_hash = poseidon::hash_fields_to_bytes(&[1, 2, 3]).unwrap();
        let salt = [42u8; 32];
        let fields = [21u64, 840, 1, 0, 0, 0, 0, 0];

        let c1 = compute_attestation_commitment(&fields, &schema_hash, &kp.public_key, &salt).unwrap();
        let c2 = compute_attestation_commitment(&fields, &schema_hash, &kp.public_key, &salt).unwrap();
        assert_eq!(c1, c2);
    }

    #[test]
    fn test_commitment_changes_with_data() {
        let kp = babyjubjub::generate_keypair().unwrap();
        let schema_hash = poseidon::hash_fields_to_bytes(&[1]).unwrap();
        let salt = [0u8; 32];

        let c1 = compute_attestation_commitment(&[21], &schema_hash, &kp.public_key, &salt).unwrap();
        let c2 = compute_attestation_commitment(&[22], &schema_hash, &kp.public_key, &salt).unwrap();
        assert_ne!(c1, c2);
    }

    #[test]
    fn test_commitment_changes_with_salt() {
        let kp = babyjubjub::generate_keypair().unwrap();
        let schema_hash = poseidon::hash_fields_to_bytes(&[1]).unwrap();

        let c1 = compute_attestation_commitment(&[21], &schema_hash, &kp.public_key, &[0u8; 32]).unwrap();
        let c2 = compute_attestation_commitment(&[21], &schema_hash, &kp.public_key, &[1u8; 32]).unwrap();
        assert_ne!(c1, c2);
    }

    #[test]
    fn test_commitment_changes_with_holder() {
        let kp1 = babyjubjub::generate_keypair().unwrap();
        let kp2 = babyjubjub::generate_keypair().unwrap();
        let schema_hash = poseidon::hash_fields_to_bytes(&[1]).unwrap();
        let salt = [0u8; 32];

        let c1 = compute_attestation_commitment(&[21], &schema_hash, &kp1.public_key, &salt).unwrap();
        let c2 = compute_attestation_commitment(&[21], &schema_hash, &kp2.public_key, &salt).unwrap();
        assert_ne!(c1, c2);
    }

    #[test]
    fn test_data_hash_matches_commitment_input() {
        let kp = babyjubjub::generate_keypair().unwrap();
        let schema_hash = [1u8; 32];
        let salt = [2u8; 32];
        let fields = [10u64, 20, 30];

        let direct = compute_attestation_commitment(&fields, &schema_hash, &kp.public_key, &salt).unwrap();
        let data_hash = hash_attestation_data(&fields).unwrap();
        let via_hash = compute_commitment_from_data_hash(&data_hash, &schema_hash, &kp.public_key, &salt).unwrap();

        assert_eq!(direct, via_hash);
    }
}
