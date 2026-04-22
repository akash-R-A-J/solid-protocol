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
| CRITICAL  | 3    | 0           | 0     | 0        | 0         | 3     |
| HIGH      | 12   | 0           | 0     | 0        | 0         | 12    |
| MEDIUM    | 13   | 0           | 0     | 0        | 0         | 13    |
| LOW       | 5    | 0           | 0     | 0        | 0         | 5     |
| INFO      | 5    | 0           | 0     | 0        | 0         | 5     |
| **Total** | 38   | 0           | 0     | 0        | 0         | 38    |

---

## Status board

| ID              | Severity | Status | Title                                                              |
|-----------------|----------|--------|--------------------------------------------------------------------|
| SOLID-SEC-001   | CRITICAL | Open   | Batch circuit: `queryCredentialIndices` / `queryFieldIndices` unconstrained |
| SOLID-SEC-002   | CRITICAL | Open   | `register_schema` Poseidon integrity check commented out           |
| SOLID-SEC-003   | CRITICAL | Open   | `issue_credential` missing schema + tree pubkey binding            |
| SOLID-SEC-004   | HIGH     | Open   | No in-circuit issuer pubkey binding; revoked issuers still verify  |
| SOLID-SEC-005   | HIGH     | Open   | `currentTimestamp` public input not bound to `Clock`               |
| SOLID-SEC-006   | HIGH     | Open   | VK overwrite at chunk 0 has no freeze-gate; truncated VK finalizable|
| SOLID-SEC-007   | HIGH     | Open   | BJJ public keys not subgroup-checked at registration               |
| SOLID-SEC-008   | HIGH     | Open   | Nullifier does not include epoch / global root                     |
| SOLID-SEC-009   | HIGH     | Open   | WASM bridge fractured across 3 locations                           |
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
| SOLID-SEC-020   | MEDIUM   | Open   | E2E scripts persist plaintext issuer + holder secrets              |
| SOLID-SEC-021   | MEDIUM   | Open   | Depth-20 circuit caps global tree at ~250K holders                 |
| SOLID-SEC-022   | LOW      | Open   | Local `IsZero` reimplementation in `credential_atom.circom` (also NEW-SEC-09 in master audit) |
| SOLID-SEC-023   | LOW      | Open   | `active_issuers` counter drifts on Cooldown -> Revoked path        |
| SOLID-SEC-024   | LOW      | Open   | `unstake_tokens` uses raw `-=` instead of `checked_sub`            |
| SOLID-SEC-025   | INFO     | Open   | `CheckIssuerStatus` ungated and never called on-chain              |
| SOLID-SEC-026   | INFO     | Open   | `Credential::verify_integrity` never called on-chain               |
| SOLID-SEC-027   | INFO     | Open   | `docs/IMPROVEMENTS_ROADMAP.md` has stale unticked checkboxes       |
| SOLID-SEC-028   | MEDIUM   | Open   | `CLAUDE.md` and test README document wrong WASM build path         |
| SOLID-SEC-029   | MEDIUM   | Open   | `IdentityAnchor` always has `enabled=1`; padding slots over-constrained |
| SOLID-SEC-030   | MEDIUM   | Open   | `transfer_slashed_lamports` can drain `stake_vault` to zero        |
| SOLID-SEC-031   | HIGH     | Open   | `bufToDecimal` LE interpretation of Solana pubkey risks breaking `verifierAddress` match |
| SOLID-SEC-032   | HIGH     | Open   | `SCHEMA_REGISTRY_ID_BYTES` hardcoded without build-time validation |
| SOLID-SEC-033   | HIGH     | Open   | Identity cohesion check compares master pubkey; circuit uses per-schema derived (E2E blocker) |
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
- **Status:** Open
- **Introduced:** 2026-04-22
- **Evidence:** `programs/schema-registry/src/lib.rs:114-130`
- **Description.** `require!(computed_hash == schema_hash)` is
  commented out. Anyone can register a schema whose metadata does not
  match the declared hash.
- **Impact.** Root enabler for SOLID-SEC-003. SEC-06 in the
  remediation audit is not actually closed.
- **Remediation.** Re-enable using `light-poseidon` (~170K CU).
- **Regression gate.** Unit test: mismatched hash returns
  `ErrorCode::InvalidSchemaHash`.

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
- **Status:** Open
- **Introduced:** 2026-04-22
- **Evidence:** `programs/zk-verifier/src/lib.rs:157-198`;
  `circuits/batch_credential_query.circom:65,192-198`
- **Description.** `public_inputs[30]` carries `currentTimestamp`.
  Circuit enforces `currentTimestamp <= expirationTimestamp` per
  credential. On-chain verifier never compares
  `public_inputs[30]` to `Clock::get()?`.
- **Impact.** Attacker supplies `currentTimestamp = 0`; expired
  credentials verify.
- **Remediation.** Bind with configurable skew (10 min) stored in
  `VerifierConfig`, not hardcoded.
- **Regression gate.** Integration tests
  `10_verify_expired_credential_rejected`,
  `11_verify_future_timestamp_rejected`.

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
- **Status:** Open
- **Introduced:** 2026-04-22
- **Evidence:** `crates/solid-core/Cargo.toml:10`; `wasm/src/lib.rs`;
  `.github/workflows/ci.yml:196-197,256-257,301-303`;
  `ts-sdk/packages/core/package.json`
- **Description.** `Cargo.toml` declares a `wasm` feature in
  `solid-core` with zero `#[wasm_bindgen]` exports; the real bridge
  lives in `wasm/`; CI builds from `crates/solid-core`; SDK imports
  from `../../../wasm/pkg` which neither CI nor any script creates.
  See SOLID-SEC-028 for the documentation variant of this defect.
- **Impact.** Silent drift between Rust primitives and TS SDK
  consumers.
- **Remediation.** Keep `wasm/` standalone (solid-core must remain
  BPF-compatible). Fix CI + SDK pinned path.
- **Regression gate.** CI job `wasm_bridge_smoke`.

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
- **Status:** Open
- **Evidence:** `scripts/issue.ts:118-125`; `.gitignore`
- **Remediation.** Move state to `/tmp` or encrypted keystore; add to
  `.gitignore`; pre-commit hook rejects BJJ-key magic bytes.
- **Regression gate.** Hook test.

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
- **Status:** Open
- **Evidence:** `docs/IMPROVEMENTS_ROADMAP.md` (37 items all `[ ]`)
- **Remediation.** Flip closed items. `scripts/check_docs.py`
  enforces agreement with this registry.

---

### Findings introduced by the 2026-04-22 v0.4 audit pass

### SOLID-SEC-028 -- CLAUDE.md and test README document wrong WASM build path

- **Severity:** MEDIUM
- **Status:** Open
- **Introduced:** 2026-04-22 (v0.4 audit)
- **Evidence:**
  - `CLAUDE.md:49-50` (wrong build command: `crates/solid-core`)
  - `tests/integration/README.md:19-20` (same wrong command)
  - Real bridge: `wasm/src/lib.rs` (305 lines, exports present)
  - `crates/solid-core/src/` has zero `#[wasm_bindgen]` exports
- **Description.** The real WASM bridge at `wasm/src/lib.rs` is
  complete and production-quality. However, CLAUDE.md and the test
  README both document `wasm-pack build crates/solid-core` as the
  build command, which produces an empty pkg. Any developer
  following the documented sequence gets a broken SDK with no error
  message. Combined with SOLID-SEC-009 (the CI/SDK fracture), both
  must be fixed for E2E to work.
- **Impact.** Every new contributor and every CI run that follows
  the docs produces a non-functional WASM layer. Silent failure.
- **Remediation.** Change both files to:
  `wasm-pack build wasm/ --target nodejs --out-dir ts-sdk/packages/core/wasm --release`
- **Regression gate.** `wasm_bridge_smoke` CI step: build pkg from
  `wasm/`, call `computeHardenedNullifier`, assert non-zero.
- **Blocks:** SOLID-SEC-009, SOLID-SEC-010, all E2E. Close in Phase 1.

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
- **Status:** Open
- **Introduced:** 2026-04-22 (v0.4 audit; folds in master-audit BUG-NEW-03)
- **Evidence:** `programs/issuer-registry/src/lib.rs:1192-1206`
- **Description.** `transfer_slashed_lamports` uses raw lamport
  manipulation to move funds from the shared
  `PDA([b"stake-vault"])` to `dao_treasury`. When
  `slash_amount == vault_lamports`, the vault drops to 0 lamports.
  A 0-lamport account not explicitly closed is garbage-collected by
  the Solana runtime. This permanently destroys the shared stake
  vault PDA; future `register_issuer` SOL deposits fail with
  "account not found". This is the acute form of SOLID-SEC-014
  (single shared vault); per-issuer vaults in Phase 2 close the
  structural concern.
- **Impact.** Triggered when the last remaining issuer's entire
  stake is slashed. Destroys DAO staking infrastructure.
- **Remediation.**
  ```rust
  let min_bal = Rent::get()?.minimum_balance(0);
  require!(
      vault_lamports.saturating_sub(slash_amount) >= min_bal,
      ErrorCode::StakeVaultWouldGoBelow
  );
  ```
- **Regression gate.** Unit test: slash with
  `amount == vault_lamports` fails; slash with
  `amount == vault_lamports - min_bal` succeeds.

---

### Findings introduced by the 2026-04-22 master consolidated audit (renumbered after v0.4 collision)

### SOLID-SEC-031 -- `bufToDecimal` LE interpretation of Solana pubkey

- **Severity:** HIGH (pending end-to-end serialization trace; may
  escalate to CRITICAL if confirmed)
- **Status:** Open
- **Introduced:** 2026-04-22 (master-audit NEW-SEC-06; renumbered
  from 028)
- **Evidence:**
  - `ts-sdk/packages/holder/src/index.ts:391-397` (bufToDecimal
    walks buf.length-1 -> 0, treats buf[0] as LSB = LE)
  - `ts-sdk/packages/holder/src/index.ts:322`
    (`verifierAddress: bufToDecimal(VERIFIER_ID_BYTES)`)
  - `programs/zk-verifier/src/lib.rs:174-178` (on-chain
    `require!(public_inputs[28] == ID.to_bytes())`)
- **Description.** `bufToDecimal` is correct for LE-encoded field
  elements but interprets a Solana pubkey as LE. If the round-trip
  re-packs the resulting bigint via a BE `bigintToBytes32`,
  `public_inputs[28]` will be reverse of `ID.to_bytes()` and the
  on-chain check fails for every proof.
- **Impact.** If the pack is BE: every E2E verification fails.
  Explains missing devnet deploy.
- **Remediation.** Separate helpers `bigintFromBytesLE` /
  `bigintFromBytesBE`; BE for Solana pubkeys.
- **Regression gate.** `vector_verifier_id_roundtrip` added to
  cross-language suite.

### SOLID-SEC-032 -- `SCHEMA_REGISTRY_ID_BYTES` hardcoded without build-time check

- **Severity:** HIGH
- **Status:** Open
- **Introduced:** 2026-04-22 (master-audit NEW-SEC-02; renumbered
  from 029)
- **Evidence:** `crates/solid-light/src/cpi_helpers.rs:54-59`
- **Description.** Hand-decoded base58 pubkey stored as a 32-byte
  constant. The P0-2 owner-check on `global_tree` / `schema_tree_N`
  depends on this constant. `scripts/check_program_ids.py` does
  NOT validate it. Future redeploy that updates `Anchor.toml` but
  not this constant silently reopens the forged-trust-root attack.
- **Remediation.** Rust `#[test]` decoding base58 from build-time
  constant; extend `check_program_ids.py`.
- **Regression gate.** `schema_registry_id_bytes_matches_anchor_toml`.

### SOLID-SEC-033 -- Identity cohesion catch-22 (E2E blocker)

- **Severity:** HIGH
- **Status:** Open
- **Introduced:** 2026-04-22 (master-audit BUG-NEW-01; renumbered
  from 030)
- **Evidence:**
  - `ts-sdk/packages/holder/src/index.ts:250-255` (compare against
    `masterPublicKey.x`)
  - `ts-sdk/packages/holder/src/index.ts:281-285` (circuit leaf uses
    `deriveCredentialKey(masterPrivateKey, c.schemaHash)`)
  - `circuits/lib/identity_anchor.circom:24-43`
    (`identityState = Poseidon(derived.Ax, derived.Ay, revocNonce)`)
- **Description.** SEC-17 cohesion check compares
  `cred.holderPubKeyX` to master pubkey X. Circuit derives
  per-schema keypair and anchors `identityState` using the DERIVED
  key. No branch produces a provable witness.
- **Impact.** No correctly-issued credential can produce a verifying
  proof via the current holder SDK. Most likely reason E2E has
  never been run successfully.
- **Remediation.** Derive the per-schema pubkey inside the cohesion
  check and compare against that.
- **Regression gate.** `cohesion_check_passes_for_derived_key`.

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
