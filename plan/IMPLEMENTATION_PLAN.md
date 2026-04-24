# SolID Protocol -- Implementation Plan

Living plan. Single source of truth for scope, phasing, and
acceptance. Every item references `SOLID-SEC-NNN` in
`sec/SECURITY_REGISTRY.md` and/or `ADR-NNNN` in `adr/`.

- Protocol version in scope: v0.6.1 (Phase 3 impl 1-4 closed) ->
  v1.0 mainnet
- Last revision: 2026-04-25 (Phase 3 impl 1-4: SEC-007 + SEC-006
  Part 1 + SEC-041 + SEC-044 all flipped Fixed; v0.6.1 audit
  dropped in, registering SEC-045 and SEC-046 as MEDIUM;
  canonical state-of-protocol snapshot at
  `sec/audits/2026-04-25_v0.6.1_deep_comprehensive_audit.md`)
- Next revision trigger: P0 close-out per v0.6.1 Section 6.1
  (SEC-045, SEC-046, SEC-010, integration suite 02..11, module
  split, devnet deploy)

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

---

## 1. Current state snapshot (as of 2026-04-25, post-Phase-2)

### Done and verified in code

Cross-checked against code and
`sec/audits/2026-04-25_v0.6.1_deep_comprehensive_audit.md`
(supersedes the v0.6 audit):

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

---

*End of plan. One source of truth. One place to track what is
done, what is left, and how the rest gets shipped.*
