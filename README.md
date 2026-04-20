# SolID Protocol

**Private Onchain Identity Infrastructure for Solana**

[![License](https://img.shields.io/badge/license-Apache--2.0%2FMIT-blue)](LICENSE)

> Prove who you are without revealing who you are.

SolID enables **selective disclosure** and **privacy-preserving verification**
of identity credentials on Solana using Zero-Knowledge proofs. Built on
**SPL Account Compression** for state, **Groth16 + alt_bn128 syscalls** for
on-chain proof verification, and **BabyJubJub / Poseidon** for the issuance
signature scheme.

> **v0.2 — April 2026.** The compressed-state backend has been migrated from
> Light Protocol's stateless client to SPL Account Compression. The on-chain
> verifier is backend-agnostic by design; all cryptographic contracts
> (commitment layout, nullifier derivation, circuit public-input ordering)
> are preserved.

---

## Architecture

```
┌──────────────────────── Applications ────────────────────────┐
│  Healthcare dApps  │  Hospitality  │  Supply Chain  │  DeFi  │
└───────────────────────────┬──────────────────────────────────┘
                            │
┌───────────────── TypeScript SDK ─────────────────────────────┐
│  @solid-protocol/core     │  WASM-powered crypto             │
│  @solid-protocol/issuer   │  Credential issuance (SPL AC CPI)│
│  @solid-protocol/holder   │  ZK proof generation (snarkjs)   │
│  @solid-protocol/verifier │  On-chain proof submission       │
│  @solid-protocol/light    │  SPL AC adapter + proof adapter  │
│  @solid-protocol/sdk      │  One-call resilient entrypoint   │
└───────────────────────────┬──────────────────────────────────┘
                            │
┌─────────────── On-Chain Programs (Anchor) ───────────────────┐
│  zk-verifier       │  Groth16 verification + nullifier PDAs  │
│  issuer-registry   │  DAO trust + issue_credential CPI       │
│  schema-registry   │  Schemas + tree / global-root bindings  │
└───────────────────────────┬──────────────────────────────────┘
                            │
┌──────────────── Foundation Infrastructure ───────────────────┐
│  SPL Account Compression │  Concurrent Merkle trees (append) │
│  groth16-solana          │  alt_bn128 syscall verification   │
│  circomlib               │  Proven ZK circuit primitives     │
└──────────────────────────────────────────────────────────────┘
```

## Quick Start

### Prerequisites

The full toolchain is pinned by `flake.nix` + `scripts/bootstrap.sh`.
Outside of Nix, install these versions exactly:

- **Rust** `1.79.0` (for a v3 `Cargo.lock` that Anchor's bundled Cargo accepts),
  with `wasm32-unknown-unknown`
- **Solana CLI** `1.18.22` (Anchor 0.30.1 links against solana-program 1.18.22)
- **Anchor CLI** `0.30.1` (install via AVM)
- **Circom** `2.1.9` · **snarkjs** `0.7.5`
- **Node.js** `18` · **npm** `10+` (or `pnpm` for contributor UX)
- **wasm-pack** `0.13.1`

### Reproducible setup (recommended)

```bash
# Enters a shell with every tool pinned to the versions above.
nix develop
# Or, if you don't use Nix, run the bootstrap directly:
bash scripts/bootstrap.sh
export PATH="$PWD/.toolchain/bin:$PATH"
```

A VS Code Devcontainer at `.devcontainer/devcontainer.json` wraps the flake
for one-click onboarding.

### Build & test

```bash
# Rust libraries (solid-core, solid-light) + zk-verifier unit tests
cargo test -p solid-core -p solid-light
cargo test -p zk-verifier --lib

# BPF build of the three Anchor programs
anchor build

# Circuit compile + Groth16 setup (see circuits/scripts/setup.js)
cd circuits && node scripts/setup.js && cd ..

# WASM bridge (nodejs target — used by the TS SDK)
wasm-pack build crates/solid-core --target nodejs \
  --out-dir ts-sdk/packages/core/wasm --release

# TypeScript SDK
cd ts-sdk && npm ci && npm run build
```

### Deploy to devnet

```bash
solana config set --url devnet
solana airdrop 2

# Sanity-check: Anchor.toml ↔ declare_id!() ↔ deployments/*.json must agree.
python3 scripts/check_program_ids.py

anchor deploy --provider.cluster devnet
```

If `check_program_ids.py` fails, follow
[`docs/PROGRAM_ID_RECONCILIATION.md`](docs/PROGRAM_ID_RECONCILIATION.md).

## Packages

| Package | Type | Description |
|---|---|---|
| `solid-core` | Rust crate | Poseidon hash, BabyJubJub EdDSA, commitments, nullifiers, SAS types |
| `solid-light` | Rust crate | SPL-AC binding helpers (parse `SchemaTreeBinding` / `GlobalStateBinding` PDAs, verify-root CPI) |
| `solid-prover` | Rust crate (separate workspace at `tools/solid-prover/`) | Native Groth16 prover via `ark-circom` |
| `@solid-protocol/core` | TypeScript | WASM loader + `QueryBuilder` DSL |
| `@solid-protocol/issuer` | TypeScript | Wraps `issuer-registry::issue_credential` (SPL AC CPI) |
| `@solid-protocol/holder` | TypeScript | Groth16 proof generation (snarkjs + pluggable `MerkleProofAdapter`) |
| `@solid-protocol/verifier` | TypeScript | On-chain proof submission |
| `@solid-protocol/light` | TypeScript | SPL Account-Compression adapter, proof fetching, event parsing |
| `@solid-protocol/sdk` | TypeScript | One-call resilient entrypoint (`ResilientConnection` failover) |
| `zk-verifier` | Anchor | Groth16 verification (alt_bn128) + PDA-per-nullifier replay protection |
| `issuer-registry` | Anchor | DAO stake/vote/slash + `issue_credential` |
| `schema-registry` | Anchor | Schema + tree / global-root bindings |

## Documentation

- [Architecture Overview](docs/architecture.md)
- [State Compression (SPL Account Compression)](docs/light-protocol.md) — was "Light Protocol integration"
- [Integration Guide](docs/integration-guide.md)
- [Issuer Guide](docs/issuer-guide.md)
- [Verifier Guide](docs/verifier-guide.md)
- [Circuit Design](docs/circuits.md)
- [Key Management](docs/key-management.md)
- [Schema Reference](docs/schemas.md)
- [Revocation Design (v1 / v1.1 SMT)](docs/REVOCATION_DESIGN.md)
- [Deployment & Testing](docs/DEPLOYMENT_AND_TESTING.md)
- [Program-ID Reconciliation Runbook](docs/PROGRAM_ID_RECONCILIATION.md)
- [Infrastructure Status](docs/SOLID_INFRA_IMPROVEMENTS_STATUS.md)

## How It Works

### 1. Issuer attests a credential

The issuer hashes credential data with Poseidon, signs it with BabyJubJub
EdDSA, and calls `issuer-registry::issue_credential` — which CPIs to SPL
Account Compression's `append` using an `issuer-registry`-owned
`tree-authority` PDA. The leaf lands in a per-schema concurrent Merkle
tree whose root is tracked by `schema-registry::SchemaTreeBinding`.

### 2. Holder generates a ZK proof

The holder fetches the current Merkle proof via a `MerkleProofAdapter`
(local replica for dev, DAS/indexer in production), then runs the
Groth16 circuit proving e.g. *"I hold a valid credential where
`age >= 21 AND country == US`"* — without revealing age, country, or
identity.

### 3. Verifier checks the proof on-chain

The verifier calls `zk-verifier::verify_batch_proof`, which:
1. Validates the Groth16 proof via `alt_bn128` syscalls.
2. Confirms each schema's Merkle root against the `SchemaTreeBinding` PDA.
3. Initializes a fresh nullifier PDA (atomic, O(1) anti-replay).

## Trust Model

- **DAO-Governed Issuer Registry.** Issuers stake SOL and are approved via
  token-weighted voting with flash-loan prevention (100-slot stake
  maturity).
- **Slashing.** Malicious issuers lose stake through `submit_fraud_proof`.
- **Nullifiers.** Each proof mints a unique `(masterKey, revNonce,
  verifier, queryHash, verifierNonce)` nullifier PDA — scoped to the
  verifier, unlinkable across sessions, O(1) replay protection.
- **Backend-agnostic verifier.** The on-chain verifier reads roots from
  `SchemaTreeBinding`/`GlobalStateBinding` PDAs; the compressed-state
  backend (currently SPL AC) can be swapped without a circuit change.

## License

Dual licensed under [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT),
at your option.
