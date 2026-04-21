# Revocation Design

v0.3, April 2026. Design-complete, implementation in progress.

Status note (corrected 2026-04-21):

v1 (rotate-whole-identity) is fully supported by the circuit and by
schema-registry's update_global_root instruction. The on-chain scaffolding
is already in place. What remains is the holder-side workflow (detect
revocation, increment revocationNonce, re-derive identity leaf, request
re-insertion into the global tree) and the issuer-side revocation
procedure (publish revocation event, bump the identity-tree root).
Previous versions of this document described v1 as live; that was
aspirational. This revision restates the status accurately.

v1.1 (SMT per-credential revocation) is scheduled for the release after
v1 ships. This document is the canonical reference for both models, the
trade-offs between them, and the migration plan.

---

## 1. Threat model

Revocation must answer exactly one question every time a holder submits a
proof:

> _"Is credential `C` (or the identity bundle `(PK_master, revocationNonce)`
> that issued the nullifier) currently valid, as of the verifier's
> trusted-root horizon?"_

We require:

- **Soundness** — a revoked credential/identity MUST NOT pass verification.
- **Privacy** — the verifier MUST NOT learn *which* credential was revoked,
  only that the holder's membership proof is against a root the verifier
  accepts.
- **Liveness** — revocation propagates to verifiers in O(minutes), not
  O(days).  The holder MUST NOT need to re-prove interactively with the
  issuer every time.

---

## 2. v1 (current) — Identity-state rotation

### Mechanism

Every holder maintains an **identity bundle**:

```
identity_leaf = Poseidon( PK_master_x , PK_master_y , revocationNonce )
```

This leaf is inserted into the **global identity-state tree** (bound by
`schema_registry::global_binding`).  The circuit proves:

1. `identity_leaf` is a member of the global-state tree at root `G`.
2. Each credential leaf `C_i` is a member of its schema-bound tree at
   root `R_i`.
3. The nullifier is `Poseidon( sk_master , revocationNonce , verifier_id ,
   queryContextHash , verifier_nonce )`.

Because `revocationNonce` is **inside** both the identity leaf and the
nullifier, the only way for a revoked credential to pass is if a holder
can produce a membership proof for a stale identity leaf against the
current global-state root — which is infeasible once the issuer/DAO
rotates the identity leaf out of the tree and bumps the revocation nonce.

### Revocation procedure (v1)

1. Issuer (or DAO policy) instructs `schema_registry::update_global_root`
   to replace the old global-state root with one that excludes the revoked
   identity leaf.
2. Holder regenerates their `identity_leaf` with `revocationNonce' =
   revocationNonce + 1`.
3. Holder requests re-insertion into the global-state tree.
4. All future proofs use `revocationNonce'`; old nullifiers are worthless.

### What v1 buys us

- **Simple to audit** — one membership proof and one nonce, no
  non-membership circuits, no SMT gymnastics.
- **Uses only SPL Account Compression primitives** — no custom sparse
  tree op needed on day 1.
- **Hard failure mode for the attacker** — without the new
  `revocationNonce` the holder cannot produce a valid `identity_leaf`
  against the current `G`.

### v1 cost / pain points

- **Every** revocation is O(tree_depth) writes against the global-state
  tree (remove-and-reinsert), even when only one credential of many was
  revoked.
- Holders learn about revocation only by failing a proof; they must then
  regenerate their bundle out-of-band.
- Does not support **per-credential** revocation semantics — the model
  revokes the entire holder identity, not just one credential.
- Every `revocationNonce` bump forces the holder to re-derive every
  nullifier they care about, which may include unrelated credentials.

These costs are acceptable for v1 devnet / pilot, but they are not the
long-term answer.  The circuits already carry all the signals needed for
v1.1 (we anchor the identity in the global tree today precisely so we
have the hook for non-membership tomorrow).

---

## 3. v1.1 (planned) — Sparse Merkle Tree non-membership

### Target mechanism

Introduce a **revocation Sparse Merkle Tree (SMT)** keyed by a stable
per-credential `revocation_id`:

```
revocation_id = Poseidon( schema_hash , credential_id , issuer_pubkey )
```

Each SMT leaf stores a single bit (`revoked = 1`, `active = 0`).  The
root `Rev_root` is anchored in the `schema_registry::global_binding` PDA
alongside `G`.

The circuit adds **one** extra check:

```
nonMembership( revocation_id , Rev_root ) == active
```

`nonMembership` is the standard SMT non-inclusion proof: the holder
provides the sibling path and the circuit verifies that the slot at
`revocation_id` resolves to the default ("active") leaf.

### Why SMT, not SPL AC

SPL Account Compression's concurrent Merkle tree is optimized for
**append-only** insertion, not for efficient non-membership proofs.  The
revocation use case is the opposite: writes are rare (issuer policy
decides), reads are every single proof.  A sparse tree with a
deterministic default leaf is the right data structure.  We accept the
higher per-write cost (O(tree_depth) rather than O(1)) because writes
are ~1000× less frequent than reads.

### Revocation procedure (v1.1)

1. Issuer calls a new `schema_registry::revoke_credential(revocation_id)`
   instruction.
2. Program flips the leaf at slot `revocation_id` from `0` → `1`, updates
   the SMT root, and stores it in a new `RevocationBinding` PDA.
3. Holder's next proof includes an SMT non-membership witness.  The
   circuit proves the leaf is still `active`; if the issuer has revoked
   it, the non-membership proof cannot be constructed.
4. No global-state root rotation needed; the holder's `identity_leaf`
   and `revocationNonce` remain untouched.

### Benefits

- **Per-credential granularity** — revoking one credential does not
  invalidate the rest of the holder's bundle.
- **Cheaper for holders** — no forced nonce-bump cascade across
  unrelated credentials.
- **Cheaper for verifiers** — a single extra constraint in the circuit
  (the non-membership check) instead of two full tree rotations.
- **Privacy preserved** — the verifier still learns nothing about which
  credential, only that the proof passes against the accepted
  `Rev_root`.

### Circuit cost estimate

SMT depth = 32 (the same 2³² capacity SPL AC trees use).  Non-membership
proof ≈ 32 Poseidon(2) hashes + 32 inclusion checks.  Against a current
circuit of ≈ 220 K constraints, the delta is ≈ 3 K — under 1.5 %
overhead.  Proving-time impact is comparable.

---

## 4. Migration plan (v1 → v1.1)

Phased, **backwards-compatible**:

1. **v1.1.0 — Additive deploy.**
   - Add `RevocationBinding` PDA to `schema-registry`.
   - Add `revoke_credential` / `unrevoke_credential` instructions
     (authority-gated, identical trust model to `update_global_root`).
   - Keep v1 rotate-identity path alive; verifier accepts **either** an
     old-style or a new-style circuit (feature-gated by circuit version
     in `VerifierConfig`).
2. **v1.1.1 — Circuit opt-in.**
   - Ship the updated circuit with the non-membership check.
   - Holders who opt in start producing the extra witness; verifiers
     that upgraded `VerifierConfig.circuit_id` require it.
3. **v1.2.0 — Deprecate rotation path.**
   - After a 90-day migration window, mark the v1 rotate-identity flow
     deprecated; the DAO can vote to retire the old circuit version.

No re-issuance is required.  Existing credentials and identity leaves
keep working; only the *revocation signal* changes.

---

## 5. Out-of-scope (intentional)

- **Revocation transparency** (proving a credential is *or is not* in a
  published revocation list) — this is a separate layer on top of the
  SMT.  Day-1 v1.1 does not ship it; the SMT is the substrate that makes
  it possible.
- **Time-based revocation / expiry** — already handled by the circuit's
  `expirationTimestamp` check; the SMT is for **explicit** issuer
  revocation only.
- **Batch revocation** (revoke N credentials in one tx) — will be added
  in v1.2 once SMT write primitives are production-proven.

---

## 6. References

- Circuit: `circuits/batch_credential_query.circom` (identity-cohesion
  Step 3, global-root binding Step 7).
- `schema-registry::initialize_global_binding` / `update_global_root` —
  the existing global-root anchoring used as the trust anchor for both
  v1 and v1.1.
- `@solid-protocol/light` — the SPL AC adapter that will also expose
  `getCurrentRevocationRoot` in v1.1.
