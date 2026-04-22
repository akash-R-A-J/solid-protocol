# Resume -- where to pick up next session

Living handoff doc. Read this first when starting a new session.
Updated at the end of each session; the last-updated line is
authoritative.

- **Last updated:** 2026-04-23 end-of-session
- **Current branch:** `main` at commit `d34d0ca`
- **Current phase:** Phase 1 (Unbrick) -- mid-flight, 9/14 items closed
- **Discipline in force:** root-cause only, no regressions, no doc lies,
  one source of truth per artifact (see `plan/IMPLEMENTATION_PLAN.md`
  Section 0).

---

## Where we left off

Nine Phase 1 items landed across five clean commits. Registry state:
29 open / 9 fixed / 38 total. All host-side tests green:
`solid-core` 43/43, `zk-verifier` 11/11, `solid-light` 10/10. Python
CI gate `scripts/check_program_ids.py` passes. No workarounds shipped.

### Commits landed this session (2026-04-23)

| Commit    | Closes                                 | Key change |
|-----------|----------------------------------------|------------|
| `174cf50` | SOLID-SEC-020, -027, -028              | `.gitignore` + pre-commit hook; roadmap reconciliation; stale-doc archive; WASM build-path docs. |
| `402fb4e` | SOLID-SEC-002, -005, -030              | schema-hash integrity check re-enabled via shared `solid_core::schema::compute_schema_hash_from_parts`; `currentTimestamp` bound to `Clock::get()` with configurable skew in `VerifierConfig`; `stake_vault` rent-floor guard in `transfer_slashed_lamports`. |
| `4b37425` | SOLID-SEC-031, -033                    | new `bufToDecimalBE` helper for Solana pubkeys; cohesion check now compares per-schema derived key, not master. |
| `d34d0ca` | SOLID-SEC-032                          | two-layer guard on `SCHEMA_REGISTRY_ID_BYTES`: Rust host tests + extended `check_program_ids.py`. |

### Registry status board (open items only, ranked by priority)

```
CRITICAL (2 open)
  SOLID-SEC-001  Batch circuit query indices unconstrained (SOUNDNESS)
  SOLID-SEC-003  issue_credential missing schema + tree pubkey binding

HIGH (8 open)
  SOLID-SEC-004  No in-circuit issuer pubkey binding
  SOLID-SEC-006  VK overwrite has no freeze-gate
  SOLID-SEC-007  BJJ pubkeys not subgroup-checked
  SOLID-SEC-008  Nullifier lacks epoch / global root
  SOLID-SEC-009  WASM bridge fractured (CI + SDK path)
  SOLID-SEC-010  Cross-language vectors narrow
  SOLID-SEC-011  E2E scripts bugged
  SOLID-SEC-012  Trusted setup single-party

MEDIUM (10 open), LOW (5 open), INFO (4 open)
  See sec/SECURITY_REGISTRY.md for the rest.
```

Phase 1 scope covers 14 items total; five remain open:
`SOLID-SEC-001, 003, 009, 011, 029`.

---

## Next action (sequenced)

Work through in this exact order. Each step lists the registry ID,
the primary files, the regression gate, and a rough size.

### 1. SOLID-SEC-009 -- WASM bridge CI + SDK pinned path (MEDIUM size)

**Root cause.** `.github/workflows/ci.yml` builds `wasm-pack build
crates/solid-core` which produces an empty `pkg` because the real
bridge is at `wasm/src/lib.rs`. `ts-sdk/packages/core/package.json`
pins `file:../../../wasm/pkg`, which neither CI nor any script
creates. Doc was already fixed in Tier 1 (SOLID-SEC-028); this is
the code-layer fix.

**Change set.**
- `.github/workflows/ci.yml`: every `wasm-pack build crates/solid-core`
  -> `wasm-pack build wasm/` with the same `--out-dir
  ts-sdk/packages/core/wasm --release` target.
- `crates/solid-core/Cargo.toml`: either remove the dead `wasm`
  feature entirely, or leave it but add a comment explaining it is
  unused (the real bridge is `wasm/`).
- `ts-sdk/packages/core/package.json`: keep the pinned path but
  verify it matches what CI actually produces.

**Regression gate.** New CI job `wasm_bridge_smoke` (outline in
`plan/IMPLEMENTATION_PLAN.md` Section 6): build pkg from `wasm/`,
import from the SDK's pinned path, call
`computeHardenedNullifier`, assert non-zero. Locally, manual smoke:
`wasm-pack build wasm/ --target nodejs --out-dir
ts-sdk/packages/core/wasm --release && ls
ts-sdk/packages/core/wasm` should list `*.js`, `*.d.ts`, `*.wasm`.

### 2. SOLID-SEC-003 -- issue_credential must bind to registered schema + tree (MEDIUM-LARGE)

**Root cause.** `programs/issuer-registry/src/lib.rs:619-716` and
context struct at `:909-937`: the handler derives
`PDA(b"tree-authority", schema_hash)` but does NOT verify that
`schema_hash` corresponds to a registered `SchemaAccount` or that
`merkle_tree.key()` matches `SchemaTreeBinding.tree_pubkey`. An
approved issuer can spawn a rogue schema/tree universe.

**Change set.**
- Add `schema_account` + `schema_tree_binding` as required accounts
  on the `IssueCredential` context.
- Seed-constrain `schema_account` to `[b"schema",
  schema_account.name.as_bytes(), &[schema_account.version]]` under
  schema-registry.
- `require!(schema_account.schema_hash == schema_hash)`.
- `require!(merkle_tree.key() == schema_tree_binding.tree_pubkey)`.
  Read the binding's tree_pubkey from bytes `[40..72)` of the PDA
  (layout documented at `programs/schema-registry/src/lib.rs:6-30`).
- Add new variants to `ErrorCode`: `SchemaNotRegistered`,
  `TreeBindingMismatch`.

**Downstream.** `scripts/issue.ts` currently calls
`issueCredential` without these accounts -- it must be updated as
part of SOLID-SEC-011 (next step).

**Regression gate.** Bankrun integration tests in Phase 2 scope:
`integration_06_issue_credential_rejects_unregistered_schema`,
`integration_07_issue_credential_rejects_wrong_tree`. For Phase 1
ship, at minimum a Rust compile + a manual happy-path test in the
E2E script (SOLID-SEC-011).

### 3. SOLID-SEC-011 -- E2E scripts + bootstrap_issuer.ts (MEDIUM)

**Root cause.** Three concrete bugs plus a missing script:

(a) `circuits/scripts/setup.js:57` writes `circuit_final.zkey` but
`scripts/prove.ts:123` reads `batch_credential_query_final.zkey`.
Pick one and reconcile. Recommended: unify on
`batch_credential_query_final.zkey` since that matches the circuit
name and will be the expected filename after the SOLID-SEC-001 +
-029 setup rerun.

(b) `scripts/prove.ts:105` references `keccak256HashPair` but the
symbol is never imported. Actual requirement is Poseidon; use
`poseidonHashPair` from `@solid-protocol/core` consistently (check
if that's the exported name; if not, use `poseidonHashBytes` and
call it with two leaves).

(c) `scripts/issue.ts` lacks the issuer-approval bootstrap. On a
clean registry, `issueCredential` fails because the issuer is not
`Approved`. Add the full sequence -- split it into a new dedicated
script `scripts/bootstrap_issuer.ts` so `scripts/issue.ts` stays
focused on credential issuance:

```
scripts/bootstrap_issuer.ts:
  initialize_registry (if not already)
  register_issuer
  stake_tokens (or go via trust-anchor)
  vote_on_issuer (N times to pass the DAO threshold) OR
    approve_via_trust_anchor under a Government-tier anchor
  finalize_voting (if voted) to flip Pending -> Approved
  print: issuer authority pubkey, stake PDA, issuer PDA
```

**Also update for SOLID-SEC-003.** `scripts/issue.ts` must pass the
new `schema_account` and `schema_tree_binding` accounts when
constructing the `issueCredential` instruction.

**Secrets hygiene (SOLID-SEC-020 follow-up).** The scripts still
write plaintext keypairs to `scripts/e2e_state.json`. That file is
now .gitignore'd (Tier 1) and the pre-commit hook is in place, but
the write itself should move to `$XDG_RUNTIME_DIR` or `/tmp`.
Bundle into this task: change the state-file path, keep the hook as
defense-in-depth.

**Regression gate.** New CI job `e2e_localnet` (outline in
`plan/IMPLEMENTATION_PLAN.md` Section 6): clean checkout ->
bootstrap -> deploy -> bootstrap_issuer -> issue -> prove -> verify.
Green on every push.

### 4. SOLID-SEC-001 + SOLID-SEC-029 -- Circuit fixes + TESTNET trusted setup (LARGE)

Bundle both into ONE circuit revision and ONE trusted-setup run.
Phase 2 will bundle the SOLID-SEC-004, -008 circuit changes in its
own revision; do not try to bundle Phase 1 and Phase 2 circuit work
together -- that would delay Phase 1 closure.

**Change set (circuits).**

(a) SOLID-SEC-001 range checks. In `batch_credential_query.circom`
just after the public-input declarations (around lines 56-59) and
in `compound_query.circom` at the analogous point:

```
for (var i = 0; i < MAX_PREDICATES; i++) {
    component credIdxCheck_<i> = LessThan(8);
    credIdxCheck_<i>.in[0] <== queryCredentialIndices[i];
    credIdxCheck_<i>.in[1] <== NUM_CREDS;
    credIdxCheck_<i>.out === 1;

    component fieldIdxCheck_<i> = LessThan(8);
    fieldIdxCheck_<i>.in[0] <== queryFieldIndices[i];
    fieldIdxCheck_<i>.in[1] <== NUM_FIELDS;
    fieldIdxCheck_<i>.out === 1;
}
```

Note: `compound_query.circom` uses a single FieldSelector not
BatchFieldSelector; adapt the bound accordingly.

(b) SOLID-SEC-029 `IdentityAnchor.enabled` gate. In
`circuits/lib/identity_anchor.circom`:

```
// Add a new input:
signal input enabled;

// Change line 47:
globalInclusion.enabled <== enabled;
```

In `circuits/batch_credential_query.circom` where
`IdentityAnchor` is instantiated:

```
for (var i = 0; i < NUM_CREDS; i++) {
    anchors[i] = IdentityAnchor(GLOBAL_DEPTH);
    anchors[i].enabled <== 1 - isZero[i].out;   // new line
    anchors[i].masterIdentityKey <== masterIdentityKey;
    // ... existing wiring ...
}
```

**Change set (trusted setup, TESTNET-ONLY).**

`circuits/scripts/setup.js` already produces a single-party
ceremony. For Phase 1 the zkey is labeled TESTNET ONLY in the plan
(Phase 3 ships the multi-party mainnet version under
SOLID-SEC-012). Run:

```
cd circuits
npm install
node scripts/setup.js
```

Publish the resulting `.zkey` hash in the resulting PR body for
audit trail. Update `scripts/prove.ts`'s zkey filename constant
(see SOLID-SEC-011 sub-task (a)) to match whatever `setup.js` emits.

**Regression gate.** `circuit_range_check_property_test`: property-
based witness test with 1000 random out-of-range indices (must fail
witness generation). `circuit_padding_slot_requires_no_global_proof`:
3-credential batch with 1 padding slot generates a valid witness
without any zero-schema global-tree entry.

### 5. Registry + plan cleanup after Phase 1 closes

- Flip SOLID-SEC-001, -003, -009, -011, -029 to `Fixed` in
  `sec/SECURITY_REGISTRY.md` with commit hashes + regression test
  names.
- Bump the summary counts.
- Append a new row to the registry's History table for the Phase 1
  close-out audit snapshot.
- Write the snapshot:
  `sec/audits/2026-04-<DD>_v0.5_phase1_closeout.md` following the
  template used by `sec/audits/2026-04-22_v0.3_comprehensive_audit.md`.
- Update `plan/IMPLEMENTATION_PLAN.md` Section 4 Phase 1 status
  table: every item shows `Fixed` with commit hash.

---

## Verification steps on resume

Before starting work, confirm the state matches what is written
above.

```
cd /Users/rajakash/Desktop/testing/solid-protocol
git fetch origin
git log --oneline origin/main | head -6

# Expected top 6:
#   d34d0ca  Phase 1 (SEC-032): two-layer guard on SCHEMA_REGISTRY_ID_BYTES
#   6e46dda  Phase 1 Tier 3a: close SOLID-SEC-031, SOLID-SEC-033  (post-rebase hash may differ slightly if remote added commits)
#   402fb4e  Phase 1 Tier 2: close SOLID-SEC-002, SOLID-SEC-005, SOLID-SEC-030
#   174cf50  Phase 1 Tier 1: close SOLID-SEC-020, SOLID-SEC-027, SOLID-SEC-028
#   b6f3b40  sec+adr+plan: merge master-audit findings and seed decision + plan dirs
#   4741c77  docs: add MODULE_CONTRACTS.md -- full component contracts for every system module

cargo test -p solid-core --lib           # expect 43/43
cargo test -p zk-verifier --lib          # expect 11/11
cargo test -p solid-light --lib          # expect 10/10 (incl. SEC-032 tests)
cargo check -p issuer-registry -p schema-registry   # warnings are pre-existing
python3 scripts/check_program_ids.py      # expect "consistent" message
```

If any of the above diverges, reconcile BEFORE starting new work.
A divergence means either upstream changed something or my memory
of state is wrong.

---

## Open decisions

These came up during Phase 1 Tier 3a and are left for explicit
decision tomorrow rather than guessed:

1. **`masterPublicKey` in `generateBatchProof` signature.** The fix
   to SOLID-SEC-033 makes the parameter redundant (the function
   uses `masterPrivateKey` and derives everything internally). I
   kept it for v0.2 API compatibility with a `void` discard. Decide
   whether to:
   - (a) leave as-is and remove in a v1.0 major bump (ADR required).
   - (b) remove now and document the breaking change clearly.

2. **Phase 1 trusted setup ceremony output filename.** Either:
   - Rename `setup.js`'s output to `batch_credential_query_final.zkey`
     (match the prover's expectation).
   - Rename `prove.ts`'s expectation to `circuit_final.zkey` (match
     the setup script).
   I recommend the first -- the circuit name is more informative
   and future-proofs against multi-circuit repos -- but either works
   as long as the chosen name is used consistently across
   `setup.js`, `prove.ts`, `ts-sdk/packages/sdk/src/config.ts`, and
   any doc reference.

3. **`crates/solid-core` `wasm` feature.** The feature is declared
   in Cargo.toml but the module has zero `#[wasm_bindgen]` exports.
   Either:
   - Remove the feature entirely (cleaner; requires confirming no
     downstream depends on `solid-core/wasm`).
   - Keep the feature and add a single `wasm_version()` function so
     the feature has semantic meaning.
   Less load-bearing than items 1 and 2 -- can defer to Phase 2.

---

## Rules of engagement (reprinted)

Full list in `plan/IMPLEMENTATION_PLAN.md` Section 9. For quick
reference:

1. Root cause only. No workarounds, no suppression, no
   `--no-verify`, no `#[cfg(feature = "unchecked")]`.
2. No regressions. Every fix ships a regression gate.
3. No doc lies. Either the doc matches the code or the doc moves to
   `docs/archive/` with a HISTORICAL banner.
4. One source of truth per artifact.
5. PR review checklist: root cause? regression test? doc truth?
   single source of truth? ADR compliance? registry aligned?

The work done so far holds to all six. The remaining items should
hold to all six too.
