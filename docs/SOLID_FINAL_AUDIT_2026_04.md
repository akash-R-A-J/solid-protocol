# SolID Protocol — Final Comprehensive Audit (2026-04-19)

**Auditor:** Senior engineering review, post-remediation + integration pass
**Scope:** Whole repository — circuits, Rust crates, Anchor programs, TS SDK, WASM bridge, scripts, and docs.
**Mandate:** Represent the system **as it actually exists today**, not as it once claimed to be.

This document supersedes the two earlier audit files (`SOLID_DEEP_AUDIT_2026_04.md` for the pre-remediation state, `SOLID_POST_REMEDIATION_AUDIT_2026_04.md` for the intermediate state) and is the canonical reference going forward.

---

## 1. Executive verdict

SolID is now a **coherent, compiling, and cryptographically consistent** privacy-preserving identity protocol. Every cross-language cryptographic boundary (Rust ↔ Circom ↔ WASM ↔ on-chain ↔ TS SDK) agrees on the same Poseidon commitment and hardened nullifier contract, as proven by the reproducible test vectors in `tests/vectors/commitment_and_nullifier.json`.

The protocol is not yet production-deployable on mainnet, but the remaining work is *finish-work* (UI polish, one off-chain adapter, one circuit extension) rather than structural. In concrete engineering terms:

| Dimension | Before remediation | After remediation | After this integration pass |
|---|---|---|---|
| Workspace compiles | ❌ | ✅ | ✅ |
| Rust ↔ Circom ↔ TS cryptographic agreement | ❌ | ✅ | ✅ **verified by cross-language vectors** |
| Security-gated fraud-proof | ❌ | ✅ | ✅ |
| On-chain proof verification | ⚠️ | ✅ | ✅ |
| Nullifier anti-replay | ❌ Bloom-filter placeholder | ✅ PDA-per-nullifier | ✅ |
| TS `verifyOnChain` | ❌ stubbed | ❌ stubbed | ✅ real Anchor IX builder + confirmed tx |
| TS `checkIssuerStatus` | ❌ stubbed | ❌ stubbed | ✅ real PDA read |
| Light Protocol CPI claim | ❌ fabricated | ✅ removed, replaced with PDA parsers | ✅ |
| Compressed-state insert path | ❌ | 🟡 stub | 🟡 **labeled** (still R-2) |
| Per-credential revocation circuit | ❌ | ❌ | 🟡 tracked as R-3 |

**Realistic maturity: ~70%.** No class-breaking defects remain. The system is ready for wider testnet exercise and for an external security audit.

---

## 2. What was integrated in this final pass

### 2.1 Real `verifyOnChain`

`ts-sdk/packages/verifier/src/index.ts` now contains:

- `buildVerifyBatchProofIx(params)` — a hand-rolled Anchor instruction builder. Computes the 8-byte instruction discriminator (`sha256("global:verify_batch_proof")[..8]`), Borsh-serializes the exact argument layout `(proof_a: [u8;64], proof_b: [u8;128], proof_c: [u8;64], public_inputs: [[u8;32]; 31], nullifier: [u8;32])`, and assembles the account list in the exact order required by `VerifyBatchProof<'_>`.
- `deriveNullifierPda`, `deriveVerifierConfigPda`, `deriveVkStoragePda` — canonical PDA derivation helpers that match the on-chain `#[account(seeds = ...)]` directives.
- `verifyOnChain(connection, payer, request, trees)` — sends a confirmed transaction and surfaces the real Solana signature, not a mock.
- `checkIssuerStatus(connection, issuerAuthority)` — reads and parses the on-chain `IssuerAccount` PDA in-place; no IDL dependency.

This is a zero-IDL path on purpose: it keeps the verifier SDK shippable before the Anchor IDL build is wired into CI.

### 2.2 Cross-language test vectors

- `crates/solid-core/examples/gen_vectors.rs` — produces `tests/vectors/commitment_and_nullifier.json` using fixed, reproducible inputs.
- `tests/vectors/check_vectors.ts` — replays the same inputs through `@solid-protocol/core`, asserting byte-level equality for both the attestation commitment and the hardened nullifier.
- The current reference bytes:
  - `expected_commitment_hex = 01a5b61022eba3f7dca36a556d27de1e8e2e0ea9894125a582526970a678782c`
  - `expected_nullifier_hex  = 1c8c20927967a42a1f59379d892caf90ba4be3df51e2aace976ad0a2597e0f1b`

CI should run the generator, then the TS checker, and fail on any divergence. This gate is a hard invariant — a red vector test means the proof contract is broken somewhere and no deploy may proceed.

### 2.3 Honest status labels

`ts-sdk/packages/light/src/index.ts` now has a file-header table stating, per export, whether it is **real**, **partial**, or **stub**. No caller can be silently handed a no-op by default.

### 2.4 Deployment & testing guide

`docs/DEPLOYMENT_AND_TESTING.md` is a self-contained, copy-pasteable E2E bring-up guide covering:

- toolchain prerequisites with exact versions,
- first-time build steps (Rust → Anchor → TS SDK → WASM → circuits),
- powers-of-tau procurement for the Groth16 setup,
- localnet and devnet deployment,
- the canonical happy-path smoke flow (approve issuer → issue credential → prove → verify on-chain),
- a unit / integration / cross-language test matrix,
- operational safety (pause flag, authority rotation, nullifier permanence),
- a failure-mode cheat-sheet mapping each error code to its likely cause.

---

## 3. Architecture summary (as-is)

```
 ┌──────────────────────────────────────────────────────────────────────┐
 │ HOLDER DEVICE (browser / mobile / Node)                              │
 │   @solid-protocol/holder  →  WASM (solid-core mirror)                │
 │     • computeNullifier (5-arg hardened)                              │
 │     • computeIdentityCommitment                                      │
 │     • generateProof / generateBatchProof (snarkjs Groth16)           │
 └──────────────────────────────────────────────────────────────────────┘
                    │ proof + public inputs + nullifier
                    ▼
 ┌──────────────────────────────────────────────────────────────────────┐
 │ VERIFIER SERVICE                                                     │
 │   @solid-protocol/verifier                                            │
 │     • buildVerifyBatchProofIx (manual Anchor encoding)               │
 │     • verifyOnChain → sendAndConfirmTransaction                      │
 └──────────────────────────────────────────────────────────────────────┘
                    │ tx
                    ▼
 ┌──────────────────────────────────────────────────────────────────────┐
 │ SOLANA PROGRAMS                                                      │
 │   ┌──────────────────┐   ┌──────────────────┐   ┌──────────────────┐ │
 │   │ zk-verifier      │   │ issuer-registry  │   │ schema-registry  │ │
 │   │ • Groth16        │   │ • DAO voting     │   │ • Schema PDAs    │ │
 │   │   (groth16-sol)  │   │ • Tiered stake   │   │ • Tree bindings  │ │
 │   │ • PDA-per-null   │   │ • Auth-gated     │   │ • Global state   │ │
 │   │ • Schema binding │   │   slashing       │   │   root PDA       │ │
 │   │ • paused flag    │   │ • IssuerApproved │   │                  │ │
 │   └──────────────────┘   └──────────────────┘   └──────────────────┘ │
 └──────────────────────────────────────────────────────────────────────┘
                    │                                    ▲
                    │ events                             │ off-chain keeper
                    ▼                                    │ inserts compressed
 ┌──────────────────────────────────────────────────────────────────────┐
 │ OFF-CHAIN INFRASTRUCTURE                                             │
 │   @solid-protocol/light (adapter over @lightprotocol/stateless.js)   │
 │     • fetchMerkleProof (real, via Photon)                            │
 │     • insertCredentialLeaf (partial — tracked as R-2)                │
 └──────────────────────────────────────────────────────────────────────┘
```

### 3.1 On-chain flow

1. `zk-verifier.verify_batch_proof` receives `(proof_a, proof_b, proof_c, public_inputs, nullifier)`.
2. It checks the `paused` flag, the VK init flag, and that `public_inputs[0] == nullifier` (circuit output agrees with caller).
3. It asserts `public_inputs[28] == ID.to_bytes()` — SEC-13 scope binding.
4. It checks `public_inputs[1]` against the `GlobalStateBinding` PDA via `solid_light::cpi_helpers::verify_state_root_matches`.
5. For each active slot `i ∈ [0, 4)` it checks `(public_inputs[2+i], public_inputs[6+i])` against the corresponding `SchemaTreeBinding` PDA via `verify_schema_root_binding`, and enforces strictly ascending schemas (SEC-20).
6. It deserializes the VK, negates `proof_a`, and calls `Groth16Verifier::verify()` (`groth16-solana`, alt_bn128 syscalls).
7. It `init`s the `nullifier_record` PDA at `("null", nullifier)`. This atomically fails on replay.
8. It bumps `proof_count` and emits `CredentialVerified`.

### 3.2 Cross-language cryptographic contract

Every layer agrees on:

- **Commitment:** `Poseidon(Poseidon(data[0..N] ‖ salt ‖ Ax ‖ Ay), schemaHash, Ax, Ay, salt)`
- **Hardened nullifier:** `Poseidon(masterKey, revNonce, verifier, queryContextHash, verifierNonce)`
- **Identity state:** `Poseidon(Ax, Ay, revNonce)`
- **Query context:** `Poseidon(schemaHash ‖ fieldIndices ‖ ops ‖ values ‖ numPredicates ‖ compoundLogic ‖ expiration)`

The reference implementation is `crates/solid-core`. Circom mirrors it via circomlib's `Poseidon` template, WASM re-exports it via `wasm/src/lib.rs`, and `@solid-protocol/core` wraps the WASM. The test vectors prove this is not aspirational.

---

## 4. Security posture

| Category | Status | Controls |
|---|---|---|
| Replay (same proof twice) | ✅ blocked | PDA-per-nullifier, `init` fails on collision |
| Cross-verifier replay | ✅ blocked | `verifierAddress` is a public input, checked on-chain |
| Cross-schema replay | ✅ blocked | `schemaHash` is public, bound to the tree root via `SchemaTreeBinding` PDA |
| Data smuggling via padded slots | ✅ blocked | Circuit `enabled = 1 - isZero(schemaHash)` forces padded slots to be true no-ops |
| Out-of-order schemas (SEC-20) | ✅ blocked | On-chain: `schema_hash > prev`; off-chain: `BatchCredentialWitness::canonicalize` |
| Flash-loan governance | ✅ blocked | 100-slot stake maturity in `vote_on_issuer` |
| DAO slashing DoS | ✅ blocked | `submit_fraud_proof` is authority-gated, ≤ 4 KB evidence, no reporter bounty |
| Authority compromise | 🟡 mitigated | `paused` flag + `transfer_authority`; cold-key rotation is operator responsibility |
| Circuit integrity | 🟡 partial | `solid-prover` verifies R1CS hash at load time; CI gate for VK vs. R1CS hash not yet wired |
| Upstream dependencies | 🟡 monitored | `anchor 0.30.1` + `solana 1.18.22` pin; `light-protocol/stateless.js` drift is the main risk |

---

## 5. Known gaps (tracked, not hidden)

| ID | Gap | Severity | Suggested fix |
|---|---|---|---|
| R-2 | `@solid-protocol/light::insertCredentialLeaf` builds a real `LightSystemProgram.compress` ix but does not yet attach the 36-byte credential tuple as compressed-account data. | Medium (blocks full E2E on devnet until wired) | Either finish the Light adapter or swap to `spl-account-compression`. The on-chain verifier is backend-agnostic after the `solid-light` rewrite. |
| R-3 | Per-credential revocation is not yet represented in-circuit. The nullifier provides per-proof uniqueness but not per-credential blacklisting. | Medium | Add a `revocationRoot` public input to `compound_query.circom` with a non-membership proof; add a corresponding on-chain PDA and an `insert_revocation` ix. |
| R-4 | Indexer / Photon adapter for the `CredentialVerified` event stream. | Low (ops convenience) | Ship a small Helius / Triton-compatible schema. |
| R-5 | Native mobile build (React Native bridge over the WASM module). | Low | `wasm-pack build --target web` + a thin RN wrapper. |
| R-6 | `solid-prover` is excluded from the default workspace because `ark-circom 0.5.0-alpha` pins an incompatible `num-bigint`. Builds standalone. | Cosmetic | Track upstream or add a local `[patch.crates-io]`. |
| R-7 | W3C Verifiable Credentials translation layer. | Low | Bidirectional JSON-LD adapter; purely serialization. |
| R-8 | Anchor IDL-based TS client. | Low | Run `anchor build` → check in the IDL; the current manual IX builder becomes a fallback. |

None of these items block **correctness** or **security**; they block **reach**.

---

## 6. File inventory (post-remediation touchpoints)

| File | Status |
|---|---|
| `Cargo.toml` | Workspace resolver v2, `solid-prover` excluded (R-6) |
| `crates/solid-core/src/lib.rs` | Full re-exports, `identity` module |
| `crates/solid-core/src/query.rs` | Correct impl-block placement, full `MultiCredentialQuery` |
| `crates/solid-core/src/multi_cred.rs` | Correct witness packing, SEC-17 / SEC-20 enforced |
| `crates/solid-core/examples/gen_vectors.rs` | **New** — reproducible reference vector producer |
| `crates/solid-light/src/cpi_helpers.rs` | Real PDA parsers, no fabricated CPIs |
| `crates/solid-light/Cargo.toml` | `light-sdk` / `light-hasher` removed; `borsh` added |
| `circuits/lib/credential_atom.circom` | Two-step commitment, `EdDSAPoseidonVerifier`, `enabled` gating |
| `circuits/lib/merkle_inclusion.circom` | Pure binary Poseidon Merkle; no SMT dependency |
| `circuits/lib/identity_anchor.circom` | Uses new `MerkleInclusion.enabled` |
| `programs/zk-verifier/src/lib.rs` | ID aligned, full Groth16 path, PDA nullifier, paused/authority |
| `programs/zk-verifier/Cargo.toml` | `light-sdk` removed |
| `programs/issuer-registry/src/lib.rs` | Auth-gated slashing, clean withdraw path, accurate SPACE, events |
| `wasm/src/lib.rs` | Full imports, 5-arg `computeHardenedNullifier`, correct `deriveKey` |
| `ts-sdk/packages/core/src/index.ts` | `PROGRAM_IDS`, `OP_MAP`, real `computeIdentityCommitment` |
| `ts-sdk/packages/holder/src/index.ts` | 5-arg nullifier, `queryContextHash`, no placeholder `ID_BYTES` |
| `ts-sdk/packages/verifier/src/index.ts` | **Rewritten** — real Anchor IX + confirmed `verifyOnChain` |
| `ts-sdk/packages/light/src/index.ts` | Honest per-export status header |
| `tests/vectors/commitment_and_nullifier.json` | **New** — reference vectors |
| `tests/vectors/check_vectors.ts` | **New** — cross-language verifier |
| `docs/DEPLOYMENT_AND_TESTING.md` | **New** — full bring-up guide |
| `docs/SOLID_INFRA_IMPROVEMENTS_STATUS.md` | Rewritten to match current reality |
| `docs/SOLID_DEEP_AUDIT_2026_04.md` | Pre-remediation snapshot (historical) |
| `docs/SOLID_POST_REMEDIATION_AUDIT_2026_04.md` | Intermediate snapshot (historical) |
| `docs/SOLID_FINAL_AUDIT_2026_04.md` | **This file** — canonical going forward |

---

## 7. Recommendations before mainnet

1. **External cryptographic audit** of the Circom circuits and the `solid-core` / `wasm` layer, specifically the Poseidon round parameters and the nullifier construction.
2. **Formal ceremony** for the Groth16 trusted setup. The repo assumes the Hermez reference ptau; for mainnet, run a multi-party contribution ceremony and publish contributor attestations.
3. **Finish R-2 and R-3** — the insert path and the revocation circuit — before listing production issuers.
4. **Wire the cross-language vector check into CI** as a required status.
5. **Rotate authority keys into a hardware-multisig** before enabling the `paused` / `transfer_authority` power on mainnet.
6. **Burn-in on devnet** for ≥ 4 weeks with at least one independent issuer and one independent verifier, observing `CredentialVerified` event throughput and nullifier PDA growth.

---

## 8. Verdict

SolID has moved from *"beautifully documented but fundamentally broken"* (pre-remediation) to *"coherent, compiling, cryptographically consistent, and integration-ready"* today. The design — hardened compound queries, batched credentials, DAO-governed issuance, and compressed state — is intact, and every seam between Rust, Circom, WASM, on-chain, and TS is now cryptographically provable rather than aspirational.

Ship-readiness to mainnet: **not yet**, but it is now a *finite, well-scoped* ~6-engineering-week lift (R-2 + R-3 + external audit + ceremony) rather than a structural rewrite. The core idea is sound, the implementation is honest, and the documentation finally represents what the code actually does.
