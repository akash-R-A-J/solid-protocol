# Architecture Overview

> **Last refreshed:** 2026-04-20 (v0.2 — SPL Account Compression migration).
> The core thesis, trust model, and circuit contract are unchanged; only the
> compressed-state backend and its SDK wrapper have been replaced.

## System Layers

SolID Protocol is organized in 5 layers:

### Layer 0: Cryptographic Primitives

| Component | Implementation | Purpose |
|---|---|---|
| **Poseidon Hash** | `light-poseidon` (Circom-compatible) | SNARK-friendly hashing for commitments, nullifiers |
| **BabyJubJub EdDSA** | `ark-ed-on-bn254` | Issuer signatures over attestation commitments |
| **Concurrent Merkle Tree** | SPL Account Compression (`spl-account-compression`) | Credential storage with inclusion proofs |

All crypto is implemented in Rust (`solid-core`) and compiled to WASM for
browser use. The TypeScript SDK contains **zero crypto reimplementation** —
everything calls WASM.

### Layer 1: Foundation Infrastructure

- **SPL Account Compression** — Concurrent-Merkle-tree program maintained by
  the Solana Foundation / Solana Labs. Writes are bounded by the tree's
  `maxBufferSize`; reads go through `ConcurrentMerkleTreeAccount`, which
  exposes `getCurrentRoot()` to any Solana RPC. No Photon, no second
  indexer, no off-chain assumptions.
- **groth16-solana** — On-chain Groth16 verification using Solana's native
  `alt_bn128` syscalls.
- **Merkle proof adapters** (pluggable) — `LocalReplicaAdapter` for dev /
  E2E; a `HeliusDasAdapter` (or custom indexer) for production. The circuit
  contract for what it consumes is identical.

### Layer 2: On-Chain Programs

Three Anchor programs:

1. **ZK Verifier** (`programs/zk-verifier/`)
   - Verifies Groth16 proofs via `groth16_solana::Groth16Verifier` (alt_bn128 syscalls).
   - **PDA-per-nullifier replay protection** (atomic, O(1); `init` fails ⇒ tx reverts).
   - Reads Merkle roots indirectly via `schema-registry::SchemaTreeBinding`
     PDAs — making the verifier agnostic to the compressed-state backend.
   - Verification key stored in a separate `VkStorage` PDA (10 KB,
     chunked upload).
   - Stack-owned `VkBuf` parser — no heap allocation, no `Box::leak`
     (ensures BPF stack budget is honored at compile time).

2. **Issuer Registry** (`programs/issuer-registry/`)
   - Full DAO-governed trust management (stake / vote / slash).
   - 14-day withdrawal cooldowns; snapshot voting with 100-slot
     stake-maturity flash-loan guard.
   - **`issue_credential`** — CPIs to SPL AC `append` using an
     `issuer-registry`-owned `tree-authority` PDA (seeded by `schema_hash`).
     Emits `CredentialIssued` events for off-chain indexers.

3. **Schema Registry** (`programs/schema-registry/`)
   - Modular credential schema definitions with deprecation management.
   - **`SchemaTreeBinding` PDAs** (145 bytes, discriminator `b"schmtree"`)
     — link each schema to the concurrent Merkle tree it writes into and
     the current root.
   - **`GlobalStateBinding` PDA** (80 bytes, discriminator `b"globroot"`)
     — singleton that tracks the identity-state tree root.
   - Authority-gated `update_tree_root` / `update_global_root`; status
     flag supports freezing a binding when rotating backends.

### Layer 3: SDK Layer

```
Rust Crates (workspace root):
├── solid-core   — Poseidon, BJJ EdDSA, commitments, nullifiers, queries, SAS types
└── solid-light  — SPL-AC binding parsers + verify-root CPI helpers

Rust Crates (separate workspace):
└── tools/solid-prover  — Native Groth16 prover via ark-circom
                          (isolated because ark-circom hard-pins num-bigint 0.4.3)

@solid-protocol/ (TypeScript):
├── core      — WASM loader + QueryBuilder DSL
├── light     — SPL Account-Compression adapter (tree creation, root
│               reads, MerkleProofAdapter interface, event parsing)
├── issuer    — Wraps issuer-registry::issue_credential on-chain
├── holder    — Groth16 proof generation (snarkjs + MerkleProofAdapter)
├── verifier  — On-chain proof submission
└── sdk       — Unified entrypoint + ResilientConnection (priority failover)
```

### Layer 4: Applications

Any dApp can integrate SolID by:
1. Installing `@solid-protocol/verifier` (or `@solid-protocol/sdk`).
2. Creating a `CompoundQuery` via `QueryBuilder`.
3. Sending the query to the holder.
4. Receiving and submitting the proof on-chain.

## Data Flow

```
Issuer                          Holder                         Verifier
  │                               │                               │
  │ 1. Compute commitment         │                               │
  │    (Poseidon hash)            │                               │
  │ 2. Sign with BJJ EdDSA        │                               │
  │ 3. issue_credential ─────────►│ (on-chain CPI → SPL AC append)│
  │ ──── credential bundle ─────► │                               │
  │                               │                               │
  │                               │ ◄──── compound query ──────── │
  │                               │                               │
  │                               │ 4. Fetch Merkle proof via     │
  │                               │    MerkleProofAdapter         │
  │                               │ 5. Build circuit inputs       │
  │                               │ 6. Generate Groth16 proof     │
  │                               │    (snarkjs + WASM)           │
  │                               │ ──── proof + nullifier ─────► │
  │                               │                               │
  │                               │                               │ 7. verify_batch_proof
  │                               │                               │    → alt_bn128 syscalls
  │                               │                               │ 8. check SchemaTreeBinding
  │                               │                               │ 9. init nullifier PDA
  │                               │                               │ ✅ Verified!
```

## Circuit Design

The compound query circuit (`compound_query.circom` / `batch_credential_query.circom`)
has 8 steps:

| Step | Operation | Type |
|---|---|---|
| 1 | Identity Anchoring | `IdentityAnchor` (global-state tree) |
| 2 | Key Derivation | `Poseidon(masterKey, schema)` |
| 3 | Credential Verification | `CredentialAtom` (per-schema tree) |
| 4 | Verify Issuer Signature | `EdDSA-Poseidon (BabyJubJub)` |
| 5 | Evaluate Predicates | Modular predicate evaluators |
| 6 | Apply Compound Logic | `AND` / `OR` |
| 7 | Check Expiration | `currentTimestamp ≤ expirationTimestamp` |
| 8 | Compute Nullifier | `Poseidon(masterKey, revNonce, verifier, queryCtxHash, verifierNonce)` |

## Key Design Decisions

1. **Separate BJJ Keys.** BJJ identity is independent from the Solana
   wallet. Enables multi-device use.
2. **Rust-First Crypto.** All primitives in Rust, compiled to WASM. No JS
   crypto.
3. **DAO Governance.** Full stake/vote/slash. No multisig shortcuts.
4. **Backend-Agnostic Verifier.** The verifier reads roots from
   `SchemaTreeBinding` / `GlobalStateBinding` PDAs. The compressed-state
   backend (currently SPL AC; previously Light Protocol) can be swapped
   without recompiling the circuit or re-running the trusted setup.
5. **Circomlib Compatible.** All Poseidon / EdDSA matches circomlib
   bit-for-bit, enforced by `tests/vectors/commitment_and_nullifier.json`
   in CI.
6. **SAS Data Layer.** Credentials mapped to SAS attestations via CPI types.
7. **Atomic Nullifier PDAs.** Deterministic anti-replay, O(1), no Bloom
   filter false positives.
8. **Separate VK Storage.** Verification key decoupled from verifier config
   for large circuit support; `VkBuf` parser is stack-owned, leak-free.

## v0.1 → v0.2 delta (for historical context)

| Area | v0.1 (Light-based) | v0.2 (SPL-AC) |
|---|---|---|
| Compressed state | Light Protocol stateless client + Photon indexer | SPL Account Compression + standard Solana RPC |
| TS `@solid-protocol/light` | Wraps `@lightprotocol/stateless.js` | SPL-AC adapter + `MerkleProofAdapter` interface |
| Insert path | TS-only stub | On-chain `issuer-registry::issue_credential` CPI to SPL AC |
| Root binding | `SchemaTreeBinding` PDA (same byte layout) | `SchemaTreeBinding` + `GlobalStateBinding` (same layout, now authority-gated updates) |
| Nullifiers | PDA-per-nullifier | PDA-per-nullifier (unchanged) |
| Circuit contract | 31 public inputs, 5-arg nullifier | 31 public inputs, 5-arg nullifier (unchanged) |
| Prover crate | Excluded from root workspace via `exclude = [...]` | Separate workspace at `tools/solid-prover/` (no exclude) |
| VK parsing | `Box::leak` heap allocation | Stack-owned `VkBuf`, no leak |

The **cryptographic contract did not change**, so v0.1 trusted-setup
artifacts are reusable across the migration. Only the *delivery mechanism*
for state — where leaves land and how proofs are fetched — has moved.
