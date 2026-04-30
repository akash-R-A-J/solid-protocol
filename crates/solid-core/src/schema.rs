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
    /// The hash is computed from the schema name + version + field-names +
    /// category.  Must be deterministic for the same schema definition and
    /// agree byte-for-byte with `schema-registry::register_schema`'s
    /// on-chain check.  Both sides delegate to
    /// `compute_schema_hash_from_parts` so the preimage layout cannot
    /// drift.  See SOLID-SEC-002 (preimage-shared) and SOLID-SEC-063 / H5
    /// (preimage-widened to bind field semantics + category).
    pub fn compute_hash(&self) -> Result<[u8; 32]> {
        let field_names: Vec<String> = self.fields.iter().map(|f| f.name.clone()).collect();
        let cat = format!("{:?}", self.category);
        compute_schema_hash_from_parts(&self.name, self.version, &field_names, &cat)
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
/// SOLID-SEC-063 / H5 (2026-04-29): the pre-fix derivation only bound
/// `(name, version, field_count)`.  Two schemas differing only in
/// field semantics (different `field_names`) or `category` collided to
/// the same hash, so a verifier accepting a proof issued under one
/// schema would treat it as another.  Robust fix: widen the preimage
/// to include canonical `field_names` and `category` via a 5-input
/// final Poseidon over per-component digests.
///
/// Preimage layout (5 [u8;32] field elements fed to `hash_bytes`):
/// 1. `name_h = poseidon_compress_bytes(name.as_bytes())` --
///    Merkle-Damgard-style absorb so arbitrary-length names cannot
///    collide via truncation.
/// 2. `version_e` -- 1-byte version in byte 0, rest zero.
/// 3. `count_e`   -- field_count as u64-LE in bytes [0..8], rest zero.
/// 4. `fnames_h`  -- absorb of `(u32 LE count) || (u32 LE len || bytes)*`.
///                   Length-prefixing makes the encoding canonical
///                   (no two distinct field-name lists serialize to
///                   the same byte stream).
/// 5. `cat_h`     -- `poseidon_compress_bytes(category.as_bytes())`.
///
/// `poseidon_compress_bytes` (defined below) is a Merkle-Damgard
/// absorb over `solid_core::poseidon::hash_bytes` with a final
/// length-tag round, so an arbitrary-length input compresses to a
/// single 32-byte digest with no collision risk via padding.  All
/// primitives are dual-target (BPF + host) so the on-chain
/// `register_schema` and the off-chain `SchemaDefinition::compute_hash`
/// produce byte-identical output.
///
/// Load-bearing for SOLID-SEC-002 + SOLID-SEC-063.
pub fn compute_schema_hash_from_parts(
    name: &str,
    version: u8,
    field_names: &[String],
    category: &str,
) -> Result<[u8; 32]> {
    let name_h = poseidon_compress_bytes(name.as_bytes())?;

    // Canonical field-names byte stream: count-prefix + per-name
    // length-prefix + bytes.  No two distinct lists serialize the
    // same way (the length prefixes prevent boundary-shift attacks).
    let mut fb: Vec<u8> = Vec::new();
    fb.extend_from_slice(&(field_names.len() as u32).to_le_bytes());
    for n in field_names {
        let nb = n.as_bytes();
        fb.extend_from_slice(&(nb.len() as u32).to_le_bytes());
        fb.extend_from_slice(nb);
    }
    let fnames_h = poseidon_compress_bytes(&fb)?;

    let cat_h = poseidon_compress_bytes(category.as_bytes())?;

    let mut version_e = [0u8; 32];
    version_e[0] = version;

    let mut count_e = [0u8; 32];
    count_e[..8].copy_from_slice(&(field_names.len() as u64).to_le_bytes());

    poseidon::hash_bytes(&[name_h, version_e, count_e, fnames_h, cat_h])
}

/// Merkle-Damgard absorb of an arbitrary-length byte slice through
/// `poseidon::hash_bytes`.  Each round consumes 31 bytes (so they fit
/// in one Bn254 field element with byte 31 reserved for a chunk-length
/// marker, which makes the padding canonical and refuses padding-shift
/// collisions).  A final length-tag round binds the total input length
/// into the digest.
///
/// Output is byte-identical on BPF (sol_poseidon syscall) and host
/// (light-poseidon fallback) -- see `solid_core::poseidon` for the
/// dual-target guarantee.
fn poseidon_compress_bytes(data: &[u8]) -> Result<[u8; 32]> {
    let mut state = [0u8; 32];
    for chunk in data.chunks(31) {
        let mut elem = [0u8; 32];
        elem[..chunk.len()].copy_from_slice(chunk);
        // Byte 31 carries the chunk length, making each absorb round
        // canonical.  Two distinct inputs that align on 31-byte
        // boundaries cannot reach the same internal state.
        elem[31] = chunk.len() as u8;
        state = poseidon::hash_bytes(&[state, elem])?;
    }
    let mut len_tag = [0u8; 32];
    len_tag[..8].copy_from_slice(&(data.len() as u64).to_le_bytes());
    state = poseidon::hash_bytes(&[state, len_tag])?;
    Ok(state)
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

    /// SOLID-SEC-002 + SOLID-SEC-063 regression gate.
    ///
    /// `SchemaDefinition::compute_hash` and `compute_schema_hash_from_parts`
    /// are the off-chain and on-chain entry points for the same derivation.
    /// They MUST produce identical outputs for identical inputs.  Any
    /// divergence breaks `register_schema` and fails this test.
    #[test]
    fn test_compute_schema_hash_parts_matches_definition() {
        let s = basic_identity_v1().unwrap();
        let via_definition = s.compute_hash().unwrap();
        let field_names: Vec<String> = s.fields.iter().map(|f| f.name.clone()).collect();
        let via_parts = compute_schema_hash_from_parts(
            &s.name,
            s.version,
            &field_names,
            &format!("{:?}", s.category),
        )
        .unwrap();
        assert_eq!(via_definition, via_parts);

        let s2 = vaccination_v1().unwrap();
        let via_definition2 = s2.compute_hash().unwrap();
        let field_names2: Vec<String> = s2.fields.iter().map(|f| f.name.clone()).collect();
        let via_parts2 = compute_schema_hash_from_parts(
            &s2.name,
            s2.version,
            &field_names2,
            &format!("{:?}", s2.category),
        )
        .unwrap();
        assert_eq!(via_definition2, via_parts2);
    }

    /// SOLID-SEC-063 / H5: same name+version+count but different field
    /// names MUST produce different hashes.  Same name+version+fields
    /// but different category MUST produce different hashes.  Pre-fix
    /// these all collided.
    #[test]
    fn test_compute_schema_hash_binds_field_names_and_category() {
        let names_a = vec!["age".into(), "country".into()];
        let names_b = vec!["age".into(), "score".into()]; // same count, different second name
        let h_a = compute_schema_hash_from_parts("schema_v1", 1, &names_a, "Identity").unwrap();
        let h_b = compute_schema_hash_from_parts("schema_v1", 1, &names_b, "Identity").unwrap();
        assert_ne!(h_a, h_b, "field-name change must change schema_hash");

        let h_cat = compute_schema_hash_from_parts("schema_v1", 1, &names_a, "Healthcare").unwrap();
        assert_ne!(h_a, h_cat, "category change must change schema_hash");

        // Permutation of field-names must also change the hash (canonical
        // ordering is the issuer's responsibility; the hash binds the
        // exact ordering submitted).
        let names_perm = vec!["country".into(), "age".into()];
        let h_perm =
            compute_schema_hash_from_parts("schema_v1", 1, &names_perm, "Identity").unwrap();
        assert_ne!(h_a, h_perm, "field-name permutation must change schema_hash");
    }

    /// Determinism + sensitivity to single-field changes.
    #[test]
    fn test_compute_schema_hash_parts_deterministic_and_sensitive() {
        let names: Vec<String> = (0..8).map(|i| format!("f{}", i)).collect();
        let a = compute_schema_hash_from_parts("schema_v1", 1, &names, "Identity").unwrap();
        let b = compute_schema_hash_from_parts("schema_v1", 1, &names, "Identity").unwrap();
        assert_eq!(a, b);

        let c = compute_schema_hash_from_parts("schema_v1", 2, &names, "Identity").unwrap();
        assert_ne!(a, c);

        let names_short: Vec<String> = (0..7).map(|i| format!("f{}", i)).collect();
        let d = compute_schema_hash_from_parts("schema_v1", 1, &names_short, "Identity").unwrap();
        assert_ne!(a, d);

        let e = compute_schema_hash_from_parts("schema_v2", 1, &names, "Identity").unwrap();
        assert_ne!(a, e);
    }

    /// SOLID-SEC-063 / H5: padding-shift attack must be impossible.
    /// Two different (name, category) pairs that "look the same" after
    /// boundary alignment must hash differently.
    #[test]
    fn test_schema_hash_no_padding_shift_collision() {
        let names: Vec<String> = vec!["x".into()];
        // "schema_v1" (9 bytes) + cat="Identity" (8 bytes) vs
        // "schema_v" (8 bytes) + cat="1Identity" (9 bytes) -- byte
        // streams differ but a naive concat-then-hash would collide.
        let h_a = compute_schema_hash_from_parts("schema_v1", 1, &names, "Identity").unwrap();
        let h_b = compute_schema_hash_from_parts("schema_v", 1, &names, "1Identity").unwrap();
        assert_ne!(h_a, h_b);
    }
}
