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

### `dao_membership_v1`

**Category:** Community

| Index | Field | Type | Range Queryable | Example Values |
|---|---|---|---|---|
| 0 | `member_active` | Boolean | no | 0=No, 1=Yes |
| 1 | `dao_id` | Uint64 | no | 1001 |
| 2 | `membership_tier` | Uint64 | yes | 1=Member, 2=Contributor, 3=Core |
| 3 | `joined_at` | Timestamp | yes | Unix timestamp |
| 4 | `voting_power_band` | Uint64 | yes | Bucketed voting power |
| 5 | `contribution_score` | Uint64 | yes | 0-100 |
| 6 | `role_code` | Enum | no | Issuer-defined role code |
| 7 | `valid_until` | Timestamp | yes | Membership expiry |

**Example queries:**
- DAO member: `field[0] == 1`
- Contributor access: `field[2] >= 2`

---

### `accredited_investor_v1`

**Category:** Finance

| Index | Field | Type | Range Queryable | Example Values |
|---|---|---|---|---|
| 0 | `accredited_status` | Boolean | no | 0=No, 1=Yes |
| 1 | `jurisdiction` | Uint64 | no | 840 (US), 826 (UK), 356 (IN) |
| 2 | `accreditation_level` | Uint64 | yes | 1=Basic, 2=Reviewed, 3=Institutional |
| 3 | `income_band` | Enum | no | Issuer-defined band |
| 4 | `net_worth_band` | Enum | no | Issuer-defined band |
| 5 | `entity_type` | Enum | no | 1=Individual, 2=Entity, 3=Trust |
| 6 | `verification_date` | Timestamp | yes | Unix timestamp |
| 7 | `valid_until` | Timestamp | yes | Credential expiry |

**Example queries:**
- Private pool eligibility: `field[0] == 1 AND field[2] >= 2`
- Fresh accreditation: `field[7] >= <current unix timestamp>`

---

### `defi_eligibility_v1`

**Category:** DeFi

| Index | Field | Type | Range Queryable | Example Values |
|---|---|---|---|---|
| 0 | `eligible` | Boolean | no | 0=No, 1=Yes |
| 1 | `jurisdiction` | Uint64 | no | ISO 3166-1 numeric |
| 2 | `kyc_level` | Uint64 | yes | 1=Basic, 2=KYC, 3=Enhanced |
| 3 | `risk_tier` | Enum | no | Issuer-defined risk tier |
| 4 | `accredited_status` | Boolean | no | 0=No, 1=Yes |
| 5 | `sanctions_screened_at` | Timestamp | yes | Unix timestamp |
| 6 | `verification_date` | Timestamp | yes | Unix timestamp |
| 7 | `valid_until` | Timestamp | yes | Eligibility expiry |

**Example queries:**
- Compliant DeFi access: `field[0] == 1 AND field[2] >= 2`
- Still valid: `field[7] >= <current unix timestamp>`

---

### `proof_of_humanity_v1`

**Category:** Identity

| Index | Field | Type | Range Queryable | Example Values |
|---|---|---|---|---|
| 0 | `human_verified` | Boolean | no | 0=No, 1=Yes |
| 1 | `liveness_level` | Uint64 | yes | 1=Low, 2=Standard, 3=Strong |
| 2 | `uniqueness_level` | Uint64 | yes | 1=Device, 2=Social, 3=Biometric |
| 3 | `method_code` | Enum | no | Issuer-defined method |
| 4 | `country_code` | Uint64 | no | ISO 3166-1 numeric |
| 5 | `verification_date` | Timestamp | yes | Unix timestamp |
| 6 | `recheck_after` | Timestamp | yes | Recommended recheck time |
| 7 | `valid_until` | Timestamp | yes | Credential expiry |

**Example queries:**
- One-person access: `field[0] == 1 AND field[2] >= 2`
- Strong liveness: `field[1] >= 2`

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

## Making A Schema Proof-Ready

Adding a schema to `solid-sim` only makes it selectable in the simulator UI.
For a schema to behave like `basic_identity_v2` across the real devnet flow, it
must be launched through the protocol path:

1. Register the schema on-chain in `schema_registry`.
2. Create a depth-20 credential Merkle tree for that schema.
3. Initialize the schema tree binding PDA for the schema hash.
4. Grant issuer permission for the schema before issuing real credentials.
5. Publish the schema hash, tree, binding, and current root through the devnet
   manifest or the indexer's registered-schema store.
6. Run the indexer with a root-sync authority key so fresh issue events update
   the service-side binding roots before holders generate proofs.

Current local operator command for step 6:

```bash
cd /Users/rajakash/Desktop/testing/solid-protocol/indexer
SOLID_ROOT_SYNC_KEYPAIR_PATH=$HOME/.config/solana/solid-devnet-admin.json npm run start
```

Without these steps, a built-in schema is only a UI/catalog option: users can
see the shape of the credential, but the verifier path cannot produce a real
on-chain proof against that schema.

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
