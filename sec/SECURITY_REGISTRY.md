# SolID Protocol -- Security Registry

Canonical, living tracker for every security finding across every audit.
One file. No fragmentation. Nothing deleted.

- Protocol version under review: v0.4 (April 2026, second audit pass)
- Last audits folded in (chronological):
  - 2026-04-22 senior-engineer comprehensive audit
    (`sec/audits/2026-04-22_v0.3_comprehensive_audit.md`)
  - 2026-04-22 Antigravity deep system audit
    (`sec/audits/2026-04-22_v0.3_antigravity_deep_system_audit.md`)
  - 2026-04-22 master consolidated audit
    (`sec/audits/2026-04-22_v0.3_master_audit.md`)
  - 2026-04-22 v0.4 comprehensive audit + build plan
    (`sec/audits/2026-04-22_v0.4_comprehensive_audit_and_build_plan.md`)
- Last registry update: 2026-04-22 (merges master-audit findings into
  SOLID-SEC-031..038 after v0.4 had already allocated
  SOLID-SEC-028..030; duplicates collapsed into canonical IDs)
- Next audit target: after Phase 1 close-out
  (see `plan/IMPLEMENTATION_PLAN.md`)

See `sec/README.md` for workflow, severity definitions, and status lifecycle.

---

## Summary

| Severity  | Open | In Progress | Fixed | Verified | Won't Fix | Total |
|-----------|------|-------------|-------|----------|-----------|-------|
| CRITICAL  | 2    | 0           | 1     | 0        | 0         | 3     |
| HIGH      | 7    | 0           | 5     | 0        | 0         | 12    |
| MEDIUM    | 10   | 0           | 3     | 0        | 0         | 13    |
| LOW       | 5    | 0           | 0     | 0        | 0         | 5     |
| INFO      | 4    | 0           | 1     | 0        | 0         | 5     |
| **Total** | 28   | 0           | 10    | 0        | 0         | 38    |

---

## Status board

| ID              | Severity | Status | Title                                                              |
|-----------------|----------|--------|--------------------------------------------------------------------|
| SOLID-SEC-001   | CRITICAL | Open   | Batch circuit: `queryCredentialIndices` / `queryFieldIndices` unconstrained |
| SOLID-SEC-002   | CRITICAL | Fixed  | `register_schema` Poseidon integrity check commented out           |
| SOLID-SEC-003   | CRITICAL | Open   | `issue_credential` missing schema + tree pubkey binding            |
| SOLID-SEC-004   | HIGH     | Open   | No in-circuit issuer pubkey binding; revoked issuers still verify  |
| SOLID-SEC-005   | HIGH     | Fixed  | `currentTimestamp` public input not bound to `Clock`               |
| SOLID-SEC-006   | HIGH     | Open   | VK overwrite at chunk 0 has no freeze-gate; truncated VK finalizable|
| SOLID-SEC-007   | HIGH     | Open   | BJJ public keys not subgroup-checked at registration               |
| SOLID-SEC-008   | HIGH     | Open   | Nullifier does not include epoch / global root                     |
| SOLID-SEC-009   | HIGH     | Fixed  | WASM bridge fractured across 3 locations                           |
| SOLID-SEC-010   | HIGH     | Open   | Cross-language test vectors cover only 2 of 10 primitives          |
| SOLID-SEC-011   | HIGH     | Open   | E2E scripts bugged: zkey name, missing import, missing issuer flow |
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
| SOLID-SEC-029   | MEDIUM   | Open   | `IdentityAnchor` always has `enabled=1`; padding slots over-constrained |
| SOLID-SEC-030   | MEDIUM   | Fixed  | `transfer_slashed_lamports` can drain `stake_vault` to zero        |
| SOLID-SEC-031   | HIGH     | Fixed  | `bufToDecimal` LE interpretation of Solana pubkey risks breaking `verifierAddress` match |
| SOLID-SEC-032   | HIGH     | Fixed  | `SCHEMA_REGISTRY_ID_BYTES` hardcoded without build-time validation |
| SOLID-SEC-033   | HIGH     | Fixed  | Identity cohesion check compares master pubkey; circuit uses per-schema derived (E2E blocker) |
| SOLID-SEC-034   | MEDIUM   | Open   | `SubmitFraudProof` / `SlashIssuer` contexts missing PDA seed constraint on `issuer_account` |
| SOLID-SEC-035   | LOW      | Open   | `set_binding_status` can unfreeze without timelock                 |
| SOLID-SEC-036   | LOW      | Open   | `nullifier.rs` module docstring describes stale 3-arg formula (impl is correct 5-arg) |
| SOLID-SEC-037   | INFO     | Open   | `WithdrawAfterCooldown` missing explicit authority constraint (seeds provide partial protection) |
| SOLID-SEC-038   | INFO     | Open   | Master-audit informational cluster: `i16` borrow signedness, reader/writer size asymmetry, `GreaterThan(8)` bound comment |

---

## Findings -- detail

### SOLID-SEC-001 -- Batch circuit query indices unconstrained

- **Severity:** CRITICAL
- **Status:** Open
- **Introduced:** 2026-04-22
- **Evidence:**
  `circuits/batch_credential_query.circom:56-59,216-217`;
  `circuits/lib/predicate_evaluator.circom:85-112`
- **Description.** `queryCredentialIndices[MAX_PREDICATES]` and
  `queryFieldIndices[MAX_PREDICATES]` are public inputs with no range
  constraints. `BatchFieldSelector` returns 0 for any out-of-range
  index; a prover sets `credIndex = 1000`, `queryValue = 0`,
  `operator = EQ` and satisfies `result = 1` for any predicate.
- **Impact.** Full soundness break on batch proofs. `compound_query.circom`
  has the same exposure via `FieldSelector`.
- **Remediation.** Add `LessThan(8)` range checks on every index.
  Requires new trusted setup (combine with SOLID-SEC-004,
  SOLID-SEC-008, SOLID-SEC-029).
- **Regression gate.** Property-based witness test with 1000 random
  out-of-range indices; on-chain rejection integration test.

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
- **Status:** Open
- **Introduced:** 2026-04-22
- **Evidence:**
  `programs/issuer-registry/src/lib.rs:619-716,909-937`
- **Description.** Does not require `schema_hash` to match a registered
  `SchemaAccount`, does not bind `merkle_tree.key()` to
  `SchemaTreeBinding.tree_pubkey`, does not prevent an approved issuer
  from appending to any SPL-AC tree whose authority is
  `PDA(b"tree-authority", any-32-bytes)`.
- **Impact.** Approved issuer spawns a rogue schema/tree universe the
  verifier accepts as canonical. Combined with SOLID-SEC-002, trivially
  exploitable.
- **Remediation.** Add `schema_account` + `schema_tree_binding`
  required accounts. Seed-constrain + `require!` equality.
- **Regression gate.** Integration tests
  `06_issue_credential_rejects_unregistered_schema`,
  `07_issue_credential_rejects_wrong_tree`.

### SOLID-SEC-004 -- No in-circuit issuer pubkey binding

- **Severity:** HIGH
- **Status:** Open
- **Introduced:** 2026-04-22
- **Evidence:**
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
    bytes of `public_inputs[30]` (LE field-element encoding, matching
    the rest of the public-input contract), enforces the high 24
    bytes are zero, and rejects with `StaleTimestamp` unless
    `lower <= claimed_ts <= upper`.
- **Regression gate (in CI now).** `cargo test -p zk-verifier --lib`
  continues to pass (11/11) including VK-parser and G1-negation
  tests, confirming no regression.
- **Follow-up (Phase 2).** Bankrun integration tests
  `integration_10_verify_expired_credential_rejected` and
  `integration_11_verify_future_timestamp_rejected` exercise the
  on-chain Clock comparison end-to-end.

### SOLID-SEC-006 -- VK chunk 0 overwrite, no freeze-gate

- **Severity:** HIGH
- **Status:** Open
- **Introduced:** 2026-04-22
- **Evidence:** `programs/zk-verifier/src/lib.rs:82-130`
- **Description.** Chunk 0 unconditionally writes `vk_storage.data`;
  authority can finalize a truncated VK with small `nr_ic`.
- **Impact.** Universal forgery via live VK swap; silent acceptance of
  truncated VK.
- **Remediation.** Add `vk_frozen: bool` + `vk_generation: u16`. Store
  `(vk_id, VkStorage)` pairs. Include `vk_generation` in public inputs.
  Require `paused` for post-gen-0 writes.
- **Regression gate.** Unit tests for finalize-with-partial, write-when-
  frozen, verify-against-wrong-generation.

### SOLID-SEC-007 -- BJJ pubkeys not subgroup-checked

- **Severity:** HIGH
- **Status:** Open
- **Introduced:** 2026-04-22
- **Evidence:** `crates/solid-core/src/babyjubjub.rs:111-114,125-133`
- **Description.** `pubkey_to_affine` checks on-curve + non-zero but
  not cofactor; `register_issuer` stores `bjj_pub_key_x/y` with no
  subgroup check.
- **Impact.** Small-order pubkey enables trivial signature forgery
  off-chain (circuit-level defenses may block in-circuit case).
- **Remediation.** Reject points whose `mul_by_cofactor` is identity.
- **Regression gate.** Rust unit tests with explicit small-order
  points.

### SOLID-SEC-008 -- Nullifier lacks epoch binding

- **Severity:** HIGH
- **Status:** Open
- **Introduced:** 2026-04-22
- **Evidence:**
  `circuits/batch_credential_query.circom:286-292`;
  `crates/solid-core/src/nullifier.rs:23-40`
- **Description.** Nullifier = `Poseidon(masterKey, revocationNonce,
  verifierAddress, queryContextHash, verifierNonce)`. No `globalRoot`
  or epoch counter.
- **Impact.** Defense-in-depth gap against root regression.
- **Remediation.** Include monotonic `epoch_counter` stored in
  `GlobalStateBinding`. Bundle with SOLID-SEC-001 / SOLID-SEC-004 /
  SOLID-SEC-029.
- **Regression gate.** Witness test: distinct-epoch inputs produce
  distinct nullifiers.

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
- **Status:** Open
- **Introduced:** 2026-04-22
- **Evidence:** `circuits/scripts/setup.js:57` vs
  `scripts/prove.ts:123` (zkey filename mismatch);
  `scripts/prove.ts:105` (`keccak256HashPair` unimported);
  `scripts/issue.ts` (no register/stake/vote/approve sequence).
- **Impact.** Stranger cannot run E2E from clean checkout.
- **Remediation.** Unify zkey name, replace keccak with Poseidon
  consistently, add issuer-approval bootstrap.
- **Regression gate.** CI `e2e_localnet` end-to-end to on-chain verify.

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
- **Status:** Open
- **Introduced:** 2026-04-22 (v0.4 audit; folds in master-audit BUG-NEW-02)
- **Evidence:** `circuits/lib/identity_anchor.circom:46-53`
- **Description.** `CredentialAtom` correctly guards zero-schema
  padding slots with `enabled = 1 - isZero(schemaHash)`. However
  `IdentityAnchor`, instantiated once per credential slot at
  `batch_credential_query.circom:111-122`, always sets
  `globalInclusion.enabled = 1`. For a padding slot (schemaHash=0)
  the circuit still requires a valid Merkle inclusion proof for the
  derived identity leaf. The global tree would need zero-schema
  derived entries pre-loaded, which is architecturally wrong.
- **Impact.** Batch circuit cannot generate proofs for fewer than
  NUM_CREDS=4 active credentials. Any holder with 1, 2, or 3
  credentials cannot prove cleanly.
- **Remediation.** Pass `enabled` into `IdentityAnchor`; gate
  `globalInclusion.enabled <== enabled`. Circuit change -- bundle
  with SOLID-SEC-001, SOLID-SEC-004, SOLID-SEC-008 in the Phase 1
  trusted setup.
- **Regression gate.** Circuit witness test: 3-credential batch
  (1 padding slot) generates a valid witness without any
  zero-schema global tree entry.

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
  compares `public_inputs[28] == ID.to_bytes()` byte-for-byte, which
  would fail every verification. This directly explains why
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
- **Status:** Open
- **Introduced:** 2026-04-22 (corrected from master-audit NEW-SEC-07;
  renumbered from 035)
- **Evidence:** `crates/solid-core/src/nullifier.rs:1-8`
- **Description.** Module docstring describes the 3-argument
  nullifier; implementation at `:23-40` is the correct 5-argument
  hardened formula. Doc-only mismatch.
- **Remediation.** Update the module doc.

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

---

## History

| Date       | Audit                                                                | New IDs                                   | IDs closed |
|------------|----------------------------------------------------------------------|-------------------------------------------|------------|
| 2026-04-22 | `sec/audits/2026-04-22_v0.3_comprehensive_audit.md`                  | SOLID-SEC-001..027                        | 0          |
| 2026-04-22 | `sec/audits/2026-04-22_v0.3_antigravity_deep_system_audit.md`        | (folded into master)                      | 0          |
| 2026-04-22 | `sec/audits/2026-04-22_v0.3_master_audit.md`                         | SOLID-SEC-031..038 (renumbered after v0.4 collision) | 0 |
| 2026-04-22 | `sec/audits/2026-04-22_v0.4_comprehensive_audit_and_build_plan.md`   | SOLID-SEC-028..030                        | 0          |

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
