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
   - Verifies Groth16 proofs from the compound query circuit
   - Maintains nullifier registry for anti-replay
   - Uses `groth16-solana` for efficient native verification

2. **Issuer Registry** (`issuer-registry/`)
   - Full DAO-governed trust management
   - Issuers stake SOL to register
   - Token-weighted voting for approval
   - Slashing mechanism for malicious issuers

3. **Schema Registry** (`schema-registry/`)
   - Modular credential schema definitions
   - Community-addable verticals
   - Deprecation management

### Layer 3: SDK Layer

```
@solid-protocol/
├── core      — WASM loader + QueryBuilder DSL
├── light     — Light Protocol tree operations
├── issuer    — Credential issuance pipeline
├── holder    — Groth16 proof generation
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
| 1 | Hash attestation data | `Poseidon(data[0..7])` |
| 2 | Compute commitment | `Poseidon(dataHash, schema, holderX, holderY, salt)` |
| 3 | Verify issuer signature | `EdDSA-Poseidon verification` |
| 4 | Verify Merkle inclusion | `SMT verification (depth=20)` |
| 5 | Evaluate predicates | `4 predicates × 7 operators` |
| 6 | Apply compound logic | `AND / OR` |
| 7 | Check expiration | `currentTimestamp ≤ expirationTimestamp` |
| 8 | Compute nullifier | `Poseidon(privKey, schema, nonce)` |

## Key Design Decisions

1. **Separate BJJ Keys** — BJJ identity is independent from Solana wallet. Enables multi-device use.
2. **Rust-First Crypto** — All primitives in Rust, compiled to WASM. No JS crypto.
3. **DAO Governance** — Full stake/vote/slash. No multisig shortcuts.
4. **Light Protocol** — Compressed Merkle trees for 100x storage savings.
5. **Circomlib Compatible** — All Poseidon/EdDSA matches circomlib bit-for-bit.
