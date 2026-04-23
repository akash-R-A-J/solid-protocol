# ADR 0006: Hardened nullifier (Phase 2 revision: 6 Poseidon inputs)

- **Status:** Superseded by this same ADR's Phase 2 revision (below).
  The v0.x five-input form stays documented as historical; the
  current production nullifier is six-input as of the ADR-0014
  circuit revision.
- **Date:** Phase 3.5 (5-input) landed pre-2026-04; formalized
  2026-04-22; revised 2026-04-24 for the ADR-0014 circuit rev.
- **Deciders:** founding team + auditors
- **Affects:** `crates/solid-core/src/nullifier.rs`,
  `circuits/batch_credential_query.circom`,
  `circuits/compound_query.circom`

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

### Phase 2 revision (current, ADR-0014; 6-input)

```
nullifier = Poseidon(
    masterIdentityKey,
    revocationNonce,       // holder's; rotates on bump
    verifierAddress,
    queryContextHash,
    verifierNonce,
    issuerTreeRoot         // NEW: binds proof to an issuer-tree epoch
)
```

- `issuerTreeRoot` is the same public input consumed by the
  verifier's ADR-0014 owner-checked `IssuerTreeBinding` parse.  Any
  issuer status transition (revoke / re-approve) bumps that issuer's
  leaf -> tree root changes -> the nullifier universe shifts.  A
  replay of a pre-revocation proof after root rotation fails BOTH
  the in-circuit Merkle-membership check AND (if somehow submitted)
  would not match a nullifier PDA anyone else is contending for.
- All five earlier preimage components stay (rationale below).

### Phase 3.5 baseline (historical; 5-input)

```
nullifier = Poseidon(
    masterIdentityKey,
    revocationNonce,
    verifierAddress,
    queryContextHash,
    verifierNonce
)
```

- `revocationNonce` rotates all nullifiers for the holder when bumped
  by the issuer.  Proofs against the prior identity state stop
  verifying because the global-tree `idLeaf` changes.
- `verifierAddress` = Solana program ID of the caller verifier,
  pinned to `public_inputs[VERIFIER_ADDRESS_INPUT_INDEX]` on-chain
  (index shifted to 29 by ADR-0014; see ADR-0012 revision in the
  same PR).
- `queryContextHash` = Poseidon hash of the predicate vector -- binds
  the nullifier to the specific query.
- `verifierNonce` = verifier-supplied per-session nonce.

See `crates/solid-core/src/nullifier.rs:23-40` for the authoritative
implementation (will be updated to 6-input in Phase 2 impl 3
alongside the holder SDK).  The module docstring is stale
(SOLID-SEC-036) and is updated in the same commit.

## Consequences

- **Positive.** Nullifier is unique per (holder, revocation epoch,
  verifier, query, session). Replays across verifiers, across queries,
  and across sessions are blocked cryptographically.
- **Negative (closed by Phase 2 revision).** The 5-input form did
  not include the global root or a monotonic tree epoch
  (SOLID-SEC-008).  Phase 2 closes this by adding `issuerTreeRoot`
  as the 6th input -- every issuer status transition rotates it,
  which is the same signal the holder-visible "revocation epoch"
  would carry.
- **Neutral.** Six-input Poseidon is a standard circomlib primitive
  (`Poseidon(6)`); one additional round over the 5-input form.

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
