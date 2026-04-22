# ADR 0001: Use Groth16 over alt_bn128 for on-chain verification

- **Status:** Accepted
- **Date:** (pre-v0.1, formalized 2026-04-22)
- **Deciders:** founding team
- **Affects:** `programs/zk-verifier`, `circuits/`, `tools/solid-prover`

## Context

The protocol needs a succinct proof system that (a) verifies cheaply
on Solana, (b) has mature tooling for authoring identity circuits,
and (c) composes with the BabyJubJub / Poseidon choices in ADR-0002.

Solana exposes native alt_bn128 pairing, scalar multiplication, and
addition as syscalls. A single pairing check is approximately 165K
CU; the full Groth16 verify for a 31-public-input circuit comes in
around 280-320K CU, comfortably under the 400K default and well
under the 1.4M max.

The alternatives are: Plonk (BN254 or KZG), STARKs, Halo2. None has
first-class Solana syscall support today. A non-native verifier
would cost an order of magnitude more CU and would either exceed the
tx limit or force proof splitting.

## Decision

Use Groth16 over BN254 (alt_bn128) as the single on-chain proof
system for v0.3. Author circuits in Circom 2.1.9; compile to R1CS
and to WebAssembly for client-side witness generation; produce the
`.zkey` via snarkjs Phase 2; verify via Solana's `alt_bn128_pairing`
syscall through the `groth16-solana` crate.

## Consequences

- **Positive.** Cheapest possible on-chain verification on Solana
  today. Mature tooling (circomlib, snarkjs, ark-circom). Trivial to
  port witness generation to any language with a BN254 library.
- **Negative.** Circuit-specific trusted setup. Every structural
  circuit change requires a fresh ceremony (see ADR-0007 and
  SOLID-SEC-001, SOLID-SEC-004, SOLID-SEC-008 for bundling policy).
  No native proof aggregation.
- **Neutral.** Locks the protocol to BN254, which in turn pins
  BabyJubJub and Poseidon choices in ADR-0002.

## Alternatives considered

- **Plonk.** Would allow a universal setup but doubles on-chain
  verification cost; no Solana-native pairing syscall path today.
- **STARKs.** No trusted setup, but proof size and verifier cost
  are an order of magnitude larger; no Solana native support.
- **Halo2.** Recursive, no trusted setup, but maturity and tooling
  on Solana are not there in 2026.

## References

- `programs/zk-verifier/src/lib.rs:157-297` (verify_batch_proof)
- `tools/solid-prover/src/lib.rs` (native ark-groth16 server prover)
- `CLAUDE.md:10-14` (load-bearing invariant)
- Related: ADR-0002, ADR-0012
