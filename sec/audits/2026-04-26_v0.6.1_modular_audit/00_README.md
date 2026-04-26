# Modular + Integration Audit — solid-protocol v0.6.1

Date: 2026-04-26
Branch: main (HEAD: 59c99c2 docs: fold v0.6.1 audit into the registry + pointer graph)
Auditor: Claude Opus 4.7 (max effort) + delegated specialist agents
Source of truth: code under `programs/`, `crates/`, `wasm/`, `circuits/`, `ts-sdk/`, `scripts/`, `tests/`, `tools/`.
Reference only (not source of truth): `docs/`, `adr/`, `sec/` (existing audits).

This audit treats the code as the only authoritative artefact. Docs and ADRs are read for narrative context but every claim in this audit is grounded in the current contents of the source tree at HEAD 59c99c2 on 2026-04-26.

---

## Folder layout

```
sec/audits/2026-04-26_v0.6.1_modular_audit/
  00_README.md                          this file
  01_modular/
    01_zk_verifier.md                   Anchor program: programs/zk-verifier/
    02_issuer_registry.md               Anchor program: programs/issuer-registry/
    03_schema_registry.md               Anchor program: programs/schema-registry/
    04_solid_core.md                    crate: crates/solid-core/
    05_solid_light.md                   crate: crates/solid-light/
    06_wasm_bridge.md                   crate: wasm/ (solid-wasm)
    07_circuits.md                      circuits/*.circom + lib + scripts + tests
    08_ts_sdk.md                        ts-sdk/packages/* (core, holder, issuer, light, sdk, verifier)
    09_scripts_tests_tools.md           scripts/, tests/, tools/solid-prover/
  02_integration/
    00_integration_audit.md             cross-program, Rust<->TS, circuit<->verifier, end-to-end
  03_dependencies/
    dependency_audit.md                 Cargo.lock + npm lockfiles, CVE/yanked/MSRV
  04_compute/
    cu_budget.md                        per-instruction CU envelope, hot paths, SOLID-SEC-046
  05_docs_drift/
    docs_drift.md                       every doc claim vs current code
  06_roadmap/
    roadmap.md                          done / open / next, ordered with reasoning
  raw/                                  raw command outputs (git, find, listings)
```

---

## Methodology

**Wave 1 — parallel specialist audits.** Eleven agents, one per module / cross-cutting concern. Each reads every file in its scope at line level, dissects every function (inputs / processing / outputs / failure modes), and emits findings into its dedicated file. Code is the source of truth.

**Wave 2 — synthesis.** Cross-cutting integration audit, docs-vs-code drift pass, prioritized roadmap. Built from wave-1 outputs plus the orchestrator's hand-audit.

**Wave 3 — verification + critical-path deep dive.** Direct hand-audit of the most security-critical surface (zk-verifier handler body, public-input layout end-to-end, VK lifecycle, atomic handler invariants, BJJ subgroup workaround, 6-input nullifier preimage) against the source. Sanity-checks the agents' findings.

---

## Conventions

- `file:line` for every code reference. Quote the actual excerpt where it matters.
- ASCII markdown only. No emoji, no box-drawing characters (matches `CLAUDE.md` doc convention).
- Each finding has: ID (e.g., `M01-H01` = module 01, High-severity #1), severity, location, code excerpt, impact, recommendation.
- "Why is this here?" — every non-obvious construct gets a short reasoned answer.
- Push back on workarounds; flag every regression risk; ask "what could go wrong?" of every input boundary.
- Compute-aware: every on-chain hot path includes a CU consideration (or an explicit "no concern" with reasoning).
- Dependency-aware: stale, yanked, or CVE-laden crates are findings.

---

## Severity rubric

- **Critical** — soundness or replay vector that defeats a privacy / integrity guarantee, or panics on adversarial input.
- **High** — defense-in-depth gap, missing owner-check, feature flag that flips an invariant in production.
- **Medium** — workaround shipped in default builds, regression risk that CI does not catch, dependency CVE without an exploit path.
- **Low** — lint, dead code, undocumented behavior with no soundness impact.
- **Info** — observation worth recording (does not yet rise to a finding).

---

## Hard invariants under test

These come from `CLAUDE.md` and `adr/`. Every one is verified against current code.

1. `declare_id!` literals in `programs/*/src/lib.rs` agree with `Anchor.toml` and `deployments/*.json`.
   - zk_verifier:      `DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb`
   - issuer_registry:  `5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx`
   - schema_registry:  `4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1`
2. Rust and TypeScript primitives are byte-identical (cross-language vectors gate, `tests/vectors/`).
3. On-chain verifier owner-checks `global_tree`, `schema_tree_N` (against `schema_registry`) and `issuer_tree_binding` (against `issuer_registry`) — ADR-0010 + ADR-0014.
4. `VerifierConfig` SPACE = 60: `vk_finalized 1`, `vk_generation 2`, `rotate_request_ts 8`, `next_vk_chunk 2`, `timestamp_skew_seconds 4` (post ADR-0015).
5. `circuits/batch_credential_query.circom` `NR_PUBLIC_INPUTS = 32` (post ADR-0014 +1 shift; was 31). `issuerTreeRoot` at index `[10]`. The constants `ISSUER_TREE_ROOT_INPUT_INDEX`, `VERIFIER_ADDRESS_INPUT_INDEX`, `CURRENT_TIMESTAMP_INPUT_INDEX` in `programs/zk-verifier/src/lib.rs` are the handler's single source of truth (ADR-0012).
6. Nullifier preimage = 6-input Poseidon (ADR-0006 revision; `issuerTreeRoot` added so revoking any issuer invalidates every pre-revocation proof's nullifier universe — SOLID-SEC-008).

---

## Known active workarounds (must surface in the roadmap)

- `sec007-skip-onchain` Cargo feature on `issuer-registry` — bypasses `babyjubjub::require_in_prime_order_subgroup` because the cofactor-8 check costs >1.4M CU on BPF (exceeds the ~1.4M per-tx ceiling). Off-chain TS predicate `isInPrimeOrderSubgroup` is currently load-bearing while this is enabled. Tracked at SEC-048.

---

## Open SOLID-SEC items (per `CLAUDE.md`)

- SEC-006 part 2 — VK generation in public-input contract (circuit change pending; batches with next trusted-setup cycle).
- SEC-010 — extend cross-language vectors 3/10 -> 10/10 primitives.
- SEC-043 — `issuer_tree_operator` is single signer (must move to multisig before mainnet).
- SEC-045 — atomic handlers do not update `IssuerTreeBinding.current_root` in the same ix; pre-transition proofs replayable until `update_issuer_tree_root` runs separately.
- SEC-046 — no CU regression gate in CI.
- SEC-007 — see workaround above.
- SEC-012 — multi-party trusted-setup ceremony (mainnet blocker).
- Integration test scenarios 02..11 — specified, none implemented.
- Revocation v1 — circuit + on-chain support landed; SDK helpers + indexer event contract still required.

---

## Code volume in scope (LOC)

- `programs/issuer-registry/src/lib.rs`              2497
- `programs/zk-verifier/src/lib.rs`                  1261
- `programs/schema-registry/src/lib.rs`               599
- `crates/solid-core/src/*` (10 files)              ~2370
- `crates/solid-light/src/*` (3 files)                890
- `wasm/src/lib.rs`                                   299
- `circuits/*.circom + lib + setup`                  ~890
- `ts-sdk/packages/*/src/*` (8 files)                2614
- `tools/solid-prover/src/lib.rs`                       ?
- `scripts/*` (orchestration)                           ?
- `tests/integration/* + tests/vectors/*`               ?

Total in-scope source ~12,285 LOC excluding `tools/solid-prover`, `scripts/`, `tests/`.

---

## Status

- Wave 1 dispatched: 2026-04-26
- Wave 2: pending
- Wave 3: pending (interleaved with wave 1)
- Final roadmap: pending
