# ADR 0009: Token-weighted DAO voting plus higher-tier trust-anchor fast-approve

- **Status:** Accepted (slashing/freeze controls are single-key today;
  Phase 3 adds multisig + timelock per SOLID-SEC-013)
- **Date:** v0.3 (formalized 2026-04-22)
- **Deciders:** founding team
- **Affects:** `programs/issuer-registry/src/lib.rs`

## Context

Issuer approval is the governance bottleneck for every credential
issued on the protocol. The approval mechanism has two competing
requirements:

1. **Decentralization.** Approval must not be gated by a single
   operator; the community of stakers should judge applicants.
2. **Regulatory fast-path.** Government / regulated-entity issuers
   (e.g., national ID authorities, licensed KYC providers) need a
   deterministic approval path that does not wait on a public vote.

## Decision

Two approval modes, neither of which is mutually exclusive:

1. **DAO voting.** Stakers vote on an `IssuerAccount` in `Pending`
   status. A voting deadline and token-weighted tally produce the
   `Approved` transition. Flash-loan protection: stake must be at
   least 100 slots old before voting
   (`programs/issuer-registry/src/lib.rs:158-161`). Active-votes
   counter tracks unlocked stake.
2. **Trust-anchor bypass.** A higher-tier approved issuer
   (`Government` or `Regulated`) can approve a `Pending` target
   directly via `approve_via_trust_anchor`. The tier order
   `Community < Enterprise < Regulated < Government` encodes trust
   precedence.

Issuer lifecycle: `Pending -> Approved -> Cooldown -> Revoked`,
with `Slashed` as an out-of-band penalty state.

## Consequences

- **Positive.** Community governance for normal onboarding.
  Regulatory path for entities with prior off-chain legitimacy.
  Flash-loan protection closes a trivial attack.
- **Negative.** Slashing and fraud-proof authority is single-key
  (SOLID-SEC-013). A compromised `registry_config.authority` key can
  globally slash. Phase 3 adds Squads multisig and a challenge window.
- **Negative.** `approve_via_trust_anchor` has no minimum-tier gate
  on targets (SOLID-SEC-015); a Government anchor can approve a
  Community target with no staking or vote. Phase 1 adds the tier-
  strictly-higher constraint.
- **Neutral.** `CheckIssuerStatus` is a read-only lookup for off-
  chain use; on-chain CPI is not wired up today (see SOLID-SEC-004
  and SOLID-SEC-025).

## Alternatives considered

- **Single-authority approval (no DAO).** Rejected: conflicts with
  the decentralization goal.
- **DAO-only (no trust anchor).** Rejected: blocks deterministic
  onboarding for regulated entities.
- **Multisig-only.** Rejected: concentrates approval power in a
  small committee regardless of broader community preference.

## References

- `programs/issuer-registry/src/lib.rs:59-77` (initialize_registry)
- `programs/issuer-registry/src/lib.rs:152-220` (vote_on_issuer)
- `programs/issuer-registry/src/lib.rs:533-574` (approve_via_trust_anchor)
- `programs/issuer-registry/src/lib.rs:409-460` (slash_issuer)
- Related: ADR-0004
- SOLID-SEC-013, SOLID-SEC-015, SOLID-SEC-030, SOLID-SEC-034,
  SOLID-SEC-035
