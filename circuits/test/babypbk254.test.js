/**
 * Phase 3.4 regression: BabyPbk254 derives the same BabyJubJub
 * public key as the off-chain SDK.
 *
 * Drives the isolated template at
 *   circuits/test/templates/babypbk254_isolated.circom
 * which exposes `BabyPbk254`, the 254-bit-safe replacement that
 * Phase 3.4 introduced for `circomlib::BabyPbk()` inside
 * `IdentityAnchor`.  The original used `Num2Bits(253)` which
 * rejected ~50% of valid Poseidon-derived credPriv scalars (the
 * ones with bit 253 set); `BabyPbk254` uses `Num2Bits_strict()` +
 * `EscalarMulFix(254, BASE8)` to faithfully cover the full
 * canonical Fp range.
 *
 * Coverage:
 *   1. Ground-truth fixture vectors (15 cases) emitted by the Rust
 *      generator at `crates/solid-core/examples/gen_circuit_vectors.rs`,
 *      including 6 bit-253-set regression cases.  Each vector pins
 *      both `(pk_x, pk_y)` produced by Rust against the witness
 *      output, byte-identical -- this is the SDK<->circuit
 *      cross-language contract.
 *   2. The identity case: sk = 1 -> pk == Base8.
 *   3. Output-on-curve sanity: every produced (Ax, Ay) satisfies
 *      circomlib's twisted Edwards equation
 *      168700 * Ax^2 + Ay^2 == 1 + 168696 * Ax^2 * Ay^2 (mod p).
 *      This is what `babyjubjub.rs::is_on_curve` validates Rust-
 *      side and what we re-prove at every byte boundary in the
 *      coordinate-mismatch fix from Phase 3.4 / b1d.
 *
 * Run: `cd circuits && npm test`.
 */

const { expect } = require('chai');
const path = require('path');
const fs = require('fs');
const wasm_tester = require('circom_tester').wasm;

const TEMPLATE_PATH = path.join(
  __dirname, 'templates', 'babypbk254_isolated.circom',
);
const FIXTURE_PATH = path.join(
  __dirname, 'fixtures', 'circuit_vectors.json',
);

// BN254 base field prime (same modulus as the BabyJubJub
// embedding field).  All arithmetic in the on-curve check happens
// in Fq (= Fr_BN254) per circomlib convention.
const Q = 21888242871839275222246405745257275088548364400416034343698204186575808495617n;
const BJJ_A = 168700n;
const BJJ_D = 168696n;

function loadFixture() {
  return JSON.parse(fs.readFileSync(FIXTURE_PATH, 'utf8'));
}

function modQ(x) {
  let r = x % Q;
  if (r < 0n) r += Q;
  return r;
}

/// circomlib BJJ on-curve test: A*x^2 + y^2 == 1 + D*x^2*y^2.
function onCurveCircomlib(x, y) {
  const xx = (x * x) % Q;
  const yy = (y * y) % Q;
  const lhs = modQ(BJJ_A * xx + yy);
  const rhs = modQ(1n + BJJ_D * xx * yy);
  return lhs === rhs;
}

describe('Phase 3.4: BabyPbk254 SDK<->circuit pubkey derivation', function () {
  this.timeout(180_000);
  let circuit;
  let fixture;

  before(async () => {
    fixture = loadFixture();
    circuit = await wasm_tester(TEMPLATE_PATH, {
      include: [path.join(__dirname, '..', 'node_modules')],
    });
    // Sanity: fixture must declare circomlib-form Base8.  This is
    // the literal anchor that the b1d coordinate-mismatch fix
    // converged on and that the Rust SDK now emits.
    expect(fixture.bjj_base8).to.be.an('object');
    expect(fixture.bjj_base8.x_dec).to.equal(
      '5299619240641551281634865583518297030282874472190772894086521144482721001553',
    );
    expect(fixture.bjj_base8.y_dec).to.equal(
      '16950150798460657717958625567821834550301663161624707787222815936182638968203',
    );
  });

  it('matches Rust ground truth on all fixture vectors', async () => {
    expect(fixture.baby_pbk254.length).to.be.greaterThan(0);
    for (const v of fixture.baby_pbk254) {
      const w = await circuit.calculateWitness(
        { in: BigInt(v.sk_dec) }, true,
      );
      await circuit.checkConstraints(w);
      await circuit.assertOut(w, {
        Ax: BigInt(v.pk_x_dec),
        Ay: BigInt(v.pk_y_dec),
      });
    }
  });

  it('exercises bit-253-set regression vectors', async () => {
    const regs = fixture.baby_pbk254.filter(v => v.bit253_set);
    expect(regs.length).to.be.greaterThan(
      0, 'fixture must include bit-253-set regression vectors',
    );
    for (const v of regs) {
      const w = await circuit.calculateWitness(
        { in: BigInt(v.sk_dec) }, true,
      );
      await circuit.checkConstraints(w);
      await circuit.assertOut(w, {
        Ax: BigInt(v.pk_x_dec),
        Ay: BigInt(v.pk_y_dec),
      });
    }
  });

  it('sk = 1 -> Base8 (identity-like anchor)', async () => {
    const w = await circuit.calculateWitness({ in: 1n }, true);
    await circuit.checkConstraints(w);
    await circuit.assertOut(w, {
      Ax: BigInt(fixture.bjj_base8.x_dec),
      Ay: BigInt(fixture.bjj_base8.y_dec),
    });
  });

  it('every fixture (Ax, Ay) lies on the BabyJubJub curve', () => {
    for (const v of fixture.baby_pbk254) {
      const x = BigInt(v.pk_x_dec);
      const y = BigInt(v.pk_y_dec);
      expect(onCurveCircomlib(x, y)).to.equal(
        true, `off-curve: ${v.note}`,
      );
    }
  });
});

void expect;
