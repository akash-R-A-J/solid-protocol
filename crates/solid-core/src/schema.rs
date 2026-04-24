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
    /// This must be deterministic for the same schema definition, and must agree
    /// byte-for-byte with `schema-registry::register_schema`'s on-chain check.
    /// Both sides therefore delegate to `compute_schema_hash_from_parts` below
    /// so the preimage layout cannot drift between off-chain SDK and on-chain
    /// program. See SOLID-SEC-002.
    pub fn compute_hash(&self) -> Result<[u8; 32]> {
        compute_schema_hash_from_parts(&self.name, self.version, self.fields.len())
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

/// Canonical schema-hash derivation shared between the off-chain SDK
/// (`SchemaDefinition::compute_hash`) and the on-chain registry handler
/// (`schema-registry::register_schema`).
///
/// Preimage layout (all values are `u64` Poseidon inputs):
/// 1. Schema name bytes chunked into 8-byte little-endian groups, each
///    interpreted as a `u64` via `u64::from_le_bytes`.
/// 2. `version` as `u64`.
/// 3. `field_count` as `u64`.
///
/// The vector is truncated to 16 inputs to stay within the Poseidon
/// width ceiling used by `light-poseidon` / `circomlib`.
///
/// The output is 32 bytes in little-endian field-element encoding. This
/// is the SAME encoding used by every other primitive in the protocol.
///
/// Load-bearing for SOLID-SEC-002: if this function diverges between
/// off-chain and on-chain implementations, `register_schema` will reject
/// every correctly-derived schema hash the SDK produces.
pub fn compute_schema_hash_from_parts(
    name: &str,
    version: u8,
    field_count: usize,
) -> Result<[u8; 32]> {
    let mut hash_inputs: Vec<u64> = Vec::new();
    for chunk in name.as_bytes().chunks(8) {
        let mut buf = [0u8; 8];
        buf[..chunk.len()].copy_from_slice(chunk);
        hash_inputs.push(u64::from_le_bytes(buf));
    }
    hash_inputs.push(version as u64);
    hash_inputs.push(field_count as u64);
    if hash_inputs.len() > 16 {
        hash_inputs.truncate(16);
    }
    poseidon::hash_fields_to_bytes(&hash_inputs)
}

// ─── Pre-built Schema Constructors ─────────────────────────────────────────

/// Create the "basic_identity_v1" schema.
pub fn basic_identity_v1() -> Result<SchemaDefinition> {
    SchemaDefinition::new(
        "basic_identity_v1",
        1,
        SchemaCategory::Hospitality,
        vec![
            FieldDef {
                name: "age".into(),
                field_type: FieldType::Uint64,
                range_queryable: true,
                description: "Age in years".into(),
            },
            FieldDef {
                name: "country_code".into(),
                field_type: FieldType::Uint64,
                range_queryable: false,
                description: "ISO 3166-1 numeric country code".into(),
            },
            FieldDef {
                name: "resident_region".into(),
                field_type: FieldType::Uint64,
                range_queryable: false,
                description: "Region code".into(),
            },
            FieldDef {
                name: "id_type".into(),
                field_type: FieldType::Enum(vec![
                    "Passport".into(),
                    "DL".into(),
                    "NationalID".into(),
                ]),
                range_queryable: false,
                description: "Identity document type".into(),
            },
            FieldDef {
                name: "verification_level".into(),
                field_type: FieldType::Uint64,
                range_queryable: true,
                description: "1=Self, 2=KYC, 3=InPerson".into(),
            },
            FieldDef {
                name: "issued_date".into(),
                field_type: FieldType::Timestamp,
                range_queryable: true,
                description: "Issuance timestamp".into(),
            },
            FieldDef {
                name: "nationality".into(),
                field_type: FieldType::Uint64,
                range_queryable: false,
                description: "Nationality code".into(),
            },
            FieldDef {
                name: "_reserved".into(),
                field_type: FieldType::Uint64,
                range_queryable: false,
                description: "Reserved".into(),
            },
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
            FieldDef {
                name: "vaccine_type".into(),
                field_type: FieldType::Enum(vec!["COVID".into(), "Flu".into(), "Measles".into()]),
                range_queryable: false,
                description: "Vaccine type".into(),
            },
            FieldDef {
                name: "dose_number".into(),
                field_type: FieldType::Uint64,
                range_queryable: true,
                description: "Dose number".into(),
            },
            FieldDef {
                name: "date_administered".into(),
                field_type: FieldType::Timestamp,
                range_queryable: true,
                description: "Administration date".into(),
            },
            FieldDef {
                name: "issuer_authority".into(),
                field_type: FieldType::Uint64,
                range_queryable: false,
                description: "1=CDC, 2=WHO, 3=NHS".into(),
            },
            FieldDef {
                name: "batch_number".into(),
                field_type: FieldType::Uint64,
                range_queryable: false,
                description: "Batch ID".into(),
            },
            FieldDef {
                name: "expiry_date".into(),
                field_type: FieldType::Timestamp,
                range_queryable: true,
                description: "Expiry (0 = none)".into(),
            },
            FieldDef {
                name: "country_code".into(),
                field_type: FieldType::Uint64,
                range_queryable: false,
                description: "ISO country code".into(),
            },
            FieldDef {
                name: "recipient_age".into(),
                field_type: FieldType::Uint64,
                range_queryable: true,
                description: "Age at vaccination".into(),
            },
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
            FieldDef {
                name: "product_category".into(),
                field_type: FieldType::Uint64,
                range_queryable: false,
                description: "Product category".into(),
            },
            FieldDef {
                name: "certification_level".into(),
                field_type: FieldType::Uint64,
                range_queryable: true,
                description: "1=Basic, 2=Standard, 3=Premium".into(),
            },
            FieldDef {
                name: "audit_date".into(),
                field_type: FieldType::Timestamp,
                range_queryable: true,
                description: "Last audit date".into(),
            },
            FieldDef {
                name: "auditor_id".into(),
                field_type: FieldType::Uint64,
                range_queryable: false,
                description: "Auditor identifier".into(),
            },
            FieldDef {
                name: "compliance_score".into(),
                field_type: FieldType::Uint64,
                range_queryable: true,
                description: "0-100 score".into(),
            },
            FieldDef {
                name: "region".into(),
                field_type: FieldType::Uint64,
                range_queryable: false,
                description: "Region code".into(),
            },
            FieldDef {
                name: "organic_flag".into(),
                field_type: FieldType::Boolean,
                range_queryable: false,
                description: "Organic certification".into(),
            },
            FieldDef {
                name: "valid_until".into(),
                field_type: FieldType::Timestamp,
                range_queryable: true,
                description: "Validity expiry".into(),
            },
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
        let result =
            SchemaDefinition::new("empty", 1, SchemaCategory::Custom("test".into()), vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_prebuilt_schemas() {
        assert!(basic_identity_v1().is_ok());
        assert!(vaccination_v1().is_ok());
        assert!(product_certification_v1().is_ok());
    }

    /// SOLID-SEC-002 regression gate.
    ///
    /// `SchemaDefinition::compute_hash` and `compute_schema_hash_from_parts`
    /// are the off-chain and on-chain entry points for the same derivation.
    /// They MUST produce identical outputs for identical (name, version,
    /// field_count). Any future divergence (e.g. someone "simplifies" one
    /// side) immediately breaks `register_schema` and fails this test.
    #[test]
    fn test_compute_schema_hash_parts_matches_definition() {
        let s = basic_identity_v1().unwrap();
        let via_definition = s.compute_hash().unwrap();
        let via_parts = compute_schema_hash_from_parts(&s.name, s.version, s.fields.len()).unwrap();
        assert_eq!(via_definition, via_parts);

        let s2 = vaccination_v1().unwrap();
        let via_definition2 = s2.compute_hash().unwrap();
        let via_parts2 =
            compute_schema_hash_from_parts(&s2.name, s2.version, s2.fields.len()).unwrap();
        assert_eq!(via_definition2, via_parts2);
    }

    /// SOLID-SEC-002 regression gate.
    ///
    /// Same inputs must produce the same bytes; a single flipped field must
    /// produce different bytes. Any failure here is a Poseidon breakage.
    #[test]
    fn test_compute_schema_hash_parts_deterministic_and_sensitive() {
        let a = compute_schema_hash_from_parts("basic_identity_v1", 1, 8).unwrap();
        let b = compute_schema_hash_from_parts("basic_identity_v1", 1, 8).unwrap();
        assert_eq!(a, b);

        // Version change -> different hash.
        let c = compute_schema_hash_from_parts("basic_identity_v1", 2, 8).unwrap();
        assert_ne!(a, c);

        // Field-count change -> different hash.
        let d = compute_schema_hash_from_parts("basic_identity_v1", 1, 7).unwrap();
        assert_ne!(a, d);

        // Name change -> different hash.
        let e = compute_schema_hash_from_parts("vaccination_v1", 1, 8).unwrap();
        assert_ne!(a, e);
    }
}
