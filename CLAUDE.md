# CLAUDE.md

Auto-loaded context for Claude Code sessions working in this repo.

## What this repo is

solid-protocol is private onchain identity infrastructure for Solana. It uses
Groth16 plus alt_bn128 syscalls for on-chain proof verification, BabyJubJub
EdDSA with Poseidon for issuance signatures, and SPL Account Compression for
credential trees. Three Anchor programs (zk-verifier, issuer-registry,
schema-registry) plus a TypeScript SDK plus Circom circuits.

Current release is v0.3 (April 2026, post-remediation). See
docs/POST_REMEDIATION_AUDIT.md for the canonical state-of-the-protocol
assessment.

## Hard invariants

The following are load-bearing. Breaking any of them will silently compromise
soundness or availability:

- Anchor.toml, declare_id literals in programs/*/src/lib.rs, and
  deployments/*.json must all agree. CI enforces via
  scripts/check_program_ids.py.
- Canonical program IDs:
  - zk_verifier: BZkVFdMhAEeGMvEAhXNjt3r3bEA2sCPqEFcsEbSbFGj2
  - issuer_registry: CRGYfonXwDk6gKEm9fC1U33VVBkqnQVD3sPdLKzqHWoR
  - schema_registry: DPk6XUH6CArLWt4KMqJmpNBnPwQ3gG9P3dBd3MDVE3bT
- Rust and TypeScript primitives must agree byte-for-byte. The
  cross_language_vectors CI job hard-gates this via tests/vectors/.
- The on-chain verifier must owner-check global_tree and schema_tree_N
  against schema_registry's program ID. Removing the owner check re-opens
  the forged-trust-root attack.
- VerifierConfig layout includes next_vk_chunk (2 bytes). Any change to
  VerifierConfig requires bumping the constant VerifierConfig::SPACE.
- circuits/batch_credential_query.circom has NR_PUBLIC_INPUTS = 31 with a
  fixed index scheme. zk-verifier's verify_batch_proof depends on this
  exact ordering. See docs/circuits.md for the layout.

## Build sequence

Run from the repo root:

```
# Circuits (produces .wasm and .zkey for the current circuit)
cd circuits && npm install && node scripts/setup.js && cd ..

# WASM bridge for the TS SDK
wasm-pack build crates/solid-core --target nodejs \
    --out-dir ts-sdk/packages/core/wasm --release

# Anchor programs
anchor build

# TS SDK workspace
cd ts-sdk && npm ci && npm run build && cd ..
```

End-to-end against localnet:

```
solana-test-validator --reset &
anchor deploy --provider.cluster localnet
ts-node scripts/initialize.ts
ts-node scripts/issue.ts
ts-node scripts/prove.ts
```

Pre-commit gates to run locally:

```
cargo fmt --all -- --check
cargo clippy -p solid-core -p solid-light -- -D warnings
cargo test -p solid-core -p solid-light
cargo test -p zk-verifier --lib
python3 scripts/check_program_ids.py
```

## Toolchain pins

Outside Nix you must match these exactly. flake.nix and scripts/bootstrap.sh
do this for you inside Nix.

- rust 1.79.0 with wasm32-unknown-unknown target
- solana-cli 1.18.22
- anchor-cli 0.30.1
- circom 2.1.9
- snarkjs 0.7.5
- wasm-pack 0.13.1
- node 18 with npm 10+

Recent cargo versions (1.95+) work for host tests but anchor build still
wants the pinned rust-toolchain.

## Doc conventions

- Do not use emoji or unicode box-drawing characters in any doc under docs/
  or in the root README. Plain ASCII markdown only.
- Use file:line references when pointing to code.
- docs/POST_REMEDIATION_AUDIT.md is the canonical post-fix audit. Prior audit
  docs under docs/SOLID_*.md are marked historical.
- Update docs/IMPROVEMENTS_ROADMAP.md when closing or adding a P0 / P1 / P2
  item, so CI and audits share the same backlog.

## Open work (M2 and M3 scope)

- Revocation v1 operator workflow. Circuit and on-chain support are in
  place. Needs holder SDK helper, issuer SDK helper, and indexer event
  contract. See docs/REVOCATION_DESIGN.md.
- In-circuit issuer pubkey binding. Design choice pending between
  verify_batch_proof CPI-ing into issuer_registry::check_issuer_status or
  adding a compressed issuer tree with Merkle-membership proof in the
  circuit. Either requires a circuit change plus a new trusted setup.
- Integration test suite expansion. tests/integration/README.md lists 11
  scenarios; one is implemented.
- Multi-party trusted setup ceremony to replace circuits/scripts/setup.js
  before mainnet.
- External audit of the post-remediation codebase.

## Working-style notes

- Large asks are welcome; prefer dense, thorough output over abridged
  summaries.
- When an earlier document or assumption is wrong, correct it directly and
  explicitly. Do not paper over contradictions.
- Autonomous multi-step work is fine; report progress at natural
  checkpoints (per milestone, per program, per doc cluster).
