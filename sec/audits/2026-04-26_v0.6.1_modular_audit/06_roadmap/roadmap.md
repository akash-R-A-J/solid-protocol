# Prioritized Roadmap — solid-protocol v0.6.1 → mainnet

| Field | Value |
|-------|-------|
| Date | 2026-04-26 |
| Synthesizes | All wave-1 module audits + integration audit + docs drift |
| Sequencing principle | **Dependency order first** (later items often depend on earlier ones), then **blast radius** (close the largest open soundness surfaces first), then **leverage** (low-effort high-value items). |

---

## Read-me-first

**Mainnet posture**: NOT YET. Four blockers must close (Section 1).

**External-audit posture**: nearly ready. The post-Phase-3 codebase is reasonably complete; closing Section 2 items strengthens what the auditor sees.

**v0.6.1 posture**: ready to ship to devnet today. CI gates verify drift across program IDs; cross-language byte-identity holds for the two vectored primitives; behavioural soundness is sound modulo SEC-045's pre-revocation replay window (depth-of-tree dependent, requires operator to call `update_issuer_tree_root` immediately after each atomic ix).

---

## Section 1 — Mainnet blockers (4 items)

These MUST close before the protocol can deploy to mainnet. Order is dependency-driven.

### Blocker 1: SOLID-SEC-012 — multi-party trusted-setup ceremony

**Status**: open. Single-party ceremony today (`circuits/scripts/setup.js` line 6-14: explicit "TESTNET / DEVELOPMENT ONLY" banner). Whoever runs the script holds the toxic waste. Anyone holding the toxic waste can forge universal proofs.

**Why it's first**: every subsequent circuit change (SEC-006 part 2, SEC-007 in-circuit) requires the ceremony to be re-run. Better to do it once with the final circuit.

**Remediation**: organize a multi-party ceremony with attestation chain. Standard pattern: 3-7 participants, each contributes entropy; final transcript is publicly verifiable. Tools: snarkjs's `phase2.contribute` (already used in setup.js).

**Pre-req**: settle the final circuit shape (resolves SEC-006 part 2 and SEC-007).

**Effort**: weeks (logistics + verification).

### Blocker 2: SOLID-SEC-007 / SEC-048 — BJJ subgroup check

**Status**: open. On-chain `require_in_prime_order_subgroup` costs >1.4M CU (exceeds per-tx ceiling). `programs/issuer-registry/Cargo.toml:37` declares feature `sec007-skip-onchain` to bypass; off-chain TS predicate `isInPrimeOrderSubgroup` is load-bearing.

**Why it batches with Blocker 1**: the canonical fix is in-circuit (Option E — adds ~30K constraints, ~250ms additional proving cost). Requires fresh trusted setup.

**Remediation paths** (per `04_compute/cu_budget.md` §4):
- **Interim** (no circuit change, ship today): Option B — cofactor-clear in SDK before submission. Off-chain `multiply by 8` (3 BJJ doublings, ~3ms) followed by the on-chain consolation gate (`is_on_curve` + `!is_identity`). Strictly improves over the current state.
- **Canonical** (with Blocker 1): Option E — in-circuit subgroup constraint. Strongest soundness.
- **Future** (multi-quarter): Option F — Solana `sol_babyjubjub_*` syscall. Lets the protocol drop the in-circuit constraint.

**Effort**: Option B is days; Option E is weeks (circuit + proving + audit).

### Blocker 3: SOLID-SEC-045 / M02-H01 — atomic handlers don't update IssuerTreeBinding.current_root

**Status**: open. `revoke_issuer_atomic` and `request_withdrawal_atomic` (`programs/issuer-registry/src/lib.rs:1257-1373, 1415-1533`) replace the SPL-AC tree leaf in the same ix but leave `IssuerTreeBinding.current_root` stale. Pre-revocation proofs continue to verify until `update_issuer_tree_root` runs separately.

**Why it's a blocker**: the pre-revocation replay window depends on operator discipline (must call `update_issuer_tree_root` in the same tx bundle). Outside the operator's control, an indexer-driven attacker could submit a pre-revocation proof in the gap.

**Remediation**: inside each atomic ix, after the `replace_leaf` CPI, read the merkle_tree's current_root from the SPL-AC header and write it into `IssuerTreeBinding[40..72]` along with `Clock::slot.to_le_bytes()` at `[72..80]`. ~10-20K CU additional cost. Pattern mirrors the existing `update_issuer_tree_root` writer.

**Effort**: hours. Single-PR fix.

### Blocker 4: SOLID-SEC-043 — issuer_tree_operator multisig

**Status**: open. Every gated handler in issuer-registry that touches the tree (`update_issuer_tree_root`, `set_issuer_tree_binding_status`, `append_issuer_leaf`, `revoke_issuer_atomic`) checks `authority == registry_config.authority` — a single key.

**Remediation**: migrate to a Squads 3-of-5 PDA signer or DAO threshold PDA. Pre-mainnet operational hardening.

**Effort**: days (Squads integration + migration ix).

---

## Section 2 — External-audit blockers (close before formal audit)

These don't block mainnet directly but the external auditor will flag them. Closing them before the audit lowers the auditor's report severity.

### 2.1 SOLID-SEC-010 / M06-H01 — extend cross-language vectors (2 → 9)

Add vectors for the 7 missing primitives:
- `poseidonHash` (direct, BigUint64Array path)
- `poseidonHashBytes` (direct, multi-chunk path)
- `isInPrimeOrderSubgroup`
- `sign + verify roundtrip` (incl. tamper-detect)
- `computeIdentityState`
- `deriveCredentialKey` (private + pub_x + pub_y triplet)
- `computeIssuerLeaf`

Wire the extended `check_vectors.ts` into the `wasm_bridge_smoke` CI job so a failing vector blocks the same PR that built the divergent artifact.

**Effort**: ~150 LOC across `crates/solid-core/examples/gen_vectors.rs` + `tests/vectors/check_vectors.ts`. Days.

### 2.2 SOLID-SEC-046 / CU-M01 — CU regression gate

Per `04_compute/cu_budget.md` §5 — concrete CI job spec:
- Deterministic test vector at `tests/vectors/cu_baseline_proof.json`.
- Submit each handler with `ComputeBudgetProgram.setComputeUnitLimit({ units: 800_000 })` (catches 250% regressions).
- Parse `Program X consumed Y of 800000 compute units` from `solana confirm -v`.
- Compare against baseline in `docs/CU_BUDGET.md`; fail on >10% regression.
- Cover `verify_batch_proof`, `register_issuer` (with bypass), `revoke_issuer_atomic`, `request_withdrawal_atomic`, `append_issuer_leaf`, `issue_credential`.

**Effort**: ~2-3 days (CI plumbing + baseline capture).

### 2.3 M02-H02 — CI gate against shipping `sec007-skip-onchain` to mainnet

Extend `scripts/check_program_ids.py` (or release-build script) to inspect the compiled feature set of `issuer-registry` and reject mainnet builds with `sec007-skip-onchain` active. Today the safety net is documentation only.

**Effort**: hours. Single-script change.

### 2.4 M09-H01 — implement integration tests 02-11

Per `tests/integration/README.md`:
- 02: issuer lifecycle.
- 03: slash transfers lamports (regression for HIGH-01).
- 04: schema + bindings.
- 05: issue_credential.
- 06: verify happy path.
- 07: verify replay rejected.
- 08: verify forged global tree rejected.
- 09: verify forged schema tree rejected.
- 10: verify expired credential rejected.
- 11: cross-language vectors (re-runs check_vectors.ts).

**Priority order**: 11 (cheap, gates §2.1) → 03 (HIGH-01 regression) → 06 (full proof path) → 07 (replay) → 08, 09 (P0-2 regression) → 10 (expiration) → 02 (lifecycle) → 04, 05 (schema + issue).

**Effort**: ~2-3 days per test on average, parallelizable. Total ~3-4 weeks of work.

### 2.5 M09-H02 — `tools/solid-prover` test-RNG

`tools/solid-prover/src/lib.rs:95` uses `ark_std::test_rng()` (deterministic). Currently the prover is not in the production proof-generation path (snarkjs is), but if it's ever ramped, replace with `OsRng`.

**Effort**: hours.

### 2.6 SEC-019 / M03-M02 — schema-hash re-assertion

Add `require!(&data[8..40] == schema_hash, ErrorCode::SchemaHashMismatch)` to `set_binding_status` and `transfer_tree_binding_authority` in schema-registry. Two-line fix.

**Effort**: hours.

### 2.7 M01-M01 — two-step authority handover for zk-verifier

Replace single-step `transfer_authority` with `pending_authority: Option<Pubkey>` + `accept_authority` instruction. Prevents typo-locks on the on-chain authority.

**Effort**: hours.

### 2.8 SOLID-SEC-006 part 2 — VK generation in public-input contract

Bind `vk_generation` into the public-input contract so cross-VK replay is formally impossible. Today, replay is prevented by the VK changing (Groth16 catches via different IC); part 2 makes the property explicit. Requires circuit change → batches with Blocker 1.

**Effort**: weeks (with the trusted-setup cycle).

---

## Section 3 — High-leverage close-outs (do now, no dependencies)

Quick wins. Each one is hours-to-low-days and closes a tracked finding.

### 3.1 Dependency hygiene (one-PR set)

- **DEP-L05**: drop unused `hex` from `crates/solid-light/Cargo.toml:14`.
- **DEP-L03**: drop unused `ark-groth16` from workspace Cargo.toml:45.
- **DEP-L04**: remove unused `num-traits` from `crates/solid-core/Cargo.toml:45` (verify with `cargo udeps`).
- **DEP-L06**: remove unused `serde_json` from `wasm/Cargo.toml:19`.
- **DEP-L09**: remove unused `ts-node` from `.toolchain/npm/package.json:4`.
- **DEP-H01**: tighten `light-poseidon = "0.2"` to `=0.2.0` (byte-identity gate).
- **DEP-M02**: pin `proc-macro2 = "=1.0.94"` (anchor 0.30.1 idl-build gate).
- **DEP-M04**: commit `tools/solid-prover/Cargo.lock`.
- **DEP-L01**: unify snarkjs to `^0.7.5` across all four manifests.
- **DEP-L02**: add `engines: { node: ">=18.0.0", npm: ">=10.0.0" }` to root `package.json` and `ts-sdk/package.json`.

**Effort**: hours. Net diff: minimal manifest edits.

### 3.2 Doc reconciliation (Wave 3 of this audit)

Per `05_docs_drift/docs_drift.md`:
- DRIFT-01 / -03 / -04 / -05: fix `docs/MODULE_CONTRACTS.md` — schema-registry handler signatures are wrong.
- DRIFT-06: fix `circuits/batch_credential_query.circom:103-106` issuerAuthority BE/LE comment.
- DRIFT-08: rewrite `programs/zk-verifier/src/lib.rs:25-40` slot doc-comment notation.
- DRIFT-12: update `ts-sdk/packages/core/src/index.ts:435` "31 inputs" → "32".
- DRIFT-14: patch `01_zk_verifier.md` audit doc with correct nullifier preimage.

**Effort**: hours.

### 3.3 Dead-code removal (one-PR set)

- M07-L01: delete `circuits/lib/nullifier_expiry.circom::NullifierComputer` template.
- M07-L02: delete `circuits/lib/signature_verifier.circom`.
- M07-L03: delete `circuits/lib/credential_hasher.circom`.
- M07-INVESTIGATE-02: confirm `circuits/compound_query.circom` is dead, delete; if alive, audit separately.
- M08-CRITICAL01 / M08-H01: delete `holder.generateProof` (single-credential path with broken Poseidon arity); confirm only `generateBatchProof` is in production.
- M08-M01: delete `QueryBuilder._computeContextHash` (incorrect helper, unused in production).
- M05-LO-01: delete dead `build_insert_*` builders + `Compressed*` types from `solid-light` (or move to a host-only sibling crate if external consumers exist).
- M04-L02: delete or document `crates/solid-core/src/sas.rs` (forward-compat stub with placeholder program ID).

**Effort**: ~1 day. CI regen confirms no consumer.

### 3.4 Defensive code improvements (zk-verifier)

Per `01_zk_verifier.md`:
- Cache `Clock::get()?` once in `verify_batch_proof` (saves ~3K CU, no semantic change).
- Split `Unauthorized` into `UnauthorizedSigner` + `InvalidNewAuthority` for diagnosability.
- Add compile-time `static_assert!(size_of::<VerifyBatchProof>() < N)` analogous to existing VkBuf guard.

Per `02_issuer_registry.md`:
- M02-M01: route `withdraw_stake` and `withdraw_after_cooldown` through `transfer_slashed_lamports` (rent-floor + checked_sub).
- M02-M02: add `governance_mint: Account<Mint>` and mint constraint to `UnstakeTokens`.
- M02-L01: bound `voting_period > 0` in `initialize_registry`.
- M02-INF01: bound `approval_threshold <= 10000`.
- M02-INF05: replace `try_into().unwrap()` with `?`-propagated form in `update_issuer_tree_root` / `set_issuer_tree_binding_status`.

**Effort**: ~2 days. All low-risk hardening.

### 3.5 Tightening (schema-registry)

- M03-L05 / M03-L06: reject `Pubkey::default()` in `initialize_tree_binding.tree_pubkey` and `transfer_*_authority.new_authority`.
- M03-L07: add `msg!()` to silent state-mutating handlers (`update_tree_root`, `update_global_root`, `set_binding_status`, `increment_usage`).
- M03-LOW: change `compute_schema_hash_from_parts` truncation from 16 to 12 (matches actual Poseidon cap).

**Effort**: hours.

### 3.6 WASM bridge cleanup

- M06-L01: delete unused `wasm-bindgen-test` dev-dep, OR add at least one `#[wasm_bindgen_test]` byte-identity test.
- M06-L04: drop `serde(rename_all = "camelCase")` from `KeypairResult` / `DerivedKey`; remove `snakeCaseBjj` shim.
- M06-L05: pass field name into `to_arr32` for better error UX.
- M06-L07: pin a fixed expected output in the smoke test (cheap but stronger than non-zero / non-equal).

**Effort**: hours.

---

## Section 4 — CI / operational hygiene

These extend CI to enforce gates that are documented but not enforced.

- **M09-M01**: confirm `cross_language_vectors` runs in CI (visible to `tests/vectors/check_vectors.ts`).
- **M09-M02**: confirm `scripts/check_program_ids.py` runs in CI.
- **M09-M03**: ship CU regression gate (cross-ref §2.2).
- **M09-M04**: add `cargo audit` to CI (catch RustSec advisories beyond the static checks).
- **M03-M01**: add `[dev-dependencies]` to `programs/schema-registry/Cargo.toml` + at least one in-program unit test asserting discriminator constants match `solid-light`'s parser-side constants.
- **M01-M02**: add host-testable harness for the 8 pre-Groth16 gates in `verify_batch_proof` (arity, nullifier match, verifier address, timestamp window, owner checks, schema ordering). Closes integration-test gap on the verifier path.
- Add `npm audit --omit=dev` to CI.
- Add `cargo tree -d` regression check (catches new dep duplication beyond known ark-* / borsh / syn split).
- Add `cargo deny` rule disallowing direct `borsh` imports outside solid-light, and direct `ed25519-dalek = "1"` imports outside solana-sdk.
- Add `scripts/check_docs.py` (referenced in IMPROVEMENTS_ROADMAP.md) — automated `[x]` reconciliation between roadmap and registry.

---

## Section 5 — Deferred + investigation items

### Investigation (do not delete until confirmed)

- **M07-INVESTIGATE-02**: is `circuits/compound_query.circom` (275 LOC) alive or dead? Cross-reference M08-H01 (single-credential `generateProof` path).
- **M03-M04**: `initialize_global_binding` is permissionless. Operationally mitigated by private-RPC bootstrap (deployer runs `scripts/initialize.ts` immediately after `anchor deploy`). Pre-mainnet: consider gating to program upgrade authority via `BPFLoaderUpgradeable::ProgramData`. Same pattern likely affects `programs/issuer-registry::initialize_registry` and `programs/zk-verifier::initialize`.
- ADR-0014 `revocation_nonce` re-approval semantics: re-registration creates a new IssuerAccount via `init`; lifecycle "previously revoked re-approved in place" cannot occur. Verify doc matches.

### Deferred deep-audits

- M04-TODO01: `crates/solid-core/src/multi_cred.rs` (335 LOC) — multi-credential helpers, padding-slot encoding.
- M04-TODO02: `crates/solid-core/src/query.rs` (388 LOC) — predicate types, range constraints.
- M08-TODO01: `ts-sdk/packages/issuer/src/index.ts` (269 LOC) — credential issuance signing path.
- M08-TODO02: `ts-sdk/packages/light/src/index.ts` (503 LOC) — SPL-AC adapter, fetchMerkleProof failure modes.
- M08-TODO03: top-level `ts-sdk/packages/sdk/src/{index,config,rpc}.ts`.
- M09-TODO01: `scripts/initialize.ts` (490 LOC) — VK upload arithmetic, idempotency cleanliness.

These are surface gaps in this audit; none are known to harbor specific findings, but a thorough pre-mainnet pass would close them.

---

## Section 6 — What's done (closed since v0.6 audit)

Per `Phase 3 impl 1-4` commits + this audit's verification:

- **SOLID-SEC-007 part 1**: BJJ subgroup check enforced at issuer registration in builds without `sec007-skip-onchain`. Off-chain TS predicate `isInPrimeOrderSubgroup` covers the remaining gap. (Commit `fd68620`)
- **SOLID-SEC-006 part 1**: VK freeze-gate + 48h rotation timelock. (Commit `d7bf9b1`)
- **SOLID-SEC-041**: content-addressed VK artifact (`circuits/build/verification_key.sha256`). (Commit `4d97692`)
- **SOLID-SEC-044**: `request_withdrawal_atomic` mirrors `revoke_issuer_atomic` for cooldown handler. (Commit `212eb6f`)
- **ADR-0014**: compressed issuer tree with BJJ-binding leaf; 6-input nullifier with issuerTreeRoot. NR_PUBLIC_INPUTS 31 → 32. SEC-008 epoch bind. (Phase 2 commits)
- **SOLID-SEC-002**: schema-hash integrity check enforced in `register_schema`.
- **SOLID-SEC-003**: schema-tree binding gate in `issue_credential`.
- **SOLID-SEC-029**: `IdentityAnchor.enabled` for padding slots.
- **SOLID-SEC-031**: BE encoding for `verifierAddress` (matches on-chain `ID.to_bytes()`).
- **SOLID-SEC-032**: hardcoded program-ID byte arrays in `cpi_helpers.rs` with drift tests.
- **SOLID-SEC-033**: identity cohesion check in holder SDK + per-schema key derivation in circuit.
- **SOLID-SEC-047**: BPF stack-frame fix via `lto = "thin"` + `#[inline(never)]` + heap-resident IC table.
- **SOLID-SEC-001**: range checks on queryCredentialIndices + queryFieldIndices in circuit.
- **SOLID-SEC-005**: timestamp skew window, on-chain Clock binding to `currentTimestamp`.
- **SOLID-SEC-013**: verifier scope binding via `verifierAddress` public input.
- **SOLID-SEC-019** (partial): `update_tree_root` re-asserts schema_hash. **Open** for `set_binding_status` and `transfer_tree_binding_authority` (M03-M02).
- **SOLID-SEC-020**: canonical schema ordering in proof (strict ascending).
- **B6**: three-target Poseidon dispatch (BPF / non-wasm host / wasm32) via `solid-core` cfg arms.
- **B10**: governance vault PDA split into separate `init_governance_vault` ix.
- ADR-0015 implementation: VK rotation timelock with comprehensive unit tests.

---

## Recommended start sequence

If I had to prioritize a single 2-week sprint to maximize value:

**Week 1**:
- Day 1: Section 3.1 (dependency hygiene) — single PR, low risk, immediate visibility.
- Day 1-2: Section 3.2 (doc reconciliation) — single PR.
- Day 2-3: Section 3.3 (dead-code removal) — single PR per cleanup, CI-gated.
- Day 3-5: Blocker 3 (SEC-045 atomic-handler binding root) — high-value soundness fix.
- Day 4-5: Section 2.6 (SEC-019 schema-hash re-assertion) — two-line fix.

**Week 2**:
- Day 6-8: Section 2.1 (cross-language vectors expansion) — closes the highest-leverage external-audit blocker.
- Day 8-10: Section 2.2 (CU regression gate) — protects against future SEC-006 part 2 changes.
- Day 9-10: Section 4 (CI gate confirmations + cargo audit + npm audit).

**Outcome**: at the end of these 10 days, the protocol is materially closer to external-audit-ready. The remaining mainnet blockers (SEC-007/048 in-circuit, SEC-012 ceremony, SEC-043 multisig) are all structurally larger items that require planning beyond a 2-week sprint.

---

## End-of-audit checklist (for the next reviewer)

When picking this up:

- [ ] Verify `scripts/check_program_ids.py` exits 0 at HEAD (`python3 scripts/check_program_ids.py`).
- [ ] Verify `cargo test -p solid-core -p solid-light` passes at HEAD.
- [ ] Verify `cargo test -p zk-verifier --lib` passes at HEAD.
- [ ] Verify `tests/vectors/check_vectors.ts` passes at HEAD (after `npm ci && npm run build`).
- [ ] Verify circuit compiles: `cd circuits && npm install && npx circom batch_credential_query.circom --r1cs --wasm`.
- [ ] Verify wasm-pack build completes: `wasm-pack build wasm/ --target nodejs --out-dir ts-sdk/packages/core/wasm --release`.
- [ ] Verify `bash scripts/wasm_bridge_smoke.mjs` passes after the wasm-pack build.
- [ ] Cross-check this roadmap against `sec/SECURITY_REGISTRY.md` for any new SOLID-SEC-* IDs.
- [ ] Confirm Wave 3 docs reconciliation (`05_docs_drift/docs_drift.md`) has been started.

---

## End

This roadmap is the synthesized output of the 2026-04-26 modular audit. Per CLAUDE.md, code is the source of truth; docs follow. The roadmap is dense but ordered: **start with Section 3.1, finish with Section 1**. The four mainnet blockers (Section 1) are tracked and have concrete remediation paths.
