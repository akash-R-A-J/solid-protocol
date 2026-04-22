# ADR 0006: Hardened nullifier with five Poseidon inputs

- **Status:** Accepted (hardened from v0.1 three-input form in
  Phase 3.5 remediation)
- **Date:** Phase 3.5 landed pre-2026-04; formalized 2026-04-22
- **Deciders:** founding team + auditors
- **Affects:** `crates/solid-core/src/nullifier.rs`,
  `circuits/batch_credential_query.circom`

## Context

The nullifier must (a) detect double-use of the same credential
for the same verifier + query, (b) be unlinkable across verifiers,
(c) be unlinkable across queries for the same verifier, (d) bind to
the verifier program's address to prevent cross-program replay, and
(e) rotate on revocation-nonce change so the holder can invalidate
past proofs.

The v0.1 three-input nullifier `Poseidon(holderPrivKey, schemaHash,
verifierNonce)` satisfied (a) and (b) but failed (c) -- two distinct
queries against the same verifier produced different nullifiers
only if `verifierNonce` differed, which the verifier can force but
the query context cannot.

## Decision

```
nullifier = Poseidon(
    masterIdentityKey,    // rotates on revocation-nonce change? no -- see revocationNonce
    revocationNonce,
    verifierAddress,
    queryContextHash,
    verifierNonce
)
```

- `revocationNonce` rotates all nullifiers for the holder when bumped
  by the issuer. Proofs against the prior identity state stop verifying
  because the global-tree `idLeaf` changes.
- `verifierAddress` = Solana program ID of the caller verifier, pinned
  to `public_inputs[28]` on-chain (see ADR-0012).
- `queryContextHash` = Poseidon hash of the predicate vector -- binds
  the nullifier to the specific query.
- `verifierNonce` = verifier-supplied per-session nonce.

See `crates/solid-core/src/nullifier.rs:23-40` for the authoritative
implementation. The module docstring is stale (SOLID-SEC-036) and
must be updated in Phase 2.

## Consequences

- **Positive.** Nullifier is unique per (holder, revocation epoch,
  verifier, query, session). Replays across verifiers, across queries,
  and across sessions are blocked cryptographically.
- **Negative.** Nullifier does not include the global root or a
  monotonic tree epoch (see SOLID-SEC-008). If the root regresses
  (reorg, bug), the same nullifier could apply across tree
  generations. Phase 2 adds an `epoch_counter` to the preimage.
- **Neutral.** Five-input Poseidon is a standard circomlib primitive
  (`Poseidon(5)`); constraint cost is roughly one additional round
  over the three-input form.

## Alternatives considered

- **Three-input nullifier (v0.1).** Rejected: failed query-level
  unlinkability and cross-verifier defense.
- **Include the global root directly in the preimage.** Rejected:
  breaks nullifiers on every legitimate root update. Phase 2 instead
  introduces a monotonic epoch counter that only bumps on holder-
  driven revocation.

## References

- `crates/solid-core/src/nullifier.rs:23-40`
- `circuits/batch_credential_query.circom:286-292`
- Related: ADR-0005, ADR-0012
- SOLID-SEC-008, SOLID-SEC-036
