/**
 * Off-chain prover-artifact integrity gate (CRIT-3 / SOLID-SEC-058).
 *
 * The on-chain VK is content-addressed (SOLID-SEC-041) -- both via
 * `circuits/build/verification_key.sha256` and the `SOLID_VK_SHA256`
 * pin enforced by `scripts/initialize.ts`.  Pre-CRIT-3, the off-chain
 * prover artifacts (`.wasm`, `.zkey`) had no analog: a compromised CDN
 * or DNS hijack could serve a backdoored zkey, and Groth16 would
 * gladly produce verifying proofs from any toxic-waste-knowing
 * substituter.  The on-chain VK gate does not catch this -- the
 * verifier accepts proofs from any prover whose zkey was generated
 * either honestly or dishonestly under the SAME VK.
 *
 * Robust fix: pin SHA-256 of every artifact the prover loads, refuse
 * to proceed on mismatch, and refuse to fetch from the production CDN
 * without a pin in place.
 *
 * Pin source priority (highest first):
 *   1. Env var (`SOLID_CIRCUIT_WASM_SHA256`, `SOLID_CIRCUIT_ZKEY_SHA256`,
 *      `SOLID_VK_SHA256`).  Useful in CI where the artifacts are
 *      regenerated each run.
 *   2. Sidecar file `<artifact>.sha256` next to the artifact.
 *      `circuits/scripts/setup.js` writes these.
 *   3. Embedded constant in `SOLID_CONFIG.ARTIFACT_SHA256`.  Set by
 *      the production publish pipeline; a fixed value pinned in this
 *      repo is the canonical hash for a given protocol release.
 *
 * If artifact comes from `ARTIFACT_BASE_URL` (CDN) and no pin is found
 * across any source, this module THROWS -- production MUST be pinned.
 *
 * Dev escape hatch: `SOLID_CIRCUIT_ARTIFACT_INTEGRITY=skip` (with a
 * loud warning print).  Never set this in production.
 */

import { createHash } from 'crypto';
import { SOLID_CONFIG } from './config';

export interface ArtifactPin {
  envVar: string;
  configKey?: keyof typeof SOLID_CONFIG.ARTIFACT_SHA256;
  sidecarPath: string;
}

/** Artifact identifiers known to the SDK. */
export type ArtifactKind = 'wasm' | 'zkey' | 'vk';

const SKIP_ENV = 'SOLID_CIRCUIT_ARTIFACT_INTEGRITY';
let skipWarned = false;

function isSkipMode(): boolean {
  return (process.env[SKIP_ENV] ?? '').toLowerCase() === 'skip';
}

function warnSkipOnce(name: string) {
  if (skipWarned) return;
  skipWarned = true;
  /* eslint-disable no-console */
  console.warn(
    `[artifact-integrity] WARNING: ${SKIP_ENV}=skip is set; integrity ` +
      `verification BYPASSED for ${name} and any subsequent artifacts. ` +
      `This MUST NOT be used in production -- a compromised .wasm/.zkey ` +
      `becomes a proof-forgery oracle (CRIT-3 / SOLID-SEC-058).`,
  );
  /* eslint-enable no-console */
}

/**
 * Compute the lowercase-hex SHA-256 of `bytes`.
 */
export function sha256Hex(bytes: Uint8Array): string {
  const h = createHash('sha256');
  h.update(bytes);
  return h.digest('hex').toLowerCase();
}

/**
 * Resolve the expected SHA-256 pin for an artifact.  Returns the pin
 * (lowercase hex) or `undefined` if no source provides one.
 *
 * The caller decides whether `undefined` is acceptable based on the
 * artifact source (CDN -> hard fail; local file -> permitted with a
 * one-shot warning).
 */
export function resolveExpectedSha256(
  pin: ArtifactPin,
  loadSidecar: (path: string) => string | undefined,
): string | undefined {
  const env = (process.env[pin.envVar] ?? '').trim().toLowerCase();
  if (env.length > 0) return env;

  if (pin.configKey != null) {
    const fromConfig = (SOLID_CONFIG.ARTIFACT_SHA256[pin.configKey] ?? '')
      .trim()
      .toLowerCase();
    if (fromConfig.length > 0) return fromConfig;
  }

  const sidecar = loadSidecar(pin.sidecarPath);
  if (sidecar != null) {
    const trimmed = sidecar.trim().toLowerCase();
    if (trimmed.length > 0) return trimmed;
  }

  return undefined;
}

/**
 * Verify that `bytes` matches an expected SHA-256 from the pin sources.
 * Returns the actual hex hash on success.  Throws if any pin source
 * disagrees, or if the artifact came from the CDN and no pin was found.
 *
 * `name` is used for error messages and the skip-mode warning.
 * `cameFromCdn` controls whether a missing pin is fatal.
 */
export function verifyArtifactSha256(
  bytes: Uint8Array,
  pin: ArtifactPin,
  loadSidecar: (path: string) => string | undefined,
  name: string,
  cameFromCdn: boolean,
): string {
  const actual = sha256Hex(bytes);

  if (isSkipMode()) {
    warnSkipOnce(name);
    return actual;
  }

  const expected = resolveExpectedSha256(pin, loadSidecar);

  if (expected == null) {
    if (cameFromCdn) {
      throw new Error(
        `[artifact-integrity] ${name}: refusing to use CDN-loaded artifact ` +
          `with no SHA-256 pin.  Set ${pin.envVar} or ` +
          `${pin.sidecarPath}, or override with ${SKIP_ENV}=skip (dev only). ` +
          `See SOLID-SEC-058 / CRIT-3.`,
      );
    }
    /* eslint-disable no-console */
    console.warn(
      `[artifact-integrity] ${name}: no pin source set; loaded artifact has ` +
        `SHA-256 ${actual}.  Set ${pin.envVar} or ${pin.sidecarPath} to gate.`,
    );
    /* eslint-enable no-console */
    return actual;
  }

  if (expected.length !== 64 || !/^[0-9a-f]{64}$/.test(expected)) {
    throw new Error(
      `[artifact-integrity] ${name}: pin "${expected}" is not 64-char ` +
        `lowercase hex (got from env / sidecar / config).`,
    );
  }

  if (actual !== expected) {
    throw new Error(
      `[artifact-integrity] ${name}: SHA-256 mismatch.\n` +
        `  expected: ${expected}\n` +
        `  actual:   ${actual}\n` +
        `Refusing to use this artifact.  See SOLID-SEC-058 / CRIT-3.`,
    );
  }

  return actual;
}

export const WASM_PIN: ArtifactPin = {
  envVar: 'SOLID_CIRCUIT_WASM_SHA256',
  configKey: 'BATCH_QUERY_WASM',
  sidecarPath: 'circuits/build/batch_credential_query.wasm.sha256',
};

export const ZKEY_PIN: ArtifactPin = {
  envVar: 'SOLID_CIRCUIT_ZKEY_SHA256',
  configKey: 'BATCH_QUERY_ZKEY',
  sidecarPath: 'circuits/build/batch_credential_query.zkey.sha256',
};

export const VK_PIN: ArtifactPin = {
  envVar: 'SOLID_VK_SHA256',
  configKey: 'BATCH_QUERY_VK',
  sidecarPath: 'circuits/build/verification_key.sha256',
};
