# SolID Protocol

**Private Onchain Identity Infrastructure for Solana**

[![License](https://img.shields.io/badge/license-Apache--2.0%2FMIT-blue)](LICENSE)

> Prove who you are without revealing who you are.

SolID enables **selective disclosure** and **privacy-preserving verification** of identity credentials on Solana using Zero-Knowledge proofs. Built on Light Protocol for compressed state, Groth16 for proof verification, and BabyJubJub for signature schemes.

---

## 🏗 Architecture

```
┌──────────────────────── Applications ────────────────────────┐
│  Healthcare dApps  │  Hospitality  │  Supply Chain  │  DeFi  │
└───────────────────────────┬──────────────────────────────────┘
                            │
┌───────────────── TypeScript SDK ─────────────────────────────┐
│  @solid-protocol/core     │  WASM-powered crypto             │
│  @solid-protocol/issuer   │  Credential issuance             │
│  @solid-protocol/holder   │  ZK proof generation (snarkjs)   │
│  @solid-protocol/verifier │  On-chain proof submission       │
│  @solid-protocol/light    │  Light Protocol integration      │
└───────────────────────────┬──────────────────────────────────┘
                            │
┌─────────────── On-Chain Programs (Anchor) ───────────────────┐
│  zk-verifier       │  Groth16 proof verification + nullifiers│
│  issuer-registry   │  DAO-governed trust (stake/vote/slash)  │
│  schema-registry   │  Modular credential schemas             │
└───────────────────────────┬──────────────────────────────────┘
                            │
┌──────────────── Foundation Infrastructure ───────────────────┐
│  Light Protocol  │  Compressed state trees (~100x cheaper)   │
│  groth16-solana  │  Native alt_bn128 syscall verification    │
│  circomlib       │  Proven ZK circuit primitives             │
└──────────────────────────────────────────────────────────────┘
```

## 🚀 Quick Start

### Prerequisites

- **Rust** 1.80+ with `wasm32-unknown-unknown` target
- **Solana CLI** 2.0+ & **Anchor CLI** 0.30+
- **Circom** 2.1+ & **snarkjs** 0.7+
- **Node.js** 18+

### Build & Test Core

```bash
# Clone the repo
git clone https://github.com/your-org/solid-protocol.git
cd solid-protocol

# Run all 40 core crypto tests
cargo test -p solid-core

# Build WASM module
cd wasm && wasm-pack build --target web && cd ..

# Compile ZK circuit
cd circuits && npm install && npm run compile && cd ..

# Build Anchor programs
anchor build
```

### Deploy to Devnet

```bash
solana config set --url devnet
solana airdrop 5
anchor deploy --provider.cluster devnet
```

## 📦 Packages

| Package | Type | Description |
|---|---|---|
| `solid-core` | Rust crate | Poseidon hash, BabyJubJub EdDSA, commitments, nullifiers |
| `solid-wasm` | Rust → WASM | Browser/Node.js bridge for solid-core |
| `@solid-protocol/core` | TypeScript | WASM loader + QueryBuilder DSL |
| `@solid-protocol/issuer` | TypeScript | Credential issuance + Light Protocol tree insertion |
| `@solid-protocol/holder` | TypeScript | Groth16 proof generation (snarkjs) |
| `@solid-protocol/verifier` | TypeScript | On-chain proof submission |
| `@solid-protocol/light` | TypeScript | Light Protocol compressed tree operations |
| `zk-verifier` | Anchor | Groth16 verification + nullifier registry |
| `issuer-registry` | Anchor | DAO-governed issuer trust management |
| `schema-registry` | Anchor | Modular schema definitions |

## 📖 Documentation

- [Architecture Overview](docs/architecture.md)
- [Integration Guide](docs/integration-guide.md)
- [Issuer Guide](docs/issuer-guide.md)
- [Verifier Guide](docs/verifier-guide.md)
- [Circuit Design](docs/circuits.md)
- [Key Management](docs/key-management.md)
- [Light Protocol Integration](docs/light-protocol.md)
- [Schema Reference](docs/schemas.md)

## 🔐 How It Works

### 1. **Issuer** attests a credential

The issuer hashes credential data with Poseidon, signs it with BabyJubJub EdDSA, and inserts the commitment into a Light Protocol compressed Merkle tree. Cost: **~0.00001 SOL** per credential.

### 2. **Holder** generates a ZK proof

The holder runs a Groth16 circuit proving: "I hold a valid credential where `age >= 21 AND country == US`" — without revealing their actual age, country, or identity.

### 3. **Verifier** checks the proof on-chain

The verifier submits the proof to the ZK Verifier program, which validates it using alt_bn128 native syscalls and records the nullifier to prevent replay.

## 🏛 Trust Model

- **DAO-Governed Issuer Registry**: Issuers stake SOL and are approved via token-weighted voting
- **Slashing**: Malicious issuers lose their stake
- **Nullifiers**: Each proof produces a unique nullifier scoped to the verifier, preventing double-use while maintaining holder privacy

## 📄 License

Dual licensed under [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option.
