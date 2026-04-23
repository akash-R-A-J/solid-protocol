/**
 * Shared E2E state-file helpers.
 *
 * SOLID-SEC-020 / SOLID-SEC-011 follow-up.  The issuer + holder BJJ
 * keypairs written by `scripts/issue.ts` were previously persisted to
 * `scripts/e2e_state.json` -- inside the repo working tree.  Even with
 * the file .gitignore'd and the pre-commit hook (Tier 1), that location
 * is wrong on principle: private keys do not belong under a source tree
 * that developers routinely `rm -rf`, `tar czf`, or `git clean`.
 *
 * This module moves the state file to one of:
 *   1. `$SOLID_E2E_STATE_FILE`        (explicit override; absolute path)
 *   2. `$XDG_RUNTIME_DIR/solid-e2e/state.json`   (Linux; tmpfs, mode 0700)
 *   3. `$TMPDIR/solid-e2e-<uid>/state.json`       (macOS / BSD)
 *   4. `/tmp/solid-e2e-<uid>/state.json`          (fallback)
 *
 * All ancestor directories are created with `mode 0700`; the file itself
 * is written with `mode 0600`.  Callers should use `readState` /
 * `writeState` instead of touching fs directly.
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

/// Resolve the absolute path to the E2E state file. Idempotent; does not
/// create the file, but does create its parent directory with 0700.
export function stateFilePath(): string {
  const override = process.env.SOLID_E2E_STATE_FILE;
  if (override && override.length > 0) {
    ensureParent(override);
    return override;
  }

  const uid = typeof process.getuid === 'function' ? process.getuid() : 0;

  const xdg = process.env.XDG_RUNTIME_DIR;
  if (xdg) {
    const dir = path.join(xdg, 'solid-e2e');
    ensureDir(dir);
    return path.join(dir, 'state.json');
  }

  const tmp = process.env.TMPDIR ?? os.tmpdir() ?? '/tmp';
  const dir = path.join(tmp, `solid-e2e-${uid}`);
  ensureDir(dir);
  return path.join(dir, 'state.json');
}

/// Create the parent directory of `file` with mode 0700 if missing.
function ensureParent(file: string): void {
  const dir = path.dirname(file);
  ensureDir(dir);
}

/// Create `dir` (and ancestors) with mode 0700.
function ensureDir(dir: string): void {
  fs.mkdirSync(dir, { recursive: true, mode: 0o700 });
  // mkdirSync(recursive) does not re-chmod an existing directory, so
  // chmod explicitly. Swallow EPERM on exotic filesystems.
  try {
    fs.chmodSync(dir, 0o700);
  } catch {
    // noop
  }
}

/// Read and parse the state file.  Returns `null` if it doesn't exist.
export function readStateOrNull(): any | null {
  const file = stateFilePath();
  if (!fs.existsSync(file)) return null;
  return JSON.parse(fs.readFileSync(file, 'utf-8'));
}

/// Like `readStateOrNull` but throws a clear error if the caller needs
/// a prior-step state file and it's not there.
export function readState(caller: string): any {
  const state = readStateOrNull();
  if (state === null) {
    throw new Error(
      `${caller}: state file not found at ${stateFilePath()}.\n` +
      `Did the upstream step run?  See scripts/lib/e2e_state.ts.`,
    );
  }
  return state;
}

/// Write state atomically with mode 0600.
export function writeState(state: unknown): string {
  const file = stateFilePath();
  const tmp = `${file}.tmp.${process.pid}`;
  fs.writeFileSync(tmp, JSON.stringify(state, null, 2), { mode: 0o600 });
  fs.renameSync(tmp, file);
  return file;
}
