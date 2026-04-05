# Architecture Overview

## System Layers

SolID Protocol is organized in 5 layers:

### Layer 0: Cryptographic Primitives

| Component | Implementation | Purpose |
|---|---|---|
| **Poseidon Hash** | `light-poseidon` (Circom-compatible) | SNARK-friendly hashing for commitments, nullifiers |
| **BabyJubJub EdDSA** | `ark-ed-on-bn254` | Issuer signatures over attestation commitments |
| **Sparse Merkle Tree** | Light Protocol state trees | Credential storage with inclusion proofs |

All crypto is implemented in Rust (`solid-core`) and compiled to WASM for browser use. The TypeScript SDK contains **zero crypto reimplementation** — everything calls WASM.

### Layer 1: Foundation Infrastructure

- **Light Protocol** — Compressed state trees reduce credential storage from ~0.002 SOL to ~0.00001 SOL
- **groth16-solana** — On-chain Groth16 verification using Solana's native `alt_bn128` syscalls
- **Photon Indexer** — Queries compressed account state for Merkle proof generation

### Layer 2: On-Chain Programs

Three Anchor programs:

1. **ZK Verifier** (`zk-verifier/`)
   - Verifies Groth16 proofs via `groth16_solana::Groth16Verifier` (alt_bn128 syscalls)
   - Light Protocol Compressed State nullifier registry (Infinite capacity, deterministic lookup)
   - On-Chain Root Verification via CPI to Light Protocol program
   - Verification key stored in separate `VkStorage` PDA (10KB, chunked upload)
   - Events emitted for indexer consumption

2. **Issuer Registry** (`issuer-registry/`)
   - Full DAO-governed trust management
   - Issuers stake SOL to register with 14-day withdrawal cooldowns
   - Snapshot Voting: Weighted voting with flash-loan prevention (slot-tracking)
   - Programmable Slashing: Immediate slashing for ZK-provable fraud

3. **Schema Registry** (`schema-registry/`)
   - Modular credential schema definitions
   - Community-addable verticals
   - Deprecation management

### Layer 3: SDK Layer

```
Rust Crates:
├── solid-core   — Poseidon, BJJ EdDSA, commitments, nullifiers, queries, SAS types
└── solid-light  — Light Protocol CPI helpers (insert, revoke, verify root)

@solid-protocol/ (TypeScript):
├── core      — WASM loader + QueryBuilder DSL
├── light     — Light Protocol tree operations (Photon Indexer)
├── issuer    — Credential issuance + Light tree insertion
├── holder    — Groth16 proof generation (snarkjs + Photon)
└── verifier  — On-chain proof submission
```

### Layer 4: Applications

Any dApp can integrate SolID by:
1. Installing `@solid-protocol/verifier`
2. Creating a CompoundQuery via `QueryBuilder`
3. Sending the query to the holder
4. Receiving and submitting the proof on-chain

## Data Flow

```
Issuer                          Holder                         Verifier
  │                               │                               │
  │ 1. Compute commitment         │                               │
  │    (Poseidon hash)            │                               │
  │ 2. Sign with BJJ EdDSA        │                               │
  │ 3. Insert into Light tree     │                               │
  │ ──── credential bundle ─────→ │                               │
  │                               │                               │
  │                               │ ←──── compound query ──────── │
  │                               │                               │
  │                               │ 4. Fetch Merkle proof         │
  │                               │    (Photon Indexer)            │
  │                               │ 5. Build circuit inputs       │
  │                               │ 6. Generate Groth16 proof     │
  │                               │    (snarkjs + WASM)            │
  │                               │ ──── proof + nullifier ─────→ │
  │                               │                               │
  │                               │                               │ 7. Verify on-chain
  │                               │                               │    (groth16-solana)
  │                               │                               │ 8. Check nullifier
  │                               │                               │ 9. Check issuer registry
  │                               │                               │ ✅ Verified!
```

## Circuit Design

The compound query circuit (`compound_query.circom`) has 8 steps:

| Step | Operation | Type |
|---|---|---|
| 1 | Identity Anchoring | `IdentityAnchor` (Global depth=20) |
| 2 | Key Derivation | `Poseidon(masterKey, schema)` |
| 3 | Credential Verification | `CredentialAtom` (Local depth=20) |
| 4 | Verify Issuer Signature | `EdDSA-Poseidon (BabyJubJub)` |
| 5 | Evaluate Predicates | `Modular Predicate Evaluators` |
| 6 | Apply Compound Logic | `AND / OR` |
| 7 | Check Expiration | `currentTimestamp ≤ expirationTimestamp` |
| 8 | Compute Nullifier | `Poseidon(masterKey, context)` |

## Key Design Decisions

1. **Separate BJJ Keys** — BJJ identity is independent from Solana wallet. Enables multi-device use.
2. **Rust-First Crypto** — All primitives in Rust, compiled to WASM. No JS crypto.
3. **DAO Governance** — Full stake/vote/slash. No multisig shortcuts.
4. **Light Protocol (dual-layer)** — TS SDK for client operations + Rust `solid-light` crate for on-chain CPI.
5. **Circomlib Compatible** — All Poseidon/EdDSA matches circomlib bit-for-bit.
6. **SAS Data Layer** — Credentials mapped to SAS attestations via CPI types.
7. **Light Protocol Nullifiers** — Deterministic anti-replay with infinite capacity via compressed state.
8. **Separate VK Storage** — Verification key decoupled from verifier config for large circuit support.
