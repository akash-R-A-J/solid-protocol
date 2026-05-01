#!/usr/bin/env python3
"""
SOLID-SEC-081 regression gate for `scripts/check_program_ids.py`.

Exercises the program-ID consistency checker against synthetic minimal
fixtures (Anchor.toml + lib.rs + cpi_helpers.rs + ts-sdk owner files +
deployment manifests) so we can assert the gate fires on every drift
class without poking at the live repo.

This file is the gate's own gate.  Run with:

    python3 scripts/test_check_program_ids.py

or under pytest if you have it.  No external deps; stdlib only.
"""

from __future__ import annotations

import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest
from typing import Dict


SCRIPT_PATH = pathlib.Path(__file__).resolve().parent / "check_program_ids.py"

# Canonical IDs used in every fixture.  These are the live repo's
# canonical IDs as of 2026-05-01 -- mirroring them here means a future
# rename in the real repo trips this test, which is a desirable "did
# you also update the gate?" signal.
ID_ZK = "DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb"
ID_ISSUER = "5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx"
ID_SCHEMA = "4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1"

# Recognisable wrong-shape ID (still 32-44 base58 chars so the regex
# matches; literally nothing on chain).  Use this for drift planting so
# the assertion "this is wrong" is unambiguous.
ID_DRIFT = "WRoNGwRoNGwRoNGwRoNGwRoNGwRoNGwRoNG12345678"


def _write(path: pathlib.Path, body: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body)


def _build_clean_fixture(root: pathlib.Path, ids: Dict[str, str]) -> None:
    """Write a minimal repo-shaped tree under `root` that the gate
    accepts as fully consistent.  Drift tests then mutate ONE file in
    this tree to plant the failure mode."""
    _write(
        root / "Anchor.toml",
        '[programs.localnet]\n'
        f'zk_verifier = "{ids["zk_verifier"]}"\n'
        f'issuer_registry = "{ids["issuer_registry"]}"\n'
        f'schema_registry = "{ids["schema_registry"]}"\n',
    )
    for prog, dirname in (
        ("zk_verifier", "zk-verifier"),
        ("issuer_registry", "issuer-registry"),
        ("schema_registry", "schema-registry"),
    ):
        _write(
            root / "programs" / dirname / "src" / "lib.rs",
            f'declare_id!("{ids[prog]}");\n',
        )
    _write(
        root / "crates" / "solid-light" / "src" / "cpi_helpers.rs",
        f'pub const SCHEMA_REGISTRY_PROGRAM_ID: &str = "{ids["schema_registry"]}";\n'
        f'pub const ISSUER_REGISTRY_PROGRAM_ID: &str = "{ids["issuer_registry"]}";\n',
    )
    # ts-sdk owner files that the SEC-081 gate validates.
    _write(
        root / "ts-sdk/packages/core/src/index.ts",
        f"export const PROGRAM_PUBKEYS = {{\n"
        f"  zkVerifier: '{ids['zk_verifier']}',\n"
        f"  issuerRegistry: '{ids['issuer_registry']}',\n"
        f"  schemaRegistry: '{ids['schema_registry']}',\n"
        f"}};\n",
    )
    _write(
        root / "ts-sdk/packages/sdk/src/config.ts",
        f"export const PROGRAM_IDS = {{\n"
        f"  ZK_VERIFIER: '{ids['zk_verifier']}',\n"
        f"  ISSUER_REGISTRY: '{ids['issuer_registry']}',\n"
        f"  SCHEMA_REGISTRY: '{ids['schema_registry']}',\n"
        f"}};\n",
    )
    _write(
        root / "ts-sdk/packages/light/src/index.ts",
        f"import {{ PublicKey }} from '@solana/web3.js';\n"
        f"export const ISSUER_REGISTRY_PROGRAM_ID = "
        f"new PublicKey('{ids['issuer_registry']}');\n"
        f"export const SCHEMA_REGISTRY_PROGRAM_ID = "
        f"new PublicKey('{ids['schema_registry']}');\n",
    )
    _write(
        root / "tests/integration/01_registry_init.test.ts",
        f"const ISSUER_REGISTRY = '{ids['issuer_registry']}';\n",
    )
    # No deployments/ -- localnet doesn't require one (test_clean
    # asserts this is OK).


def _run_gate(repo_root: pathlib.Path) -> subprocess.CompletedProcess:
    env = os.environ.copy()
    env["SOLID_REPO_ROOT"] = str(repo_root)
    return subprocess.run(
        [sys.executable, str(SCRIPT_PATH)],
        env=env,
        capture_output=True,
        text=True,
    )


class CheckProgramIdsTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = pathlib.Path(tempfile.mkdtemp(prefix="check_pids_"))
        self.canonical = {
            "zk_verifier": ID_ZK,
            "issuer_registry": ID_ISSUER,
            "schema_registry": ID_SCHEMA,
        }
        _build_clean_fixture(self.tmp, self.canonical)

    def tearDown(self) -> None:
        shutil.rmtree(self.tmp, ignore_errors=True)

    # ───────────── Baseline ─────────────

    def test_clean_fixture_passes(self) -> None:
        """A minimal correctly-aligned tree must return exit 0."""
        result = _run_gate(self.tmp)
        self.assertEqual(
            result.returncode,
            0,
            f"clean fixture rejected.\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}",
        )

    # ───────────── SOLID-SEC-032 (Rust side, regression of pre-existing gate) ─────────────

    def test_declare_id_drift_is_caught(self) -> None:
        """A typo in any program's `declare_id!()` must be rejected
        by the existing Rust-side gate (SOLID-SEC-032 era; included
        here to assert the new TS extension didn't accidentally
        regress the older check)."""
        path = self.tmp / "programs/zk-verifier/src/lib.rs"
        path.write_text(f'declare_id!("{ID_DRIFT}");\n')
        result = _run_gate(self.tmp)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("declare_id", result.stderr)

    # ───────────── SOLID-SEC-081 (NF-05) — the new TS gate ─────────────

    def test_ts_owner_typo_is_caught(self) -> None:
        """An owner-allowlisted TS file with a literal that doesn't
        match Anchor.toml must trip the SOLID-SEC-081 gate."""
        path = self.tmp / "ts-sdk/packages/core/src/index.ts"
        body = path.read_text().replace(ID_ZK, ID_DRIFT)
        path.write_text(body)
        result = _run_gate(self.tmp)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("ts-sdk/packages/core/src/index.ts", result.stderr)
        self.assertIn("zk_verifier", result.stderr)

    def test_unauthorized_ts_file_is_caught(self) -> None:
        """A non-allowlisted TS file that hard-codes a canonical
        program ID must trip the SOLID-SEC-081 gate."""
        path = self.tmp / "ts-sdk/packages/issuer/src/badfile.ts"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            f"// Should import from @solid-protocol/core; this is the\n"
            f"// drift surface SOLID-SEC-081 closes.\n"
            f"const REG = '{ID_ISSUER}';\n"
        )
        result = _run_gate(self.tmp)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("badfile.ts", result.stderr)
        self.assertIn("NOT in the SOLID-SEC-081 allowlist", result.stderr)

    def test_owner_missing_id_is_caught(self) -> None:
        """An owner-allowlisted TS file whose declared program ID has
        been deleted (not just mistyped) must also trip the gate -- a
        consumer importing this file would see `undefined`."""
        path = self.tmp / "ts-sdk/packages/core/src/index.ts"
        # Strip out the zk_verifier line entirely.
        body = "\n".join(
            line for line in path.read_text().splitlines()
            if "zkVerifier" not in line
        ) + "\n"
        path.write_text(body)
        result = _run_gate(self.tmp)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("ts-sdk/packages/core/src/index.ts", result.stderr)
        self.assertIn("zk_verifier", result.stderr)


if __name__ == "__main__":
    unittest.main(verbosity=2)
