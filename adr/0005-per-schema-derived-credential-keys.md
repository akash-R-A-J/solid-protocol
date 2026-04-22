# ADR 0005: Per-schema derived credential keys

- **Status:** Accepted
- **Date:** remediation landed 2026-04 ("BUG-04 fix"), formalized
  2026-04-22
- **Deciders:** founding team + auditors
- **Affects:** `circuits/lib/identity_anchor.circom`,
  `crates/solid-core/src/identity.rs`,
  `ts-sdk/packages/holder/src/index.ts`

## Context

The v0.1 identity model anchored every credential to the holder's
single master BabyJubJub pubkey. Every credential in the global
tree was therefore linked to the master key, and an indexer could
group credentials by holder by reading the global tree -- an
unlinkability failure.

Semaphore, Polygon ID, and similar zk-identity stacks solve this
by deriving a per-context key from the master secret. The protocol
follows the same pattern.

## Decision

For each credential under `schemaHash`:

```
credPriv   = Poseidon(masterIdentityKey, schemaHash)
(Ax, Ay)   = BabyPbk(credPriv)
idLeaf     = Poseidon(Ax, Ay, revocationNonce)
```

The global-state tree stores `idLeaf` per (holder, schema) pair, not
per holder. The in-circuit `IdentityAnchor` derives `(Ax, Ay)`
inside the circuit and proves `idLeaf` is a member of the global
tree.

## Consequences

- **Positive.** Cross-schema linkability is broken. An indexer
  walking the global tree sees only per-schema derived keys; two
  credentials for the same holder across two schemas look like two
  unrelated identities.
- **Negative.** Every SDK and holder-side helper must now derive the
  per-schema key before comparing to credential fields. The v0.3
  cohesion check at `ts-sdk/packages/holder/src/index.ts:250-255`
  still compares against the master key -- this is SOLID-SEC-033, an
  E2E blocker that Phase 1 must fix.
- **Neutral.** The global tree grows by `O(holders * schemas)`
  rather than `O(holders)`. At depth 20 this pushes the practical
  protocol cap down to ~250K holders (see SOLID-SEC-021).

## Alternatives considered

- **Single master-key anchor (v0.1 model).** Rejected: trivial
  cross-schema linkability.
- **Per-credential nonce instead of per-schema derivation.**
  Rejected: either leaks the nonce publicly (breaks unlinkability)
  or requires a separate private-nonce-tree (adds another tree and
  trusted-setup change).

## References

- `circuits/lib/identity_anchor.circom:24-43`
- `crates/solid-core/src/identity.rs`
- `ts-sdk/packages/holder/src/index.ts:281-285`
- Related: ADR-0002, ADR-0003
- SOLID-SEC-021, SOLID-SEC-029, SOLID-SEC-033
