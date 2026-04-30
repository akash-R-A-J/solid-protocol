# Remediation Options Archive

This document preserves the remediation alternatives we considered but did
**not** take, so future maintainers can re-evaluate them if the protocol's
constraints change. Each entry records: the decision context, the option's
mechanics, why it was not chosen, and the trigger that would make it the
right answer in the future.

Source-of-truth for the chosen path lives in:

- `docs/E2E_BLOCKERS.md` (B13 remediation)
- `sec/SECURITY_REGISTRY.md` (SOLID-SEC-048, -054)
- `plan/IMPLEMENTATION_PLAN.md` Appendix D
- `sec/audits/2026-04-26_v0.6.1_modular_audit/04_compute/cu_budget.md` §4

This file is the *graveyard* of paths we deliberately rejected.

---

## 1. SOLID-SEC-054 (B13) -- legacy-tx wire size overflow

**Context.** `verify_batch_proof` ix data was 1324 bytes, exceeding
Solana's 1232-byte legacy-transaction packet limit. The chosen path is
**Option 1** (on-chain reconstruction of 11 redundant public inputs;
1324 -> 972 bytes). What follows are the alternatives we did not take.

### 1.1 Decision 1 -- Option 2: buffer-account / chunked upload

**Mechanics.** Caller chunks the proof bytes into a scratch PDA seeded
by `[b"proof-buffer", payer]` over N transactions (typical N is 2-4
depending on chunk size; each ix data << 1232 bytes). A final
`verify_batch_proof_v2(buffer_pda)` ix reads from the scratch PDA and
runs Groth16. Buffer PDA is closed at end of verify to refund rent.

**Why we rejected it.**

- The protocol ships a single-tx Groth16 verify model (alt_bn128
  syscalls + groth16-solana). Splitting that into a multi-tx ceremony
  trades a wire-size constraint for a permanent DX tax: every wallet,
  every SDK, every indexer would have to know about the buffer-PDA
  lifecycle (allocate, chunk, finalize, close).
- The wire-size overflow has a clean single-ix remedy (Option 1) that
  preserves the one-call-one-proof contract integrators want.
- Shrinking 1324 -> 972 bytes leaves a 260-byte margin under 1232; we
  do not need the unconditional headroom Option 2 buys.

**Triggers that flip it to the right answer.**

- *Proof aggregation / batch-of-batches.* If we ever want one tx to
  verify multiple holders' proofs in a single call (indexer-driven
  sweeps, multi-issuer cross-attestation), the buffer pattern is the
  natural primitive for staging the inputs.
- *Circuit grows past the single-tx CU ceiling.* If a future
  trusted-setup cycle blows past 1.4M CU on Groth16 verify itself
  (independent of input count -- driven by IC cardinality and pairing
  structure), we'd split verify into pre-loaded inputs (buffer) plus a
  slim verify ix.
- *Public-input set grows past 21 + framing into 1232 bytes.* With
  current numbers, Option 1 has 260-byte headroom. ~8 more public
  inputs would close that. SEC-006 Part 2 + predicate-operand range
  checks together add < 5 inputs, so we expect Option 1 to last
  through Phase 4.

**Cost reference for sizing.** Adding 2 chunked uploads costs ~5K CU
each (Solana tx framing + signature verify + small writeable PDA),
plus a refundable rent allocation. Latency cost: +400ms (one extra
slot) per chunk for confirmation.

### 1.2 Decision 2 -- shaped struct on the wire (named-field encoding)

**Mechanics.** Define a new Anchor type
`VerifyBatchProofPublicInputsV2 { nullifier, query_credential_indices:
[[u8;32]; 4], ..., current_timestamp }` with named fields. Handler maps
each named field into the corresponding positional slot before Groth16
verify.

**Why we rejected it.**

- The circuit's 32-input layout is positional, frozen by ADR-0012, and
  baked into the trusted setup. A shaped wire struct introduces a
  **third** canonical representation alongside the circuit layout and
  the handler-side reconstruction. Each layer needs a bidirectional
  mapping; the SOLID-SEC-010 cross-language vector job has to verify
  all three stay in sync.
- The Anchor IDL ergonomics argument (`buildVerifyBatchProofIx({
  nullifier, queryResults, ... })` reads nicely) is satisfied at the
  *SDK public surface* level by the hand-rolled wrapper in
  `ts-sdk/packages/verifier/src/index.ts` -- callers get a named-field
  API; the wire stays positional underneath.
- The chosen path keeps two slot-mapping arrays in
  `programs/zk-verifier/src/lib.rs` as the single source of truth:
  `RECONSTRUCTED_INPUT_SLOTS` (length 11) and `WIRE_INPUT_SLOTS`
  (length 21). Adding a public input is "bump NR_PUBLIC_INPUTS, decide
  which array it joins, append its index" -- mechanical.

**Trigger that flips it to the right answer.** Anchor IDL emerges as
the *primary* SDK surface (rather than the hand-rolled wrapper) and
the integration cost of dual encoding outweighs the schema-drift cost.
Not foreseeable on the current trajectory.

### 1.3 Decision 3 -- fully reconstruct currentTimestamp from Clock

**Mechanics.** Drop slot 31 from the wire as well; have the handler
write `Clock::unix_timestamp` (as a u64 LE-padded to 32 bytes) into
`public_inputs[31]` before Groth16 verify. Saves an additional 32
bytes (12 reconstructible total, 988 bytes total -- the original plan
math).

**Why we rejected it -- soundness, not policy.**

- Groth16 public-input equality is polynomial-commitment equality.
  Off-chain provers commit the witness to a specific `T_off` at
  proof-build time; the proof's pairing-equation only holds for that
  exact field element.
- The on-chain `Clock::unix_timestamp` reads `T_chain` at the slot the
  validator includes the tx in. `T_chain != T_off` in general -- slot
  times are ~400ms, network propagation + tx queueing routinely add
  1-2 slots, so `T_chain - T_off` is non-zero with overwhelming
  probability.
- Groth16 has no skew tolerance. `T_chain != T_off` -> rejection of
  every honest proof.
- The SEC-005 skew window (`VerifierConfig.timestamp_skew_seconds`,
  default 600s) is an *additional* on-chain freshness predicate that
  wraps the wire-supplied `T_off`. It is not a substitute for
  cryptographic equality; it cannot make Groth16 forgiving.

**Trigger that flips it to the right answer.** None. This option is
unsound by construction. Recorded here to document the reasoning so a
future maintainer doesn't try to "save 32 more bytes" without reading
the full constraint.

---

## 2. SOLID-SEC-048 -- BJJ prime-order subgroup safety

**Context.** `register_issuer` accepts a BabyJubJub pubkey and must
verify it lies in the prime-order subgroup (cofactor-8 component
absent). The full check (`r * P == O`, `r` ~251-bit subgroup prime)
costs ~1.6-2.0M CU on BPF, exceeding Solana's 1.4M per-tx ceiling.
The chosen path is **Option E** (in-circuit subgroup constraint).
What follows are the alternatives.

### 2.1 Option A -- on-chain bypass + off-chain TS predicate

**Status.** This is the *current shipped state* (under Cargo feature
`sec007-skip-onchain` for localnet/devnet builds only). Superseded by
Option E once that lands.

**Mechanics.** The on-chain `require_in_prime_order_subgroup` call is
gated behind `#[cfg(not(feature = "sec007-skip-onchain"))]`. With the
feature on, the handler runs only the consolation gate (`is_on_curve +
!is_identity`) and emits a structured `Sec007Bypass` event. The
off-chain TS SDK is the load-bearing predicate: every caller of
`register_issuer` is expected to run `isInPrimeOrderSubgroup(pkX, pkY)`
pre-submit.

**Why we rejected it as a long-term answer.**

- "Honest caller forgets the predicate" failure mode is one careless
  integration away from a cofactor-8 torsion key being registered.
- A malicious caller who skips the predicate registers a torsion key
  unhindered; the on-chain consolation gate does not catch it.
- An impacted issuer's downstream EdDSA-Poseidon signatures verify
  only over the 8-element subgroup -- security parameter drops from
  251 bits to 3 bits. Catastrophic for that issuer's credentials.
- M02-H02 finding: no CI gate prevents `sec007-skip-onchain` from
  being compiled into a mainnet release. Documentation-only
  enforcement on a flag whose accidental flip downgrades issuer
  security to negligible.

**Trigger that re-instates it.** Never. Option E closes this for good.

### 2.2 Option B -- off-chain cofactor-clear in SDK

**Mechanics.** Off-chain SDK pre-multiplies the candidate pubkey by 8
(the cofactor) before submission; this kills the cofactor-8 component
by construction. On-chain stays at the consolation gate (~3.5K CU).

**Why we rejected it.**

- Strictly improves on Option A *only against honest callers* (the
  failure mode shrinks from "remember to call the predicate" to "call
  `.mul(8)`"). Against an adversary who deliberately submits a
  torsion-tainted key without multiplying, on-chain detection is
  identical to Option A: nothing catches it.
- "No workaround" stance (this session, 2026-04-28) rules out posture
  improvements that still rely on off-chain enforcement against
  adversaries.

**Trigger that flips it to the right answer.** *Trusted-setup
ceremony cannot be re-run.* If we ever needed an interim improvement
in a window where the ceremony is locked, Option B was the recommended
path. Once Option E is in place, Option B has no remaining role.

### 2.3 Option C -- precomputed-table subgroup check on-chain

**Mechanics.** Pre-compute a table of "valid prime-order subgroup
points" off-chain; on-chain handler verifies the candidate is in the
table.

**Why we rejected it.**

- The prime-order subgroup is large (~2^251 elements). A complete
  table is impossible.
- Any *partial* table only catches keys we anticipated; an attacker
  can choose a key outside the table.
- Table generation requires the same scalar-mul we cannot afford
  on-chain, so this just moves the cost off-chain without solving the
  actual problem.

**Trigger that flips it to the right answer.** None. Recorded as a
strawman that was considered and dismissed during the cu_budget audit
walkthrough.

### 2.4 Option D -- SD-EdDSA torsion-killer multiply on-chain

**Mechanics.** On verify, the handler multiplies the recovered
signature point by 8 (the cofactor) before verification, killing the
cofactor-8 component during signature check rather than at issuer
registration.

**Why we rejected it.**

- This *is* the operation that costs 1.6M CU on BPF. Moving it from
  registration to verification just relocates the same compute wall.
- Verifying signatures on-chain is also outside the protocol's hot
  path -- signatures are verified inside the Groth16 circuit. So
  this option never had a place.

**Trigger that flips it to the right answer.** None.

### 2.5 Option F -- Solana sol_babyjubjub_* syscall

**Mechanics.** Propose adding a `sol_babyjubjub_subgroup_check`
(or full `sol_babyjubjub_*` family) syscall to the Solana runtime
upstream so on-chain code can perform subgroup / scalar-mul checks at
curve speed (~3K CU).

**Why we rejected it as the immediate fix.**

- Multi-quarter timeline. Requires a SIMD, validator buy-in, and
  upstream Solana team prioritization. We need SEC-048 closed before
  mainnet, not on a calendar that depends on Solana's roadmap.
- The risk profile of "wait for upstream" is asymmetric: the cost of
  waiting is a mainnet date slip; the cost of running Option E is one
  ceremony.

**Trigger that flips it to a future improvement.** When the syscall
ships, the in-circuit subgroup constraint becomes redundant (the
on-chain register_issuer can resume the full check at curve speed for
~3K CU). At that point we can run another trusted-setup cycle to drop
the in-circuit constraint -- saving ~30K constraints and ~250ms
proving cost per proof. Pure performance tuning, not a soundness
question; bump only if proving cost becomes a felt UX problem.

---

## 3. Conventions for adding entries

When a future option is rejected, add a subsection under its parent
finding with this shape:

- **Mechanics.** What the option does, in plain language.
- **Why we rejected it.** Reasoning that future-you can re-read.
- **Trigger that flips it to the right answer.** Concrete scenario
  (constraint change, new syscall, scope shift) that would make this
  the right call. "None" is a valid answer if the option was
  unsound or strictly dominated.

The bar is: a maintainer six months from now should be able to read
this doc and either (a) adopt the option without re-deriving its
viability or (b) understand why it's still the wrong call without
having to re-litigate the argument.
