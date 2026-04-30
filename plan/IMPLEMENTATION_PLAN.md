# SolID Protocol -- Implementation Plan

Living plan. Single source of truth for scope, phasing, and
acceptance. Every item references `SOLID-SEC-NNN` in
`sec/SECURITY_REGISTRY.md` and/or `ADR-NNNN` in `adr/`.

- Protocol version in scope: v0.6.1 (Phase 3 impl 1-4 closed) ->
  v1.0 mainnet
- Last revision: 2026-04-28 (circuit/ZK audit + e2e bring-up).
  Five SEC findings landed in code: **SOLID-SEC-045** (atomic
  binding update via on-chain Keccak path-recompute helper),
  **SOLID-SEC-049 NEW** (`SPL_AC_REPLACE_LEAF_DISCRIMINATOR`
  was wrong; latent because no integration test had ever
  exercised revoke / cooldown -- caught by the discriminator-
  derivation regression test added in the program test sweep),
  **SOLID-SEC-050 NEW** (batch circuit schema-canonicality
  bypass; circuit constraint added; trusted-setup re-run with
  new VK pin
  `8385b82b032f65e505c784b28486ca8bec7da3f3d4b97b82724e697734565146`),
  **SOLID-SEC-052 NEW partial** (BPF / cross-layer coord-form
  drift; `is_on_curve` rewritten + WASM bridge re-emitted +
  process gate added at `docs/E2E_BLOCKERS.md` B11),
  **SOLID-SEC-053 NEW** (EdDSA-Poseidon cofactor-8 mismatch
  between off-chain `sign` and circomlib's in-circuit
  `EdDSAPoseidonVerifier`; gate that unlocked Groth16
  witness-gen during `npm run prove`).  Test counts:
  169/169 cargo + 39/39 circuit witness-tester (mocha) +
  1 host-side ignored forensic snapshot.  E2E pipeline
  status: `init-onchain -> backfill-issuer-tree ->
  bootstrap-schema-tree -> bootstrap-issuer -> issue ->
  prove (Groth16 generated)` all green; live edge is **B13**
  (`verify_batch_proof` ix data 1324 bytes exceeds Solana's
  1232-byte legacy-tx wire size; architectural blocker for
  on-chain submission).  See `plan/RESUME.md` "2026-04-28"
  section for full receipts; next-session pickup is the
  B13 remediation choice (recommended path: reconstruct
  the 11 redundant public inputs on-chain from accounts already
  passed to the ix; saves 11 * 32 = 352 bytes; total shrinks
  from 1324 to 972 bytes; `currentTimestamp` and `verifierNonce`
  stay on the wire because they are witness-bound).
- Prior revision: 2026-04-27 (Phase 3.4 circuit + SDK alignment).
  Landed: `LessThanBN254` + `BabyPbk254` in circuits; Rust fixture
  generator `gen_circuit_vectors`; mocha harness under `circuits/test/`
  (lt / babypbk / identity anchor) wired into the
  `cross_language_vectors` CI job; BabyJubJub **circomlib-native vs
  arkworks-normalized** coordinate isomorphism in
  `crates/solid-core/src/babyjubjub.rs`; pinned field elements use
  `Fq::from_le_bytes_mod_order` + `const [u8;32]` (no `MontFp!`) for
  rust-analyzer stability.  Snapshot rows updated in
  `docs/CURRENT_STATE.md` §2 / §5.2--5.3 / §6.
- Previous revision: 2026-04-25 (Phase 3 impl 1-4: SEC-007 + SEC-006
  Part 1 + SEC-041 + SEC-044 all flipped Fixed; v0.6.1 audit
  dropped in, registering SEC-045 and SEC-046 as MEDIUM;
  canonical state-of-protocol snapshot at
  `sec/audits/2026-04-25_v0.6.1_deep_comprehensive_audit.md`).
  Late-session addendum 2026-04-25: SOLID-SEC-007 partially
  reopened on-chain as **SOLID-SEC-048** (BJJ prime-order subgroup
  check exceeds the 1.4M CU per-tx ceiling on BPF; localnet/devnet
  runs an interim feature-gated bypass; **mainnet deploy-blocker**;
  promoted to Phase 4 P0 in `docs/FORWARD_ROADMAP.md` and P0-7 in
  `docs/IMPROVEMENTS_ROADMAP.md`; tracked end-to-end in
  `docs/E2E_BLOCKERS.md` B9 + O7 and in this plan's §1.16 below).
- Next revision trigger: B13 (`verify_batch_proof` legacy-tx
  wire size) close-out (recommended remediation: on-chain
  reconstruction of redundant public inputs); SEC-046 CU gate
  (pairs with B13 since on-chain reconstruction adds CU);
  SEC-010 cross-language vectors 3->10; integration suite 02..11
  bankrun harness stand-up; SEC-051 + SEC-006 Part 2 + predicate-
  operand range checks (next sanctioned trusted-setup cycle).

---

## 0. Objective and non-negotiables

### Objective

Take SolID from its credible-v0.3/v0.4-testnet state to a mainnet-
ready production identity protocol that can be honestly called
infrastructure: sound, low-latency, decentralized, scalable,
fault-tolerant, and with distributed authority. Ship in phases
with a working E2E flow at the end of Phase 1 and a governance-
mature, externally-audited mainnet at the end of Phase 3.

### Non-negotiables

1. **Root-cause fixes only.** No workarounds, no suppression, no
   "log and continue", no `#[cfg(feature = "unchecked")]` escape
   hatches, no doc-only remediations for code-level issues.
2. **No regressions.** Every fix ships a regression test that gates
   CI. The test must stay green on `main` for one full sprint before
   a finding can be flipped `Fixed -> Verified`.
3. **No doc lies.** Either the doc matches the code or the doc moves
   to `docs/archive/` with a `HISTORICAL -- DO NOT USE` banner.
4. **One source of truth per artifact.** WASM bridge, program IDs,
   VK, circuit build outputs, and deployment manifests each have
   exactly one canonical location.
5. **External audit is not a rubber stamp.** If the external audit
   surfaces any CRITICAL-equivalent finding, the mainnet date slips.
   Schedule mainnet against audit close-out plus one sprint, not
   against audit submission.

> **Standing exception register.** The non-negotiables above
> ADMIT NO EXCEPTIONS for production / mainnet artifacts.  They
> permit one explicitly-named, time-boxed, infrastructure-gated
> exception for development clusters only:
>
>   * **SOLID-SEC-048 (`sec007-skip-onchain` Cargo feature on
>     issuer-registry, lit 2026-04-25).**  Violates rule (1) on
>     localnet/devnet only -- the feature exists *because* the
>     root-cause fix for the BPF CU exhaustion is multi-week
>     (circuit constraint, SDK cofactor-clear, or `sol_babyjubjub_*`
>     syscall) and `npm run e2e` cannot otherwise run.  The
>     feature is opt-in (no default); mainnet builds MUST reject
>     it via a CI gate (P0-7 deliverable in
>     `docs/IMPROVEMENTS_ROADMAP.md`); a `Sec007Bypass` event is
>     emitted on every triggering call and a production-cluster
>     monitor MUST alert on observation.  Exception lifts on
>     SOLID-SEC-048 close (real fix landed AND
>     `tests/integration/register_issuer_compute_units.test.ts`
>     green on a no-feature build).  Any further candidate
>     exception goes through the same form: registered finding +
>     CI mainnet-build rejection + on-chain telemetry +
>     time-boxed close criteria.  See §1.16 and §6 row "SEC-048".

---

## 1. Current state snapshot (as of 2026-04-27; Phase 3.4 circuit hardening included)

### Done and verified in code

Cross-checked against code and
`sec/audits/2026-04-25_v0.6.1_deep_comprehensive_audit.md`
(supersedes the v0.6 audit), plus Phase 3.4 items below verified in
`cargo test`, `cargo run --example gen_circuit_vectors`, and
`circuits/npm test` (also gated in CI `cross_language_vectors`).

- RegistryConfig space 112 bytes; matches struct.
- Owner-check on `global_tree` and `schema_tree_N` against
  `SCHEMA_REGISTRY_ID` (ADR-0010).
- `vote_on_issuer` increments `active_votes_count` with overflow
  check; enforces deadline; 100-slot flash-loan window.
- Nullifier decimal-not-hex parsing.
- `@solid-protocol/verifier` package (`buildVerifyBatchProofIx`,
  `verifyOnChain`, PDA helpers, `checkIssuerStatus`).
- `ReleaseVote` accounts marked `mut`; `IncrementUsage` gated;
  slashed-lamport transfer; `approve_via_trust_anchor` emits event
  + seed-constrained.
- VK chunk ordering with `next_vk_chunk` cursor (ADR-0008).
- `VerifierConfig::SPACE = 60` matches struct post ADR-0015
  (SEC-042 drift closed at 49 in Phase 2; Phase 3 impl 2 grew the
  struct by 11 bytes for the freeze-gate fields).
- `paused` flag and `set_paused`.
- 32 public-input contract (ADR-0012 revised by ADR-0014;
  `ISSUER_TREE_ROOT_INPUT_INDEX = 10`,
  `VERIFIER_ADDRESS_INPUT_INDEX = 29`,
  `VERIFIER_NONCE_INPUT_INDEX = 30`,
  `CURRENT_TIMESTAMP_INPUT_INDEX = 31`).
- Per-schema derived credential keys at circuit level (ADR-0005).
- Canonical schema ascending ordering.
- Phase 3.4: `LessThanBN254` replaces the old `LessThan(252)` /
  `Num2Bits(253)` ordering path in `batch_credential_query.circom`;
  `BabyPbk254` replaces `BabyPbk` in `identity_anchor.circom` for
  full-domain `credPriv` scalars; isolated mocha templates + Rust
  fixture `circuits/test/fixtures/circuit_vectors.json` (generator
  `crates/solid-core/examples/gen_circuit_vectors.rs`) lock the
  SDK-to-circuit byte contract (including bit-253-set regression
  vectors).
- BabyJubJub: circomlib-native wire form vs ark-ed-on-bn254 normalized
  curve model bridged in `babyjubjub.rs` via fixed `sqrt(168700)` /
  inverse; pinned `Fq` values use LE byte arrays + `from_le_bytes_mod_order`
  (rust-analyzer-safe; same constants as the former `MontFp!` literals).
- Atomic nullifier replay via `init` (ADR-0007).
- Hardened 6-input nullifier (ADR-0006 revised by ADR-0014;
  Poseidon(masterKey, revocationNonce, verifierAddress,
  queryContextHash, verifierNonce, issuerTreeRoot)).
- Compressed issuer tree with BJJ-binding leaf (ADR-0014); atomic
  `revoke_issuer_atomic`; legacy paths refuse when
  `enrolled_in_tree`.
- Monotonic root updates.
- Program-ID agreement CI-enforced (`scripts/check_program_ids.py`
  now also validates `deployments/<cluster>.json`; SEC-040 fix).
- 80 host tests passing across solid-core (44), solid-light (25),
  zk-verifier (11) at commit `f09b093`.
- Cross-language vectors CI gate (narrow: 3/10 primitives post-
  Phase-2 -- see SOLID-SEC-010).
- Circuit witness tests CI job (`circuit_witness_tests`) landed in
  Phase 2 with property tests for SEC-001 range checks and SEC-029
  padding-slot integrity.
- `e2e_localnet` CI job runs the full `npm run e2e` pipeline on
  every push.
- Dedicated `sec/`, `adr/`, `plan/` directories (ADR-0013).

### Shipped incomplete

- Revocation v1: circuit done, on-chain done (atomic via
  `revoke_issuer_atomic` + issuer-tree `replace_leaf`); holder SDK
  helper, issuer SDK helper, `RevocationEvent` wiring still missing
  (not a soundness gap; adoption gap).
- Integration tests: 1 of 11.
- `ts-sdk/packages/sdk/src/config.ts:35-36` placeholders.
- No schema-registry events for mutable state transitions.
- No indexer / `HeliusDasAdapter`.
- `docs/IMPROVEMENTS_ROADMAP.md` reconciled at each phase close
  (last sweep 2026-04-25).

### Blocked

Each blocker is a registry entry with a root-cause fix in Phase 1:

- SOLID-SEC-001, SOLID-SEC-002, SOLID-SEC-003 (CRITICALs).
- SOLID-SEC-005 (`currentTimestamp` unbound).
- SOLID-SEC-009 (WASM bridge CI/SDK fracture).
- SOLID-SEC-011 (E2E scripts bugged).
- SOLID-SEC-028 (WASM build path in docs).
- SOLID-SEC-029 (`IdentityAnchor` padding over-constrained).
- SOLID-SEC-030 (`stake_vault` GC edge).
- SOLID-SEC-031 (pubkey endianness -- potential universal verify
  failure).
- SOLID-SEC-032 (load-bearing constant unvalidated).
- SOLID-SEC-033 (cohesion catch-22).

No live devnet deploy exists (`deployments/devnet.json` has
all-null deploy fields).

### Open (not started)

- In-circuit issuer binding (SOLID-SEC-004).
- Multi-party ceremony (SOLID-SEC-012).
- External audit.
- VK versioning + grace (SOLID-SEC-006).
- Nullifier scale-escape design (SOLID-SEC-018 rent math).
- Multisig governance (SOLID-SEC-013).
- Native mobile prover; W3C VC translation; recursive SNARKs;
  cross-chain anchoring.

### 1.16 Active interim bypass -- SOLID-SEC-048 (BJJ subgroup BPF CU)

**Status as of 2026-04-25 late-session:** lit on the working tree;
mainnet deploy-blocker; promoted to Phase 4 P0 (top of list); no
P1/P2 work in any phase advances against this until the real fix
(or its regression-gate proxy) is in CI.

**What is bypassed.**  `register_issuer` no longer calls
`solid_core::babyjubjub::require_in_prime_order_subgroup` when the
issuer-registry crate is built with the `sec007-skip-onchain`
Cargo feature.  Localnet/devnet/CI builds enable it; mainnet
builds MUST NOT.  The on-chain consolation gate is
`is_on_curve(pk) && !is_identity(pk)`; this rejects off-curve
garbage and the Edwards neutral element but DOES NOT reject
cofactor-8 torsion points -- a custom caller that bypasses the
off-chain SDK can still register a compromised key.

**Where the soundness now lives.**  The off-chain TS predicate
`isInPrimeOrderSubgroup` in `ts-sdk/packages/core/src/index.ts`
is the load-bearing SEC-007 gate while SOLID-SEC-048 is open.
Every off-chain `register_issuer` caller MUST run it pre-submit;
`scripts/bootstrap_issuer.ts` does so.  Any third-party SDK or
custom caller that skips this predicate produces an unsound
issuer registration.

**Telemetry.**  The bypass arm emits `msg!("SEC-048: ...")` plus a
structured `Sec007Bypass { issuer_authority, slot }` event on
every call.  Operator obligation: any mainnet observation of this
event is a deployment incident (mis-built binary on a production
cluster) and triggers immediate rollback.

**Real-fix candidates (must pick one before mainnet).**
1. **SDK cofactor-clear.**  Multiply by 8 in the issuer SDK before
   submission; on-chain stays at the consolation gate.  Cheapest;
   trusts the off-chain caller.
2. **In-circuit subgroup binding.**  Add the prime-order constraint
   to the issuance circuit (~30K extra constraints, one EdDSA-style
   scalar mul).  Strongest soundness; batches with SOLID-SEC-006
   Part 2 trusted-setup cycle.
3. **`sol_babyjubjub_*` syscall upstream.**  Multi-quarter timeline.

**Regression gate that closes SOLID-SEC-048.**
`tests/integration/register_issuer_compute_units.test.ts` (P0-7
deliverable; not yet authored).  Lands `register_issuer` on a
real validator at the default 200K CU budget; against the
no-feature build it MUST keep failing with
`exceeded CUs meter at BPF instruction` until the real fix lands.
The day that test passes against a no-feature build, SOLID-SEC-048
flips Fixed.

**Cross-references (canonical, do not duplicate elsewhere):**
sec/SECURITY_REGISTRY.md -> SOLID-SEC-048;
docs/E2E_BLOCKERS.md -> B9 (live), O7 (close-out);
docs/IMPROVEMENTS_ROADMAP.md -> P0-7;
docs/FORWARD_ROADMAP.md -> Phase 4 P0 first item;
plan/RESUME.md -> "Open / Fixed punch-list" + reopened SEC-007
section;
crates/solid-core/src/babyjubjub.rs -> `is_on_curve`,
`is_identity` (consolation gate);
programs/issuer-registry/src/lib.rs -> feature-gated arm +
`Sec007Bypass` event;
programs/issuer-registry/Cargo.toml -> `sec007-skip-onchain`
feature with NOT-FOR-MAINNET banner;
ts-sdk/packages/core/src/index.ts -> `isInPrimeOrderSubgroup`;
scripts/bootstrap_issuer.ts -> off-chain pre-submit gate +
adjusted CU cap.

---

## 2. Root-cause discipline audit

For every Open `SOLID-SEC-NNN`, the proposed fix is classified
either `ROOT-CAUSE` (addresses the actual defect) or `WORKAROUND`
(masks symptoms). Rule: nothing ships as `WORKAROUND`.

| ID  | Finding (short)                                      | Discipline | Justification |
|-----|------------------------------------------------------|------------|---------------|
| 001 | Query indices unconstrained                          | ROOT       | Add missing `LessThan` constraints; forces new setup. |
| 002 | Poseidon hash check commented out                    | ROOT       | Re-enable the check using `light-poseidon`. |
| 003 | `issue_credential` missing bindings                  | ROOT       | Add `schema_account` + `schema_tree_binding` accounts. |
| 004 | No in-circuit issuer binding                         | ROOT       | Compressed issuer tree + in-circuit membership. |
| 005 | `currentTimestamp` unbound                           | ROOT       | Bind to `Clock::get()` with configurable skew. |
| 006 | VK rotation unsafe                                   | ROOT       | `vk_frozen` + generation counter + pause gate. |
| 007 | BJJ subgroup unchecked                               | ROOT       | Cofactor rejection via arkworks. |
| 008 | Nullifier lacks epoch                                | ROOT       | Include monotonic epoch in preimage. |
| 009 | WASM bridge CI/SDK fractured                         | ROOT       | Pick one canonical location (wasm/ per ADR-0002). |
| 010 | Cross-language vectors narrow                        | ROOT       | Emit vectors for every primitive with a TS caller. |
| 011 | E2E scripts bugged                                   | ROOT       | Fix filenames, imports, and missing bootstrap. |
| 012 | Trusted setup single-party                           | ROOT       | Multi-party ceremony, 10+ contributors. |
| 013 | Single-key slashing                                  | ROOT       | Squads multisig + timelock. |
| 014 | Shared `stake_vault`                                 | ROOT       | Per-issuer vault PDAs. |
| 015 | Trust anchor no tier gate                            | ROOT       | Tier-strictly-higher constraint. |
| 016 | Single-step authority transfer                       | ROOT       | Propose/accept pattern. |
| 017 | `test_rng` in prover                                 | ROOT       | Switch to `OsRng`. |
| 018 | `verifier_config` write lock                         | ROOT       | Drop counter or shard the account. |
| 019 | Missing schema-hash re-assertion                     | ROOT       | Add `require!` in each handler. |
| 020 | Plaintext E2E secrets                                | ROOT       | Move to ephemeral store + `.gitignore` + hook. |
| 021 | Depth-20 ceiling ~250K holders                       | ROOT       | Depth-24 circuit in next setup. |
| 022 | Local `IsZero` shadow                                | ROOT       | Delete local, include circomlib. |
| 023 | `active_issuers` drift                               | ROOT       | Centralize transitions. |
| 024 | Raw `-=` in unstake                                  | ROOT       | `checked_sub` everywhere, clippy gate. |
| 025 | Ungated `CheckIssuerStatus`                          | ROOT       | Wire up or delete. |
| 026 | `verify_integrity` never called                      | ROOT       | Docstring clarifies scope. |
| 027 | Stale roadmap checkboxes                             | ROOT       | Flip + archive; auto-generate from registry. |
| 028 | WASM build path wrong in docs                        | ROOT       | Correct documented command to `wasm-pack build wasm/ ...`. |
| 029 | `IdentityAnchor` always enabled                      | ROOT       | Thread `enabled` through from CredentialAtom. |
| 030 | `stake_vault` GC edge case                           | ROOT       | Rent-exempt check + explicit close. |
| 031 | `bufToDecimal` LE on pubkey                          | ROOT       | Separate BE/LE helpers. |
| 032 | `SCHEMA_REGISTRY_ID_BYTES` hardcoded                 | ROOT       | Build-time assertion + CI check. |
| 033 | Cohesion catch-22                                    | ROOT       | Compare derived pubkey, not master. |
| 034 | PDA seeds missing on slash/fraud                     | ROOT       | Add seed constraints. |
| 035 | Unfreeze without timelock                            | ROOT       | 24h timelock OR document trade-off in ADR-0009. |
| 036 | Stale nullifier docstring                            | ROOT       | Update module doc. |
| 037 | WithdrawAfterCooldown constraint parity              | ROOT       | Add constraint. |
| 038 | Informational cluster                                | ROOT       | Code comments. |

**Discipline verdict: 38 / 38 items are root-cause fixes.** Zero
workarounds in the proposed plan.

### What "diverging from root cause" looks like (and is forbidden)

For each CRITICAL, the rejected anti-patterns:

- **SOLID-SEC-001.** "We will validate query indices off-chain in
  the SDK." -- Rejected. The circuit's soundness must be self-
  contained.
- **SOLID-SEC-002.** "We document that schema authorities must
  self-verify." -- Rejected. Trusting caller honesty is the
  definition of no security guarantee.
- **SOLID-SEC-003.** "We add a 'recommended' middleware in the
  SDK that checks the tree binding." -- Rejected. On-chain programs
  cannot assume SDK wrappers were used.

---

## 3. Overengineering audit

- **ESSENTIAL:** without it, the protocol cannot honestly be called
  infrastructure.
- **RIGHT-SIZED:** addresses a real concern at minimum viable
  complexity.
- **OVER-SCOPED:** adds complexity without proportionate benefit at
  current scale.
- **NICE-TO-HAVE:** genuinely optional.

### Phase 1 scope

| Item                                          | Classification | Notes |
|-----------------------------------------------|---------------|-------|
| Circuit range checks (001)                    | ESSENTIAL     | Soundness. |
| Re-enable schema-hash check (002)             | ESSENTIAL     | One line, blocks critical exploit. |
| Bind `issue_credential` (003)                 | ESSENTIAL     | Closes rogue-schema attack surface. |
| Bind `currentTimestamp` (005)                 | ESSENTIAL     | Closes expiry bypass. |
| WASM bridge single source: code (009)         | ESSENTIAL     | Silent-drift guard. |
| WASM build path doc fix (028)                 | ESSENTIAL     | Without it, every new contributor builds an empty pkg. |
| E2E script fixes (011)                        | ESSENTIAL     | E2E blocker. |
| `IdentityAnchor` enabled gate (029)           | ESSENTIAL     | Batch correctness for padding. Bundled in Phase 1 setup. |
| `stake_vault` GC guard (030)                  | ESSENTIAL     | Cheap rent-floor check prevents infra brick. |
| `bufToDecimal` endianness (031)               | ESSENTIAL     | Potential universal verify failure. |
| `SCHEMA_REGISTRY_ID_BYTES` check (032)        | ESSENTIAL     | Guards the most critical on-chain check. |
| Cohesion catch-22 (033)                       | ESSENTIAL     | E2E blocker. |
| Fresh single-party setup for TESTNET          | RIGHT-SIZED   | Phase 3 redoes multi-party for mainnet. |
| Doc archival + roadmap fix (027)              | RIGHT-SIZED   | Prevents future auditors re-opening closed items. |
| Secrets hygiene (020)                         | ESSENTIAL     | Cheap; leak is catastrophic. |

### Phase 2 scope

| Item                                          | Classification | Notes |
|-----------------------------------------------|---------------|-------|
| In-circuit issuer binding (004)               | ESSENTIAL     | Closes unapproved-issuer gap. |
| VK versioning + grace (006)                   | ESSENTIAL     | Prevents rotation bricking in-flight proofs. |
| BJJ subgroup checks (007)                     | ESSENTIAL     | Blocks small-order forgery. |
| Nullifier epoch (008)                         | RIGHT-SIZED   | Defense-in-depth; bundled with 004 setup. |
| Vector expansion (010)                        | ESSENTIAL     | Drift guard. |
| Remaining 9 integration tests                 | ESSENTIAL     | No production without this. |
| Schema-registry events                        | ESSENTIAL     | Indexer-visibility. |
| Revocation v1 E2E                             | ESSENTIAL     | Long-lived credentials need it. |
| Authority transfer two-step (016)             | RIGHT-SIZED   | Operator-footgun prevention. |
| Trust-anchor tier gate (015)                  | RIGHT-SIZED   | Closes centralization edge. |
| PDA seed parity (034, 037)                    | RIGHT-SIZED   | Defense-in-depth. |
| Per-issuer stake vaults (014)                 | RIGHT-SIZED   | Simplifies math + isolates failure. |
| Registry hash-match (019)                     | RIGHT-SIZED   | Defense-in-depth. |
| Docstring / hygiene cluster (035, 036, 037, 038) | RIGHT-SIZED | Cheap clarity fixes. |

### Phase 3 scope

| Item                                          | Classification | Notes |
|-----------------------------------------------|---------------|-------|
| Multi-party ceremony (012)                    | ESSENTIAL     | Mainnet blocker. |
| Squads multisig + timelock (013)              | ESSENTIAL     | Universal-forgery surface otherwise. |
| External audit                                | ESSENTIAL     | Non-negotiable. |
| Nullifier scale-escape (018, 021)             | ESSENTIAL     | Rent + throughput. |
| Artifact versioning + IPFS/Arweave            | ESSENTIAL     | Reproducibility. |
| Monitoring + runbook                          | ESSENTIAL     | Ops requirement. |
| Depth-24 circuit                              | OVER-SCOPED at < 10K holders; RIGHT-SIZED at > 100K. Ship artifacts, defer deploy until monitor warns at 60%. |
| `verifier_config` sharding (018)              | OVER-SCOPED if < 50 verify/sec sustained; RIGHT-SIZED beyond. Gate on telemetry. |

### Phase 4 scope

| Item                                          | Classification | Notes |
|-----------------------------------------------|---------------|-------|
| Recursive aggregation (Nova / folding)        | NICE-TO-HAVE at 1K verify/sec; ESSENTIAL at 10K+. Strategic. |
| Native mobile prover                          | ESSENTIAL for consumer; NICE-TO-HAVE for B2B-only. |
| Attested TLS (zkPass style)                   | NICE-TO-HAVE for v1. Strategic unlock. |
| Cross-chain anchoring                         | NICE-TO-HAVE; defer until real multi-chain partner. |
| Threshold-issuer signing (FROST)              | NICE-TO-HAVE; ESSENTIAL only if regulated-issuer co-sign becomes a stated requirement. |
| Biometric PoP integration                     | NICE-TO-HAVE; product question. |
| Per-credential SMT revocation (v1.1)          | RIGHT-SIZED for financial/medical; OVER-SCOPED for age-gates. Feature flag. |
| DA-snapshot publication (Celestia)            | NICE-TO-HAVE; defer. |
| Formal verification (Kani / Certora)          | OVER-SCOPED at current scope; RIGHT-SIZED if a second external audit still flags soundness. |
| Confidential-compute issuer enclaves          | NICE-TO-HAVE; required only for regulated enterprise issuance. |

**Overengineering verdict: Phase 1-3 scope is essential or right-
sized across the board. Phase 4 contains three items that risk
being built ahead of need (recursive aggregation, attested TLS,
cross-chain). Gate those behind real demand signals.**

---

## 4. Phased plan

### Phase 1 -- Unbrick (CLOSED 2026-04-23)

Closed the three CRITICALs and every E2E blocker.  Snapshot:
`sec/audits/2026-04-23_v0.5_phase1_closeout.md`.

#### Entry criteria (met)

- `sec/SECURITY_REGISTRY.md`, `adr/`, `plan/` in place.
- Team alignment on the non-negotiables in Section 0.

#### Scope and close-out

| SOLID-SEC  | Title                                                | Fix layer                    | Status | Landed |
|------------|------------------------------------------------------|------------------------------|--------|--------|
| 001        | Circuit query indices unconstrained                  | circom + R1CS + setup        | Fixed  | `bc971e6` |
| 002        | `register_schema` integrity check                    | schema-registry              | Fixed  | `402fb4e` |
| 003        | `issue_credential` bindings                          | issuer-registry + accounts   | Fixed  | `8a31abf` |
| 005        | `currentTimestamp` clock binding                     | zk-verifier                  | Fixed  | `402fb4e` |
| 009        | WASM bridge: code + CI + SDK path                    | wasm/ + CI + ts-sdk          | Fixed  | `0c333fb` |
| 011        | E2E script fixes                                     | scripts                      | Fixed  | `8ad20e3` |
| 020        | Secrets hygiene                                      | `.gitignore` + hook          | Fixed  | `174cf50` + `8ad20e3` |
| 027        | Roadmap + historical doc archive                     | docs/                        | Fixed  | `174cf50` |
| 028        | WASM build path documentation                        | CLAUDE.md + test README      | Fixed  | `174cf50` |
| 029        | `IdentityAnchor` enabled gate                        | circom + setup               | Fixed  | `bc971e6` |
| 030        | `stake_vault` GC rent-floor guard                    | issuer-registry              | Fixed  | `402fb4e` |
| 031        | `bufToDecimal` endianness                            | ts-sdk holder                | Fixed  | `4b37425` |
| 032        | `SCHEMA_REGISTRY_ID_BYTES` build-time check          | solid-light + Rust test      | Fixed  | `d34d0ca` |
| 033        | Cohesion check uses derived key                      | ts-sdk holder                | Fixed  | `4b37425` |

#### Root-cause rationale (Phase 1)

- 001 + 029 require circuit changes. Bundle into **one** Phase 1
  trusted setup (single-party, labeled "TESTNET ONLY"). Multi-party
  ceremony for mainnet lives in Phase 3. The single-party setup
  here is a sequencing decision to unblock E2E, not a workaround on
  the multi-party requirement.
- 002 is one line re-enabled.
- 003 is an account-layout change matching the schema-registry PDA
  seeding.
- 005 bumps `VerifierConfig` layout; assert the `SPACE` invariant.
- 009 picks `wasm/` per ADR-0002 and ADR-0013. 028 updates the doc
  to match; both must land together.
- 011 is three mechanical fixes.
- 020 adds `.gitignore` entry + pre-commit hook rejecting BJJ key
  magic bytes.
- 027 archives stale docs under `docs/archive/`; `check_docs.py`
  enforces registry/roadmap agreement.
- 030 adds a rent-floor saturating check before lamport transfer.
- 031 splits `bigintFromBytesLE` / `bigintFromBytesBE`; BE for
  pubkeys, LE for field elements. Round-trip vector added.
- 032 adds Rust `#[test]` + `check_program_ids.py` extension that
  assert the base58 decode matches the hardcoded constant.
- 033 changes the cohesion check to derive per-schema pubkey
  before comparing.

#### Regression gates (Phase 1)

Every gate must exist in CI before Phase 1 can close.

- `circuit_range_check_property_test` -- 1000 random out-of-range
  indices fail witness generation.
- `circuit_padding_slot_requires_no_global_proof` -- 3-credential
  batch generates valid witness without zero-schema global entry.
- `register_schema_rejects_mismatch_unit_test` -- returns
  `InvalidSchemaHash`.
- `integration_06_issue_credential_rejects_unregistered_schema`.
- `integration_07_issue_credential_rejects_wrong_tree`.
- `integration_10_verify_expired_credential_rejected`.
- `integration_11_verify_future_timestamp_rejected`.
- `e2e_localnet` CI job: clean checkout -> bootstrap -> deploy ->
  issue -> prove -> verify, green on every push.
- `wasm_bridge_smoke` CI job: build pkg from `wasm/`, import from
  SDK's pinned path, round-trip every primitive.
- `schema_registry_id_bytes_matches_anchor_toml` Rust unit test.
- `cohesion_check_passes_for_derived_key` holder SDK test.
- `check_docs_py` CI job: registry and roadmap agree; archived
  docs carry the banner; no emoji under `docs/` (excluding
  `archive/`).
- `vector_verifier_id_roundtrip` in cross-language vectors.
- `stake_vault_not_garbage_collected_after_full_slash` unit test.
- `gitignore_e2e_state_json` pre-commit hook test.

#### Exit criteria (Phase 1)

1. Every SOLID-SEC-001, 002, 003, 005, 009, 011, 020, 027, 028,
   029, 030, 031, 032, 033 flipped `Open -> Fixed` with commit
   hashes + regression-test references. **MET 2026-04-23.**
2. Every host-side gate green on `main` for at least one sprint.
   Met for every fix whose gate is host-side. Integration-level
   gates (`e2e_localnet`, circuit witness property tests)
   land with the Phase 2 CI bring-up and are listed as
   Phase-2-deliverable regression gates inside their registry
   entries.
3. A stranger can execute:
   ```
   git clone ...
   nix develop
   cargo test -p solid-core -p solid-light -p zk-verifier
   anchor build
   cd circuits && npm install && node scripts/setup.js && cd ..
   wasm-pack build wasm/ --target nodejs \
       --out-dir ts-sdk/packages/core/wasm --release
   cd ts-sdk && npm ci && npm run build && cd ..
   solana-test-validator --reset &
   anchor deploy --provider.cluster localnet
   ts-node scripts/initialize.ts
   ts-node scripts/bootstrap_issuer.ts
   ts-node scripts/issue.ts
   ts-node scripts/prove.ts
   ```
   with every step green in under 30 minutes.
4. New dated audit snapshot under `sec/audits/` confirms closures.
5. `deployments/devnet.json` has real `deployed_at`,
   `deployer.address`, `upgrade_authority`.

#### Phase 1 forbidden outcomes

- Any workaround on 001 / 002 / 003.
- Skipping a regression gate "temporarily."
- Leaving roadmap stale.
- Declaring closed without an audit snapshot.

---

### Phase 2 -- Close the soundness loop (8-10 weeks after Phase 1)

#### Entry criteria

- Phase 1 exit criteria green.
- ADR recorded on in-circuit issuer binding approach (compressed
  tree vs CPI; default compressed tree).

#### Scope

| SOLID-SEC | Title                                       | Fix layer                  | Setup impact |
|-----------|---------------------------------------------|----------------------------|--------------|
| 004       | In-circuit issuer binding                   | circuits + prog           | **Bundled Phase 2 setup** |
| 006       | VK versioning + grace                        | zk-verifier                | Public-input +1 if `vk_generation` |
| 007       | BJJ subgroup checks                          | solid-core + registry      | None |
| 008       | Nullifier epoch                              | circuits                  | Bundled Phase 2 setup |
| 010       | Cross-language vector expansion              | solid-core + tests         | None |
| 013       | Slashing/fraud authority preview             | issuer-registry            | Pre-work for Phase 3 multisig |
| 014       | Per-issuer stake vaults                      | issuer-registry            | None |
| 015       | Trust-anchor tier gate                       | issuer-registry            | None |
| 016       | Two-step authority transfer                  | zk-verifier + schema-registry | None |
| 017       | `OsRng` in solid-prover                      | solid-prover               | None |
| 019       | Schema-hash re-assertion                     | schema-registry            | None |
| 022       | Circomlib `IsZero`                           | circuits                  | R1CS hash (no setup if identical) |
| 023       | `active_issuers` drift                       | issuer-registry            | None |
| 024       | Clippy arithmetic_side_effects                | workspace                 | None |
| 025       | Wire or delete `CheckIssuerStatus`           | issuer-registry            | None |
| 034       | PDA seeds on slash/fraud                     | issuer-registry            | None |
| 035       | Unfreeze timelock                            | schema-registry OR ADR-0009 | None |
| 036       | Nullifier docstring                          | solid-core                | None |
| 037       | WithdrawAfterCooldown constraint             | issuer-registry            | None |
| 038       | Informational cluster                        | comments                   | None |
| new       | Remaining 9 integration tests                | tests/integration          | None |
| new       | Schema-registry events                       | schema-registry            | None |
| new       | Revocation v1 E2E (holder + issuer + event)  | ts-sdk + programs          | None |

#### Root-cause rationale (Phase 2)

- 004 + 008 bundled into **one** Phase 2 circuit rev with 006's
  optional `vk_generation` public input.
- 006 adds `vk_frozen: bool` + `vk_generation: u16` to
  `VerifierConfig`; `VerifierConfig::SPACE` bumps in lockstep.
  Stored `(vk_id, VkStorage)` PDAs allow a deprecated-but-valid
  generation during rotation.
- 013 preview: add Squads-compatible seeds + contexts. Actual
  multisig migration in Phase 3.
- Revocation v1 E2E: holder SDK watches for `RevocationEvent`,
  reads the global root, recomputes `identityState`, re-proves.
  Issuer SDK exposes `bumpRevocationNonce(credentialId)`.

#### Regression gates (Phase 2)

- `circuit_witness_fuzz_10k` for the new Phase 2 circuit.
- `snarkjs_fullprove_ci` round-trip.
- `vk_rotation_grace_window_test`.
- `integration_02_issuer_lifecycle` .. `integration_09_*`.
- Cross-language vectors cover every TS-consuming primitive.
- `two_issuer_stake_isolation`.
- `bjj_small_order_pubkey_rejected`.
- `revocation_e2e_flow`.

#### Exit criteria (Phase 2)

1. All in-scope SOLID-SEC items `Fixed` with green regression.
2. 11/11 integration tests green.
3. Second Phase 2 audit snapshot confirms closures.
4. Phase 2 TESTNET zkey published and content-addressed locally.

---

### Phase 3 -- Production infrastructure (12-16 weeks after Phase 2)

#### Entry criteria

- Phase 2 exit criteria green.
- External audit firm engaged.
- Squads multisig addresses provisioned.
- IPFS + Arweave pinning accounts provisioned.

#### Scope

| SOLID-SEC | Title                                          | Notes |
|-----------|------------------------------------------------|-------|
| 012       | Multi-party trusted setup ceremony             | 10+ contributors; attestation chain on IPFS + Arweave. |
| 013       | Squads multisig + timelock                     | 3-of-5 on `VerifierConfig.authority`; 48h timelock on VK rotation; 24h challenge on slashing. |
| 018       | `verifier_config` throughput relief            | Drop counter or shard, gated on load telemetry. |
| 021       | Depth-24 circuit artifact                      | Ship, defer deploy until monitor warns at 60%. |
| new       | External audit close-out                       | OtterSec / Halborn / Trail of Bits. |
| new       | Artifact versioning                            | zkey / wasm / .so / IDL on IPFS + Arweave, hashes pinned in `deployments/mainnet.json`. |
| new       | Monitoring + runbook                           | Prometheus exporter; on-call runbook for unexpected rejections, over-issuance, VK upload race. |
| new       | Finalized-commitment SDK path                  | `'finalized'` helpers for every verify. |
| new       | Mainnet deploy procedure                       | Squads propose -> review -> execute. |

#### Regression gates (Phase 3)

- External audit report published.
- `deployments/mainnet.json` pins artifact hashes, multisig
  authorities, final ceremony hash.
- `verify_ceremony.js` green from a fresh checkout.
- Staged outage test exercises monitoring alerts.
- Slash from non-multisig key fails.
- 1000 random devnet proofs in a 1-hour window with no
  serialization failure from verifier-config contention.

#### Exit criteria (Phase 3)

1. Mainnet contract live behind multisig.
2. External audit complete; any CRITICAL-equivalent resets clock.
3. Third audit snapshot confirms closures.
4. Pre-go-live ADR status review.

---

### Phase 4 -- Next-gen (6-12 months, strategic)

Not blocking mainnet. Items gated on demand signals.

#### Gated by adoption data
- Recursive aggregation.
- `verifier_config` sharding beyond Phase 3 level.
- Depth-24 deploy.
- Per-credential SMT revocation (v1.1).

#### Gated by product scope
- Native mobile prover.
- Attested TLS.
- Threshold-issuer signing (FROST).
- Biometric PoP integration.
- Confidential-compute enclaves.

#### Gated by external demand
- Cross-chain anchoring.
- DA-snapshot publication.

#### Gated by second-audit findings
- Formal verification.

**Phase 4 rule.** Every item requires a new ADR and a demand
signal before implementation. Building ahead of demand is
overengineering by definition.

---

## 5. End-to-end acceptance criteria

All simultaneously true:

- [ ] Stranger runs `nix develop` -> `bootstrap_issuer` -> `issue`
      -> `prove` on `solana-test-validator` with successful on-chain
      verify in < 30 minutes.
- [ ] `deployments/devnet.json` has non-null deploy fields.
- [ ] `tests/integration/` has 11 passing bankrun tests.
- [ ] All CRITICAL + HIGH entries in the registry are `Fixed` or
      `Verified`.
- [ ] `cross_language_vectors` covers every TS-consuming primitive.
- [ ] `e2e_localnet` green on every push.
- [ ] External audit report published (Phase 3 exit).
- [ ] Roadmap byte-for-byte agrees with registry (enforced by
      `scripts/check_docs.py`).
- [ ] Mainnet deploy behind Squads multisig with timelock.

---

## 6. Regression gate matrix

| Gate name                                                         | Phase | Closes finding |
|-------------------------------------------------------------------|-------|----------------|
| `circuit_range_check_property_test`                               | 1     | 001 |
| `circuit_padding_slot_requires_no_global_proof`                   | 1     | 029 |
| `register_schema_rejects_mismatch_unit_test`                      | 1     | 002 |
| `integration_06_issue_credential_rejects_unregistered_schema`     | 1     | 003 |
| `integration_07_issue_credential_rejects_wrong_tree`              | 1     | 003 |
| `integration_10_verify_expired_credential_rejected`               | 1     | 005 |
| `integration_11_verify_future_timestamp_rejected`                 | 1     | 005 |
| `e2e_localnet`                                                    | 1     | 011 |
| `wasm_bridge_smoke`                                               | 1     | 009, 028 |
| `schema_registry_id_bytes_matches_anchor_toml`                    | 1     | 032 |
| `cohesion_check_passes_for_derived_key`                           | 1     | 033 |
| `check_docs_py`                                                   | 1     | 027 |
| `vector_verifier_id_roundtrip`                                    | 1     | 031 |
| `stake_vault_not_garbage_collected_after_full_slash`              | 1     | 030 |
| `gitignore_e2e_state_json`                                        | 1     | 020 |
| `integration_02_issuer_lifecycle`                                 | 2     | multiple |
| `integration_03_slash_transfers_lamports`                         | 2     | multiple |
| `integration_04_schema_and_bindings`                              | 2     | 019 |
| `integration_05_issue_credential`                                 | 2     | 003 (positive) |
| `integration_08_verify_forged_global_tree_rejected`               | 2     | ADR-0010 |
| `integration_09_verify_forged_schema_tree_rejected`               | 2     | ADR-0010 |
| `circuit_witness_fuzz_10k`                                        | 2     | 001, 004, 008, 029 |
| `snarkjs_fullprove_ci`                                            | 2     | 001, 004, 008, 029 |
| `vk_rotation_grace_window_test`                                   | 2     | 006 |
| `bjj_small_order_pubkey_rejected`                                 | 2     | 007 |
| `solid_prover_osrng_unlinkability`                                | 2     | 017 |
| `revocation_e2e_flow`                                             | 2     | new |
| `two_issuer_stake_isolation`                                      | 2     | 014 |
| `trust_anchor_same_tier_rejection`                                | 2     | 015 |
| `propose_accept_authority_transfer`                               | 2     | 016 |
| `active_issuers_transition_property_test`                         | 2     | 023 |
| `clippy_arithmetic_side_effects_workspace_gate`                   | 2     | 024 |
| `slash_and_fraud_pda_seed_constraint`                             | 2     | 034 |
| `verify_ceremony_attestation_chain`                               | 3     | 012 |
| `slash_from_non_multisig_rejected`                                | 3     | 013 |
| `verifier_throughput_300_concurrent`                              | 3     | 018 |
| `mainnet_artifact_hash_pin_check`                                 | 3     | new |
| `staged_outage_alerting`                                          | 3     | new |
| `register_issuer_compute_units` (no-feature build hits CU ceiling; bypass build emits `Sec007Bypass`) | 4 (P0-7) | 048 |
| `mainnet_build_rejects_sec007_skip_onchain_feature` (CI gate)     | 4 (P0-7) | 048 |
| `mainnet_monitor_alerts_on_sec007bypass_event`                    | 4 (P0-7) | 048 |

---

## 7. Milestone cadence

- End of each phase: dated audit snapshot in `sec/audits/` with
  filename `YYYY-MM-DD_<version>_<slug>.md`.
- Mid-phase: weekly status-column updates in Section 4. No prose
  changes mid-phase without a superseding ADR or new audit.
- Scope additions: new `SOLID-SEC-NNN` first, then insert here.

---

## 8. Risks and open questions

| Risk                                                               | Likelihood | Impact | Mitigation |
|--------------------------------------------------------------------|-----------|--------|------------|
| Phase 1 setup race: mid-phase circuit bug forces second setup      | Medium    | High   | Freeze branch; staging-net test before publish. |
| External audit surfaces a new CRITICAL                             | Medium    | High   | Phase 3 exit allows clock reset. |
| `SCHEMA_REGISTRY_ID_BYTES` re-diverges on a future redeploy        | Low       | Catastrophic | SOLID-SEC-032 build-time assertion + CI. |
| `bufToDecimal` endianness escalates to CRITICAL                    | Unknown   | High   | Phase 1 trace + vector adds the round-trip gate. |
| Rent-per-nullifier becomes a blocker at < 10^5 proofs/month        | Low       | High   | Phase 3 compressed-nullifier design starts in Phase 2 pre-work. |
| Multi-party ceremony participants drop out                         | Medium    | High   | Recruit 15 for a 10-participant ceremony. |
| Multisig key-holder compromise                                     | Low       | Catastrophic | Key-holder rotation procedure in Phase 3 runbook. |
| ADR silently violated by a future change                           | Medium    | Medium | CODEOWNER on `adr/`; PR template checklist. |
| External partners push for cross-chain or mobile ahead of Phase 3  | High      | Scope creep | Phase 4 gating rule. |

---

## 9. Discipline rules (reprinted for PR reviewers)

When reviewing any PR against the protocol, check:

1. **Root cause.** Does the fix address the actual defect?
2. **No regressions.** Is a regression test included? Does it gate CI?
3. **Doc truth.** Are all referenced docs updated? Is the roadmap
   touched if a registry item is affected?
4. **Single source of truth.** Does the change introduce a second
   canonical location for anything (VK, WASM bridge, program IDs,
   deployment manifest)?
5. **ADR compliance.** Does the change violate an `Accepted` ADR
   without a superseding ADR in the same PR?
6. **Registry alignment.** If the PR closes a finding, does it flip
   the registry status and cite the commit hash?

A PR that fails any of the six does not merge.

---

## Appendix A -- How to add a new finding

1. Add a `SOLID-SEC-NNN` to `sec/SECURITY_REGISTRY.md` with
   evidence, impact, remediation.
2. Determine phase: severity + dependency + setup impact.
3. Append to the right phase scope table in Section 4.
4. Add the regression-gate name to Section 6.
5. Update summary counts in the registry header.

## Appendix B -- How to add a new ADR

See `adr/README.md`. New `adr/NNNN-title.md`, add to
`adr/INDEX.md`, status `Proposed` until the decision lands.

## Appendix C -- How to close a phase

1. All items in the phase scope are `Fixed` or `Verified`.
2. Every Section 6 gate for the phase is green for >= one sprint.
3. Write the audit snapshot:
   `sec/audits/<date>_<version>_<slug>.md`.
4. Update this plan's status columns and the next-phase entry
   criteria.

## Appendix D -- Next session pickup (2026-04-30)

Updated 2026-04-29 evening after the (1)+(3) attempt + pivot.
For the day-by-day receipts, read
`plan/SESSION_LOG_2026-04-29.md` end-to-end before touching
anything; the L7-L9 discipline rules in §5 of that doc are new
and load-bearing.

Live edge: **B13 Option 2 -- buffer-account / chunked upload.**
The originally-recommended (1)+(3) path landed in code (B13
on-chain reconstruction + Versioned-tx + ALT) and surfaced three
latent BE/LE byte-order bugs which were also fixed.  But the
combined surface is 37 bytes over the 1232-byte legacy-tx cap
once the required `setComputeUnitLimit` ix is included.  Hence
the pivot.

Recommended path (per `docs/E2E_BLOCKERS.md` B13 + the pivot
analysis added there 2026-04-29 + `docs/REMEDIATION_OPTIONS_ARCHIVE.md`
§1.1): **buffer-account upload + verify-from-buffer.**

  * The (1)+(3) reconstruction pieces (slot mapping, extract
    helpers, ALT helpers, the LB1-LB3 byte-order fixes) shipped
    as code 2026-04-29 and stay in tree.  See
    `plan/SESSION_LOG_2026-04-29.md` §1.3 for the full list.
    What did NOT ship: the actual on-chain submission, because
    the resulting v0+ALT+cuIx tx is 1269 raw bytes (37 over the
    1232-byte cap).
  * Buffer-account approach (this session pickup):
      - `init_proof_buffer(payer)` -- new ix; allocates a
        scratch PDA seeded by `[b"proof-buffer", payer.key]`,
        ~1024 bytes capacity.
      - `upload_proof_chunk(buffer, offset: u32, bytes:
        Vec<u8>)` -- new ix; writes `bytes` into
        `buffer.data[offset..offset+bytes.len()]`.
        Idempotent on re-upload-of-same-bytes.  Chunk size is
        chosen so each upload tx fits cleanly under 1232 bytes;
        ~700-byte chunks cover the proof in 2 uploads.
      - `verify_batch_proof_v2(buffer)` -- new ix; reads the
        staged proof from `buffer.data`, performs the same B13
        reconstruction the existing `verify_batch_proof`
        already does (reuse the extract helpers in
        `crates/solid-light/src/cpi_helpers.rs`), runs Groth16,
        atomically initializes the nullifier PDA via the same
        `init` constraint, closes the buffer (rent reclaimed
        to payer).
  * Slots 30 (`verifierNonce`) and 31 (`currentTimestamp`) stay
    in the wire-equivalent buffer payload because they are
    witness-bound (Groth16 has zero-tolerance polynomial
    equality on public inputs; the SEC-005 skew window is an
    additional on-chain freshness predicate, not a substitute).
  * Trust boundaries (preserve all):
      - Buffer PDA owner is `zk_verifier` -- only this program
        can write via `upload_proof_chunk`.
      - `verify_batch_proof_v2` rejects underfilled buffers
        (length mismatch, missing chunks).
      - Buffer is closed at end of verify (rent return); re-use
        requires a fresh `init_proof_buffer`.
      - All existing soundness gates from `verify_batch_proof`
        replicate in `_v2` (owner checks, schema-canonicality,
        issuer-tree binding, timestamp skew, nullifier bind).
  * Circuit unchanged; trusted setup unchanged; the SEC-048
    Option E ceremony (arc 5) is unaffected by this pivot.

Steps for the implementer:

1. Add the three new ixs to
   `programs/zk-verifier/src/lib.rs`.  Reuse the existing
   `WIRE_INPUT_SLOTS` / `RECONSTRUCTED_INPUT_SLOTS` constants
   and the `extract_active_*` helpers in
   `cpi_helpers.rs` -- both already shipped 2026-04-29.
2. Update `ts-sdk/packages/verifier/src/index.ts::verifyOnChain`
   to orchestrate the chunked-upload flow:
   `ensureLookupTable` (already implemented) ->
   `init_proof_buffer` -> `upload_proof_chunk` * N ->
   `verify_batch_proof_v2(buffer)`.  Cache the ALT pubkey
   across runs (already implemented); cache the buffer PDA
   per-payer.
3. Add cargo unit tests:
   - chunked upload assembles to byte-identical payload as the
     legacy direct-tx wire.
   - missing chunk in buffer -> `_v2` rejects with a typed
     error.
   - over-chunk (write past buffer end) -> rejects.
   - buffer-from-different-payer -> rejects (wrong PDA owner).
4. Add an integration test that submits a proof with a forged
   buffer (wrong globalRoot bytes injected) and asserts
   Groth16 rejects.  Pair with the existing P0-2 owner-check
   tests.
5. Run `npm run e2e`; assert `verified: true` tail.
6. Promote SEC-054 from "Open (interim partial)" to "Fixed"
   in `sec/SECURITY_REGISTRY.md` once green.  "Verified" after
   one full sprint of the integration test in CI per the
   non-negotiable in §0.

Order of operations the next session:

  1. Read `plan/SESSION_LOG_2026-04-29.md` end-to-end,
     especially §5 (learnings L7-L9 are new).
  2. Read `plan/RESUME.md` "Pickup tomorrow" section.
  3. Read `docs/E2E_BLOCKERS.md` B13 for the full byte-math
     ledger of why (1)+(3) doesn't fit.
  4. Read `docs/REMEDIATION_OPTIONS_ARCHIVE.md` §1.1 for the
     mechanics + future-trigger of Option 2.
  5. Commit-split the WIP per RESUME §"Step 1" before any new
     code.  L9 of the session log is explicit on this.
  6. Implement `init_proof_buffer` / `upload_proof_chunk` /
     `verify_batch_proof_v2`; add the cargo unit + integration
     tests (the regression gates ship in the same commit as
     the fix, per L4).
  7. Run e2e end-to-end; confirm `verified: true`.
  8. Promote SEC-054 to Fixed.

After B13 closes, the immediate Phase 4 P0 backlog is:

  - SOLID-SEC-046 (CU regression gate; pair with the new B13
    baseline).
  - SOLID-SEC-010 (cross-language vectors 3 -> 10).
  - Integration suite 02..11 (bankrun + jest harness; SPL AC
    + noop fixtures; 10 scenarios per `tests/integration/README.md`).
  - SOLID-SEC-017 (`OsRng` in solid-prover; trivial).

---

*End of plan. One source of truth. One place to track what is
done, what is left, and how the rest gets shipped.*
