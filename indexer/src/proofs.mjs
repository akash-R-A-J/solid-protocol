import { PublicKey } from '@solana/web3.js';
import { initWasm, poseidonHashBytes } from '@solid-protocol/core';
import { activeLeavesForTree } from './store.mjs';
import { bytesToHex, hexToBytes32, normalizeHex32 } from './encoding.mjs';

export async function buildMerkleProof(store, manifest, treeAddress, leafHex) {
  new PublicKey(treeAddress);
  const targetLeaf = normalizeHex32(leafHex, 'leaf');
  const depth = treeDepthFor(manifest, treeAddress);
  const leaves = activeLeavesForTree(store, treeAddress);
  const target = leaves.find((entry) => entry.leaf === targetLeaf);
  if (!target) {
    const error = new Error(`Leaf ${targetLeaf} has not been indexed for tree ${treeAddress}`);
    error.code = 'LEAF_NOT_INDEXED';
    throw error;
  }

  await initWasm();
  const leafByIndex = new Map(leaves.map((entry) => [entry.leafIndex, hexToBytes32(entry.leaf, 'leaf')]));
  const zero = new Uint8Array(32);
  const zeroLevels = [zero];
  for (let height = 1; height <= depth; height++) {
    const below = zeroLevels[height - 1];
    zeroLevels.push(poseidonHashPair(below, below));
  }

  const subtreeRoot = (start, height) => {
    if (height === 0) {
      return leafByIndex.get(start) ?? zero;
    }
    const span = 1 << height;
    const hasAnyLeaf = leaves.some((entry) =>
      entry.leafIndex >= start && entry.leafIndex < start + span,
    );
    if (!hasAnyLeaf) return zeroLevels[height];
    const mid = start + (span >> 1);
    return poseidonHashPair(subtreeRoot(start, height - 1), subtreeRoot(mid, height - 1));
  };

  const siblings = [];
  const pathIndices = [];
  let cursor = target.leafIndex;
  for (let height = 0; height < depth; height++) {
    const isRight = (cursor & 1) === 1;
    const siblingIndex = isRight ? cursor - 1 : cursor + 1;
    const siblingSubtreeStart = siblingIndex << height;
    siblings.push(bytesToHex(subtreeRoot(siblingSubtreeStart, height)));
    pathIndices.push(isRight ? 1 : 0);
    cursor >>= 1;
  }

  return {
    root: bytesToHex(subtreeRoot(0, depth)),
    siblings,
    pathIndices,
    leafIndex: target.leafIndex,
    slot: target.slot,
  };
}

export function treeDepthFor(manifest, treeAddress) {
  const schema = (manifest.schemas ?? []).find((entry) => entry.tree_address === treeAddress);
  if (schema?.tree_depth !== undefined && schema.tree_depth !== null) {
    return Number(schema.tree_depth);
  }
  if (manifest.trees?.issuer_tree?.tree_address === treeAddress) {
    return Number(manifest.trees.issuer_tree.depth ?? 20);
  }
  if (manifest.trees?.global_state_tree?.tree_address === treeAddress) {
    return Number(manifest.trees.global_state_tree.depth ?? 20);
  }
  return 20;
}

function poseidonHashPair(left, right) {
  return poseidonHashBytes([left, right]);
}
