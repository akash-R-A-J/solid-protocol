# Resume marker — 2026-04-26 audit

Stopped: 2026-04-26 ~03:00 (user end-of-day).
HEAD when stopped: `59c99c2` (no commits made; this audit is read-only against the tree).
Pick up at: this file → then `06_roadmap/roadmap.md` → then act on Section 3.1 (cheapest first).

---

## What's done (Wave 1 + Wave 2 complete)

All audit files are persisted in `sec/audits/2026-04-26_v0.6.1_modular_audit/`:

```
00_README.md                                index + methodology
01_modular/
  01_zk_verifier.md                         orchestrator hand-audit (1261 LOC dissected)
  02_issuer_registry.md                     specialist re-dispatch (23 handlers, 12 findings)
  03_schema_registry.md                     specialist (10 handlers, 11 findings)
  04_solid_core.md                          orchestrator hand-audit (10 files, 10 findings)
  05_solid_light.md                         specialist (21 unit tests catalogued, 5 findings)
  06_wasm_bridge.md                         specialist (11 exports, 13 findings, 1 HIGH)
  07_circuits.md                            orchestrator hand-audit (459 LOC + 7 lib files)
  08_ts_sdk.md                              orchestrator hand-audit (8 TS files, 1 CRITICAL)
  09_scripts_tests_tools.md                 orchestrator hand-audit (CI + integration + prover)
02_integration/
  00_integration_audit.md                   cross-cutting + 5 trust contracts
03_dependencies/
  dependency_audit.md                       specialist (~30 findings DEP-*)
04_compute/
  cu_budget.md                              specialist (every handler CU-mapped, SEC-046 gate spec)
05_docs_drift/
  docs_drift.md                             20 drift items vs current code; NO doc edits yet
06_roadmap/
  roadmap.md                                priority-ordered action list (mainnet blockers + close-outs)
  orchestrator_zk_verifier_handcheck.md     duplicate of 01_zk_verifier.md (kept for posterity)
```

---

## Highest-priority findings (open)

**Mainnet blockers (4)**
1. **SEC-012** — single-party trusted setup; needs multi-party ceremony.
2. **SEC-007 / SEC-048** — BJJ subgroup check on-chain costs >1.4M CU; bypassed via `sec007-skip-onchain` Cargo feature; off-chain TS predicate is load-bearing.
3. **SEC-045 / M02-H01** — atomic handlers (`revoke_issuer_atomic`, `request_withdrawal_atomic`) don't update `IssuerTreeBinding.current_root` in same ix; pre-revocation replay window. Hours-to-fix.
4. **SEC-043** — `issuer_tree_operator` is a single signer; needs Squads 3-of-5 / DAO threshold PDA.

**External-audit blockers**
- SEC-010 / M06-H01 — extend cross-language vectors 2 → 9 primitives.
- SEC-046 / CU-M01 — CU regression gate in CI (spec ready in `04_compute/cu_budget.md` §5).
- M02-H02 — no CI gate against `sec007-skip-onchain` shipping to mainnet.
- M09-H01 — 10 of 11 integration tests not implemented.
- M08-CRITICAL01 — `holder.generateProof` (single-credential) broken at runtime (Poseidon arity 16>12); confirm dead, delete.

**Dependency HIGH**
- DEP-H01..H04 — light-poseidon pin-to-exact, groth16-solana dual ark trees, ed25519-dalek 1.0.1 (RUSTSEC-2022-0093), curve25519-dalek 3.2.1 (RUSTSEC-2024-0344).

---

## Wave 3 (NOT done — pending user approval)

`docs/MODULE_CONTRACTS.md` reconciliation. The 20 drift items are listed in `05_docs_drift/docs_drift.md`. **No doc edits made yet** — user said code is source of truth; docs follow once delta is approved.

Highest-impact drifts to fix:
- DRIFT-01, -03, -04, -05: schema-registry `GlobalStateBinding` layout + handler signatures in MODULE_CONTRACTS.md.
- DRIFT-06: circuit BE/LE comment for `issuerAuthority`.
- DRIFT-08: zk-verifier slot doc-comment notation.
- DRIFT-12: TS SDK "31 inputs" → "32".
- DRIFT-14: this audit's `01_zk_verifier.md` mis-stated nullifier preimage; patch.

---

## Recommended pick-up sequence (when fresh)

**~30 min**: read `06_roadmap/roadmap.md` end-to-end, decide which lane to start.

**Easiest first PR (1-2 hours)**: Section 3.1 (dependency hygiene). All manifest-only edits, low risk:
- Drop unused `hex` from `crates/solid-light/Cargo.toml:14`.
- Drop unused `ark-groth16` from workspace `Cargo.toml:45`.
- Drop unused `serde_json` from `wasm/Cargo.toml:19` (verify with `cargo udeps -p solid-wasm`).
- Drop unused `num-traits` from `crates/solid-core/Cargo.toml:45`.
- Tighten `light-poseidon = "0.2"` to `=0.2.0`.
- Pin `proc-macro2 = "=1.0.94"` in workspace deps.
- Commit `tools/solid-prover/Cargo.lock`.
- Unify snarkjs to `^0.7.5` across all four manifests.

**Highest soundness value (4-6 hours)**: Section 1 Blocker 3 (SEC-045 atomic-handler binding root). Pattern is in `programs/issuer-registry/src/lib.rs:1015-1053` (`update_issuer_tree_root` writes `[40..72]` + `[72..80]`); inline that write into `revoke_issuer_atomic` and `request_withdrawal_atomic` after the `replace_leaf` CPI.

**Highest leverage (1-2 days)**: Section 2.1 (extend cross-language vectors 2→9). Single-PR effort across `crates/solid-core/examples/gen_vectors.rs` + `tests/vectors/check_vectors.ts`. Closes the largest external-audit blocker.

---

## Investigation items (resolve when you can)

- `circuits/compound_query.circom` (275 LOC) — alive or dead? If dead, delete (and delete `holder.generateProof` with it). Cross-ref M07-INVESTIGATE-02 + M08-H01.
- Confirm CI workflow runs `tests/vectors/check_vectors.ts` (M09-M01) and `scripts/check_program_ids.py` (M09-M02). Skim `.github/workflows/ci.yml` for `run: ts-node tests/vectors/check_vectors.ts` and `run: python3 scripts/check_program_ids.py`.
- Confirm `tools/solid-prover` is NOT in production hot path (snarkjs is). M09-H02 downgrades to "document non-prod" if confirmed.

---

## Notes for tomorrow-you

- Code is source of truth; docs/* are reference only. Every audit finding is grounded in `file:line`.
- The orchestrator's `01_zk_verifier.md` has one known error (nullifier preimage description) — patch ready in DRIFT-14.
- Three of the dispatched specialist agents (zk-verifier, scripts, ts-sdk, solid-core, circuits, issuer-registry) stalled out at 600s. Re-dispatch is wasteful; the orchestrator hand-audits replaced them.
- Wave 1 totals: ~315 KB of audit prose across 11 files.
- Sleep well.

---

End of resume marker. Open `06_roadmap/roadmap.md` to start tomorrow.
