/**
 * Cross-language test-vector checker.
 *
 * Reads `commitment_and_nullifier.json` and reproduces both the attestation
 * commitment and the hardened nullifier via `@solid-protocol/core`. Fails
 * loudly if any byte differs from the Rust-generated reference.
 *
 * Usage:
 *   pnpm -F @solid-protocol/core build
 *   ts-node tests/vectors/check_vectors.ts
 */

import * as fs from 'fs';
import * as path from 'path';
import {
  initWasm,
  computeCommitment,
  computeNullifier,
} from '@solid-protocol/core';

function hexToBytes(h: string): Uint8Array {
  if (h.length % 2 !== 0) throw new Error('hex must be even-length');
  const out = new Uint8Array(h.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(h.slice(i * 2, i * 2 + 2), 16);
  return out;
}

function bytesToHex(b: Uint8Array): string {
  return Array.from(b).map(x => x.toString(16).padStart(2, '0')).join('');
}

async function main() {
  const vectors = JSON.parse(
    fs.readFileSync(path.join(__dirname, 'commitment_and_nullifier.json'), 'utf-8'),
  );

  await initWasm();

  // Commitment
  const c = vectors.commitment;
  const commitmentTs = computeCommitment(
    c.data_fields.map((n: number) => BigInt(n)),
    hexToBytes(c.schema_hash_hex),
    hexToBytes(c.holder_pub_x_hex),
    hexToBytes(c.holder_pub_y_hex),
    hexToBytes(c.salt_hex),
  );
  const commitmentHexTs = bytesToHex(commitmentTs);
  if (commitmentHexTs !== c.expected_commitment_hex) {
    console.error('COMMITMENT MISMATCH');
    console.error('  expected:', c.expected_commitment_hex);
    console.error('  got     :', commitmentHexTs);
    process.exit(1);
  }
  console.log('✔ commitment matches Rust reference');

  // Nullifier (6-input post ADR-0014)
  const n = vectors.nullifier;
  if (typeof n.issuer_tree_root_hex !== 'string') {
    console.error(
      'nullifier vector missing issuer_tree_root_hex; regenerate via ' +
      '`cargo run -p solid-core --example gen_vectors -- tests/vectors/commitment_and_nullifier.json`',
    );
    process.exit(1);
  }
  const nullifierTs = computeNullifier(
    hexToBytes(n.master_key_hex),
    BigInt(n.rev_nonce),
    hexToBytes(n.verifier_addr_hex),
    hexToBytes(n.query_hash_hex),
    hexToBytes(n.verifier_nonce_hex),
    hexToBytes(n.issuer_tree_root_hex),
  );
  const nullifierHexTs = bytesToHex(nullifierTs);
  if (nullifierHexTs !== n.expected_nullifier_hex) {
    console.error('NULLIFIER MISMATCH');
    console.error('  expected:', n.expected_nullifier_hex);
    console.error('  got     :', nullifierHexTs);
    process.exit(1);
  }
  console.log('✔ nullifier matches Rust reference');
  console.log('\nAll cross-language vectors agree.');
}

main().catch(e => {
  console.error(e);
  process.exit(1);
});
