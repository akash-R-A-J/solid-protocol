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

- `zk_verifier`: `DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb`
- `issuer_registry`: `5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx`
- `schema_registry`: `4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1`

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

## Addendum: 2026-04-25 ID rotation

The first localnet `anchor deploy` after the v0.6 / B6 fix landed
generated fresh per-program keypairs in `target/deploy/` (a path that
is gitignored), so the deployed program IDs diverged from the
hard-coded canonical values shown above. Rather than re-deploy under
the old keypairs (which were never committed and therefore not
recoverable), the canonical IDs were rotated in-place to match the
new keypairs and the keypair material itself was moved to a tracked
location to make the canonical set durable across machines:

- `zk_verifier`: was `BZkVFdMhAEeGMvEAhXNjt3r3bEA2sCPqEFcsEbSbFGj2`,
  now `DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb`.
- `issuer_registry`: was `CRGYfonXwDk6gKEm9fC1U33VVBkqnQVD3sPdLKzqHWoR`,
  now `5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx`.
- `schema_registry`: was `DPk6XUH6CArLWt4KMqJmpNBnPwQ3gG9P3dBd3MDVE3bT`,
  now `4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1`.

Three sources of truth changed in lock-step:

1. `Anchor.toml` `[programs.localnet]` and `[programs.devnet]`.
2. `declare_id!` in `programs/{zk-verifier,issuer-registry,schema-registry}/src/lib.rs`.
3. `deployments/devnet.json`, plus the byte-array constants
   `SCHEMA_REGISTRY_ID_BYTES` and `ISSUER_REGISTRY_ID_BYTES` in
   `crates/solid-light/src/cpi_helpers.rs` (the latter are exercised
   by the `id_bytes_tests` mod in the same file, which decodes the
   base58 literal at runtime and asserts byte equality).

The keypair files moved from `target/deploy/*-keypair.json` (untracked)
to `keys/localnet/*-keypair.json` (tracked). Future `anchor deploy`
runs reuse those keypairs so the canonical IDs stay stable. The
`scripts/check_program_ids.py` CI gate was unchanged; it continues
to enforce the three-way invariant between `Anchor.toml`,
`declare_id!`, and `deployments/devnet.json`.

This is a pure key rotation. No interface, account layout, ADR, or
security property changed.
