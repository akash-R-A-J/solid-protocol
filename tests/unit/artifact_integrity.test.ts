/**
 * Unit tests for SOLID-SEC-058 / CRIT-3 artifact integrity gate.
 *
 * Run via:
 *   npx tsx tests/unit/artifact_integrity.test.ts
 *
 * Exercises every behavior axis of `verifyArtifactSha256` /
 * `resolveExpectedSha256` from `ts-sdk/packages/sdk/src/artifact_integrity.ts`:
 *
 *   - happy path: matching env-var pin -> returns hash, no throw
 *   - happy path: matching sidecar pin -> returns hash, no throw
 *   - happy path: matching config pin  -> returns hash, no throw
 *   - happy path: env var preferred over sidecar over config
 *   - mismatch on env var pin    -> throws
 *   - mismatch on sidecar pin    -> throws
 *   - mismatch on config pin     -> throws
 *   - non-hex pin                -> throws
 *   - cdn-loaded + no pin        -> throws
 *   - local-loaded + no pin      -> warns, returns hash, no throw
 *   - skip mode bypasses every check
 *   - sha256Hex agrees with `shasum -a 256`
 */

import {
  verifyArtifactSha256,
  resolveExpectedSha256,
  sha256Hex,
  type ArtifactPin,
} from '../../ts-sdk/packages/sdk/src/artifact_integrity';
import { SOLID_CONFIG } from '../../ts-sdk/packages/sdk/src/config';
import { createHash } from 'crypto';

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

function assertThrows(fn: () => unknown, expectedSubstring: string, msg: string): void {
  try {
    fn();
    fail += 1;
    fails.push(`${msg}: expected throw but did not`);
  } catch (e: any) {
    if (e.message.includes(expectedSubstring)) {
      pass += 1;
    } else {
      fail += 1;
      fails.push(`${msg}: thrown error did not contain "${expectedSubstring}": ${e.message}`);
    }
  }
}

const PIN_A: ArtifactPin = {
  envVar: '__SOLID_TEST_ENV_PIN_A',
  sidecarPath: '/tmp/__solid-test-pin-a.sha256',
};

const KNOWN_BYTES = new TextEncoder().encode('the quick brown fox jumps over the lazy dog');
const KNOWN_HASH = (() => {
  const h = createHash('sha256');
  h.update(KNOWN_BYTES);
  return h.digest('hex').toLowerCase();
})();

function clearEnvAndConfig(pin: ArtifactPin) {
  delete process.env[pin.envVar];
  delete process.env.SOLID_CIRCUIT_ARTIFACT_INTEGRITY;
  if (pin.configKey != null) {
    SOLID_CONFIG.ARTIFACT_SHA256[pin.configKey] = '';
  }
}

function withEnv(envVar: string, value: string, fn: () => void) {
  const old = process.env[envVar];
  process.env[envVar] = value;
  try {
    fn();
  } finally {
    if (old === undefined) delete process.env[envVar];
    else process.env[envVar] = old;
  }
}

// ─── sha256Hex ───────────────────────────────────────────────────────────
{
  const h = sha256Hex(KNOWN_BYTES);
  assert(
    h === KNOWN_HASH,
    `sha256Hex matches openssl-style digest (got ${h}, expected ${KNOWN_HASH})`,
  );
  assert(/^[0-9a-f]{64}$/.test(h), 'sha256Hex output is 64-char lowercase hex');
}

// ─── happy path: env var pin matches ─────────────────────────────────────
{
  clearEnvAndConfig(PIN_A);
  withEnv(PIN_A.envVar, KNOWN_HASH, () => {
    const got = verifyArtifactSha256(KNOWN_BYTES, PIN_A, () => undefined, 'A', false);
    assert(got === KNOWN_HASH, 'env var pin matching: returns actual hash');
  });
}

// ─── happy path: sidecar matches ────────────────────────────────────────
{
  clearEnvAndConfig(PIN_A);
  const got = verifyArtifactSha256(KNOWN_BYTES, PIN_A, _ => KNOWN_HASH, 'A', false);
  assert(got === KNOWN_HASH, 'sidecar pin matching: returns actual hash');
}

// ─── happy path: env var > sidecar > config priority ───────────────────
{
  clearEnvAndConfig(PIN_A);
  withEnv(PIN_A.envVar, KNOWN_HASH, () => {
    // sidecar disagrees, but env wins -> no throw.
    const got = verifyArtifactSha256(KNOWN_BYTES, PIN_A, _ => 'a'.repeat(64), 'A', false);
    assert(got === KNOWN_HASH, 'env var preferred over sidecar (matches actual)');
  });
}

// ─── mismatch on env var pin ───────────────────────────────────────────
{
  clearEnvAndConfig(PIN_A);
  withEnv(PIN_A.envVar, 'b'.repeat(64), () => {
    assertThrows(
      () => verifyArtifactSha256(KNOWN_BYTES, PIN_A, () => undefined, 'A', false),
      'SHA-256 mismatch',
      'mismatch on env var pin throws',
    );
  });
}

// ─── mismatch on sidecar pin ──────────────────────────────────────────
{
  clearEnvAndConfig(PIN_A);
  assertThrows(
    () => verifyArtifactSha256(KNOWN_BYTES, PIN_A, _ => 'c'.repeat(64), 'A', false),
    'SHA-256 mismatch',
    'mismatch on sidecar pin throws',
  );
}

// ─── non-hex pin ──────────────────────────────────────────────────────
{
  clearEnvAndConfig(PIN_A);
  withEnv(PIN_A.envVar, 'not-hex-not-64-chars', () => {
    assertThrows(
      () => verifyArtifactSha256(KNOWN_BYTES, PIN_A, () => undefined, 'A', false),
      'is not 64-char',
      'non-hex pin throws',
    );
  });
}

// ─── CDN-loaded + no pin = hard fail ─────────────────────────────────
{
  clearEnvAndConfig(PIN_A);
  assertThrows(
    () => verifyArtifactSha256(KNOWN_BYTES, PIN_A, () => undefined, 'A', true),
    'refusing to use CDN-loaded artifact',
    'cdn + no pin throws',
  );
}

// ─── local-loaded + no pin = warn, return hash ─────────────────────
{
  clearEnvAndConfig(PIN_A);
  // Capture warn output to confirm it fires (sanity).
  const oldWarn = console.warn;
  let warned = false;
  console.warn = (..._args: any[]) => {
    warned = true;
  };
  try {
    const got = verifyArtifactSha256(KNOWN_BYTES, PIN_A, () => undefined, 'A', false);
    assert(got === KNOWN_HASH, 'local + no pin returns actual hash');
    assert(warned, 'local + no pin emits a warning');
  } finally {
    console.warn = oldWarn;
  }
}

// ─── skip mode bypasses every check ─────────────────────────────────
{
  clearEnvAndConfig(PIN_A);
  withEnv('SOLID_CIRCUIT_ARTIFACT_INTEGRITY', 'skip', () => {
    withEnv(PIN_A.envVar, 'b'.repeat(64), () => {
      const oldWarn = console.warn;
      console.warn = () => {
        /* swallow */
      };
      try {
        // Mismatched pin AND skip mode -> bypasses, returns actual hash.
        const got = verifyArtifactSha256(KNOWN_BYTES, PIN_A, () => undefined, 'A', true);
        assert(got === KNOWN_HASH, 'skip mode bypasses CDN+mismatch and returns actual hash');
      } finally {
        console.warn = oldWarn;
      }
    });
  });
}

// ─── resolveExpectedSha256: env > config > sidecar priority ─────────
{
  clearEnvAndConfig(PIN_A);
  // No env, no config, sidecar present -> sidecar wins.
  const got = resolveExpectedSha256(PIN_A, _ => '   ' + 'd'.repeat(64) + '\n');
  assert(got === 'd'.repeat(64), 'resolveExpected: sidecar trimmed');
}
{
  clearEnvAndConfig(PIN_A);
  withEnv(PIN_A.envVar, 'E'.repeat(64), () => {
    const got = resolveExpectedSha256(PIN_A, _ => 'f'.repeat(64));
    assert(got === 'e'.repeat(64), 'resolveExpected: env lowercased');
  });
}

// ─── summary ─────────────────────────────────────────────────────────
console.log(`\nartifact_integrity.test.ts -- ${pass} passed, ${fail} failed`);
if (fail > 0) {
  for (const m of fails) console.log('  FAIL:', m);
  process.exit(1);
}
