# Resume -- where to pick up next session

Living handoff doc. Read this first when starting a new session.
Updated at the end of each session; the last-updated line is
authoritative.

- **Last updated:** 2026-04-25 end-of-session (Phase 3 impl 1-4
  landed: SEC-007, SEC-006 Part 1, SEC-041, SEC-044 all closed.
  v0.6.1 deep audit dropped in, registering SEC-045 + SEC-046
  as MEDIUM.  E2E runbook `docs/DEPLOYMENT_AND_TESTING.md`
  rewritten as the canonical install / build / deploy / run /
  verify / probe doc.)
- **Current branch:** `main`
- **Current phase:** Phase 2 closed; Phase 3 open
- **Discipline in force:** root-cause only, no regressions, no doc
  lies, one source of truth per artifact (see
  `plan/IMPLEMENTATION_PLAN.md` Section 0).

---

## Where we left off

**Phase 2 is closed.**  Both load-bearing soundness items (SEC-004
+ SEC-008) shipped end-to-end: on-chain atomic status transitions,
circuit rev with compressed issuer tree + 6-input nullifier, SDK
migration across every package, backfill script, E2E pipeline
green through `npm run e2e`.  Snapshot:
`sec/audits/2026-04-24_v0.6_phase2_closeout.md`.

Registry state: **22 open / 24 fixed / 46 total** (post v0.6.1
audit on 2026-04-25: SEC-006 Part 1 + SEC-007 + SEC-041 + SEC-044
closed earlier that day; the v0.6.1 external review then opened
SEC-045 and SEC-046 as MEDIUM; SEC-043 and SEC-044 were registered
in the morning Phase 3 doc sweep).  Severities: **CRITICAL 0
open**, HIGH 2 open, MEDIUM 12 open, LOW 4 open, INFO 4 open.

### Commits landed this Phase 2 session

| Commit    | What                                                         |
|-----------|--------------------------------------------------------------|
| `ad944ff` | Prelude: doc drifts, script gaps, safety guards (SEC-039/-040/-042 fixed). |
| `a9f0b9d` | ADR-0014: compressed issuer tree with BJJ-binding leaf.      |
| `4fba821` | CI harness: circuit_witness_tests + e2e_localnet.            |
| `58afb93` | Impl 1: IssuerTreeBinding PDA + solid-light parser.          |
| `73871dc` | Impl 2: circuit rev (issuer-tree inclusion + 6-input nullifier) + zk-verifier. |
| `df33ffe` | Impl 3a: 6-input nullifier across Rust/WASM/TS + IssuerAccount fields. |
| `167138e` | Impl 3b: atomic status-transition hooks + leaf lifecycle.    |
| (this commit) | Impl 3c: SDK + E2E pipeline + backfill + Phase 2 close-out. |

### Host-side baseline (green at close-out)

```
cargo test -p solid-core  --lib    ->  44/44
cargo test -p solid-light --lib    ->  25/25
cargo test -p zk-verifier --lib    ->  11/11
python3 scripts/check_program_ids.py  -> consistent
```

### Open registry (by priority)

```
HIGH (2 open)
  SOLID-SEC-010  Cross-language vectors narrow (covers 3 of ~10 primitives)
  SOLID-SEC-012  Trusted setup single-party (mainnet blocker)

MEDIUM (12 open)
  -013..-019, -021, -034  (governance + throughput + schema-hash
                            re-assertion + fraud-proof seed check)
  -043                    (IssuerTreeBinding.operator single signer;
                            gate behind Squads 3-of-5 before ext. audit)
  -045                    (atomic handlers don't update
                            IssuerTreeBinding.current_root in-ix;
                            v0.6.1 NEW-01; half-day P0)
  -046                    (no CU-budget regression gate on
                            verify_batch_proof; v0.6.1 NEW-02;
                            half-day P0)

LOW (4 open)   -022, -023, -024, -035
               (SEC-041 and -044 closed 2026-04-25 Phase 3 impl 3/4)

INFO (4 open)  -025, -026, -037, -038

See sec/SECURITY_REGISTRY.md for the full detail + remediation plan
on each.
```

---

## Next action (Phase 3 kick-off, sequenced)

### 1. ~~SOLID-SEC-006 -- VK freeze-gate~~ (Part 1 closed 2026-04-25)

Landed in Phase 3 impl 2.  ADR-0015.  `VerifierConfig` grew by 11
bytes (vk_finalized, vk_generation, rotate_request_ts; SPACE
49 -> 60).  Four new ix: `finalize_verification_key`,
`request_vk_rotation`, `cancel_vk_rotation`,
`rotate_verification_key`.  48-hour timelock.
`store_verification_key` now refuses every write against a
finalized VK.  Six host regression tests; zk-verifier 11 -> 17
green.  Part 2 (bind `vk_generation` into the public-input contract
and reject cross-VK replay) is deferred to the next trusted-setup
cycle where it batches with SEC-010 expansion.

### 2. ~~SOLID-SEC-007 -- BJJ subgroup check at registration~~ (closed 2026-04-25)

Landed in Phase 3 impl 1.  `crates/solid-core/src/babyjubjub.rs`
gained `is_in_prime_order_subgroup` + `require_in_prime_order_
subgroup`, and `pubkey_to_affine` + `verify()` now use
`EdwardsAffine::new_unchecked` with an explicit
`is_in_correct_subgroup_assuming_on_curve()` check.
`programs/issuer-registry::register_issuer` calls the new helper
before touching state, returning `ErrorCode::InvalidBJJPubKey`.
WASM side exposes `isBjjInPrimeOrderSubgroup`.  5 new Rust unit
tests; solid-core 44 -> 49 host tests green.

### 2a. SOLID-SEC-045 -- atomic handlers update `IssuerTreeBinding` in-ix

v0.6.1 NEW-01.  MEDIUM, P0 per v0.6.1 Section 6.1.  Half-day.

**Root cause.**  `revoke_issuer_atomic` and
`request_withdrawal_atomic` CPI `replace_leaf` into the SPL AC
tree (new root lands immediately) but leave
`IssuerTreeBinding.current_root` untouched.  The handlers document
"caller MUST invoke `update_issuer_tree_root` in the same tx",
which is enforcement-by-convention rather than
enforcement-by-code.  Until a follow-up `update_issuer_tree_root`
lands, a cached pre-transition proof still verifies against the
stale binding.  SEC-008's epoch-bound nullifier prevents
regenerating the same proof, but does not stop replay of a
pre-captured one.

**Change set.**  Promote `issuer_tree_binding` from
`UncheckedAccount` to a writable account in `RevokeIssuerAtomic`
and `RequestWithdrawalAtomic` contexts.  After `invoke_signed`
succeeds, derive the new root on-chain from the proof nodes in
`remaining_accounts` (or re-read from SPL AC changelog) and write
it plus `Clock::slot` into the binding's `current_root` +
`last_updated_slot` fields in the same ix.  Owner-check the
binding against `crate::ID`.  Update ADR-0014 amendment prose to
reflect the full atomicity claim.

**Regression gate.**  `tests/integration/07b_revoke_atomic_
binding_update.test.ts` plus mirror for `request_withdrawal_
atomic`.  Invoke atomic handler, then immediately attempt
`verify_batch_proof` with a pre-transition proof; expect
`IssuerTreeRootMismatch`, not success.

### 2b. SOLID-SEC-046 -- CU-budget regression gate on `verify_batch_proof`

v0.6.1 NEW-02.  MEDIUM, P0 per v0.6.1 Section 6.1.  Half-day.

**Root cause.**  No CI job measures `verify_batch_proof`'s CU
utilisation.  The compile-time stack-slice assertion at
`programs/zk-verifier/src/lib.rs:84-87` catches stack growth
only.  SEC-006 Part 2 (`vk_generation` public input) and SEC-010
primitives will bump CU at the next trusted-setup cycle; without
a baseline gate a future circuit change can push over the per-tx
CU ceiling silently.

**Change set.**  Add `.github/workflows/`
`verify_batch_proof_cu_baseline` CI job:
  1. Start localnet + run E2E through `issue.ts`.
  2. Construct `verify_batch_proof` tx with
     `ComputeBudgetProgram::set_compute_unit_limit(1_400_000)`.
  3. Parse "consumed X of Y compute units" from the tx log.
  4. Assert `X <= BASELINE * 1.10` (10% tolerance).
  5. Re-record baseline in `docs/CU_BUDGET.md` on sanctioned bumps.

**Regression gate.**  The CI job itself, plus the first-run
baseline recorded in `docs/CU_BUDGET.md`.  Trigger a conscious
budget-review on every sanctioned constraint addition.

### 3. SOLID-SEC-010 -- Extend cross-language vectors

Primitives still missing from `tests/vectors/`:
- Poseidon raw (fields + bytes)
- BJJ keypair generation
- BJJ sign / verify
- `derive_credential_key`
- `computeIdentityState`
- `QueryBuilder._computeContextHash`
- ADR-0014 issuer leaf (already implicitly covered via the WASM
  round-trip, but vector it explicitly)

Extend `crates/solid-core/examples/gen_vectors.rs` +
`tests/vectors/check_vectors.ts`.  Straightforward; blocks external
audit readiness.

### 4. ~~SOLID-SEC-041 -- Content-addressed VK artifact~~ (closed 2026-04-25)

Landed in Phase 3 impl 3.  `circuits/scripts/setup.js` now writes
`verification_key.sha256` next to the VK artifact and prints the VK
hash in the console summary.  `scripts/initialize.ts` enforces the
pin via `SOLID_VK_SHA256` env var (authoritative for release) or
the file (developer-loop convenience); refuses to upload when
either (a) the pin is missing, or (b) the computed hash does not
match the pin.

### 5. SOLID-SEC-012 -- Multi-party trusted setup

Mainnet blocker.  Must run a real ceremony with 10+ geographically
distributed contributors, each signing an attestation chain.
Artifacts (zkey, ptau, VK) hosted on IPFS + Arweave.  Not
one-sprint scope.

### 6. SOLID-SEC-043 -- gate issuer_tree_operator behind multisig

Replace the single-pubkey `IssuerTreeBinding.operator` with a PDA
signer (Squads 3-of-5 or SolID DAO threshold PDA). All four
tree-mutating ix (`update_issuer_tree_root`, `append_issuer_leaf`,
`replace_issuer_leaf`, `revoke_issuer_atomic`) wrap existing logic
behind that authority. Amend ADR-0014. Bundled with the wider
SEC-013 Squads migration.

### 7. ~~SOLID-SEC-044 -- request_withdrawal_atomic~~ (closed 2026-04-25)

Landed in Phase 3 impl 4.  `request_withdrawal_atomic` added to
issuer-registry; clones the `revoke_issuer_atomic` CPI flow with
`Cooldown` as the target status and the issuer's own keypair as
signer.  Legacy `request_withdrawal` now rejects enrolled issuers
with `IssuerTreeUpdateRequired`.  New `RevokeReason::
CooldownRequested` variant on `IssuerLeafReplaced`.  ADR-0014 now
carries the "Cooldown is verify-negative" amendment.

### 8. Governance cluster + LOW/INFO cleanup

SOLID-SEC-013..019, -034.  Squads 3-of-5 multisig, per-issuer stake
vaults, propose/accept authority transfer, etc.  Work separable
into small commits; good "warm-up" work while -012 is in flight.

### 9. External audit

After the items above close, schedule the external audit.  Per
Section 0 non-negotiable #5, mainnet slips one sprint after audit
close-out, not one sprint after submission.

---

## Verification steps on resume

```
cd /Users/rajakash/Desktop/testing/solid-protocol
git fetch origin
git log --oneline origin/main | head -10

cargo test -p solid-core  --lib           # expect 44/44
cargo test -p solid-light --lib           # expect 25/25
cargo test -p zk-verifier --lib           # expect 11/11
python3 scripts/check_program_ids.py      # expect "consistent"
head -3 Cargo.lock                        # must be version = 3
```

---

## Open decisions carried into Phase 3

1. **VK freeze-gate semantics.**  Should rotation require a DAO
   vote + 48h timelock (strongest), or registry-authority multisig
   only?  Ties into the SEC-013 Squads migration.  Decide in the
   ADR for SEC-006.

2. **Cooldown status semantics.**  Now tracked as SOLID-SEC-044
   (LOW, Open) in the registry.  Default Phase 3 stance: Cooldown
   disables proof verification; `request_withdrawal_atomic` mirrors
   `revoke_issuer_atomic` with a `replace_leaf` CPI.  Final ADR-0014
   amendment lands with the SEC-044 fix.

3. **`scripts/e2e_localnet` CI gate.**  Landed as scaffolding in
   `4fba821`.  Should it be a hard gate on every push (current),
   or graduated (nightly only)?  Resolution depends on runtime
   cost once we measure it on CI.

---

## Rules of engagement (reprinted)

Full list in `plan/IMPLEMENTATION_PLAN.md` Section 9.  Quick
reference:

1. Root cause only.  No workarounds, no suppression, no
   `--no-verify`, no `#[cfg(feature = "unchecked")]`.
2. No regressions.  Every fix ships a regression gate.
3. No doc lies.  Either the doc matches the code or the doc moves
   to `docs/archive/` with a HISTORICAL banner.
4. One source of truth per artifact.
5. PR review checklist: root cause? regression test? doc truth?
   single source of truth? ADR compliance? registry aligned?

Phase 1 held to all six.  Phase 2 held to all six.  Phase 3 must
too.
