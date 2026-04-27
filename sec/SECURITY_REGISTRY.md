# SolID Protocol -- Security Registry

Canonical, living tracker for every security finding across every audit.
One file. No fragmentation. Nothing deleted.

- Protocol version under review: v0.6.1 (April 2026, post-Phase-3-impl-4)
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
  - 2026-04-25 v0.6.1 deep comprehensive audit
    (`sec/audits/2026-04-25_v0.6.1_deep_comprehensive_audit.md`)
    -- **canonical post-Phase-3-impl-4 state-of-protocol**
- Last registry update: 2026-04-25 late-session (build-pipeline
  restoration session: opens **SOLID-SEC-047** as Fixed -- BPF
  stack-frame overflow in `verify_batch_proof` wrapper, surfaced
  once the upstream light-poseidon link failure was lifted by the
  Poseidon syscall refactor; refreshes summary counts).  Earlier
  same day folded the v0.6.1 audit (opens SOLID-SEC-045 and
  SOLID-SEC-046 from Section 5.4).
- Next audit target: after P0 close-out (NEW-01/45, NEW-02/46,
  NEW-03/47-followups, SEC-010, integration suite 02..11) per
  v0.6.1 Section 6.1.  Localnet `npm run e2e` punch list lives
  in `docs/E2E_BLOCKERS.md`.

See `sec/README.md` for workflow, severity definitions, and status lifecycle.

---

## Summary

| Severity  | Open | In Progress | Fixed | Verified | Won't Fix | Total |
|-----------|------|-------------|-------|----------|-----------|-------|
| CRITICAL  | 0    | 0           | 3     | 0        | 0         | 3     |
| HIGH      | 3    | 0           | 10    | 0        | 0         | 13    |
| MEDIUM    | 12   | 0           | 5     | 0        | 0         | 17    |
| LOW       | 4    | 0           | 5     | 0        | 0         | 9     |
| INFO      | 4    | 0           | 2     | 0        | 0         | 6     |
| **Total** | 23   | 0           | 25    | 0        | 0         | 48    |

Delta vs prior summary: SOLID-SEC-047 added as Fixed (MEDIUM); SOLID-
SEC-048 added as Open / interim-bypass (HIGH; mainnet deploy-blocker).
SEC-048 captures the BPF-runtime extension to SEC-007: the in-handler
prime-order subgroup check is logic-correct but exceeds the 1.4M CU
per-tx ceiling on BPF, so localnet/devnet builds gate it behind a
`sec007-skip-onchain` Cargo feature with the off-chain SDK predicate
acting as the load-bearing gate while the real fix is in flight.  This
session's other working-tree fixes (circuit compile, dep cascade,
Poseidon BPF refactor) are build-pipeline issues, not security
findings; they're tracked in `docs/E2E_BLOCKERS.md`, not here.

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
| SOLID-SEC-044   | LOW      | Fixed  | Cooldown status does not replace the issuer's tree leaf (proofs from Cooldown issuers still verify) |
| SOLID-SEC-045   | MEDIUM   | Fixed  | `revoke_issuer_atomic` / `request_withdrawal_atomic` do not update `IssuerTreeBinding.current_root` in the same ix |
| SOLID-SEC-046   | MEDIUM   | Open   | No CU-budget regression gate on `verify_batch_proof` |
| SOLID-SEC-047   | MEDIUM   | Fixed  | `verify_batch_proof` Anchor wrapper exceeds BPF 4 KB per-frame stack by ~456 B (structural; surfaced post Poseidon refactor) |
| SOLID-SEC-048   | HIGH     | Open (interim bypass live) | `register_issuer` BJJ prime-order subgroup check exceeds 1.4M CU per-tx ceiling on BPF; localnet/devnet builds gate the check behind `sec007-skip-onchain` Cargo feature -- mainnet-blocking until a CU-affordable on-chain replacement ships |
| SOLID-SEC-049   | HIGH     | Fixed  | `SPL_AC_REPLACE_LEAF_DISCRIMINATOR` mismatched `sha256("global:replace_leaf")[..8]` -- both atomic ixs would have failed at the SPL AC CPI with `InstructionFallbackNotFound`; latent because no integration test had ever exercised revoke / cooldown |
| SOLID-SEC-050   | MEDIUM   | Fixed  | `batch_credential_query.circom` schema-ordering canonicality bypass -- strict-ascending check skipped when next slot is inactive, so non-canonical interleavings like `[A1, 0, A2, A3]` admit multiple distinct nullifiers per logical credential set, defeating the per-claim rate-limit |
| SOLID-SEC-051   | LOW      | Open   | `batch_credential_query.circom` admits all-padding `[0,0,0,0]` proofs that the on-chain handler accepts (skips zero-schema slots at lib.rs:500-502); query-driven semantics force most predicates to fail on zero data, but "at least 1 credential" is not a circuit-level invariant -- defense-in-depth fix is `IsZero(schemaHashes[0]).out === 0` (1 constraint) |
| SOLID-SEC-052   | HIGH     | Fixed (partial) | Two BPF-runtime / cross-layer coord-form drifts surfaced by the 2026-04-28 e2e bring-up: (a) `is_on_curve` / `is_identity` rebuilt to evaluate the circomlib-native curve equation directly (avoids `EdwardsAffine::new_unchecked + iso transform` which fails on BPF for valid points); (b) the WASM bridge `solid_wasm_bg.wasm` was stale (Apr 25, pre-cff06c2) and produced arkworks-form pubkey bytes while the on-chain code post-cff06c2 expected circomlib-native form -- rebuilding `wasm-pack build wasm/` aligned both layers and unblocked `register_issuer`.  Outstanding: `pubkey_to_affine` and `is_in_prime_order_subgroup` still go through the BPF-incompat iso path; only matters when SEC-048 closes (full subgroup check on-chain). |
| SOLID-SEC-053   | HIGH     | Fixed  | Off-chain `solid_core::babyjubjub::sign` computed `S = r + h * sk` while circomlib's `EdDSAPoseidonVerifier.circom` (the in-circuit verifier) checks `S * Base8 == R8 + h * 8 * A` (cofactor-8 scaling on the right).  Self-consistent on host (sign + verify both omitted the 8) but every signature got rejected by the Groth16 witness inside `CredentialAtom`.  Doc-lie at the function header explicitly miswrote the circomlib equation as the no-8 form (L6).  Fixed: `S = r + h * 8 * sk` in `sign`, matching `rhs = R8 + h * 8 * A` in `verify`.  Regression gate: `sec_053_eddsa_cofactor_8_round_trip` (positive: fresh sig verifies; negative: hand-crafted no-8 sig is rejected). |

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
- **Status:** Fixed (2026-04-25, Phase 3 impl 4, ADR-0014 amendment)
- **Introduced:** 2026-04-24 (surfaced in the v0.6 deep audit,
  `sec/audits/2026-04-24_v0.6_deep_comprehensive_audit.md` section 4.2).
  Entered the codebase with ADR-0014 atomic-hook design (commit
  `167138e`): `revoke_issuer_atomic` wires a `replace_leaf` CPI into
  the revocation path, but `request_withdrawal` -- the voluntary
  Approved -> Cooldown transition -- did not.
- **Evidence (pre-fix).** `programs/issuer-registry/src/lib.rs` ::
  the `request_withdrawal` handler flipped `IssuerAccount.status`
  from `Approved` to `Cooldown` and bumped `status_epoch`, but left
  the issuer's Poseidon(5) leaf in the SPL-AC tree as the
  Approved-time leaf.  Because the circuit's STEP 0.75 membership
  proof checks the leaf against the current tree root and does not
  read `status` on chain, a credential issued under a Cooldown
  issuer continued to produce a valid proof for the full 14-day
  cooldown window.
- **Impact.** Cooldown was functionally equivalent to Approved for
  proof-verification purposes.  Holders of credentials from a
  Cooldown issuer could still prove membership against the active
  issuer tree, contradicting the intent that Cooldown signals "the
  issuer is winding down -- do not rely on its attestations".  Not
  a soundness break (the issuer had not been revoked for cause);
  was a semantic gap.
- **Remediation (landed).**  ADR-0014 amendment + new instruction:
  - `programs/issuer-registry/src/lib.rs` ::
    `request_withdrawal_atomic(old_root)` mirrors
    `revoke_issuer_atomic` exactly except for the target status.
    Bumps `revocation_nonce` + `status_epoch`, flips `status` to
    `Cooldown`, sets `cooldown_ends_at`, and CPIs
    `spl_account_compression::replace_leaf` in one tx.  Authority
    is the issuer's own keypair (withdrawal is voluntary, unlike
    revocation).
  - Legacy `request_withdrawal` handler now returns
    `IssuerTreeUpdateRequired` for any `is_tree_enrolled == true`
    issuer.  Pre-backfill (unenrolled) issuers retain the legacy
    path.
  - New `RevokeReason::CooldownRequested` variant in the
    `IssuerLeafReplaced` event so indexers can distinguish
    voluntary exit from punitive revocation.
  - ADR-0014 amended with the "Cooldown is verify-negative" stance
    and the leaf-replacement correctness argument.
- **Regression gate.**  Full transaction-level coverage lands in
  the integration suite expansion (`tests/integration/
  02_issuer_lifecycle.test.ts` and `08_revoke_and_tree.test.ts`).
  Static coverage today: the handler is a direct clone of
  `revoke_issuer_atomic` with the status target changed; the Phase
  2 ADR-0014 regression harness for atomic hooks covers the
  shared CPI plumbing.

### SOLID-SEC-045 -- Atomic handlers do not update `IssuerTreeBinding.current_root` in-ix

- **Severity:** MEDIUM
- **Status:** Fixed (2026-04-27, this session).  Closure receipts:
  helper unit tests `keccak_root_recompute_*` (7 cases) +
  `write_binding_root_*` (5 cases) + program-level layout pins
  (5 cases); BPF link green for `issuer-registry`; full host-side
  workspace 154+ tests green.  Integration test 07b
  (`tests/integration/07b_revoke_atomic_binding_update.test.ts`)
  is the canonical end-to-end gate per the audit and remains the
  outstanding deliverable -- it requires SPL AC + noop fixtures
  plus a bankrun + jest harness that did not exist when this fix
  landed; tracked as a follow-up under the integration suite.
- **Introduced:** 2026-04-24 (Phase 2 design of
  `revoke_issuer_atomic`, extended by the Phase 3 impl 4 clone to
  `request_withdrawal_atomic`).  Surfaced in the v0.6.1 deep audit
  (`sec/audits/2026-04-25_v0.6.1_deep_comprehensive_audit.md`
  Section 5.4, finding NEW-01).
- **Evidence (current code):** `programs/issuer-registry/src/lib.rs`
  at `revoke_issuer_atomic:~1156-1160` and `request_withdrawal_
  atomic:~1313-1316` both explicitly document that the caller MUST
  invoke `update_issuer_tree_root` in the same transaction.  The
  handler does not enforce this.  The SPL AC `replace_leaf` CPI
  atomically moves the tree root; the `IssuerTreeBinding`
  singleton PDA is a separate account and stays at the pre-replace
  root until someone explicitly calls `update_issuer_tree_root`.
- **Impact.**  If a caller -- DAO operator, a compromised key, or a
  buggy SDK -- omits the paired `update_issuer_tree_root`, the
  SPL AC tree has the new (post-revoke or post-cooldown) root but
  `IssuerTreeBinding.current_root` still points at the old root.
  `verify_batch_proof` reads `IssuerTreeBinding.current_root` and
  cross-checks against `public_inputs[ISSUER_TREE_ROOT_INPUT_
  INDEX]`, so a proof generated against the pre-transition root
  continues to verify until the binding is updated.  The nullifier
  universe already shifted (SEC-008 fix binds `issuerTreeRoot` into
  the nullifier preimage), so the holder cannot regenerate the same
  proof, but a cached pre-transition proof is still replayable
  against the stale binding until the operator catches up.  The
  revocation / cooldown is therefore not atomic in practice despite
  the handler name.
- **Remediation (landed 2026-04-27).**
  1. `RevokeIssuerAtomic` and `RequestWithdrawalAtomic` accounts
     structs both gained an `mut` `issuer_tree_binding:
     UncheckedAccount` (seeds `[b"issuer-tree-binding"]`).
  2. After `invoke_signed(replace_leaf, ...)`, the handlers now
     recompute the post-CPI root on-chain by walking the proof
     path with Keccak256 (helper:
     `solid_light::cpi_helpers::compute_concurrent_merkle_root_
     keccak`).  Soundness rests on (a) the CPI's prior validation
     of the path against the pre-CPI root and (b) Keccak256
     pre-image resistance: a malicious caller cannot lie about the
     new root without breaking either invariant.  A pre-CPI gate
     `require!(remaining_accounts.len() == ISSUER_TREE_DEPTH)`
     forbids canopy-shortened paths so the recomputation always
     hits the actual root.
  3. The binding write itself is now a small helper
     `write_issuer_tree_binding_root(data, &new_root, slot)` that
     re-asserts owner-checked discriminator + active status before
     copying `new_root` into `[40..72)` and `slot` into `[72..80)`.
  4. Doc comments at both ixs updated from "caller MUST also call
     `update_issuer_tree_root`" to "this ix replaces
     `IssuerTreeBinding.current_root` atomically".
     `update_issuer_tree_root` remains as an escape hatch for
     `append_issuer_leaf` (where the new root is not derivable
     from inputs without reading SPL AC state) and for
     out-of-band reconciliation; calling it after an atomic ix is
     safely redundant.
- **Regression gate.**  Layered:
  * Host (this session): keccak path-recompute against hand-built
    reference trees (7 cases incl. depth-0 identity, depth-1 left
    vs right, full depth-3 round-trip, high-bit-of-index ignored,
    known-answer depth 2, leaf-swap-changes-root,
    full ISSUER_TREE_DEPTH=16 walk).  Plus binding-write helper
    positive + frozen + bad-discriminator + short-buffer +
    non-active-status cases.
  * Integration (TBD):
    `tests/integration/07b_revoke_atomic_binding_update.test.ts`
    -- invoke `revoke_issuer_atomic` end-to-end against a real
    SPL AC tree and attempt `verify_batch_proof` with the
    pre-revocation proof in the same slot; expect
    `IssuerTreeRootMismatch`.  Pending the bankrun + jest +
    SPL AC fixture work tracked under the integration-suite
    deliverable.
- **Why this was not caught earlier.**  The v0.5 audit and
  ADR-0014 deliberately split replace_leaf from the binding
  update to keep the CPI ix CU-cheap.  The caller-discipline was
  documented but not graded against the "compromised operator or
  buggy SDK omits the second tx" threat model.  The atomicity
  argument is strictly stronger if the binding update lives in
  the same ix.

### SOLID-SEC-046 -- No CU-budget regression gate on `verify_batch_proof`

- **Severity:** MEDIUM
- **Status:** Open
- **Introduced:** 2026-04-22 (zk-verifier initial design; never had
  a CU budget gate).  Surfaced in the v0.6.1 deep audit Section
  5.4, finding NEW-02.
- **Evidence.**  No CI job or runtime measurement of
  `verify_batch_proof`'s CU utilisation.  The compile-time
  assertion `assert!(sz < 3072, "VkBuf exceeds safe BPF stack
  slice")` at `programs/zk-verifier/src/lib.rs:84-87` catches
  stack growth, but does not measure CU.  CU can drift invisibly
  -- a circuit-side constraint that bumps IC point count, a
  public-input addition (SEC-006 Part 2 will do exactly this),
  or a future Solana runtime change in `alt_bn128` syscall costs.
- **Impact.**  Future constraint additions or circuit revisions
  could push `verify_batch_proof` over Solana's per-tx CU ceiling
  (1.4M default; 200K cap on individual pairing via
  `alt_bn128_pairing` before ComputeBudgetProgram::set_limits).
  Failure mode at runtime is
  `Error: Program failed to complete: exceeded maximum number of
  instructions allowed` -- recoverable, but a loud mainnet
  regression rather than a quiet CI catch.  SOLID-SEC-006 Part 2
  is the next near-term trigger; SEC-010 vector expansion and any
  Phase 4 predicate addition also trigger.
- **Remediation (planned).**  Add a CI job
  `verify_batch_proof_cu_baseline` that:
  1. Brings up a fresh `solana-test-validator`.
  2. Runs the full E2E pipeline up through `issue.ts`.
  3. Constructs a single `verify_batch_proof` transaction with
     an explicit `ComputeBudgetProgram::set_compute_unit_limit`.
  4. Inspects the transaction log for `consumed X of Y compute
     units`, parses `X`, and asserts `X <= RECORDED_BASELINE *
     1.10` (10 percent over-budget tolerance).
  5. Re-records the baseline in `docs/CU_BUDGET.md` on sanctioned
     bumps.
- **Regression gate.**  The CI job itself (it is its own gate).
  Initial baseline lands with the first run.  Pair with a
  `docs/CU_BUDGET.md` that enumerates the budget, the expected
  growth from SEC-006 Part 2 and SEC-010, and the triggering
  constants (`NR_PUBLIC_INPUTS`, VK IC count).

### SOLID-SEC-047 -- `verify_batch_proof` Anchor wrapper exceeds BPF 4 KB per-frame stack

- **Severity:** MEDIUM (security exploit class: nil; deploy-blocker
  class: HIGH -- without the fix, `cargo-build-sbf` refuses to link
  `zk_verifier.so`).
- **Status:** Fixed (working tree, **uncommitted as of this
  registry update**).
- **Introduced:** 2026-04-24 (Phase 2 impl 2, commit `73871dc`,
  ADR-0014).  When `NR_PUBLIC_INPUTS` grew from 31 to 32 to make
  room for the new `issuerTreeRoot` slot, the by-value
  `public_inputs: [[u8; 32]; NR_PUBLIC_INPUTS]` ix arg gained 32 B
  -- pushing the Anchor `__global::verify_batch_proof` wrapper's
  frame from "fits-with-margin" to "overflows-by-456-B".  Was
  masked since by the upstream light-poseidon BPF link failure
  (Poseidon's stack-allocated round-constants table aborted the
  link earlier in the same `anchor build`); surfaced once the
  Poseidon refactor lifted that upstream failure on 2026-04-25.
- **Surfaced by:** 2026-04-25 build-pipeline restoration session
  (this registry update).  Discovery method: clean
  `anchor build --program-name schema_registry --no-idl` SUCCEEDED
  (proving the Poseidon fix), but the subsequent
  `--program-name zk_verifier` failed with
  `Stack offset of ... exceeded max offset of 4096 by 456 bytes`
  in the platform-tools linker.
- **Evidence (pre-fix).**
  - `programs/zk-verifier/src/lib.rs:329` ix signature was
    `verify_batch_proof(ctx, public_inputs: [[u8; 32]; NR_PUBLIC_INPUTS], nullifier: [u8; 32])`
    -- 1024 B on the wrapper's stack for the deserialised ix struct
    plus ~1024 B for the BPF outgoing-arg slots when the wrapper
    calls the user fn.
  - `Cargo.toml:42` workspace `[profile.release]` has
    `lto = "fat"`, but **the overflow is independent of LTO**.  We
    verified by toggling `lto = "thin"` -- frame size stayed at
    exactly 4552 B, proving the bloat is from declarative storage
    requirements (struct sizes + BPF calling-convention spill),
    not from optimiser inlining.
  - The compile-time `assert!(size_of::<VkBuf>() < 3072, ...)` at
    `programs/zk-verifier/src/lib.rs:80-87` correctly bounds one
    component (VkBuf) but does NOT bound the function's total
    stack frame, so the regression slipped past every "tests
    green" reporting.
- **Stack accounting (matches the reported 456 B overflow):**
  ```
  __ix struct (deserialized in wrapper)
    proof_a: [u8; 64]  +  proof_b: [u8; 128]  +  proof_c: [u8; 64]
    + public_inputs: [[u8; 32]; 32]  +  nullifier: [u8; 32]
                                                    = 1312 B
  VerifyBatchProof accounts struct (10 accts)        ~  900 B
  ctx + bumps + slice fat-ptr + return slot          ~  200 B
  BPF outgoing-arg slots for the user fn call        ~ 1312 B
  Saved registers + alignment + spill                ~  800 B
                                                     -------
                                                     ~ 4524 B
  -                                  4 KB BPF ceiling -4096 B
                                                     -------
                                            overflow ~  428-456 B
  ```
- **Impact.**  `cargo-build-sbf` refuses to link `zk_verifier.so`
  -- a deploy-blocker, not a runtime exploit.  No security
  regression in deployed code (the previous `zk_verifier.so` was
  never deployed against the post-Phase-2-impl-2 source tree
  because the link never succeeded on a clean machine; the older
  green baselines were against locally-cached older deps that
  resolved to a smaller frame).
- **Remediation (landed in working tree).**  Fix in
  `programs/zk-verifier/src/lib.rs`:
  - Ix arg signature changed:
    `public_inputs: [[u8; 32]; NR_PUBLIC_INPUTS]` ->
    `public_inputs: Vec<[u8; 32]>`.  The 1024 B payload moves from
    the wrapper's stack into the BPF heap allocator (32 KB
    bump-allocator budget); the wrapper stack overhead drops to a
    24-byte `Vec` descriptor.
  - Explicit `require!(public_inputs.len() == NR_PUBLIC_INPUTS,
    ErrorCode::InvalidPublicInputsLength)` immediately on entry
    closes the only new attack surface the change introduces
    (otherwise `Vec` deserialisation is unbounded -- though
    Solana's 1232-byte ix data limit is a practical defence in
    depth).
  - Inner Groth16-verify body extracted into a separate
    `#[inline(never)]` helper at lines ~717-723 that takes
    `public_inputs: &[[u8; 32]; NR_PUBLIC_INPUTS]` by reference.
    The `#[inline(never)]` on the user fn AND on the inner helper
    forces two distinct BPF frames; each gets its own 4 KB
    budget.
  - The audit-called-out **stack-owned `VkBuf`** invariant
    (Section 2.1, item 6 of the v0.6.1 audit) is preserved.  No
    `Box<VkBuf>` introduced; no LTO knob changed.
- **Verification (this session).**
  - `target/deploy/zk_verifier.so` builds cleanly (327 KB ELF
    eBPF, valid magic `7f 45 4c 46 02 01 01 00`).
  - `issuer_registry.so` (684 KB) and `schema_registry.so`
    (332 KB) also build (Poseidon refactor proven for both).
  - `cargo test -p zk-verifier --lib`: 17/17 (existing tests
    use a host-side helper that does not go through the Anchor
    wrapper, so they continue to construct `public_inputs` as
    `[[u8; 32]; 32]` and the host code path is unchanged).
- **Regression gate.**  Two layers:
  1. `tests/integration/verify_batch_proof_rejects_wrong_public_inputs_length.test.ts`
     should be added when the integration suite (02..11) lands;
     it asserts the `InvalidPublicInputsLength` path on a length
     mismatch.
  2. **Stack-frame size CI gate** -- a build-side test that emits
     the actual `verify_batch_proof` BPF frame size and fails
     CI if it's > 3.6 KB (10% margin under 4 KB).  Closes the
     gap the component-level `VkBuf < 3072` assertion does NOT
     cover.  Pairs with **SOLID-SEC-046** (CU-budget gate).
- **SDK / IDL impact.**  Anchor IDL emits the new `Vec<[u8; 32]>`
  type for `public_inputs`; downstream TS SDK encoders must
  prepend the 4-byte Borsh `Vec` length before the 32x32-byte
  payload.  If the SDK uses Anchor's auto-generated client (the
  recommended path), this is handled by IDL regeneration.
  Hand-rolled byte-layout encoders need an audit pass.  Net wire
  delta: +4 bytes per verify ix data blob.  Cross-language
  vectors (`tests/vectors/commitment_and_nullifier.json`) are
  unaffected -- they exercise hashing, not the ix wire format.
- **Companion follow-ups (P0 per v0.6.1 Section 6.1):**
  - Add the stack-frame size CI gate (paired with SOLID-SEC-046's
    CU-baseline gate).
  - Land an integration test that exercises the new
    `InvalidPublicInputsLength` rejection path.
  - Document the `Vec<T>` argument-marshalling pattern in
    `docs/MODULE_CONTRACTS.md` as the canonical answer for any
    on-chain ix whose by-value arg surface approaches BPF's per-
    frame stack ceiling.

### SOLID-SEC-048 -- `register_issuer` BJJ subgroup check exceeds 1.4M CU per-tx ceiling on BPF

- **Severity:** HIGH (security exploit class on mainnet: HIGH if the
  bypass build is deployed there; deploy-blocker class on
  localnet/devnet without the bypass: HIGH -- e2e cannot complete).
- **Status:** Open, with an interim feature-gated bypass live in the
  working tree for localnet/devnet only.  Real fix tracked under this
  ID and as a P0 entry in `docs/IMPROVEMENTS_ROADMAP.md`.
- **Discovered:** 2026-04-25 (this session, while running
  `npm run bootstrap-issuer` against a fresh localnet validator).
- **Relationship to SOLID-SEC-007:** SEC-007 was closed on host-side
  logic correctness: the prime-order subgroup check correctly rejects
  cofactor-8 torsion points, the Edwards neutral element, and
  off-curve points (47 host-side test cases pass).  SEC-048 is the
  BPF-runtime extension of that gate -- the same logic, structurally
  not affordable at on-chain compute prices.  The handler-level
  enforcement point is therefore not currently sound on BPF without
  the off-chain SDK acting as the load-bearing predicate.
- **Trust assumption while bypass is active:** every off-chain caller
  of `register_issuer` MUST run `isInPrimeOrderSubgroup(pkX, pkY)`
  pre-submit (the canonical reference implementation lives in
  `ts-sdk/packages/core/src/index.ts` and routes through
  `solid_core::babyjubjub::is_in_prime_order_subgroup` on host via
  the WASM bridge).  A caller who skips the predicate and submits a
  cofactor-8 torsion pubkey can register an issuer whose downstream
  EdDSA-style signatures verify only over the 8-element subgroup --
  i.e., the security parameter for credentials issued by that key
  drops from 251 bits to 3 bits.  The `is_on_curve + !is_identity`
  consolation gate on-chain catches off-curve garbage and the
  identity element, but does NOT catch this attack class.
- **Why audit didn't catch this earlier.**  B3 (light-poseidon BPF
  refactor) explicitly notes the subgroup helpers were
  "intentionally NOT gated, kept dual-target."  That is true: both
  targets compile and link.  What was not measured is the runtime
  BPF compute cost per call.  The omission is that the
  dual-target-link gate is not the same as a dual-target-runtime
  gate, and there was no on-chain test exercising
  `register_issuer` against a real validator with default compute
  budgets.  See **regression gate** below for the closing test.
- **Evidence.**
  - Failure signature: BPF program log:
    `Program 5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx
     consumed 1400000 of 1400000 compute units` followed by
    `Program failed to complete: exceeded CUs meter at BPF
     instruction`.
  - Reproduces with `ComputeBudgetProgram.setComputeUnitLimit({
    units: 1_400_000 })` (the per-tx ceiling) -- raising the budget
    is not an option, the cost is structural.
  - Build flags tried: toggling `lto = "thin"` / `"fat"`, varying
    `codegen-units`, `opt-level = "z"` / `3`, all left the cost
    pegged at the ceiling.
  - Cost decomposition: `EdwardsAffine::mul_bigint` over a ~251-bit
    scalar = ~251 doublings + ~125 conditional additions through
    arkworks' BPF-portable backend.  Each doubling is ~5K CU
    (modular arithmetic over a 254-bit prime field on BPF), so the
    aggregate is ~1.6-2.0M CU before the comparison.  No individual
    arkworks operation is anomalously expensive; the structural
    251-bit width is the hard wall.  Solana's alt_bn128 syscall
    family (used by `verify_batch_proof`) is BN254 G1/G2, a
    different curve, and does not help here.
- **Interim remediation (working tree, 2026-04-25; NOT FOR MAINNET).**
  - `programs/issuer-registry/Cargo.toml` -- new Cargo feature
    `sec007-skip-onchain` (no default; `[features]` block carries an
    in-file warning that mainnet builds MUST NOT set it).
  - `programs/issuer-registry/src/lib.rs:189` --
    `solid_core::babyjubjub::require_in_prime_order_subgroup(&bjj_pub_key)`
    is gated behind `#[cfg(not(feature = "sec007-skip-onchain"))]`.
    With the feature set, the handler runs the consolation gate
    (`is_on_curve + !is_identity`), emits
    `msg!("SEC-048: sec007-skip-onchain active; off-chain SDK
     predicate is enforcement point")`, and emits a structured
    `Sec007Bypass { issuer_authority, slot }` event so off-chain
    monitors can detect a bypass binary on a cluster it shouldn't
    be on.
  - `crates/solid-core/src/babyjubjub.rs` -- new public helpers
    `is_on_curve(&BJJPublicKey) -> bool` and
    `is_identity(&BJJPublicKey) -> bool` for the consolation gate.
  - `ts-sdk/packages/core/src/index.ts` -- `isInPrimeOrderSubgroup`
    re-exported as the canonical client-side predicate, with
    extensive doc-comments naming itself the load-bearing gate
    while the on-chain check is bypassed.
  - `scripts/bootstrap_issuer.ts` -- pre-submit gate now invokes the
    new export and aborts the script before `register_issuer` is
    even built if the generated key fails the check.  Compute
    budget on the on-chain ix reduced from 1.4M to 400K CU
    (measured ~120K CU steady-state with the bypass; 400K leaves
    headroom for `init`, the `system_program::transfer` CPI, and
    Clock syscalls on noisy validators).
- **Build / deploy invocation (localnet/devnet only):**
  ```
  CARGO_TARGET_DIR=$PWD/target cargo build-sbf -p issuer-registry \
    --features sec007-skip-onchain
  solana program deploy target/deploy/issuer_registry.so \
    --program-id keys/localnet/issuer_registry-keypair.json
  ```
  `check_program_ids.py` is unaffected (program ID unchanged).  IDL
  hash is unaffected (pure feature gating, no schema change).
- **Real fix candidates (in increasing soundness preference):**
  1. **Cofactor-clear in the issuer SDK.**  Multiply candidate
     pubkey by `8` (the cofactor) off-chain before submission; check
     the result is non-identity.  On-chain stays at the
     `is_on_curve + !is_identity` consolation gate.  Cheapest, but
     trusts the off-chain SDK to do the multiplication; a malicious
     caller can skip it and the bypass is invisible until the
     issuer's first signed credential fails to verify in-circuit.
  2. **Move the subgroup gate into the issuance circuit.**  Every
     `issue_credential` proof commits to the issuer's pubkey; adding
     a `is_in_prime_order_subgroup` constraint there makes the gate
     enforcement a circuit-level invariant rather than a handler-
     level one.  Costs ~30K extra constraints (one EdDSA-style
     scalar mul) and shifts ZK proving cost up correspondingly.
     Strongest soundness binding; slowest to ship (requires trusted
     setup re-run, batches with SEC-006 Part 2).
  3. **Solana BJJ syscall.**  Propose adding `sol_babyjubjub_*`
     syscalls upstream so on-chain code can do subgroup /
     scalar-mul checks at curve speed (a few thousand CU).  Multi-
     quarter timeline; depends on validator buy-in.
- **Mainnet deploy-blocker mechanics:**
  - `programs/issuer-registry/Cargo.toml` carries an in-file
    "NOT FOR MAINNET" warning on the `sec007-skip-onchain` feature.
  - SEC-048 closeout includes a CI gate that rejects mainnet release
    builds setting the feature (added to `scripts/check_program_ids.py`
    or a sibling `scripts/check_release_features.py`).
  - Production cluster monitor: alert (or hard-fail rollout) on any
    `Sec007Bypass` event observed in mainnet logs -- the event is
    deliberately structured for index-time matching.
- **Regression gate.**  Two layers:
  1. Add `tests/integration/register_issuer_compute_units.test.ts`
     (E2E suite) that lands a `register_issuer` against a real
     validator with the default 200K CU budget (no
     `setComputeUnitLimit` override) and asserts the bypass build
     succeeds; against the no-feature build it asserts the
     `exceeded CUs meter` failure mode.  Closes the dual-target-link
     vs dual-target-runtime gap that let SEC-048 slip past SEC-007's
     verification.
  2. Add a CI step that runs
     `cargo build-sbf -p issuer-registry --release --no-default-features`
     (the mainnet shape) and re-runs the same on-chain regression
     test against a localnet started with that binary.  The test
     MUST fail on the no-feature build until the real fix lands;
     when it passes, SEC-048 closes.
- **Tracking cross-references.**
  - Build-pipeline tracker: `docs/E2E_BLOCKERS.md` B9 (interim
    bypass) and O7 (registry pointer).
  - Roadmap entry: `docs/IMPROVEMENTS_ROADMAP.md` (P0 SEC-048).
  - Top-of-mind blockers: `plan/RESUME.md`,
    `docs/FORWARD_ROADMAP.md`.
  - Audit trail: SOLID-SEC-007 was closed on host-logic
    correctness; SEC-048 extends it to BPF-runtime feasibility
    and is the active line until closed.

### SOLID-SEC-050 -- Schema-ordering canonicality bypass via interleaved padding

- **Severity:** MEDIUM
- **Status:** Fixed (2026-04-27, this session).  Closure receipts:
  - Constraint added at
    `circuits/batch_credential_query.circom:225-241` -- one extra
    R1CS constraint per consecutive pair: `isZero[i].out *
    (1 - isZero[i+1].out) === 0`.
  - 17-case witness-tester regression at
    `circuits/test/schema_ordering.test.js` covers (a) every
    canonical layout, (b) every non-canonical interleave the
    pre-fix circuit admitted, (c) ascending-pair breaks,
    (d) zero-schema integrity, (e) a property-test
    enumeration that proves only the canonical placement is
    accepted for every K-of-NUM_CREDS active subset.
  - Circuit recompiles clean; trusted-setup re-run committed
    new `verification_key.sha256` (recorded in
    `circuits/build/verification_key.sha256` after re-run; the
    pin file consumed by `scripts/initialize.ts` remains the
    SOLID-SEC-041 gate).
  - 39/39 circuit tests pass.
- **Discovered:** 2026-04-27 (this session) during a manual
  audit of `batch_credential_query.circom` STEP 0 ordering
  logic.
- **Evidence (pre-fix code):** `circuits/batch_credential_query.
  circom:214-223` -- the strict-ascending check was gated on
  `orderingNextNotZero[i] = 1 - isZero[i+1].out`, so any pair
  where the NEXT slot was inactive (schemaHash == 0) silently
  skipped the comparator.  An interleaving like `[A1, 0, A2, A3]`
  passed because (i=0) had next=0 (skipped), (i=1) had next=A2
  active (0 < A2 trivially holds), (i=2) had next=A3 (A2 < A3).
  No constraint linked A1 to A2 or A3.
- **Impact.**  A holder with K credentials could encode the same
  set in multiple distinct ways:
  ```
  K=2, schemas=(5, 10):
     [5, 10, 0, 0]   <- canonical
     [5, 0, 10, 0]   <- pre-fix admitted
     [0, 5, 0, 10]   <- pre-fix admitted (with all-padding-prefix)
     [5, 10, 0, 0]   etc.
  ```
  Each encoding lands the credentials at different array
  indices.  `queryCredentialIndices[i]` -- a public input the
  prover writes to point a predicate at a specific slot -- now
  carries a different value, so `qHasherIndices = Poseidon8(
  cred[0], field[0], cred[1], field[1], ...)` differs across
  encodings, `queryContextHash = Poseidon4(qHasherIndices,
  qHasherOps, numPredicates, compoundLogic)` differs, and
  `nullifier = Poseidon6(masterKey, revNonce, verifierAddr,
  queryContextHash, verifierNonce, issuerTreeRoot)` differs.
  The verifier's per-claim rate-limit (one nullifier-PDA per
  proof, asserted by Anchor's `init` constraint) was therefore
  bypassable: the holder could submit N copies of the "same"
  logical proof with N distinct nullifiers, all valid.
- **Threat model.**  Defeats verifier policies of the form "one
  proof per holder per query semantics".  Examples: per-account
  rate limits, sybil-resistance for credentials gating airdrops,
  voting weight ("one vote per holder" via nullifier).  Does NOT
  enable credential forgery (the per-active-slot integrity
  constraints + EdDSA + Merkle inclusion still hold for every
  active credential), but it does enable claim multiplication
  beyond what the protocol was designed to allow.
- **Remediation (landed).**  Padding canonicality:
  ```
  isZero[i].out * (1 - isZero[i+1].out) === 0
  ```
  applied to every consecutive pair `(i, i+1)`.  Reads as: "if
  slot `i` is inactive, slot `i+1` MUST also be inactive".
  Combined with the existing strict-ascending check on
  active->active pairs, this forces the canonical layout
  `[A1 < A2 < ... < AK, 0, 0, ..., 0]` -- exactly one valid
  encoding per credential set (modulo the prover's choice of
  which credentials to include).
- **Why this was not caught earlier.**  Phase 3.6 hardening
  added strict-ascending on active->active pairs, intended for
  canonicality.  The "next is inactive -> skip" guard was a
  legitimate part of the design (so [A, 0, 0, 0] is allowed),
  but the symmetric "current is inactive -> next must also be
  inactive" wasn't surfaced as a separate canonicality
  invariant in the audit text -- the prior audit folded it under
  "schemas strictly ascending for active credentials" without
  explicitly enumerating the interleave class.  The new
  property-test in `schema_ordering.test.js` is the source-level
  regression gate; any future weakening of either constraint
  fails CI immediately.
- **Pairs with SOLID-SEC-051** (separate, LOW): the same audit
  pass surfaced an "all-padding admitted" finding (no constraint
  enforces at least one active credential).  SEC-050 closes the
  reshuffling axis; SEC-051 closes the empty-batch axis.  Both
  fixes share a trusted-setup cycle if landed together.
- **Trusted-setup impact.**  Circuit revision changes the R1CS
  (constraint count up by 3, from 86,616 to 86,619; wires
  92,430 vs. the pre-fix value).  New `verification_key.json`
  + `verification_key.sha256` written by `node scripts/setup.js`;
  the SEC-041 pin updates downstream.  No on-chain handler
  changes required (the verifier already consumes
  `schemaHashes` from public_inputs via the existing
  registry-lookup flow).

### SOLID-SEC-051 -- All-padding `[0,0,0,0]` proofs admitted

- **Severity:** LOW (most realistic verifier queries fail on
  zero data; corner case for unusual queries; not a credential-
  forgery class issue).
- **Status:** Open.  Single-constraint fix prepared:
  `IsZero(schemaHashes[0]).out === 0`.  Defer-with-justification
  is acceptable -- the on-chain handler's behaviour is described
  below for full context, but the circuit-level invariant
  ("at least 1 active credential per proof") is not currently
  enforced and could surprise a future verifier integration that
  relied on it.
- **Discovered:** 2026-04-27 alongside SOLID-SEC-050.
- **Evidence.**  `programs/zk-verifier/src/lib.rs:500-502`
  explicitly skips all schema-tree-binding validation for any
  slot where `merkle_root == [0u8; 32] && schema_hash == [0u8;
  32]`.  The circuit accepts `schemaHashes = [0, 0, 0, 0]`
  because every per-credential signal is forced to zero by the
  STEP 0 IsZero pattern, the IdentityAnchor and CredentialAtom
  enabled flags both gate to 0, and the predicate-evaluation
  layer happily evaluates against zero data.
- **Impact (concrete).**  An attacker holding zero credentials
  can satisfy any verifier query whose predicate evaluates true
  on zero inputs -- e.g. `EQ 0` on field 0 (asks "is the value
  0?"), or `GTE 0` on any field.  Realistic verifier queries
  ("age GT 21", "country EQ 'US'") fail because the data is
  forced to zero and the predicate returns 0.  The risk is in
  unusual / poorly-specified queries, OR in any future verifier
  integration that interprets a successful proof as "the holder
  has at least one credential" (a guarantee the circuit does
  NOT actually provide today).
- **Remediation (proposed, defer-with-justification).**
  Add `component anySchemaSet = IsZero(); anySchemaSet.in
  <== schemaHashes[0]; anySchemaSet.out === 0;` as the first
  STEP 0 constraint.  Combined with SOLID-SEC-050's
  padding-at-end, this enforces: at least one active slot at
  position 0, plus a sorted active prefix, plus padding tail.
- **Why fixing now is deferred.**  This session lands SEC-050
  in a single trusted-setup cycle.  Bundling SEC-051 means
  killing and re-running the in-progress ceremony, which would
  waste the powers-of-tau already produced.  The next sanctioned
  trusted-setup cycle (the one that pairs with SEC-006 Part 2
  per `docs/IMPROVEMENTS_ROADMAP.md`) is the correct landing
  pad: SEC-051 + SEC-006 Part 2 + the predicate-operand
  range-checks (a separate Phase-4 TODO at
  `circuits/lib/predicate_evaluator.circom:20-40`) all rebase
  onto the new VK in one go.
- **Workaround during the open window.**  Verifier integrations
  that rely on "at least one credential held" should hash a
  per-verifier salt into queryValues / queryContextHash that is
  guaranteed non-zero, OR check `schemaHashes[0] != 0` off-chain
  before accepting the proof.  Document this in
  `docs/integration-guide.md` alongside SEC-051 closure.
- **Tracking.**  Will close together with SEC-006 Part 2 in the
  next trusted-setup cycle.

### SOLID-SEC-052 -- BPF / cross-layer coord-form drift in BJJ pubkey path

- **Severity:** HIGH (functional break of `register_issuer` on BPF
  for any honest WASM-generated keypair; bypassed every operator
  trying to deploy post-cff06c2 source until the WASM bridge was
  rebuilt).
- **Status:** Fixed (partial) 2026-04-28.  The two surfaces that
  block `bootstrap_issuer.ts` are closed; an outstanding cross-layer
  drift is tracked separately as the EdDSA witness debug (no
  registry entry yet -- WIP).
- **Discovered:** 2026-04-28 e2e bring-up session, via the
  `register_issuer` `InvalidBJJPubKey` error firing on every
  freshly-generated keypair under the `sec007-skip-onchain` build.
- **Root cause(s).**
  1. The post-cff06c2 (2026-04-27 "circuit update") rewrite of
     `crates/solid-core/src/babyjubjub.rs` introduced the
     circomlib<->arkworks coordinate-form iso (`x_ark = sqrt(a) *
     x_circ`) and routed `is_on_curve` / `is_identity` through
     `EdwardsAffine::new_unchecked(x_circ_to_ark(x_circ), y).
     is_on_curve()`.  On host x86 / WASM this evaluates correctly;
     on the Solana BPF target it consistently rejects valid
     keypairs that the same code path accepts on host.  The
     symptom is "off-chain `isInPrimeOrderSubgroup` says yes,
     on-chain `is_on_curve` says no" for the SAME bytes.
  2. The WASM bridge artifact at
     `ts-sdk/packages/core/wasm/solid_wasm_bg.wasm` was last built
     2026-04-25 21:59, BEFORE cff06c2.  Pre-cff06c2 the
     `affine_to_pubkey` helper returned arkworks-form bytes;
     post-cff06c2 it returns circomlib-native bytes.  An operator
     who pulled `main` and rebuilt only the on-chain `.so`
     artifacts ended up with off-chain (WASM) producing
     arkworks-form pubkey bytes and on-chain code expecting
     circomlib-native -- the two sides diverged on every wire byte
     `BJJPublicKey` carried.  This was the actual dominant cause
     of the symptom; `is_on_curve` was secondary.
- **Why audit didn't catch this earlier.**  cff06c2 landed alongside
  Phase 3.4 / 3.6 circuit work whose regression gate was
  `cargo test -p solid-core --lib babyjubjub` (host-only) and
  `cd circuits && npm test` (witness-tester, host-only).  Both
  pass.  Neither exercises the **on-chain BPF** runtime nor the
  **off-chain WASM** runtime against a real keypair flow; both
  branches independently work, but their wire contract was never
  cross-validated end-to-end.  The implicit assumption "host tests
  green => BPF + WASM agree" failed here.  Same gap pattern as
  SOLID-SEC-048 ("dual-target-link != dual-target-runtime").
- **Remediation (landed 2026-04-28).**
  - `crates/solid-core/src/babyjubjub.rs::is_on_curve` rewritten
    to evaluate the circomlib-native twisted-Edwards equation
    directly:
    ```
    a * x^2 + y^2 == 1 + d * x^2 * y^2     (a = 168700, d = 168696)
    ```
    Pure `Fq * Fq` and `Fq + Fq` operations; no
    `EdwardsAffine::new_unchecked`, no iso transform, no
    `is_on_curve()` call into arkworks.  Behaviour identical on
    host and BPF.  Same treatment for `is_identity` (circomlib
    neutral element is `(0, 1)`; equality check on field
    elements).
  - WASM bridge rebuilt:
    `wasm-pack build wasm/ --target nodejs --out-dir
    ts-sdk/packages/core/wasm --release` -> emits a
    post-cff06c2 binary that produces circomlib-native bytes,
    matching on-chain expectation.
  - Host regression gate added at
    `crates/solid-core/src/babyjubjub.rs::tests::test_keygen_passes_on_chain_consolation_gate`
    (16 random keypairs round-trip through `is_on_curve` +
    `!is_identity` + `is_in_prime_order_subgroup` and assert all
    accept).  The pre-fix code path also passes this on host;
    the test exists as a stable contract for the new
    implementation to defend against future drift.
  - Process gate (CLAUDE.md / runbook): documented requirement
    that `wasm-pack build wasm/` MUST run on any commit that
    touches `crates/solid-core/src/babyjubjub.rs` byte-format
    helpers (`affine_to_pubkey`, `pubkey_to_affine`,
    `affine_to_circomlib_xy`, the SQRT_A_LE / BASE8_X_ARK_LE
    constants).  See the runbook update in
    `docs/E2E_BLOCKERS.md` B11.
- **Outstanding** (tracked as "EdDSA witness drift" follow-up,
  pending more debug).  After SEC-052 a/b were fixed,
  `bootstrap_issuer.ts` + `issue.ts` complete green but
  `prove.ts` rejects the issuer's EdDSA signature inside the
  `CredentialAtom`'s `EdDSAPoseidonVerifier` (witness-gen
  failure at `ForceEqualIfEnabled_324:56`).  Most likely a
  remaining contract drift between off-chain `sign()` byte
  output and the circuit's `EdDSAPoseidonVerifier` expectation
  (R8 coord form, message-hashing convention, or the
  off-chain-vs-circuit holder-pubkey derivation under the new
  `BabyPbk254`).  Will be promoted to a registry entry once
  diagnosed.
- **Tracking.**  Session log in `plan/RESUME.md` (today).  Cross-
  references: SEC-048 (dual-target gap), SEC-007 (subgroup check),
  cff06c2 commit ("circuit update").

### SOLID-SEC-049 -- Wrong `SPL_AC_REPLACE_LEAF_DISCRIMINATOR` -- atomic ixs would have failed at the CPI

- **Severity:** HIGH (functional break of every atomic revocation
  path; no on-chain state reachable through the broken ixs, so no
  data-integrity exposure -- but the entire SOLID-SEC-044 / -045
  control surface was non-functional in production).
- **Status:** Fixed (2026-04-27, this session).
- **Discovered:** 2026-04-27, surfaced by an explicit derivation
  test added to `programs/issuer-registry/src/lib.rs::tests`
  while landing the SOLID-SEC-045 fix.  The test computes
  `sha256("global:replace_leaf")[..8]` and compares against the
  hardcoded `SPL_AC_REPLACE_LEAF_DISCRIMINATOR` constant.  The
  constant was `[0xe3, 0x88, 0x6a, 0x74, 0x10, 0xe4, 0xe8, 0x2c]`;
  the canonical preimage gives `[0xcc, 0xa5, 0x4c, 0x64, 0x49,
  0x93, 0x00, 0x80]`.  The mismatched byte sequence does not match
  any candidate preimage tested
  (`global:replace_leaf`, `global:append`, `global:set_leaf`,
  `replace_leaf`, `spl_account_compression:replace_leaf`,
  `global:transfer_authority`, `global:verify_leaf`,
  `global:close_empty_tree`, `global:insert_or_append`,
  `global:init_empty_merkle_tree`).  The constant was apparently
  invented out of band and never sanity-checked.
- **Evidence (pre-fix code):** the constant lived at
  `programs/issuer-registry/src/lib.rs:53-54` and was used in
  `revoke_issuer_atomic` (line ~1392) and
  `request_withdrawal_atomic` (line ~1604) inside the
  hand-rolled `replace_leaf` CPI's data buffer.  The doc comment
  above the constant claimed it was
  `sha256("global:replace_leaf")[..8]` -- a doc lie (CLAUDE.md
  L6) that was never enforced.
- **Impact.**  Both atomic ixs are the *only* sound path from
  `Approved` to `Revoked` / `Cooldown` for an issuer who has
  been enrolled into the issuer tree (legacy `revoke_issuer` and
  `request_withdrawal` REFUSE for `is_tree_enrolled = true`).
  With the wrong discriminator the SPL AC program would have
  returned `InstructionFallbackNotFound` on the very first call;
  the wrapper handler never wrote any state because it returns
  on `?`-propagated CPI errors before the registry bookkeeping +
  binding update.  Net effect: the working ADR-0014 atomicity
  story was non-functional in production, AND the SOLID-SEC-044
  Phase 3 closeout claim was structurally incorrect.
- **Latent because.**  No integration test had ever exercised
  either atomic ix.  Unit tests only covered VK / Poseidon
  primitives.  `npm run e2e` walks through
  `register_issuer` -> `vote_on_issuer` -> `finalize_voting` ->
  `append_issuer_leaf` -> `issue` -> `prove`; revoke /
  cooldown were not on the happy-path script and were never
  reached.  A first integration test on `revoke_issuer_atomic`
  (the missing test 07b in the v0.6.1 audit's Section 5.4
  remediation list) would have caught this on day one.
- **Remediation (landed this session).**
  - `programs/issuer-registry/src/lib.rs:53-67` -- constant
    replaced with `[0xcc, 0xa5, 0x4c, 0x64, 0x49, 0x93, 0x00,
    0x80]` plus an extended doc-comment naming SEC-049 and
    pointing at the regression gate.
  - Same file's test module: two tests
    (`replace_leaf_discriminator_matches_anchor_global_
    replace_leaf` + `append_discriminator_matches_anchor_global_
    append`) compute `sha256("global:<name>")[..8]` from the
    canonical preimage at test time and compare against the
    constants.  Any future Anchor / SPL AC namespace change OR
    any silent typo on the constants fails CI.
- **Regression gate.**  The two host-side discriminator tests
  above; plus the same SPL-AC-integration test 07b once
  the bankrun + jest harness lands -- a wrong discriminator now
  means the integration test fails at the CPI rather than
  hiding behind the missing binding-update gap that 07b was
  originally designed to surface.
- **Why this was not caught earlier.**  Two compounding gaps:
  (a) the only validation of the constant was a doc-comment
  claim that was never machine-checked; (b) no integration
  pathway exercised the dependent ixs, so the runtime failure
  signal was absent.  The fix closes both: a unit-test gate that
  recomputes from the canonical preimage, and a registry note
  that the integration test for revoke / cooldown is now
  load-bearing for SEC-044, SEC-045, AND SEC-049.

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
| 2026-04-25 | Phase 3 impl 4 (ADR-0014 Cooldown-verify-negative amendment)         | --                                        | SOLID-SEC-044 (request_withdrawal_atomic; legacy request_withdrawal gated on !is_tree_enrolled) |
| 2026-04-25 | `sec/audits/2026-04-25_v0.6.1_deep_comprehensive_audit.md` (post Phase-3-impl-4 snapshot) | SOLID-SEC-045, -046               | 0 (both introduced Open)                          |
| 2026-04-25 | Build-pipeline restoration session (circuit compile fixes, dep-cascade resolution, Poseidon BPF refactor, `verify_batch_proof` frame fix; `docs/E2E_BLOCKERS.md` tracker created) | SOLID-SEC-047                       | SOLID-SEC-047 (registered Fixed in working tree; `target/deploy/zk_verifier.so` builds cleanly post-fix) |
| 2026-04-25 | E2E unblock session (B6/B7 closed; `npm run e2e` walked through `initialize` -> `backfill-issuer-tree` -> `bootstrap-issuer`; SEC-007 BPF CU exhaustion surfaced on `register_issuer` and a feature-gated bypass landed in working tree; documented as B9 in E2E_BLOCKERS) | SOLID-SEC-048                       | 0 (introduced Open with interim bypass live; localnet/devnet only) |
| 2026-04-27 | Aggressive program-test sweep + SEC-045 atomic-binding closure session (this session)                              | SOLID-SEC-049                             | SOLID-SEC-045 (fixed via on-chain Keccak path-recompute + atomic binding write); SOLID-SEC-049 introduced AND fixed in the same session via the discriminator-derivation regression test |
| 2026-04-27 | Circuit + ZK audit pass (this session, continuation)                                                                | SOLID-SEC-050, SOLID-SEC-051              | SOLID-SEC-050 (fixed; padding-canonicality constraint + 17-case witness-tester regression; trusted-setup re-run; new VK pin); SOLID-SEC-051 introduced Open (deferred to next trusted-setup cycle bundled with SEC-006 Part 2 + the predicate-operand range checks); also closed `docs/E2E_BLOCKERS.md` O4 (padding_slot test verified passing under Phase 3.4 BabyPbk254) |

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
