//! Schema types and hash computation.
//!
//! Schemas define the structure of attestation data (field names, types, constraints).
//! The schema hash is a Poseidon hash used as a public input to the circuit.

use serde::{Deserialize, Serialize};

use crate::error::{Result, SolidError};
use crate::poseidon;
use crate::query::MAX_FIELDS;

/// Field types supported by schemas.
///
/// All types are stored as `u64` in the circuit for simplicity.
/// The type information is metadata for the SDK, not enforced in-circuit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldType {
    /// Standard numeric value
    Uint64,
    /// Boolean (0 or 1)
    Boolean,
    /// Named enum variants (stored as u64 index)
    Enum(Vec<String>),
    /// Unix timestamp
    Timestamp,
}

/// A single field definition within a schema.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldDef {
    /// Field name (e.g., "age", "country_code")
    pub name: String,
    /// Field type
    pub field_type: FieldType,
    /// Whether this field supports range queries
    pub range_queryable: bool,
    /// Human-readable description
    pub description: String,
}

/// Schema category for the issuer registry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchemaCategory {
    Healthcare,
    Hospitality,
    SupplyChain,
    Finance,
    Education,
    Government,
    Custom(String),
}

/// A schema definition for attestation data.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SchemaDefinition {
    /// Unique schema name
    pub name: String,
    /// Schema version
    pub version: u8,
    /// Category
    pub category: SchemaCategory,
    /// Field definitions (up to MAX_FIELDS for circuit compatibility)
    pub fields: Vec<FieldDef>,
    /// Computed Poseidon hash of this schema (set after creation)
    pub schema_hash: Option<[u8; 32]>,
}

impl SchemaDefinition {
    /// Create a new schema definition.
    pub fn new(
        name: impl Into<String>,
        version: u8,
        category: SchemaCategory,
        fields: Vec<FieldDef>,
    ) -> Result<Self> {
        if fields.is_empty() || fields.len() > MAX_FIELDS {
            return Err(SolidError::InvalidInput(format!(
                "Schema must have 1-{} fields, got {}",
                MAX_FIELDS,
                fields.len()
            )));
        }

        let mut schema = Self {
            name: name.into(),
            version,
            category,
            fields,
            schema_hash: None,
        };

        // Compute and cache the schema hash
        let hash = schema.compute_hash()?;
        schema.schema_hash = Some(hash);
        Ok(schema)
    }

    /// Compute the Poseidon hash of this schema.
    ///
    /// The hash is computed from the schema name (as field elements) + version + field count.
    /// This must be deterministic for the same schema definition.
    pub fn compute_hash(&self) -> Result<[u8; 32]> {
        // Encode schema name as u64 values (first 8 bytes → u64, next 8 → u64, etc.)
        let name_bytes = self.name.as_bytes();
        let mut name_fields: Vec<u64> = Vec::new();
        for chunk in name_bytes.chunks(8) {
            let mut buf = [0u8; 8];
            buf[..chunk.len()].copy_from_slice(chunk);
            name_fields.push(u64::from_le_bytes(buf));
        }

        // Build hash inputs: name fields + version + field count
        let mut hash_inputs: Vec<u64> = name_fields;
        hash_inputs.push(self.version as u64);
        hash_inputs.push(self.fields.len() as u64);

        // Truncate to max Poseidon input size (16)
        if hash_inputs.len() > 16 {
            hash_inputs.truncate(16);
        }

        poseidon::hash_fields_to_bytes(&hash_inputs)
    }

    /// Get the cached schema hash, computing if needed.
    pub fn hash(&self) -> Result<[u8; 32]> {
        if let Some(h) = self.schema_hash {
            Ok(h)
        } else {
            self.compute_hash()
        }
    }
}

// ─── Pre-built Schema Constructors ─────────────────────────────────────────

/// Create the "basic_identity_v1" schema.
pub fn basic_identity_v1() -> Result<SchemaDefinition> {
    SchemaDefinition::new(
        "basic_identity_v1",
        1,
        SchemaCategory::Hospitality,
        vec![
            FieldDef { name: "age".into(), field_type: FieldType::Uint64, range_queryable: true, description: "Age in years".into() },
            FieldDef { name: "country_code".into(), field_type: FieldType::Uint64, range_queryable: false, description: "ISO 3166-1 numeric country code".into() },
            FieldDef { name: "resident_region".into(), field_type: FieldType::Uint64, range_queryable: false, description: "Region code".into() },
            FieldDef { name: "id_type".into(), field_type: FieldType::Enum(vec!["Passport".into(), "DL".into(), "NationalID".into()]), range_queryable: false, description: "Identity document type".into() },
            FieldDef { name: "verification_level".into(), field_type: FieldType::Uint64, range_queryable: true, description: "1=Self, 2=KYC, 3=InPerson".into() },
            FieldDef { name: "issued_date".into(), field_type: FieldType::Timestamp, range_queryable: true, description: "Issuance timestamp".into() },
            FieldDef { name: "nationality".into(), field_type: FieldType::Uint64, range_queryable: false, description: "Nationality code".into() },
            FieldDef { name: "_reserved".into(), field_type: FieldType::Uint64, range_queryable: false, description: "Reserved".into() },
        ],
    )
}

/// Create the "vaccination_status_v1" schema.
pub fn vaccination_v1() -> Result<SchemaDefinition> {
    SchemaDefinition::new(
        "vaccination_v1",
        1,
        SchemaCategory::Healthcare,
        vec![
            FieldDef { name: "vaccine_type".into(), field_type: FieldType::Enum(vec!["COVID".into(), "Flu".into(), "Measles".into()]), range_queryable: false, description: "Vaccine type".into() },
            FieldDef { name: "dose_number".into(), field_type: FieldType::Uint64, range_queryable: true, description: "Dose number".into() },
            FieldDef { name: "date_administered".into(), field_type: FieldType::Timestamp, range_queryable: true, description: "Administration date".into() },
            FieldDef { name: "issuer_authority".into(), field_type: FieldType::Uint64, range_queryable: false, description: "1=CDC, 2=WHO, 3=NHS".into() },
            FieldDef { name: "batch_number".into(), field_type: FieldType::Uint64, range_queryable: false, description: "Batch ID".into() },
            FieldDef { name: "expiry_date".into(), field_type: FieldType::Timestamp, range_queryable: true, description: "Expiry (0 = none)".into() },
            FieldDef { name: "country_code".into(), field_type: FieldType::Uint64, range_queryable: false, description: "ISO country code".into() },
            FieldDef { name: "recipient_age".into(), field_type: FieldType::Uint64, range_queryable: true, description: "Age at vaccination".into() },
        ],
    )
}

/// Create the "product_certification_v1" schema.
pub fn product_certification_v1() -> Result<SchemaDefinition> {
    SchemaDefinition::new(
        "product_cert_v1",
        1,
        SchemaCategory::SupplyChain,
        vec![
            FieldDef { name: "product_category".into(), field_type: FieldType::Uint64, range_queryable: false, description: "Product category".into() },
            FieldDef { name: "certification_level".into(), field_type: FieldType::Uint64, range_queryable: true, description: "1=Basic, 2=Standard, 3=Premium".into() },
            FieldDef { name: "audit_date".into(), field_type: FieldType::Timestamp, range_queryable: true, description: "Last audit date".into() },
            FieldDef { name: "auditor_id".into(), field_type: FieldType::Uint64, range_queryable: false, description: "Auditor identifier".into() },
            FieldDef { name: "compliance_score".into(), field_type: FieldType::Uint64, range_queryable: true, description: "0-100 score".into() },
            FieldDef { name: "region".into(), field_type: FieldType::Uint64, range_queryable: false, description: "Region code".into() },
            FieldDef { name: "organic_flag".into(), field_type: FieldType::Boolean, range_queryable: false, description: "Organic certification".into() },
            FieldDef { name: "valid_until".into(), field_type: FieldType::Timestamp, range_queryable: true, description: "Validity expiry".into() },
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_hash_deterministic() {
        let s1 = basic_identity_v1().unwrap();
        let s2 = basic_identity_v1().unwrap();
        assert_eq!(s1.hash().unwrap(), s2.hash().unwrap());
    }

    #[test]
    fn test_different_schemas_different_hashes() {
        let s1 = basic_identity_v1().unwrap();
        let s2 = vaccination_v1().unwrap();
        assert_ne!(s1.hash().unwrap(), s2.hash().unwrap());
    }

    #[test]
    fn test_schema_field_count_enforced() {
        let result = SchemaDefinition::new("empty", 1, SchemaCategory::Custom("test".into()), vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_prebuilt_schemas() {
        assert!(basic_identity_v1().is_ok());
        assert!(vaccination_v1().is_ok());
        assert!(product_certification_v1().is_ok());
    }
}
