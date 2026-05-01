pragma circom 2.1.0;

// SOLID-SEC-048 Option B (registration-time subgroup proof).
//
// Templates that prove a candidate BabyJubJub point P=(Ax, Ay) lies in
// the prime-order subgroup of the BJJ twisted Edwards curve.  The
// invariant enforced is the standard EC group test:
//
//     on_curve(P)
//     P != identity                    (where identity = (0, 1))
//     [r] * P == identity              (r = BJJ subgroup prime order)
//
// `r` is constant for the protocol:
//   r = 2736030358979909402780800718157159386076813972158567259200215660948447373041
// and the BJJ curve has order n = 8 * r (cofactor 8).  An honest issuer
// pubkey (in the prime-order subgroup) satisfies [r]*P = identity by
// Lagrange's theorem.  A torsion-tainted point with a non-trivial
// 2/4/8-component fails this test.
//
// Spec correction (2026-05-01): the earlier "[8]*P == P" framing was
// wrong; cofactor multiplication does NOT leave subgroup points fixed.
// See `sec/SECURITY_REGISTRY.md::SOLID-SEC-048` Real Fix Candidates
// section for the corrected spec.
//
// The constant-scalar multiplication is implemented as a vanilla
// double-and-add over the LSB-first bit decomposition of `r` using
// circomlib's `BabyAdd` + `BabyDbl` primitives DIRECTLY -- NOT
// `EscalarMulAny`, whose comments say it assumes the input point is
// already in the subgroup (a false negative for our test class).
//
// Cost (predicted, to be measured post-compile):
//   ~250 BabyDbl                     ~1,750 R1CS
//   115 BabyAdd  (Hamming weight)    ~690 R1CS
//   1   BabyCheck (curve eq)         ~3-5 R1CS
//   1   non-identity gate            ~3 R1CS
//   ----------------------------------------
//   Total                            ~2,500 R1CS

include "../node_modules/circomlib/circuits/babyjub.circom";
include "../node_modules/circomlib/circuits/comparators.circom";

// MulPointBy_R: computes [r] * P for the BJJ subgroup prime order r.
// Does NOT assume P is in the subgroup -- works for any curve point.
template MulPointBy_R() {
    signal input  px;
    signal input  py;
    signal output ox;
    signal output oy;

    // r in LSB-first 251-bit form (Hamming weight 115).  Generated from:
    //   r = 2736030358979909402780800718157159386076813972158567259200215660948447373041
    //   bits = [(r >> i) & 1 for i in range(251)]
    // Regenerable via:
    //   python3 -c "r = 2736030358979909402780800718157159386076813972158567259200215660948447373041; \
    //               print(','.join(str((r>>i)&1) for i in range(251)))"
    var R_BITS[251] = [
        1, 0, 0, 0, 1, 1, 1, 1, 0, 1, 1, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1, 1, 1, 0, 0,
        0, 0, 1, 1, 1, 0, 1, 1, 1, 1, 1, 0, 1, 0, 0, 1, 0, 1, 0, 0, 1, 1, 1, 0, 1, 1, 1, 0, 0, 1, 1, 0,
        0, 1, 0, 1, 0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1, 1, 1, 0, 0,
        0, 0, 0, 1, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 1, 1, 0, 0, 1, 1, 0, 1, 0, 1, 0, 1,
        1, 1, 0, 1, 0, 0, 0, 0, 1, 1, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 1, 0, 1, 1,
        0, 1, 1, 0, 1, 1, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1, 0, 0,
        1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 1, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 0, 0, 1, 1, 1, 0, 1, 0,
        0, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0, 1, 1
    ];

    var N = 251;

    // cur[i]  = 2^i * P              (cur[0] = P; cur[i+1] = 2 * cur[i])
    // acc[i]  = sum_{j<i, R_BITS[j]=1} 2^j * P    (acc[0] = identity; acc[N] = [r] * P)
    signal cur_x[N];
    signal cur_y[N];
    signal acc_x[N + 1];
    signal acc_y[N + 1];

    cur_x[0] <== px;
    cur_y[0] <== py;
    acc_x[0] <== 0;
    acc_y[0] <== 1;  // Edwards identity

    // We instantiate one BabyDbl per step (i in [0, N-1)) for the cur
    // ladder and one BabyAdd per step where R_BITS[i] == 1 for the
    // accumulator update.  Steps where R_BITS[i] == 0 just thread acc
    // unchanged -- saves ~6 constraints per zero bit (vs the dynamic
    // Mux1-based version).
    component dbls[N - 1];
    component adds[N];   // sparse: only allocated where R_BITS[i] == 1

    for (var i = 0; i < N; i++) {
        if (R_BITS[i] == 1) {
            adds[i] = BabyAdd();
            adds[i].x1 <== acc_x[i];
            adds[i].y1 <== acc_y[i];
            adds[i].x2 <== cur_x[i];
            adds[i].y2 <== cur_y[i];
            acc_x[i + 1] <== adds[i].xout;
            acc_y[i + 1] <== adds[i].yout;
        } else {
            acc_x[i + 1] <== acc_x[i];
            acc_y[i + 1] <== acc_y[i];
        }

        if (i < N - 1) {
            dbls[i] = BabyDbl();
            dbls[i].x <== cur_x[i];
            dbls[i].y <== cur_y[i];
            cur_x[i + 1] <== dbls[i].xout;
            cur_y[i + 1] <== dbls[i].yout;
        }
    }

    ox <== acc_x[N];
    oy <== acc_y[N];
}

// BjjSubgroupCheck: full check that P=(Ax, Ay) is in the prime-order
// subgroup of BabyJubJub.  Three constraints (composed):
//
//   1. P is on the BJJ twisted Edwards curve (BabyCheck()).
//   2. P != identity = (0, 1).
//   3. [r] * P == identity = (0, 1).
//
// Together these are equivalent to "P has order dividing r" AND "P
// has order > 1", which is exactly "P is a non-identity element of
// the prime-order subgroup".  Sufficient to make subgroup membership
// a circuit-level invariant.
template BjjSubgroupCheck() {
    signal input Ax;
    signal input Ay;

    // (1) Curve equation: 168700*Ax^2 + Ay^2 == 1 + 168696*Ax^2*Ay^2 (mod p).
    component curveCheck = BabyCheck();
    curveCheck.x <== Ax;
    curveCheck.y <== Ay;

    // (2) Non-identity: identity = (0, 1) is the only point with
    //     Ax = 0 AND Ay = 1.  Reject if both hold.
    //     Compute s = Ax^2 + (Ay - 1)^2; assert s != 0.
    signal axSq;
    axSq <== Ax * Ax;
    signal aySub1;
    aySub1 <== Ay - 1;
    signal aySub1Sq;
    aySub1Sq <== aySub1 * aySub1;
    signal sumSq;
    sumSq <== axSq + aySub1Sq;
    component sumSqIsZero = IsZero();
    sumSqIsZero.in <== sumSq;
    sumSqIsZero.out === 0;  // assert sumSq != 0 (i.e., P != identity)

    // (3) [r] * P == identity = (0, 1).
    //     Uses our vanilla double-and-add (no subgroup assumption).
    component mul = MulPointBy_R();
    mul.px <== Ax;
    mul.py <== Ay;
    // Assert [r]*P = (0, 1).
    mul.ox === 0;
    mul.oy === 1;
}
