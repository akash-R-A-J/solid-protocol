# SolID Protocol — Infrastructure Improvements & Implementation Status

> **Last updated:** 2026-04-19 (post-remediation)
> Companion docs: [`SOLID_DEEP_AUDIT_2026_04.md`](./SOLID_DEEP_AUDIT_2026_04.md) (pre-fix), [`SOLID_POST_REMEDIATION_AUDIT_2026_04.md`](./SOLID_POST_REMEDIATION_AUDIT_2026_04.md), [`SOLID_FINAL_AUDIT_2026_04.md`](./SOLID_FINAL_AUDIT_2026_04.md), [`DEPLOYMENT_AND_TESTING.md`](./DEPLOYMENT_AND_TESTING.md).

This document tracks the evolution of SolID from concept to production-grade
identity primitive for Solana. It is the authoritative status record.

---

## 0. Build & test health

| Gate | Status | Command |
|---|---|---|
| Rust workspace compiles | ✅ | `cargo check --workspace` → exit 0, warnings only |
| Cross-language vectors agree | ✅ | `cargo run --example gen_vectors -p solid-core && ts-node tests/vectors/check_vectors.ts` |
| `anchor build` | ✅ | All three programs compile under Anchor 0.30.1 |
| Circuits compile | 🟡 | Compile fine; full Groth16 setup requires the 2¹⁶ powers-of-tau file (see `DEPLOYMENT_AND_TESTING.md`) |
| TS SDK builds | ✅ | `pnpm -r build` clean |

---

## 1. Security & Cryptographic Integrity

| Improvement | Status | Notes |
|---|---|---|
| Identity Binding (P0) | ✅ | `compound_query.circom` binds holder private key via `BabyPbk` |
| Privacy-First Key Hierarchy | ✅ | Master-derived, per-scope sub-keys |
| Commitment formula parity | ✅ | Rust / Circom / WASM all compute `Poseidon(dataHash, schemaHash, Ax, Ay, salt)`; cross-language vectors in `tests/vectors/` |
| Hardened Nullifier (5-arg) | ✅ | `Poseidon(masterKey, revNonce, verifier, queryHash, verifierNonce)` across all layers |
| Anti-Replay (atomic) | ✅ | **PDA-per-nullifier** in `zk-verifier`. Replay = `init` fails = tx reverts. |
| Schema ↔ Root Binding | ✅ | `verify_schema_root_binding` parses `SchemaTreeBinding` PDA (discriminator + root equality); no more `return true` |
| Canonical Ordering (SEC-20) | ✅ | Strictly ascending schema hashes enforced in both `multi_cred.rs` and `programs/zk-verifier/src/lib.rs` |
| Zero-Schema Integrity | ✅ | Circuit uses `enabled = 1 - isZero(schemaHash)` to bypass signature/Merkle checks for padded slots |
| Fraud-Proof Slashing | ✅ | `submit_fraud_proof` is authority-gated, ≤ 4 KB evidence, no reporter bounty (no DoS vector) |
| Paused / Authority Transfer | ✅ | `zk-verifier::set_paused`, `zk-verifier::transfer_authority` |
| Flash-Loan Governance | ✅ | 100-slot stake maturity in `issuer-registry::vote_on_issuer` |

---

## 2. Scalability & Efficiency

| Improvement | Status | Notes |
|---|---|---|
| Compressed credential leaves | 🟡 | On-chain verifier is backend-agnostic. TS `@solid-protocol/light` wraps `@lightprotocol/stateless.js`; the insert path still needs to attach the 36-byte credential tuple as the compressed-account payload (tracked as R-2 in the final audit). |
| Stateless verification | ✅ | Verifier consumes Merkle roots via PDAs; no per-credential on-chain account |
| Event-driven indexer hand-off | ✅ | `IssuerApproved` etc. events drive off-chain compressed-state writes instead of a fabricated CPI |

---

## 3. Decentralized Trust & Governance

| Feature | Status | Notes |
|---|---|---|
| DAO voting (tokens, not wallets) | ✅ | `stake_tokens` → `vote_on_issuer` with Quadratic weighting |
| Approve/Reject lifecycle | ✅ | `finalize_voting` emits `IssuerApproved`; no hardcoded auto-approval |
| Cooldown withdrawal (14 d) | ✅ | `request_withdrawal` → `withdraw_after_cooldown` |
| Rejected-issuer refund | ✅ | `withdraw_stake` for rejected issuers (no dead-code duplicate) |
| Tiered Staking (Community/Enterprise/Regulated/Government) | ✅ | `register_issuer` with `checked_mul` overflow guard |

---

## 4. Developer & User Experience

| Improvement | Status | Notes |
|---|---|---|
| `@solid-protocol/core` single source of truth | ✅ | Exports `MAX_CREDENTIALS`, `MAX_PREDICATES`, `NUM_FIELDS`, `TREE_DEPTH`, `PROGRAM_IDS`, `OP_MAP`, `computeIdentityCommitment`, `computeNullifier` |
| `@solid-protocol/holder` | ✅ | Real 5-arg nullifier + proper `queryContextHash`; no placeholder bytes |
| `@solid-protocol/verifier` | ✅ | Real `buildVerifyBatchProofIx` + `verifyOnChain` sending a confirmed tx; `checkIssuerStatus` reads `IssuerAccount` PDA |
| `@solid-protocol/light` | 🟡 | Merkle-proof fetch is real; full insert/revoke payload wiring pending (honest labels in file header) |
| `@solid-protocol/sdk` (one-call) | ✅ | Unified wrapper with resilient RPC failover |
| WASM zero-copy Poseidon | ✅ | `poseidonHashShared` path |
| Multi-credential batch (N=4) | ✅ | `generateBatchProof` with 31 public inputs |
| Native wallet integration | ⭕ | Not started |
| `solid-prover` (native Rust prover) | ✅ | Builds standalone via `cargo build --manifest-path crates/solid-prover/Cargo.toml`; excluded from default workspace because `ark-circom 0.5.0-alpha` pins an incompatible `num-bigint` |

---

## 5. Cross-language contract

- **Rust-generated reference vectors:** `tests/vectors/commitment_and_nullifier.json`
- **TS verifier:** `tests/vectors/check_vectors.ts` (`✔ commitment matches`, `✔ nullifier matches`)
- **Circom:** witness generation consumes the same hex in `circuits/tests/` (add vectors test-bench next iteration).

---

## 6. Ecosystem interoperability

| Feature | Status |
|---|---|
| Solana Attestation Service (SAS) data availability | ✅ — SolID is the computation layer over SAS data |
| W3C VC translation | ⭕ Not started |
| Indexer APIs (Helius / Triton) | 🟡 Events exist (`CredentialVerified`, `IssuerApproved`); no hosted indexer adapter yet |

---

## 7. Remaining work (residual gaps)

| ID | Scope | Notes |
|---|---|---|
| R-2 | TS `@solid-protocol/light` insert path | Attach the 36-byte `(commitment, schemaHash, issuer, nonce)` tuple as compressed-account data; or swap the backend to `spl-account-compression` — the on-chain verifier is agnostic. |
| R-3 | Per-credential revocation circuit | Add `revocationRoot` public input to `compound_query.circom` and a non-membership check. |
| R-6 | `solid-prover` dep pin | Track `ark-circom` for a release that relaxes the `num-bigint = 0.4.3` exact-version pin. |
| R-7 | W3C VC translation layer | Bidirectional mapping to JSON-LD Verifiable Credentials. |
| R-8 | Native mobile wallet build | Requires a `wasm-pack` `web` target + React Native bridge. |

---

## 8. Implementation principles (unchanged; still binding)

1. **Mathematical consistency crate-first.** All cryptographic logic lives in
   `solid-core`; circuits and programs consume it, never redefine it.
2. **Full constraint enforcement, no placeholders.** A function that claims a
   check must perform the check. This was violated in the pre-remediation
   tree (`return true;`) and has been closed.
3. **Versioned state evolution.** Account discriminators are explicit so
   tomorrow's schema migrations do not invalidate today's PDAs.
4. **Shared test vectors.** The `tests/vectors/` directory is the only place
   byte-level cryptographic truth is declared; Rust, TS, and Circom consume
   it; CI enforces equality.
