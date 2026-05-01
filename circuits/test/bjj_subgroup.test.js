/**
 * SOLID-SEC-048 Option B regression: `BjjSubgroupCheck` enforces
 * prime-order subgroup membership for a candidate BabyJubJub pubkey.
 *
 * Drives the isolated template at
 *   circuits/test/templates/bjj_subgroup_isolated.circom
 * which exposes `BjjSubgroupCheck`, the registration-time gate that
 * `programs/issuer-registry/src/lib.rs::register_issuer` will verify
 * on-chain via Groth16.
 *
 * Coverage:
 *   1. Honest subgroup point (Base8): MUST accept.
 *   2. Honest random subgroup point ([k]*Base8 for arbitrary k): MUST accept.
 *   3. Identity (0, 1): MUST reject (caught by the non-identity gate).
 *   4. Order-2 point (0, p-1): MUST reject (NOT in prime-order subgroup).
 *   5. Off-curve point (1, 1): MUST reject (caught by BabyCheck).
 *   6. Cofactor-tainted point (Base8 + order-2): MUST reject ([r]*P != identity).
 *
 * The negative tests assert that the witness-generation FAILS (i.e.,
 * `calculateWitness` throws) -- if the circuit is buggy and accepts
 * a non-subgroup point, it produces a satisfying witness and the
 * test fails.
 *
 * Run: `cd circuits && npm test`.
 */

const { expect } = require('chai');
const path = require('path');
const wasm_tester = require('circom_tester').wasm;
const { buildBabyjub } = require('circomlibjs');

const TEMPLATE_PATH = path.join(
  __dirname, 'templates', 'bjj_subgroup_isolated.circom',
);

// BN254 base field prime (BJJ embedding field).
const Q = 21888242871839275222246405745257275088548364400416034343698204186575808495617n;

describe('SOLID-SEC-048 Option B: BJJ prime-order subgroup gate', function () {
  this.timeout(300_000);

  let circuit;
  let bjj;
  let F;
  let Base8;

  before(async () => {
    circuit = await wasm_tester(TEMPLATE_PATH, {
      include: [path.join(__dirname, '..', 'node_modules')],
    });
    bjj = await buildBabyjub();
    F = bjj.F;
    Base8 = bjj.Base8;
    // Sanity: Base8 must be the canonical generator the rest of the
    // protocol uses.  These coordinates are pinned in
    // `circuits/test/babypbk254.test.js` and the Rust SDK.
    expect(F.toObject(Base8[0]).toString()).to.equal(
      '5299619240641551281634865583518297030282874472190772894086521144482721001553',
    );
    expect(F.toObject(Base8[1]).toString()).to.equal(
      '16950150798460657717958625567821834550301663161624707787222815936182638968203',
    );
  });

  // ─── Positive cases: prime-order subgroup points MUST accept ─────────

  it('accepts Base8 (canonical generator of the subgroup)', async () => {
    const Ax = F.toObject(Base8[0]);
    const Ay = F.toObject(Base8[1]);
    const w = await circuit.calculateWitness({ Ax, Ay }, true);
    await circuit.checkConstraints(w);
  });

  it('accepts a random [k]*Base8 for k in [1, 2^32)', async () => {
    // Three deterministic but arbitrary scalars to exercise the full
    // double-and-add chain (high-bit-set, low-bit-set, mixed).
    const scalars = [
      1n,                 // smallest non-trivial
      (1n << 32n) - 1n,   // 32-bit all-ones
      0xdeadbeefn,        // mixed pattern
    ];
    for (const k of scalars) {
      const P = bjj.mulPointEscalar(Base8, k);
      const Ax = F.toObject(P[0]);
      const Ay = F.toObject(P[1]);
      const w = await circuit.calculateWitness({ Ax, Ay }, true);
      await circuit.checkConstraints(w);
    }
  });

  // ─── Negative cases: non-subgroup points MUST reject ─────────────────

  /// Helper: assert that `calculateWitness` rejects.  circom_tester
  /// surfaces constraint failures as exceptions; we don't care about
  /// the exact message, only that it threw.
  async function expectRejection(inputs, label) {
    let threw = false;
    try {
      await circuit.calculateWitness(inputs, true);
    } catch (e) {
      threw = true;
    }
    expect(threw, `${label} MUST be rejected by BjjSubgroupCheck`).to.equal(true);
  }

  it('rejects identity (0, 1) via the non-identity gate', async () => {
    await expectRejection({ Ax: 0n, Ay: 1n }, 'identity (0, 1)');
  });

  it('rejects order-2 point (0, p-1)', async () => {
    // (0, -1 mod p) is on the curve: A*0 + (p-1)^2 = (p-1)^2 ≡ 1 (mod p).
    // [2]*(0, p-1) = (0, 1) = identity, so order is exactly 2.
    // Not in the prime-order subgroup; [r]*P != identity.
    await expectRejection({ Ax: 0n, Ay: Q - 1n }, 'order-2 point (0, p-1)');
  });

  it('rejects off-curve point (1, 1) via BabyCheck', async () => {
    // Curve eq: 168700*1 + 1 = 168701; rhs = 1 + 168696*1 = 168697.
    // 168701 != 168697 (mod p); off-curve.
    await expectRejection({ Ax: 1n, Ay: 1n }, 'off-curve (1, 1)');
  });

  it('rejects a cofactor-tainted point (Base8 + order-2)', async () => {
    // Base8 is in the prime-order subgroup; (0, p-1) is order 2.
    // Their sum is on the curve but has order 2*r (full subgroup x torsion),
    // so [r]*sum = (0, p-1) != identity.  Must reject.
    const order2 = [F.e(0n), F.e(Q - 1n)];
    const tainted = bjj.addPoint(Base8, order2);
    const Ax = F.toObject(tainted[0]);
    const Ay = F.toObject(tainted[1]);
    // Sanity: the tainted sum should still be on the curve.
    const xx = (Ax * Ax) % Q;
    const yy = (Ay * Ay) % Q;
    const lhs = (168700n * xx + yy) % Q;
    const rhs = (1n + 168696n * xx * yy) % Q;
    expect(lhs).to.equal(rhs, 'tainted point should still be on-curve');
    await expectRejection({ Ax, Ay }, 'cofactor-tainted (Base8 + (0,p-1))');
  });
});
