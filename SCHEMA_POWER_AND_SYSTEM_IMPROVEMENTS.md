# Schema Power and System Improvement Plan

This document explains how SolID can make schemas more powerful, more
flexible, and more product-friendly while respecting the current protocol
constraints.

The short version:

- The current proof system is already useful for private eligibility.
- The current circuit is intentionally numeric: each credential has up to
  8 numeric fields, and one proof can check up to 4 predicates across up
  to 4 credentials.
- We can make schemas feel much richer at the product and SDK layer today
  by adding schema catalogs, field codecs, dictionaries, and better query
  builders.
- Truly arbitrary strings, set membership, nested objects, issuer-tier
  policies, and richer logic need circuit and/or registry upgrades.

---

## 1. Current Schema Model

The current schema model has three layers:

| Layer | What exists today | What it means |
| --- | --- | --- |
| On-chain schema registry | `name`, `version`, `category`, `field_names`, `schema_hash`, `authority`, `deprecated`, `usage_count` | Canonical schema identity and lifecycle. |
| SDK schema metadata | Field names, field indexes, type hints, descriptions, range-query hints | Product and developer semantics. |
| Circuit data | `data[NUM_CREDS][NUM_FIELDS]` numeric field values | The actual proof inputs. |

Current hard limits:

| Limit | Current value |
| --- | --- |
| Credentials per proof | `4` |
| Predicates per proof | `4` |
| Fields per credential | `8` |
| Compound logic | `AND` or `OR` |
| Operators | `NOOP`, `EQ`, `NE`, `GT`, `GTE`, `LT`, `LTE` |
| Field values | Numeric field elements, practically best treated as bounded integers |

Important reality: the circuit does not know what `country_code`,
`kyc_level`, `degree_type`, or `passport` means. It only knows that field
index `i` has numeric value `x`.

Schema power therefore comes from two things:

1. Defining strong semantic conventions around those numbers.
2. Expanding the circuit/registry when numeric conventions are no longer
   enough.

---

## 2. What We Can Do Today Without Circuit Changes

These improvements are available immediately with SDK, docs, scripts, and
product work only.

### 2.1 Add Rich Schema Descriptors

Create a canonical schema catalog format:

```json
{
  "name": "geo_eligibility_v1",
  "version": 1,
  "category": "Compliance",
  "hash": "<schema_hash_hex>",
  "fields": [
    {
      "index": 0,
      "name": "country_code",
      "type": "iso3166_numeric",
      "codec": "iso3166_numeric_v1",
      "rangeQueryable": false,
      "description": "ISO 3166-1 numeric country code"
    },
    {
      "index": 1,
      "name": "kyc_level",
      "type": "enum",
      "codec": "enum_v1",
      "values": {
        "none": 0,
        "basic": 1,
        "standard": 2,
        "enhanced": 3
      },
      "rangeQueryable": true
    }
  ]
}
```

This lets apps write:

```ts
solid.defineRequirement({
  schema: "geo_eligibility_v1",
  predicates: [
    { field: "country_code", op: "==", value: "IN" },
    { field: "kyc_level", op: ">=", value: "standard" }
  ],
  action: { appId: "launchpad", action: "join_pool" }
});
```

and the SDK compiles that into numeric circuit predicates:

```ts
country_code == 356
kyc_level >= 2
```

The product feels non-numeric, while the circuit remains numeric.

### 2.2 Add Field Codecs

A field codec is a reversible or deterministic mapping from product values
to circuit-safe numeric values.

Good codecs today:

| Product type | Encoding | Example |
| --- | --- | --- |
| Boolean | `false = 0`, `true = 1` | `sanctions_clear == true` -> `1` |
| Enum | integer code | `passport = 0`, `drivers_license = 1` |
| Country | ISO 3166-1 numeric | `IN = 356`, `US = 840` |
| Timestamp | Unix seconds | `2026-05-04` -> `1777852800` |
| Date only | Unix day number | `floor(timestamp / 86400)` |
| Decimal | scaled integer | `$12.34` -> `1234` with scale `2` |
| Percentage | basis points | `12.5%` -> `1250` |
| Tier | ordered enum | `basic = 1`, `pro = 2`, `enterprise = 3` |
| Region | registry-defined numeric code | `KA = 29`, `CA = 6` |
| Category | dictionary ID | `defi = 10`, `gaming = 20` |

This is the best immediate path. Most real compliance and eligibility
claims can be represented with careful numeric encodings.

### 2.3 Use Dictionaries for Non-Numeric Values

For strings like:

- country names
- credential types
- universities
- employers
- license types
- occupations
- jurisdictions
- asset categories

do not store strings directly in the circuit. Use dictionaries.

Example:

```json
{
  "codec": "dictionary_v1",
  "dictionaryHash": "sha256:<hash>",
  "values": {
    "Stanford University": 1001,
    "MIT": 1002,
    "IIT Bombay": 2001
  }
}
```

Then the proof checks:

```ts
institution_id == 2001
```

Advantages:

- Works with the current circuit.
- Supports equality and inequality.
- Supports ordered ranges if the dictionary is intentionally ordered.
- Keeps product UX readable.

Risk:

- Dictionary governance matters. A verifier and issuer must agree on the
  exact dictionary version.

### 2.4 Flatten Complex Objects

The current circuit does not support nested objects. Flatten them.

Instead of:

```json
{
  "address": {
    "country": "IN",
    "state": "KA"
  },
  "kyc": {
    "level": "standard",
    "provider": "AcmeKYC"
  }
}
```

use fields:

```text
country_code
region_code
kyc_level
provider_code
```

### 2.5 Use Multiple Credentials for Composability

Because one proof can include up to 4 credentials, schema power does not
have to come from one giant schema.

Example:

| Credential slot | Schema | Predicate |
| --- | --- | --- |
| 0 | `basic_identity_v1` | `age >= 21` |
| 1 | `geo_eligibility_v1` | `country_code != 840` |
| 2 | `dao_membership_v1` | `member_active == 1` |
| 3 | `reputation_v1` | `score >= 700` |

One proof can represent:

```text
age >= 21
AND country is allowed
AND DAO membership is active
AND reputation score is high enough
```

This is one of the most important current advantages of SolID.

### 2.6 Version Schemas Aggressively

Schemas should be immutable in meaning. If semantics change, register a
new version.

Good:

```text
kyc_basic_v1
kyc_basic_v2
```

Bad:

```text
kyc_basic_v1, but field 3 means something different now
```

The on-chain schema hash binds `name`, `version`, `field_names`, and
`category`, but product semantics like enum dictionaries and descriptions
need their own versioned metadata.

### 2.7 Add Schema Packs

Create curated schema packs:

```text
schemas/identity/basic_identity_v1.json
schemas/compliance/geo_eligibility_v1.json
schemas/finance/accredited_investor_v1.json
schemas/dao/dao_membership_v1.json
schemas/reputation/reputation_score_v1.json
schemas/education/education_degree_v1.json
schemas/health/vaccination_v1.json
```

Then publish them through the SDK:

```ts
import { schemaCatalog } from "@solid-protocol/schemas";

const solid = new SolidVerifier({
  cluster: "devnet",
  schemaCatalog
});
```

---

## 3. Making Schemas Feel Non-Numeric

The product should not expose field indexes and raw integers to normal
developers. The SDK should compile rich values into circuit values.

### 3.1 Better Requirement API

Target API:

```ts
const requirement = solid.defineRequirement({
  schema: "accredited_investor_v1",
  predicates: [
    { field: "accredited_status", op: "==", value: true },
    { field: "jurisdiction", op: "in", value: ["US", "IN", "SG"] },
    { field: "expires_at", op: ">=", value: new Date() }
  ],
  action: { appId: "launchpad", action: "join_pool_42" }
});
```

Compiled today into:

```text
accredited_status == 1
jurisdiction_code == <one numeric value>
expires_at >= <unix timestamp>
```

The `in` operator is not currently native. Today it can be represented
only in limited ways:

- Use `OR` if the whole query is just alternatives.
- Issue a derived boolean field such as `jurisdiction_allowed = 1`.
- Use a future set-membership circuit.

### 3.2 Field Indexes Should Be Internal

Current low-level query:

```ts
new QueryBuilder()
  .schemas(schemaHashes)
  .where(0, 0, "GTE", 21n)
```

Better product query:

```ts
solid.require("basic_identity_v1")
  .field("age").gte(21)
```

The SDK should resolve:

```text
field("age") -> index 0
gte -> GTE
21 -> 21n
```

### 3.3 Codecs Should Be Shared by Issuer, Holder, and Verifier

The same schema catalog must be used by:

- issuer when encoding credential data
- holder when displaying what will be proved
- verifier when compiling requirements
- console when rendering schemas
- wallet when showing proof requests

If each product invents its own mapping, proofs will become technically
valid but semantically dangerous.

---

## 4. What Needs Protocol or Circuit Improvement

Some flexibility cannot be done safely with metadata alone.

### 4.1 Arbitrary Strings

Current system does not natively prove arbitrary strings.

Bad idea:

```text
field = "India"
```

The circuit cannot compare strings.

Possible approaches:

1. Dictionary IDs: best today.
2. Hash strings into field values: possible for equality-style checks, but
   dangerous with current predicate comparators.
3. Add a dedicated string/hash-equality circuit path.

Important warning: the current predicate evaluator instantiates range
comparators even when the selected operator is `EQ`. Those comparators are
252-bit bounded. Raw Poseidon hashes are full BN254 field outputs and may
not fit that domain. So do not blindly put arbitrary Poseidon string hashes
into `data[i]` and expect equality proofs to always work.

Recommended improvement:

- Split equality predicates from range predicates in the circuit.
- Allow full-field equality for hashed values.
- Keep range checks only for bounded numeric values.

### 4.2 Set Membership

Useful product queries:

```text
country in [IN, SG, UAE]
country not in [US, KP, IR]
credential_type in [passport, national_id]
```

Current workaround:

- Use issuer-derived booleans like `is_allowed_jurisdiction`.
- Use global OR only for simple alternatives.

Better protocol feature:

- Add set-membership predicates.
- Public input includes a set root or dictionary root.
- Private input includes membership path.
- Circuit proves field belongs or does not belong to the set.

This would make compliance schemas much more powerful.

### 4.3 Nested AND/OR Logic

Current compound logic is one global mode:

```text
P1 AND P2 AND P3
```

or:

```text
P1 OR P2 OR P3
```

It does not support:

```text
(P1 AND P2) OR (P3 AND P4)
```

Possible improvement:

- Add expression-tree queries.
- Or add small fixed query shapes:
  - all
  - any
  - anyOfGroups
  - threshold-k-of-n

The most useful next form is probably:

```text
k-of-n predicates pass
```

Example:

```text
2 of 3 reputation providers say score >= 700
```

### 4.4 Issuer-Specific Requirements

Current proofs show that the issuer is approved in the issuer tree. They
do not cleanly expose a product-level policy like:

```text
accepted issuers = [Civic, Persona, AcmeKYC]
issuer tier >= Regulated
issuer jurisdiction == US
```

Potential improvements:

- Include issuer tier in the issuer-tree leaf.
- Add issuer category or issuer class to the leaf.
- Add public issuer policy inputs.
- Add verifier-side issuer allowlist support.
- Add SDK validation that `trustedIssuers` is actually enforced by the
  proof, not just included in a request fingerprint.

This matters a lot for real compliance use cases.

### 4.5 Rich Schema Metadata On-Chain

Today the on-chain schema stores names and field names. Rich types,
dictionaries, codec IDs, and descriptions live off-chain.

Possible upgrade:

```text
SchemaMetadata PDA:
  schema_hash
  metadata_uri
  metadata_sha256
  codec_registry_hash
  status
```

Keep big JSON off-chain, but pin it on-chain by hash.

This lets verifiers and wallets trust that they are using the same schema
semantics without bloating Solana accounts.

### 4.6 Schema Governance

Current schema registration is flexible. That is good for local/devnet,
but production needs more control.

Possible policy modes:

| Mode | Who can register | Best for |
| --- | --- | --- |
| Open | Anyone | experimentation |
| Curated | DAO approves public schemas | devnet/mainnet catalog |
| Namespace-owned | Issuer controls `issuer/schema_name` namespace | issuer-specific schemas |
| Verified schema pack | Foundation/team publishes canonical schemas | production integrations |

Suggested path:

1. Keep open registration for devnet.
2. Add curated schema catalog for official examples.
3. Add DAO-approved schema status for production.

---

## 5. Suggested Schema Roadmap

### Phase 0: Use Current System Better

No circuit changes.

Deliverables:

- `@solid-protocol/schemas` package.
- JSON schema descriptor format.
- Field codecs.
- Default schema catalog with 5-10 useful schemas.
- Schema registration CLI.
- QueryBuilder by field name.
- Issuer-side encoder.
- Holder-side proof request display.
- Verifier-side requirement compiler.

Example CLI:

```bash
solid schema register schemas/finance/accredited_investor_v1.json \
  --cluster devnet
```

Example issuer encoding:

```ts
const encoded = encodeCredential("accredited_investor_v1", {
  accredited_status: true,
  country: "IN",
  investor_type: "individual",
  verification_level: "standard",
  expires_at: "2027-05-04"
});
```

### Phase 1: Product-Grade Multi-Credential Requirements

No circuit changes, but SDK improvements.

Target:

```ts
solid.defineRequirement({
  all: [
    {
      schema: "basic_identity_v1",
      predicates: [{ field: "age", op: ">=", value: 21 }]
    },
    {
      schema: "dao_membership_v1",
      predicates: [{ field: "active", op: "==", value: true }]
    }
  ],
  action: { appId: "demo-dao", action: "vote" }
});
```

The SDK maps schemas to credential slots and compiles predicates to the
current batch circuit.

This is a big UX win because it exposes the true strength of the current
system: 4 credentials in one proof.

### Phase 2: Circuit Upgrade for Richer Types

Requires new circuit artifacts and trusted setup.

Best upgrades:

- Full-field equality support for hash-like values.
- 254-bit-safe predicate comparators or explicit operand range checks.
- Set membership predicates.
- k-of-n predicate logic.
- Better string/hash equality story.
- Optional issuer-tier or issuer-class constraint.
- Optional schema metadata hash public input.

### Phase 3: Schema Governance and Registry API

Protocol plus product.

Deliverables:

- Schema explorer.
- Schema proposal and approval flow.
- Schema metadata pinning.
- Official schema packs.
- Deprecated/frozen schema UX.
- Schema usage analytics.
- Indexer API:

```text
GET /v1/schemas
GET /v1/schemas/:hash
GET /v1/issuers/:issuer/schemas
GET /v1/schemas/:hash/tree
```

---

## 6. High-Value Schemas To Add First

### 6.1 KYC / Geo Eligibility

Fields:

```text
country_code
region_code
kyc_level
sanctions_clear
verified_at
expires_at
provider_code
reserved
```

Use cases:

- launchpad access
- DeFi compliance
- gated airdrops
- jurisdiction restrictions

### 6.2 Accredited Investor

Fields:

```text
accredited_status
country_code
investor_type
verification_level
verified_at
expires_at
provider_code
reserved
```

Use cases:

- private markets
- tokenized assets
- restricted offerings

### 6.3 DAO Membership

Fields:

```text
dao_id
role
membership_active
joined_at
voting_power_tier
delegation_status
expires_at
reserved
```

Use cases:

- private voting
- role-gated apps
- DAO access

### 6.4 Education

Fields:

```text
institution_id
degree_level
field_of_study_code
graduation_year
credential_status
issuer_code
expires_at
reserved
```

Use cases:

- grants
- hiring
- student perks

### 6.5 Reputation

Fields:

```text
score
score_bucket
source_code
epoch
activity_count
trust_level
expires_at
reserved
```

Use cases:

- sybil resistance
- reputation-gated communities
- undercollateralized products

### 6.6 Human / Uniqueness

Fields:

```text
human_verified
provider_code
uniqueness_level
verified_at
expires_at
region_code
reserved_a
reserved_b
```

Use cases:

- anti-sybil
- fair mints
- airdrop filtering

---

## 7. Overall System Improvements

### 7.1 Make the First Product Loop Real

The most important product loop:

```text
issuer registers
issuer gets approved
issuer issues credential
holder stores credential
verifier requests proof
holder approves proof
proof verifies on-chain
console shows the result
```

Every feature should be judged by whether it makes this loop easier.

### 7.2 Build a Real Holder Wallet Flow

Needed:

- encrypted credential storage
- schema-aware credential display
- proof request inbox
- human-readable disclosure screen
- proof generation worker
- response transport back to dApp
- proof history
- origin permissions

The wallet should show:

```text
App wants to verify:
  - age >= 21
  - country is not US
  - KYC level >= standard

Revealed:
  - only whether the statement is true

Not revealed:
  - exact age
  - name
  - ID number
  - address
```

### 7.3 Make the Console Real

Needed:

- live schema registry view
- issuer application queue
- issuer approval voting
- issuer-tree status
- schema-tree status
- credential issuance form
- proof verification tool that calls `verifyOnChainV2`
- devnet health checks
- event history from indexer

The console should stop pretending when data is demo. It should clearly
show `demo`, `localnet`, or `devnet`.

### 7.4 Make Verifier Integration Boring

Target:

```ts
const result = await solid.verifyRequirement({
  requirement: {
    schema: "kyc_geo_v1",
    predicates: [
      { field: "kyc_level", op: ">=", value: "standard" },
      { field: "country_code", op: "!=", value: "US" }
    ],
    action: "join_pool"
  },
  wallet,
  payer
});
```

The app developer should not think about:

- public input slots
- proof buffers
- Merkle trees
- schema-tree PDAs
- issuer-tree PDAs
- nullifier PDAs
- artifact hashes

### 7.5 Add a Schema Studio

A schema studio would let a developer:

1. Create a schema visually.
2. Choose field codecs.
3. Preview example credentials.
4. Preview proof requests.
5. Register schema on localnet/devnet.
6. Generate issuer and verifier code snippets.

This would make SolID feel like a product, not just a protocol.

### 7.6 Add Scenario Tests

Add scenario tests for:

- age gate
- geo gate
- KYC level
- accredited investor
- DAO membership
- expired credential
- revoked issuer
- replayed proof
- multi-credential proof
- OR proof
- custom schema proof

Each scenario should run locally and be visible in docs.

### 7.7 Improve Governance Before Mainnet

Needed:

- multi-party trusted setup
- multisig/timelock for DAO authority
- issuer-tree operator multisig
- schema governance policy
- official schema catalog signing
- stronger fraud/slash process
- external audit

---

## 8. Design Principles

### 8.1 Keep Circuits Small, Push Semantics Up

The circuit should not know every human concept.

Good:

```text
country_code == 356
```

Bad:

```text
country_name == "India"
```

The SDK should make the first one feel like the second one.

### 8.2 Make Encodings Explicit

Every schema field should say:

- field name
- field index
- field type
- codec
- allowed values
- whether range operators are allowed
- example values
- who controls the dictionary

### 8.3 Never Change Meaning In Place

If a field meaning changes, create a new schema version.

### 8.4 Avoid Private Data Leakage Through Bad Predicates

Even if raw data is not revealed, a predicate can leak information.

Example:

```text
age >= 18
age >= 21
age >= 25
age >= 30
```

Repeated queries can narrow the actual age. Product surfaces should:

- show users exactly what is being checked
- use coarse buckets when possible
- create sensible standard predicates
- consider rate limits or privacy budgets later

### 8.5 Make Official Schemas Boring

Official schemas should be conservative, stable, and easy to reason about.
Experimental schemas can exist, but official schemas should be boring in
the best way.

---

## 9. Recommended Next Steps

Highest leverage next tasks:

1. Create `@solid-protocol/schemas` with JSON descriptors and codecs.
2. Add schema-aware issuer encoding.
3. Add schema-aware verifier requirement compilation.
4. Add holder proof request display using schema descriptors.
5. Add local examples for 5 custom schemas.
6. Upgrade `QueryBuilder` with `.and()`, `.or()`, named fields, and
   multi-credential helpers.
7. Add schema registration CLI.
8. Add schema metadata hash pinning plan.
9. Plan next circuit upgrade around:
   - full-field equality
   - set membership
   - k-of-n logic
   - issuer tier constraints

The current system is already useful. The biggest unlock is making its
numeric proof core feel semantic, safe, and easy for normal developers.

