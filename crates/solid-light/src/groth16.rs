//! Groth16 verification primitives over BN254 (alt_bn128).
//!
//! These were previously private to `programs/zk-verifier/src/lib.rs`.
//! SEC-048 Phase E.1 (2026-05-XX) lifted them here so a second on-chain
//! consumer (`programs/issuer-registry`'s `register_issuer` subgroup
//! verify) can share the same syscall-driven path without duplicating
//! ~150 lines of cryptographic code across programs (L6: doc lies and
//! "two copies of the same hot crypto path" are tomorrow's silent
//! divergence).
//!
//! The module is parameterised over `const N: usize` -- the public-input
//! count -- so a single instantiation serves both the 32-input batch
//! circuit (zk-verifier) and the 2-input subgroup circuit
//! (issuer-registry, ADR-0015 / SEC-048 Option B).
//!
//! ## Surface
//!
//! - [`VkBuf`] -- on-chain VK byte buffer parser, IC table on the heap
//!   to keep the BPF stack-resident shell well under 1 KB.
//! - [`negate_g1_point`] -- subtract Y from the BN254 base-field prime
//!   for groth16-solana's `-A` representation.
//! - [`verify_groth16_proof`] -- isolated, never-inlined wrapper that
//!   parses the VK, negates `proof_a`, builds a `Groth16Verifier<N>`,
//!   and runs `verifier.verify()`.

use core::marker::PhantomData;
use groth16_solana::groth16::{Groth16Verifier, Groth16Verifyingkey};

/// Parser errors returned by [`VkBuf::parse`].  Each variant is
/// observable by tests; callers typically map to their program-local
/// `ErrorCode` for on-wire reporting.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum VkParseError {
    /// Buffer shorter than the fixed-size header (4 + 64 + 128*3 bytes).
    TooShort,
    /// Header parsed but the IC region is truncated.
    TruncatedIc,
    /// `nr_ic` does not equal the expected `MAX_IC = N + 1` for this
    /// circuit.  M5 / SOLID-SEC-069: strict equality only.
    IcOverflow,
}

/// Error returned by [`verify_groth16_proof`].  Callers typically map
/// to their program-local `ErrorCode` to preserve on-wire error
/// discriminants.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Groth16VerifyError {
    /// VK byte buffer malformed, IC count wrong, proof bytes malformed,
    /// or `Groth16Verifier::new` rejected the proof shape.
    InvalidProofFormat,
    /// Pairing check evaluated but did not yield identity -- the proof
    /// is well-formed but does not satisfy the relation.
    ProofVerificationFailed,
}

/// Parsed Groth16 verification key, parameterised over the circuit's
/// public-input count `N`.
///
/// Expected byte layout of the stored VK (little-endian counts,
/// big-endian curve points):
/// ```text
///   u32         nr_ic        -- number of IC points (= N + 1)
///   [u8; 64]    alpha_g1
///   [u8; 128]   beta_g2
///   [u8; 128]   gamma_g2
///   [u8; 128]   delta_g2
///   [u8; 64]    ic[0..nr_ic]
/// ```
///
/// Heap residency for the IC table is intentional: a fixed
/// `[[u8; 64]; MAX_IC]` field made the inlined
/// `__global::verify_batch_proof` BPF frame overflow the 4 KB
/// per-frame stack budget by ~456 bytes (the parent frame already
/// holds Anchor's ~1 312-byte deserialized argument struct).  `Vec`
/// keeps the borrow lifetime tied to `self`, so [`VkBuf::as_verifying_key`]
/// is safe with no `Box::leak`.
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub struct VkBuf<const N: usize> {
    nr_ic: usize,
    alpha: [u8; 64],
    beta: [u8; 128],
    gamma: [u8; 128],
    delta: [u8; 128],
    ic: Vec<[u8; 64]>,
    _phantom: PhantomData<[(); N]>,
}

impl<const N: usize> VkBuf<N> {
    /// Number of IC points the VK must declare for a circuit with
    /// `N` public inputs.  Equal to `N + 1` by Groth16 construction.
    pub const MAX_IC: usize = N + 1;

    /// Parse the on-chain VK byte buffer into a `VkBuf` whose IC table
    /// lives on the heap.  The fixed-size scalars
    /// (alpha/beta/gamma/delta) stay inline in the returned struct.
    ///
    /// Bounds-checked at every cursor advance; malformed input returns
    /// a typed error rather than panicking.  `nr_ic` MUST equal
    /// `MAX_IC` (= `N + 1`) exactly.  Pre-fix the parser tolerated any
    /// `0 < nr_ic <= MAX_IC`; that's a soundness gap because a VK with
    /// fewer IC points than the circuit declares would silently accept
    /// a proof against the wrong public-input contract.  See M5 /
    /// SOLID-SEC-069 (closed 2026-05-01).
    pub fn parse(bytes: &[u8]) -> core::result::Result<Self, VkParseError> {
        const HEADER: usize = 4 + 64 + 128 * 3;
        if bytes.len() < HEADER {
            return Err(VkParseError::TooShort);
        }

        let nr_ic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        // M5 / SOLID-SEC-069: strict equality against MAX_IC = N + 1.
        if nr_ic != Self::MAX_IC {
            return Err(VkParseError::IcOverflow);
        }
        if bytes.len() < HEADER + nr_ic * 64 {
            return Err(VkParseError::TruncatedIc);
        }

        let mut alpha = [0u8; 64];
        let mut beta = [0u8; 128];
        let mut gamma = [0u8; 128];
        let mut delta = [0u8; 128];

        let mut cursor = 4;
        alpha.copy_from_slice(&bytes[cursor..cursor + 64]);
        cursor += 64;
        beta.copy_from_slice(&bytes[cursor..cursor + 128]);
        cursor += 128;
        gamma.copy_from_slice(&bytes[cursor..cursor + 128]);
        cursor += 128;
        delta.copy_from_slice(&bytes[cursor..cursor + 128]);
        cursor += 128;

        // Allocate the IC table directly on the heap with exact capacity.
        // No intermediate `[[u8; 64]; MAX_IC]` ever lives on the stack.
        let mut ic: Vec<[u8; 64]> = Vec::with_capacity(nr_ic);
        for _ in 0..nr_ic {
            let mut slot = [0u8; 64];
            slot.copy_from_slice(&bytes[cursor..cursor + 64]);
            cursor += 64;
            ic.push(slot);
        }

        Ok(VkBuf {
            nr_ic,
            alpha,
            beta,
            gamma,
            delta,
            ic,
            _phantom: PhantomData,
        })
    }

    /// Produce a `Groth16Verifyingkey` that borrows from this buffer.
    /// The returned view is valid for the lifetime of `self`.
    pub fn as_verifying_key(&self) -> Groth16Verifyingkey<'_> {
        Groth16Verifyingkey {
            // IC count = nr_pubinputs + 1.  Saturating for the edge case
            // where a caller hands us a 1-element IC; the verifier will
            // reject later.
            nr_pubinputs: self.nr_ic.saturating_sub(1),
            vk_alpha_g1: self.alpha,
            vk_beta_g2: self.beta,
            vk_gamme_g2: self.gamma,
            vk_delta_g2: self.delta,
            vk_ic: &self.ic[..self.nr_ic],
        }
    }
}

// Compile-time assertion: the stack-resident `VkBuf<N>` shell must stay
// well under Solana's 4 KB per-frame BPF stack budget.  N is in a
// PhantomData so it does not contribute to layout; one assertion at the
// most common N value is sufficient (the IC table lives on the heap).
//
//   VkBuf = 8 (nr_ic) + 64 (alpha) + 128 (beta) + 128 (gamma)
//         + 128 (delta) + 24 (Vec) + 0 (PhantomData) ~= 480 bytes.
//   The 1 KB ceiling leaves the rest of the BPF 4 KB per-frame budget
//   for Anchor's deserialized argument struct (~1 312 bytes) plus the
//   verifier's other locals.  See SOLID-SEC-047 for the regression
//   history (the previous `[[u8; 64]; MAX_IC]` field pushed
//   `__global::verify_batch_proof` past 4 KB by ~456 bytes).
const _: () = {
    let sz_32 = core::mem::size_of::<VkBuf<32>>();
    assert!(sz_32 < 1024, "VkBuf<32> shell exceeds 1 KB stack budget");
    let sz_2 = core::mem::size_of::<VkBuf<2>>();
    assert!(sz_2 < 1024, "VkBuf<2> shell exceeds 1 KB stack budget");
};

/// Run Groth16 verification in an isolated, never-inlined stack frame.
///
/// All heavy locals -- the parsed `VkBuf<N>` shell, the by-value
/// `Groth16Verifyingkey` view, `proof_a_neg`, and the
/// `Groth16Verifier<N>` state -- live here and disappear at function
/// exit.  Because this helper is `#[inline(never)]`, LTO=fat cannot
/// fold it into Anchor's `__global::verify_*` wrapper, so its frame
/// doesn't merge with the wrapper's deserialized-argument frame.  This
/// is the structural fix for the BPF 4 KB per-frame stack overflow
/// described in SOLID-SEC-047.
///
/// Inputs are taken by reference to avoid duplicating the (already
/// argument-deserialized) payload across stack frames.  Heap
/// allocations: one `Vec<[u8; 64]>` of length `nr_ic` inside `VkBuf`,
/// dropped at exit.  No `Box::leak`; no static state.
#[inline(never)]
pub fn verify_groth16_proof<const N: usize>(
    vk_storage_data: &[u8],
    proof_a: &[u8; 64],
    proof_b: &[u8; 128],
    proof_c: &[u8; 64],
    public_inputs: &[[u8; 32]; N],
) -> core::result::Result<(), Groth16VerifyError> {
    let vk_buf =
        VkBuf::<N>::parse(vk_storage_data).map_err(|_| Groth16VerifyError::InvalidProofFormat)?;
    let vk = vk_buf.as_verifying_key();
    let proof_a_neg =
        negate_g1_point(proof_a).map_err(|_| Groth16VerifyError::InvalidProofFormat)?;

    let mut verifier =
        Groth16Verifier::<N>::new(&proof_a_neg, proof_b, proof_c, public_inputs, &vk)
            .map_err(|_| Groth16VerifyError::InvalidProofFormat)?;

    verifier
        .verify()
        .map_err(|_| Groth16VerifyError::ProofVerificationFailed)?;
    Ok(())
}

/// Negate the Y coordinate of a G1 point for groth16-solana's expected
/// `-A` representation.  Assumes 32-byte big-endian X || Y.  Returns
/// `Err(())` only if the byte-level subtraction underflows (i.e. the
/// supplied Y is greater than the BN254 base-field prime, which is
/// outside the canonical range).  Callers map to their program-local
/// `ErrorCode::InvalidProofFormat`.
#[allow(clippy::result_unit_err)]
pub fn negate_g1_point(point: &[u8; 64]) -> core::result::Result<[u8; 64], ()> {
    // BN254 base-field prime (alt_bn128), big-endian.
    const P_BE: [u8; 32] = [
        0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58,
        0x5d, 0x97, 0x81, 0x6a, 0x91, 0x68, 0x71, 0xca, 0x8d, 0x3c, 0x20, 0x8c, 0x16, 0xd8, 0x7c,
        0xfd, 0x47,
    ];

    // Out = (X || P - Y), computed as big-endian subtraction.
    let mut out = [0u8; 64];
    out[..32].copy_from_slice(&point[..32]); // X unchanged.

    let mut borrow: i16 = 0;
    for i in (0..32).rev() {
        let p = P_BE[i] as i16;
        let y = point[32 + i] as i16;
        let mut diff = p - y - borrow;
        if diff < 0 {
            diff += 256;
            borrow = 1;
        } else {
            borrow = 0;
        }
        out[32 + i] = diff as u8;
    }
    if borrow != 0 {
        return Err(());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Synthesize a plausibly-shaped VK byte buffer with `nr_ic` IC
    // points.  The values are not cryptographically meaningful -- the
    // test is about the parser, not pairing correctness.
    fn synth_vk_bytes(nr_ic: usize) -> Vec<u8> {
        let mut v = Vec::with_capacity(4 + 64 + 128 * 3 + nr_ic * 64);
        v.extend_from_slice(&(nr_ic as u32).to_le_bytes());
        v.extend_from_slice(&[1u8; 64]); // alpha
        v.extend_from_slice(&[2u8; 128]); // beta
        v.extend_from_slice(&[3u8; 128]); // gamma
        v.extend_from_slice(&[4u8; 128]); // delta
        for i in 0..nr_ic {
            v.extend_from_slice(&[i as u8; 64]);
        }
        v
    }

    // Canonical batch VK: 32 public inputs -> 33 IC points.
    type BatchVk = VkBuf<32>;
    const BATCH_MAX_IC: usize = BatchVk::MAX_IC;

    // Subgroup VK: 2 public inputs -> 3 IC points (SEC-048 Option B).
    type SubgroupVk = VkBuf<2>;
    const SUBGROUP_MAX_IC: usize = SubgroupVk::MAX_IC;

    #[test]
    fn vk_parse_round_trips_max_ic_batch() {
        let bytes = synth_vk_bytes(BATCH_MAX_IC);
        let buf = BatchVk::parse(&bytes).expect("parse");
        assert_eq!(buf.nr_ic, BATCH_MAX_IC);
        assert_eq!(buf.alpha, [1u8; 64]);
        assert_eq!(buf.beta, [2u8; 128]);
        assert_eq!(buf.gamma, [3u8; 128]);
        assert_eq!(buf.delta, [4u8; 128]);
        assert_eq!(buf.ic[0], [0u8; 64]);
        assert_eq!(buf.ic[BATCH_MAX_IC - 1], [(BATCH_MAX_IC - 1) as u8; 64]);
    }

    #[test]
    fn vk_parse_round_trips_max_ic_subgroup() {
        // SEC-048 Phase E: 2 public inputs (Ax, Ay) -> 3 IC points.
        let bytes = synth_vk_bytes(SUBGROUP_MAX_IC);
        let buf = SubgroupVk::parse(&bytes).expect("parse");
        assert_eq!(buf.nr_ic, SUBGROUP_MAX_IC);
        assert_eq!(SUBGROUP_MAX_IC, 3);
    }

    #[test]
    fn vk_parse_rejects_too_short_header() {
        let bytes = vec![0u8; 4 + 64 + 128 * 3 - 1];
        assert_eq!(BatchVk::parse(&bytes), Err(VkParseError::TooShort));
        assert_eq!(SubgroupVk::parse(&bytes), Err(VkParseError::TooShort));
    }

    #[test]
    fn vk_parse_rejects_truncated_ic_region() {
        // M5 / SOLID-SEC-069: synth a bytes buffer with the canonical
        // nr_ic = MAX_IC but truncate the IC region.
        let mut bytes = synth_vk_bytes(BATCH_MAX_IC);
        bytes.truncate(bytes.len() - 10);
        assert_eq!(BatchVk::parse(&bytes), Err(VkParseError::TruncatedIc));
    }

    #[test]
    fn vk_parse_rejects_zero_ic() {
        let bytes = synth_vk_bytes(0);
        // Header alone is valid size; nr_ic != MAX_IC must be rejected.
        assert_eq!(BatchVk::parse(&bytes), Err(VkParseError::IcOverflow));
    }

    #[test]
    fn vk_parse_rejects_ic_overflow() {
        // Claim more IC points than the canonical count.  Post M5 /
        // SOLID-SEC-069: any nr_ic != MAX_IC is rejected.
        let too_many = BATCH_MAX_IC + 1;
        let mut bytes = Vec::with_capacity(4 + 64 + 128 * 3 + too_many * 64);
        bytes.extend_from_slice(&(too_many as u32).to_le_bytes());
        bytes.extend_from_slice(&[0u8; 64 + 128 * 3 + 64]); // partial payload
        assert_eq!(BatchVk::parse(&bytes), Err(VkParseError::IcOverflow));
    }

    #[test]
    fn vk_parse_rejects_nr_ic_below_max() {
        // M5 / SOLID-SEC-069 regression gate: nr_ic less than MAX_IC
        // must be rejected even though the prior parser accepted it.
        // Pre-fix, a VK with `nr_ic = MAX_IC - 1` parsed cleanly and
        // the `Groth16Verifyingkey` view returned `nr_pubinputs =
        // MAX_IC - 2`, silently accepting proofs against the wrong
        // public-input contract.
        for too_few in &[1usize, 2, BATCH_MAX_IC - 1] {
            let bytes = synth_vk_bytes(*too_few);
            assert_eq!(
                BatchVk::parse(&bytes),
                Err(VkParseError::IcOverflow),
                "nr_ic = {} must be rejected (only MAX_IC = {} is accepted)",
                too_few,
                BATCH_MAX_IC,
            );
        }
    }

    #[test]
    fn vk_parse_subgroup_rejects_batch_size_ic() {
        // SEC-048 Phase E.1 cross-instantiation gate: a subgroup-VK
        // parser MUST reject a buffer carrying a 33-IC batch VK, and
        // vice versa.  Closes the cross-circuit replay surface where
        // two on-chain consumers share VkBuf-shaped storage but expect
        // different N.
        let batch_bytes = synth_vk_bytes(BATCH_MAX_IC);
        assert_eq!(
            SubgroupVk::parse(&batch_bytes),
            Err(VkParseError::IcOverflow),
        );
        let subgroup_bytes = synth_vk_bytes(SUBGROUP_MAX_IC);
        assert_eq!(
            BatchVk::parse(&subgroup_bytes),
            Err(VkParseError::IcOverflow),
        );
    }

    #[test]
    fn vk_view_borrows_from_buffer() {
        let bytes = synth_vk_bytes(BATCH_MAX_IC);
        let buf = BatchVk::parse(&bytes).expect("parse");
        let vk = buf.as_verifying_key();
        assert_eq!(vk.nr_pubinputs, BATCH_MAX_IC - 1);
        assert_eq!(vk.vk_ic.len(), BATCH_MAX_IC);
        assert_eq!(vk.vk_alpha_g1, [1u8; 64]);
    }

    #[test]
    fn vk_buf_fits_in_stack_budget_runtime() {
        // Mirrors the const assertion at the top of the file; runtime
        // form gives a readable failure message.  After moving the IC
        // table to the heap, the stack-resident shell is ~480 bytes.
        assert!(core::mem::size_of::<BatchVk>() < 1024);
        assert!(core::mem::size_of::<SubgroupVk>() < 1024);
    }

    // ─── negate_g1_point ──────────────────────────────────────────────

    #[test]
    fn negate_g1_zero_y_is_field_prime() {
        // Y = 0 should negate to Y = P (mod P) = 0 too, but our byte-
        // level routine writes P - 0 = P, which is accepted because
        // groth16-solana canonicalizes before pairing.  The key
        // invariant: no panic, correct bitwidth.
        let mut point = [0u8; 64];
        point[0] = 0x12; // X
        let out = negate_g1_point(&point).expect("negate");
        assert_eq!(&out[..32], &point[..32], "X must be unchanged");
    }

    #[test]
    fn negate_g1_is_involutive_modulo_p() {
        // Pick a Y strictly less than P so P - (P - Y) = Y.
        let mut point = [0u8; 64];
        point[0] = 0xAB;
        for (i, b) in point[32..].iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(7);
        }
        let once = negate_g1_point(&point).expect("negate once");
        let twice = negate_g1_point(&once).expect("negate twice");
        assert_eq!(point, twice, "double negation is identity");
    }

    #[test]
    fn negate_g1_known_answer_y_one() {
        // X = 0, Y = 1 (BE).  P - 1 (BE) is BN254_FQ - 1.
        // BN254_FQ in hex (BE):
        //   0x30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47
        // P - 1 = ...d46 (last byte: 0x47 - 0x01 = 0x46).
        let mut point = [0u8; 64];
        point[63] = 0x01; // Y = 1 in big-endian
        let out = negate_g1_point(&point).expect("negate");
        // X must be unchanged.
        assert_eq!(&out[..32], &[0u8; 32]);
        // Y must be P - 1 (BE).  Last byte of P is 0x47 -> P-1 ends 0x46.
        assert_eq!(out[63], 0x46);
        // High byte of Y must be 0x30 (matches BN254_FQ first byte).
        assert_eq!(out[32], 0x30);
    }

    #[test]
    fn negate_g1_zero_zero_no_panic() {
        // The (0, 0) "infinity sentinel" must not panic.  groth16-solana
        // canonicalises before use; whatever bytes we produce, the
        // pairing layer must accept or reject without us crashing.
        let point = [0u8; 64];
        let out = negate_g1_point(&point).expect("negate (0,0)");
        // X is unchanged.
        assert_eq!(&out[..32], &[0u8; 32]);
        // Y becomes P - 0 = P.
        assert_eq!(out[32], 0x30);
        assert_eq!(out[63], 0x47);
    }
}
