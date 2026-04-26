# Integration Audit — solid-protocol v0.6.1

| Field | Value |
|-------|-------|
| Date | 2026-04-26 |
| HEAD | `59c99c2` |
| Scope | Cross-program flows, Rust ↔ TS ↔ Circuit byte-identity, end-to-end trust boundaries |
| Auditor | Orchestrator (synthesizing wave-1 findings + direct cross-cut verification) |

This file integrates the per-module audits into a coherent cross-cutting view. It does not re-state findings already in the per-module files; it identifies where module boundaries leak, where invariants depend on multiple modules being correct simultaneously, and where the system's load-bearing contracts live.

---

## 1. The five trust boundaries

The protocol's security rests on five interlocking contracts. A break in any one collapses guarantees:

### Contract 1 — Cross-language byte-identity (Rust ⇔ TypeScript ⇔ Circuit)

**Players**: `crates/solid-core` (Rust), `wasm/` (WASM bridge), `ts-sdk/packages/core` (TS), `circuits/*.circom` (Circom).

**Contract**: every cryptographic primitive (Poseidon, BabyJubJub group ops, EdDSA-Poseidon sign/verify, schema hash, attestation commitment, identity-state, issuer leaf, 6-input nullifier, per-schema key derivation) produces byte-identical outputs across Rust BPF, Rust host, Rust wasm32, TS WASM, and the Circom circuit witness.

**Verification status** (per audits 04, 06, 07, 08):

| Primitive | Rust ↔ Circuit | Rust ↔ TS WASM | Vectored? |
|-----------|----------------|----------------|-----------|
| Poseidon hash | ✓ (ADR-0006, Bn254X5/LE) | ✓ (M06) | ✗ direct |
| BJJ subgroup check | ✓ | ✓ | ✗ |
| EdDSA sign | n/a (off-chain) | ✓ | ✗ |
| EdDSA verify | ✓ (in CredentialAtom) | ✓ | ✗ |
| Commitment | ✓ (M07 §3.2) | ✓ | ✓ tested |
| Identity state | ✓ (M07 §3.3) | ✓ | ✗ |
| Per-schema key derivation | ✓ (M07 §3.4) | ✓ | ✗ |
| Issuer leaf (Poseidon5) | ✓ (M07 §3.1) | ✓ | ✗ |
| Nullifier preimage (Poseidon6) | ✓ (M07 §2) | ✓ | ✓ tested |

**Gap**: 2 of 9 primitives have pinned vectors (`tests/vectors/commitment_and_nullifier.json`). The other 7 are cross-language correct by construction but not regression-gated. **SOLID-SEC-010 / M06-H01 / M08 cross-references**.

**Doc-drift detected**:
- Circuit comment at `batch_credential_query.circom:103-106` says `issuerAuthority` is BE; Rust + SDK both use LE. Comment is wrong; code matches Rust ↔ TS. M07-INVESTIGATE-01 → M08-DOCS01.
- Earlier orchestrator zk-verifier hand-audit incorrectly described nullifier preimage as `(identityCommitment, secret, schemaId, credentialIndex, currentTimestamp, issuerTreeRoot)` — actual is `(masterKey, revocationNonce, verifierAddress, queryContextHash, verifierNonce, issuerTreeRoot)`. M07-FIX01.

### Contract 2 — On-chain owner-checks on tree PDAs (ADR-0010 + ADR-0014)

**Players**: zk-verifier (consumer), schema-registry (writer of GlobalStateBinding + SchemaTreeBinding), issuer-registry (writer of IssuerTreeBinding), solid-light (parsers).

**Contract**: every tree-binding PDA the verifier reads has its owner cross-checked against the canonical writer program ID; the parser layer adds discriminator + length + content gates.

**Verification** (per audits 01, 02, 03, 05):

`zk-verifier::verify_batch_proof` does five layered checks per tree PDA:
1. Owner check (`require_keys_eq!(*account.owner, EXPECTED_PROGRAM_ID)`).
2. Length check (parser).
3. Discriminator check (parser).
4. Content match (root or schema_hash + root, depending on PDA type).
5. Status check (only active bindings).

For `IssuerTreeBinding` specifically, an additional Anchor `seeds::program = ISSUER_REGISTRY_ID` constraint at the account-context level (`programs/zk-verifier/src/lib.rs:884-889`) makes layer 0 explicit too.

**Drift gates verified**:
- `cpi_helpers.rs:99-150` Rust unit tests pin the `*_ID_BYTES` constants against the base58 string literals (SOLID-SEC-032 + ADR-0014).
- `scripts/check_program_ids.py` performs a 4-way drift check across `Anchor.toml`, `declare_id!()` literals, `cpi_helpers.rs` literals, and `deployments/*.json`.

**Gap**: SOLID-SEC-045 / M02-H01 — atomic handlers (`revoke_issuer_atomic`, `request_withdrawal_atomic`) replace the SPL-AC tree leaf in the same ix but do NOT update `IssuerTreeBinding.current_root`. There's a window between the atomic ix and the operator's separate `update_issuer_tree_root` call where pre-revocation proofs continue to verify because the verifier reads the stale binding root.

### Contract 3 — Replay protection (ADR-0007 + ADR-0006 + ADR-0014)

**Players**: zk-verifier (atomic PDA init), circuit (nullifier output), holder SDK (compute nullifier off-chain).

**Contract**: every successful proof creates a unique nullifier PDA seeded by `[b"null", nullifier]`. Replay attempts fail atomically via Anchor's `init` constraint.

**Verification**:
- Circuit at `batch_credential_query.circom:434-441` outputs `nullifierHash = Poseidon6(masterKey, revNonce, verifierAddr, queryContextHash, verifierNonce, issuerTreeRoot)`.
- zk-verifier at `programs/zk-verifier/src/lib.rs:567-570` initializes `NullifierRecord` PDA with seeds `[NULLIFIER_SEED, nullifier]`.
- Anchor's `init` fails on existing PDA (replay rejected).
- Verifier additionally checks `nullifier == public_inputs[0]` (NullifierMismatch).

**Cross-epoch property** (SOLID-SEC-008): revoking any issuer rotates `IssuerTreeBinding.current_root`, which:
(a) Breaks the in-circuit Merkle membership proof for the revoked issuer's pre-revocation credentials.
(b) Shifts the nullifier universe (root is the 6th input). Pre-revocation cached proofs cannot collide with post-revocation proofs.

**Both directions of the epoch boundary are closed.** ✓

**Open issue**: M02-H01 (SEC-045) leaves a window where the IssuerTreeBinding cache is stale. Within that window, replays of pre-revocation proofs ARE possible because (b) above depends on the binding root rotating in lockstep with the SPL-AC tree.

### Contract 4 — VK lifecycle (ADR-0015 / SOLID-SEC-006 part 1)

**Players**: zk-verifier handlers `store_verification_key` / `finalize_verification_key` / `request_vk_rotation` / `cancel_vk_rotation` / `rotate_verification_key`; off-chain orchestration via `initialize.ts`; SOLID-SEC-041 content-addressed VK artifact.

**Contract**: VK upload is chunked + ordered + capacity-bounded. Once finalized, the VK is immutable. Rotation requires a 48-hour timelock + multisig (today single-signer; SEC-043 tracks the multisig migration).

**Verification** (per audit 01):
- Chunked upload with `next_vk_chunk` cursor, `VK_MAX_BYTES = 10_228` (matches Solana `MAX_PERMITTED_DATA_INCREASE`).
- Freeze-gate (`vk_finalized` bool) refuses every write after finalize.
- Rotation: `request_vk_rotation` → 48h wait → `rotate_verification_key` clears `vk_initialized`/`vk_finalized`/`next_vk_chunk` and bumps `vk_generation`. Comprehensive unit tests (SPACE invariant, timelock boundary, saturation).
- SOLID-SEC-041: `circuits/build/verification_key.sha256` content-addressed; `initialize.ts` should refuse mismatched VK upload (verified in setup.js but not directly traced in initialize.ts in this audit).

**Gap**: SOLID-SEC-006 part 2 — `vk_generation` not yet bound into the public-input contract. Today, an old proof against generation N cannot verify against generation N+1's IC (Groth16 catches it), but the binding gives a stronger formal property. Deferred to next trusted-setup cycle.

### Contract 5 — BJJ subgroup safety (SOLID-SEC-007 / SEC-048)

**Players**: `crates/solid-core/src/babyjubjub.rs` (Rust check), `wasm/src/lib.rs` (WASM bridge), `ts-sdk/packages/core/src/index.ts` (off-chain SDK predicate `isInPrimeOrderSubgroup`), `programs/issuer-registry/src/lib.rs` (on-chain check, currently bypassed).

**Contract**: every BJJ public key registered as an issuer must lie in the prime-order subgroup (cofactor-8 component absent). Off-curve and identity points also rejected.

**Production posture**:
- The on-chain `require_in_prime_order_subgroup` costs > 1.4M CU on BPF (full `r * P == O` scalar mul through arkworks). Exceeds per-tx CU ceiling.
- `programs/issuer-registry/Cargo.toml:37` declares feature `sec007-skip-onchain`. When enabled (localnet/devnet/CI builds today), the on-chain check is skipped; consolation gate (`is_on_curve` + `!is_identity`) runs instead — DOES NOT catch cofactor-8 torsion.
- The off-chain TS predicate `isInPrimeOrderSubgroup` (in `ts-sdk/packages/core/src/index.ts:162-168`) is the load-bearing gate while the bypass is active.
- Mainnet is BLOCKED until SEC-048 closes.

**Critical gap**: M02-H02 — no static / CI gate enforces that mainnet builds compile WITHOUT `sec007-skip-onchain`. Documentation only. A misbuild silently re-opens the gap.

**Recommended remediation path** (per `cu_budget.md` §4):
- **Interim**: Option B — cofactor-clear in SDK before submission. Off-chain `multiply by 8` (3 doublings, ~3ms) ensures the registered key is in the prime-order subgroup. On-chain consolation gate then catches off-curve / identity. Defense in depth without trusted-setup work.
- **Canonical**: Option E — in-circuit subgroup constraint. Adds ~30K constraints (one EdDSA-style scalar mul, ~250ms additional proving cost). Strongest soundness. Requires trusted-setup re-run; batches with SEC-006 part 2 + SEC-012 ceremony.

---

## 2. End-to-end identity flow (issuer → holder → verifier)

### 2.1 Issuance path (issuer-registry::issue_credential)

```
Off-chain:
  Issuer SDK:
    1. compute_attestation_commitment(data, schema, holder_pub, salt)
       -> commitment (Poseidon5 — see audit 04 §5)
    2. EdDSA-Poseidon sign(issuer_priv, commitment)
       -> signature

On-chain (programs/issuer-registry::issue_credential):
    3. require!(issuer.status == Approved)
    4. Cross-program reads:
       - SchemaAccount (typed) with Anchor seed validation against
         schema-registry's program ID.
       - SchemaTreeBinding (UncheckedAccount + parser + owner check).
    5. SOLID-SEC-003 verify_schema_tree_binding_for_issue:
       data[8..40] == schema_hash, data[40..72] == merkle_tree.key,
       status == Active.
    6. CPI invoke_signed -> spl-account-compression::append(commitment).
    7. issuer.credentials_issued += 1.
    8. Emit CredentialIssued.
```

**Trust boundaries crossed**: issuer → schema-registry (via Account<SchemaAccount>) → schema-registry SchemaTreeBinding (via UncheckedAccount + parser) → spl-account-compression.

**Verified invariants**:
- Schema match (post SEC-003).
- Tree match (post SEC-003).
- Status active (post SEC-003).
- Issuer approved (state machine).

### 2.2 Holder side: building proof (holder.generateBatchProof)

```
1. Sort credentials by schemaHash (SEC-20 canonical ordering).
2. Identity cohesion check (SOLID-SEC-033):
   for each cred i:
     derived_kp_i = deriveCredentialKey(masterPrivKey, schema_i)
     require: cred_i.holderPubKeyX == derived_kp_i.public_key_x
3. Per-credential global-state inclusion proof:
   identity_leaf_i = Poseidon3(derived_pub_x_i, derived_pub_y_i, revocationNonce)
   fetch siblings/path from adapter.
4. Per-credential issuer-tree inclusion proof:
   issuer_leaf_i = Poseidon5(authority, bjj_x, bjj_y, statusEpoch, revocationNonce)
   fetch siblings/path; cross-check root == options.issuerTreeRoot.
5. Build circuit input (32 public + lots of private inputs).
6. snarkjs.groth16.fullProve(circuit, wasm, zkey).
7. publicSignals[0] becomes the on-chain nullifier (read directly).
8. formatProofForSolana for groth16-solana BE convention.
```

**Trust boundaries**: holder → MerkleProofAdapter (Helius DAS or LocalReplica) → snarkjs prover.

**Critical**: the production path reads `publicSignals[0]` directly as nullifier (no off-chain Poseidon recompute). This means the (broken) `holder.generateProof` single-credential path with its 16-input Poseidon arity bug is dead code; only `generateBatchProof` ships.

### 2.3 Verification path (zk-verifier::verify_batch_proof)

```
Verifier SDK:
  1. buildVerifyBatchProofIx(payer, request, trees):
     - Hand-roll Anchor discriminator (sha256("global:verify_batch_proof")[0..8])
     - Borsh layout: 8(disc) + 64(A) + 128(B) + 64(C) + 4(Vec len) + 32*32(public_inputs) + 32(nullifier)
     - 11 accounts in canonical order.

On-chain (programs/zk-verifier::verify_batch_proof):
  Pre-flight: !paused, vk_initialized.
  Arity gate: public_inputs.try_into() to [u8;32; 32].
  Nullifier binding: nullifier == public_inputs[0].
  Verifier scope: public_inputs[29] == ID.to_bytes() (BE per SOLID-SEC-031).
  Timestamp: bytes [8..32] of public_inputs[31] zero, claim_ts in [now-skew, now+skew].
  Global-tree binding:
    require_keys_eq!(global_tree.owner, SCHEMA_REGISTRY_ID)
    cpi_helpers::verify_state_root_matches(data, public_inputs[1])
  Issuer-tree binding:
    require_keys_eq!(issuer_tree_binding.owner, ISSUER_REGISTRY_ID)
    cpi_helpers::verify_issuer_tree_binding_for_proof(data, public_inputs[10])
  Schema loop (i in 0..4, skip if both root and hash zero):
    schema strictly ascending vs prev
    require_keys_eq!(schema_tree_i.owner, SCHEMA_REGISTRY_ID)
    cpi_helpers::verify_schema_root_binding(data, public_inputs[2+i], public_inputs[6+i])
  Groth16 verify (delegated to verify_groth16_proof, #[inline(never)]):
    VkBuf::parse(vk_storage.data)
    negate_g1_point(proof_a)
    Groth16Verifier::<32>::new + verify
  Atomic nullifier PDA init (replay protection).
  Metrics (proof_count++) + emit CredentialVerified.
```

**Trust boundaries**: 11 accounts, 5 trust gates per tree PDA, 1 atomic init for replay.

**CU envelope**: ~280-345K CU, ~75% headroom against the 1.4M ceiling. Dominated by alt_bn128 syscalls (~250-300K CU).

---

## 3. Cross-program dependency map

```
zk-verifier
  ├─ uses: solid-light (cpi_helpers parsers, ID constants)
  ├─ reads: GlobalStateBinding (owned by schema-registry)
  ├─ reads: SchemaTreeBinding[0..4] (owned by schema-registry)
  └─ reads: IssuerTreeBinding (owned by issuer-registry)

issuer-registry
  ├─ uses: solid-light (verify_schema_tree_binding_for_issue, ID constants)
  ├─ uses: solid-core (BJJ subgroup, Poseidon, identity ops)
  ├─ uses: schema-registry (typed SchemaAccount, no-entrypoint)
  ├─ reads: SchemaAccount + SchemaTreeBinding (issue_credential)
  └─ writes: IssuerAccount, IssuerTreeBinding, SPL-AC issuer-tree leaves

schema-registry
  └─ uses: solid-core (compute_schema_hash_from_parts)
  └─ writes: SchemaAccount, SchemaTreeBinding, GlobalStateBinding,
             SPL-AC schema-tree initialization

solid-core (no on-chain CPIs)
  └─ provides primitives to all three programs (BPF dual-target)

solid-light (no entry point)
  └─ provides parsers + ID constants to all three programs

solid-wasm (no on-chain consumer)
  └─ wraps solid-core for TS SDK consumption
```

**Observations**:
- `zk-verifier` reads cross-program PDAs but never CPIs (no signing, no state mutation across programs). Pure consumer pattern.
- `issuer-registry` is the most-connected program: typed cross-program reads (SchemaAccount), pattern-matched cross-program reads (SchemaTreeBinding via solid-light), CPIs into spl-account-compression.
- `solid-light` is the layout contract bridge — it owns the byte offsets that all three programs trust.
- No circular dependencies; no CPI from zk-verifier to issuer-registry or schema-registry.

---

## 4. Highest-priority HIGH+ findings (consolidated)

| ID | Source | Finding | Status |
|----|--------|---------|--------|
| **CU-H01 / SEC-048** | 04_compute | `register_issuer` exceeds 1.4M CU on BPF without bypass; mainnet-blocking | OPEN; bypass active |
| **M02-H01 / SEC-045** | 02_issuer_registry | Atomic handlers don't update `IssuerTreeBinding.current_root`; pre-revocation replay window | OPEN |
| **M02-H02** | 02_issuer_registry | No CI gate against shipping `sec007-skip-onchain` to mainnet | OPEN |
| **M07-H01 / SEC-012** | 07_circuits | Single-party trusted setup; mainnet blocker | OPEN |
| **M06-H01 / SEC-010** | 06_wasm_bridge | Cross-language vectors only cover 2 of 9 primitives | OPEN |
| **M08-CRITICAL01** | 08_ts_sdk | `holder.generateProof` (single-credential) broken at runtime (Poseidon arity 16 > 12 cap) | DEAD CODE — confirm + delete |
| **M09-H01** | 09_scripts_tests_tools | 10 of 11 integration tests not implemented | OPEN |
| **M09-H02** | 09_scripts_tests_tools | `tools/solid-prover` uses `test_rng()` — unsafe for production | NON-PROD; document or fix |
| **DEP-H01** | 03_dependencies | `light-poseidon` upgrade-blocked; pin to exact `=0.2.0` | OPEN; trivial fix |
| **DEP-H02** | 03_dependencies | `groth16-solana 0.2` pulls dual ark-* trees | OPEN; structural |
| **DEP-H03** | 03_dependencies | `ed25519-dalek 1.0.1` (RUSTSEC-2022-0093); pinned by solana 1.18 | OPEN; gated by anchor 0.31 cutover |
| **DEP-H04** | 03_dependencies | `curve25519-dalek 3.2.1` (RUSTSEC-2024-0344) | OPEN; same gate |
| **M01-M01** (zk-verifier) | 01_zk_verifier | No two-step authority handover; typo locks program | Pre-mainnet item |
| **M01-M02** (zk-verifier) | 01_zk_verifier | No host-testable harness for 8 pre-Groth16 gates | OPEN |

**Critical / Blocker**: SEC-048 (CU-H01), SEC-045 (M02-H01), SEC-012 (M07-H01), M02-H02 (build-side enforcement) — all four must close before mainnet.

---

## 5. Doc / Code consistency: drift items found during this audit

(Full inventory in `05_docs_drift/docs_drift.md`. The integration audit cites the most load-bearing drifts here.)

| Where | What | Impact |
|-------|------|--------|
| `circuits/batch_credential_query.circom:103-106` | Says `issuerAuthority` is BE; SDK + Rust use LE | Doc only — code is correct |
| `programs/zk-verifier/src/lib.rs:25-40` | Slot doc-comment uses inclusive-end notation | Cognitive trap; constants are correct |
| `crates/solid-light/src/credential_tree.rs:62` | Says nullifier is 3-input Poseidon | Stale; current is 6-input ADR-0006 rev |
| `docs/MODULE_CONTRACTS.md:710-717` | `GlobalStateBinding` documented as 104 bytes with `global_tree_pubkey` | Wrong; actual is 80 bytes, no such field |
| `docs/MODULE_CONTRACTS.md:723-725` | Says `register_schema` "DISABLED -- SEC-002 fix pending" | Stale; fix landed |
| `docs/MODULE_CONTRACTS.md:741-744, 746-747` | `update_tree_root` / `update_global_root` signatures wrong | Stale args list |
| `ts-sdk/packages/core/src/index.ts:435` | "Solana verifier expects 31 inputs" | Stale; now 32 (post ADR-0014) |
| `ts-sdk/packages/core/src/index.ts:381-403` | `QueryBuilder._computeContextHash` concatenates; circuit interleaves | Helper unused in proof path; would be a bug if used |

---

## 6. The four mainnet blockers (in dependency order)

1. **SEC-012**: multi-party trusted-setup ceremony (replaces single-party `setup.js`). Must come first because subsequent fixes (SEC-006 part 2, SEC-007 in-circuit) require fresh ceremony.
2. **SEC-048 / SEC-007**: BJJ subgroup check on-chain (or cheaper alternative). Recommended Option E (in-circuit) — batches with #1.
3. **SEC-045 (M02-H01)**: atomic handler updates IssuerTreeBinding.current_root in same ix.
4. **SEC-043**: `issuer_tree_operator` migrates from single-signer to Squads 3-of-5 / DAO threshold PDA.

Plus operational/CI:
- **SEC-010 / M06-H01**: extend cross-language vectors 2→9.
- **SEC-046 / CU-M01**: CU regression gate.
- **M02-H02**: CI gate against shipping `sec007-skip-onchain` to mainnet.
- **DEP-M04**: commit `tools/solid-prover/Cargo.lock`.
- **M09-H01**: integration tests 02-11.

---

## 7. What's working well

- **Defense-in-depth on tree binding reads**: 5 layers (Anchor seed-program, owner check, length, discriminator, content match, status check). Excellent.
- **Drift gates**: `scripts/check_program_ids.py` + `cpi_helpers.rs::id_bytes_tests` are robust 4-way invariant validation.
- **VK lifecycle (ADR-0015)**: chunked + freeze + 48h timelock + content-addressed artifact (SOLID-SEC-041 closed). Clean state machine.
- **6-input nullifier (ADR-0014)**: epoch-binds proof to issuer-tree root; both directions of revocation boundary closed.
- **BPF stack-frame discipline**: `lto = "thin"` + `#[inline(never)]` + compile-time `assert!(VkBuf < 1024)` — durable fix for SEC-047.
- **Three-target Poseidon dispatch**: `solid-core` cleanly splits BPF / non-wasm host / wasm32 with byte-identity by construction (`light-poseidon 0.2.0` shared backend).
- **Circuit padding integrity (SOLID-SEC-029)**: integrity zero-constraints on padding slots prevent state smuggling.
- **Schema-hash double-check (SEC-002 fix)**: circuit matches Rust matches on-chain registry handler; tested with `test_compute_schema_hash_parts_matches_definition`.
- **SOLID-SEC-001**: every public-input index used by the verifier is range-checked in-circuit (numPredicates, compoundLogic, queryCredentialIndices, queryFieldIndices).
- **Replay protection**: Anchor's `init` constraint on nullifier PDA gives O(1) atomic replay-safety. ADR-0007.

---

## 8. What needs investigation (uncertain status)

1. **`compound_query.circom` (275 LOC)** — is this circuit alive or dead? Not mentioned in CLAUDE.md. References in code (`holder.computeQueryContextHash` doc comment line 661 cites it). If alive, audit separately. If dead, delete (M07-INVESTIGATE-02).
2. **`generateProof` (single-credential)** — broken at runtime per M08-CRITICAL01. Confirm not called in production; if confirmed dead, delete.
3. **`tools/solid-prover`** — uses `test_rng()`. Confirm not in production hot path; if it ever ships as the production prover, replace RNG.
4. **CI workflow gate completeness** — verify `cross_language_vectors` and `check_program_ids.py` run in CI (M09-M01 / M09-M02).
5. **ADR-0014 `issuer_tree_leaf_index` cap (2^32)** — `revoke_issuer_atomic` does `leaf_index.try_into::<u32>()` with `IssuerTreeLeafIndexTooLarge` error. ~4B issuers is a hard cap; document or relax pre-mainnet.

---

## 9. Synthesis

**Soundness**: the protocol's privacy and integrity guarantees are sound, modulo SEC-048 (BJJ subgroup bypass) and SEC-045 (atomic handler binding root lag). Both are tracked and have concrete remediations.

**Build / CI hygiene**: very strong on declarative invariants (program ID drift gates, byte-layout tests, byte-identity vectors for the two pinned primitives). Weak on behavioural test coverage (1/11 integration tests) and CU regression (no gate yet).

**Code quality**: minimal `unwrap()`s (only 3 instances in issuer-registry, all on length-guaranteed slices). No reachable panic paths in the on-chain hot paths. Clean error variant taxonomy across all programs. Good comment density on load-bearing constructs.

**Dependency health**: structural pins (light-poseidon, wasm-bindgen, solana-program 1.18) are explicit and documented. Two HIGH-class advisories (ed25519-dalek 1.0.1, curve25519-dalek 3.2.1) are upstream-pinned and unblock only on the anchor 0.30→0.31 + solana 1.18→2.x cutover.

**Documentation**: ADRs are dense and accurate. `docs/MODULE_CONTRACTS.md` has multiple drifts (M03-DOCS items) but is recoverable. CLAUDE.md is current.

**Net assessment**: v0.6.1 is in solid shape for a pre-mainnet external audit. The four mainnet blockers (SEC-012, SEC-048, SEC-045, SEC-043) are all tracked with concrete remediation paths. The cross-language byte-identity contract is sound where exercised; the SOLID-SEC-010 gap is the highest-leverage close-out item that requires no circuit changes.

---

End of integration audit.
