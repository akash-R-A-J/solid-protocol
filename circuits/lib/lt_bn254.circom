pragma circom 2.1.0;

include "../node_modules/circomlib/circuits/bitify.circom";

/// LessThanBN254
///
/// Strictly compares two BN254 field elements `in[0]` and `in[1]`
/// (each canonically reduced to [0, p)) and outputs 1 iff in[0] < in[1]
/// when both are interpreted as non-negative integers.
///
/// Motivation
/// ----------
/// `circomlib::LessThan(n)` carries `assert(n <= 252)` because it
/// expresses the comparison via `Num2Bits(n+1)` on the difference
/// `in[0] + 2^n - in[1]`.  For full-domain Fp inputs (e.g. Poseidon
/// outputs, which span the full BN254 scalar field where p ~= 2^253.85),
/// the diff can exceed 2^253 and the underlying `Num2Bits` either
/// overflows the constraint domain or, worse, admits multiple bit
/// decompositions modulo p (Num2Bits is non-strict for N >= 254).
///
/// Both failure modes were observed during the v0.6 e2e bring-up:
/// the schema-hash ordering check at `batch_credential_query.circom`
/// rejected ~26% of valid (master, schema) combinations.
///
/// Construction
/// ------------
/// 1. Strict-decompose each input into 254 canonical bits via
///    `Num2Bits_strict()` (which attaches an `AliasCheck`, enforcing
///    in < p; alternative bit decompositions are rejected).
/// 2. Walk MSB -> LSB tracking two scalars:
///       eq[k] = 1 iff a and b agree on the k highest bits scanned.
///       lt[k] = 1 iff at the first index where they differ within
///               those k bits, a had 0 and b had 1.
///    Recurrence per bit i (i = 253 .. 0):
///       eq_bit  = 1 - (a_i XOR b_i)         // == a_i*b_i + (1-a_i)(1-b_i)
///               = 1 - a_i - b_i + 2 a_i b_i
///       lt_bit  = (1 - a_i) * b_i           // == 1 only when a_i=0, b_i=1
///               = b_i - a_i b_i
///       eq[k+1] = eq[k] * eq_bit
///       lt[k+1] = lt[k] + eq[k] * lt_bit
/// 3. `out` is `lt[254]`.  By construction, once two bits differ the
///    eq factor goes to 0 and the lt accumulator freezes, so the
///    final value is determined by the first differing bit.
///
/// Cost
/// ----
/// Two `Num2Bits_strict()` (~510 constraints each) + ~5 constraints
/// per scanned bit * 254 bits ~= 2270.  ~3300 constraints total per
/// invocation.  Acceptable for the (NUM_CREDS-1 = 3) ordering checks
/// in batch_credential_query.
///
/// Hard invariant
/// --------------
/// Both `in[0]` and `in[1]` MUST already lie in [0, p) -- i.e. they
/// must be the canonical Fp representation, not an arbitrary 256-bit
/// integer.  Num2Bits_strict's AliasCheck enforces this internally,
/// so any over-large input throws inside this template rather than
/// silently aliasing.
template LessThanBN254() {
    signal input in[2];
    signal output out;

    component aBits = Num2Bits_strict();
    component bBits = Num2Bits_strict();
    aBits.in <== in[0];
    bBits.in <== in[1];

    signal ab[254];
    signal eqBit[254];
    signal ltBit[254];
    signal eq[255];
    signal lt[255];
    eq[0] <== 1;
    lt[0] <== 0;

    for (var k = 0; k < 254; k++) {
        var idx = 253 - k;
        ab[k]    <== aBits.out[idx] * bBits.out[idx];
        eqBit[k] <== 1 - aBits.out[idx] - bBits.out[idx] + 2 * ab[k];
        ltBit[k] <== bBits.out[idx] - ab[k];
        eq[k+1]  <== eq[k] * eqBit[k];
        lt[k+1]  <== lt[k] + eq[k] * ltBit[k];
    }

    out <== lt[254];
}
