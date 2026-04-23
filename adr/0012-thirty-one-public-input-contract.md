# ADR 0012: 32 public-input contract between circuit and verifier

> **Note on filename.**  The file is named `...thirty-one...` because
> that was the v0.x public-input count.  It is kept for git-history
> continuity; the *current* invariant is **32** public inputs post
> ADR-0014.  If a future split is ever warranted, rename the file.

- **Status:** Accepted (load-bearing invariant).  Revised from 31 to
  32 by ADR-0014 on 2026-04-24.
- **Date:** Phase 3 remediation (SEC-19); formalized 2026-04-22;
  revised 2026-04-24 for the ADR-0014 circuit rev.
- **Deciders:** founding team + auditors
- **Affects:** `circuits/batch_credential_query.circom`,
  `circuits/compound_query.circom`,
  `programs/zk-verifier/src/lib.rs`, every TS SDK package that
  constructs proof payloads.

## Context

The Groth16 verifier collapses the circuit's public inputs into an
input commitment that must match the proof's IC combination. A
single out-of-order public input breaks the verifier without a clear
error signal; a single missing one causes parse failures that look
like arbitrary rejections. The circuit, the on-chain program, and
the SDKs must agree exactly on count and order.

## Decision

Public-input count is fixed at **32**, with layout:

| Index    | Name                  | Source                                                 |
|----------|-----------------------|--------------------------------------------------------|
| 0        | nullifierHash         | Hardened nullifier output (ADR-0006; 6-input post ADR-0014) |
| 1        | globalRoot            | Root of the global identity-state tree                 |
| 2..5     | merkleRoots[4]        | Per-schema credential tree roots                       |
| 6..9     | schemaHashes[4]       | Schema identifiers for each slot (0 for padding)       |
| **10**   | **issuerTreeRoot**    | **ADR-0014: singleton issuer-tree root; SEC-004 + SEC-008** |
| 11..14   | queryCredentialIndices[4] | (shifted +1 from v0.x)                             |
| 15..18   | queryFieldIndices[4]  | (shifted +1 from v0.x)                                 |
| 19..22   | queryOperators[4]     | (shifted +1 from v0.x)                                 |
| 23..26   | queryValues[4]        | (shifted +1 from v0.x)                                 |
| 27       | numPredicates         | Active-predicate count (<= MAX_PREDICATES)             |
| 28       | compoundLogic         | 0 = AND, 1 = OR                                        |
| 29       | verifierAddress       | = `ID.to_bytes()` of the verifier program              |
| 30       | verifierNonce         | Verifier-supplied session nonce                        |
| 31       | currentTimestamp      | Expiry check input (SOLID-SEC-005 binds to `Clock`)    |

The contract is enforced at three layers:

1. Circuit: `circuits/batch_credential_query.circom` public-list
   declaration near the bottom of the file; `component main {public
   [...]}`.
2. Rust verifier: `NR_PUBLIC_INPUTS = 32` const with per-index
   equality checks (`programs/zk-verifier/src/lib.rs`).  The named
   constants `ISSUER_TREE_ROOT_INPUT_INDEX`,
   `VERIFIER_ADDRESS_INPUT_INDEX`, and `CURRENT_TIMESTAMP_INPUT_INDEX`
   are the handler's single source of truth for the shifted slots.
3. TS SDK: every proof-constructor guard must assert
   `publicInputs.length === 32`.  Updated in Phase 2 impl 3
   alongside the holder-side issuer-tree proof wiring.

## Consequences

- **Positive.** A mismatched public-input count fails fast at the
  SDK layer; a mismatched index ordering fails at the on-chain
  equality checks. Zero silent-acceptance surface.
- **Negative.** Any change to the layout requires a synchronized
  edit across circuit + verifier + every SDK + test vectors + the
  trusted setup + the on-chain VK.  The ADR-0014 revision paid this
  cost once to close SEC-004 + SEC-008; no further public-input
  additions are planned for Phase 2.
- **Neutral.** `verifierAddress` lives at slot 29, `currentTimestamp`
  at slot 31, `issuerTreeRoot` at slot 10.  All three require
  specific on-chain equality / freshness checks, which the verifier
  handler does inline.

## Migration note

Proofs produced against the v0.x 31-input circuit are incompatible
with the ADR-0014-revised verifier and vice versa.  A coordinated
deploy is required:

1. Rerun `circuits/scripts/setup.js` to produce a new zkey that
   matches the 32-input circuit.
2. Re-upload the resulting `verification_key.json` via
   `scripts/initialize.ts` (which CPIs `store_verification_key`
   chunks into `verifier_config`).
3. Back off any in-flight proofs that were produced against the
   previous zkey; they will fail the IC check.

## Alternatives considered

- **Hash multiple inputs into one.** Rejected: saves public-input
  slots but pushes verification cost off-chain and complicates
  debugging.
- **Let the circuit emit fewer inputs.** Rejected: opaque outputs
  make future audits harder.
- **Put `issuerTreeRoot` at the END of the layout** rather than
  slot 10.  Rejected: semantically it groups with the other
  Merkle-root public inputs (`globalRoot`, `merkleRoots[]`,
  `schemaHashes[]`), and keeping related roots adjacent aids
  reviewability.

## References

- `programs/zk-verifier/src/lib.rs:24-72` (constants + indices)
- `circuits/batch_credential_query.circom` (public list at bottom)
- `CLAUDE.md` (hard invariant -- bumped to 32 in the same commit)
- ADR-0014 (the driver for this revision)
- Related: ADR-0001, ADR-0006, ADR-0011
- SOLID-SEC-005, SOLID-SEC-031, SOLID-SEC-004, SOLID-SEC-008
