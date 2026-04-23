# SolID Protocol

Private onchain identity infrastructure for Solana.

Prove who you are without revealing who you are.

SolID enables selective disclosure and privacy-preserving verification of identity
credentials on Solana using zero-knowledge proofs. It is built on SPL Account
Compression for credential state, Groth16 plus alt_bn128 syscalls for on-chain
proof verification, and BabyJubJub with Poseidon for the issuance signature
scheme.

v0.3 (April 2026). This version lands the full 2026-04 remediation set:
program, circuit, and SDK fixes that unblock end-to-end testing on localnet.
The on-chain verifier remains backend-agnostic: all cryptographic contracts
(commitment layout, nullifier derivation, circuit public-input ordering) are
preserved.

## Architecture

Layers, top to bottom.

```
Applications
  Healthcare dApps, Hospitality, Supply Chain, DeFi.
TypeScript SDK
  @solid-protocol/core      WASM-powered crypto primitives
  @solid-protocol/issuer    Credential issuance via SPL AC CPI
  @solid-protocol/holder    ZK proof generation via snarkjs
  @solid-protocol/verifier  On-chain proof submission
  @solid-protocol/light     SPL AC adapter and Merkle proof adapters
  @solid-protocol/sdk       High-level facade over the sub-packages
On-chain programs (Anchor)
  zk-verifier       Groth16 verification, PDA-per-nullifier replay protection
  issuer-registry   DAO stake and vote, SPL AC issue_credential, slashing,
                    IssuerTreeBinding + atomic revoke_issuer_atomic (ADR-0014)
  schema-registry   Schemas, SchemaTreeBinding, GlobalStateBinding
Foundation
  SPL Account Compression    concurrent Merkle trees
  groth16-solana             alt_bn128 syscall verification
  circomlib                  proven ZK circuit primitives
```

## Quick start

### Prerequisites

The full toolchain is pinned by flake.nix and scripts/bootstrap.sh. Outside Nix
install these versions exactly:

- Rust 1.79.0 with wasm32-unknown-unknown target
- Solana CLI 1.18.22
- Anchor CLI 0.30.1
- circom 2.1.9, snarkjs 0.7.5
- Node 18 with npm 10+
- wasm-pack 0.13.1

### Reproducible setup

```
nix develop
# or
bash scripts/bootstrap.sh
export PATH="$PWD/.toolchain/bin:$PATH"
```

A VS Code Devcontainer wraps the flake for one-click onboarding.

### Build and test

```
# Rust crates plus zk-verifier host tests
cargo test -p solid-core -p solid-light
cargo test -p zk-verifier --lib

# BPF build of the three Anchor programs
anchor build

# Circuits: compile then run the trusted setup
cd circuits && node scripts/setup.js && cd ..

# WASM bridge for the TS SDK
wasm-pack build crates/solid-core --target nodejs \
    --out-dir ts-sdk/packages/core/wasm --release

# TypeScript SDK
cd ts-sdk && npm ci && npm run build && cd ..
```

### Deploy to devnet

```
solana config set --url devnet
solana airdrop 2

# Sanity check: Anchor.toml, declare_id, deployments/devnet.json must agree.
python3 scripts/check_program_ids.py

anchor deploy --provider.cluster devnet

# Refresh deployments/devnet.json after the deploy completes.
python3 scripts/regen_devnet_manifest.py > deployments/devnet.json
```

If check_program_ids.py fails, follow
[docs/PROGRAM_ID_RECONCILIATION.md](docs/PROGRAM_ID_RECONCILIATION.md).

### End-to-end run (localnet)

```
solana-test-validator &
anchor deploy --provider.cluster localnet
ts-node scripts/initialize.ts
ts-node scripts/issue.ts
ts-node scripts/prove.ts
```

The scripts share state through scripts/e2e_state.json. initialize.ts takes
SOLID_TREE_PUBKEY and SOLID_GOVERNANCE_MINT from the environment if you want
to pin them to pre-existing accounts.

## Packages

| Package | Type | Description |
|---|---|---|
| solid-core | Rust crate | Poseidon hash, BabyJubJub EdDSA, commitments, nullifiers |
| solid-light | Rust crate | SPL AC binding helpers, schema and global root parsers |
| solid-prover | Rust crate (tools/solid-prover) | Native Groth16 prover via ark-circom |
| @solid-protocol/core | TypeScript | WASM loader plus QueryBuilder |
| @solid-protocol/issuer | TypeScript | issue_credential wrapper |
| @solid-protocol/holder | TypeScript | Groth16 proof generation, MerkleProofAdapter |
| @solid-protocol/verifier | TypeScript | On-chain proof submission, PDA helpers |
| @solid-protocol/light | TypeScript | SPL AC adapter, LocalReplicaAdapter, event parsing |
| @solid-protocol/sdk | TypeScript | One-call facade over core, holder, verifier, light |
| zk-verifier | Anchor | Groth16 verification with PDA-per-nullifier replay protection |
| issuer-registry | Anchor | DAO stake, vote, slash, issue_credential |
| schema-registry | Anchor | Schemas, SchemaTreeBinding, GlobalStateBinding |

## Documentation

- [Architecture Overview](docs/architecture.md)
- [State Compression, SPL Account Compression integration](docs/light-protocol.md)
- [Integration Guide](docs/integration-guide.md)
- [Issuer Guide](docs/issuer-guide.md)
- [Verifier Guide](docs/verifier-guide.md)
- [Circuit Design](docs/circuits.md)
- [Key Management](docs/key-management.md)
- [Schema Reference](docs/schemas.md)
- [Revocation Design (v1 and v1.1 SMT)](docs/REVOCATION_DESIGN.md)
- [Deployment and Testing](docs/DEPLOYMENT_AND_TESTING.md)
- [Program-ID Reconciliation Runbook](docs/PROGRAM_ID_RECONCILIATION.md)
- [Post-Remediation Audit (April 2026)](docs/POST_REMEDIATION_AUDIT.md)

## How it works

### Issuer attests a credential

The issuer hashes credential data with Poseidon, signs it with BabyJubJub EdDSA,
and calls issuer-registry::issue_credential. That instruction CPIs into SPL AC
append using an issuer-registry-owned tree-authority PDA. The leaf lands in a
per-schema concurrent Merkle tree whose root is tracked by
schema-registry::SchemaTreeBinding.

### Holder generates a proof

The holder:

1. Derives a per-schema BabyJubJub subkey via Poseidon(masterKey, schemaHash)
   then BabyPbk, exactly matching the circuit's IdentityAnchor.
2. Computes the per-schema identity leaf Poseidon(subAx, subAy, revocationNonce).
3. Fetches the Merkle inclusion proof for that leaf from the global-state tree.
4. Fetches the credential's Merkle proof from its schema-scoped tree.
5. Runs the batch Groth16 circuit proving for example
   "I hold a valid credential where age is at least 21 and country is US"
   without revealing age, country, or identity.

### Verifier checks on-chain

The verifier calls zk-verifier::verify_batch_proof, which:

1. Validates the Groth16 proof via alt_bn128 syscalls.
2. Confirms the proof is bound to this verifier program's ID.
3. Confirms each schema's Merkle root against its SchemaTreeBinding PDA,
   rejecting any account not owned by schema-registry.
4. Confirms the global-state root against the GlobalStateBinding PDA with the
   same ownership check.
5. ADR-0014: confirms the issuer-tree root against the IssuerTreeBinding PDA,
   owner-checked against issuer-registry.  The circuit additionally proves
   per-credential Merkle membership of a BJJ-bound issuer leaf, so revoking
   any issuer invalidates every pre-revocation proof (SOLID-SEC-004 /
   SOLID-SEC-008).
6. Initialises a fresh nullifier PDA via init, giving atomic O(1) replay
   protection.

## Trust model

- DAO-governed issuer registry. Issuers stake SOL and are approved via token-
  weighted voting with flash-loan protection (100-slot stake maturity).
- Slashing. Malicious issuers lose stake to the DAO treasury PDA via
  slash_issuer and submit_fraud_proof. Lamports move atomically, not just
  accounting.
- Voting discipline. vote_on_issuer increments active_votes_count and refuses
  votes after voting_ends_at. release_vote is required before the voter can
  unstake.
- Nullifiers (ADR-0006 Phase 2 revision). Each proof mints a unique
  Poseidon(masterKey, revNonce, verifierAddr, queryCtxHash, verifierNonce,
  issuerTreeRoot) nullifier PDA, scoped to the verifier AND to a specific
  issuer-tree epoch (so a revoked issuer's old proofs cannot replay after
  root rotation).
- Backend-agnostic verifier. The verifier parses the trust roots by byte
  offset and rejects any account whose owner is not the expected registry
  program (schema-registry for global / schema trees; issuer-registry for
  the issuer tree); storage backend can be swapped without a circuit change.

## License

Dual licensed under [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your
option.
