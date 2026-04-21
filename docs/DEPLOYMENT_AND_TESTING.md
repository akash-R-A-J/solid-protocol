# Deployment and End-to-End Testing

v0.3, April 2026. Post-remediation canonical workflow.

This document walks you from a clean checkout to a verified proof on-chain.
It is the reference for the complete development loop. Every command is
expected to succeed as written, without workarounds.

## Prerequisites

Install the pinned toolchain. Using the Nix flake is recommended:

```
nix develop
```

Outside Nix, install each of the following at the exact version:

- rust 1.79.0 with the wasm32-unknown-unknown target
- solana-cli 1.18.22
- anchor-cli 0.30.1
- circom 2.1.9
- snarkjs 0.7.5
- wasm-pack 0.13.1
- node 18
- npm 10 or higher

scripts/bootstrap.sh will fetch and pin these into .toolchain/bin if you
prefer an explicit install tree.

Verify program-ID consistency before doing anything else. This catches the
most common source of deployment errors.

```
python3 scripts/check_program_ids.py
```

## Build all artifacts

```
# Rust workspace tests (primitives, zk-verifier host tests, solid-prover)
cargo test -p solid-core -p solid-light
cargo test -p zk-verifier --lib
cargo test --manifest-path tools/solid-prover/Cargo.toml

# BPF build of the three Anchor programs
anchor build

# Circuits: compile and run the trusted setup
cd circuits && npm install && node scripts/setup.js && cd ..

# WASM bridge for the TS SDK
wasm-pack build crates/solid-core --target nodejs \
    --out-dir ts-sdk/packages/core/wasm --release

# TypeScript SDK
cd ts-sdk && npm ci && npm run build && cd ..
```

If the circuit step fails for insufficient powers-of-tau entropy, open
circuits/scripts/setup.js and follow the comments for regenerating the
phase-1 ptau file. For mainnet, replace this single-party ceremony with a
multi-party Hermez-style ceremony.

## Deploy the programs

### Localnet

```
solana-test-validator --reset &
solana config set --url localhost
solana airdrop 10
anchor deploy --provider.cluster localnet
```

### Devnet

```
solana config set --url devnet
solana airdrop 2
solana airdrop 2

anchor deploy --provider.cluster devnet

# Record the canonical manifest.
python3 scripts/regen_devnet_manifest.py > deployments/devnet.json
python3 scripts/check_program_ids.py   # must be green after every deploy
```

If check_program_ids.py fails at this point, follow
docs/PROGRAM_ID_RECONCILIATION.md.

## Initialize on-chain state

Initialisation is idempotent and safe to re-run. It uses the program IDs in
Anchor.toml; override SOLID_RPC_URL, SOLID_TREE_PUBKEY, and
SOLID_GOVERNANCE_MINT via the environment if you need to pin them.

```
SOLID_RPC_URL="http://127.0.0.1:8899" ts-node scripts/initialize.ts
```

This runs six steps, in order:

1. initialize_registry on issuer-registry with governance_token_mint,
   min_stake, voting_period, and approval_threshold_bps.
2. register_schema on schema-registry for the reference
   basic_identity_v1 schema.
3. initialize_tree_binding binding the schema to an SPL AC credential tree.
4. initialize_global_binding creating the GlobalStateBinding PDA.
5. initialize on zk-verifier, creating verifier_config.
6. store_verification_key in 900-byte chunks until the key is fully loaded.

scripts/e2e_state.json records every PDA and tree pubkey the downstream
scripts need.

### Before step 3: create the credential tree

The SPL AC tree must already exist and have its tree authority set to the
tree-authority PDA derived from the schema hash. For convenience you can
call createCredentialTree from @solid-protocol/light, or use the standard
SPL AC initialisation flow. Set SOLID_TREE_PUBKEY before running
initialize.ts so step 3 writes the correct binding.

## Issue a credential

```
ts-node scripts/issue.ts
```

The script:

- Generates a fresh issuer BabyJubJub keypair, a fresh holder master
  keypair, and a fresh Solana authority for the issuer.
- Derives the holder's per-schema keypair with the remediated
  deriveCredentialKey primitive.
- Signs the commitment under the issuer's BabyJubJub key.
- Calls issuer-registry::issue_credential which CPIs into SPL AC
  append via the ["tree-authority", schema_hash] PDA.
- Persists the credential, both holder keypairs, and the issuer authority
  secret into scripts/e2e_state.json.

## Generate a proof and verify on-chain

```
ts-node scripts/prove.ts
```

The script:

- Reads the credential and keypairs from scripts/e2e_state.json.
- Seeds a LocalReplicaAdapter for the schema's Merkle tree and for the
  global-state tree. The global-state leaf uses the remediated per-schema
  identity commitment.
- Runs generateBatchProof via @solid-protocol/holder. This is a real
  Groth16 proof over the circuits/build/batch_credential_query_final.zkey.
- Submits verify_batch_proof via @solid-protocol/verifier and captures the
  confirmed transaction signature.
- Re-submits the same transaction and asserts that the nullifier PDA's
  init constraint rejects it.

Expected output includes a successful verification tx signature and a
"replay rejected" confirmation. If either fails, check the post-remediation
audit in docs/POST_REMEDIATION_AUDIT.md for the specific regression gate.

## Integration test suite

tests/integration/ contains a suite of bankrun-based scenario tests
covering every P0 and P1 regression gate plus the happy-path end-to-end.
See tests/integration/README.md for the scenarios and the harness they
run in. Run them with:

```
cd ts-sdk && npm run test:integration
```

## Mainnet checklist

Do not deploy to mainnet-beta until every item below is satisfied:

- All P0 and P1 items in docs/IMPROVEMENTS_ROADMAP.md are marked complete.
- A multi-party trusted setup ceremony has completed with published
  contributor attestations.
- The circuit artifacts are hosted on IPFS and Arweave with content
  addressing, not on cdn.solid-protocol.com.
- AUTHORITY_PUBKEY and SOLID_TOKEN_MINT in ts-sdk/packages/sdk/src/config.ts
  are set to real production keys.
- All three program upgrade authorities have been transferred to a 3-of-5
  multisig, such as a Squads Protocol vault.
- At least one third-party audit (OtterSec, Halborn, or Trail of Bits)
  has completed with no open findings at High or above.
- Helius DAS or equivalent production Merkle-proof indexer is wired in via
  a HeliusDasAdapter implementation of MerkleProofAdapter.
- Monitoring and alerting are in place for CredentialIssued,
  CredentialVerified, and SchemaBindingFrozen events.
- deployments/mainnet.json is regenerated from regen_devnet_manifest.py.
