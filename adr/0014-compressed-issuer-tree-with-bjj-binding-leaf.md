# ADR 0014: Compressed issuer tree with BJJ-binding leaf

- **Status:** Accepted (pending implementation; drives Phase 2 circuit rev)
- **Date:** 2026-04-23 (Phase 2 kickoff)
- **Deciders:** founding team; Phase 2 entry review
- **Supersedes:** ADR-0012 (31-public-input contract) will be
  revised to 32 when the issuer-tree root becomes a public input.
- **Affects:** `circuits/batch_credential_query.circom`,
  `circuits/compound_query.circom`,
  `circuits/lib/identity_anchor.circom` (sibling pattern),
  `programs/zk-verifier/src/lib.rs`,
  `programs/issuer-registry/src/lib.rs`,
  `crates/solid-light/src/cpi_helpers.rs`,
  `ts-sdk/packages/issuer/src/index.ts`,
  `ts-sdk/packages/holder/src/index.ts`.

## Context

SOLID-SEC-004 (HIGH, open at Phase 1 close).  The batch circuit
(`circuits/batch_credential_query.circom:81-82`) accepts
`issuerPubKeyAxs[NUM_CREDS]` and `issuerPubKeyAys[NUM_CREDS]` as
*private* inputs.  Nothing cross-checks those keys against the
on-chain `IssuerAccount`.  `check_issuer_status`
(`programs/issuer-registry/src/lib.rs:584`) exists but is never
CPI'd.  Two concrete consequences:

1. **Unapproved-issuer smuggling.**  A malicious SDK can attest with
   a BJJ key whose corresponding Solana authority was never DAO-
   approved.  The on-chain verifier is satisfied because it never
   learned which authority the circuit was talking about.
2. **Post-revocation replay.**  An issuer revoked via
   `revoke_issuer` keeps functioning from the circuit's perspective
   because revocation never touches anything the circuit sees.  Old
   signatures verify indefinitely.

This is the largest remaining soundness gap and the one external
auditors are most likely to flag as critical.  Fix design is now
unblocked; ADR captures the chosen path.

## Decision

**Introduce a compressed issuer tree.**  The leaf binds the BJJ
signing key into the on-chain issuer state, and the tree's root is
a public input to the batch circuit.  Membership is proved in-circuit
per credential slot.

### Leaf composition (load-bearing)

Each issuer's leaf is:

```
issuer_leaf = Poseidon5(
    authority,         // 32-byte Solana pubkey (as BN254 field element)
    bjj_x,             // 32-byte BJJ public key x-coord
    bjj_y,             // 32-byte BJJ public key y-coord
    status_epoch,      // u64: DAO finalisation slot at time of Approval
    revocation_nonce,  // u64: increments on revoke_issuer
)
```

Every field is load-bearing:

- **`bjj_x, bjj_y`** are what ties the authority to the signing key
  the circuit verifies.  Without them the attack only shifts from
  "any BJJ key" to "any approved authority + any BJJ key" -- the
  issuer tree would prove Solana-side approval without proving
  which BJJ key was authorised.
- **`authority`** ties the leaf to the Solana keypair that can
  mutate `IssuerAccount`.
- **`status_epoch`** makes the leaf change whenever the issuer's
  status transitions (Pending -> Approved, Approved -> Cooldown,
  etc.).  Only leaves whose on-chain `IssuerAccount.status ==
  Approved` are included in the tree; any transition drops / adds
  the leaf with a new `status_epoch`.
- **`revocation_nonce`** bumps on `revoke_issuer`.  Combined with
  SOLID-SEC-008's epoch-bound nullifier (below), this invalidates
  every prior proof from the revoked issuer.

### Tree depth

`ISSUER_TREE_DEPTH = 16`.  Supports 65 536 simultaneously-approved
issuers -- two orders of magnitude above any realistic v1 issuer
base, while keeping the per-credential sibling count (NUM_CREDS = 4
credentials x 16 siblings = 64 new private inputs) tractable.
Depth 20 would double the sibling load for no near-term capacity
benefit.  If the bound tightens later, add a `ISSUER_TREE_DEPTH_V2`
template parameter and migrate via a new circuit rev.

### Tree backend

Same SPL Account Compression substrate as the credential +
identity trees.  The tree's authority is a PDA of
`issuer-registry` so only that program can append / remove / update
leaves -- cleanest Solana-native ownership pattern and reuses the
existing `initialize_tree_binding` path.  A new binding PDA
`IssuerTreeBinding` analogous to `SchemaTreeBinding`, seeded at
`[b"issuer-tree-binding"]`, carries the current root.

### Integration with SOLID-SEC-008 (epoch-bound nullifier)

Bundle: one circuit rev + one trusted setup for SEC-004 **and**
SEC-008 jointly.  The issuer-tree root serves as the epoch
identifier SEC-008 needs.  Nullifier becomes a 6-input Poseidon:

```
nullifier = Poseidon6(
    masterKey,
    revocation_nonce,          // holder's, not issuer's
    verifierAddress,
    queryContextHash,
    verifierNonce,
    issuer_tree_root,          // NEW: ties the proof to an issuer epoch
)
```

Revoking an issuer bumps their leaf's `revocation_nonce`, which
changes the tree root, which changes the nullifier universe.  Every
pre-revocation proof's nullifier is now mint-new against the new
root, but the membership proof fails against the new root.  Clean
invalidation with no separate "epoch counter" bookkeeping.

### Public-input contract update

Current (ADR-0012): 31 public inputs.  New: **32 public inputs**,
with `issuerTreeRoot` inserted after `schemaHashes[NUM_CREDS]` and
before `queryCredentialIndices`:

```
[0]      nullifierHash
[1]      globalRoot
[2..5]   merkleRoots[NUM_CREDS]
[6..9]   schemaHashes[NUM_CREDS]
[10]     issuerTreeRoot                         // NEW
[11..14] queryCredentialIndices[MAX_PREDICATES]
[15..18] queryFieldIndices[MAX_PREDICATES]
[19..22] queryOperators[MAX_PREDICATES]
[23..26] queryValues[MAX_PREDICATES]
[27]     numPredicates
[28]     compoundLogic
[29]     verifierAddress
[30]     verifierNonce
[31]     currentTimestamp
```

ADR-0012 is revised in the same PR that lands the circuit.  The
hard-coded `NR_PUBLIC_INPUTS = 31` in
`programs/zk-verifier/src/lib.rs` bumps to 32.

### On-chain verifier wiring

`verify_batch_proof` gains a new required account:

```rust
#[account(
    seeds = [b"issuer-tree-binding"],
    bump,
    seeds::program = ISSUER_REGISTRY_ID,
)]
pub issuer_tree_binding: UncheckedAccount<'info>,
```

The handler owner-checks it against `ISSUER_REGISTRY_ID` (a new
constant in `crates/solid-light/src/cpi_helpers.rs` mirroring the
`SCHEMA_REGISTRY_ID` pattern; same `SOLID-SEC-032`-style two-layer
guard) and parses out the `current_root`, asserting it equals
`publicInputs[10]`.

## Consequences

### Positive

- Revocation is propagated into every subsequent proof via the
  root change; no CPI per verify, no stateful nullifier cache.
- Unapproved-issuer smuggling is eliminated: the prover must supply
  a Merkle path to an Approved leaf whose embedded `(bjj_x, bjj_y)`
  matches the signing key the circuit verifies.
- Bundles cleanly with SOLID-SEC-008.  One trusted setup covers
  both CRITICAL/HIGH soundness items.
- Matches the existing SPL-AC infrastructure; no new substrate.

### Negative

- **Circuit cost.**  4 credentials x 16 siblings = 64 new private
  inputs and 4 new Merkle inclusion subcircuits.  Rough constraint
  count bump: ~8-10% on the batch circuit.  Fits in the already-
  chosen `PTAU_POWER = 17` pot (128k constraints).
- **Trusted setup rerun.**  TESTNET single-party, labelled as such,
  same drill as Phase 1.  Multi-party for mainnet is SOLID-SEC-012.
- **Operator complexity.**  Every `register_issuer` / status
  transition / `revoke_issuer` must also touch the issuer tree
  (append / update leaf).  Indexer event contract needed so an
  off-chain operator can rebuild the tree state if the backing SPL
  AC tree is ever ingested from scratch.
- **Public-input contract change.**  ADR-0012 is no longer a
  frozen constant.  Any existing consumer of the 31-input layout
  (SDK, audit docs, tests) must be migrated in lockstep.  The
  hard invariant in CLAUDE.md is updated to `NR_PUBLIC_INPUTS = 32`
  in the same commit that lands the circuit.

### Neutral

- Holder-side UX: the holder SDK fetches the issuer tree's current
  root + the holder's issuer's Merkle path at proof-generation
  time.  Added to `StoredCredential.issuerMerkleProof` or fetched
  fresh per proof via the DAS adapter; TBD in the implementation
  PR.

## Alternatives considered and rejected

### Alt A: CPI from `verify_batch_proof` into `check_issuer_status`

Add a required `issuer_account` (plus `issuer_authority` derivation)
per active credential, CPI into `issuer_registry::check_issuer_status`.

**Rejected because:**

1. **Does not fix the BJJ binding.**  `check_issuer_status` returns
   "authority X is approved" but the circuit's witness is
   `(bjj_x, bjj_y)`.  A malicious prover can pass an approved
   `authority` and a forged `(bjj_x, bjj_y)`.  To close the
   binding we would need the CPI to return the on-chain BJJ key
   for a post-CPI `require_keys_eq!` in the verifier -- pushing
   tx size up and still not reaching zero-knowledge (the BJJ key
   becomes implicitly observable via the chosen issuer PDA).
2. **CU-budget tight.**  Active credentials are typically 1-2 per
   proof, not 4, so this is 1-2 CPIs not 4.  But a single
   `check_issuer_status` CPI already adds context-switch overhead
   on top of an already-expensive Groth16 verify; 2 CPIs per
   verify on mainnet peak would eat into the per-slot budget.
3. **Stateful coupling.**  Changes to `IssuerAccount` layout
   ripple into `check_issuer_status`'s contract with the verifier,
   which breaks the clean separation between `schema-registry`,
   `issuer-registry`, and `zk-verifier`.

### Alt B: Private-input issuer leaf + public-input issuer root (the chosen design)

This is the decision above.

### Alt C: Public-input issuer pubkey + out-of-band approval lookup

Make `(bjj_x, bjj_y)` public inputs, look up the matching issuer
account in the verifier, assert approval.

**Rejected because:** leaks the issuer's BJJ key to anyone reading
the proof / tx, which is load-bearing for privacy in several
downstream verticals (financial-KYC, age-checked content).  The
compressed-tree pattern keeps the key private-input and still gets
the binding.

### Alt D: Defer SEC-004 to Phase 3

**Rejected because:** SEC-004 is on the external-audit radar and
will almost certainly be flagged as critical.  Shipping Phase 2
without it means Phase 3 (external audit + mainnet) starts with a
known critical finding, which under the Section-0 non-negotiable
"External audit is not a rubber stamp" slips mainnet by at least
one sprint beyond audit close-out.  Fix now, not later.

## Open sub-decisions for the implementation PR

Called out so the implementation PR can link to explicit resolution
rather than invent:

1. **Revocation nonce rollover semantics.**  `u64` is overkill for
   anything other than administrative re-approval.  One bump per
   revocation event per issuer; never decrements.  If an issuer is
   re-approved post-cooldown, does their leaf return with the same
   `revocation_nonce` (causing cross-epoch nullifier collision with
   their own old proofs -- bad) or a strictly greater value (clean,
   but requires persisting a counter in `IssuerAccount`)?
   **Tentative choice:** strictly greater.  `IssuerAccount` gains a
   `revocation_nonce: u64` field that monotonically increments;
   every re-approval + every revocation bumps it.

2. **Tree authority.**  PDA `[b"issuer-tree-authority"]` under
   `issuer-registry`.  Same pattern as `tree-authority` for
   credential trees (cf. `programs/issuer-registry/src/lib.rs:45`).

3. **Leaf update pattern.**  SPL AC exposes `append` + `replace_leaf`.
   Status transitions use `replace_leaf` to preserve the issuer's
   slot index; `revoke_issuer` + new approvals use `append` for new
   slots.  Document in the impl PR.

4. **Migration of existing on-chain issuers.**  At deploy time the
   issuer tree is empty; existing approved issuers must be enrolled
   into the tree before proofs can be generated.  The PR ships a
   `scripts/backfill_issuer_tree.ts` that walks `IssuerAccount`
   PDAs and appends each Approved one's leaf.  One-shot; safe to
   re-run (idempotent via leaf lookup).

## References

- SOLID-SEC-004 (this entry is its remediation design).
- SOLID-SEC-008 (epoch-bound nullifier; bundled with this).
- SOLID-SEC-012 (multi-party ceremony; tracks the trusted setup
  that this rev will also need to be redone under).
- ADR-0003 (SPL AC as credential-tree substrate; same substrate).
- ADR-0009 (DAO voting + trust-anchor bypass; status transitions
  are triggered by these paths).
- ADR-0010 (owner-check on tree PDAs; the same pattern applies to
  `IssuerTreeBinding`).
- ADR-0012 (31-public-input contract; revised to 32 in the
  implementation commit).
- `plan/IMPLEMENTATION_PLAN.md` Phase 2 scope.
- `plan/RESUME.md` Phase 2 Next Action item 2.
