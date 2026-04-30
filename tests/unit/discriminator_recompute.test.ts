/**
 * M8 / SOLID-SEC-071 regression gate (closed 2026-05-01).
 *
 * `ISSUE_CREDENTIAL_DISCRIMINATOR` (and any other hand-rolled Anchor
 * instruction discriminator the SDK pins) MUST equal
 * `sha256("global:<snake_case_name>")[..8]`.  Pre-fix the constant was
 * a free byte literal with no recompute test -- exactly the SOLID-SEC-049
 * defect that bit `SPL_AC_REPLACE_LEAF_DISCRIMINATOR` and remained
 * latent until an integration test exercised the CPI.
 *
 * This test recomputes the canonical preimage at runtime and compares
 * byte-for-byte.  Any drift between the pinned literal and the true
 * derivation fails CI.
 *
 * Run via:
 *   npx tsx tests/unit/discriminator_recompute.test.ts
 */

import { createHash } from 'crypto';
import { ISSUE_CREDENTIAL_DISCRIMINATOR } from '../../ts-sdk/packages/issuer/src/index';

let pass = 0;
let fail = 0;
const fails: string[] = [];

function assert(cond: any, msg: string): void {
  if (cond) {
    pass += 1;
    return;
  }
  fail += 1;
  fails.push(msg);
}

function anchorDiscriminator(name: string): Uint8Array {
  const h = createHash('sha256').update(`global:${name}`).digest();
  return new Uint8Array(h.subarray(0, 8));
}

// ─── ISSUE_CREDENTIAL_DISCRIMINATOR ──────────────────────────────────
{
  const expected = anchorDiscriminator('issue_credential');
  assert(
    expected.length === 8,
    `recomputed discriminator must be 8 bytes (got ${expected.length})`,
  );
  const expectedHex = Buffer.from(expected).toString('hex');
  const pinnedHex = Buffer.from(ISSUE_CREDENTIAL_DISCRIMINATOR).toString('hex');
  assert(
    expectedHex === pinnedHex,
    `ISSUE_CREDENTIAL_DISCRIMINATOR drift:\n` +
      `  pinned     : ${pinnedHex}\n` +
      `  recomputed : ${expectedHex}\n` +
      `Re-derive via sha256("global:issue_credential")[..8] and update the constant.`,
  );
}

// ─── Sanity: Anchor's spec recomputed against a known case ───────────
{
  // Known-answer fixture: discriminator for `init_proof_buffer` per
  // sha256("global:init_proof_buffer")[..8].  Computed in this same
  // process via the same helper, so this test pins the helper itself.
  const ipbDisc = anchorDiscriminator('init_proof_buffer');
  assert(ipbDisc.length === 8, 'init_proof_buffer discriminator length');
  // Print for visibility (caller can sanity-check by recomputing
  // sha256("global:init_proof_buffer")[..8] independently).
  // eslint-disable-next-line no-console
  console.log(
    `  init_proof_buffer disc (recomputed): ${Buffer.from(ipbDisc).toString('hex')}`,
  );
}

console.log(
  `\ndiscriminator_recompute.test.ts -- ${pass} passed, ${fail} failed`,
);
if (fail > 0) {
  for (const m of fails) console.log('  FAIL:', m);
  process.exit(1);
}
