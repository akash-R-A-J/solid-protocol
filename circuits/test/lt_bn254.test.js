/**
 * Phase 3.4 regression: 254-bit-safe LessThan over the BN254 scalar
 * field.
 *
 * Drives the isolated template at
 *   circuits/test/templates/lt_bn254_isolated.circom
 * which exposes `LessThanBN254`, the comparator that replaced
 * `circomlib::LessThan(252)` inside `batch_credential_query.circom`
 * STEP-3.5b after Phase 3.4 surfaced a 26% rejection rate on valid
 * Poseidon-derived schema hashes.
 *
 * Coverage:
 *   1. Deterministic ground-truth vectors (15 cases) emitted by the
 *      Rust generator at `crates/solid-core/examples/gen_circuit_vectors.rs`
 *      via `circuits/test/fixtures/circuit_vectors.json`.  Includes
 *      9 bit-253-set regression vectors.
 *   2. A property test driving 64 random pairs over the full BN254
 *      scalar field; expected `lt` is computed locally from the
 *      JS-side BigInt comparison.
 *   3. Edge cases: 0/0, p-1/p-1, 0/p-1, p-1/0.
 *   4. Canonical reduction: in[0] = p reduces to 0 (the circuit's
 *      `Num2Bits_strict()` carries an `AliasCheck` that constrains
 *      any prover-supplied bit decomposition to in < p; the JS
 *      witness calculator always picks the canonical reduction, so
 *      the surface contract here is "value mod p == observed input"
 *      and the AliasCheck protects soundness against a malicious
 *      prover -- separately exercised by snarkjs verification in CI).
 *
 * Run: `cd circuits && npm test`.
 */

const { expect } = require('chai');
const path = require('path');
const fs = require('fs');
const wasm_tester = require('circom_tester').wasm;

const TEMPLATE_PATH = path.join(
  __dirname, 'templates', 'lt_bn254_isolated.circom',
);
const FIXTURE_PATH = path.join(
  __dirname, 'fixtures', 'circuit_vectors.json',
);

// BN254 scalar field prime.
const P = 21888242871839275222246405745257275088548364400416034343698204186575808495617n;
const N_RANDOM = 64;

function loadFixture() {
  const raw = fs.readFileSync(FIXTURE_PATH, 'utf8');
  return JSON.parse(raw);
}

/// Witness-calc that must succeed and produce the expected `out`.
async function checkLt(circuit, a, b, expected, _tag) {
  const witness = await circuit.calculateWitness({ a, b }, true);
  await circuit.checkConstraints(witness);
  await circuit.assertOut(witness, { out: expected });
}

/// Random BigInt in [0, P).
function randFr() {
  // 256 random bits, reduced mod P.  Uses Math.random because
  // determinism isn't needed here -- test is a property check.
  let r = 0n;
  for (let i = 0; i < 8; i++) {
    r = (r << 32n) | BigInt(Math.floor(Math.random() * 0x1_0000_0000));
  }
  return r % P;
}

describe('Phase 3.4: LessThanBN254 254-bit-safe Fp comparator', function () {
  this.timeout(180_000);
  let circuit;
  let fixture;

  before(async () => {
    fixture = loadFixture();
    circuit = await wasm_tester(TEMPLATE_PATH, {
      include: [path.join(__dirname, '..', 'node_modules')],
    });
  });

  it('matches Rust ground truth on all fixture vectors', async () => {
    expect(fixture.lt_bn254.length).to.be.greaterThan(0);
    for (const v of fixture.lt_bn254) {
      await checkLt(
        circuit,
        BigInt(v.a_dec),
        BigInt(v.b_dec),
        BigInt(v.expected_lt),
        v.note,
      );
    }
  });

  it('exercises bit-253-set regression cases (the Phase 3.4 trigger)', async () => {
    const regs = fixture.lt_bn254.filter(
      v => v.a_bit253_set || v.b_bit253_set,
    );
    expect(regs.length).to.be.greaterThan(
      0, 'fixture must include bit-253-set regression vectors',
    );
    for (const v of regs) {
      await checkLt(
        circuit,
        BigInt(v.a_dec),
        BigInt(v.b_dec),
        BigInt(v.expected_lt),
        `bit253: ${v.note}`,
      );
    }
  });

  it('matches JS-side BigInt comparison on random Fp pairs', async () => {
    for (let k = 0; k < N_RANDOM; k++) {
      const a = randFr();
      const b = randFr();
      const expected = a < b ? 1n : 0n;
      await checkLt(circuit, a, b, expected, `random[${k}]`);
    }
  });

  it('handles edge cases: 0/0, (p-1)/(p-1), 0/(p-1), (p-1)/0', async () => {
    const zero = 0n;
    const top = P - 1n;
    await checkLt(circuit, zero, zero, 0n, '0 < 0');
    await checkLt(circuit, top, top, 0n, '(p-1) < (p-1)');
    await checkLt(circuit, zero, top, 1n, '0 < (p-1)');
    await checkLt(circuit, top, zero, 0n, '(p-1) < 0');
  });

  it('canonicalizes in[0] = p to 0 (AliasCheck contract)', async () => {
    // calculateWitness reduces the BigInt input mod p before any
    // bit decomposition, so passing literal P is observationally
    // identical to passing 0.  This anchors that contract; the
    // separate AliasCheck inside Num2Bits_strict guards against a
    // malicious prover supplying non-canonical bits, which is
    // verified end-to-end by snarkjs proof verification (see CI).
    await checkLt(circuit, P, 0n, 0n, 'P input -> reduces to 0');
    await checkLt(circuit, 0n, P, 0n, 'P input on b -> reduces to 0');
  });

  // Large but in-range: p-1 and p-2 ensure the highest comparable
  // bits are 1 in both inputs without crossing the alias bound.
  it('handles near-modulus near-equal pairs', async () => {
    await checkLt(circuit, P - 2n, P - 1n, 1n, 'p-2 < p-1');
    await checkLt(circuit, P - 1n, P - 2n, 0n, 'p-1 not < p-2');
  });
});

void expect;
