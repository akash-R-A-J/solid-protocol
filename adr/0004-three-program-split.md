# ADR 0004: Three-program split (zk-verifier, issuer-registry, schema-registry)

- **Status:** Accepted
- **Date:** (pre-v0.1, formalized 2026-04-22)
- **Deciders:** founding team
- **Affects:** `programs/zk-verifier`, `programs/issuer-registry`,
  `programs/schema-registry`, `Anchor.toml`, `deployments/`

## Context

The protocol has three distinct concerns with different upgrade
cadences, different authority models, and different storage shapes:

1. **Verification** (Groth16 verify, VK storage, nullifier PDA).
2. **Issuance governance** (DAO voting, staking, slashing, trust
   anchors, issuer lifecycle).
3. **Schema lifecycle** (registering schemas, binding trees,
   mirroring tree roots, authority transfers).

Packing all three into one program means any upgrade to one concern
(say, moving the VK into a different layout) requires a buildable
unified deploy. Separating them lets the DAO upgrade issuer policy
without touching the verifier, and lets the verifier add a new
circuit revision without touching issuer state.

## Decision

Three independent Anchor programs with distinct declared IDs:

- `zk_verifier`: `BZkVFdMhAEeGMvEAhXNjt3r3bEA2sCPqEFcsEbSbFGj2`
- `issuer_registry`: `CRGYfonXwDk6gKEm9fC1U33VVBkqnQVD3sPdLKzqHWoR`
- `schema_registry`: `DPk6XUH6CArLWt4KMqJmpNBnPwQ3gG9P3dBd3MDVE3bT`

Program IDs are load-bearing invariants. `Anchor.toml`,
`declare_id!` in each program, and `deployments/*.json` must agree.
CI enforces via `scripts/check_program_ids.py`.

The on-chain trust model between the three:

- `zk-verifier` **reads** tree roots from `schema-registry` PDAs via
  byte-level state parsing (`crates/solid-light::cpi_helpers`). No
  CPI, owner-check only (see ADR-0010).
- `issuer-registry` **writes** credential leaves via SPL AC CPI using
  a PDA authority seeded from `schema_hash`. Today this trust is
  under-bound (see SOLID-SEC-003); Phase 1 fix adds schema-registry
  account checks.
- `schema-registry` is **read-only** from the verifier's perspective
  and **mutated** only by schema authorities.

## Consequences

- **Positive.** Independent upgrade authority per concern. Blast
  radius of a bug is contained per program. Easier to reason about
  security boundaries.
- **Negative.** More accounts per transaction (verifier reads five
  tree accounts + two config accounts). Program-ID discipline is
  non-negotiable -- a mismatched ID silently breaks owner-checks
  (see SOLID-SEC-029).
- **Neutral.** Forces deliberate interface design between programs.

## Alternatives considered

- **Monorepo single program.** Rejected: couples three concerns with
  very different upgrade cadences; every change touches every
  concern's account layout.
- **Four programs (separating VK storage from verify).** Rejected:
  VK is verifier-specific and adds no value to split further; one
  more account in the tx for no security gain.

## References

- `Anchor.toml:1-15`
- `deployments/devnet.json`
- `scripts/check_program_ids.py`
- `CLAUDE.md:14-23` (canonical program IDs invariant)
- Related: ADR-0010
- SOLID-SEC-003, SOLID-SEC-029
