import { PublicKey } from '@solana/web3.js';
import { LocalReplicaAdapter, poseidonHashPair } from '@solid-protocol/light';

const ZERO_LEAF = new Uint8Array(32);

export interface RootAppendPlan {
  kind: 'append';
  leafIndex: number;
  oldLeaf: Uint8Array;
  newLeaf: Uint8Array;
  newRoot: Uint8Array;
  siblings: Uint8Array[];
  pathIndices: number[];
  matchedPrefixLength: number;
}

export interface RootSkipPlan {
  kind: 'skip';
  currentRoot: Uint8Array;
  matchedPrefixLength: number;
}

export type RootUpdatePlan = RootAppendPlan | RootSkipPlan;

export function bytesToHex(bytes: Uint8Array): string {
  return Buffer.from(bytes).toString('hex');
}

export function hexToBytes32(value: string, name = 'hex'): Uint8Array {
  const hex = normalizeHex32(value, name);
  return Uint8Array.from(Buffer.from(hex, 'hex'));
}

export function normalizeHex32(value: string, name = 'hex'): string {
  const hex = String(value ?? '').trim().replace(/^0x/i, '').toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(hex)) {
    throw new Error(`${name} must be a 32-byte hex string`);
  }
  return hex;
}

export function appendTrackedLeafHex(existing: unknown, leaf: Uint8Array): string[] {
  const next = Array.isArray(existing)
    ? existing.map((entry, index) => normalizeHex32(String(entry), `trackedLeaves[${index}]`))
    : [];
  const leafHex = bytesToHex(leaf);
  if (!next.includes(leafHex)) next.push(leafHex);
  return next;
}

export function rootForLeaves(depth: number, leaves: Uint8Array[]): Uint8Array {
  const replica = new LocalReplicaAdapter(depth, poseidonHashPair);
  for (const leaf of leaves) replica.appendLeaf(leaf);
  return replica.getRoot();
}

export async function planAppendToLiveRoot(params: {
  depth: number;
  trackedLeafHexes: string[];
  currentRoot: Uint8Array;
}): Promise<RootUpdatePlan> {
  const leaves = params.trackedLeafHexes.map((leaf, index) =>
    hexToBytes32(leaf, `trackedLeafHexes[${index}]`),
  );
  const currentHex = bytesToHex(params.currentRoot);

  for (let prefixLength = leaves.length; prefixLength >= 0; prefixLength--) {
    const prefixLeaves = leaves.slice(0, prefixLength);
    const prefixRoot = rootForLeaves(params.depth, prefixLeaves);
    if (bytesToHex(prefixRoot) !== currentHex) continue;

    if (prefixLength === leaves.length) {
      return {
        kind: 'skip',
        currentRoot: prefixRoot,
        matchedPrefixLength: prefixLength,
      };
    }

    const afterLeaves = leaves.slice(0, prefixLength + 1);
    const replica = new LocalReplicaAdapter(params.depth, poseidonHashPair);
    for (const leaf of afterLeaves) replica.appendLeaf(leaf);
    const newLeaf = afterLeaves[prefixLength];
    const proof = await replica.fetch(PublicKey.default, newLeaf);
    return {
      kind: 'append',
      leafIndex: prefixLength,
      oldLeaf: ZERO_LEAF,
      newLeaf,
      newRoot: proof.root,
      siblings: proof.siblings,
      pathIndices: proof.pathIndices,
      matchedPrefixLength: prefixLength,
    };
  }

  throw new Error(
    'Tracked leaves do not match the live binding root. Rebuild the leaf history from the indexer or use a fresh depth-compatible schema tree.',
  );
}

export function flattenSiblings(siblings: Uint8Array[], depth: number): Uint8Array {
  if (siblings.length !== depth) {
    throw new Error(`Expected ${depth} Merkle siblings, got ${siblings.length}`);
  }
  const flat = new Uint8Array(depth * 32);
  for (let i = 0; i < depth; i++) flat.set(siblings[i], i * 32);
  return flat;
}
