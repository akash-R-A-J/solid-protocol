# Architecture Decision Records -- Index

Living index. One line per ADR. See individual files for detail.

Last updated: 2026-04-23

## Cryptography and proof system

- [ADR-0001](0001-use-groth16-over-alt-bn128-for-on-chain-verification.md)
  (Accepted) -- Use Groth16 over alt_bn128 for on-chain verification.
- [ADR-0002](0002-use-babyjubjub-and-poseidon-for-issuance-signing.md)
  (Accepted) -- Use BabyJubJub + Poseidon for issuance signatures and
  commitments.
- [ADR-0006](0006-hardened-nullifier-five-poseidon-inputs.md)
  (Accepted) -- Five-element Poseidon nullifier preimage.  **Superseded
  in spirit by ADR-0014's 6-input nullifier once the Phase 2 circuit
  rev lands; this ADR stays as the historical v0.x record.**
- [ADR-0012](0012-thirty-one-public-input-contract.md)
  (Accepted) -- 31 public-input contract between circuit and verifier.
  **To be revised to 32 by ADR-0014 in the Phase 2 circuit rev.**
- [ADR-0014](0014-compressed-issuer-tree-with-bjj-binding-leaf.md)
  (Accepted, pending implementation) -- Compressed issuer tree with
  BJJ-binding leaf; closes SOLID-SEC-004 and bundles SOLID-SEC-008
  epoch-bound nullifier into one Phase 2 circuit revision.

## Storage and state

- [ADR-0003](0003-spl-account-compression-for-credential-trees.md)
  (Accepted) -- SPL Account Compression for credential trees (migrated
  from Light Protocol in v0.2).
- [ADR-0007](0007-nullifier-per-pda-atomic-replay.md)
  (Accepted) -- Nullifier-per-PDA for atomic replay protection.
- [ADR-0010](0010-owner-check-on-tree-pdas.md)
  (Accepted) -- Owner-check all tree PDAs against schema-registry's
  program ID before trusting their roots.

## Program architecture

- [ADR-0004](0004-three-program-split.md)
  (Accepted) -- Three-program split: zk-verifier, issuer-registry,
  schema-registry.
- [ADR-0005](0005-per-schema-derived-credential-keys.md)
  (Accepted) -- Per-schema derived credential keys for unlinkability.
- [ADR-0008](0008-chunked-vk-upload.md)
  (Accepted) -- Chunked VK upload with sequential `next_vk_chunk`
  cursor.
- [ADR-0011](0011-batch-num-creds-four.md)
  (Accepted) -- Batch circuit fixed at NUM_CREDS=4, MAX_PREDICATES=4.

## Governance

- [ADR-0009](0009-dao-voting-plus-trust-anchor-bypass.md)
  (Accepted) -- Token-weighted DAO voting for issuer approval with
  a higher-tier trust-anchor fast-approve path.

## Process / meta

- [ADR-0013](0013-dedicated-directories-sec-adr-plan.md)
  (Accepted) -- Dedicated `sec/`, `adr/`, and `plan/` directories for
  security tracking, decision records, and implementation planning.

## Proposed / pending

- **Revision to ADR-0012** (31 -> 32 public inputs) attached to the
  Phase 2 circuit revision under ADR-0014.
- **Revision to ADR-0006** (5 -> 6 Poseidon nullifier inputs)
  attached to the same circuit revision.

Both revisions land together; either is a no-op without the other.
