# ADR 0007: Nullifier-per-PDA for atomic replay protection

- **Status:** Accepted
- **Date:** v0.3 (formalized 2026-04-22)
- **Deciders:** founding team
- **Affects:** `programs/zk-verifier/src/lib.rs`

## Context

Once a proof is accepted on-chain, its nullifier must never be
acceptable again in the same scope. The replay-protection mechanism
must be (a) atomic (no race between check and insert), (b) cheap to
verify, and (c) persistent across restarts / reorgs post-finality.

v0.2 briefly considered a Bloom-filter backed by a single compressed
account. Bloom filters fail (a) because a false-positive retry
breaks determinism, and fail atomicity at the runtime level (multi-
instruction check-then-insert has TOCTOU).

## Decision

Allocate one PDA per accepted nullifier, seeded
`["null", nullifier_bytes]` under `zk-verifier`. Use Anchor's
`#[account(init, ...)]` on the nullifier account:

```rust
#[account(
    init,
    payer = payer,
    space = 8 + NullifierRecord::SPACE,
    seeds = [b"null", nullifier.as_ref()],
    bump,
)]
pub nullifier_record: Account<'info, NullifierRecord>,
```

If the PDA exists, `init` fails atomically -- the Solana runtime
guarantees no other instruction in the same tx can observe a
half-written state.

## Consequences

- **Positive.** O(1) replay check. Atomic by construction. Easy to
  reason about. No race window.
- **Negative.** Linear state growth: one PDA per accepted proof.
  `NullifierRecord::SPACE + 8 = 56 bytes`; rent-exempt minimum
  ~0.00093 SOL per PDA. At 1M proofs/month this is ~$140K/month
  recurring rent (see SOLID-SEC-018's amortized cost discussion and
  `plan/IMPLEMENTATION_PLAN.md` Phase 3 scale-escape item).
- **Neutral.** A future compressed nullifier tree (Phase 3) becomes a
  second option layered on top; the primary path stays the PDA
  pattern for short-lived proofs.

## Alternatives considered

- **Bloom filter.** Rejected: non-determinism, no atomicity, false-
  positive retry path.
- **Single large bitmap account.** Rejected: 1M bits ~ 128KB, still
  one-write-per-proof contention, no clean sharding.
- **Compressed nullifier Merkle tree.** Deferred to Phase 3. Needs
  off-chain proof of non-inclusion + Merkle-membership after
  insertion, increasing client work and adding another trusted-setup
  dependency if the membership proof is in-circuit.

## References

- `programs/zk-verifier/src/lib.rs:485-517` (handler)
- `programs/zk-verifier/src/lib.rs:548-565` (NullifierRecord struct)
- Related: ADR-0006
- SOLID-SEC-018 (scale ceiling), `plan/IMPLEMENTATION_PLAN.md` (Phase 3)
