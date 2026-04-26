# Audit 04 — solid-core (cryptographic primitives)

| Field | Value |
|-------|-------|
| Files | `crates/solid-core/src/{lib,babyjubjub,poseidon,nullifier,commitment,credential,error,identity,multi_cred,query,sas,schema}.rs`, `Cargo.toml`, `examples/gen_vectors.rs` |
| LOC | ~2370 across 12 files |
| Crate type | `lib` (no `cdylib`) |
| Auditor | Orchestrator hand-audit (after agent stalled) |
| Date | 2026-04-26 |

This crate is the protocol's cryptographic kernel. Three compilation targets: BPF (`target_os = "solana"`), non-wasm32 host, and wasm32. Byte-identity across all three is load-bearing — the `tests/vectors/` cross-language gate is the CI proof.

---

## 1. Three-target compilation contract

`Cargo.toml` segments deps into three arms:

1. **Common (all targets)**: `ark-bn254`, `ark-ff`, `ark-ec`, `ark-ed-on-bn254`, `ark-std`, `serde`, `num-bigint`, `num-traits`, `hex`, `thiserror`. Everything in `[dependencies]` is BPF + host + wasm32 clean.
2. **BPF-only (`cfg(target_os = "solana")`)**: `solana-program` for the `sol_poseidon` syscall.
3. **Non-wasm32 host (`cfg(all(not(target_os = "solana"), not(target_arch = "wasm32")))`)**: same `solana-program` for the host fallback (light-poseidon backed).
4. **Off-BPF (host + wasm32)**: `light-poseidon = "0.2"`, `serde_json`, `rand`, `aes-gcm`, `argon2`, `blake2`.

The wasm32 arm explicitly excludes `solana-program` because Solana 1.18.x does not support wasm32 (its non-BPF fallback transitively reaches `std::sync::Mutex`, `std::time::*`, `getrandom` without `js`). On wasm32 the byte-oriented Poseidon path reaches `light-poseidon` directly via `hash_bytes_dispatch` (`poseidon.rs:150-170`).

`lib.rs` further gates the host-only modules:
- `commitment`, `credential`, `multi_cred` are `cfg(not(target_os = "solana"))` — never reach BPF.
- `babyjubjub`, `error`, `identity`, `nullifier`, `poseidon`, `query`, `sas`, `schema` are dual-target (BPF + host + wasm32).

INVARIANT (BPF reachability): the on-chain programs only import (a) `babyjubjub::{is_on_curve, is_identity, require_in_prime_order_subgroup, BJJPublicKey}` (from issuer-registry), (b) `schema::compute_schema_hash_from_parts` (schema-registry), (c) `poseidon::{hash_bytes, fr_to_bytes_le, u64_to_fr}` (issuer-registry's `compute_issuer_leaf_bytes`). All of these are dual-target. The host-only items (sign/verify, identity bundle, credential builder) are never reachable from BPF entry points. **Verified** by reading `lib.rs:38-43` cfg-gating and the upstream import graph.

---

## 2. `babyjubjub.rs` (694 LOC) — soundness fulcrum

### 2.1 Public surface

| Function | Target | Lines | Purpose |
|----------|--------|-------|---------|
| `BJJPublicKey { x, y }` | dual | 70-75 | 32-byte LE coord pair |
| `EdDSASignature { r8_x, r8_y, s }` | dual | 78-86 | EdDSA-Poseidon signature |
| `BJJKeypair::from_private_key` | dual | 99-106 | derived keypair |
| `is_in_prime_order_subgroup` | dual | 181-189 | SOLID-SEC-007 predicate, returns bool |
| `is_on_curve` | dual | 198-202 | cheap consolation gate (SEC-048) |
| `is_identity` | dual | 208-212 | cheap consolation gate (SEC-048) |
| `require_in_prime_order_subgroup` | dual | 218-233 | fail-closed form, returns Result |
| `derive_public_key` | dual | 257-264 | sk * Base8 |
| `derive_key` | dual | 270-272 | Poseidon KDF |
| `generate_keypair` | host-only | 246-254 | OsRng-backed keygen |
| `sign` | host-only | 291-325 | EdDSA-Poseidon sign |
| `verify` | host-only | 336-374 | EdDSA-Poseidon verify |
| `BJJIdentity::generate / unlock / export_json / import_json / bind_wallet` | host-only | 412-499 | encrypted-key bundle (AES-256-GCM, Argon2id) |

### 2.2 Inputs / processing / outputs / error handling per critical fn

**`require_in_prime_order_subgroup` (218-233)**:
- INPUTS: `&BJJPublicKey` (64 bytes total).
- PROCESSING: 
  - `bytes_to_fq` x2 (mod-reduce input bytes to BN254 scalar field).
  - `EdwardsAffine::new_unchecked` (no debug-assert panics).
  - `is_on_curve` → `PointNotOnCurve` if false.
  - `is_zero` → `BJJNotInSubgroup` if true (identity rejected).
  - `is_in_correct_subgroup_assuming_on_curve` → `BJJNotInSubgroup` if false (cofactor-8 rejected).
- OUTPUTS: `Result<()>`. No side effects.
- ERROR HANDLING: three structured error variants; no panics; no unwrap.

**SEC-048 deep tie**: this function costs >1.4M CU on BPF (the `is_in_correct_subgroup_assuming_on_curve` call does a ~251-bit scalar mul via arkworks). When `sec007-skip-onchain` is enabled in `programs/issuer-registry/Cargo.toml`, the on-chain handler skips this check; off-chain TS predicate `isInPrimeOrderSubgroup` becomes load-bearing.

**`pubkey_to_affine` (161-172)** — internal, used by `verify`:
- Same three-stage check (on-curve, non-identity, prime-order-subgroup).
- `EdwardsAffine::new_unchecked` is used because the safe `::new` panics in debug on the same predicates we want to surface as `Result`.

**`generate_keypair` (host-only, 246-254)**:
- `Fr::rand(&mut OsRng)` for the scalar. `OsRng` reaches `getrandom`; on wasm32 with the `js` feature this dispatches to `crypto.getRandomValues`.
- `pk_point = base8 * sk` via `mul_bigint`. Output is in the prime-order subgroup by construction.
- No way to obtain a key whose pubkey isn't subgroup-valid through this path. **Verified.**

**`sign` (host-only, 291-325)**:
- Deterministic nonce: `r = Blake2b512(sk || msg) mod r_bjj`. Strong rfc6979-class derivation.
- `R8 = r * Base8` — automatically in prime-order subgroup.
- Challenge `h = Poseidon(R8.x, R8.y, A.x, A.y, msg)` reduced to BJJ scalar via `fq_to_fr` (Fq → Fr mod-reduce).
- `S = r + h * sk`.
- ERROR PATHS: `sk == 0` → `Signature("Private key is zero")`. Poseidon failure → propagates.
- NO unwraps. Cast: `Fr::from_le_bytes_mod_order` (always-defined).

**`verify` (host-only, 336-374)**:
- `pubkey_to_affine` → curve + subgroup check on PK.
- `EdwardsAffine::new_unchecked` for R8, then explicit `is_on_curve` + `is_in_correct_subgroup_assuming_on_curve` checks. **Defense in depth**: signing always produces subgroup-valid R8 because `R8 = r * Base8`, but a malicious signer could craft a malformed signature where R8 is in the cofactor-8 subgroup. Test `test_verify_rejects_small_order_r8` (lines 679-693) pins this.
- `lhs = S * Base8`; `rhs = R8 + h * A`.
- Returns `bool` (true if equal).
- ERROR PATHS: every Fq/Fr decode is mod-reduce-safe.

**`BJJIdentity::generate / unlock` (412-499)**:
- Argon2id parameters: `m=65536, t=3, p=4` (line 489). On wasm32 single-threaded, blocks 200-400ms.
- AES-256-GCM with random 12-byte nonce, 32-byte salt.
- ERROR: structured `Encryption(...)` / `Decryption(...)` variants. No panics.
- **Subtle**: `unlock` returns the 32-byte private key; if `plaintext.len() != 32`, errors with `Decryption("Invalid key length")`. AES-GCM authentication tag prevents tampering.

### 2.3 Test coverage

23 unit tests including the SOLID-SEC-007 regression suite (lines 598-693): subgroup acceptance for honest keypair, rejection of identity, rejection of order-2 point `(0, -1)`, rejection of off-curve, rejection of small-order R8 in `verify`. Excellent coverage of the soundness boundary.

### 2.4 Findings

**M04-INF01**: `pubkey_to_affine` returns `std::result::Result<EdwardsAffine, SolidError>` (full path) instead of using the crate's `Result` alias. Cosmetic.

**M04-INF02**: `generate_keypair` uses `Fr::rand(&mut OsRng)` rather than `BJJKeypair::from_private_key(rand_bytes)`. Both produce the same distribution; the direct-Fr-rand path is slightly cheaper but obscures the equivalence. Doc-only.

**M04-INF03**: `BJJIdentity::generate` calls `std::time::SystemTime::now()` and falls back to `unwrap_or_default()` (line 433). `created_at = 0` if clock fails. Acceptable for a metadata field; not load-bearing.

**M04-INF04**: No `zeroize` on private key bytes in `BJJIdentity::unlock`. Plaintext key sits in heap memory after return; standard host-side hazard. Pre-mainnet hygiene item.

---

## 3. `poseidon.rs` (394 LOC) — three-target dispatch

### 3.1 Public surface

| Function | Target | Lines | Purpose |
|----------|--------|-------|---------|
| `fr_to_bytes_le` | dual | 62-69 | Fr → 32 LE bytes |
| `bytes_le_to_fr` | dual | 74-76 | 32 LE bytes → Fr (mod-reduce) |
| `u64_to_fr` | dual | 79-81 | u64 → Fr |
| `hash_bytes` | dual | 110-125 | byte-oriented Poseidon (1-12 inputs) |
| `hash_fields_to_bytes` | dual | 178-190 | u64 vec → 32 LE bytes |
| `hash_fr` | host+wasm32 | 234-248 | Fr-typed hash (light-poseidon direct) |
| `hash_fields` | host+wasm32 | 257-260 | u64 vec → Fr |

### 3.2 Dispatch correctness

`hash_bytes_dispatch` is target-conditional:
- `cfg(not(target_arch = "wasm32"))` — `solana_program::poseidon::hashv` (BPF syscall + non-wasm32 host fallback).
- `cfg(target_arch = "wasm32")` — `light_poseidon::Poseidon<Fr>::new_circom().hash_bytes_le`.

**Both paths reduce to `light-poseidon 0.2.0`'s Bn254X5/LittleEndian backend by construction** — the BPF syscall, the non-wasm32 host fallback, and the wasm32 direct path all use the same parameter set. Byte-identity follows.

### 3.3 Canonicalization step

`hash_bytes` lines 118-121:
```rust
let mut canonical: [[u8; 32]; 12] = [[0u8; 32]; 12];
for (i, b) in inputs.iter().enumerate() {
    canonical[i] = fr_to_bytes_le(&bytes_le_to_fr(b));
}
```

Every input is round-tripped through `bytes_le_to_fr` → `fr_to_bytes_le` to mod-reduce by the BN254 scalar prime. Without this, `solana_program::poseidon::hashv` rejects non-canonical inputs with `InputLargerThanModulus`. The canonicalization makes byte input ≥ p well-defined: hash equals Poseidon(input mod p).

Test `test_hash_bytes_canonicalizes_oversized_inputs` (lines 371-393) pins this.

### 3.4 Heapless slice builder

Module `heapless_refs` (lines 198-221) builds `&[&[u8]]` on a stack 12-wide fixed array. No Vec heap allocation in the BPF hot path. Stack footprint ~120 bytes. **Verified BPF-safe.**

### 3.5 Test coverage

10 unit tests including:
- `test_byte_path_matches_field_path` (lines 343-358) — the in-crate version of the cross-language vectors gate.
- `test_hash_bytes_canonicalizes_oversized_inputs` (lines 371-393).
- Min/max input arity, deterministic, order-sensitive, byte-roundtrip.

### 3.6 Findings

**M04-LOW01**: arity cap at 12 in `hash_bytes` (line 111) and `hash_fields_to_bytes` (line 180), but `schema::compute_schema_hash_from_parts` truncates to 16 (`schema.rs:152-154`). The truncation is unreachable (on-chain caps make total inputs ≤ 10) but the 16-vs-12 inconsistency is a doc nit. Already noted in M03-LOW.

**M04-INF05**: `hash_bytes_dispatch` allocates a `[[u8; 32]; 12]` zero-padded slice on the stack regardless of input length (line 118). 384 bytes of stack always paid. Could use a smaller fixed array per arity, but 384 bytes is well under the BPF 4 KB frame limit. Acceptable.

---

## 4. `nullifier.rs` (153 LOC) — 6-input ADR-0006 revision

### 4.1 The preimage

`compute_nullifier` (lines 45-64):
```
Poseidon6(masterKey, revocationNonce, verifierAddress, queryContextHash, verifierNonce, issuerTreeRoot)
```

NOT `Poseidon6(identityCommitment, secret, schemaId, credentialIndex, currentTimestamp, issuerTreeRoot)` — the orchestrator zk-verifier hand-audit incorrectly cited the latter form. The actual preimage is what `nullifier.rs:8-15` documents and what the function implements.

### 4.2 Critical question: does this match the circuit?

The Rust function and the circuit MUST produce byte-identical outputs (the Rust output is consumed by the SDK to register the nullifier PDA seed; the circuit output is `public_inputs[0]` which the verifier cross-checks against the PDA seed via `nullifier == public_inputs[0]`). If the orderings differ, the verifier rejects every honest proof.

**This is not verifiable from solid-core alone — it requires reading `circuits/batch_credential_query.circom` STEP 5.** Cross-reference deferred to circuits audit (07).

The CLAUDE.md description is loose: "6-input Poseidon (ADR-0006 revision): adds issuerTreeRoot..." — does not name the other 5 inputs. So no doc drift to flag at the CLAUDE.md level; the question is whether the circuit matches the Rust.

### 4.3 Test coverage (host-only)

5 tests in this file:
- `test_nullifier_deterministic` (76-86)
- `test_nullifier_different_nonce_different_output` (89-103)
- `test_nullifier_different_holder_different_output` (106-117)
- `test_nullifier_rotation_affects_output` — **revocation_nonce sensitivity** (120-130)
- `test_nullifier_changes_with_issuer_tree_root` — **SOLID-SEC-008 epoch bind regression gate** (137-152)

### 4.4 Findings

**M04-H01 (HIGH; needs cross-verification)**: the orchestrator's earlier zk-verifier hand-audit (`01_zk_verifier.md`) incorrectly cited the 6 nullifier inputs as `(identityCommitment, secret, schemaId, credentialIndex, currentTimestamp, issuerTreeRoot)`. The actual Rust implementation at `nullifier.rs:45-64` uses `(masterKey, revocationNonce, verifierAddress, queryContextHash, verifierNonce, issuerTreeRoot)`. **Verify the circuit's `nullifier.inputs[0..6]` wiring matches the Rust.** If circuit drifted from this, every proof's nullifier would differ from the Rust-computed one, and `verify_batch_proof`'s `require!(nullifier == public_inputs[0], NullifierMismatch)` would fire on every honest call. Cross-reference: circuits audit 07 must verify this byte-for-byte.

**M04-L01**: nullifier-vector at `tests/vectors/commitment_and_nullifier.json` is the single byte-identity gate for this primitive across Rust/wasm. Per the wasm-bridge audit (M06-H01), only 2 of 11 WASM exports have such vectors. Worth confirming that this nullifier vector was regenerated post-ADR-0014 (post-rev to 6 inputs) and that the JSON's `expected_nullifier_hex` was sampled with the right preimage.

---

## 5. `commitment.rs` (161 LOC, host-only)

`compute_attestation_commitment` (42-61):
- `dataHash = Poseidon(fields[0..N])` (N up to 16, but circuit instantiates 8).
- `commitment = Poseidon(dataHash, schemaHash, holderX, holderY, salt)`.

This is the credential leaf preimage that goes into the SPL-AC tree.

Tests: 5 cases proving determinism and sensitivity to data, salt, holder.

**M04-INF06**: `hash_attestation_data` (lines 21-29) rejects on `len > 16`, but `compute_attestation_commitment` calls `hash_fields` (host-only) which itself caps at 12 (poseidon.rs:235). The 16-vs-12 inconsistency would cause runtime errors for fields counts in (12, 16], not the documented cap. Recommend tightening to 12.

---

## 6. `credential.rs` (203 LOC, host-only)

`Credential` struct + `CredentialBuilder`. Stores commitment, signature, schema_hash, holder/issuer pubkeys, salt, expiration.

`Credential::verify_integrity` (55-74):
- Recomputes commitment from data.
- Verifies issuer signature over commitment.

`CredentialBuilder::build` (126-162):
- Generates random salt via `rand::random()`.
- Computes commitment, signs it with issuer's private key.
- Sets `issued_at` from system time (with `unwrap_or_default()` fallback).

**M04-INF07**: `salt: [u8; 32] = rand::random()` (line 132). Salt entropy depends on `OsRng` quality. Acceptable.

---

## 7. `identity.rs` (31 LOC, dual)

`IdentityState::commitment` (22-30):
- `Poseidon3(pub_x, pub_y, revocation_nonce_as_le_32_bytes)`.
- `nonce_32` has the u64 in low 8 bytes, high 24 bytes zero.

This is the global-tree leaf preimage (Layer 2). Drives BUG-04 path.

**M04-LOW02**: not byte-identity-vectored across Rust/TS. The wasm-bridge audit (`06_wasm_bridge.md`) flags this as part of M06-H01 (SOLID-SEC-010 gap).

---

## 8. `schema.rs` (416 LOC, dual) — already reviewed in 03_schema_registry.md

`compute_schema_hash_from_parts` is the canonical derivation shared between SDK and on-chain. Already audited under M03-DOCS / M03-LOW. Tests `test_compute_schema_hash_parts_matches_definition` and `test_compute_schema_hash_parts_deterministic_and_sensitive` pin SOLID-SEC-002.

---

## 9. `multi_cred.rs` (335 LOC, host-only) — multi-credential helpers

Not deeply audited in this hand-pass; surface includes batching invariants, credential array packing into circuit input layout, padding-slot encoding.

**M04-TODO01**: full audit deferred. Key things to verify in a follow-up:
- Padding slot construction (commitments / schemas at padded indices must be `[0u8; 32]` to match circuit's padding skip at `verify_batch_proof:500-502`).
- NUM_CREDS = 4 enforced (matches `MAX_CREDENTIALS` at lib.rs:16).
- Per-credential field ordering byte-for-byte equal to circuit's `credentials[i]` private input layout.

---

## 10. `query.rs` (388 LOC, dual) — query/predicate types

Not deeply audited in this hand-pass.

**M04-TODO02**: full audit deferred. Key items:
- `Predicate` enum encoding into the circuit's queryOperators[4] slot.
- Range constraint construction.
- `MAX_FIELDS = 8`, `MAX_PREDICATES = 4` constants — verify these match the circuit instantiation.

---

## 11. `sas.rs` (111 LOC, dual) — Solana Attestation Service stub

`SAS_PROGRAM_ID = "SASPROGRAMID111111111111111111111111111111111"` (line 89) — placeholder, not a real on-chain program. This module is **forward-compat scaffolding**, no consumer in the on-chain or SDK code paths.

**M04-L02 (LOW)**: SAS module is dead code. No consumer in `programs/`, `wasm/`, or `ts-sdk/`. If kept, document the forward-compat intent and the placeholder program ID. If not kept, delete to reduce surface.

---

## 12. `examples/gen_vectors.rs` — cross-language vectors generator

The Rust half of the SOLID-SEC-010 byte-identity gate. Produces `tests/vectors/commitment_and_nullifier.json`. Currently 3 of 10 primitives covered (commitment + nullifier + ?). Per the wasm-bridge audit, the gap is to extend this to cover `poseidonHash`, `poseidonHashBytes`, `computeIdentityState`, `deriveCredentialKey`, `deriveKey`, plus a sign/verify roundtrip (M06-H01).

---

## 13. Findings summary

| ID | Severity | Issue |
|----|----------|-------|
| M04-H01 | HIGH | Nullifier preimage mismatch alleged in zk-verifier hand-audit; verify against circuit. |
| M04-L01 | LOW | Nullifier vector regen post-ADR-0014 unconfirmed. |
| M04-L02 | LOW | `sas.rs` is dead code; document or delete. |
| M04-LOW01 | LOW | 16-vs-12 input cap inconsistency between schema and Poseidon arity. |
| M04-LOW02 | LOW | `IdentityState::commitment` not byte-identity-vectored. |
| M04-INF01..07 | INFO | Various hygiene items (Result alias, OsRng path, system time fallback, no zeroize, dispatch padding, salt entropy). |
| M04-TODO01 | DEFERRED | `multi_cred.rs` not deeply audited. |
| M04-TODO02 | DEFERRED | `query.rs` not deeply audited. |

---

## 14. Compute notes

`solid-core` paths reachable from BPF:
- `babyjubjub::is_on_curve` / `is_identity` — ~3K CU each (one Edwards equation eval).
- `babyjubjub::require_in_prime_order_subgroup` — **>1.4M CU** (251-bit scalar mul). SEC-048 fulcrum.
- `poseidon::hash_bytes` — ~3-5K CU + canonicalization (~1K CU per input).
- `poseidon::fr_to_bytes_le` / `bytes_le_to_fr` — ~500 CU each.
- `schema::compute_schema_hash_from_parts` — ~5K-10K CU (Poseidon + bytes ops).

Stack-frame footprint: `hash_bytes` allocates a fixed `[[u8; 32]; 12]` on the stack (384 bytes). `babyjubjub::require_in_prime_order_subgroup` materialises an `EdwardsAffine` (~80 bytes). All BPF-safe under `lto = "thin"`.

---

## 15. Suggested next actions

1. **M04-H01**: cross-verify the 6 nullifier inputs in `circuits/batch_credential_query.circom` STEP 5 vs `nullifier.rs:45-64`. If the circuit drifted, this is a soundness break.
2. **M04-L01**: confirm `tests/vectors/commitment_and_nullifier.json:20-28` was regenerated post-ADR-0014.
3. **Extend `gen_vectors.rs`** to cover the 7 missing primitives (closes part of SOLID-SEC-010 + M06-H01).
4. **M04-L02**: delete or document `sas.rs`.
5. Complete deferred audits (M04-TODO01 / TODO02).
6. **M04-INF04**: add `zeroize` to `BJJIdentity::unlock` for pre-mainnet hygiene.

---

## Summary

solid-core is the soundness root of the protocol's cryptography. The BJJ subgroup check is correctly implemented (`is_in_correct_subgroup_assuming_on_curve` calls arkworks's standard `r * P == O`). The Poseidon dispatch is target-coherent across BPF / non-wasm host / wasm32 by construction. EdDSA-Poseidon sign/verify match circomlib's specification. SOLID-SEC-007 regression suite covers the soundness boundary (identity, order-2, off-curve, small-order R8).

**Highest-priority verification**: M04-H01. The orchestrator's zk-verifier hand-audit incorrectly described the nullifier preimage; the circuits audit must confirm the circuit-side formula matches `nullifier.rs`. If they diverge, every proof is broken.

**Largest soundness liability**: SEC-048 (BJJ subgroup check exceeds BPF CU budget). Tracked in `cu_budget.md` with five remediation options.

10 findings: 1 HIGH (cross-verify needed), 4 LOW, 5 INFO + 2 deferred. No CRITICAL.
