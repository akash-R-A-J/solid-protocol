import assert from 'node:assert/strict';
import { initWasm } from '@solid-protocol/core';
import { poseidonHashPair } from '@solid-protocol/light';
import {
  appendTrackedLeafHex,
  bytesToHex,
  hexToBytes32,
  planAppendToLiveRoot,
  rootForLeaves,
} from '../../scripts/lib/root_update_plan';

function leaf(byte: number): Uint8Array {
  return new Uint8Array(32).fill(byte);
}

function recomputeRootFromPath(
  leafBytes: Uint8Array,
  leafIndex: number,
  siblings: Uint8Array[],
): Uint8Array {
  let node = leafBytes;
  let cursor = leafIndex;
  for (const sibling of siblings) {
    const isRight = (cursor & 1) === 1;
    node = isRight ? poseidonHashPair(sibling, node) : poseidonHashPair(node, sibling);
    cursor >>= 1;
  }
  return node;
}

async function main() {
  await initWasm();

  {
  const tracked = [bytesToHex(leaf(1))];
  const currentRoot = rootForLeaves(2, []);
  const plan = await planAppendToLiveRoot({ depth: 2, trackedLeafHexes: tracked, currentRoot });
  assert.equal(plan.kind, 'append');
  assert.equal(plan.leafIndex, 0);
  assert.equal(bytesToHex(plan.oldLeaf), '00'.repeat(32));
  assert.equal(bytesToHex(plan.newRoot), bytesToHex(rootForLeaves(2, [leaf(1)])));
  assert.equal(
    bytesToHex(recomputeRootFromPath(plan.oldLeaf, plan.leafIndex, plan.siblings)),
    bytesToHex(currentRoot),
  );
}

  {
  const tracked = [bytesToHex(leaf(1)), bytesToHex(leaf(2))];
  const currentRoot = rootForLeaves(2, [leaf(1)]);
  const plan = await planAppendToLiveRoot({ depth: 2, trackedLeafHexes: tracked, currentRoot });
  assert.equal(plan.kind, 'append');
  assert.equal(plan.leafIndex, 1);
  assert.equal(bytesToHex(plan.newLeaf), bytesToHex(leaf(2)));
  assert.equal(bytesToHex(plan.newRoot), bytesToHex(rootForLeaves(2, [leaf(1), leaf(2)])));
  assert.equal(
    bytesToHex(recomputeRootFromPath(plan.oldLeaf, plan.leafIndex, plan.siblings)),
    bytesToHex(currentRoot),
  );
}

  {
  const tracked = [bytesToHex(leaf(1))];
  const currentRoot = rootForLeaves(2, [leaf(1)]);
  const plan = await planAppendToLiveRoot({ depth: 2, trackedLeafHexes: tracked, currentRoot });
  assert.equal(plan.kind, 'skip');
  assert.equal(plan.matchedPrefixLength, 1);
}

  {
  assert.deepEqual(appendTrackedLeafHex([], leaf(1)), [bytesToHex(leaf(1))]);
  assert.deepEqual(
    appendTrackedLeafHex([bytesToHex(leaf(1))], leaf(1)),
    [bytesToHex(leaf(1))],
  );
}

  {
  await assert.rejects(
    () => planAppendToLiveRoot({
      depth: 2,
      trackedLeafHexes: [bytesToHex(leaf(1))],
      currentRoot: hexToBytes32('ff'.repeat(32), 'currentRoot'),
    }),
    /Tracked leaves do not match/,
  );
}

  console.log('root update planner tests passed');
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
