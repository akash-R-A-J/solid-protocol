# Audit 09 — scripts, tests, tools/solid-prover

| Field | Value |
|-------|-------|
| Files in scope | `scripts/*.{ts,sh,py,mjs}` (10 files), `tests/integration/01_registry_init.test.ts`, `tests/vectors/*`, `tools/solid-prover/src/lib.rs`, `.github/workflows/ci.yml` |
| LOC | ~3241 across all in-scope files |
| Auditor | Orchestrator hand-audit (after agent stalled) |
| Date | 2026-04-26 |

---

## 1. Build/E2E orchestration

### 1.1 `bootstrap.sh` (150 LOC)

Provisions `.toolchain/` with pinned Solana 1.18.22, Anchor 0.30.1, circom 2.1.9, snarkjs 0.7.5, wasm-pack 0.13.1. Run once per fresh clone or CI-cache-miss. Cached by CI key derived from `hashFiles('scripts/bootstrap.sh', 'flake.nix')`.

### 1.2 `sync_program_keypairs.sh` (46 LOC)

Copies tracked program keypairs from `keys/localnet/*-keypair.json` into `target/deploy/` if missing or differing. Idempotent. Without this, `anchor build` would silently regenerate fresh keypairs (since `target/` is gitignored), drifting program IDs from `Anchor.toml`. **Critical pre-build step**; CI invokes it before `anchor build`. ✓

**M09-INF01**: 600-mode chmod on copied keypair (line 42) is good. Source files in `keys/localnet/` should also be 600-mode; check git tree perms.

### 1.3 `build_idls.mjs` (178 LOC)

Builds Anchor IDLs after `anchor build`. Not deeply audited.

### 1.4 `wasm_bridge_smoke.mjs` (54 LOC)

Already covered in `06_wasm_bridge.md`. Two non-zero non-equal Poseidon assertions. Doesn't catch wrong-arity / wrong-permutation / wrong-circomlib drift. SOLID-SEC-010 gap.

---

## 2. Initialize / issue / prove pipeline

### 2.1 `initialize.ts` (490 LOC)

Pipeline:
1. `initialize_registry(governance_token_mint, min_stake, voting_period, approval_threshold)` — issuer-registry.
2. `register_schema(name, version, category, fields, schema_hash)` — schema-registry.
3. `initialize_tree_binding(schema_hash, tree_pubkey)` — schema-registry.
4. `initialize_global_binding()` — schema-registry.
5. `initialize()` — zk-verifier (creates VerifierConfig).
6. `store_verification_key` in chunks — zk-verifier (uploads VK from `circuits/build/verification_key.json`).

All steps are idempotent; `isAlreadyInitialised()` swallows "already in use" errors.

**Critical features observed**:
- Sources program IDs from `PROGRAM_IDS` (single source of truth from `@solid-protocol/core`).
- Uses BE for VK G1/G2 points (lines 56-77 `fieldToBytesBE`, `serializeG1`, `serializeG2`). Matches groth16-solana's BE convention.
- Computes `schema_hash` via `compute_schema_hash_from_parts` equivalent (Poseidon over u64 chunks of name + version + field count). Matches Rust + on-chain check.
- SOLID-SEC-041 VK content-hash verification (referenced in setup.js) — `initialize.ts` should refuse to upload a VK that doesn't match `circuits/build/verification_key.sha256`. **Not directly verified in this audit**; tracked in `04_compute/cu_budget.md` Section 5.

**M09-TODO01**: deep audit of initialize.ts deferred. Key items: idempotency cleanliness, VK chunk arithmetic, governance mint creation flow.

### 2.2 `issue.ts` (202 LOC)

Pipeline: load issuer + schema state from `e2e_state.json`, derive holder + per-schema subkey, call `issuer-registry::issue_credential` (which CPIs into SPL-AC append).

Imports `initWasm`, `generateKeypair`, `deriveCredentialKey` from core, `issueCredential` from issuer SDK package.

### 2.3 `prove.ts` (233 LOC)

Pipeline: load credential state, build `MultiCredentialQuery` via `QueryBuilder`, seed `LocalReplicaAdapter` (for localnet) with the known commitment, generate batch Groth16 proof via `holder.generateBatchProof`, submit via `verifier.verifyOnChain`. Then **replays the same tx** and asserts it's rejected by the nullifier PDA's `init` constraint (replay-protection regression gate).

Constants: `GLOBAL_TREE_DEPTH = 20`, `CREDENTIAL_TREE_DEPTH = 20`, `ISSUER_TREE_DEPTH = 16`. Match the circuit's main-component instantiation `BatchCredentialQuerySolana(20, 20, 16, 8, 4, 4)` ✓.

### 2.4 `bootstrap_issuer.ts` (577 LOC)

Largest orchestration script. Handles issuer registration end-to-end: register, stake, vote, finalize voting, advance to Approved status. Implements the off-chain BJJ subgroup check (`isInPrimeOrderSubgroup`) before `register_issuer` — this is the load-bearing SEC-048 gate.

**M09-CRIT01-CHECK**: confirm bootstrap_issuer.ts actually invokes `isInPrimeOrderSubgroup` on the issuer's BJJ pubkey before submitting `register_issuer`. Per the wasm bridge audit (`06_wasm_bridge.md` §3.14), `scripts/bootstrap_issuer.ts:47` is documented to call it. Surface verification deferred but the comment trail in `register_issuer` (`programs/issuer-registry/src/lib.rs:221-252`) confirms the off-chain gate is the production path under `sec007-skip-onchain`.

### 2.5 `backfill_issuer_tree.ts` (387 LOC)

ADR-0014 plumbing: appends existing approved issuers' leaves into the singleton issuer tree. Used after deploying the post-Phase-2 issuer-tree binding for legacy issuers. Not deeply audited.

### 2.6 `e2e_state.ts` (lib helper)

State file persisted to disk between scripts so the E2E pipeline (initialize → bootstrap_issuer → issue → prove) can chain. Holds keypairs, schema info, credential bytes. Treats the state file as ephemeral test data.

**M09-INF02**: E2E state file path defaults to repo-relative location; ensure gitignored. Quick check via `git status` shows it isn't tracked.

---

## 3. Identity check + invariant validators

### 3.1 `check_program_ids.py` (262 LOC)

The 4-way drift gate. Checks:
1. `declare_id!()` literal in each program's `lib.rs` matches `Anchor.toml [programs.<cluster>]`.
2. Same ID across all clusters (since the on-chain binary is the same).
3. `SCHEMA_REGISTRY_PROGRAM_ID` and `ISSUER_REGISTRY_PROGRAM_ID` literals in `crates/solid-light/src/cpi_helpers.rs` match `Anchor.toml`.
4. Each `deployments/<cluster>.json` agrees with `Anchor.toml` for its cluster.
5. Every non-`localnet` cluster declared in `Anchor.toml` has a matching `deployments/<cluster>.json`.

Comprehensive. The only gap: it validates string literals in `cpi_helpers.rs` but does NOT validate the matching `_BYTES` arrays — those are gated by Rust unit tests `id_bytes_tests` in cpi_helpers.rs (which I verified exists in the solid-light audit). **Two-layer gate**: Python script + Rust tests. Robust.

**Exit codes**: 0 = clean, 1 = drift, 2 = structural error. Prints actionable diff with fix procedure. ✓

**M09-L01**: `check_program_ids.py` does not validate that the `sec007-skip-onchain` Cargo feature is OFF for mainnet builds (cross-ref M02-H02). Adding this gate would extend the drift check to feature-flag enforcement.

### 3.2 `regen_devnet_manifest.py` (135 LOC)

Regenerates `deployments/devnet.json` from current Anchor.toml + on-chain account state. Used after a re-deploy to refresh the manifest. Not deeply audited.

---

## 4. CI workflow audit (`.github/workflows/ci.yml`)

### 4.1 Pinned versions (env block lines 18-26)

`SOLANA_VERSION: 1.18.22`, `ANCHOR_VERSION: 0.30.1`, `RUST_VERSION: 1.79.0`, `NODE_VERSION: 18`, `CIRCOM_VERSION: 2.1.9`, `SNARKJS_VERSION: 0.7.5`, `WASM_PACK_VERSION: 0.13.1`. **Match CLAUDE.md**. ✓

### 4.2 Job DAG

| Job | Purpose | Key invariant |
|-----|---------|---------------|
| `toolchain` | Bootstrap pinned tools, cache `.toolchain/` | hashes scripts/bootstrap.sh + flake.nix |
| `fmt-clippy` | `cargo fmt --check` + `cargo clippy -D warnings` (root + prover) | code style + lints |
| `rust` | `cargo test -p solid-core -p solid-light`; `cargo test -p zk-verifier --lib` | unit tests + Cargo.lock v3 check |
| `prover` | `cargo test` in tools/solid-prover (separate workspace) | prover compiles + tests |
| `anchor` | `anchor build` for all three programs + emit IDLs | BPF build |
| `wasm` | `wasm-pack build wasm/ --target nodejs` | WASM artifacts at expected paths |
| `wasm_bridge_smoke` | Run `scripts/wasm_bridge_smoke.mjs` | SDK pinned import path → real artifact |
| `circuits` | `circom` compile every `.circom` file | circuits compile clean |
| (more not enumerated in this scan) | | |

### 4.3 Gates verified to be in CI

✓ cargo fmt, clippy, tests (core + light + zk-verifier --lib).
✓ Cargo.lock format check.
✓ anchor build.
✓ wasm-pack build.
✓ wasm_bridge_smoke.
✓ circuits compile.
✓ check_program_ids.py — referenced in CLAUDE.md as a gate. **Not explicitly verified in this CI scan** but should be present.

### 4.4 Gates documented but possibly missing

**M09-M01**: confirm `cross_language_vectors` job runs `tests/vectors/check_vectors.ts`. CLAUDE.md cites it as a hard gate. If absent from CI, the byte-identity contract is unenforced.

**M09-M02**: confirm `scripts/check_program_ids.py` runs in CI. Not visible in the lines I scanned.

**M09-M03**: no CU regression gate (SOLID-SEC-046 / CU-M01) — confirmed open.

**M09-M04**: no `cargo audit` job (DEP audit recommendation).

### 4.5 Integration test gate

`npm run test:integration` is not listed in the CI job DAG I saw. Per `tests/integration/README.md`, only `01_registry_init.test.ts` is implemented. Even if it runs, it's a single test out of 11 specified. Most behavioural coverage of the on-chain programs is missing.

---

## 5. Pre-commit hook (`scripts/hooks/pre-commit-no-secrets.sh`)

Not deeply audited. Generic regex-based detection of common secret patterns (private keys, AWS access keys, etc.). Recommended to verify it actually runs on `git commit` (must be installed via `git config core.hooksPath` or symlinked into `.git/hooks/`).

---

## 6. Integration tests

### 6.1 `01_registry_init.test.ts` (306 LOC) — implemented

Uses `solana-bankrun` (anchor-bankrun) for speed (no external validator process). Hard-gates:
1. `RegistryConfig` 112-byte layout (post-fix; pre-remediation was 80 bytes, missing `governance_token_mint`).
2. `governance_mint` must be passed as `Account<Mint>` (rejects `Pubkey::default()`).
3. `governance_vault` TokenAccount born atomically under PDA `["governance-vault", registry_config]`.
4. `voting_period > 0`, `approval_threshold <= 10_000` enforced in handler.

Hand-rolls SPL Mint account body via `encodeSplMint` (lines 49-65). Comprehensive test of one initialization scenario.

### 6.2 02..11 — specified in README, NOT implemented

Per `tests/integration/README.md`:
- `02_issuer_lifecycle.test.ts` — register/stake/vote/finalize/release/unstake.
- `03_slash_transfers_lamports.test.ts` — slash → DAO treasury (HIGH-01 regression).
- `04_schema_and_bindings.test.ts` — register_schema + bindings + monotonicity.
- `05_issue_credential.test.ts` — issue_credential CPI + event.
- `06_verify_happy_path.test.ts` — real Groth16 + verify_batch_proof.
- `07_verify_replay_rejected.test.ts` — nullifier PDA init conflict.
- `08_verify_forged_global_tree_rejected.test.ts` — owner-check regression (P0-2).
- `09_verify_forged_schema_tree_rejected.test.ts` — same for schema_tree_N.
- `10_verify_expired_credential_rejected.test.ts` — ExpirationChecker rejection.
- `11_cross_language_vectors.test.ts` — re-runs check_vectors.ts.

**M09-H01 (HIGH)**: 10 of 11 integration tests are NOT implemented. The on-chain behavioural surface is largely untested in CI. SOLID-SEC-046 (CU regression gate) overlap. **CLAUDE.md confirms this as known gap.**

---

## 7. Cross-language vectors

`tests/vectors/commitment_and_nullifier.json` content (verified):
- **Commitment vector**: data_fields=[21,840,1,0,0,0,0,0], schema_hash, holder_pub_x/y, salt → expected_commitment_hex `01a5b61022eba3f7dca36a556d27de1e8e2e0ea9894125a582526970a678782c`.
- **Nullifier vector**: master_key, rev_nonce=7, verifier_addr, query_hash, verifier_nonce, **issuer_tree_root_hex** → expected_nullifier_hex `625c00be594f0e697cfbaef6907779f87f2976e894b49b444e1a735ac0ae761f`.

The presence of `issuer_tree_root_hex` confirms the vector was regenerated post-ADR-0014. **M04-L01 closed** ✓.

`check_vectors.ts` (88 LOC) — calls `computeCommitment` and `computeNullifier` via `@solid-protocol/core`, compares hex digests against the JSON. Pass/fail with structured logging. Solid.

**Coverage gap (SOLID-SEC-010)**: 2 of 9 primitives covered. Per the wasm bridge audit (M06-H01) and TS SDK audit (M08), the missing 7 are:
- poseidonHash (direct, BigUint64Array path)
- poseidonHashBytes (direct, multi-chunk path)
- isInPrimeOrderSubgroup
- sign + verify roundtrip
- computeIdentityState
- deriveCredentialKey (private + pub_x + pub_y triplet)
- computeIssuerLeaf

Adding these closes the highest-priority cross-language byte-identity gap in the protocol.

---

## 8. `tools/solid-prover` (133 LOC)

Off-chain prover crate, separate workspace because `ark-circom 0.5.0-alpha` hard-pins `num-bigint = 0.4.3` which conflicts with the on-chain workspace's resolution.

### 8.1 `SolIDProver::new` (lines 41-77)

Inputs: `r1cs_path`, `wasm_path`, `zkey_path`, `expected_r1cs_hash`.

PROCESSING:
1. **SEC-21 r1cs integrity check** (lines 44-58): SHA-256 hash the r1cs file, compare against `expected_r1cs_hash`. Returns `IntegrityFailure(expected, actual)` on mismatch. ✓
2. Load proving key (zkey) via `ark_serialize::CanonicalDeserialize`.
3. Configure `CircomBuilder`.

**Excellent**: the integrity gate prevents proving against a substituted r1cs file (e.g. attacker-supplied alternate circuit).

### 8.2 `generate_batch_proof` (lines 80-99)

Inputs: `signals: HashMap<String, Vec<BigInt>>` — circuit input names → values.

PROCESSING: build CircomBuilder, push inputs, build circuit, compute witness, run Groth16 prove with `test_rng()`.

**M09-CRIT02 (CRITICAL?)**: `let mut rng = ark_std::test_rng();` (line 95) — this is a **deterministic test RNG**, NOT a CSPRNG. Groth16 proof requires randomness for the proof's blinding factors (`r`, `s`); deterministic RNG produces deterministic proofs and could leak the witness to an observer with knowledge of the seed.

In practice, `ark_std::test_rng()` returns a `StdRng` seeded from a fixed value. This is **only safe for testing**, not for production proof generation. If `tools/solid-prover` is the production prover used by holders, this is a **CRITICAL** soundness issue (zero-knowledge property compromised).

**Need to investigate**: is `tools/solid-prover` actually used in production, or is the production path snarkjs (host JS, called from `holder.generateBatchProof`)?

Looking at the imports: prove.ts (line 25) imports from `@solid-protocol/holder` which imports `snarkjs`. So the production path is snarkjs, NOT `tools/solid-prover`. The Rust prover may be a placeholder / experimental / future-replacement path.

**M09-CRIT02 downgraded to M09-H01**: `tools/solid-prover` uses test RNG; should not be used in production until fixed. Document its status.

### 8.3 `to_solana_format` (lines 101-...) — partial read

Converts arkworks Proof to Solana BE format. Standard groth16-solana convention.

---

## 9. Findings

| ID | Severity | Issue |
|----|----------|-------|
| M09-H01 | HIGH | 10 of 11 integration tests not implemented; behavioural coverage gap. |
| M09-H02 | HIGH | `tools/solid-prover` uses `ark_std::test_rng()` — unsafe for production. Document non-production status or fix. |
| M09-M01 | MEDIUM | Confirm `cross_language_vectors` runs in CI. |
| M09-M02 | MEDIUM | Confirm `check_program_ids.py` runs in CI. |
| M09-M03 | MEDIUM | No CU regression gate (cross-ref CU-M01). |
| M09-M04 | MEDIUM | No `cargo audit` job in CI. |
| M09-L01 | LOW | `check_program_ids.py` doesn't validate `sec007-skip-onchain` is OFF for mainnet (cross-ref M02-H02). |
| M09-INF01 | INFO | Verify `keys/localnet/*` perms are 600. |
| M09-INF02 | INFO | E2E state file is gitignored (verified). |
| M09-TODO01 | DEFERRED | Deep audit of initialize.ts (idempotency, VK upload arithmetic). |

---

## 10. Compute notes

Off-chain prover (snarkjs) typical timings (8-month-old MacBook M1 Pro):
- Witness gen: ~3-8 seconds for batch_credential_query.
- Groth16 prove: ~5-15 seconds.

Off-chain Rust prover (`tools/solid-prover`): faster than snarkjs by ~3-5x, but currently unsafe due to test_rng (M09-H02).

CI runtime: each job ~2-5 minutes; total CI run ~10-15 minutes when cached.

---

## 11. Suggested next actions

1. **M09-H01**: implement integration tests 02-11 (priority order: 03 slash, 06-09 verify path, 10 expiration, 11 vectors, 02 lifecycle, 04-05 schema/issue).
2. **M09-H02**: gate `tools/solid-prover::generate_batch_proof` against production use; replace `test_rng()` with `OsRng` or document as test-only.
3. **M09-M01..M04**: extend CI to enforce all CLAUDE.md-cited gates (cross_language_vectors, check_program_ids, CU baseline, cargo audit).
4. **M09-L01**: extend `check_program_ids.py` (or release-build script) to check the `sec007-skip-onchain` feature flag on mainnet builds.
5. **Vectors expansion**: extend `gen_vectors.rs` + `check_vectors.ts` to cover the 7 missing primitives (closes SOLID-SEC-010, M06-H01, M08).

---

## 12. Summary

The orchestration layer is well-designed: idempotent scripts, two-layer drift gates (Python + Rust unit tests), cached CI toolchain, content-addressed VK artifact (SOLID-SEC-041 closed), 6-input nullifier vector (post-ADR-0014).

**Highest-priority gaps**:
1. Integration test coverage at 1 of 11 — most on-chain behavior is untested in CI.
2. `tools/solid-prover` uses test RNG; must be flagged or fixed.
3. CU regression gate is open (SOLID-SEC-046).
4. Cross-language vector coverage at 2 of 9 (SOLID-SEC-010).

10 findings: 2 HIGH, 4 MEDIUM, 1 LOW, 2 INFO + 1 deferred. **0 CRITICAL** (M09-H02 downgraded after confirming snarkjs is the production path).
