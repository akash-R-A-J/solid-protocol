# Integration + Test Status -- 2026-05-02 (post-Phase-E full sweep)

Companion to `sec/audits/2026-05-02_v0.6.1_post_phase_e_full_system_audit.md`.

This doc consolidates the **drift catalog**, **done-vs-left matrix**, **integration cohesion check**, **test inventory + run results**, and **clean-slate E2E receipts** captured during the 2026-05-02 audit pass.

- HEAD: `3671943` ("SEC-048 Phase E close: SEC-083 fix + e2e green + docs sweep + CU rebaseline")
- Method: parallel multi-agent verification across 6 drift dimensions + empirical clean-slate e2e

---

## 1. TL;DR

- **279/279 host + circuit unit tests pass** (171/171 re-verified this session: solid-core 73, solid-light 68, zk-verifier 30; balance from session log: issuer-registry 33, schema-registry 30, circuits 45).
- **Clean-slate localnet `npm run e2e` is GREEN end-to-end.** Reference verify_batch_proof_v2 tx: `5hckZo1xsRPrzt3obKG5cmVLV42psxdxTjhZzcJZjEyJAeJNo2wvxPLs6haJfSXNA327SZefbw2kz1qNM5k4WiHJ`. Replay rejected. Exit 0.
- **Cryptographic primitives, public-input contract, owner-checks, atomic-ix integrity, Phase E subgroup-proof wiring all PASS** independent verification across programs + circuits + Rust crates + WASM bridge + TS SDK. No fragmentation.
- **No new soundness findings.** Two LOW operational findings introduced (NF-08 doc-claim ahead of script; NF-09 clippy/fmt CI red).
- **Backlog unchanged from 2026-05-01 synthesis** except SEC-048 + SEC-083 closed in this arc; SEC-010 (cross-language vectors 2/10) and integration suite (1/11 implemented) remain the highest-leverage open items.

---

## 2. Exhaustive drift catalog

### 2.1 Cryptographic / contract drifts -- NONE

Every "hard invariant" in CLAUDE.md was verified byte-for-byte against code. Full evidence table in the audit doc §2.

### 2.2 Code-level drifts (10)

| # | Drift | Severity | File:line | Pass | Action |
|---|-------|----------|-----------|------|--------|
| 1 | `tests/vectors/commitment_and_nullifier.json` covers 2 of 10 cross-language primitives (SOLID-SEC-010 known open) | HIGH | tests/vectors/* | C | Add 8 vectors to gen_vectors.rs + check_vectors.ts |
| 2 | `tests/integration/01_registry_init.test.ts` is the only authored integration test; 02..11 missing | HIGH | tests/integration/ | F | Author 02..11 + add `npm run test:integration` runner |
| 3 | `cargo clippy -p solid-core -p solid-light -- -D warnings` red on `crates/solid-core/src/sas.rs:45::attester` (`dead_code`) | LOW | sas.rs:45 | F | `#[allow(dead_code)]` or remove the field |
| 4 | `cargo fmt --all -- --check` red on `crates/solid-light/src/cpi_helpers.rs` (~10-line wrap diff) | LOW | cpi_helpers.rs | F | `cargo fmt --all` |
| 5 | `withdraw_after_revoke` on-chain ix has no SDK helper / no caller / no integration test | LOW | issuer-registry lib.rs:773 | D | Add `@solid-protocol/issuer::withdrawAfterRevoke`; add `tests/integration/13_withdraw_after_revoke.test.ts` |
| 6 | Legacy `verify_batch_proof` accessible via `SOLID_VERIFY_USE_LEGACY=1`; ambiguous status | LOW | verifier/index.ts:589 | D | Either delete OR document explicitly as fallback in MODULE_CONTRACTS.md |
| 7 | `tests/integration/12_subgroup_vk_init_authority_race.test.ts` missing -- SEC-083 has source-level pin only | LOW | tests/integration/ | F | One bankrun test |
| 8 | `anchor build` fails the IDL stage with proc-macro2 1.0.94 SourceFile trap on rustc 1.79 | LOW (process) | host pipeline | F | Already handled via `scripts/build_idls.mjs`; consider adding `--no-idl` shorthand wrapper |
| 9 | Six ts-sdk packages declare `"test": "jest"` with no jest install + zero `*.test.ts` files | MED | ts-sdk/packages/*/package.json | F | Author at least one suite per package OR delete the script stubs |
| 10 | `scripts/measure_cu.py` referenced by `docs/CU_BUDGET.md` + `tests/cu_baselines.json`; existence not confirmed in this audit (NF-08) | LOW | docs/CU_BUDGET.md | F | Commit script + workflow OR retract claim |

### 2.3 Doc-level drifts (6)

| # | Drift | Severity | File:line |
|---|-------|----------|-----------|
| 1 | `README.md:13-22` claims "v0.6 (April 2026, post-Phase-2)"; canonical-audit pointer is the 2026-04-24 doc; actual HEAD is post-Phase-E | HIGH | README.md:13-22 |
| 2 | `docs/CURRENT_STATE.md:1-6` header says "post-Phase-2, pre-Phase-3-close"; rows 53 (`sec007-skip-onchain` shown closed-not-deleted) and 60 (`request_withdrawal_atomic` shown open) are stale | HIGH | docs/CURRENT_STATE.md:1-6,53,60 |
| 3 | `plan/IMPLEMENTATION_PLAN.md` last revised 2026-04-28; treats SEC-048 as active interim bypass | MED | plan/IMPLEMENTATION_PLAN.md:7-62 |
| 4 | `CLAUDE.md` "Hard invariants" section does not name the Phase E subgroup VK pin (`938ab390...e9`) | MED | CLAUDE.md (Hard invariants) |
| 5 | `CLAUDE.md:130` literal placeholder "SEC-048 closed (Phase E, 2026-05-XX)" | TRIVIAL | CLAUDE.md:130 |
| 6 | `docs/IMPROVEMENTS_ROADMAP.md` reference green tx is the 2026-05-01 batch-only one; Phase E close-out tx not propagated | LOW | docs/IMPROVEMENTS_ROADMAP.md:5-9 |

### 2.4 Process drifts (3)

- **3.1.** Pre-commit hooks are opt-in via manual `ln -sf` (per `synthesis 6.3`); no `scripts/install_hooks.sh` enforces them at bootstrap.
- **3.2.** `cargo clippy` runs only against `solid-core` and `solid-light` in CI; the three programs (~6,600 LoC) are unlinted (`synthesis 6.4`).
- **3.3.** `Cargo.lock` v3 enforcement (`head -5 Cargo.lock | grep -qE '^version = 3'`) lives in CI but is not in CLAUDE.md's local pre-commit list.

---

## 3. E2E Run Receipt -- clean slate, post-Phase-E build

### 3.1 Procedure (reproducible)

```bash
cd /Users/rajakash/Desktop/testing/solid-protocol
export PATH="$(pwd)/.toolchain/bin:$PATH"

# 1. Kill validator + wipe all state stores (per memory rule:
#    feedback_e2e_clean_slate.md)
pkill -9 -f solana-test-validator 2>/dev/null
rm -rf test-ledger
rm -f $TMPDIR/solid-e2e-*/state.json

# 2. Hydrate program keypairs (idempotent)
bash scripts/sync_program_keypairs.sh --reset-state

# 3. BPF build (avoids the IDL-stage proc-macro2 trap that
#    `anchor build` triggers on rustc 1.79; CLAUDE.md documents
#    the workaround). All three programs.
cargo build-sbf --manifest-path programs/issuer-registry/Cargo.toml
cargo build-sbf --manifest-path programs/zk-verifier/Cargo.toml
cargo build-sbf --manifest-path programs/schema-registry/Cargo.toml

# 4. Boot validator with SPL AC + Noop cloned from devnet
COPYFILE_DISABLE=1 COPY_EXTENDED_ATTRIBUTES_DISABLE=1 \
  solana-test-validator --reset \
    --clone-upgradeable-program cmtDvXumGCrqC1Age74AVPhSRVXJMd8PJS91L8KbNCK \
    --clone-upgradeable-program noopb9bkMVfRPU8AsbpTUg8AQkHtKwMYZiFUjNRtMmV \
    --url https://api.devnet.solana.com &

# 5. Deploy via solana CLI (sidesteps anchor's IDL re-build)
solana airdrop 100 -u localhost
for prog in issuer_registry schema_registry zk_verifier; do
  solana program deploy target/deploy/${prog}.so \
    --program-id target/deploy/${prog}-keypair.json -u localhost
done

# 6. Run e2e (logged to file -- never `| tail`, per memory rule:
#    feedback_pipe_tail_buffering.md)
SOLID_VOTING_PERIOD_SECONDS=120 \
  npm run e2e > /tmp/e2e_step8_run.log 2>&1
```

### 3.2 Result

- **Exit code:** `0`
- **Wall-clock:** start 14:34, finish 14:38:42 IST -- ~4 minutes 30 seconds total (sync + build + deploy + e2e)
- **Validator:** `solana-test-validator 1.18.22` on local RPC `http://127.0.0.1:8899`
- **All five programs loaded:**

| Program | ID | Status |
|---------|----|----|
| `issuer_registry` | `5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx` | DEPLOYED slot 50 (820,224 bytes) |
| `schema_registry` | `4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1` | DEPLOYED slot 65 (382,144 bytes) |
| `zk_verifier`     | `DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb` | DEPLOYED slot 79 (391,224 bytes) |
| SPL Account Compression | `cmtDvXumGCrqC1Age74AVPhSRVXJMd8PJS91L8KbNCK` | CLONED from devnet |
| SPL Noop                 | `noopb9bkMVfRPU8AsbpTUg8AQkHtKwMYZiFUjNRtMmV` | CLONED from devnet |

### 3.3 Pipeline-step receipts

```
[build:idl]              -- regenerated target/idl/{issuer,schema,zk}_*.json
[init-onchain]           -- 8/8 steps green (incl. main VK + subgroup VK chunked upload + finalize + freeze)
[backfill-issuer-tree]   -- 4/4 steps green (SPL AC tree + binding init + initial leaves)
[bootstrap-schema-tree]  -- 4/4 steps green
[bootstrap-issuer]       -- 8/8 steps green
   issuer_account     : 5CnVXRx9pU2YDEkTYCHV7LFjGbeSJps4vXHBnNqNYndR
   issuer_authority   : 9UkaxyEhQUjjhPLKd1esRbPtepAiGr2ZetqHEFKBsJdG
   stake_vault        : CQgHHoHWF9W6SAJynNLTtFzGWs8LGCuoBV4V8PXhZPy2
   governance_mint    : FjEW7B5KBYaEVYCgZGchrk4Mng9dFrRUYQcynRPQTzkD
[issue]                  -- 3/3 steps green
   commitment: 5606515df4e56f29daa78c5fffee2fa32b5cee6734120f1bd7c62ae7d8a52007
   issue tx:   34chsEMpmEdTWedQjKQVrRJ8a694rmL5diR51M6NVzmvmDNVSj8GaTVE3f19YYf6e4MKXJEELp4xGDdiM3w1nLPS
[prove]                  -- 4/4 steps green
   Groth16 proof generated in 4.69s (snarkjs.fullProve)
   LOCAL groth16 verify: OK
   ALT: G7xUgmzB9rN1e6qbRt8rsuq8oqhCnTvs9mBc5b81ZAhv (fresh)
   ix.data.length = 972 (expected 972)
   nullifier  : 0ecb5d747053164315532af1ed960178bf92ec54012e9961a4c2c9caa1a07ed1
   verified   : true
   verify tx  : 5hckZo1xsRPrzt3obKG5cmVLV42psxdxTjhZzcJZjEyJAeJNo2wvxPLs6haJfSXNA327SZefbw2kz1qNM5k4WiHJ
[Replay test]            -- ok (rejected by nullifier PDA init constraint)
```

### 3.4 Confirming the green tx

```bash
solana confirm 5hckZo1xsRPrzt3obKG5cmVLV42psxdxTjhZzcJZjEyJAeJNo2wvxPLs6haJfSXNA327SZefbw2kz1qNM5k4WiHJ \
               --url http://127.0.0.1:8899
# Expected: Finalized
```

### 3.5 Public-input layout actually submitted (32 slots, post-ADR-0014)

Captured live from `scripts/prove.ts` debug dump:

| Slot | Field | BE hex | Source |
|------|-------|--------|--------|
| 0 | nullifierHash | `0ecb5d74...a07ed1` | circuit output |
| 1 | globalRoot | `1a5417cb...85fd91` | reconstructed from `global_binding` PDA |
| 2 | merkleRoots[0] (active credential) | `05b0df54...875f` | reconstructed from `schema_tree_binding` PDA |
| 3-5 | merkleRoots[1..3] padding | zeros | empty slots |
| 6 | schemaHashes[0] | `1fb3729e...31f0a` | from on-chain SchemaAccount |
| 7-9 | schemaHashes[1..3] padding | zeros | |
| 10 | **issuerTreeRoot (ADR-0014)** | `0cbf71be...071d6d` | reconstructed from `issuer_tree_binding` PDA |
| 11-18 | queryCredentialIndices/queryFieldIndices | zeros (or selectors) | |
| 19 | queryOperator[0] | `04` | EQ |
| 23 | queryValue[0] | `15` (=21) | sample `age >= 21` predicate |
| 27 | numPredicates | `01` | |
| 28 | compoundLogic | `00` | AND |
| 29 | verifierAddress | `033ba619...506780` (BE-encoded pubkey) | |
| 30 | verifierNonce | `0101...0101` | |
| 31 | currentTimestamp | `0x69f5bf10` (= 1777712912) | |

All 32 slots present; the ones reconstructed on-chain (slots 1, 2..5, 6..9, 10, 31) are byte-reverse-corrected per LB3; slot 29 stays BE per SOLID-SEC-031. Wire transmits the 21-element subset (`WIRE_INPUT_SLOTS`).

### 3.6 Phase E subgroup proof receipt (live this session)

`bootstrap_issuer.ts` ran the full Phase E.3 path: generated a Groth16 proof of BJJ subgroup membership (~50-150ms snarkjs.fullProve against `bjj_subgroup_proof.zkey`), submitted as 256-byte `subgroup_proof` arg to `register_issuer`. On-chain `verify_groth16_proof::<2>` accepted the proof. `sec007-skip-onchain` Cargo feature is no longer in tree; no `Sec007Bypass` event fired.

### 3.7 CU baseline match

The e2e run's measured CUs are within 1.10x of the pinned baselines in `tests/cu_baselines.json` (which were captured from a prior 2026-05-02 run). Highlights:
- `RegisterIssuer` -- ~180,037 CU (Groth16 N=2 verify; 35% of the 500K cuIx)
- `VerifyBatchProofV2` -- ~322,356 CU (1.008x baseline)
- All 21 tracked ixs within tolerance

---

## 4. Done vs Left -- master matrix

### 4.1 DONE (green at HEAD `3671943`)

**Phase 1 (security remediation):** SEC-001..SEC-009, SEC-011, SEC-013-class fixes. Closed 2026-04-23. 
**Phase 2 (ADR-0014 + 6-input nullifier):** SEC-004, SEC-005, SEC-006 Part 1, SEC-007, SEC-008, SEC-029-031, SEC-036, SEC-041. Closed 2026-04-24. 
**Phase 3 impl 1-4 (build pipeline + atomic-ix integrity):** SEC-044, SEC-045, SEC-049, SEC-050, SEC-052, SEC-053. Closed 2026-04-28. 
**Phase 3 close (P0/P1 batch + B13 wire-size + NF-batch):** SEC-046, SEC-054, SEC-058, SEC-059, SEC-061-064, SEC-066, SEC-067 (LB5), SEC-072, SEC-077, SEC-078, SEC-079. Closed 2026-04-30 / 2026-05-01.
**SEC-080 + SEC-081 + SEC-017:** Closed 2026-05-01.
**SEC-048 (Phase E Option B) + SEC-083 (auth-race hardening):** Closed 2026-05-02 -- this arc.

**Cumulative: 45 of 67 registry findings closed; 6 CRITICAL all closed; 19 of 23 HIGH closed.**

### 4.2 LEFT (open at HEAD `3671943`)

| ID | Severity | Title | Mainnet blocker? |
|----|----------|-------|------------------|
| SEC-010 | HIGH | Cross-language test vectors at 2/10 primitives | No (devnet OK) |
| SEC-012 | HIGH | Multi-party trusted setup ceremony | YES |
| SEC-013 | MEDIUM | `slash_issuer`/`submit_fraud_proof` single-key | YES (governance) |
| SEC-014 | MEDIUM | Single shared `stake_vault` PDA | No (operational) |
| SEC-015 | MEDIUM | `approve_via_trust_anchor` no minimum-tier gate | No |
| SEC-016 | MEDIUM | `transfer_authority` no propose/accept | No |
| SEC-018 | MEDIUM | `verifier_config` write-lock throughput cap | No (perf) |
| SEC-019 | MEDIUM | `set_binding_status` schema-hash re-assertion | No (defense-in-depth) |
| SEC-021 | MEDIUM | Depth-20 cap at ~250K holders | No (capacity) |
| SEC-022 | LOW | Local `IsZero` reimpl in credential_atom.circom | No |
| SEC-023 | LOW | `active_issuers` counter drift on Cooldown→Revoked | No |
| SEC-024 | LOW | `unstake_tokens` raw `-=` instead of `checked_sub` | No |
| SEC-025 | INFO | `CheckIssuerStatus` ungated/never called | No |
| SEC-026 | INFO | `Credential::verify_integrity` never called on-chain | No |
| SEC-034 | MEDIUM | `SubmitFraudProof`/`SlashIssuer` PDA seed constraint | No (with SEC-013) |
| SEC-035 | LOW | `set_binding_status` no timelock | No |
| SEC-037 | INFO | `WithdrawAfterCooldown` explicit authority | No |
| SEC-038 | INFO | i16/reader-writer/GreaterThan(8) cluster | No |
| SEC-043 | MEDIUM | Single-signer `IssuerTreeBinding.operator` | YES |
| SEC-051 | LOW | All-padding circuit acceptance (1-line constraint deferred to next setup) | No |
| SEC-082 | LOW | `cooldown_ends_at` field-reuse layout migration | No |
| SEC-006 P2 | -- | `vk_generation` in circuit publics (deferred to next setup) | No |

**Open mainnet blockers:** SEC-012 (multi-party ceremony), SEC-013/SEC-034 (governance multisig on slash/fraud), SEC-043 (multisig on tree-operator).

**Open devnet blockers:** none.

### 4.3 Tests left to write

1. **Integration suite 02..11** (10 scenarios; per `tests/integration/README.md`).
2. **`tests/integration/12_subgroup_vk_init_authority_race.test.ts`** (SEC-083 bankrun).
3. **`tests/integration/13_withdraw_after_revoke.test.ts`** (SEC-061 happy path).
4. **ts-sdk jest baseline** for all 6 packages (currently 0 `*.test.ts` files; jest declared, not installed).
5. **Cross-language vectors** -- close SEC-010 by adding 8 missing primitives.
6. **`scripts/measure_cu.py`** + `cu_regression` CI workflow if not yet committed (NF-08 reconciliation).
7. **`tests/fixtures/spl_ac_tree.bin`** SPL AC layout fixture (synthesis 8.1).
8. **Circuit negative-case coverage** for SEC-051 (all-padding rejection) once next ceremony lands.

---

## 5. Integration cohesion check -- VERIFIED

The user's framing: "are all the systems integrated with each other or not? we don't want our code to be fragmented."

Eight cohesion edges were verified at HEAD; all PASS:

```
                    ┌─────────────────┐
                    │   docs / ADRs   │  PASS-with-fixable-drift (§2.3)
                    │   plans / sec   │
                    └────────┬────────┘
                             │
                             ▼
         ┌───────────────────────────────────┐
         │           Programs (3)            │  PASS
         │  zk-verifier  issuer  schema      │
         │  + canonical IDs + owner-checks   │
         └────┬───────────┬─────────────────┘
              │           │
              ▼           ▼
       ┌────────────┐  ┌────────────┐
       │  Circuits  │  │ Rust crates│  PASS (host + BPF dual-target)
       │  batch +   │  │  solid-core│
       │  subgroup  │  │  solid-light│
       └────┬───────┘  └─────┬──────┘
            │                │
            └────────┬───────┘
                     ▼
              ┌──────────────┐
              │ WASM bridge  │  PASS (top-level wasm/ crate; solid-core BPF-clean)
              └──────┬───────┘
                     ▼
        ┌────────────────────────────┐
        │      TS SDK (6 pkgs)       │  PASS
        │ core / holder / issuer /   │
        │ light / sdk / verifier     │
        └────────┬───────────────────┘
                 ▼
           ┌──────────────┐
           │   Scripts    │  PASS (every step calls SDK, not raw Anchor client)
           │ + e2e (npm)  │  PASS (clean-slate green; tx ref §3)
           └──────────────┘
```

| Edge | Direction | Verified by | Result |
|------|-----------|-------------|--------|
| Programs ↔ circuits | public-input contract | NR_PUBLIC_INPUTS=32 + named indices + Poseidon arities + static asserts (audit §3) | PASS |
| Programs ↔ programs | cross-program owner checks | Const ID bytes in cpi_helpers + regression tests | PASS |
| Programs ↔ Rust crates | shared primitives | solid-core BPF-clean; lifted Groth16 to solid-light in E.1 | PASS |
| Rust crates ↔ WASM bridge | byte-format helpers | wasm/src/lib.rs is pass-through | PASS |
| WASM ↔ TS SDK | crypto primitives | Six pkgs all import `@solid-protocol/core` | PASS |
| TS SDK ↔ scripts | Anchor-typed callers | Every script consumes SDK pkgs not raw Anchor | PASS |
| Scripts ↔ on-chain ix surface | per-ix caller matrix | 10/11 ixs have a caller; `withdraw_after_revoke` exposed but unused (D-2) | PASS |
| Docs ↔ code | drift inventory | 6 doc drifts, all bounded to README/CURRENT_STATE/IMPLEMENTATION_PLAN/CLAUDE.md placeholders | PASS-with-fixable-drift |

**Conclusion: the system is integrated, not fragmented.** Every layer has exactly one source of truth (programs and circuits at the bottom, scripts at the top), and every consumer references upward without parallel reimplementations of crypto primitives, ix discriminators, or program-ID literals.

---

## 6. Test inventory + run results (this session)

### 6.1 Inventory by layer

| Layer | Files | `#[test]` / `it()` count | Status |
|-------|-------|--------------------------|--------|
| `crates/solid-core` | 9 modules | 73 | implemented; 1 ignored (`b12_diagnostic_e2e_session_signature_verifies_host_side` -- forensic relic from pre-SEC-053) |
| `crates/solid-light` | 2 modules | 68 | implemented |
| `programs/zk-verifier` (`--lib`) | 1 file | 30 | implemented |
| `programs/issuer-registry` (`--lib`) | 1 file | 33 | implemented; +10 in Phase E |
| `programs/schema-registry` (`--lib`) | 1 file | 30 | implemented |
| `tools/solid-prover` | 1 file | 1 | implemented; SEC-017 closure |
| `tests/unit/` (TS standalone) | 3 files | ~50 axes | `artifact_integrity.test.ts` (SEC-058), `discriminator_recompute.test.ts` (SEC-071), `holder_debug_redaction.test.ts` (SEC-066) |
| `tests/integration/` | 1 file | 4 it-blocks | `01_registry_init.test.ts` only; 02..11 missing |
| `tests/vectors/` | gen_vectors.rs + check_vectors.ts + 1 fixture | 2 primitives | SEC-010 partial |
| `circuits/test/` | 7 mocha files | 45 | implemented |
| `ts-sdk/packages/*/jest` | 0 `*.test.ts` files | 0 | jest declared in 6 package.json `test` scripts; not installed; not authored |

### 6.2 Run results captured this session

| Command | Exit | Result |
|---------|------|--------|
| `cargo test -p solid-core` | 0 | **73 passed**, 0 failed, 1 ignored |
| `cargo test -p solid-light` | 0 | **68 passed**, 0 failed |
| `cargo test -p zk-verifier --lib` | 0 | **30 passed**, 0 failed |
| `cargo build-sbf -p issuer-registry` | 0 | clean (post-Phase-E) |
| `cargo build-sbf -p schema-registry` | 0 | clean |
| `cargo build-sbf -p zk-verifier` | 0 | clean |
| `cd ts-sdk && npm run build` | 0 | all 6 pkgs tsc-compile clean |
| `npm run check-vectors` (Rust↔TS vectors) | 0 | 2/2 vectors agree |
| `npm run e2e` (full pipeline) | **0** | **GREEN; verify_batch_proof_v2 confirmed on-chain** |
| `cargo fmt --all -- --check` | 1 | ~10-line wrap diff in `cpi_helpers.rs` |
| `cargo clippy -p solid-core -p solid-light -- -D warnings` | non-zero | 1 `dead_code` on `sas.rs:45::attester` |
| `anchor build` (full pipeline) | 1 | proc-macro2 1.0.94 SourceFile trap during HOST IDL compile (BPF outputs are correct) |
| `cd circuits && npm test` (mocha) | not run this session | per HEAD's commit message: 45/45 green |
| `cargo test -p issuer-registry --lib` | not run this session | per HEAD's commit message: 33/33 green |
| `cargo test -p schema-registry --lib` | not run this session | per HEAD's commit message: 30/30 green |

### 6.3 Aggregate

- **171/171 host tests verified GREEN this session** (solid-core + solid-light + zk-verifier).
- Per HEAD's commit message + RESUME.md: **279/279 total host + circuit** (171 + 33 + 30 + 45) at HEAD.
- **Cross-language vectors: 2/2 GREEN** (covering 2 of 10 SEC-010 primitives).
- **E2E: GREEN** (clean slate; verified-true tx confirmed on local validator).
- **Clippy / fmt CI gate: RED** -- two-line fix.

---

## 7. Recommended next-action ordering (tightest-payoff first)

| # | Action | Effort | Closes |
|---|--------|--------|--------|
| 1 | `#[allow(dead_code)]` on `sas.rs:45::attester` + `cargo fmt --all` | 5 min | NF-09 / Drifts 3,4 |
| 2 | Doc-drift refresh: README, CURRENT_STATE, IMPLEMENTATION_PLAN, CLAUDE.md (date placeholder + subgroup VK pin in invariants) | 1 hour | Doc drifts 1-6 |
| 3 | Reconcile NF-08: commit `scripts/measure_cu.py` + `cu_regression` workflow OR retract docs claim | 30 min - 2 hours | Drift 10 / NF-08 |
| 4 | Author thin `withdrawAfterRevoke` SDK helper + integration test 13 | 2 hours | Drifts 5, plus retire UNUSED status |
| 5 | Delete-or-document legacy `verify_batch_proof` env-flag fallback | 30 min | Drift 6 |
| 6 | Author bankrun integration test 12 (SEC-083 live constraint) | 2 hours | Drift 7 |
| 7 | Cross-language vectors expansion to 10/10 (gen_vectors.rs + check_vectors.ts) | 1 day | SEC-010 / Drift 1 |
| 8 | Integration suite 02..11 (10 scenarios; pick `litesvm-anchor` over solana-bankrun for friction; prioritise 03/04/06/07/08/09) | 5-10 days | Drift 2 |
| 9 | ts-sdk jest baseline (one suite per pkg) | 5 days | Drift 9 |
| 10 | Multi-party trusted setup ceremony for both batch + subgroup circuits, batched with SEC-006 P2 + SEC-051 one-line constraints | 1-2 weeks | SEC-012 + SEC-006 P2 + SEC-051 (mainnet-blocking) |
| 11 | Governance multisig (Squads 3-of-5) on `IssuerTreeBinding.operator` + slash/fraud authorities | 2-4 days | SEC-043 + SEC-013 + SEC-034 (mainnet-blocking) |

Items 1-6 are the 1-day "polish-the-current-state" sweep. Items 7-9 are the next-session-or-two work that retires the highest-leverage open backlog. Items 10-11 are the mainnet-blocking gates that need a coordinated ceremony + governance migration.

---

## 8. Reproducibility appendix

### 8.1 Hashes / pins captured this session

| Artifact | SHA-256 |
|----------|---------|
| Batch circuit VK | `8385b82b032f65e505c784b28486ca8bec7da3f3d4b97b82724e697734565146` |
| Subgroup circuit VK (Phase E) | `938ab39020f31156fa7e8fc230fc458adba5f13e08c64d41d9dbffdbc3643ce9` |
| Batch circuit wasm | `add8cb0390511405faf2ffb1213d3792b858c7b2082d5b4b92a84dcd627e61b4` |
| Batch circuit zkey | `7f43bbac249e8c913ac384ff4b007138d1ffb5bc489a16be42737598d96395e8` |
| Subgroup circuit wasm (Phase E) | `2c00e5a455a3b6fe1dc2a8761737acf2911baf396aab70165de3abd2673608b0` |
| Subgroup circuit zkey (Phase E) | `ea401ea9cbeb9ef829be30e0080b83a3c82544af53367b95a17f24a75845bd4a` |

These are all on disk under `circuits/build/*.sha256` and consumed via the SEC-058 pin chain in `ts-sdk/packages/sdk/src/artifact_integrity.ts:168-207`.

### 8.2 Live tx receipts (this e2e run)

- `issue_credential` -- `34chsEMpmEdTWedQjKQVrRJ8a694rmL5diR51M6NVzmvmDNVSj8GaTVE3f19YYf6e4MKXJEELp4xGDdiM3w1nLPS`
- `verify_batch_proof_v2` -- `5hckZo1xsRPrzt3obKG5cmVLV42psxdxTjhZzcJZjEyJAeJNo2wvxPLs6haJfSXNA327SZefbw2kz1qNM5k4WiHJ`
- ALT: `G7xUgmzB9rN1e6qbRt8rsuq8oqhCnTvs9mBc5b81ZAhv`
- Replay test: rejected (expected) by nullifier PDA init constraint

### 8.3 e2e step-by-step log

Full log available locally at `/tmp/e2e_step8_run.log` for the duration of the active validator. Validator log at `/tmp/e2e_validator.log`. Both will rotate on the next clean-slate run.

---

*End. The companion full-system audit is `sec/audits/2026-05-02_v0.6.1_post_phase_e_full_system_audit.md`.*
