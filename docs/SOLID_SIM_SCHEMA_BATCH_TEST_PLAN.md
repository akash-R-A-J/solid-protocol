# SolID Sim Schema And Batch Proof Test Plan

Run this after the base `solid-sim` E2E in
[`SOLID_SIM_UI_E2E_FLOW.md`](./SOLID_SIM_UI_E2E_FLOW.md) is green.

Goal:

1. Add three high-value schemas beyond `basic_identity_v2`.
2. Issue one credential for each schema to the same holder.
3. Test the current single-schema UI proof path for each schema.
4. Test the protocol batch feature: one Groth16 proof over up to four
   credential slots.
5. Test one fully custom schema through the UI proposal/registration flow.

## What "4 In One" Means

The current batch circuit supports one proof over up to four credential slots.
It is not four separate proofs bundled together.

Supported by the protocol/circuit:

| Limit | Current value |
| --- | --- |
| Credential slots per proof | `4` |
| Predicate slots per proof | `4` |
| Numeric fields per schema | `8` |
| Public inputs | `32` |
| Schema roots in public inputs | `merkleRoots[0..3]` |
| Schema hashes in public inputs | `schemaHashes[0..3]` |
| Padding | unused credential slots are zero-padded |

Important behavior:

- A one-credential proof uses slot `0` and pads slots `1..3`.
- A four-credential proof fills slots `0..3`.
- The active schema hashes must be ordered deterministically, with padding last.
- The proof still has one nullifier, one verifier nonce, one issuer-tree root,
  and one timestamp.
- If the verifier accepts the proof, that nullifier is consumed; replaying the
  same batch proof should fail.

Current `solid-sim` UI status:

- The normal `Verifier -> Query Builder` and `Wallet -> Wallet Vault` flow is
  currently single-schema/single-credential.
- The protocol, circuit, verifier program, and SDK shape support up to four
  credential slots.
- To test four credentials today, use an SDK/script path or extend `solid-sim`
  to let the verifier request multiple schema predicates and let the holder
  select multiple credentials.

## Test Order

Use this order:

1. Run the base UI E2E for `basic_identity_v2`.
2. Add and test the three schemas below one at a time.
3. Issue all four credentials to the same holder:
   - `basic_identity_v2`
   - `dao_membership_v1`
   - `accredited_investor_v1`
   - `product_cert_v1`
4. Test single-schema proofs for each new schema.
5. Test a four-slot batch proof with all four credentials.
6. Test one extra custom schema through the UI custom schema flow.

## Schema 1: DAO Membership

Use case:

- Token-gated or community-gated actions without revealing the exact wallet's
  full DAO history.
- Example verifier: "Can this holder access a DAO-only launchpad deal?"

Schema name:

```text
dao_membership_v1
```

Fields:

| Index | Field | Type | Example |
| --- | --- | --- | --- |
| 0 | `dao_id` | Uint64 | `1001` |
| 1 | `member_active` | Boolean | `1` |
| 2 | `membership_tier` | Uint64 | `2` |
| 3 | `joined_at` | Timestamp | `1704067200` |
| 4 | `voting_power_band` | Uint64 | `3` |
| 5 | `contribution_score` | Uint64 | `85` |
| 6 | `role_code` | Uint64 | `1` |
| 7 | `valid_until` | Timestamp | `1893456000` |

Good single-schema proof:

```text
member_active == 1
membership_tier >= 2
```

Expected verifier result:

- Local proof passes.
- On-chain proof accepts if the proof is fresh.
- Verifier can grant DAO-gated access.

## Schema 2: Accredited Investor

Use case:

- Private finance, gated investment access, or compliant DeFi pools.
- Example verifier: "Can this holder access a private credit pool?"

Schema name:

```text
accredited_investor_v1
```

Fields:

| Index | Field | Type | Example |
| --- | --- | --- | --- |
| 0 | `jurisdiction` | Uint64 | `840` |
| 1 | `accredited_status` | Boolean | `1` |
| 2 | `accreditation_level` | Uint64 | `2` |
| 3 | `income_band` | Uint64 | `3` |
| 4 | `net_worth_band` | Uint64 | `3` |
| 5 | `professional_status` | Uint64 | `1` |
| 6 | `verification_date` | Timestamp | `1735689600` |
| 7 | `valid_until` | Timestamp | `1893456000` |

Good single-schema proof:

```text
accredited_status == 1
accreditation_level >= 2
valid_until >= <current unix timestamp>
```

Expected verifier result:

- The verifier learns the holder is eligible.
- The verifier does not learn exact income or net worth.

## Schema 3: Product Certification

Use case:

- Supply-chain certificates, real-world asset attestations, product origin,
  organic status, or audit quality.
- Example verifier: "Can this product-backed asset enter a marketplace?"

Schema name:

```text
product_cert_v1
```

Fields:

| Index | Field | Type | Example |
| --- | --- | --- | --- |
| 0 | `product_category` | Uint64 | `12` |
| 1 | `certification_level` | Uint64 | `3` |
| 2 | `audit_date` | Timestamp | `1735689600` |
| 3 | `auditor_id` | Uint64 | `77` |
| 4 | `compliance_score` | Uint64 | `92` |
| 5 | `region` | Uint64 | `356` |
| 6 | `organic_flag` | Boolean | `1` |
| 7 | `valid_until` | Timestamp | `1893456000` |

Good single-schema proof:

```text
certification_level >= 2
compliance_score >= 80
valid_until >= <current unix timestamp>
```

Expected verifier result:

- Marketplace/verifier can accept the certification gate.
- The verifier does not need the full certificate document.

## Add A Schema Through The UI

Repeat this for each schema above if it is not already live in the manifest.

### 1. Issuer proposes schema

Connect:

```text
Issuer wallet
```

Open:

```text
Issuer -> Request Schema
```

Actions:

1. Fill `Propose custom schema`.
2. Use the schema name, version `1`, category, and the eight fields above.
3. Submit the proposal.

Expected:

- The proposal appears in the issuer's schema request area.
- The DAO/admin side can see the proposal.

### 2. DAO/admin registers schema and tree

Connect:

```text
DAO/admin wallet
```

Open:

```text
DAO -> Schema Permissions
```

Actions:

1. Find the custom schema proposal.
2. Click register.
3. Approve the schema registration transaction.
4. Approve the credential tree creation transaction.
5. Approve the tree-authority / binding transaction.

Expected:

- `System -> Schemas` shows the schema.
- The schema has a real schema hash.
- The schema has a depth-20 credential tree.
- The schema can be selected by issuer, wallet, and verifier pages.

### 3. Issuer requests permission

Connect:

```text
Issuer wallet
```

Open:

```text
Issuer -> Request Schema
```

Actions:

1. Select the newly registered schema.
2. Enter the permission reason.
3. Request DAO permission.

Expected:

- The permission request appears in `DAO -> Schema Permissions`.

### 4. DAO/admin grants issuer permission

Connect:

```text
DAO/admin wallet
```

Open:

```text
DAO -> Schema Permissions
```

Actions:

1. Find the issuer/schema permission request.
2. Approve it.
3. Approve the transaction.

Expected:

- The issuer can issue credentials for this schema.

## Single-Schema Proof Test For Each New Schema

For each schema:

1. Holder requests credential from `Wallet -> Request Credential`.
2. Issuer issues from `Issuer -> Request Inbox`.
3. Holder confirms the credential appears in `Wallet -> Wallet Vault`.
4. Verifier creates a proof request in `Verifier -> Query Builder`.
5. Holder approves in `Wallet -> Wallet Vault`.
6. Verifier loads proof in `Verifier -> Verify Proof`.
7. Submit on-chain while the proof is fresh.

Expected:

- Each schema works independently.
- Verifier shows `Proof accepted`.
- Reusing the same proof fails by nullifier replay protection.

## Four-Credential Batch Proof Test

This is the advanced test after the single-schema tests pass.

Target credential set:

| Slot | Schema | Example predicate |
| --- | --- | --- |
| 0 | `basic_identity_v2` | `age >= 18` |
| 1 | `dao_membership_v1` | `member_active == 1` |
| 2 | `accredited_investor_v1` | `accredited_status == 1` |
| 3 | `product_cert_v1` | `compliance_score >= 80` |

Expected proof shape:

- One Groth16 proof.
- Four active schema hashes.
- Four active Merkle roots.
- Four predicate slots.
- One verifier nonce.
- One timestamp.
- One nullifier.
- One on-chain verification result.

Expected public signal shape:

```text
publicSignals[0]      nullifierHash
publicSignals[1]      globalRoot
publicSignals[2..5]   merkleRoots for slots 0..3
publicSignals[6..9]   schemaHashes for slots 0..3
publicSignals[10]     issuerTreeRoot
publicSignals[11..18] query field indices
publicSignals[19..22] operators
publicSignals[23..26] values
publicSignals[27]     numPredicates
publicSignals[28]     compoundLogic
publicSignals[29]     verifierAddress
publicSignals[30]     verifierNonce
publicSignals[31]     currentTimestamp
```

How to test today:

1. Use the SDK/script path, not the current single-schema `solid-sim` UI path.
2. Build a `QueryBuilder` with four schema hashes.
3. Provide four stored credentials for the same holder seed.
4. Add one predicate per credential slot.
5. Generate one batch proof.
6. Submit it through the verifier proof-buffer path.

Expected result:

- Local `snarkjs` verification passes.
- On-chain `verify_batch_proof_v2` accepts the proof.
- Replay of the same proof fails.

If this fails:

| Symptom | Meaning | Fix |
| --- | --- | --- |
| Schema hash ordering error | Active schema slots are not sorted/padded the way the circuit expects | Sort active credentials deterministically and keep zero padding last |
| Leaf not indexed | At least one credential's schema/global/issuer leaf is missing from the indexer | Re-run issuance indexing and root sync before proving |
| Stale timestamp | The proof was generated too long before submission | Regenerate and submit promptly |
| Proof verifies locally but not on-chain | Public roots/signals do not match live bindings, or wrong account list was submitted | Compare `publicSignals[1..10]` with live binding roots and verifier account order |

## Custom Schema Test

After the three planned schemas work, create one additional schema that is not
predefined. Recommended test schema:

```text
event_access_v1
```

Use case:

- Conference, hackathon, IRL event, or gated community access.

Fields:

| Index | Field | Type | Example |
| --- | --- | --- | --- |
| 0 | `event_id` | Uint64 | `20260509` |
| 1 | `ticket_tier` | Uint64 | `2` |
| 2 | `kyc_level` | Uint64 | `2` |
| 3 | `checked_in` | Boolean | `1` |
| 4 | `issued_at` | Timestamp | `1778320000` |
| 5 | `valid_until` | Timestamp | `1893456000` |
| 6 | `region` | Uint64 | `356` |
| 7 | `reserved` | Uint64 | `0` |

Good proof:

```text
event_id == 20260509
ticket_tier >= 1
valid_until >= <current unix timestamp>
```

Expected:

- Custom schema proposal registers.
- Issuer receives permission.
- Credential issues and imports.
- Single-schema proof verifies.
- The schema can later become one slot in a four-credential batch proof.

## Pass Criteria

This whole follow-up test is green when:

1. Base `basic_identity_v2` E2E is green.
2. `dao_membership_v1` credential issues and verifies.
3. `accredited_investor_v1` credential issues and verifies.
4. `product_cert_v1` credential issues and verifies.
5. One custom schema issues and verifies.
6. One SDK/script-generated four-credential batch proof verifies locally.
7. The same four-credential proof verifies on-chain through
   `verify_batch_proof_v2`.
8. Replaying that same batch proof fails.
