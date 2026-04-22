# ADR 0011: Batch circuit fixed at NUM_CREDS = 4, MAX_PREDICATES = 4

- **Status:** Accepted
- **Date:** v0.3; formalized 2026-04-22
- **Deciders:** founding team
- **Affects:** `circuits/batch_credential_query.circom`,
  `programs/zk-verifier/src/lib.rs` (NR_PUBLIC_INPUTS = 31)

## Context

The batch circuit proves compound predicates over a fixed number of
credentials. The parameters choose a point on the trade-off curve
between:

- **Expressivity.** Larger NUM_CREDS and MAX_PREDICATES support
  richer query grammars.
- **Proof time and size.** More credentials linearly inflate
  witness generation and R1CS constraints.
- **On-chain CU budget.** More public inputs means more alt_bn128
  scalar multiplications during the IC combination step.
- **Trusted-setup scope.** Every parameter change requires a new
  ceremony.

Benchmarking in Phase 3.1 showed NUM_CREDS=4 with MAX_PREDICATES=4
produces an ~90K R1CS constraint system, ~15-30s proof time on
desktop WASM, and fits in a single Solana tx at ~280-320K CU.

## Decision

Fix:

- `NUM_CREDS = 4`
- `MAX_PREDICATES = 4`
- `TREE_DEPTH = 20` (per-schema credential trees)
- `GLOBAL_DEPTH = 20` (global identity-state tree)
- `NR_PUBLIC_INPUTS = 31` (see ADR-0012 for the exact layout)

Zero-schema padding is supported: callers may set any slot's
`schemaHash` to 0 and the circuit skips it (see ADR-0005 note about
the `IdentityAnchor` gap tracked as SOLID-SEC-031).

## Consequences

- **Positive.** Deterministic circuit size. Predictable on-chain
  CU. Single tx per verify. Tooling (WASM, zkey, SDK) has one shape
  to support.
- **Negative.** A holder with > 4 credentials needed per query
  cannot use a single batch; they must submit multiple proofs and
  pay multiple verifies. Tree depth 20 caps the protocol at ~250K
  holders globally (SOLID-SEC-021). Any parameter change requires a
  new trusted setup.
- **Neutral.** Client-side query composition must sort and pad
  credentials before witness generation; the SDK already handles
  this via `sortedCredentials` + `indexMap`.

## Alternatives considered

- **Variable NUM_CREDS via circuit parameterization.** Rejected:
  Circom does not support truly variable-sized public inputs; any
  "variable" N means shipping multiple compiled circuits and
  matching VKs, multiplying the setup surface.
- **NUM_CREDS = 8 or 16.** Rejected at this iteration: proof time
  grows linearly and already sits uncomfortably close to the
  interactive-flow ceiling.
- **NUM_CREDS = 2.** Rejected: many real query patterns (age gate +
  residency + affiliation) need at least 3 credentials.

## References

- `circuits/batch_credential_query.circom:35-48`
- `circuits/lib/credential_atom.circom`
- Related: ADR-0001, ADR-0005, ADR-0012
- SOLID-SEC-001, SOLID-SEC-021, SOLID-SEC-029
