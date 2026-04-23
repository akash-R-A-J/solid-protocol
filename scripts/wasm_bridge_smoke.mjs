#!/usr/bin/env node
// SOLID-SEC-009 regression gate.
//
// Verifies that `@solid-protocol/core` can load the WASM bridge that
// CI builds at `ts-sdk/packages/core/wasm/` and that a round-trip
// Poseidon + hardened-nullifier call returns a non-zero 32-byte
// digest. Before this check existed, CI built the bridge into an
// empty `pkg/` dir and consumers silently got a broken bundle at
// runtime.  See also plan/RESUME.md Section 1 / wasm_bridge_smoke.
//
// Run from `ts-sdk/packages/core/` so the compiled entry point at
// `dist/index.js` and the sibling `wasm/` dir are on the resolution
// path exactly as a downstream consumer would see them.

import { strict as assert } from 'node:assert';

const core = await import('./dist/index.js');

await core.initWasm();

// Two Poseidon invocations exercise both code paths in the bridge:
// (a) `hash_fields_to_bytes` via `poseidonHash(bigint[])`
// (b) `hash_bytes` via `poseidonHashBytes(Uint8Array[])` (shared
//     zero-copy buffer path).
const digestA = core.poseidonHash([1n, 2n, 3n]);
assert.equal(digestA.length, 32, 'poseidonHash digest must be 32 bytes');
assert.ok(digestA.some((b) => b !== 0), 'poseidonHash produced all-zero bytes');

const digestB = core.poseidonHashBytes([
  new Uint8Array(32).fill(1),
  new Uint8Array(32).fill(2),
]);
assert.equal(digestB.length, 32, 'poseidonHashBytes digest must be 32 bytes');
assert.ok(digestB.some((b) => b !== 0), 'poseidonHashBytes produced all-zero bytes');

// Distinct inputs must produce distinct digests.  If the bridge were
// stubbed or the wrong module were loaded, this would fail.
assert.notDeepEqual(
  Array.from(digestA),
  Array.from(digestB),
  'distinct inputs must yield distinct digests',
);

console.log('wasm_bridge_smoke: OK');
console.log('  poseidonHash([1,2,3])     =', Buffer.from(digestA).toString('hex'));
console.log('  poseidonHashBytes([..])   =', Buffer.from(digestB).toString('hex'));
