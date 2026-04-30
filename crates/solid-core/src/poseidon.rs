//! Circomlib-compatible Poseidon hash.
//!
//! All outputs are byte-for-byte identical to circomlib's `poseidon.circom`.
//! This is critical — any divergence means proofs generated off-chain won't
//! verify in the on-chain Groth16 verifier.
//!
//! ## Three-target implementation
//!
//! `solid-core` is consumed by three distinct compilation targets, each
//! with a Poseidon backend that physically fits its environment.  All
//! three converge on the same byte output by construction (same crate,
//! same parameters, same canonicalization).  See SOLID-SEC-010 / ADR-0006
//! and `docs/E2E_BLOCKERS.md` B6 for the full rationale.
//!
//! - **BPF** (`target_os = "solana"`): byte-oriented `hash_bytes` /
//!   `hash_fields_to_bytes` call `solana_program::poseidon::hashv`,
//!   which dispatches to the `sol_poseidon` syscall.  No
//!   `light-poseidon` parameter table is loaded into the BPF binary,
//!   so there is no 4 KB stack-frame overflow under `lto = "fat"`
//!   (B3 fix).
//! - **Non-wasm32 host** (`cfg(all(not(target_os = "solana"),
//!   not(target_arch = "wasm32")))`): same call path as BPF.
//!   `solana_program::poseidon::hashv` falls back to a pure-Rust
//!   implementation backed by `light-poseidon 0.2.0`, byte-identical
//!   to the syscall.  Used by `cargo test`, `examples/gen_vectors`,
//!   the off-chain prover, and the cross-language-vectors gate.
//! - **wasm32** (`cfg(target_arch = "wasm32")`): reaches
//!   `light_poseidon::Poseidon::<Fr>::new_circom(..).hash_bytes_le(..)`
//!   directly.  `solana-program 1.18.x` does not support
//!   wasm32-unknown-unknown (its non-BPF fallback transitively reaches
//!   `std::sync::Mutex`, `std::time::*`, and `getrandom` without `js`),
//!   so wasm32 must use the pure-Rust backend.  This produces the
//!   exact same byte output as the host fallback by construction —
//!   both paths reduce to `light-poseidon 0.2.0`'s `hash_bytes_le`
//!   under `Bn254X5` / `LittleEndian` parameters, gated by the
//!   `tests/vectors/` cross-language CI job.
//!
//! The field-element-oriented helpers (`hash_fr`, `hash_fields`) are
//! off-BPF only (host + wasm32) because they expose `light-poseidon`'s
//! `Fr` API, which only the SDK / prover side needs.  See ADR-0006.
//!
//! Internally operates on `ark_bn254::Fr` (BN254 scalar field), which is the
//! same field as `ark_ed_on_bn254::Fq` (BabyJubJub base field / point coordinates).

use ark_bn254::Fr;
use ark_ff::{BigInteger, PrimeField};

use crate::error::{Result, SolidError};

// Field-element Poseidon API is off-BPF only (uses `light-poseidon`'s
// Fr-typed hasher).  BPF on-chain code paths only ever need the
// byte-oriented API.  This import is also the wasm32 byte-path's
// dispatch target — see `hash_bytes_dispatch` below.
#[cfg(not(target_os = "solana"))]
use light_poseidon::{Poseidon, PoseidonHasher};

// ─── Field Element ↔ Bytes Conversion (dual-target) ────────────────────────

/// BN254 scalar field modulus `p` as little-endian bytes.  Used by
/// [`is_canonical_bn254_le`] (SOLID-SEC-062 / H4) to reject
/// non-canonical 32-byte field-element encodings at trust boundaries.
///
/// `p = 21888242871839275222246405745257275088548364400416034343698204186575808495617`
///
/// Big-endian hex (sanity):
/// `0x30644E72E131A029B85045B68181585D2833E84879B9709143E1F593F0000001`.
///
/// BabyJubJub (used for issuer pubkey x/y) is defined over the BN254
/// scalar field, so the same modulus applies to BJJ coordinates and to
/// the Poseidon-output commitment.
pub const BN254_FR_MODULUS_LE: [u8; 32] = [
    0x01, 0x00, 0x00, 0xf0, 0x93, 0xf5, 0xe1, 0x43, 0x91, 0x70, 0xb9, 0x79, 0x48, 0xe8, 0x33, 0x28,
    0x5d, 0x58, 0x81, 0x81, 0xb6, 0x45, 0x50, 0xb8, 0x29, 0xa0, 0x31, 0xe1, 0x72, 0x4e, 0x64, 0x30,
];

/// Returns `true` iff `bytes` (interpreted as a little-endian 256-bit
/// integer) is strictly less than the BN254 scalar field modulus.
///
/// Use at every trust boundary that accepts a 32-byte field element
/// from an untrusted caller (commitment, BJJ pubkey x/y, etc.).
/// Without this gate, a malicious issuer can pick `c1, c2` with `c1 !=
/// c2` but `c1 mod p == c2 mod p`, producing two on-chain leaves the
/// circuit treats as one -- enabling nullifier-confusion attacks
/// against revocation accounting.  See SOLID-SEC-062 (H4 of the
/// 2026-04-29 synthesis audit).
///
/// CU cost on BPF: O(32) byte compares; <100 CU.
#[inline]
pub fn is_canonical_bn254_le(bytes: &[u8; 32]) -> bool {
    // Compare in MSB-first order: walk from byte 31 (most significant
    // in LE) down to byte 0.  First byte that differs decides.
    for i in (0..32).rev() {
        if bytes[i] < BN254_FR_MODULUS_LE[i] {
            return true;
        }
        if bytes[i] > BN254_FR_MODULUS_LE[i] {
            return false;
        }
    }
    // bytes == p exactly is NOT canonical (the canonical encoding of p
    // is 0).
    false
}

/// Convert an `Fr` field element to 32 bytes (little-endian).
///
/// This matches circomlib's internal representation.
pub fn fr_to_bytes_le(fr: &Fr) -> [u8; 32] {
    let bigint = fr.into_bigint();
    let le_vec = bigint.to_bytes_le();
    let mut out = [0u8; 32];
    let len = le_vec.len().min(32);
    out[..len].copy_from_slice(&le_vec[..len]);
    out
}

/// Convert 32 bytes (little-endian) to an `Fr` field element.
///
/// Automatically reduces modulo the BN254 scalar field prime.
pub fn bytes_le_to_fr(bytes: &[u8; 32]) -> Fr {
    Fr::from_le_bytes_mod_order(bytes)
}

/// Convert a `u64` to an `Fr` field element.
pub fn u64_to_fr(val: u64) -> Fr {
    Fr::from(val)
}

// ─── Byte-oriented Poseidon (dual-target) ──────────────────────────────────

/// Hash byte arrays (each 32 bytes, little-endian Fr representation) using
/// Poseidon-Bn254-x5.
///
/// Byte-identical to circomlib's `poseidon.circom` and to the historical
/// `light-poseidon 0.2.0` field-element path
/// (`Poseidon::<Fr>::new_circom(N).hash(&[Fr::from_le_bytes_mod_order(b)..])`
/// → `fr_to_bytes_le`).
///
/// ## Canonicalization
///
/// Inputs are reduced modulo the BN254 scalar field prime *before* dispatch.
/// This matches:
/// - circomlib's circuit semantics (Poseidon takes field elements; the
///   witness reduces inputs to `Fr` automatically), and
/// - the historical Rust path, which round-tripped each input through
///   `Fr::from_le_bytes_mod_order` → `fr_to_bytes_le`.
///
/// Without this step, `solana_program::poseidon::hashv` (host fallback +
/// BPF syscall, both backed by `light-poseidon`'s strict modulus check)
/// rejects any byte slice ≥ p with `InputLargerThanModulus`.  The most
/// common such inputs in this codebase are 32-byte verifier addresses and
/// verifier nonces (top byte often above 0x30, BN254's MSB).
///
/// On BPF the reduction is a single bigint comparison + subtraction inside
/// `ark-bn254`; the dominant cost is still the syscall itself.
pub fn hash_bytes(inputs: &[[u8; 32]]) -> Result<[u8; 32]> {
    if inputs.is_empty() || inputs.len() > 12 {
        return Err(SolidError::PoseidonHash(format!(
            "Poseidon supports 1-12 inputs, got {}",
            inputs.len()
        )));
    }

    let mut canonical: [[u8; 32]; 12] = [[0u8; 32]; 12];
    for (i, b) in inputs.iter().enumerate() {
        canonical[i] = fr_to_bytes_le(&bytes_le_to_fr(b));
    }
    let input_refs: heapless_refs::Refs = heapless_refs::collect(&canonical[..inputs.len()]);

    hash_bytes_dispatch(&input_refs)
}

/// Target-conditional Poseidon dispatch for the canonicalized byte path.
///
/// On BPF and non-wasm32 host this is `solana_program::poseidon::hashv`
/// (syscall on BPF, `light-poseidon`-backed fallback on host).  On
/// wasm32 this is `light_poseidon::Poseidon::<Fr>::new_circom().hash_bytes_le`
/// directly, because `solana-program 1.18.x` does not build for
/// `wasm32-unknown-unknown`.  All three paths produce byte-identical
/// outputs under `Bn254X5` / `LittleEndian` parameters by construction;
/// the `tests/vectors/` cross-language CI gate enforces this contract.
/// See module-level docs and `docs/E2E_BLOCKERS.md` B6 for the full
/// rationale.
#[cfg(not(target_arch = "wasm32"))]
fn hash_bytes_dispatch(input_refs: &heapless_refs::Refs<'_>) -> Result<[u8; 32]> {
    let h = solana_program::poseidon::hashv(
        solana_program::poseidon::Parameters::Bn254X5,
        solana_program::poseidon::Endianness::LittleEndian,
        input_refs.as_slice(),
    )
    .map_err(|e| SolidError::PoseidonHash(format!("Hash: {:?}", e)))?;

    Ok(h.to_bytes())
}

#[cfg(target_arch = "wasm32")]
fn hash_bytes_dispatch(input_refs: &heapless_refs::Refs<'_>) -> Result<[u8; 32]> {
    use light_poseidon::PoseidonBytesHasher;

    let slice = input_refs.as_slice();
    let mut hasher = Poseidon::<Fr>::new_circom(slice.len())
        .map_err(|e| SolidError::PoseidonHash(format!("Hasher init: {}", e)))?;
    let digest = hasher
        .hash_bytes_le(slice)
        .map_err(|e| SolidError::PoseidonHash(format!("Hash: {}", e)))?;

    if digest.len() != 32 {
        return Err(SolidError::PoseidonHash(format!(
            "Poseidon backend returned {} bytes, expected 32",
            digest.len()
        )));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    Ok(out)
}

/// Hash `u64` values and return the result as 32 bytes (little-endian).
///
/// Equivalent to converting each `u64` to its little-endian 32-byte `Fr`
/// representation and calling [`hash_bytes`].  `u64` values are strictly less
/// than the BN254 scalar field modulus, so the round-trip
/// `u64 → Fr → bytes_le → Fr` is the identity.
pub fn hash_fields_to_bytes(fields: &[u64]) -> Result<[u8; 32]> {
    let mut input_bytes: [[u8; 32]; 12] = [[0u8; 32]; 12];
    if fields.len() > 12 {
        return Err(SolidError::PoseidonHash(format!(
            "Poseidon supports 1-12 inputs, got {}",
            fields.len()
        )));
    }
    for (i, &v) in fields.iter().enumerate() {
        input_bytes[i] = fr_to_bytes_le(&Fr::from(v));
    }
    hash_bytes(&input_bytes[..fields.len()])
}

// ─── Tiny stack-only `&[&[u8]]` builder ────────────────────────────────────
//
// `solana_program::poseidon::hashv` takes `&[&[u8]]`.  We can't allocate a
// `Vec` in a hot BPF path (heap pressure + LTO inlining), so we materialise
// the slice-of-slices on a 12-wide fixed-size array.  Poseidon's circomlib
// arity caps at 12 inputs, so 12 is the upper bound.
mod heapless_refs {
    pub struct Refs<'a> {
        storage: [&'a [u8]; 12],
        len: usize,
    }

    impl<'a> Refs<'a> {
        pub fn as_slice(&self) -> &[&'a [u8]] {
            &self.storage[..self.len]
        }
    }

    pub fn collect<'a>(inputs: &'a [[u8; 32]]) -> Refs<'a> {
        // Caller-validated: inputs.len() ∈ [1, 12].
        let mut storage: [&'a [u8]; 12] = [&[]; 12];
        for (i, b) in inputs.iter().enumerate() {
            storage[i] = b.as_slice();
        }
        Refs {
            storage,
            len: inputs.len(),
        }
    }
}

// ─── Field-element Poseidon (host-only) ────────────────────────────────────

/// Hash an arbitrary number of `Fr` field elements using Poseidon.
///
/// Matches `circomlib/circuits/poseidon.circom` output exactly.
/// Supports 1–12 inputs (circomlib arity).
///
/// **Host-only.**  Uses `light-poseidon`'s `Fr`-typed hasher, which is not
/// BPF-safe under `lto = "fat"`.  BPF call sites should convert their inputs
/// to `[u8; 32]` and use [`hash_bytes`] instead.
#[cfg(not(target_os = "solana"))]
pub fn hash_fr(inputs: &[Fr]) -> Result<Fr> {
    if inputs.is_empty() || inputs.len() > 12 {
        return Err(SolidError::PoseidonHash(format!(
            "Poseidon supports 1-12 inputs, got {}",
            inputs.len()
        )));
    }

    let mut hasher = Poseidon::<Fr>::new_circom(inputs.len())
        .map_err(|e| SolidError::PoseidonHash(format!("Hasher init: {}", e)))?;

    hasher
        .hash(inputs)
        .map_err(|e| SolidError::PoseidonHash(format!("Hash: {}", e)))
}

/// Hash `u64` values using Poseidon (returning `Fr`).
///
/// Convenience wrapper that converts u64 → Fr before hashing.
/// Used for hashing attestation data fields off-chain.
///
/// **Host-only.**  See [`hash_fr`].
#[cfg(not(target_os = "solana"))]
pub fn hash_fields(fields: &[u64]) -> Result<Fr> {
    let fr_inputs: Vec<Fr> = fields.iter().map(|&f| Fr::from(f)).collect();
    hash_fr(&fr_inputs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_single_element() {
        // Poseidon(0) should produce a deterministic, non-zero output
        let result = hash_fields(&[0]).unwrap();
        assert_ne!(result, Fr::from(0));
    }

    #[test]
    fn test_hash_deterministic() {
        let a = hash_fields(&[1, 2, 3]).unwrap();
        let b = hash_fields(&[1, 2, 3]).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_hash_different_inputs_different_outputs() {
        let a = hash_fields(&[1, 2, 3]).unwrap();
        let b = hash_fields(&[1, 2, 4]).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn test_hash_order_matters() {
        let a = hash_fields(&[1, 2]).unwrap();
        let b = hash_fields(&[2, 1]).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn test_bytes_roundtrip() {
        let original = hash_fields(&[42, 99]).unwrap();
        let bytes = fr_to_bytes_le(&original);
        let recovered = bytes_le_to_fr(&bytes);
        assert_eq!(original, recovered);
    }

    #[test]
    fn test_hash_bytes_matches_hash_fields() {
        let fields = [10u64, 20u64, 30u64];
        let result_fields = hash_fields_to_bytes(&fields).unwrap();

        let fr_inputs: Vec<[u8; 32]> = fields
            .iter()
            .map(|&f| fr_to_bytes_le(&Fr::from(f)))
            .collect();
        let result_bytes = hash_bytes(&fr_inputs).unwrap();

        assert_eq!(result_fields, result_bytes);
    }

    #[test]
    fn test_reject_empty_input() {
        assert!(hash_fields(&[]).is_err());
        assert!(hash_bytes(&[]).is_err());
        assert!(hash_fields_to_bytes(&[]).is_err());
    }

    #[test]
    fn test_reject_too_many_inputs() {
        let inputs: Vec<u64> = (0..13).collect();
        assert!(hash_fields(&inputs).is_err());
        assert!(hash_fields_to_bytes(&inputs).is_err());
    }

    #[test]
    fn test_max_inputs() {
        let inputs: Vec<u64> = (0..12).collect();
        assert!(hash_fields(&inputs).is_ok());
        assert!(hash_fields_to_bytes(&inputs).is_ok());
    }

    /// Cross-check that the dual-target byte-oriented `hash_bytes` matches
    /// the host-only field-element `hash_fr` path bit-for-bit.  This is the
    /// in-crate version of the cross-language vectors gate; if it ever
    /// diverges, every credential commitment + nullifier preimage in the
    /// system silently breaks compatibility with circomlib.
    #[test]
    fn test_byte_path_matches_field_path() {
        let inputs_u64 = [11u64, 22, 33, 44, 55];

        // Field-element path (`light-poseidon` directly).
        let frs: Vec<Fr> = inputs_u64.iter().map(|&v| Fr::from(v)).collect();
        let via_fr = fr_to_bytes_le(&hash_fr(&frs).unwrap());

        // Byte path (syscall on BPF, light-poseidon-backed fallback on host).
        let bytes: Vec<[u8; 32]> = frs.iter().map(fr_to_bytes_le).collect();
        let via_bytes = hash_bytes(&bytes).unwrap();

        assert_eq!(
            via_fr, via_bytes,
            "Fr-typed Poseidon and byte-typed Poseidon must agree"
        );
    }

    /// Regression: `hash_bytes` must mod-reduce non-canonical inputs rather
    /// than reject them.  This pins compatibility with circomlib (the
    /// circuit takes inputs as field elements, inherently mod p) and with
    /// the historical pre-syscall path
    /// (`bytes_le_to_fr` → `hash_fr` → `fr_to_bytes_le`).
    ///
    /// Without canonicalization, `solana_program::poseidon::hashv` rejects
    /// any byte slice ≥ p with `InputLargerThanModulus`.  In practice we
    /// feed it 32-byte verifier addresses, verifier nonces, and arbitrary
    /// issuer-tree roots, all of which can have a top byte above 0x30
    /// (BN254's MSB).  See `nullifier::compute_nullifier`.
    #[test]
    fn test_hash_bytes_canonicalizes_oversized_inputs() {
        // 0x99... in little-endian is ≈ 0x99 in the most-significant byte,
        // which is well above BN254's modulus top byte (0x30).
        let oversized = [0x99u8; 32];
        let canonical = fr_to_bytes_le(&bytes_le_to_fr(&oversized));
        assert_ne!(
            oversized, canonical,
            "test setup invariant: 0x99-repeated must reduce non-trivially"
        );

        // Both shapes must hash to the same digest.
        let h1 = hash_bytes(&[oversized]).unwrap();
        let h2 = hash_bytes(&[canonical]).unwrap();
        assert_eq!(
            h1, h2,
            "hash_bytes must mod-reduce inputs ≥ p, not reject them"
        );

        // And it must agree bit-for-bit with the host field-element path.
        let h3 = fr_to_bytes_le(&hash_fr(&[bytes_le_to_fr(&oversized)]).unwrap());
        assert_eq!(h1, h3, "byte path with oversized input must equal Fr path");
    }

    // ─── BN254 canonicality (SOLID-SEC-062 / H4) ─────────────────────────

    #[test]
    fn is_canonical_zero_is_canonical() {
        assert!(is_canonical_bn254_le(&[0u8; 32]));
    }

    #[test]
    fn is_canonical_one_is_canonical() {
        let mut one = [0u8; 32];
        one[0] = 1;
        assert!(is_canonical_bn254_le(&one));
    }

    #[test]
    fn is_canonical_modulus_minus_one_is_canonical() {
        // p - 1 (LE) = (modulus - 1) — must accept.
        let mut p_minus_one = BN254_FR_MODULUS_LE;
        // LE: subtract 1 by decrementing byte 0 (no borrow because byte 0 = 0x01).
        p_minus_one[0] -= 1;
        assert!(is_canonical_bn254_le(&p_minus_one));
    }

    #[test]
    fn is_canonical_exact_modulus_is_rejected() {
        // p itself is NOT canonical (must be strictly less than p).
        assert!(!is_canonical_bn254_le(&BN254_FR_MODULUS_LE));
    }

    #[test]
    fn is_canonical_modulus_plus_one_is_rejected() {
        let mut p_plus_one = BN254_FR_MODULUS_LE;
        // LE add 1 (byte 0 = 0x01 -> 0x02; no overflow into byte 1).
        p_plus_one[0] += 1;
        assert!(!is_canonical_bn254_le(&p_plus_one));
    }

    #[test]
    fn is_canonical_max_field_value_is_rejected() {
        // 0xFF * 32 == 2^256 - 1; well above p.
        assert!(!is_canonical_bn254_le(&[0xFFu8; 32]));
    }

    #[test]
    fn is_canonical_high_byte_above_top_is_rejected() {
        // Top byte of p is 0x30; setting byte 31 to 0x31 with rest zero
        // gives a value that is > p iff the high byte alone exceeds p's
        // high byte.
        let mut v = [0u8; 32];
        v[31] = 0x31;
        assert!(!is_canonical_bn254_le(&v));
    }

    #[test]
    fn is_canonical_high_byte_below_top_is_canonical() {
        let mut v = [0u8; 32];
        v[31] = 0x2F;
        assert!(is_canonical_bn254_le(&v));
    }

    #[test]
    fn is_canonical_high_bytes_match_low_bytes_below_p_is_canonical() {
        // Build a value whose top 16 bytes match `p`'s top 16 bytes but
        // whose bottom 16 bytes are zero.  Strictly less than `p`, so
        // canonical.  Catches an off-by-one in the MSB-first walk that
        // returns false too early on byte equality.
        let mut v = [0u8; 32];
        for i in 16..32 {
            v[i] = BN254_FR_MODULUS_LE[i];
        }
        // Top 16 bytes equal p; bottom 16 are zero whereas p's bottom
        // 16 starts with 0x01 -- so v < p strictly.
        assert!(is_canonical_bn254_le(&v));
    }

    #[test]
    fn is_canonical_modulus_le_constant_round_trips_through_fr() {
        // The on-chain const must agree with arkworks' notion of p.
        // bytes_le_to_fr reduces `p` to 0; round-tripping via
        // fr_to_bytes_le yields the canonical encoding of 0.
        let fr_zero = bytes_le_to_fr(&BN254_FR_MODULUS_LE);
        let bytes = fr_to_bytes_le(&fr_zero);
        assert_eq!(bytes, [0u8; 32]);
    }
}
