/**
 * SOLID-SEC-050 regression gate.
 *
 * Drives the isolated template at
 *   circuits/test/templates/schema_ordering_isolated.circom
 * to verify the schemaHashes canonicality contract:
 *
 *   1. Strict-ascending order on active credentials (positive case).
 *   2. Padding-at-end: once `schemaHashes[i] == 0`, every following
 *      slot MUST also be 0 (SEC-050 fix).
 *   3. Active-active pair ordering: descending or equal pairs reject.
 *   4. Zero-schema integrity: `data[i] != 0` when `schemaHashes[i] == 0`
 *      rejects (forced-zero sentinel from STEP 0 of the batch circuit).
 *
 * Pre-fix (SEC-050), interleavings like `[A1, 0, A2, A3]` and
 * `[0, A1, 0, A2]` were silently admitted, letting a prover emit
 * multiple valid encodings of the same credential set with distinct
 * `queryCredentialIndices` -> distinct `queryContextHash` -> distinct
 * nullifiers, bypassing the verifier's per-claim rate-limit.  Post-fix,
 * the canonical form `[A1 < A2 < ... < AK, 0, 0, ..., 0]` is the only
 * valid layout.
 *
 * Run: `cd circuits && PATH="$PWD/../.toolchain/bin:$PATH" npm test`.
 */

const path = require('path');
const wasm_tester = require('circom_tester').wasm;

const TEMPLATE_PATH = path.join(
  __dirname, 'templates', 'schema_ordering_isolated.circom',
);
const NUM_CREDS = 4;

async function mustSucceed(circuit, input, tag) {
  const w = await circuit.calculateWitness(input, true);
  await circuit.checkConstraints(w);
}

async function mustReject(circuit, input, tag) {
  try {
    await circuit.calculateWitness(input, true);
  } catch (_) {
    return;
  }
  throw new Error(`witness unexpectedly succeeded: ${tag}`);
}

function mkInput(schemas, datas) {
  // Convenience: data defaults to zeros if not supplied.
  const d = datas !== undefined ? datas : Array(NUM_CREDS).fill(0);
  return { schemaHashes: schemas, data: d };
}

describe('SOLID-SEC-050: schemaHashes canonicality (padding-at-end + ascending)', function () {
  this.timeout(120_000);
  let circuit;

  before(async () => {
    circuit = await wasm_tester(TEMPLATE_PATH, {
      include: [path.join(__dirname, '..', 'node_modules')],
    });
  });

  // ─── Positive: canonical layouts ────────────────────────────────────────

  it('accepts fully-active strict-ascending [5, 10, 15, 20]', async () => {
    await mustSucceed(circuit, mkInput([5, 10, 15, 20], [1, 2, 3, 4]), 'fully active');
  });

  it('accepts active prefix + trailing padding [5, 10, 0, 0]', async () => {
    await mustSucceed(circuit, mkInput([5, 10, 0, 0], [1, 2, 0, 0]), 'two active');
  });

  it('accepts single active slot [5, 0, 0, 0]', async () => {
    await mustSucceed(circuit, mkInput([5, 0, 0, 0], [1, 0, 0, 0]), 'single active');
  });

  it('accepts all-padding [0, 0, 0, 0]', async () => {
    await mustSucceed(circuit, mkInput([0, 0, 0, 0]), 'all padding');
  });

  it('accepts large schema-hash values (BN254 near-modulus)', async () => {
    // Pre Phase 3.4 fix would reject these because of Num2Bits(252)
    // overflow in the old LessThan(252).  LessThanBN254 handles them.
    const NEAR_MAX = '21888242871839275222246405745257275088548364400416034343698204186575808495616'; // p-1
    const HALF = '10944121435919637611123202872628637544274182200208017171849102093287904247808';
    await mustSucceed(circuit, mkInput([HALF, NEAR_MAX, 0, 0], [0, 0, 0, 0]), 'near-modulus active pair');
  });

  // ─── Negative: SEC-050 padding-canonicality breaks ──────────────────────

  it('SEC-050: rejects [A1, 0, A2, 0] (active-after-padding)', async () => {
    await mustReject(circuit, mkInput([5, 0, 10, 0], [1, 0, 2, 0]), 'A1, 0, A2, 0');
  });

  it('SEC-050: rejects [A1, 0, A2, A3]', async () => {
    await mustReject(circuit, mkInput([5, 0, 10, 15], [1, 0, 2, 3]), 'A1, 0, A2, A3');
  });

  it('SEC-050: rejects [0, A1, 0, A2]', async () => {
    await mustReject(circuit, mkInput([0, 5, 0, 10], [0, 1, 0, 2]), '0, A1, 0, A2');
  });

  it('SEC-050: rejects [0, 0, A1, 0]', async () => {
    await mustReject(circuit, mkInput([0, 0, 5, 0], [0, 0, 1, 0]), '0, 0, A1, 0');
  });

  it('SEC-050: rejects [0, A1, A2, A3]', async () => {
    await mustReject(circuit, mkInput([0, 5, 10, 15], [0, 1, 2, 3]), '0 prefix');
  });

  it('SEC-050: rejects [A1, A2, 0, A3] (gap in middle)', async () => {
    await mustReject(circuit, mkInput([5, 10, 0, 15], [1, 2, 0, 3]), 'gap in middle');
  });

  // ─── Negative: strict-ascending breaks ──────────────────────────────────

  it('rejects active descending pair [10, 5, 0, 0]', async () => {
    await mustReject(circuit, mkInput([10, 5, 0, 0], [1, 2, 0, 0]), 'descending');
  });

  it('rejects active equal pair [5, 5, 0, 0]', async () => {
    await mustReject(circuit, mkInput([5, 5, 0, 0], [1, 2, 0, 0]), 'equal');
  });

  it('rejects fully-active descending [20, 15, 10, 5]', async () => {
    await mustReject(circuit, mkInput([20, 15, 10, 5], [1, 2, 3, 4]), 'all descending');
  });

  // ─── Negative: zero-schema integrity ────────────────────────────────────

  it('rejects non-zero data on a padding slot [5, 0, 0, 0] data=[1, 1, 0, 0]', async () => {
    await mustReject(
      circuit,
      mkInput([5, 0, 0, 0], [1, 1, 0, 0]),
      'data[1]=1 with schema[1]=0',
    );
  });

  it('rejects non-zero data on an all-padding slot [0, 0, 0, 0] data=[0, 0, 1, 0]', async () => {
    await mustReject(
      circuit,
      mkInput([0, 0, 0, 0], [0, 0, 1, 0]),
      'data[2]=1 with all schemas zero',
    );
  });

  // ─── Combined: only canonical layouts pass ──────────────────────────────

  it('property: every K-of-NUM_CREDS active set has exactly one canonical layout', async () => {
    // Enumerate K=2 -> exactly one valid placement (positions 0, 1).
    // Permute over all 6 placements of 2 active slots in 4 positions;
    // the canonical placement must accept; the other 5 must reject.
    // schemas chosen to be ascending where active.
    const placements = [
      [1, 1, 0, 0], // canonical
      [1, 0, 1, 0],
      [1, 0, 0, 1],
      [0, 1, 1, 0],
      [0, 1, 0, 1],
      [0, 0, 1, 1],
    ];
    for (const mask of placements) {
      const schemas = [];
      const datas = [];
      let nextActive = 5;
      let nextData = 1;
      for (let i = 0; i < NUM_CREDS; i++) {
        if (mask[i] === 1) {
          schemas.push(nextActive);
          datas.push(nextData);
          nextActive += 5;
          nextData += 1;
        } else {
          schemas.push(0);
          datas.push(0);
        }
      }
      const isCanonical = mask[0] === 1 && mask[1] === 1 && mask[2] === 0 && mask[3] === 0;
      if (isCanonical) {
        await mustSucceed(circuit, mkInput(schemas, datas), `canonical ${mask}`);
      } else {
        await mustReject(circuit, mkInput(schemas, datas), `non-canonical ${mask}`);
      }
    }
  });
});
