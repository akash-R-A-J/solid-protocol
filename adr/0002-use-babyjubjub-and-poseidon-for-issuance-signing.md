# ADR 0002: Use BabyJubJub + Poseidon for issuance signing and commitments

- **Status:** Accepted
- **Date:** (pre-v0.1, formalized 2026-04-22)
- **Deciders:** founding team
- **Affects:** `crates/solid-core/src/babyjubjub.rs`,
  `crates/solid-core/src/poseidon.rs`, every circuit in
  `circuits/`, `programs/issuer-registry/src/lib.rs`

## Context

Per ADR-0001 the protocol verifies Groth16 proofs over BN254. In-
circuit signature verification therefore needs a curve whose scalar
field matches BN254's scalar field Fr. BabyJubJub (Barretenberg /
iden3) is the canonical twisted Edwards curve over BN254's scalar
field and supports in-circuit scalar multiplication via Circom's
`BabyPbk` primitive with only a few thousand constraints.

The hash function needs to be SNARK-friendly (few constraints per
hash), collision-resistant, and have mature circomlib support.
Poseidon is the de-facto standard for BN254-based circuits in
2023-2026.

## Decision

- Issuance signatures: EdDSA over BabyJubJub with Poseidon as the
  internal hash.
- Credential commitments: Poseidon (t=6, 5 inputs + 1 output per round).
- Per-schema credential key derivation: Poseidon(masterKey, schemaHash).
- Identity-state leaf: Poseidon(credentialPubKeyAx, credentialPubKeyAy,
  revocationNonce).
- Nullifier: Poseidon of five elements (see ADR-0006).

Cross-language parity is enforced via `tests/vectors/` (see
SOLID-SEC-010 for the current coverage gap).

## Consequences

- **Positive.** All in-circuit signature and hash operations are
  cheap (a few thousand constraints per sig). No hash/circuit
  mismatch between Rust, WASM, and circomlib.
- **Negative.** BabyJubJub and Poseidon are not standard outside the
  SNARK world. Any future integration with EVM-native or traditional
  PKI systems requires bridge primitives. Small-order pubkey attack
  surface if cofactor checks are missed (see SOLID-SEC-007).
- **Neutral.** Locks key storage and wallet UX to BJJ keypairs
  separate from the Solana signing key. `docs/key-management.md`
  describes the holder-side encrypted export/import.

## Alternatives considered

- **Ed25519 native signatures.** Verifiable by Solana's native syscall
  on-chain, but ~100K constraints to verify in-circuit. Prohibitive.
- **ECDSA over secp256k1.** Same in-circuit prohibitive cost.
- **SHA-256 commitments.** ~30K constraints per hash; Poseidon is two
  orders of magnitude cheaper.

## References

- `crates/solid-core/src/babyjubjub.rs:1-467`
- `crates/solid-core/src/poseidon.rs:1-153`
- `circuits/lib/signature_verifier.circom`
- Related: ADR-0001, ADR-0005, ADR-0006
- SOLID-SEC-007, SOLID-SEC-008, SOLID-SEC-010
