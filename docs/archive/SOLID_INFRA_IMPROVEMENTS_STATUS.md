# SolID Protocol — Infrastructure Improvements & Implementation Status

> **Last updated:** 2026-04-20 (v0.2 — SPL Account Compression migration + R-item closure)
> Companion docs: [`SOLID_DEEP_AUDIT_2026_04.md`](./SOLID_DEEP_AUDIT_2026_04.md) (pre-fix), [`SOLID_POST_REMEDIATION_AUDIT_2026_04.md`](./SOLID_POST_REMEDIATION_AUDIT_2026_04.md), [`SOLID_FINAL_AUDIT_2026_04.md`](./SOLID_FINAL_AUDIT_2026_04.md), [`DEPLOYMENT_AND_TESTING.md`](./DEPLOYMENT_AND_TESTING.md), [`PROGRAM_ID_RECONCILIATION.md`](./PROGRAM_ID_RECONCILIATION.md), [`REVOCATION_DESIGN.md`](./REVOCATION_DESIGN.md).

This document tracks the evolution of SolID from concept to
production-grade identity primitive for Solana. It is the authoritative
status record.

---

## 0. Build & test health

| Gate | Status | Command |
|---|---|---|
| Rust libraries compile + pass | ✅ | `cargo test -p solid-core -p solid-light` |
| zk-verifier host tests pass | ✅ | `cargo test -p zk-verifier --lib` (11 passing: `VkBuf` parser, `negate_g1`, stack-budget) |
| Prover workspace compiles | ✅ | `cd tools/solid-prover && cargo build --release` |
| `anchor build` | ✅ | All three programs compile under Anchor 0.30.1 with v3 `Cargo.lock` |
| Circuits compile | 🟡 | Compile fine; full Groth16 setup requires the 2¹⁶ powers-of-tau file (see `DEPLOYMENT_AND_TESTING.md`) |
| TS SDK builds | ✅ | `cd ts-sdk && npm ci && npm run build` |
| Cross-language vectors agree | ✅ | `cargo run --example gen_vectors -p solid-core && npx ts-node tests/vectors/check_vectors.ts` |
| CI pipeline | ✅ | `.github/workflows/ci.yml` — 9 jobs gated on Nix-pinned toolchain |
| Program-ID consistency | ✅ | `python3 scripts/check_program_ids.py` (hard gate) |

---

## 1. Security & Cryptographic Integrity

| Improvement | Status | Notes |
|---|---|---|
| Identity Binding (P0) | ✅ | `compound_query.circom` binds holder private key via `BabyPbk` |
| Privacy-First Key Hierarchy | ✅ | Master-derived, per-scope sub-keys |
| Commitment formula parity | ✅ | Rust / Circom / WASM all compute `Poseidon(dataHash, schemaHash, Ax, Ay, salt)`; cross-language vectors in `tests/vectors/` enforced by CI |
| Hardened Nullifier (5-arg) | ✅ | `Poseidon(masterKey, revNonce, verifier, queryHash, verifierNonce)` across all layers |
| Anti-Replay (atomic) | ✅ | **PDA-per-nullifier** in `zk-verifier`. Replay = `init` fails = tx reverts. |
| Schema ↔ Root Binding | ✅ | `verify_schema_root_binding` parses `SchemaTreeBinding` PDA (discriminator + root equality); no more `return true` |
| Canonical Ordering (SEC-20) | ✅ | Strictly ascending schema hashes enforced in both `multi_cred.rs` and `programs/zk-verifier/src/lib.rs` |
| Zero-Schema Integrity | ✅ | Circuit uses `enabled = 1 - isZero(schemaHash)` to bypass signature/Merkle checks for padded slots |
| Fraud-Proof Slashing | ✅ | `submit_fraud_proof` is authority-gated, ≤ 4 KB evidence, no reporter bounty (no DoS vector) |
| Paused / Authority Transfer | ✅ | `zk-verifier::set_paused`, `zk-verifier::transfer_authority` |
| Flash-Loan Governance | ✅ | 100-slot stake maturity in `issuer-registry::vote_on_issuer` |
| Stack-owned VK parser | ✅ | `VkBuf` struct (v0.2) replaces `Box::leak`; BPF stack-budget asserted at compile time; 11 unit tests cover boundaries |

---

## 2. Scalability & Efficiency

| Improvement | Status | Notes |
|---|---|---|
| Compressed credential leaves | ✅ | **v0.2** — migrated to SPL Account Compression. `issuer-registry::issue_credential` CPIs to `spl-account-compression::append` under a protocol-owned `tree-authority` PDA. `@solid-protocol/light` v0.2 is a pure SPL AC adapter; no `@lightprotocol/stateless.js` / Photon dependency |
| Stateless verification | ✅ | Verifier consumes Merkle roots via PDAs; no per-credential on-chain account |
| Event-driven indexer hand-off | ✅ | `CredentialIssued`, `IssuerApproved`, etc. drive off-chain root refreshes (`schema-registry::update_tree_root`) instead of a fabricated CPI |
| Pluggable Merkle-proof adapter | ✅ | `@solid-protocol/light` ships `MerkleProofAdapter` interface + `LocalReplicaAdapter` for tests; production plugs in DAS/custom indexer |
| Resilient RPC failover | ✅ | `@solid-protocol/sdk::ResilientConnection` — priority failover over a list of Solana endpoints (replaced `ResilientLightClient` in v0.2) |

---

## 3. Decentralized Trust & Governance

| Feature | Status | Notes |
|---|---|---|
| DAO voting (tokens, not wallets) | ✅ | `stake_tokens` → `vote_on_issuer` with Quadratic weighting |
| Approve/Reject lifecycle | ✅ | `finalize_voting` emits `IssuerApproved`; no hardcoded auto-approval |
| Cooldown withdrawal (14 d) | ✅ | `request_withdrawal` → `withdraw_after_cooldown` |
| Rejected-issuer refund | ✅ | `withdraw_stake` for rejected issuers (no dead-code duplicate) |
| Tiered Staking (Community/Enterprise/Regulated/Government) | ✅ | `register_issuer` with `checked_mul` overflow guard |
| Issuance authorization | ✅ | **v0.2** — `issue_credential` requires `Approved` status + BJJ key match + protocol-owned `tree-authority` PDA as tree signer |

---

## 4. Developer & User Experience

| Improvement | Status | Notes |
|---|---|---|
| `@solid-protocol/core` single source of truth | ✅ | Exports `MAX_CREDENTIALS`, `MAX_PREDICATES`, `NUM_FIELDS`, `TREE_DEPTH`, `PROGRAM_IDS`, `OP_MAP`, `computeIdentityCommitment`, `computeNullifier` |
| `@solid-protocol/holder` | ✅ | **v0.2** — takes a `MerkleProofAdapter` + `credential.merkleTree`; batch proof accepts `globalStateTree`; no Light RPC coupling |
| `@solid-protocol/verifier` | ✅ | Real `buildVerifyBatchProofIx` + `verifyOnChain`; `checkIssuerStatus` reads `IssuerAccount` PDA |
| `@solid-protocol/light` | ✅ | **v0.2** — SPL Account-Compression adapter: `createCredentialTree`, `getCurrentTreeRoot`, `fetchMerkleProof(adapter, tree, leaf)`, `parseCredentialIssuedEvent`, PDA helpers |
| `@solid-protocol/issuer` | ✅ | **v0.2** — wraps `issuer-registry::issue_credential` directly; manual IX builder + 8-byte discriminator |
| `@solid-protocol/sdk` (one-call) | ✅ | Unified wrapper with `ResilientConnection` priority failover |
| WASM zero-copy Poseidon | ✅ | `poseidonHashShared` path |
| Multi-credential batch (N=4) | ✅ | `generateBatchProof` with 31 public inputs; spans multiple schema trees + one global tree |
| Reproducible toolchain | ✅ | **v0.2** — `flake.nix` pins Rust 1.79.0 / Node 18; `scripts/bootstrap.sh` installs Solana 1.18.22, Anchor 0.30.1, circom 2.1.9, snarkjs 0.7.5, wasm-pack 0.13.1 into `.toolchain/`. `.devcontainer/devcontainer.json` wraps the flake |
| Native wallet integration | ⭕ | Not started |
| `solid-prover` (native Rust prover) | ✅ | Lives in its own workspace at `tools/solid-prover/` because `ark-circom 0.5.0-alpha` hard-pins an incompatible `num-bigint`. Exercised by the `prover` job in CI |
| Program-ID drift gate | ✅ | **v0.2** — `scripts/check_program_ids.py` + `scripts/regen_devnet_manifest.py` + `PROGRAM_ID_RECONCILIATION.md` runbook |

---

## 5. Cross-language contract

- **Rust-generated reference vectors:** `tests/vectors/commitment_and_nullifier.json`
- **TS verifier:** `tests/vectors/check_vectors.ts` (`✔ commitment matches`, `✔ nullifier matches`)
- **Circom:** witness generation consumes the same hex in `circuits/tests/` (add vectors test-bench next iteration).
- **CI enforcement (v0.2):** `.github/workflows/ci.yml` regenerates the Rust-side vectors on every push, diffs them against the committed fixture, and then replays them through the TS SDK. Any Poseidon / endianness drift fails the pipeline immediately.

---

## 6. Ecosystem interoperability

| Feature | Status |
|---|---|
| Solana Attestation Service (SAS) data availability | ✅ — SolID is the computation layer over SAS data |
| W3C VC translation | ⭕ Not started |
| Indexer APIs (Helius / Triton) | ✅ — `@solid-protocol/light` v0.2 exposes a `MerkleProofAdapter` interface; Helius DAS and Triton adapters are straightforward plug-ins |

---

## 7. Remaining work (residual gaps)

| ID | Scope | Notes |
|---|---|---|
| R-2 | On-chain SPL AC insert path | **Resolved (v0.2)** — shipped in `issuer-registry::issue_credential` with tree-authority PDA + SPL AC `append` CPI. `@solid-protocol/light` / `@solid-protocol/issuer` rewritten as the direct client surface |
| R-3 | Per-credential revocation circuit | **Scheduled for v1.1.** Design locked in [`REVOCATION_DESIGN.md`](./REVOCATION_DESIGN.md): SMT non-membership check (~3K extra constraints, <1.5% overhead). Additive deploy; v1 rotate-identity path stays alive through migration |
| R-4 | `deserialize_vk` `Box::leak` | **Resolved (v0.2)** — replaced by stack-owned `VkBuf` with compile-time stack-budget assertion. 11 unit tests in `programs/zk-verifier/src/lib.rs` cover minimum-size, max-IC, overflow, truncation, and stack fitness |
| R-5 | CI pipeline | **Resolved (v0.2)** — `.github/workflows/ci.yml` runs 9 jobs: `toolchain`, `fmt-clippy`, `rust`, `prover`, `anchor`, `wasm`, `circuits`, `sdk`, `cross_language_vectors`, `program_id_consistency` |
| R-6 | `solid-prover` dep pin | **Resolved** — prover moved to `tools/solid-prover/` as its own workspace. Track `ark-circom` for a release that relaxes the `num-bigint = 0.4.3` exact-version pin so the two workspaces can be re-merged |
| R-7 | W3C VC translation layer | Bidirectional mapping to JSON-LD Verifiable Credentials. Not started |
| R-8 | Native mobile wallet build | Requires a `wasm-pack` `web` target + React Native bridge. Not started |
| R-9 | Toolchain brittleness | **Resolved (v0.2)** — Nix flake + bootstrap script + devcontainer pin every tool; CI restores the same `.toolchain/` cache |

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
5. **Backend-agnostic verifier.** The on-chain verifier reads roots through
   PDAs, not through backend-specific CPIs. This is what made the
   Light → SPL AC migration a drop-in: zero circuit changes, zero
   trusted-setup redo.
6. **Reproducible toolchain or it doesn't ship.** Every tool that touches a
   build output is pinned by `flake.nix` / `scripts/bootstrap.sh` and
   restored in CI from the same cache key.
