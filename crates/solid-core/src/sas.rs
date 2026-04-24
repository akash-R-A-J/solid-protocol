//! Solana Attestation Service (SAS) integration types.
//!
//! SAS is the data layer — SolID is the computation layer.
//! This module provides Rust types matching SAS on-chain structures
//! for CPI integration from SolID programs.

use serde::{Deserialize, Serialize};

/// SAS Schema — defines the structure of attestable data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SasSchema {
    /// Schema authority (deployer/maintainer)
    pub authority: [u8; 32],
    /// Human-readable name
    pub name: String,
    /// Schema field definitions (JSON Schema format)
    pub fields: String,
    /// Whether the schema is active
    pub active: bool,
}

/// SAS Attestation — an on-chain assertion about a subject.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SasAttestation {
    /// Schema this attestation follows
    pub schema_id: [u8; 32],
    /// Who created the attestation
    pub attester: [u8; 32],
    /// Subject of the attestation (holder wallet)
    pub subject: [u8; 32],
    /// Attestation data (ABI-encoded per schema)
    pub data: Vec<u8>,
    /// Creation timestamp
    pub created_at: i64,
    /// Expiration timestamp (0 = never)
    pub expires_at: i64,
    /// Whether attestation has been revoked
    pub revoked: bool,
}

/// Helper to build SAS attestation data for CPI calls.
#[derive(Debug)]
pub struct SasAttestationBuilder {
    schema_id: [u8; 32],
    attester: [u8; 32],
    subject: [u8; 32],
    data: Vec<u8>,
    expires_at: i64,
}

impl SasAttestationBuilder {
    pub fn new(schema_id: [u8; 32], attester: [u8; 32], subject: [u8; 32]) -> Self {
        Self {
            schema_id,
            attester,
            subject,
            data: Vec::new(),
            expires_at: 0,
        }
    }

    /// Set attestation data (raw bytes).
    pub fn data(mut self, data: Vec<u8>) -> Self {
        self.data = data;
        self
    }

    /// Set expiration (0 = never expires).
    pub fn expires_at(mut self, ts: i64) -> Self {
        self.expires_at = ts;
        self
    }

    /// Build the attestation instruction data for CPI.
    /// Format: [schema_id(32) | subject(32) | expires_at(8) | data_len(4) | data]
    pub fn build_instruction_data(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(76 + self.data.len());
        buf.extend_from_slice(&self.schema_id);
        buf.extend_from_slice(&self.subject);
        buf.extend_from_slice(&self.expires_at.to_le_bytes());
        buf.extend_from_slice(&(self.data.len() as u32).to_le_bytes());
        buf.extend_from_slice(&self.data);
        buf
    }
}

/// SAS Program ID on devnet/mainnet.
/// Update this when SAS finalizes their program deployment.
pub const SAS_PROGRAM_ID: &str = "SASPROGRAMID111111111111111111111111111111111";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attestation_builder() {
        let schema_id = [1u8; 32];
        let attester = [2u8; 32];
        let subject = [3u8; 32];

        let data = SasAttestationBuilder::new(schema_id, attester, subject)
            .data(vec![0xAA, 0xBB, 0xCC])
            .expires_at(1700000000)
            .build_instruction_data();

        // 32 (schema) + 32 (subject) + 8 (expires) + 4 (data_len) + 3 (data) = 79
        assert_eq!(data.len(), 79);
        assert_eq!(&data[0..32], &schema_id);
        assert_eq!(&data[32..64], &subject);
    }
}
