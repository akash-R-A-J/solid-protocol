# SolID Protocol -- Security Registry

Canonical, living tracker for every security finding across every audit.
One file. No fragmentation. Nothing deleted.

- Protocol version under review: v0.6 (April 2026, post-Phase-2 close)
- Last audits folded in (chronological):
  - 2026-04-22 senior-engineer comprehensive audit
    (`sec/audits/2026-04-22_v0.3_comprehensive_audit.md`)
  - 2026-04-22 Antigravity deep system audit
    (`sec/audits/2026-04-22_v0.3_antigravity_deep_system_audit.md`)
  - 2026-04-22 master consolidated audit
    (`sec/audits/2026-04-22_v0.3_master_audit.md`)
  - 2026-04-22 v0.4 comprehensive audit + build plan
    (`sec/audits/2026-04-22_v0.4_comprehensive_audit_and_build_plan.md`)
  - 2026-04-23 v0.5 deep comprehensive audit
    (`sec/audits/2026-04-23_v0.5_deep_comprehensive_audit.md`)
  - 2026-04-24 v0.6 deep comprehensive audit
    (`sec/audits/2026-04-24_v0.6_deep_comprehensive_audit.md`)
- Last registry update: 2026-04-24 (Phase 3 doc sweep: introduces
  SOLID-SEC-043 and SOLID-SEC-044 from the v0.6 audit; refreshes
  summary counts)
- Next audit target: after Phase 3 close-out
  (see `plan/IMPLEMENTATION_PLAN.md` -- SEC-006, -007, -010, -041,
  -043, -044 close-out + integration suite 02..11)

See `sec/README.md` for workflow, severity definitions, and status lifecycle.

---

## Summary

| Severity  | Open | In Progress | Fixed | Verified | Won't Fix | Total |
|-----------|------|-------------|-------|----------|-----------|-------|
| CRITICAL  | 0    | 0           | 3     | 0        | 0         | 3     |
| HIGH      | 2    | 0           | 10    | 0        | 0         | 12    |
| MEDIUM    | 10   | 0           | 4     | 0        | 0         | 14    |
| LOW       | 5    | 0           | 4     | 0        | 0         | 9     |
| INFO      | 4    | 0           | 2     | 0        | 0         | 6     |
| **Total** | 21   | 0           | 23    | 0        | 0         | 44    |

---

## Status board

| ID              | Severity | Status | Title                                                              |
|-----------------|----------|--------|--------------------------------------------------------------------|
| SOLID-SEC-001   | CRITICAL | Fixed  | Batch circuit: `queryCredentialIndices` / `queryFieldIndices` unconstrained |
| SOLID-SEC-002   | CRITICAL | Fixed  | `register_schema` Poseidon integrity check commented out           |
| SOLID-SEC-003   | CRITICAL | Fixed  | `issue_credential` missing schema + tree pubkey binding            |
| SOLID-SEC-004   | HIGH     | Fixed  | No in-circuit issuer pubkey binding; revoked issuers still verify  |
| SOLID-SEC-005   | HIGH     | Fixed  | `currentTimestamp` public input not bound to `Clock`               |
| SOLID-SEC-006   | HIGH     | Fixed  | VK overwrite at chunk 0 has no freeze-gate; truncated VK finalizable (Part 1 on-chain; Part 2 circuit binding deferred) |
| SOLID-SEC-007   | HIGH     | Fixed  | BJJ public keys not subgroup-checked at registration               |
| SOLID-SEC-008   | HIGH     | Fixed  | Nullifier does not include epoch / global root                     |
| SOLID-SEC-009   | HIGH     | Fixed  | WASM bridge fractured across 3 locations                           |
| SOLID-SEC-010   | HIGH     | Open   | Cross-language test vectors cover only 2 of 10 primitives          |
| SOLID-SEC-011   | HIGH     | Fixed  | E2E scripts bugged: zkey name, missing import, missing issuer flow |
| SOLID-SEC-012   | HIGH     | Open   | Trusted setup is single-party with timestamp entropy               |
| SOLID-SEC-013   | MEDIUM   | Open   | `slash_issuer` and `submit_fraud_proof` are single-key             |
| SOLID-SEC-014   | MEDIUM   | Open   | `stake_vault` is a single shared PDA across all issuers            |
| SOLID-SEC-015   | MEDIUM   | Open   | `approve_via_trust_anchor` has no minimum-tier gate on target      |
| SOLID-SEC-016   | MEDIUM   | Open   | `transfer_authority` is single-step (no propose/accept)            |
| SOLID-SEC-017   | MEDIUM   | Open   | `solid-prover` uses `ark_std::test_rng()` -> breaks unlinkability  |
| SOLID-SEC-018   | MEDIUM   | Open   | `verifier_config` write lock on every verify caps throughput       |
| SOLID-SEC-019   | MEDIUM   | Open   | `set_binding_status` / `transfer_tree_binding_authority` missing schema-hash re-assertion |
| SOLID-SEC-020   | MEDIUM   | Fixed  | E2E scripts persist plaintext issuer + holder secrets              |
| SOLID-SEC-021   | MEDIUM   | Open   | Depth-20 circuit caps global tree at ~250K holders                 |
| SOLID-SEC-022   | LOW      | Open   | Local `IsZero` reimplementation in `credential_atom.circom` (also NEW-SEC-09 in master audit) |
| SOLID-SEC-023   | LOW      | Open   | `active_issuers` counter drifts on Cooldown -> Revoked path        |
| SOLID-SEC-024   | LOW      | Open   | `unstake_tokens` uses raw `-=` instead of `checked_sub`            |
| SOLID-SEC-025   | INFO     | Open   | `CheckIssuerStatus` ungated and never called on-chain              |
| SOLID-SEC-026   | INFO     | Open   | `Credential::verify_integrity` never called on-chain               |
| SOLID-SEC-027   | INFO     | Fixed  | `docs/IMPROVEMENTS_ROADMAP.md` has stale unticked checkboxes       |
| SOLID-SEC-028   | MEDIUM   | Fixed  | `CLAUDE.md` and test README document wrong WASM build path         |
| SOLID-SEC-029   | MEDIUM   | Fixed  | `IdentityAnchor` always has `enabled=1`; padding slots over-constrained |
| SOLID-SEC-030   | MEDIUM   | Fixed  | `transfer_slashed_lamports` can drain `stake_vault` to zero        |
| SOLID-SEC-031   | HIGH     | Fixed  | `bufToDecimal` LE interpretation of Solana pubkey risks breaking `verifierAddress` match |
| SOLID-SEC-032   | HIGH     | Fixed  | `SCHEMA_REGISTRY_ID_BYTES` hardcoded without build-time validation |
| SOLID-SEC-033   | HIGH     | Fixed  | Identity cohesion check compares master pubkey; circuit uses per-schema derived (E2E blocker) |
| SOLID-SEC-034   | MEDIUM   | Open   | `SubmitFraudProof` / `SlashIssuer` contexts missing PDA seed constraint on `issuer_account` |
| SOLID-SEC-035   | LOW      | Open   | `set_binding_status` can unfreeze without timelock                 |
| SOLID-SEC-036   | LOW      | Fixed  | `nullifier.rs` module docstring describes stale 3-arg formula (impl is correct 5-arg) |
| SOLID-SEC-037   | INFO     | Open   | `WithdrawAfterCooldown` missing explicit authority constraint (seeds provide partial protection) |
| SOLID-SEC-038   | INFO     | Open   | Master-audit informational cluster: `i16` borrow signedness, reader/writer size asymmetry, `GreaterThan(8)` bound comment |
| SOLID-SEC-039   | LOW      | Fixed  | `scripts/bootstrap_issuer.ts` test-only DAO parameters deployable to mainnet by mistake |
| SOLID-SEC-040   | LOW      | Fixed  | `scripts/check_program_ids.py` silently passed when `deployments/<cluster>.json` was absent |
| SOLID-SEC-041   | LOW      | Fixed  | `circuits/build/verification_key.json` uploaded by `initialize.ts` is not content-addressed |
| SOLID-SEC-042   | INFO     | Fixed  | `VerifierConfig::SPACE` doc drift (45/43 vs actual 49) across POST_REMEDIATION_AUDIT + MODULE_CONTRACTS |
| SOLID-SEC-043   | MEDIUM   | Open   | `IssuerTreeBinding.operator` is a single signer; no multisig or DAO gate on issuer-tree root rotation |
| SOLID-SEC-044   | LOW      | Open   | Cooldown status does not replace the issuer's tree leaf (proofs from Cooldown issuers still verify) |

---

## Findings -- detail

### SOLID-SEC-001 -- Batch circuit query indices unconstrained

- **Severity:** CRITICAL
- **Status:** Fixed (circuit; 2026-04-23)
- **Introduced:** 2026-04-22
- **Evidence (pre-fix):**
  `circuits/batch_credential_query.circom:56-59,216-217`;
  `circuits/lib/predicate_evaluator.circom:85-112`;
  `circuits/compound_query.circom` (analogous via `FieldSelector`).
- **Description.** `queryCredentialIndices[MAX_PREDICATES]` and
  `queryFieldIndices[MAX_PREDICATES]` were public inputs with no
  range constraints.  `BatchFieldSelector` returns 0 for any
  out-of-range index; a prover set `credIndex = 1000`,
  `queryValue = 0`, `operator = EQ` and satisfied `result = 1` for
  any predicate, which -- under OR compound logic -- trivially
  satisfied the whole query.
- **Impact.** Full soundness break on batch proofs.
- **Remediation (landed).**
  - `circuits/batch_credential_query.circom`: every
    `queryCredentialIndices[i]` is now constrained
    `< NUM_CREDS` and every `queryFieldIndices[i]` is constrained
    `< NUM_FIELDS` via `LessThan(8)` components (width is
    sufficient for both bounds).
  - `circuits/compound_query.circom`: same range check on
    `queryFieldIndices[i]` (single-credential; no credIndex array).
  - Because circuit changes invalidate the existing zkey, this
    fix is bundled with SOLID-SEC-029 (anchor enable gate) into
    ONE circuit revision and ONE TESTNET single-party
    trusted-setup run.  `circuits/scripts/setup.js` produces the
    new `batch_credential_query_final.zkey` (see SOLID-SEC-011 for
    the filename reconciliation); the PR body records the zkey
    sha256 that setup.js prints.
- **Regression gate (landed).** Mocha property test at
  `circuits/test/batch_range_checks.test.js` drives the isolated
  template at `circuits/test/templates/range_check_isolated.circom`;
  happy path + two boundary cases + 100 random out-of-range
  credential indices + 100 random out-of-range field indices, all
  in a single mocha `describe`.  CI job `circuit_witness_tests` in
  `.github/workflows/ci.yml` is the hard gate.

### SOLID-SEC-002 -- `register_schema` integrity check disabled

- **Severity:** CRITICAL
- **Status:** Fixed
- **Introduced:** 2026-04-22
- **Fixed:** 2026-04-23 (Phase 1 Tier 2)
- **Evidence:** `programs/schema-registry/src/lib.rs:112-125` (now calls
  `solid_core::schema::compute_schema_hash_from_parts` and enforces
  `InvalidSchemaHash`).
- **Remediation landed.** Introduced
  `solid_core::schema::compute_schema_hash_from_parts` as the shared
  preimage builder; both off-chain (`SchemaDefinition::compute_hash`)
  and on-chain (`register_schema`) call it so the derivation cannot
  drift. Added `PoseidonFailed` variant to schema-registry's
  `ErrorCode`.
- **Regression gate (in CI now).** `cargo test -p solid-core --lib`:
  - `schema::tests::test_compute_schema_hash_parts_matches_definition`
    asserts the helper and `SchemaDefinition::compute_hash` agree
    byte-for-byte for `basic_identity_v1` and `vaccination_v1`.
  - `schema::tests::test_compute_schema_hash_parts_deterministic_and_sensitive`
    verifies determinism plus sensitivity to version/field-count/name
    changes.
- **Follow-up (Phase 2).** Bankrun-level integration test
  `integration_04_schema_and_bindings` covers the end-to-end
  `register_schema` reject-on-mismatch path.

### SOLID-SEC-003 -- `issue_credential` missing schema + tree pubkey binding

- **Severity:** CRITICAL
- **Status:** Fixed (2026-04-23)
- **Introduced:** 2026-04-22
- **Evidence (pre-fix):**
  `programs/issuer-registry/src/lib.rs:619-716,909-937`
- **Description.** `IssueCredential` did not require `schema_hash` to
  match a registered `SchemaAccount`, did not bind `merkle_tree.key()`
  to `SchemaTreeBinding.tree_pubkey`, and did not prevent an approved
  issuer from appending to any SPL-AC tree whose authority derives
  from `PDA(b"tree-authority", any-32-bytes)`.
- **Impact.** Approved issuer spawned a rogue schema/tree universe
  the verifier accepted as canonical; combined with SOLID-SEC-002 (now
  Fixed), trivially exploitable.
- **Remediation (landed).**
  - `IssueCredential` context now requires two new accounts
    (`schema_account`, `schema_tree_binding`), both PDA-derived with
    `seeds::program = SCHEMA_REGISTRY_ID` so Anchor enforces
    schema-registry provenance at the account-validation layer.
  - `schema_account` is typed as
    `Account<'info, schema_registry::SchemaAccount>` -- Anchor owner-
    and discriminator-checks it for free.  The handler also requires
    `schema_account.schema_hash == schema_hash` and
    `!schema_account.deprecated`.
  - `schema_tree_binding` is a raw 145-byte PDA; the handler owner-
    checks it (`owner == SCHEMA_REGISTRY_ID`) and calls the new
    `solid_light::cpi_helpers::verify_schema_tree_binding_for_issue`,
    which asserts (a) discriminator `b"schmtree"`, (b) embedded
    `schema_hash` matches, (c) embedded `tree_pubkey` matches
    `merkle_tree.key()`, (d) status byte is `0` (active).
  - New error variants: `SchemaHashMismatch`, `SchemaDeprecated`,
    `InvalidSchemaTreeBindingOwner`, `InvalidSchemaTreeBinding`,
    `TreeBindingMismatch`, `SchemaTreeBindingFrozen`.
  - TS SDK: `buildIssueCredentialIx` now takes `schemaName`/
    `schemaVersion` and derives the two new PDAs via
    `deriveSchemaAccount` / `deriveSchemaTreeBinding`;
    `IssueOptions` gains the same two fields.  Callers on the old
    signature fail to compile, not silently at runtime.
- **Regression gate.** Host-side unit tests in
  `crates/solid-light/src/cpi_helpers.rs`
  (`schema_tree_binding_issue_gate_*`) cover the happy path plus
  each mismatch axis (schema, tree, status, discriminator, short
  data).  Integration tests
  `06_issue_credential_rejects_unregistered_schema` and
  `07_issue_credential_rejects_wrong_tree` are scheduled for Phase 2
  (bankrun suite).

### SOLID-SEC-004 -- No in-circuit issuer pubkey binding

- **Severity:** HIGH
- **Status:** Fixed (2026-04-24, Phase 2 close-out commit `2f56771`;
  details in `sec/audits/2026-04-24_v0.6_phase2_closeout.md`)
- **Introduced:** 2026-04-22
- **Evidence (pre-fix):**
  `circuits/batch_credential_query.circom:81-82`;
  `programs/zk-verifier/src/lib.rs:148-297`;
  `programs/issuer-registry/src/lib.rs:577-582`;
  `CLAUDE.md:107-115`
- **Description.** Circuit accepts `issuerPubKeyAx/Ay` as private
  inputs. Verifier has no cross-check. `check_issuer_status` exists but
  no CPI call.
- **Impact.** Revoked issuers' prior signatures verify. Unapproved
  keys can be smuggled by a malicious SDK.
- **Remediation.** Compressed issuer Merkle tree + in-circuit
  membership proof (preferred) or CPI into `check_issuer_status`
  (alternate). Bundle with SOLID-SEC-001 / SOLID-SEC-008 /
  SOLID-SEC-029 in one circuit rev + trusted setup.
- **Regression gate.** Circuit witness test + integration test
  `05_verify_rejects_unapproved_issuer`.

### SOLID-SEC-005 -- `currentTimestamp` not bound to `Clock`

- **Severity:** HIGH
- **Status:** Fixed
- **Introduced:** 2026-04-22
- **Fixed:** 2026-04-23 (Phase 1 Tier 2)
- **Evidence:** `programs/zk-verifier/src/lib.rs` (verify_batch_proof step 2b,
  VerifierConfig.timestamp_skew_seconds, `set_timestamp_skew`
  instruction, `DEFAULT_TIMESTAMP_SKEW_SECONDS = 600`,
  `MAX_TIMESTAMP_SKEW_SECONDS = 3600`).
- **Remediation landed.**
  - Added `timestamp_skew_seconds: u32` field to `VerifierConfig`;
    `VerifierConfig::SPACE` bumped from 45 to 49. CLAUDE.md invariant
    ("Any change to VerifierConfig requires bumping SPACE") satisfied.
  - `initialize` sets the default to 600 seconds.
  - New authority-only `set_timestamp_skew` instruction caps at 3600
    to prevent a governance-without-ADR increase to an effectively-
    unbounded window.
  - `verify_batch_proof` extracts the u64 timestamp from the low 8
    bytes of `public_inputs[CURRENT_TIMESTAMP_INPUT_INDEX]` (slot 30
    at the time of this fix; slot 31 post-ADR-0014) in LE field-element
    encoding, enforces the high 24 bytes are zero, and rejects with
    `StaleTimestamp` unless `lower <= claimed_ts <= upper`.
- **Regression gate (in CI now).** `cargo test -p zk-verifier --lib`
  continues to pass (11/11) including VK-parser and G1-negation
  tests, confirming no regression.
- **Follow-up (Phase 2).** Bankrun integration tests
  `integration_10_verify_expired_credential_rejected` and
  `integration_11_verify_future_timestamp_rejected` exercise the
  on-chain Clock comparison end-to-end.

### SOLID-SEC-006 -- VK chunk 0 overwrite, no freeze-gate

- **Severity:** HIGH
- **Status:** Fixed (2026-04-25, Phase 3 impl 2, Part 1 on-chain;
  ADR-0015.  Part 2 -- circuit-bound `vk_generation` public input --
  deferred to the next trusted-setup cycle where it batches with
  SEC-010 and any further constraint changes.)
- **Introduced:** 2026-04-22
- **Evidence (pre-fix):** `programs/zk-verifier/src/lib.rs:82-130`.
  `store_verification_key(chunk_index=0, ..., is_final_chunk=true)`
  unconditionally wrote `vk_storage.data`; a compromised authority
  could swap the live VK silently between any two
  `verify_batch_proof` calls.
- **Impact.** Universal forgery via live VK swap; silent acceptance
  of truncated VK.
- **Remediation (landed, Part 1).**  ADR-0015.  Extended
  `VerifierConfig` with `vk_finalized: bool`, `vk_generation: u16`,
  `rotate_request_ts: i64` (SPACE 49 -> 60).  Added four new
  instructions:
  - `finalize_verification_key` flips `vk_finalized = true`.
  - `store_verification_key` now refuses when `vk_finalized` is set.
  - `request_vk_rotation` stamps `Clock::unix_timestamp` into
    `rotate_request_ts` (a 48-hour timelock window).
  - `cancel_vk_rotation` zeros the request.
  - `rotate_verification_key` requires the timelock to have expired;
    clears `vk_initialized` + `vk_finalized`, resets
    `next_vk_chunk = 0`, bumps `vk_generation`, clears the pending
    request.
  `VK_ROTATION_TIMELOCK_SECONDS = 172_800` (48 hours).  Authority
  remains a single pubkey until SOLID-SEC-043 replaces it with a
  Squads 3-of-5 PDA, at which point the 48-hour window composes
  with the multisig quorum for the full DAO gate.
- **Deferred (Part 2).**  Bind `vk_generation` into the public-input
  contract and reject proofs whose declared generation != current
  `VerifierConfig.vk_generation`.  Circuit change + new trusted
  setup; paired with SEC-010's expansion at the next ceremony.
- **Regression gate (landed).**  Six new host unit tests in
  `programs/zk-verifier/src/lib.rs::tests`:
  `vk_rotation_not_expired_when_no_pending_request`,
  `vk_rotation_not_expired_inside_window`,
  `vk_rotation_expired_at_and_beyond_timelock`,
  `vk_rotation_handles_saturation_safely`,
  `verifier_config_space_matches_layout` (SEC-042-class
  layout guard),
  `vk_rotation_timelock_is_48_hours` (doc-as-test for the
  constant).  Suite 11 -> 17 green.  Integration coverage for
  the four new handlers lands in
  `tests/integration/12_vk_rotation.test.ts` under B5.

### SOLID-SEC-007 -- BJJ pubkeys not subgroup-checked

- **Severity:** HIGH
- **Status:** Fixed (2026-04-25, Phase 3 impl 1)
- **Introduced:** 2026-04-22
- **Evidence (pre-fix):** `crates/solid-core/src/babyjubjub.rs:111-114,125-133`.
  `pubkey_to_affine` checked on-curve + non-zero but not cofactor;
  `register_issuer` stored `bjj_pub_key_x/y` with no subgroup check.
- **Impact.** Small-order pubkey enables trivial signature forgery
  off-chain (circuit-level defenses may block in-circuit case).
- **Remediation (landed).** Three surfaces:
  - `crates/solid-core/src/babyjubjub.rs`: `pubkey_to_affine` and
    `verify()` both now use `EdwardsAffine::new_unchecked` and
    call `is_in_correct_subgroup_assuming_on_curve()` before
    accepting any BJJ point; two new public helpers
    (`is_in_prime_order_subgroup`, `require_in_prime_order_subgroup`)
    expose the check to callers.  New `SolidError::BJJNotInSubgroup`
    variant in `error.rs`.
  - `programs/issuer-registry/src/lib.rs`: `register_issuer` calls
    `require_in_prime_order_subgroup` on the supplied
    `bjj_pub_key_x/y` before touching any state; rejects with
    `ErrorCode::InvalidBJJPubKey`.
  - `wasm/src/lib.rs`: `isBjjInPrimeOrderSubgroup` exported for
    client-side early validation in the TS SDK.
- **Regression gate (landed).** Five unit tests in
  `crates/solid-core/src/babyjubjub.rs`:
  `test_subgroup_accepts_honest_keypair`,
  `test_subgroup_rejects_identity`,
  `test_subgroup_rejects_order_two_point` (uses the known
  `(0, -1)` 2-torsion point),
  `test_subgroup_rejects_off_curve_point`,
  `test_verify_rejects_small_order_r8` (defence-in-depth for
  signature R8 components).  Suite is 49/49 green at the closing
  commit.

### SOLID-SEC-008 -- Nullifier lacks epoch binding

- **Severity:** HIGH
- **Status:** Fixed (2026-04-24, Phase 2 commit `df33ffe`;
  details in `sec/audits/2026-04-24_v0.6_phase2_closeout.md`)
- **Introduced:** 2026-04-22
- **Evidence (pre-fix):**
  `circuits/batch_credential_query.circom:286-292` (5-input
  Poseidon without issuer-tree binding);
  `crates/solid-core/src/nullifier.rs:23-40` (mirrored 5-input
  formula).
- **Description (pre-fix).** Nullifier = `Poseidon(masterKey,
  revocationNonce, verifierAddress, queryContextHash,
  verifierNonce)`. No `globalRoot` or epoch counter -> revoking an
  issuer did not invalidate nullifiers of previously-issued proofs.
- **Impact.** Defence-in-depth gap against root regression; after
  ADR-0014 this was elevated to a load-bearing soundness concern
  because an attacker who compromised an issuer could replay
  pre-revocation proofs into post-revocation verifiers.
- **Remediation (landed).** Added `issuerTreeRoot` as the 6th
  Poseidon input (ADR-0006 revision / ADR-0014 STEP 5).  Bundled
  with the SEC-004 circuit rev, new trusted-setup artifact, and
  full Rust/WASM/TS propagation.
- **Regression gate (landed).** `test_nullifier_changes_with_issuer_
  tree_root` in `crates/solid-core/src/nullifier.rs` +
  `circuits/test/templates/padding_slot.test.js` ensuring distinct-
  root inputs produce distinct nullifiers.

### SOLID-SEC-009 -- WASM bridge fractured across 3 locations

- **Severity:** HIGH
- **Status:** Fixed (2026-04-23)
- **Introduced:** 2026-04-22
- **Evidence (pre-fix):** `crates/solid-core/Cargo.toml:10`;
  `wasm/src/lib.rs`;
  `.github/workflows/ci.yml:196-197,256-257,301-303`;
  `ts-sdk/packages/core/package.json`
- **Description.** `Cargo.toml` declared a `wasm` feature on
  `solid-core` with zero `#[wasm_bindgen]` exports; the real bridge
  lived in `wasm/`; CI built from `crates/solid-core` (empty `pkg`);
  SDK imported from `../../../wasm/pkg` which neither CI nor any
  script created. See SOLID-SEC-028 for the doc variant of the same
  defect.
- **Impact.** Silent drift between Rust primitives and TS SDK
  consumers; a consumer who ran `npm ci` would import an empty
  module and fail at runtime, not at build.
- **Remediation (landed).** One source of truth per artifact:
  - `wasm/` stays standalone; `crates/solid-core` is BPF-only, with
    the dead `wasm` feature + `wasm-bindgen`/`js-sys`/
    `serde-wasm-bindgen` deps deleted
    (`crates/solid-core/Cargo.toml`, Cargo.lock).
  - All three CI invocations (`wasm`, `sdk`,
    `cross_language_vectors`) now run
    `wasm-pack build wasm/ --target nodejs --out-dir
    ts-sdk/packages/core/wasm --release`
    (`.github/workflows/ci.yml`). The `wasm` job now hard-asserts
    the expected artifacts exist on disk.
  - `ts-sdk/packages/core/package.json` no longer depends on a
    non-existent `@solid-protocol/wasm` package; the import is a
    relative runtime path `../wasm/solid_wasm.js` from
    `dist/index.js` (`ts-sdk/packages/core/src/index.ts`).
  - Stale references in `docs/system_architecture.md`,
    `docs/MODULE_CONTRACTS.md` reconciled to reality.
- **Regression gate.** New CI job `wasm_bridge_smoke`
  (`.github/workflows/ci.yml`) builds the bridge, builds
  `@solid-protocol/core`, and runs `scripts/wasm_bridge_smoke.mjs`,
  which exercises both Poseidon code paths through the SDK's
  compiled entry point and asserts a non-zero 32-byte digest plus
  distinct digests for distinct inputs.

### SOLID-SEC-010 -- Cross-language vectors 2/10 primitives

- **Severity:** HIGH
- **Status:** Open
- **Introduced:** 2026-04-22
- **Evidence:** `crates/solid-core/examples/gen_vectors.rs:55,62`;
  `tests/vectors/check_vectors.ts`
- **Description.** Only `attestation_commitment` and `nullifier`
  covered. Missing vectors for Poseidon raw, BJJ sign/verify,
  `derive_credential_key`, `computeIdentityState`,
  `QueryBuilder._computeContextHash`.
- **Impact.** Silent divergence between Rust / WASM / TS / circuits.
- **Remediation.** Extend `gen_vectors.rs` to every primitive with a
  TS caller.
- **Regression gate.** `cross_language_vectors` CI fails on divergence.

### SOLID-SEC-011 -- E2E scripts bugged

- **Severity:** HIGH
- **Status:** Fixed (2026-04-23)
- **Introduced:** 2026-04-22
- **Evidence (pre-fix):** `circuits/scripts/setup.js` built
  `compound_query.r1cs` and wrote `circuit_final.zkey`, while
  `scripts/prove.ts:123` expected `batch_credential_query_final.zkey`
  (filename mismatch); `scripts/prove.ts:105` referenced
  `keccak256HashPair`, a symbol that was never imported;
  `scripts/issue.ts` lacked any issuer register/stake/vote/approve
  sequence, so a clean checkout could never call `issue_credential`
  (issuer stays Pending).
- **Impact.** Stranger could not run E2E from a clean checkout.
- **Remediation (landed).**
  - `circuits/package.json`: `npm run compile` now targets
    `batch_credential_query.circom`; the legacy compound circuit
    kept reachable as `compile:compound` for ad-hoc work.
  - `circuits/scripts/setup.js`: rewritten to build against
    `batch_credential_query.r1cs` and emit
    `batch_credential_query_final.zkey` (matches prove.ts); moved
    from wall-clock `Date.now()` entropy to
    `crypto.randomBytes(32)` (tightens the single-party setup's
    entropy for testnet use; multi-party tracked under
    SOLID-SEC-012); prints a sha256 of the final zkey for
    audit-trail logging.
  - `scripts/prove.ts`: `keccak256HashPair` swapped to
    `poseidonHashPair` (the identity-state tree uses Poseidon; the
    SPL-AC keccak root is opaque to SolID proofs -- comment added).
  - `scripts/bootstrap_issuer.ts` (new): full 9-step DAO flow --
    governance mint + initialize_registry + register_issuer +
    stake_tokens + flash-loan cool-off + vote_on_issuer +
    finalize_voting, with a final status assertion.  Re-run safe.
  - `scripts/issue.ts`: reads the approved issuer's BJJ + authority
    keypairs from the shared state file; passes `schemaName` +
    `schemaVersion` through the SDK so `issueCredential` derives the
    new `schema_account` / `schema_tree_binding` PDAs (required post
    SOLID-SEC-003).
  - `scripts/lib/e2e_state.ts` (new): all scripts now persist state
    under `$XDG_RUNTIME_DIR/solid-e2e/state.json` (or
    `$TMPDIR/solid-e2e-$uid/state.json` fallback) with mode 0700
    directories and 0600 files; never under the repo working tree.
    Folds in the SOLID-SEC-020 follow-up.
- **Regression gate (landed).** CI job `e2e_localnet` in
  `.github/workflows/ci.yml` spins up `solana-test-validator` with
  all three `.so` binaries preloaded, runs `anchor build`, compiles
  the circuit, runs the TESTNET setup ceremony, then executes
  `initialize.ts -> bootstrap_issuer.ts -> issue.ts -> prove.ts` in
  that order.  A failure at any step fails the job.  On green, a
  stranger CAN run the pipeline from a clean checkout.

### SOLID-SEC-012 -- Trusted setup single-party

- **Severity:** HIGH
- **Status:** Open
- **Introduced:** 2026-04-22
- **Evidence:** `circuits/scripts/setup.js:4,47`
  (`'solid-entropy-' + Date.now()`)
- **Impact.** Operator controls toxic waste -> universal forgery.
- **Remediation.** Multi-party ceremony, 10+ contributors,
  attestation chain published, zkey on IPFS + Arweave.
- **Regression gate.** `verify_ceremony.js` attestation validator.

### SOLID-SEC-013 -- Single-key slashing / fraud authority

- **Severity:** MEDIUM
- **Status:** Open
- **Evidence:** `programs/issuer-registry/src/lib.rs:409,475-478,894`
- **Remediation.** Squads 3-of-5 multisig + 48h timelock on VK
  rotation + 24h challenge on slashing.
- **Regression gate.** Slash from non-multisig key must fail.

### SOLID-SEC-014 -- Shared `stake_vault` PDA

- **Severity:** MEDIUM
- **Status:** Open
- **Evidence:** `programs/issuer-registry/src/lib.rs:750-752,791-793,
  862-863`
- **Remediation.** Per-issuer vaults seeded by issuer authority.
  Related: SOLID-SEC-030 (GC guard on the shared vault as a short-
  term fix until per-issuer vaults land).
- **Regression gate.** Two-issuer stake/withdraw independence test.

### SOLID-SEC-015 -- `approve_via_trust_anchor` no tier gate

- **Severity:** MEDIUM
- **Status:** Open
- **Evidence:** `programs/issuer-registry/src/lib.rs:533-574`
- **Remediation.** Require anchor tier strictly higher than target;
  per-anchor rate limit.
- **Regression gate.** Same-tier approval must fail.

### SOLID-SEC-016 -- Single-step `transfer_authority`

- **Severity:** MEDIUM
- **Status:** Open
- **Evidence:** `programs/zk-verifier/src/lib.rs:140-146`
- **Remediation.** Propose + accept two-step pattern.
- **Regression gate.** Accept-without-propose fails; wrong-key-accept
  fails.

### SOLID-SEC-017 -- `solid-prover` uses `test_rng`

- **Severity:** MEDIUM
- **Status:** Open
- **Evidence:** `tools/solid-prover/src/lib.rs:90`
- **Impact.** Privacy: identical witness produces identical proof
  bytes.
- **Remediation.** Use `rand::rngs::OsRng`.
- **Regression gate.** Two identical-witness proofs must differ
  byte-for-byte.

### SOLID-SEC-018 -- `verifier_config` write lock

- **Severity:** MEDIUM
- **Status:** Open
- **Evidence:** `programs/zk-verifier/src/lib.rs:495`
- **Impact.** ~100 verify/sec ceiling.
- **Remediation.** Drop `proof_count` (use events) OR shard into
  `verifier_shard_{0..N}` with modulo-hash.
- **Regression gate.** Load test: 300 concurrent verifies, no
  schedule-exclusion serialization.

### SOLID-SEC-019 -- Missing schema-hash re-assertion

- **Severity:** MEDIUM
- **Status:** Open
- **Evidence:** `programs/schema-registry/src/lib.rs:322-349,428-452`
- **Remediation.** Add `require!(&data[8..40] == schema_hash)` to
  `set_binding_status` and `transfer_tree_binding_authority`.
- **Regression gate.** Unit test per handler.

### SOLID-SEC-020 -- Plaintext E2E secrets

- **Severity:** MEDIUM
- **Status:** Fixed
- **Fixed:** 2026-04-23 (Phase 1 Tier 1)
- **Evidence:** `scripts/issue.ts:118-125`; `.gitignore`
- **Remediation.** `.gitignore` now excludes `scripts/e2e_state.json`,
  `scripts/e2e_state.*.json`, `scripts/.secrets/`, and `~/.solid-protocol/`.
  `scripts/hooks/pre-commit-no-secrets.sh` blocks staging of matching
  filenames or staged diffs that introduce BJJ-key-shaped JSON fields
  (`privateKey`, `secretKey`, `holderPrivateKey`, `issuerPrivateKey`,
  `masterPrivateKey`, `bjjPrivateKey`). Install with
  `ln -sf ../../scripts/hooks/pre-commit-no-secrets.sh .git/hooks/pre-commit`.
- **Regression gate.** Phase 1 `gitignore_e2e_state_json` (manual test:
  touch `scripts/e2e_state.json`; `git check-ignore -v` must return the
  matching rule).
- **Follow-up.** Scripts themselves (e.g. `scripts/issue.ts:118-125`) still
  write plaintext state; that defensive write path itself is a separate
  refactor tracked in the Phase 1 scripts cluster (SOLID-SEC-011 bootstrap)
  -- the hook + gitignore are the ratchet that prevents an accidental
  check-in regardless.

### SOLID-SEC-021 -- Depth-20 cap at ~250K holders

- **Severity:** MEDIUM
- **Status:** Open
- **Evidence:** `circuits/batch_credential_query.circom:35-48`;
  `ts-sdk/packages/light/src/index.ts:137`
- **Remediation.** Ship depth-24 circuit rev in next trusted setup
  (16x capacity). Add capacity monitor.
- **Regression gate.** Monitoring alert at 80% capacity.

### SOLID-SEC-022 -- Local `IsZero` reimplementation

- **Severity:** LOW
- **Status:** Open
- **Evidence:** `circuits/lib/credential_atom.circom:83-90`. Also
  flagged in master audit as NEW-SEC-09.
- **Remediation.** Delete local template; include
  `circomlib/comparators.circom` explicitly.
- **Regression gate.** R1CS-hash snapshot in CI.

### SOLID-SEC-023 -- `active_issuers` counter drifts

- **Severity:** LOW
- **Status:** Open
- **Evidence:** `programs/issuer-registry/src/lib.rs:344-347` vs `:448`
- **Remediation.** Centralize transitions in one helper.
- **Regression gate.** Property-based transition-table test.

### SOLID-SEC-024 -- Raw `-=` in `unstake_tokens`

- **Severity:** LOW
- **Status:** Open
- **Evidence:** `programs/issuer-registry/src/lib.rs:285`
- **Remediation.** `checked_add` / `checked_sub` on all u64 math.
  Clippy `arithmetic_side_effects`.
- **Regression gate.** Clippy CI.

### SOLID-SEC-025 -- Ungated `CheckIssuerStatus`

- **Severity:** INFO
- **Status:** Open
- **Evidence:** `programs/issuer-registry/src/lib.rs:577-582,883-886`
- **Remediation.** Wire up (addresses SOLID-SEC-004 partially) or
  delete.

### SOLID-SEC-026 -- `verify_integrity` never called

- **Severity:** INFO
- **Status:** Open
- **Evidence:** `crates/solid-core/src/credential.rs`
- **Remediation.** Docstring noting it's SDK-internal, not consensus.

### SOLID-SEC-027 -- Stale roadmap checkboxes

- **Severity:** INFO
- **Status:** Fixed
- **Fixed:** 2026-04-23 (Phase 1 Tier 1)
- **Evidence:** `docs/IMPROVEMENTS_ROADMAP.md`
- **Remediation.** Reconciliation pass on 2026-04-23: P0 items all
  marked `[x]`; P1/P2 items reflect actual code state with `[~]` for
  items whose scope is now split with a more specific SOLID-SEC-NNN;
  stale audit artifacts (`docs/SOLID_*.md`, `docs/e2e_*`,
  `docs/infra_roadmap.md`, `docs/solid_protocol_terminal_manifesto.md`)
  relocated under `docs/archive/` with a HISTORICAL banner in
  `docs/archive/README.md`. The roadmap header now explicitly defers
  to `sec/SECURITY_REGISTRY.md` as canonical.
- **Regression gate.** `scripts/check_docs.py` (Phase 1 follow-up work)
  will diff the registry status board against the roadmap checklist
  and fail CI on drift.

---

### Findings introduced by the 2026-04-22 v0.4 audit pass

### SOLID-SEC-028 -- CLAUDE.md and test README document wrong WASM build path

- **Severity:** MEDIUM
- **Status:** Fixed
- **Introduced:** 2026-04-22 (v0.4 audit)
- **Fixed:** 2026-04-23 (Phase 1 Tier 1)
- **Evidence:**
  - `CLAUDE.md:47-52` (command now `wasm-pack build wasm/ ...`)
  - `tests/integration/README.md:19-20` (same correction)
  - Real bridge: `wasm/src/lib.rs` (305 lines, exports present)
  - `crates/solid-core/src/` still has zero `#[wasm_bindgen]` exports
    (by design per ADR-0002; solid-core stays BPF-compatible)
- **Description.** The real WASM bridge at `wasm/src/lib.rs` is
  complete and production-quality. CLAUDE.md and the test README
  previously documented `wasm-pack build crates/solid-core` which
  produced an empty pkg with no error. Both files now document the
  correct `wasm-pack build wasm/ ...` command with an inline comment
  explaining the separation.
- **Remediation landed.** Updated both documentation sites to the
  correct `wasm/` path. Added a clarifying comment in CLAUDE.md
  referencing ADR-0002.
- **Regression gate (still needed).** `wasm_bridge_smoke` CI step
  lands with SOLID-SEC-009 (CI + SDK pinned path) in Phase 1 Tier 4;
  that gate closes the silent-drift surface completely.
- **Related.** SOLID-SEC-009 (code-layer fracture) is the code fix;
  this entry was the documentation fix.

### SOLID-SEC-029 -- `IdentityAnchor` always `enabled = 1`; padding slots over-constrained

- **Severity:** MEDIUM
- **Status:** Fixed (circuit; 2026-04-23)
- **Introduced:** 2026-04-22 (v0.4 audit; folds in master-audit BUG-NEW-02)
- **Evidence (pre-fix):** `circuits/lib/identity_anchor.circom:46-53`
- **Description.** `CredentialAtom` correctly guarded zero-schema
  padding slots with `enabled = 1 - isZero(schemaHash)`, but
  `IdentityAnchor`, instantiated once per credential slot at
  `batch_credential_query.circom:111-122`, always set
  `globalInclusion.enabled = 1`.  For a padding slot (schemaHash=0)
  the circuit still required a valid Merkle inclusion proof for the
  derived identity leaf, so the global tree would have needed
  zero-schema derived entries pre-loaded -- architecturally wrong.
- **Impact.** Batch circuit could not generate proofs for fewer
  than NUM_CREDS=4 active credentials. Any holder with 1, 2, or 3
  credentials could not prove cleanly.
- **Remediation (landed).**
  - `circuits/lib/identity_anchor.circom`: new `enabled` signal
    input (bit-constrained via `enabled * (enabled - 1) === 0`);
    `globalInclusion.enabled <== enabled` replaces the hardcoded 1.
  - `circuits/batch_credential_query.circom`: STEP-0 / STEP-0.5
    reordered so `isZero[i]` is in scope before anchors[i] is
    instantiated; `anchors[i].enabled <== 1 - isZero[i].out` --
    padding slots skip the Merkle check while the STEP-0 integrity
    constraints still force every per-credential signal to zero.
  - `circuits/compound_query.circom`: `anchor.enabled <== 1` (the
    circuit rejects zero schemas up front, so anchors are always
    active; the new input just keeps the template signature
    consistent).
  - Bundled into the same circuit revision + TESTNET trusted-setup
    run as SOLID-SEC-001.
- **Regression gate (landed).** Mocha test at
  `circuits/test/padding_slot.test.js` drives the isolated template
  at `circuits/test/templates/anchor_enabled_isolated.circom`:
  (1) `enabled=0`, `schemaHash=0`, garbage siblings -> witness
  succeeds (padding slot);
  (2) `enabled=1`, garbage `globalRoot` -> witness rejects (active
  slot must prove inclusion);
  (3) `enabled=2` -> rejected by the new bit-constraint on the
  enable flag.
  CI job `circuit_witness_tests` in `.github/workflows/ci.yml`.

### SOLID-SEC-030 -- `transfer_slashed_lamports` can drain `stake_vault` to zero

- **Severity:** MEDIUM
- **Status:** Fixed
- **Introduced:** 2026-04-22 (v0.4 audit; folds in master-audit BUG-NEW-03)
- **Fixed:** 2026-04-23 (Phase 1 Tier 2)
- **Evidence:** `programs/issuer-registry/src/lib.rs` -- the
  `transfer_slashed_lamports` helper now computes
  `remaining = from_balance - amount`, checks
  `remaining >= Rent::get()?.minimum_balance(from.data_len())`, and
  rejects with `ErrorCode::StakeVaultWouldGoBelow` otherwise. Rent
  floor is computed per-call from the actual account data length so
  the check works for any source account (not just the shared
  zero-data stake_vault).
- **Remediation landed.** Guard lives inside the helper so all call
  sites (slash_issuer, submit_fraud_proof) get the same defense. The
  receiver is not guarded because lamport balances can only grow on
  the receiving side. New `StakeVaultWouldGoBelow` variant added to
  `ErrorCode`.
- **Regression gate (in CI now).** `cargo check -p issuer-registry`
  passes; structural fix verified by the compile. Full end-to-end
  test `stake_vault_not_garbage_collected_after_full_slash` lands
  with Phase 2 integration work per `plan/IMPLEMENTATION_PLAN.md`.
- **Structural follow-up.** SOLID-SEC-014 (per-issuer vaults)
  eliminates the single-vault-GC category outright in Phase 2. The
  guard here is the defense-in-depth ratchet that prevents a bug in
  any future code path from accidentally GC'ing the shared PDA.

---

### Findings introduced by the 2026-04-22 master consolidated audit (renumbered after v0.4 collision)

### SOLID-SEC-031 -- `bufToDecimal` LE interpretation of Solana pubkey

- **Severity:** HIGH (confirmed: `bigintToBytes32` packs BE;
  the LE->BE mismatch reversed the pubkey bytes on every tx)
- **Status:** Fixed
- **Introduced:** 2026-04-22 (master-audit NEW-SEC-06; renumbered
  from 028)
- **Fixed:** 2026-04-23 (Phase 1 Tier 3)
- **Trace confirmed:** `bufToDecimal` at `ts-sdk/packages/holder/src/index.ts`
  walks `i = buf.length - 1 -> 0`, putting buf[0] at the LSB
  (little-endian). `bigintToBytes32` at the same file uses
  `n.toString(16)` which emits hex in MSB-first order, then writes
  byte[0] as MSB (big-endian). The two functions disagree on
  endianness, so
  `bigintToBytes32(BigInt(bufToDecimal(ID.to_bytes())))`
  produces the byte-reversed pubkey. `zk-verifier::verify_batch_proof`
  compares `public_inputs[28] == ID.to_bytes()` byte-for-byte (slot
  28 at the time of this fix; slot 29 post-ADR-0014), which would
  fail every verification. This directly explains why
  `deployments/devnet.json` has no recorded successful deploy.
- **Remediation landed.** Added `bufToDecimalBE` helper that walks
  `i = 0 -> buf.length` so a 32-byte pubkey interpreted BE round-trips
  through `bigintToBytes32` back to the identical byte sequence. The
  VERIFIER_ID_BYTES call sites in both `generateProof` and
  `generateBatchProof` now use `bufToDecimalBE`. All other call sites
  (merkle roots, schema hashes, BJJ scalars, Poseidon outputs) stay on
  the LE `bufToDecimal` path, which matches the snarkjs / circomlib
  convention for field-element serialization. Inline comments on both
  helpers spell out the contract.
- **Regression gate (in CI now).** Syntax + type-level check via `tsc`.
  `vector_verifier_id_roundtrip` (cross-language) formalizes the gate
  in Phase 2's SOLID-SEC-010 vector expansion.

### SOLID-SEC-032 -- `SCHEMA_REGISTRY_ID_BYTES` hardcoded without build-time check

- **Severity:** HIGH
- **Status:** Fixed
- **Introduced:** 2026-04-22 (master-audit NEW-SEC-02; renumbered
  from 029)
- **Fixed:** 2026-04-23 (Phase 1)
- **Evidence:** `crates/solid-light/src/cpi_helpers.rs`
  (id_bytes_tests module); `scripts/check_program_ids.py`
  (CPI_HELPERS_PATH check section).
- **Remediation landed.** Two-layer defense:
  1. Rust host-side unit tests in `cpi_helpers::id_bytes_tests`
     decode `SCHEMA_REGISTRY_PROGRAM_ID` via
     `Pubkey::from_str` and assert byte equality with
     `SCHEMA_REGISTRY_ID_BYTES` (and with the typed
     `SCHEMA_REGISTRY_ID` `Pubkey`). Any drift between the base58
     literal and the byte array fails `cargo test -p solid-light`.
  2. `scripts/check_program_ids.py` reads the literal out of
     cpi_helpers.rs via regex and validates it against the
     `schema_registry` entry in `Anchor.toml`. Any drift between
     the literal and Anchor.toml fails the CI gate. The Rust test
     closes the byte-array loop; the Python check closes the
     cross-file loop.
- **Regression gates (in CI now).**
  - `cargo test -p solid-light --lib` covers
    `schema_registry_id_bytes_matches_program_id_literal` and
    `schema_registry_id_typed_matches_program_id_literal`.
  - `python3 scripts/check_program_ids.py` now fails on any
    literal/Anchor.toml mismatch.

### SOLID-SEC-033 -- Identity cohesion catch-22 (E2E blocker)

- **Severity:** HIGH
- **Status:** Fixed
- **Introduced:** 2026-04-22 (master-audit BUG-NEW-01; renumbered
  from 030)
- **Fixed:** 2026-04-23 (Phase 1 Tier 3)
- **Evidence:** `ts-sdk/packages/holder/src/index.ts` -- the
  `generateBatchProof` cohesion loop now compares
  `cred.holderPubKeyX` against the per-schema derived pubkey produced
  by `deriveCredentialKey(masterPrivateKey, cred.schemaHash)`, not
  against `masterPublicKey.x`.
- **Remediation landed.** Derived keypairs are computed ONCE at
  step 2b and reused for both the cohesion check (step 3) and the
  identity-state anchor leaves (step 5). This guarantees the two
  paths consume the same derivation byte-for-byte. `masterPublicKey`
  is retained in the API signature for v0.2 caller compatibility and
  is explicitly `void`-discarded inside the function with a
  comment pointing readers at the correct downstream source of
  truth.
- **Regression gate (in CI now).** Syntax + type-level check via `tsc`.
  Integration test `cohesion_check_passes_for_derived_key` lands with
  the broader bankrun harness in Phase 2.
- **Follow-up.** Consider removing the unused `masterPublicKey`
  parameter from the public API in a future major version bump (v1.0)
  together with an ADR, rather than a drive-by removal that would
  regress downstream callers.

### SOLID-SEC-034 -- Missing PDA seed constraint on `issuer_account`

- **Severity:** MEDIUM
- **Status:** Open
- **Introduced:** 2026-04-22 (master-audit NEW-SEC-03; renumbered
  from 033)
- **Evidence:** `programs/issuer-registry/src/lib.rs:799-823,861`
- **Description.** `SubmitFraudProof` and `SlashIssuer` contexts
  declare `#[account(mut)] pub issuer_account: Account<'info,
  IssuerAccount>` with no seed constraint. PDA seed derivation is
  the primary guard elsewhere (`approve_via_trust_anchor`,
  `withdraw_after_cooldown`).
- **Remediation.** Add identical seed constraint.
- **Regression gate.** Unit test: non-canonical PDA fails.

### SOLID-SEC-035 -- `set_binding_status` unfreeze without timelock

- **Severity:** LOW
- **Status:** Open
- **Introduced:** 2026-04-22 (master-audit NEW-SEC-05; renumbered
  from 034)
- **Evidence:** `programs/schema-registry/src/lib.rs:322-349`
- **Description.** Unfreeze from `STATUS_FROZEN` to `STATUS_ACTIVE`
  requires only the current authority; no timelock.
- **Remediation.** Either 24h timelock, or document the trade-off
  explicitly in `adr/0009-dao-voting-plus-trust-anchor-bypass.md`.

### SOLID-SEC-036 -- Stale nullifier module docstring

- **Severity:** LOW
- **Status:** Fixed (2026-04-24, Phase 2 commit `73871dc`; reference
  in `sec/audits/2026-04-24_v0.6_phase2_closeout.md`)
- **Introduced:** 2026-04-22 (corrected from master-audit NEW-SEC-07;
  renumbered from 035)
- **Evidence (pre-fix):** `crates/solid-core/src/nullifier.rs:1-8`.
  Module docstring described the 3-argument nullifier; implementation
  at `:23-40` was the 5-argument hardened formula.  Doc-only mismatch.
- **Remediation (landed).** Full rewrite of the module docstring to
  describe the 6-input Poseidon formula (post ADR-0014) and the
  full evolution history (3-input -> 5-input -> 6-input) with
  rationale in-source.

### SOLID-SEC-037 -- `WithdrawAfterCooldown` missing authority constraint

- **Severity:** INFO
- **Status:** Open
- **Introduced:** 2026-04-22 (master-audit NEW-SEC-01; renumbered
  from 036)
- **Evidence:** `programs/issuer-registry/src/lib.rs:995-1005`
- **Description.** No `constraint = issuer_account.authority ==
  issuer_authority.key()`, unlike `WithdrawStake` at `:788`. Seed
  derivation is primary guard.
- **Remediation.** Add the constraint for parity.

### SOLID-SEC-038 -- Master-audit informational cluster

- **Severity:** INFO
- **Status:** Open
- **Introduced:** 2026-04-22 (master-audit NEW-SEC-04, NEW-SEC-08,
  NEW-SEC-10; renumbered from 037)
- **Description.** Three informational hygiene items:
  - `borrow` in `negate_g1_point` is `i16`; add a comment.
  - `SchemaTreeBinding` writer 145 bytes, reader 113 bytes
    (forward-compat); document in `verify_schema_root_binding`.
  - `GreaterThan(8)` on `orSum` (max 4); document the bound.
- **Remediation.** Code comments only.

### SOLID-SEC-039 -- `bootstrap_issuer.ts` runnable against mainnet

- **Severity:** LOW
- **Status:** Fixed (2026-04-23, Phase 2 prelude)
- **Introduced:** 2026-04-23 (landed with SOLID-SEC-011 in commit
  `8ad20e3`; surfaced during Phase 2 entry review)
- **Evidence (pre-fix):** `scripts/bootstrap_issuer.ts` defaulted to
  `http://127.0.0.1:8899` but honored `SOLID_RPC_URL` with no guard;
  the script's baked-in parameters (20-second voting period,
  1-lamport min stake, freshly-minted governance supply) are test-
  only and would have been irreversibly installed into
  `initialize_registry` on the first run against a fresh real
  cluster.  Idempotency saves a re-run, not the first run.
- **Impact.** Footgun.  Operator mistake + first-run-of-cluster =
  real DAO initialized with joke parameters.  Not exploitable by an
  attacker, but recoverable only by closing + re-initialising the
  `RegistryConfig` PDA.
- **Remediation (landed).** Refuse to run when `SOLID_RPC_URL`
  does not match `localhost|127.0.0.1|0.0.0.0` unless
  `SOLID_ALLOW_NON_LOCALNET=1` is set.  The opt-in flag is
  documented alongside the override knobs for voting period / stake
  so a non-local run must consciously set production parameters.
- **Regression gate.** Host-side: running with any non-local RPC
  exits with code 2 and a clear error.  CI `e2e_localnet` job
  (Phase 2 deliverable) exercises the happy path.

### SOLID-SEC-040 -- `check_program_ids.py` silent on missing manifest

- **Severity:** LOW
- **Status:** Fixed (2026-04-23, Phase 2 prelude)
- **Introduced:** 2026-04-22 (script authored without the
  manifest-presence invariant)
- **Evidence (pre-fix):** `scripts/check_program_ids.py` iterated
  `deployments/*.json` but treated "no manifests" as consistent.
  Deleting `deployments/devnet.json` while Anchor.toml kept
  `[programs.devnet]` silently passed the gate -- contradicting
  CLAUDE.md's "Anchor.toml, declare_id, and deployments all agree"
  invariant.
- **Impact.** Deployment drift could land silently, caught only at
  deploy time.
- **Remediation (landed).** The script now asserts that every
  non-`localnet` cluster declared in Anchor.toml has a matching
  `deployments/<cluster>.json` containing every program in
  `PROGRAM_KEYS`.
- **Regression gate.** Manual check confirmed (move manifest
  aside -> exit 1 with actionable error; restore -> exit 0).
  The pre-existing CI job `program_id_consistency` runs this
  script on every push.

### SOLID-SEC-041 -- VK JSON artifact not content-addressed

- **Severity:** LOW
- **Status:** Fixed (2026-04-25, Phase 3 impl 3)
- **Introduced:** 2026-04-22 (initialize.ts design)
- **Evidence (pre-fix):** `scripts/initialize.ts:245-278` uploaded
  whatever `circuits/build/verification_key.json` happened to be on
  disk; `circuits/scripts/setup.js` printed a sha256 of the zkey
  but neither the zkey hash nor a VK hash was cross-checked at
  upload time.
- **Impact.** An operator pointing `initialize.ts` at a stale
  `circuits/build/` after a source-tree `git pull` uploads the old
  VK against new circuit code; every subsequent real proof
  verifies as invalid and presents as a chain-side problem rather
  than a build-side problem.
- **Remediation (landed).**  Two-way content-addressing in
  `scripts/initialize.ts`:
  - `circuits/scripts/setup.js` now writes
    `circuits/build/verification_key.sha256` (hex sha256 of the
    exact bytes written to `verification_key.json`) alongside the
    VK artifact.  The console output also prints the VK sha256
    alongside the zkey sha256 so release notes and ADRs can pin
    it.
  - `scripts/initialize.ts` reads `verification_key.json`, computes
    its sha256, and compares against `SOLID_VK_SHA256` (env var,
    authoritative) or `verification_key.sha256` (file,
    developer-loop convenience).  Fails fast on mismatch and fails
    fast if neither source is available.
  Hooks cleanly into the SOLID-SEC-012 multi-party ceremony work
  later: the canonical hash published by the ceremony replaces the
  single-party pin via the env-var path.
- **Regression gate (landed).**  Behavioural coverage comes via the
  `e2e_localnet` CI job, which runs `setup.js` + `npm run e2e`
  clean from checkout -- any drift in the serialization or the
  pinning surfaces as a hash mismatch at upload time.  Full
  handler-level failure-mode tests (wrong env var, missing pin,
  tampered VK) land in the integration suite (`12_vk_rotation` +
  adjacent).

### SOLID-SEC-042 -- `VerifierConfig::SPACE` doc drift

- **Severity:** INFO
- **Status:** Fixed (2026-04-23, Phase 2 prelude)
- **Introduced:** SEC-005 fix (`402fb4e`, Phase 1 Tier 2)
- **Evidence (pre-fix):** `docs/POST_REMEDIATION_AUDIT.md:386-388`
  still documented the constant as `= 45` bytes
  (pre-`timestamp_skew_seconds`).  `docs/MODULE_CONTRACTS.md:469`
  documented `space=45` with a stale field table claiming
  `next_vk_chunk: u8` (actual: `u16`) and
  `timestamp_skew_secs: u16` (actual: `u32`), tagged PENDING when
  the fix had already landed.  Source of truth is
  `programs/zk-verifier/src/lib.rs:615`.
- **Impact.** Doc lies.  No code bug; violates the "no doc lies"
  non-negotiable in `plan/IMPLEMENTATION_PLAN.md` Section 0.
- **Remediation (landed).** Both docs updated to match the source
  of truth (`SPACE = 49`, layout reconciled).
- **Regression gate.** Future drift is caught on eyeball review;
  a CI linter for "cite source-of-truth line number for every
  numerical SPACE claim" is a possible Phase 3 hygiene item.

### SOLID-SEC-043 -- `IssuerTreeBinding.operator` is a single signer

- **Severity:** MEDIUM
- **Status:** Open
- **Introduced:** 2026-04-24 (surfaced in the v0.6 deep audit,
  `sec/audits/2026-04-24_v0.6_deep_comprehensive_audit.md` section 4.1).
  The risk entered the codebase with ADR-0014's
  `IssuerTreeBinding` PDA (Phase 2, commit `58afb93`) but no registry
  entry tracked it until this sweep.
- **Evidence.** `programs/issuer-registry/src/lib.rs` declares
  `IssuerTreeBinding.operator: Pubkey` as a single key.  Every
  tree-mutating instruction (`update_issuer_tree_root`,
  `append_issuer_leaf`, `replace_issuer_leaf`, `revoke_issuer_atomic`)
  checks `signer.key() == binding.operator`.  A compromise of that
  single keypair lets an attacker install an arbitrary tree root and
  forge issuer-tree membership for any Poseidon(5) leaf preimage they
  choose, which propagates into every subsequent proof's 6-input
  nullifier and integrity check.
- **Impact.** On a single-key compromise the attacker can:
  (a) insert a leaf for an attacker-controlled BJJ keypair against
  any authority, (b) shift the root in a way that invalidates
  pre-compromise nullifiers and forces a chain-wide replay window,
  (c) race real atomic revocations with a stale-root write.  Same
  centralization class Polygon ID has on its state-transition
  operator; still unacceptable at external audit.
- **Remediation (planned).** Replace `operator: Pubkey` with a
  `operator_authority: Pubkey` that is required to be a PDA signer
  of either a Squads 3-of-5 multisig or the SolID DAO threshold
  PDA.  All four tree-mutating ix wrap the existing logic behind
  that authority.  ADR amendment on 0014.  Target: Phase 3
  close-out, before external audit.
- **Regression gate.** Unit test that drives `update_issuer_tree_root`
  with a non-multisig signer and asserts rejection; integration
  test in the suite's `02_issuer_lifecycle` that exercises the
  multisig happy path.

### SOLID-SEC-044 -- Cooldown status does not replace the issuer's tree leaf

- **Severity:** LOW
- **Status:** Open
- **Introduced:** 2026-04-24 (surfaced in the v0.6 deep audit,
  `sec/audits/2026-04-24_v0.6_deep_comprehensive_audit.md` section 4.2).
  Entered the codebase with ADR-0014 atomic-hook design (commit
  `167138e`): `revoke_issuer_atomic` wires a `replace_leaf` CPI into
  the revocation path, but `request_withdrawal` -- the voluntary
  Approved -> Cooldown transition -- does not.
- **Evidence.** `programs/issuer-registry/src/lib.rs` :: the
  `request_withdrawal` handler flips `IssuerAccount.status` from
  `Approved` to `Cooldown` and bumps `status_epoch`, but leaves the
  issuer's Poseidon(5) leaf in the SPL-AC tree as the Approved-time
  leaf.  Because the circuit's STEP 0.75 membership proof checks the
  leaf against the current tree root and does not read `status` on
  chain, a credential issued under a Cooldown issuer continues to
  produce a valid proof.  RESUME.md flagged this as an open Phase 3
  decision.
- **Impact.** Cooldown is today functionally equivalent to Approved
  for proof-verification purposes.  Holders of credentials from a
  Cooldown issuer can still prove membership against the active
  issuer tree, which contradicts the intent that Cooldown signals
  "the issuer is winding down -- do not rely on its attestations".
  Not a soundness break (the issuer has not been revoked for cause);
  is a semantic gap.
- **Remediation (planned).** Add `request_withdrawal_atomic`
  mirroring `revoke_issuer_atomic`: flip to Cooldown, bump
  `revocation_nonce`, CPI `replace_leaf` with the zero-leaf.
  Legacy `request_withdrawal` returns `IssuerTreeUpdateRequired`
  for any `enrolled_in_tree == true` issuer.  Amend ADR-0014 to
  document the "Cooldown is verify-negative" stance explicitly.
  Target: Phase 3 close-out.
- **Regression gate.** Rust unit test `test_request_withdrawal_
  atomic_replaces_leaf_with_zero` + the atomic-hook property test
  to add: every status transition that shifts verify-correctness
  moves the tree root in the same ix.

---

## History

| Date       | Audit                                                                | New IDs                                   | IDs closed |
|------------|----------------------------------------------------------------------|-------------------------------------------|------------|
| 2026-04-22 | `sec/audits/2026-04-22_v0.3_comprehensive_audit.md`                  | SOLID-SEC-001..027                        | 0          |
| 2026-04-22 | `sec/audits/2026-04-22_v0.3_antigravity_deep_system_audit.md`        | (folded into master)                      | 0          |
| 2026-04-22 | `sec/audits/2026-04-22_v0.3_master_audit.md`                         | SOLID-SEC-031..038 (renumbered after v0.4 collision) | 0 |
| 2026-04-22 | `sec/audits/2026-04-22_v0.4_comprehensive_audit_and_build_plan.md`   | SOLID-SEC-028..030                        | 0          |
| 2026-04-23 | `sec/audits/2026-04-23_v0.5_phase1_closeout.md`                      | --                                        | SOLID-SEC-001, -002, -003, -005, -009, -011, -020, -027, -028, -029, -030, -031, -032, -033 (14 Phase 1 items) |
| 2026-04-23 | Phase 2 prelude (code review surfaced new items; in-session fix)     | SOLID-SEC-039, -040, -041, -042           | SOLID-SEC-039, -040, -042 (3 of 4 fixed same commit) |
| 2026-04-24 | `sec/audits/2026-04-24_v0.6_phase2_closeout.md`                      | --                                        | SOLID-SEC-004, -008, -036 (Phase 2 scope closed) |
| 2026-04-25 | `sec/audits/2026-04-24_v0.6_deep_comprehensive_audit.md` + Phase 3 doc sweep | SOLID-SEC-043, -044                       | 0 (both introduced Open)                          |
| 2026-04-25 | Phase 3 impl 1 (SEC-007 + registry detail reconciliation)            | --                                        | SOLID-SEC-007 (this commit); SOLID-SEC-004, -008, -036 detail sections reconciled Open -> Fixed to match the status board flipped at Phase 2 close-out |
| 2026-04-25 | Phase 3 impl 2 (ADR-0015 VK freeze-gate + rotation timelock)         | --                                        | SOLID-SEC-006 (Part 1 on-chain; Part 2 circuit-bound vk_generation deferred to the next trusted-setup cycle) |
| 2026-04-25 | Phase 3 impl 3 (SEC-041 content-addressed VK artifact)               | --                                        | SOLID-SEC-041 (verification_key.sha256 emitted by setup.js; initialize.ts enforces the pin via env or file) |

### Note on the 2026-04-22 numbering

The v0.4 audit and the master consolidated audit were authored in
parallel branches. v0.4 landed on `main` first with SOLID-SEC-028,
029, 030 occupying those IDs. Per `sec/README.md` ("IDs are stable,
never reused"), the master audit's originally-proposed 028..037
were renumbered on merge:

- Two findings (master-audit IdentityAnchor `enabled` and
  `stake_vault` GC) were identical to v0.4's 029 and 030 and were
  collapsed into those IDs; the master-audit evidence was added to
  the existing entries.
- The other eight master-audit findings were renumbered to
  SOLID-SEC-031..038 in order.

---

## Next-cycle checklist (for the auditor running the next pass)

1. For every `Fixed` status, verify the referenced commit lands the
   claimed regression test and addresses root cause. Flip
   `Fixed -> Verified` or reopen with child entry `SOLID-SEC-NNN.1`.
2. For every `Open` status older than 90 days, decide: escalate
   severity, accept-as-`Won't Fix` with compensating control, or flag
   as milestone slip.
3. Run full audit methodology against all code changed since last
   snapshot. Net-new findings append here.
4. Update summary counts and the "Last audit" header.
5. Create new snapshot under `sec/audits/<date>_<version>_<slug>.md`.
6. Cross-reference all new IDs with `plan/IMPLEMENTATION_PLAN.md` to
   ensure each has a phase assignment and regression gate.
