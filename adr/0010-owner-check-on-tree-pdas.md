# ADR 0010: Owner-check every tree PDA against schema-registry's program ID

- **Status:** Accepted (load-bearing invariant)
- **Date:** Phase 3 security remediation (P0-2); formalized 2026-04-22
- **Deciders:** founding team + auditors
- **Affects:** `programs/zk-verifier/src/lib.rs`,
  `crates/solid-light/src/cpi_helpers.rs`

## Context

The `zk-verifier` program parses tree roots out of
`GlobalStateBinding` and `SchemaTreeBinding` accounts and compares
them against the Groth16 proof's public-input root.

Without an owner-check, an attacker can pass a **system-owned
account** whose bytes are attacker-chosen. If the bytes parse as a
valid binding with the chosen discriminator, the verifier will
accept any (forged) root -- every ZK gate collapses.

This is the exact attack surface the v0.2 audit labelled P0-2. The
fix is the single most security-critical check in the verifier.

## Decision

Before trusting any byte of a tree PDA, `zk-verifier` requires:

```rust
require_keys_eq!(
    *ctx.accounts.global_tree.owner,
    SCHEMA_REGISTRY_ID,
    ErrorCode::InvalidGlobalRoot
);
require_keys_eq!(
    *ctx.accounts.schema_tree_N.owner,
    SCHEMA_REGISTRY_ID,
    ErrorCode::InvalidSchemaRoot
);
```

Applied to every `schema_tree_0..3` and the `global_tree` in
`verify_batch_proof` (`programs/zk-verifier/src/lib.rs:188-192,
238-242`). The `SCHEMA_REGISTRY_ID` constant is the canonical program
ID declared in ADR-0004.

## Consequences

- **Positive.** The forged-trust-root attack is closed. Zero CU
  cost beyond the pubkey comparison.
- **Negative.** Load-bearing on a hardcoded 32-byte constant in
  `crates/solid-light/src/cpi_helpers.rs:54-59`. If that constant
  ever diverges from the actual schema-registry program ID, the
  owner-check silently reopens the vulnerability -- this is
  SOLID-SEC-032. Phase 1 adds a build-time assertion.
- **Neutral.** The owner-check is a sibling to the discriminator
  check (`"globroot"` / `"schmtree"`); both must agree for a binding
  to be trusted.

## Alternatives considered

- **CPI into schema-registry for every read.** Rejected: triples the
  CU cost of `verify_batch_proof`. The direct-parse-plus-owner-check
  pattern is the Solana-native idiom for cross-program reads.
- **Sign the binding with schema-registry's authority and verify the
  signature.** Rejected: enormous CU cost on every verify; owner-
  check gives the same guarantee essentially for free.

## References

- `programs/zk-verifier/src/lib.rs:188-192,238-242`
- `crates/solid-light/src/cpi_helpers.rs:54-59` (load-bearing constant)
- `CLAUDE.md:30-34` (hard invariant)
- Related: ADR-0003, ADR-0004
- SOLID-SEC-032
