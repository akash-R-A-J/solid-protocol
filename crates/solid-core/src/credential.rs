//! Credential types representing issued attestations.
//!
//! A credential is the full data bundle that a holder stores locally after
//! receiving an attestation from an issuer. It contains everything needed
//! to generate ZK proofs.

use serde::{Deserialize, Serialize};

use crate::babyjubjub::{BJJPublicKey, EdDSASignature};

/// A stored credential — everything the holder needs to generate proofs.
///
/// After issuance, the issuer sends this bundle (encrypted) to the holder.
/// The holder stores it locally and uses it as private input to the circuit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Credential {
    /// Schema this credential was issued against
    pub schema_hash: [u8; 32],

    /// The raw attestation data field values
    /// (e.g., [age=21, country_code=840, ...])
    pub attestation_data: Vec<u64>,

    /// Issuer's EdDSA-Poseidon signature over the attestation commitment
    pub issuer_signature: EdDSASignature,

    /// Issuer's BabyJubJub public key
    pub issuer_pub_key: BJJPublicKey,

    /// Holder's BabyJubJub public key (embedded in the commitment)
    pub holder_pub_key: BJJPublicKey,

    /// Random salt used in the commitment
    pub salt: [u8; 32],

    /// Attestation commitment hash (the Merkle tree leaf)
    pub commitment: [u8; 32],

    /// Index of this credential's leaf in the Light Protocol Merkle tree
    pub tree_leaf_index: Option<u64>,

    /// Expiration timestamp (0 = no expiry)
    pub expiration_timestamp: u64,

    /// Issuance timestamp
    pub issued_at: u64,
}

impl Credential {
    /// Verify the credential's internal consistency.
    ///
    /// Checks:
    /// 1. Commitment matches the attestation data + metadata
    /// 2. Issuer signature is valid over the commitment
    pub fn verify_integrity(&self) -> crate::error::Result<bool> {
        // Recompute commitment
        let recomputed = crate::commitment::compute_attestation_commitment(
            &self.attestation_data,
            &self.schema_hash,
            &self.holder_pub_key,
            &self.salt,
        )?;

        if recomputed != self.commitment {
            return Ok(false);
        }

        // Verify issuer signature over commitment
        crate::babyjubjub::verify(
            &self.issuer_pub_key,
            &self.commitment,
            &self.issuer_signature,
        )
    }

    /// Get a specific field value from attestation data.
    pub fn get_field(&self, index: usize) -> Option<u64> {
        self.attestation_data.get(index).copied()
    }

    /// Serialize to JSON for local storage.
    pub fn to_json(&self) -> crate::error::Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| crate::SolidError::Serialization(e.to_string()))
    }

    /// Deserialize from JSON.
    pub fn from_json(json: &str) -> crate::error::Result<Self> {
        serde_json::from_str(json).map_err(|e| crate::SolidError::Serialization(e.to_string()))
    }
}

/// Builder for constructing credentials during issuance.
pub struct CredentialBuilder {
    schema_hash: [u8; 32],
    attestation_data: Vec<u64>,
    holder_pub_key: BJJPublicKey,
    expiration_timestamp: u64,
}

impl CredentialBuilder {
    pub fn new(schema_hash: [u8; 32], holder_pub_key: BJJPublicKey) -> Self {
        Self {
            schema_hash,
            attestation_data: Vec::new(),
            holder_pub_key,
            expiration_timestamp: 0,
        }
    }

    pub fn data(mut self, fields: Vec<u64>) -> Self {
        self.attestation_data = fields;
        self
    }

    pub fn expiration(mut self, timestamp: u64) -> Self {
        self.expiration_timestamp = timestamp;
        self
    }

    /// Build the credential by computing the commitment and signing it.
    ///
    /// # Arguments
    /// * `issuer_private_key` — Issuer's BJJ private key for signing
    /// * `issuer_pub_key` — Issuer's BJJ public key
    pub fn build(
        self,
        issuer_private_key: &[u8; 32],
        issuer_pub_key: BJJPublicKey,
    ) -> crate::error::Result<Credential> {
        // Generate random salt
        let salt: [u8; 32] = rand::random();

        // Compute commitment
        let commitment = crate::commitment::compute_attestation_commitment(
            &self.attestation_data,
            &self.schema_hash,
            &self.holder_pub_key,
            &salt,
        )?;

        // Sign the commitment
        let issuer_signature = crate::babyjubjub::sign(issuer_private_key, &commitment)?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(Credential {
            schema_hash: self.schema_hash,
            attestation_data: self.attestation_data,
            issuer_signature,
            issuer_pub_key,
            holder_pub_key: self.holder_pub_key,
            salt,
            commitment,
            tree_leaf_index: None,
            expiration_timestamp: self.expiration_timestamp,
            issued_at: now,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::babyjubjub;

    #[test]
    fn test_credential_builder_and_verify() {
        let issuer_kp = babyjubjub::generate_keypair().unwrap();
        let holder_kp = babyjubjub::generate_keypair().unwrap();
        let schema_hash = crate::poseidon::hash_fields_to_bytes(&[1, 2, 3]).unwrap();

        let credential = CredentialBuilder::new(schema_hash, holder_kp.public_key.clone())
            .data(vec![21, 840, 1, 0, 0, 0, 0, 0])
            .expiration(0)
            .build(&issuer_kp.private_key, issuer_kp.public_key.clone())
            .unwrap();

        assert!(credential.verify_integrity().unwrap());
        assert_eq!(credential.get_field(0), Some(21));
        assert_eq!(credential.get_field(1), Some(840));
    }

    #[test]
    fn test_credential_json_roundtrip() {
        let issuer_kp = babyjubjub::generate_keypair().unwrap();
        let holder_kp = babyjubjub::generate_keypair().unwrap();
        let schema_hash = [0u8; 32];

        let credential = CredentialBuilder::new(schema_hash, holder_kp.public_key)
            .data(vec![42])
            .build(&issuer_kp.private_key, issuer_kp.public_key)
            .unwrap();

        let json = credential.to_json().unwrap();
        let recovered = Credential::from_json(&json).unwrap();
        assert_eq!(credential.commitment, recovered.commitment);
        assert!(recovered.verify_integrity().unwrap());
    }
}
