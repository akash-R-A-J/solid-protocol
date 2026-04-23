/**
 * SOLID-SEC-029 regression gate.
 *
 * Drives the isolated template at
 *   circuits/test/templates/anchor_enabled_isolated.circom
 * to verify the two shapes that matter:
 *
 *   (a) `enabled = 1`  -> the `MerkleInclusion` inside `IdentityAnchor`
 *       insists that the supplied siblings recompute to `globalRoot`.
 *       With bogus siblings the witness calc fails.
 *
 *   (b) `enabled = 0`  -> `MerkleInclusion` does NOT enforce root
 *       equality (it short-circuits to true).  Padding slots can
 *       therefore pass arbitrary siblings without producing a valid
 *       inclusion proof.  This is what makes a partial batch
 *       (1, 2, or 3 active credentials out of NUM_CREDS = 4)
 *       physically realisable.
 *
 * Pre-SEC-029, `enabled` was hardcoded to 1 inside `IdentityAnchor`
 * and shape (b) was impossible.  The fix adds the `enabled` input
 * wire so the parent batch circuit can pass `1 - isZero[i].out`.
 *
 * Run: `cd circuits && npm test`.
 */

const path = require('path');
const wasm_tester = require('circom_tester').wasm;

async function mustSucceed(circuit, input) {
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

const TEMPLATE_PATH = path.join(
  __dirname, 'templates', 'anchor_enabled_isolated.circom',
);
const GLOBAL_DEPTH = 4;

function zeroSiblings(depth) {
  return Array.from({ length: depth }, () => '0');
}

function zeroPath(depth) {
  return Array.from({ length: depth }, () => 0);
}

function baseInput() {
  return {
    enabled: 1,
    masterIdentityKey: '1',  // any non-zero; the BJJ derivation handles it
    revocationNonce: 0,
    schemaHash: '1',
    globalRoot: '0',          // deliberately not the real root
    globalSiblings: zeroSiblings(GLOBAL_DEPTH),
    globalPathIndices: zeroPath(GLOBAL_DEPTH),
  };
}

describe('SOLID-SEC-029: IdentityAnchor enable gate', function () {
  this.timeout(120_000);
  let circuit;

  before(async () => {
    circuit = await wasm_tester(TEMPLATE_PATH, {
      include: [path.join(__dirname, '..', 'node_modules')],
    });
  });

  it('enabled=0 with arbitrary siblings succeeds (padding slot)', async () => {
    const input = baseInput();
    input.enabled = 0;
    input.schemaHash = 0;
    await mustSucceed(circuit, input);
  });

  it('enabled=1 with garbage globalRoot fails (active slot must prove inclusion)', async () => {
    const input = baseInput();
    input.enabled = 1;
    await mustReject(circuit, input, 'enabled=1, root=0, siblings=0');
  });

  it('rejects enabled = 2 (bit-constraint on the enable flag)', async () => {
    const input = baseInput();
    input.enabled = 2;
    await mustReject(circuit, input, 'enabled=2');
  });
});
