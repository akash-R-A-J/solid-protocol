# SolID Protocol Architecture

v0.3, April 2026. Post-remediation release.

This document is the reference for every architectural decision in the
protocol. It is intended to be executable: every invariant it describes is
enforced in code, and every piece of code that implements it points back here.

## Programs

Three Anchor programs, strict separation of concerns.

### zk-verifier

Program ID: BZkVFdMhAEeGMvEAhXNjt3r3bEA2sCPqEFcsEbSbFGj2

Owns Groth16 verification and replay protection. Does not touch governance or
issuance. Does not know or care which storage backend produced the Merkle
roots it is asked to validate.

Accounts:

- verifier_config PDA seed "verifier-config"
- vk_storage PDA seed ["vk-storage", verifier_config]
- nullifier_record PDA seed ["null", nullifier_bytes], created atomically on
  every successful verify_batch_proof

Invariants:

- Maximum public input count is 31, matching batch_credential_query.circom.
- public_inputs[28] must equal the program ID, scope-binding the proof to
  this verifier.
- global_tree and schema_tree_N accounts must be owned by schema-registry.
  This owner check is load-bearing: without it a system-owned account
  carrying a forged globroot or schmtree discriminator would pass the raw
  byte-level parser and bypass the trust anchor.
- VkStorage has a cumulative cap of 10_240 bytes and a strict chunk
  sequence (next_vk_chunk in VerifierConfig).
- The nullifier public output must equal the nullifier seed byte string.
  Anchor's init constraint on the nullifier record PDA makes replay atomic.

### issuer-registry

Program ID: CRGYfonXwDk6gKEm9fC1U33VVBkqnQVD3sPdLKzqHWoR

DAO-governed issuer registry plus the on-chain entry point for credential
issuance into SPL AC.

Governance:

- RegistryConfig is 112 bytes including governance_token_mint.
  The pre-remediation space calculation omitted the mint and bricked every
  attempt at initialize_registry.
- vote_on_issuer increments active_votes_count and enforces
  clock.unix_timestamp less than issuer.voting_ends_at. Unstake refuses
  until the vote is released via release_vote.
- ReleaseVote marks both staker_account and vote_record as mut, otherwise
  Anchor silently discards the writes.
- slash_issuer and submit_fraud_proof move lamports from the shared
  stake_vault PDA to a dao_treasury PDA. Earlier versions only decremented
  accounting without ever moving funds.
- approve_via_trust_anchor takes target_authority as an instruction argument
  and the target_issuer is seed-constrained to ["issuer", target_authority].
  This prevents a malicious trust anchor from approving a crafted
  IssuerAccount.
- approve_via_trust_anchor emits IssuerApproved matching finalize_voting so
  off-chain indexers stay consistent across both approval paths.

Issuance:

- issue_credential signs a hand-rolled SPL AC append CPI using the
  ["tree-authority", schema_hash] PDA as the authorized appender.
- Emits CredentialIssued with the issuer, schema_hash, commitment, merkle
  tree pubkey, slot, and unix time. Off-chain indexers use this to mirror
  the refreshed tree root into schema_registry::update_tree_root.

### schema-registry

Program ID: DPk6XUH6CArLWt4KMqJmpNBnPwQ3gG9P3dBd3MDVE3bT

Registers schemas and manages the byte-level binding PDAs the zk-verifier
consumes.

Accounts:

- SchemaAccount PDA seed ["schema", name, version_byte]
- SchemaTreeBinding PDA seed ["schema-tree-binding", schema_hash]
- GlobalStateBinding PDA seed ["global-binding"]

Binding layouts (canonical byte contracts, read by cpi_helpers.rs):

```
SchemaTreeBinding (145 bytes)
  [0..8)    discriminator = b"schmtree"
  [8..40)   schema_hash
  [40..72)  tree_pubkey
  [72..104) current_root
  [104..112) last_updated_slot (u64 LE)
  [112..113) status (0 active, 1 frozen)
  [113..145) authority (Pubkey)

GlobalStateBinding (80 bytes)
  [0..8)    discriminator = b"globroot"
  [8..40)   current_root
  [40..48)  last_updated_slot (u64 LE)
  [48..80)  authority (Pubkey)
```

Invariants:

- register_schema enforces length caps on name, category, and every
  field_name, matching the allocated SchemaAccount Borsh space.
- update_tree_root and update_global_root refuse to regress the stored
  last_updated_slot. A compromised authority cannot roll the root back to a
  value that predates a revocation.
- transfer_tree_binding_authority and transfer_global_binding_authority
  allow rotation so a lost or compromised authority key does not brick the
  binding.
- increment_usage requires the schema authority as signer. Without this,
  any actor could inflate usage_count into overflow territory.

## Core crates

- solid-core exposes Poseidon, BabyJubJub EdDSA with deterministic nonce,
  commitment and nullifier composition, and identity state. Poseidon is the
  light-poseidon crate in circomlib-compatible mode. All primitives are
  covered by cross-language vectors in tests/vectors/.
- solid-light parses the binding layouts above. SCHEMA_REGISTRY_ID is a
  typed Pubkey constant the zk-verifier uses for the owner check.
- solid-prover is a native ark-circom prover, shipped as a separate workspace
  under tools/solid-prover.

## Circuits

Two entry points: batch_credential_query.circom (production) and
compound_query.circom (single credential, debugging and simpler use cases).

Both circuits share:

- IdentityAnchor template parameterised on GLOBAL_DEPTH. Per-credential
  identity leaves are Poseidon(credAx, credAy, revocationNonce) where
  credAx, credAy = BabyPbk(Poseidon(masterKey, schemaHash)).
- Hardened nullifier
  Poseidon(masterKey, revocationNonce, verifierAddress,
          queryContextHash, verifierNonce).

Batch-specific invariants:

- numPredicates is range-checked: numPredicates must be less than or equal
  to MAX_PREDICATES.
- compoundLogic is constrained to the set zero or one.
- ExpirationChecker runs per credential, gated by isZero for inactive slots.
  Before the remediation, currentTimestamp was declared public but never
  constrained and expired credentials silently passed.
- Schema hashes must be strictly ascending for active credentials.
- Zero-schema padding forces all per-credential private inputs to zero,
  including issuer public key, issuer signature, merkle root, data, salt,
  and expiration.

## SDK shape

- @solid-protocol/core wraps the WASM bridge and exposes typed primitives
  plus QueryBuilder.
- @solid-protocol/holder derives per-schema identity leaves with
  deriveCredentialKey and builds a circuit input matching the structure
  above. generateBatchProof fetches one global proof per credential and
  rejects divergent roots (stale indexer detection).
- @solid-protocol/verifier hand-rolls the Anchor discriminator and account
  ordering for verify_batch_proof. verifyOnChain returns a confirmed
  transaction signature.
- @solid-protocol/sdk is a thin facade: its prove and verifyOnChain methods
  delegate to holder and verifier and contain no stub returns.

## Backend-agnostic proof verification

The zk-verifier reads only the current root bytes from its two PDA
dependencies. The storage backend that supplied those roots is not named or
constrained beyond the owner being schema-registry. This allows an operator
to swap the SPL AC backend (currently used for credential trees) for a
native SolID tree or a future Light Protocol re-integration without any
circuit or program change.

## Event contracts

- IssuerApproved is emitted from finalize_voting and approve_via_trust_anchor.
  The payload is identical in both paths.
- CredentialIssued is emitted from issue_credential and carries the tree
  pubkey so a downstream indexer can match the SPL AC append event.
- CredentialVerified is emitted from verify_batch_proof and carries the
  nullifier plus the running proof_count.

## CI invariants

The pipeline enforces:

- cargo fmt and cargo clippy on both workspaces with deny-warnings.
- cargo test for solid-core, solid-light, the zk-verifier host tests, and
  the solid-prover test suite.
- anchor build and IDL generation for all three programs.
- wasm-pack build of crates/solid-core.
- circom compile for every circuit under circuits/.
- TypeScript build and per-package tests.
- cross_language_vectors: regenerate the Rust-side vectors, git diff exit
  code against tests/vectors/commitment_and_nullifier.json, then run the
  TypeScript-side check. Any byte-level divergence between Rust and WASM
  primitives fails CI.
- program_id_consistency: scripts/check_program_ids.py comparing Anchor.toml,
  declare_id literals, and deployments/*.json.

## Open architectural tasks

- M2.F1 Revocation v1. The revocationNonce rotation path is fully supported
  by the circuit and by update_global_root. What remains is the holder-side
  workflow and an issuer-side revocation procedure; see REVOCATION_DESIGN.md.
- M2.F2 Issuer pubkey binding in the proof path. The circuit proves the
  credential was signed by some BabyJubJub key. A future iteration will
  either CPI into issuer-registry::check_issuer_status from the verifier or
  add a compressed issuer tree as a Merkle membership input. This closes
  the last trust gap between Solana-level issuer approval and circuit-level
  issuer trust.
- Multi-party trusted setup. circuits/scripts/setup.js is a single-party
  ceremony suitable for localnet. A mainnet ceremony needs multiple
  contributors and a publicly verifiable attestation chain.
