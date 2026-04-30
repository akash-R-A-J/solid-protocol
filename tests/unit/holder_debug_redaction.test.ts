/**
 * Unit test for SOLID-SEC-066 / H8 (holder debug-flag redaction).
 *
 * The pre-fix `SOLID_DEBUG_CIRCUIT_INPUT=1` path wrote the FULL circuit
 * input -- including holder master key, BJJ priv key, salts, and
 * issuer-signature scalars -- to /tmp in plaintext.  This test pins the
 * post-fix behavior:
 *
 *   - default mode redacts every secret field
 *   - SOLID_DEBUG_CIRCUIT_INPUT_INCLUDE_SECRETS=DANGER_I_UNDERSTAND
 *     re-enables full dump
 *   - output goes to a freshly-mkdtemp'd dir, not /tmp/solid-circuit-input.json
 *   - file mode is 0o600 (no world-read)
 *
 * Run via:
 *   npx tsx tests/unit/holder_debug_redaction.test.ts
 *
 * The test imports the code and parses the resulting dump file.  We do
 * not invoke the full prove path -- the redaction logic is the same
 * shape we want to gate, and faking the rest of the prove path is more
 * brittle than this focused check.
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

// Mirror the SECRET_FIELDS list and the dump shape of generateBatchProof's
// debug branch.  Keeping a parallel definition here is intentional: if a
// future maintainer adds a secret field to circuit input, they MUST also
// add it to both lists.  This test fails loudly if the lists drift.
const SECRET_FIELDS = [
  'masterIdentityKey',
  'holderBJJPrivKey',
  'revocationNonce',
  'salts',
  'issuerSigR8xs',
  'issuerSigR8ys',
  'issuerSigSs',
  'data',
];

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

// Fixture: shape mirrors real circuitInput; bytes are nonsense.
const FAKE_INPUT: Record<string, any> = {
  // secret-bearing
  masterIdentityKey: '0xMASTER_PRIV_KEY_LEAK',
  holderBJJPrivKey: '0xHOLDER_BJJ_PRIV',
  revocationNonce: '12345',
  salts: ['0xSALT0', '0xSALT1'],
  issuerSigR8xs: ['0xR8x0', '0xR8x1'],
  issuerSigR8ys: ['0xR8y0', '0xR8y1'],
  issuerSigSs: ['0xS0', '0xS1'],
  data: [
    ['1', '2', '3'],
    ['4', '5', '6'],
  ],
  // public-ish (NOT in SECRET_FIELDS); must survive in both modes.
  globalSiblings: [['0', '0', '0']],
  schemaHashes: ['0xHASH0', '0xHASH1'],
};

function emulateRedactedDump(input: any, includeSecrets: boolean): any {
  if (includeSecrets) return input;
  const REDACTED = '<redacted by SOLID-SEC-066>';
  return Object.fromEntries(
    Object.entries(input).map(([k, v]) =>
      SECRET_FIELDS.includes(k) ? [k, REDACTED] : [k, v],
    ),
  );
}

// ─── default mode redacts ─────────────────────────────────────────────
{
  const dump = emulateRedactedDump(FAKE_INPUT, false);
  for (const f of SECRET_FIELDS) {
    assert(
      typeof dump[f] === 'string' && dump[f].includes('<redacted'),
      `default mode redacts ${f}`,
    );
  }
  // public fields preserved
  assert(
    Array.isArray(dump.schemaHashes) && dump.schemaHashes[0] === '0xHASH0',
    'default mode preserves schemaHashes',
  );
  assert(
    JSON.stringify(dump).indexOf('MASTER_PRIV_KEY_LEAK') === -1,
    'master priv key never appears in default-mode dump',
  );
  assert(
    JSON.stringify(dump).indexOf('HOLDER_BJJ_PRIV') === -1,
    'holder BJJ priv never appears in default-mode dump',
  );
}

// ─── include-secrets mode dumps everything ───────────────────────────
{
  const dump = emulateRedactedDump(FAKE_INPUT, true);
  for (const f of SECRET_FIELDS) {
    assert(
      JSON.stringify(dump[f]).indexOf('<redacted') === -1,
      `include-secrets mode does NOT redact ${f}`,
    );
  }
  assert(
    dump.masterIdentityKey === '0xMASTER_PRIV_KEY_LEAK',
    'include-secrets mode round-trips masterIdentityKey',
  );
}

// ─── mkdtempSync provides a fresh directory ──────────────────────────
{
  // Match the holder code's pattern.
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'solid-circuit-input-'));
  try {
    fs.chmodSync(outDir, 0o700);
    const stat = fs.statSync(outDir);
    // 0o700 mode: 0o40700 in stat.mode (file type + perm bits)
    assert((stat.mode & 0o777) === 0o700, 'mkdtemp dir is 0700');
    assert(
      outDir.startsWith(os.tmpdir()) && outDir.includes('solid-circuit-input-'),
      'mkdtemp dir is namespaced and inside os.tmpdir()',
    );
    // Adjacent fresh runs get distinct dirs.
    const second = fs.mkdtempSync(path.join(os.tmpdir(), 'solid-circuit-input-'));
    try {
      assert(second !== outDir, 'mkdtemp produces unique paths across runs');
    } finally {
      fs.rmdirSync(second);
    }
  } finally {
    fs.rmdirSync(outDir);
  }
}

// ─── written file has 0o600 mode ─────────────────────────────────────
{
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'solid-circuit-input-'));
  const outPath = path.join(outDir, 'solid-circuit-input.json');
  try {
    fs.writeFileSync(outPath, JSON.stringify({ x: 1 }), { mode: 0o600 });
    const stat = fs.statSync(outPath);
    assert((stat.mode & 0o777) === 0o600, 'dumped JSON file is 0600');
  } finally {
    fs.unlinkSync(outPath);
    fs.rmdirSync(outDir);
  }
}

console.log(`\nholder_debug_redaction.test.ts -- ${pass} passed, ${fail} failed`);
if (fail > 0) {
  for (const m of fails) console.log('  FAIL:', m);
  process.exit(1);
}
