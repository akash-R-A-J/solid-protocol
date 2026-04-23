# Resume -- where to pick up next session

Living handoff doc. Read this first when starting a new session.
Updated at the end of each session; the last-updated line is
authoritative.

- **Last updated:** 2026-04-23 end-of-session (Phase 1 CLOSED)
- **Current branch:** `main` at commit `bc971e6`
- **Current phase:** Phase 1 closed; Phase 2 open (see below)
- **Discipline in force:** root-cause only, no regressions, no doc
  lies, one source of truth per artifact (see
  `plan/IMPLEMENTATION_PLAN.md` Section 0).

---

## Where we left off

**Phase 1 is closed.**  All 14 items in the Phase 1 scope per
`plan/IMPLEMENTATION_PLAN.md` Section 4 are registry-status `Fixed`
with a landed commit, a regression-gate description, and (for every
host-reachable defect) a green host-side test.

Registry state post-Phase-1-close: 24 open / 14 fixed / 38 total.
After the Phase 2 prelude (same-session code review surfaced 4 new
items; 3 fixed in the prelude commit, 1 carried open):
**25 open / 17 fixed / 42 total**.
Severities (post-prelude): **CRITICAL 0 open**, HIGH 6 open,
MEDIUM 9 open, LOW 6 open, INFO 4 open.

Snapshot: `sec/audits/2026-04-23_v0.5_phase1_closeout.md`.
Prelude adds: SOLID-SEC-039 (bootstrap_issuer mainnet footgun --
Fixed), SOLID-SEC-040 (check_program_ids missing-manifest gap --
Fixed), SOLID-SEC-042 (VerifierConfig::SPACE doc drift -- Fixed),
SOLID-SEC-041 (VK artifact not content-addressed -- Open; Phase 2
scope).

### Host-side baseline (green at close-out)

```
cargo test -p solid-core  --lib    ->  43/43
cargo test -p solid-light --lib    ->  16/16  (+6 from SEC-003)
cargo test -p zk-verifier --lib    ->  11/11
python3 scripts/check_program_ids.py  -> consistent
```

### Commits landed this session (2026-04-23, after `9663506`)

| Commit    | Closes                                 | Key change |
|-----------|----------------------------------------|------------|
| `0c333fb` | SOLID-SEC-009                          | WASM bridge: one source of truth. CI builds `wasm/` into `ts-sdk/packages/core/wasm/`; SDK imports `../wasm/solid_wasm.js` relatively; unused `solid-core` `wasm` feature + deps deleted. New CI job `wasm_bridge_smoke` + `scripts/wasm_bridge_smoke.mjs`. |
| `8a31abf` | SOLID-SEC-003                          | `IssueCredential` context now requires seed-constrained `schema_account` + `schema_tree_binding` under schema-registry; parser helper in `solid-light` asserts embedded schema_hash / tree_pubkey / status. TS SDK `buildIssueCredentialIx` takes `schemaName` + `schemaVersion`.  6 new Rust unit tests. |
| `8ad20e3` | SOLID-SEC-011 (+ SEC-020 follow-up)    | zkey filename unified; `keccak256HashPair` -> `poseidonHashPair`; new `scripts/bootstrap_issuer.ts` (full 9-step DAO approval flow); `scripts/lib/e2e_state.ts` writes state to `$XDG_RUNTIME_DIR` / `$TMPDIR` (never in repo working tree). |
| `bc971e6` | SOLID-SEC-001, SOLID-SEC-029           | Batch circuit: range checks on `queryCredentialIndices` / `queryFieldIndices`. `IdentityAnchor` gains `enabled` input; `anchors[i].enabled <== 1 - isZero[i].out` so padding slots skip the global-tree inclusion proof. `compound_query.circom` updated to match template signature. |

Earlier commits in this session (pre-`9663506`) closed SEC-020, -027,
-028 (`174cf50`); -002, -005, -030 (`402fb4e`); -031, -033 (`4b37425`,
`6e46dda`); -032 (`d34d0ca`).

### Open registry (by priority)

```
HIGH (6 open)
  SOLID-SEC-004  No in-circuit issuer pubkey binding
  SOLID-SEC-006  VK overwrite has no freeze-gate
  SOLID-SEC-007  BJJ pubkeys not subgroup-checked
  SOLID-SEC-008  Nullifier lacks epoch / global root
  SOLID-SEC-010  Cross-language vectors narrow (2/10)
  SOLID-SEC-012  Trusted setup single-party (Phase 3 scope)

MEDIUM (9 open)
  -013..-019, -021, -034  (governance + throughput + schema-hash
                            re-assertion + fraud-proof seed check)

LOW (6 open)   -022..-024, -035, -036, -041
INFO (4 open)  -025, -026, -037, -038

See sec/SECURITY_REGISTRY.md for the full detail + remediation plan
on each.
```

---

## Next action (sequenced -- Phase 2 kick-off)

Work through in this order.  Each step lists the registry ID,
primary files, regression gate, and rough size.

### 1. Stand up the Phase 2 CI gates that Phase 1 listed as deferred

Before touching code, make the promised gates real so every Phase 2
fix can land behind a green check:

- **`e2e_localnet`** CI job: clean checkout -> anchor build ->
  `ts-node scripts/initialize.ts` -> `bootstrap_issuer.ts` ->
  `issue.ts` -> `prove.ts`.  Fails if any step errors.  Pairs with
  SEC-011.
- **Circuit witness property test** harness (mocha + snarkjs +
  circomlibjs): exercises out-of-range index rejection (SEC-001
  regression gate) and the 3-credential-batch-with-padding-slot
  happy path (SEC-029 regression gate).
- **Integration bankrun suite** skeleton (`06_*`, `07_*`) for SEC-003.

These are docs-promised gates that the Phase 1 registry entries
already reference.  Landing them is not optional.

### 2. SOLID-SEC-004 -- In-circuit issuer pubkey binding (LARGE)

**Root cause.**  `batch_credential_query.circom` accepts
`issuerPubKeyAxs/Ays` as private inputs; the verifier has no
cross-check; `check_issuer_status` exists but no CPI call.  Revoked
issuers' prior signatures still verify on-chain.

**Design choice pending.**  Two candidates (per RESUME.md from the
previous session + master-audit):
- (a) `verify_batch_proof` CPIs into
  `issuer_registry::check_issuer_status` for every credential slot.
  Pro: reuses existing on-chain state; no circuit change.
  Con: 4x CPI per proof; CU budget tight.
- (b) Compressed issuer tree with in-circuit Merkle-membership proof
  against a `GlobalIssuerBinding` root. Pro: O(1) on-chain; single
  root comparison.  Con: new circuit revision + trusted setup.

Pick ONE, write an ADR, implement.  Whichever is chosen, bundle the
circuit work (if any) with **Phase 2 trusted setup** (see item 5).

### 3. SOLID-SEC-008 -- Epoch-bound nullifier (MEDIUM circuit change)

**Root cause.**  `nullifier = Poseidon(masterKey, revocationNonce,
verifierAddress, queryContextHash, verifierNonce)`.  No
global-root / epoch term -> a nullifier computed against epoch N is
still accepted at epoch N+1 after root rotation, enabling
long-horizon replay.

**Change set.**  Add `epoch` (or `globalRoot`) as a 6th nullifier
input, expose it as a public input, assert against on-chain
`global_binding.last_updated_slot` / epoch counter.

Bundle circuit change with SEC-004 if (b) is chosen, otherwise this
is its own small circuit rev.

### 4. SOLID-SEC-006 -- VK freeze-gate (MEDIUM)

**Root cause.**  `store_verification_key` with `chunk_index=0`
silently overwrites existing VK bytes; no finalisation gate.  After
first full upload the VK should be immutable unless the DAO votes to
rotate.

**Change set.**  Add `verifier_config.vk_finalized: bool` (set true
on the last chunk with `finalize=true`); once true, only a
`rotate_verification_key` instruction (DAO-signed, 48h timelock) may
unset it.

### 5. Phase 2 trusted-setup revision

Required by SEC-004 (if option b) and SEC-008.  Still TESTNET (the
multi-party ceremony is SOLID-SEC-012, Phase 3).  Re-run
`circuits/scripts/setup.js`, log the new zkey sha256 in the PR body,
upload new VK through `initialize.ts`.

### 6. Remaining HIGH items

- **SOLID-SEC-007** (BJJ subgroup): add subgroup check at issuer
  registration; the pubkey must be a valid BJJ point of order r.
  Rust-only, no circuit change.
- **SOLID-SEC-010** (cross-language vectors): extend
  `crates/solid-core/examples/gen_vectors.rs` + TS caller to cover
  Poseidon raw, BJJ sign/verify, `derive_credential_key`,
  `computeIdentityState`, `QueryBuilder._computeContextHash`.
- **SOLID-SEC-012** (multi-party ceremony): Phase 3.  Do not block
  Phase 2 close-out on it.

### 7. Registry cleanup after Phase 2

Same pattern as Phase 1 close-out:
- Flip closed items `Open -> Fixed` with commit hashes + regression
  gate names in `sec/SECURITY_REGISTRY.md`.
- Append a new row to the History table.
- Write `sec/audits/<date>_v0.6_phase2_closeout.md`.
- Update `plan/IMPLEMENTATION_PLAN.md` Phase 2 status table.
- Update this file.

---

## Verification steps on resume

Before starting work, confirm the state matches what is written
above.

```
cd /Users/rajakash/Desktop/testing/solid-protocol
git fetch origin
git log --oneline origin/main | head -8

# Expected top 8:
#   bc971e6  Phase 1 (SEC-001, SEC-029): circuit range checks + anchor enable gate
#   8ad20e3  Phase 1 (SEC-011): E2E scripts work from a clean checkout
#   8a31abf  Phase 1 (SEC-003): bind issue_credential to registered schema + tree
#   0c333fb  Phase 1 (SEC-009): WASM bridge has one source of truth
#   9663506  plan: add RESUME.md for cross-session continuity
#   d34d0ca  Phase 1 (SEC-032): two-layer guard on SCHEMA_REGISTRY_ID_BYTES
#   6e46dda  Phase 1 Tier 3a: close SOLID-SEC-031, SOLID-SEC-033
#   a59e780  sec: post-pull scorecard 2026-04-23 -- 6/38 fixed, 32 open, priorities listed

cargo test -p solid-core  --lib            # expect 43/43
cargo test -p solid-light --lib            # expect 16/16
cargo test -p zk-verifier --lib            # expect 11/11
python3 scripts/check_program_ids.py        # expect "consistent"
```

If any of the above diverges, reconcile BEFORE starting new work.

---

## Open decisions (carried into Phase 2)

1. **SEC-004 implementation choice** (see Next Action item 2):
   CPI-to-check_issuer_status vs compressed issuer tree.  Needs
   ADR before implementation.

2. **`masterPublicKey` in `generateBatchProof` signature** (carried
   from Phase 1). The fix to SOLID-SEC-033 made the parameter
   redundant.  I kept it as a `void` discard for v0.2 API
   compatibility.  Decide in Phase 2:
   - (a) leave as-is and remove in a v1.0 major bump (ADR required).
   - (b) remove now and document the breaking change clearly.

3. **`scripts/e2e_state.json` legacy path**.  The new helper uses
   `$XDG_RUNTIME_DIR` / `$TMPDIR`; existing CI or local setups that
   relied on the repo-local path need to migrate.  No code still
   reads the legacy path (verified at close-out), but operator
   runbooks may need a note.

---

## Rules of engagement (reprinted)

Full list in `plan/IMPLEMENTATION_PLAN.md` Section 9. Quick
reference:

1. Root cause only. No workarounds, no suppression, no
   `--no-verify`, no `#[cfg(feature = "unchecked")]`.
2. No regressions. Every fix ships a regression gate.
3. No doc lies. Either the doc matches the code or the doc moves to
   `docs/archive/` with a HISTORICAL banner.
4. One source of truth per artifact.
5. PR review checklist: root cause? regression test? doc truth?
   single source of truth? ADR compliance? registry aligned?

Phase 1 held to all six. Phase 2 must too.
