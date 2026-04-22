# ADR 0012: 31 public-input contract between circuit and verifier

- **Status:** Accepted (load-bearing invariant)
- **Date:** Phase 3 remediation (SEC-19); formalized 2026-04-22
- **Deciders:** founding team + auditors
- **Affects:** `circuits/batch_credential_query.circom`,
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

Public-input count is fixed at **31**, with layout:

| Index    | Name              | Source                                                 |
|----------|-------------------|--------------------------------------------------------|
| 0        | nullifierHash     | Hardened nullifier output (ADR-0006)                   |
| 1        | globalRoot        | Root of the global identity-state tree                 |
| 2..5     | merkleRoots[4]    | Per-schema credential tree roots                       |
| 6..9     | schemaHashes[4]   | Schema identifiers for each slot (0 for padding)       |
| 10..17   | predicates[4] * 2 | `(operator, queryValue)` pairs                         |
| 18..25   | predicates[4] * 2 | `(queryCredentialIndex, queryFieldIndex)` pairs        |
| 26       | numPredicates     | Active-predicate count (<= MAX_PREDICATES)             |
| 27       | compoundLogic     | 0 = AND, 1 = OR                                        |
| 28       | verifierAddress   | = `ID.to_bytes()` of the verifier program              |
| 29       | verifierNonce     | Verifier-supplied session nonce                        |
| 30       | currentTimestamp  | Expiry check input (SOLID-SEC-005 binds to `Clock`)    |

The contract is enforced at three layers:

1. Circuit: `circuits/batch_credential_query.circom:296-308`.
2. Rust verifier: `NR_PUBLIC_INPUTS = 31` const with per-index
   equality checks (`programs/zk-verifier/src/lib.rs:25-39,170-200`).
3. TS SDK: `ts-sdk/packages/verifier/src/index.ts:33-34` + strict
   `publicInputs.length !== NR_PUBLIC_INPUTS` guard.

A doc drift exists in `ts-sdk/packages/core/src/index.ts:327-346`
(claims 1-6 / 7-30 / 31-33 layout); Phase 1 fixes the comment.

## Consequences

- **Positive.** A mismatched public-input count fails fast at the
  SDK layer; a mismatched index ordering fails at the on-chain
  equality checks. Zero silent-acceptance surface.
- **Negative.** Any change to the layout requires a synchronized
  edit across circuit + verifier + every SDK + test vectors. Adding
  new public inputs (e.g., `vk_generation` for SOLID-SEC-006,
  `epoch_counter` for SOLID-SEC-008) requires a circuit re-compile,
  a new trusted setup, and a coordinated deploy.
- **Neutral.** The layout includes `verifierAddress` in slot 28 and
  `currentTimestamp` in slot 30 -- both of which require on-chain
  equality / freshness checks that v0.3 only partially implements
  (SOLID-SEC-005, SOLID-SEC-031).

## Alternatives considered

- **Hash multiple inputs into one.** Rejected: saves public-input
  slots but pushes verification cost off-chain and complicates
  debugging.
- **Let the circuit emit fewer inputs.** Rejected: opaque outputs
  make future audits harder.

## References

- `programs/zk-verifier/src/lib.rs:25-39`
- `circuits/batch_credential_query.circom:296-308`
- `ts-sdk/packages/verifier/src/index.ts:33-34`
- `CLAUDE.md:39-42` (invariant)
- Related: ADR-0001, ADR-0006, ADR-0011
- SOLID-SEC-005, SOLID-SEC-031
