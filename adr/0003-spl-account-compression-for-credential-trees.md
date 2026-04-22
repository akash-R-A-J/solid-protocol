# ADR 0003: SPL Account Compression for credential trees (migrated from Light Protocol in v0.2)

- **Status:** Accepted (supersedes the prior Light Protocol CPI
  approach shipped in v0.1)
- **Date:** migration landed in v0.2; formalized 2026-04-22
- **Deciders:** founding team
- **Affects:** `programs/schema-registry`, `programs/issuer-registry`
  (issue_credential path), `crates/solid-light`

## Context

Credential trees must (a) scale to 10^6+ leaves per schema, (b) be
readable by any RPC without a specialized indexer, (c) commit a
root on-chain that the ZK verifier can check byte-for-byte against
the proof's public-input root, and (d) not require a trust
relationship with a third-party service.

v0.1 used Light Protocol's compressed-state primitives via CPI.
Operational review identified three hard failure modes:

1. A dedicated indexer (Photon) was required to even read the tree
   root, adding an external trust dependency.
2. Light Protocol's own pause / upgrade authority became a transitive
   upgrade authority on SolID availability.
3. Every read-path in the SDK had to handle indexer unavailability
   as a first-class error.

## Decision

Use SPL Account Compression as the tree backend. Each schema owns a
`ConcurrentMerkleTree` account under the `schema-registry` program.
The `schema-registry` program maintains a `SchemaTreeBinding` PDA
that mirrors the tree root for the on-chain verifier. The
`issuer-registry::issue_credential` handler appends leaves via a
direct CPI to the SPL AC program, signing with a
`PDA(b"tree-authority", schema_hash)` authority.

The crate name `solid-light` is retained for compatibility even
though the implementation no longer uses Light Protocol (see
SOLID-SEC-009 for the rename action in Phase 1).

## Consequences

- **Positive.** No external indexer dependency. Any RPC with
  `getAccountInfo` can verify a proof. Fewer trust assumptions. The
  byte-level state layout of `ConcurrentMerkleTreeAccount` is public.
- **Negative.** Tree depth is compile-time-fixed in the circuit VK;
  changing it requires a new trusted setup (see SOLID-SEC-021 for the
  ~250K-holder cap at depth 20). SPL AC's `append` discriminator is
  hard-coded in the issuer-registry CPI path; see the "NOT verified"
  list in the 2026-04-22 audits.
- **Neutral.** Docs referring to "Light Protocol" or "Photon indexer"
  must be purged; partial work tracked as SOLID-SEC-027.

## Alternatives considered

- **Remain on Light Protocol.** Rejected for the three failure modes
  above. A future re-evaluation is possible once Light Protocol's
  governance and performance story matures.
- **Custom compressed tree on SolID's own account format.** Rejected:
  reinvents SPL AC with no clear advantage, adds audit surface.

## References

- `crates/solid-light/src/cpi_helpers.rs` (root-reader)
- `crates/solid-light/src/credential_tree.rs`
- `programs/schema-registry/src/lib.rs` (tree binding + authority)
- `programs/issuer-registry/src/lib.rs:619-716` (append CPI)
- `docs/light-protocol.md`
- Related: ADR-0010
- SOLID-SEC-009, SOLID-SEC-021, SOLID-SEC-029
