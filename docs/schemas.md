# Schema Reference

> Pre-built credential schemas and how to create custom ones

## Pre-Built Schemas

### `basic_identity_v2`

**Category:** Identity

This is the current public-devnet smoke schema. It uses the same eight
numeric field slots as `basic_identity_v1`, but is registered with a
depth-20 credential tree so it matches the current batch circuit.

| Index | Field | Type | Range Queryable | Example Values |
|---|---|---|---|---|
| 0 | `age` | Uint64 | yes | 21, 35, 65 |
| 1 | `country_code` | Uint64 | no | 840 (US), 826 (UK), 356 (IN) |
| 2 | `region` | Uint64 | no | Region code |
| 3 | `id_type` | Uint64 | no | 0=Passport, 1=DL, 2=NationalID |
| 4 | `verification_level` | Uint64 | yes | 1=Self, 2=KYC, 3=InPerson |
| 5 | `issued_date` | Timestamp | yes | Unix timestamp |
| 6 | `nationality` | Uint64 | no | ISO 3166-1 numeric |
| 7 | `_reserved` | Uint64 | no | 0 |

Current devnet hash:
`6b5014bf611a025a4693b196a517ece9f2d0672672a2eb38d7a50481474e6823`.

---

### `basic_identity_v1`

**Category:** Hospitality

| Index | Field | Type | Range Queryable | Example Values |
|---|---|---|---|---|
| 0 | `age` | Uint64 | ✅ | 21, 35, 65 |
| 1 | `country_code` | Uint64 | ❌ | 840 (US), 826 (UK), 356 (IN) |
| 2 | `resident_region` | Uint64 | ❌ | Region code |
| 3 | `id_type` | Enum | ❌ | 0=Passport, 1=DL, 2=NationalID |
| 4 | `verification_level` | Uint64 | ✅ | 1=Self, 2=KYC, 3=InPerson |
| 5 | `issued_date` | Timestamp | ✅ | Unix timestamp |
| 6 | `nationality` | Uint64 | ❌ | ISO 3166-1 numeric |
| 7 | `_reserved` | Uint64 | ❌ | 0 |

**Example queries:**
- Age verification: `field[0] >= 21`
- US resident: `field[1] == 840`
- KYC verified: `field[4] >= 2`

---

### `vaccination_v1`

**Category:** Healthcare

| Index | Field | Type | Range Queryable | Example Values |
|---|---|---|---|---|
| 0 | `vaccine_type` | Enum | ❌ | 0=COVID, 1=Flu, 2=Measles |
| 1 | `dose_number` | Uint64 | ✅ | 1, 2, 3 |
| 2 | `date_administered` | Timestamp | ✅ | Unix timestamp |
| 3 | `issuer_authority` | Uint64 | ❌ | 1=CDC, 2=WHO, 3=NHS |
| 4 | `batch_number` | Uint64 | ❌ | Manufacturing batch |
| 5 | `expiry_date` | Timestamp | ✅ | Unix timestamp (0=none) |
| 6 | `country_code` | Uint64 | ❌ | ISO 3166-1 numeric |
| 7 | `recipient_age` | Uint64 | ✅ | Age at vaccination |

**Example queries:**
- COVID vaccinated: `field[0] == 0 AND field[1] >= 2`
- Recent vaccination: `field[2] >= 1704067200` (after 2024-01-01)

---

### `product_cert_v1`

**Category:** Supply Chain

| Index | Field | Type | Range Queryable | Example Values |
|---|---|---|---|---|
| 0 | `product_category` | Uint64 | ❌ | Product category code |
| 1 | `certification_level` | Uint64 | ✅ | 1=Basic, 2=Standard, 3=Premium |
| 2 | `audit_date` | Timestamp | ✅ | Last audit Unix timestamp |
| 3 | `auditor_id` | Uint64 | ❌ | Auditor identifier |
| 4 | `compliance_score` | Uint64 | ✅ | 0-100 |
| 5 | `region` | Uint64 | ❌ | Production region code |
| 6 | `organic_flag` | Boolean | ❌ | 0=No, 1=Yes |
| 7 | `valid_until` | Timestamp | ✅ | Certification expiry |

---

## Creating Custom Schemas

### 1. Define Schema (Rust)

```rust
use solid_core::schema::*;

let schema = SchemaDefinition::new(
    "my_custom_v1",
    1,
    SchemaCategory::Custom("MyVertical".into()),
    vec![
        FieldDef {
            name: "field_name".into(),
            field_type: FieldType::Uint64,
            range_queryable: true,
            description: "Description".into(),
        },
        // ... up to 8 fields
    ],
)?;

let hash = schema.hash()?; // Poseidon hash for circuit use
```

### 2. Register On-Chain

```typescript
await schemaProgram.methods.registerSchema(
    "my_custom_v1",
    1, // version
    "MyVertical",
    ["field_name", "field2", ...],
    schemaHash,
).accounts({
    schemaAccount: schemaPda,
    authority: wallet.publicKey,
}).rpc();
```

### 3. Use in Queries

```typescript
const query = new QueryBuilder()
    .schema(mySchemaHash)
    .where(0, 'GTE', 100n)
    .build();
```

## Constraints

- **Maximum 8 fields** per schema (circuit parameter `NUM_FIELDS`)
- **All values stored as u64** (circuit operates in BN254 field)
- **Enums** map to u64 indices (0, 1, 2, ...)
- **Timestamps** are Unix epoch seconds
- **Booleans** are 0 or 1
