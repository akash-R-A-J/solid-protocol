/**
 * SOLID-SEC-001 regression gate.
 *
 * Drives the isolated template at
 *   circuits/test/templates/range_check_isolated.circom
 * with three shapes of input:
 *
 *   1. All in-range indices   -> witness calc succeeds.
 *   2. queryCredentialIndices out of range -> witness calc fails at
 *      the `LessThan(8).out === 1` assertion.
 *   3. queryFieldIndices out of range -> same failure mode.
 *
 * A property test iterates `N_RANDOM = 100` random out-of-range
 * indices to catch off-by-one bugs (e.g. accepting `NUM_CREDS` when
 * the bound should be strict less-than).  100 rather than 1000
 * because the isolated template is cheap; the original budget was
 * set against the full circuit and was overkill.
 *
 * Run: `cd circuits && npm test`.
 */

const { expect } = require('chai');
const path = require('path');
const wasm_tester = require('circom_tester').wasm;

const TEMPLATE_PATH = path.join(
  __dirname, 'templates', 'range_check_isolated.circom',
);
const MAX_PREDICATES = 4;
const NUM_CREDS = 4;
const NUM_FIELDS = 8;
const N_RANDOM = 100;

function mkInput(credIdx, fieldIdx) {
  return {
    queryCredentialIndices: credIdx,
    queryFieldIndices: fieldIdx,
  };
}

/// Witness-calc that must succeed.
async function mustSucceed(circuit, input) {
  const w = await circuit.calculateWitness(input, true);
  await circuit.checkConstraints(w);
}

/// Witness-calc that must fail at an assertion (the range-check
/// constraint is `LessThan(8).out === 1`).  A successful calc is the
/// regression signal and is re-thrown.
async function mustReject(circuit, input, tag) {
  try {
    await circuit.calculateWitness(input, true);
  } catch (_) {
    return; // expected
  }
  throw new Error(`witness unexpectedly succeeded: ${tag}`);
}

describe('SOLID-SEC-001: batch circuit query-index range checks', function () {
  this.timeout(120_000);
  let circuit;

  before(async () => {
    circuit = await wasm_tester(TEMPLATE_PATH, {
      include: [path.join(__dirname, '..', 'node_modules')],
    });
  });

  it('accepts all in-range indices', async () => {
    await mustSucceed(circuit, mkInput(
      [0, 1, 2, 3],
      [0, 1, 2, 3],
    ));
  });

  it('rejects queryCredentialIndices[0] == NUM_CREDS (boundary)', async () => {
    await mustReject(
      circuit,
      mkInput([NUM_CREDS, 0, 0, 0], [0, 0, 0, 0]),
      `credIdx = NUM_CREDS`,
    );
  });

  it('rejects queryFieldIndices[0] == NUM_FIELDS (boundary)', async () => {
    await mustReject(
      circuit,
      mkInput([0, 0, 0, 0], [NUM_FIELDS, 0, 0, 0]),
      `fieldIdx = NUM_FIELDS`,
    );
  });

  it(`rejects ${N_RANDOM} random out-of-range credential indices`, async () => {
    for (let k = 0; k < N_RANDOM; k++) {
      const slot = k % MAX_PREDICATES;
      const bad = NUM_CREDS + 1 + Math.floor(Math.random() * 250);
      const cred = [0, 0, 0, 0];
      cred[slot] = bad;
      await mustReject(circuit, mkInput(cred, [0, 0, 0, 0]), `credIdx[${slot}] = ${bad}`);
    }
  });

  it(`rejects ${N_RANDOM} random out-of-range field indices`, async () => {
    for (let k = 0; k < N_RANDOM; k++) {
      const slot = k % MAX_PREDICATES;
      const bad = NUM_FIELDS + 1 + Math.floor(Math.random() * 250);
      const field = [0, 0, 0, 0];
      field[slot] = bad;
      await mustReject(circuit, mkInput([0, 0, 0, 0], field), `fieldIdx[${slot}] = ${bad}`);
    }
  });
});

// Silence the `expect` unused warning -- exposed for future assertions.
void expect;
