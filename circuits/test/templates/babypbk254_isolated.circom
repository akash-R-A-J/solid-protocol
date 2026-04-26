pragma circom 2.1.0;

include "../../lib/identity_anchor.circom";

/// Phase 3.4 regression gate -- isolated.
///
/// Drives `BabyPbk254` directly.  `BabyPbk254` is the 254-bit-safe
/// replacement for `circomlib::BabyPbk()` that this protocol uses
/// inside `IdentityAnchor`: it strict-decomposes its 254-bit input
/// via `Num2Bits_strict()` (with `AliasCheck` enforcing in < p) and
/// then sweeps all 254 bits through `EscalarMulFix(254, BASE8)`.
///
/// The off-chain Rust counterpart in
/// `crates/solid-core/src/babyjubjub.rs::derive_public_key` produces
/// byte-identical outputs (BabyJubJub Base8 has order r ~= 2^251, so
/// reducing the scalar mod r is implicit; both sides end up at the
/// same Edwards point).  Equality of (Ax, Ay) across SDK and circuit
/// is the ground-truth contract this template lets the mocha harness
/// assert against.
template BabyPbk254Isolated() {
    signal input  in;
    signal output Ax;
    signal output Ay;

    component bjj = BabyPbk254();
    bjj.in <== in;
    Ax <== bjj.Ax;
    Ay <== bjj.Ay;
}

component main = BabyPbk254Isolated();
