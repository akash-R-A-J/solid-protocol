/**
 * Phase 3.4 regression: end-to-end IdentityAnchor derivation.
 *
 * Drives the isolated template at
 *   circuits/test/templates/identity_anchor_isolated.circom
 * which wires `IdentityAnchor` with the global-tree Merkle
 * inclusion proof disabled (`enabled = 0`), so the harness can
 * exercise the full keypath
 *   credentialPrivKey = Poseidon(masterIdentityKey, schemaHash)
 *   credentialPubKey  = BabyPbk254(credentialPrivKey)
 * without standing up a 20-deep tree witness for every test.
 *
 * The on-chain leaf check is exercised separately via the
 * cross_language_vectors gate; here the contract is "Rust SDK
 * `derive_public_key(Poseidon(master, schema))` matches the circuit
 * witness output for the same inputs, byte-identical, including
 * the bit-253-set credPriv regime that broke the original BabyPbk".
 *
 * Coverage:
 *   1. Plain (low entropy) deterministic vectors.
 *   2. Bit-253-set regression vectors (the Phase 3.4 trigger).
 *   3. Cross-validate that cred_priv emitted by Rust equals
 *      Poseidon(master, schema) computed by circomlibjs (sanity
 *      check that the two Poseidon implementations agree on the
 *      same field).
 *   4. Output-on-curve assertion (BabyJubJub circomlib form).
 *
 * Run: `cd circuits && npm test`.
 */

const { expect } = require('chai');
const path = require('path');
const fs = require('fs');
const wasm_tester = require('circom_tester').wasm;
const { buildPoseidon } = require('circomlibjs');

const TEMPLATE_PATH = path.join(
  __dirname, 'templates', 'identity_anchor_isolated.circom',
);
const FIXTURE_PATH = path.join(
  __dirname, 'fixtures', 'circuit_vectors.json',
);

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

function onCurveCircomlib(x, y) {
  const xx = (x * x) % Q;
  const yy = (y * y) % Q;
  const lhs = modQ(BJJ_A * xx + yy);
  const rhs = modQ(1n + BJJ_D * xx * yy);
  return lhs === rhs;
}

async function checkAnchor(circuit, v) {
  const w = await circuit.calculateWitness({
    masterIdentityKey: BigInt(v.master_dec),
    schemaHash:        BigInt(v.schema_dec),
    revocationNonce:   0n,
  }, true);
  await circuit.checkConstraints(w);
  await circuit.assertOut(w, {
    credentialPubKeyAx: BigInt(v.pk_x_dec),
    credentialPubKeyAy: BigInt(v.pk_y_dec),
  });
}

describe('Phase 3.4: IdentityAnchor (master, schema) -> credentialPubKey', function () {
  this.timeout(180_000);
  let circuit;
  let fixture;
  let poseidon;
  let F;

  before(async () => {
    fixture = loadFixture();
    circuit = await wasm_tester(TEMPLATE_PATH, {
      include: [path.join(__dirname, '..', 'node_modules')],
    });
    // circomlibjs's poseidon -- we use it as an *independent*
    // reference implementation, distinct from both the circuit and
    // the Rust generator, so all three converging on the same
    // credPriv is a triple-witness that the SDK<->circuit Poseidon
    // contract is sound.
    poseidon = await buildPoseidon();
    F = poseidon.F;
  });

  it('matches Rust ground truth on all fixture vectors', async () => {
    expect(fixture.identity_anchor.length).to.be.greaterThan(0);
    for (const v of fixture.identity_anchor) {
      await checkAnchor(circuit, v);
    }
  });

  it('exercises bit-253-set regression vectors', async () => {
    const regs = fixture.identity_anchor.filter(v => v.bit253_set);
    expect(regs.length).to.be.greaterThan(
      0, 'fixture must include bit-253-set regression vectors',
    );
    for (const v of regs) {
      await checkAnchor(circuit, v);
    }
  });

  it('Rust cred_priv matches circomlibjs Poseidon(master, schema)', () => {
    for (const v of fixture.identity_anchor) {
      const m = F.e(BigInt(v.master_dec));
      const s = F.e(BigInt(v.schema_dec));
      const h = poseidon([m, s]);
      const got = F.toString(h);
      expect(got).to.equal(
        v.cred_priv_dec,
        `Poseidon mismatch on ${v.note}`,
      );
    }
  });

  it('every emitted credentialPubKey lies on the BabyJubJub curve', () => {
    for (const v of fixture.identity_anchor) {
      const x = BigInt(v.pk_x_dec);
      const y = BigInt(v.pk_y_dec);
      expect(onCurveCircomlib(x, y)).to.equal(
        true, `off-curve: ${v.note}`,
      );
    }
  });
});

void expect;
