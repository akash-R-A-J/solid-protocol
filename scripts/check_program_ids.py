#!/usr/bin/env python3
"""
check_program_ids.py — Hard-gate against Anchor.toml ↔ declare_id ↔ deployments drift.

Runs in CI and locally.  Fails fast when the program IDs declared in
`Anchor.toml` (the source of truth consumed by the circuits, the TS SDK, and
`declare_id!` macros in the on-chain programs) diverge from:

  (1) the `declare_id!(...)` literal in each program's `src/lib.rs`;
  (2) the `SCHEMA_REGISTRY_PROGRAM_ID` literal in
      `crates/solid-light/src/cpi_helpers.rs` (SOLID-SEC-032);
  (3) the program IDs recorded in `deployments/<cluster>.json`;
  (4) the presence of a `deployments/<cluster>.json` for every non-localnet
      cluster declared in Anchor.toml (SOLID-SEC-040, Phase 2 prelude).

Exit code 0: everything consistent.
Exit code 1: drift detected — prints an actionable diff.
Exit code 2: structural problem (missing file, malformed manifest).
"""

from __future__ import annotations
import json
import pathlib
import re
import sys
from typing import Dict, Tuple

ROOT = pathlib.Path(__file__).resolve().parent.parent
ANCHOR_TOML = ROOT / "Anchor.toml"
DEPLOYMENTS_DIR = ROOT / "deployments"

# Maps the toml section name (anchor) ↔ the JSON key (deployments/*.json).
PROGRAM_KEYS: Tuple[str, ...] = ("zk_verifier", "issuer_registry", "schema_registry")

# Files under programs/*/src/lib.rs that must carry a matching declare_id!().
DECLARE_ID_PATHS: Dict[str, pathlib.Path] = {
    "zk_verifier":     ROOT / "programs/zk-verifier/src/lib.rs",
    "issuer_registry": ROOT / "programs/issuer-registry/src/lib.rs",
    "schema_registry": ROOT / "programs/schema-registry/src/lib.rs",
}

# SOLID-SEC-032: the zk-verifier program owner-checks tree PDAs against a
# hardcoded SCHEMA_REGISTRY_ID_BYTES constant in solid-light/src/cpi_helpers.rs.
# A drift between that constant and the schema_registry program ID in
# Anchor.toml silently re-opens the forged-trust-root attack (ADR-0010).
# We validate the base58 string literal here. The companion Rust tests in
# `crates/solid-light/src/cpi_helpers.rs` (id_bytes_tests) validate the
# byte array against the string.
CPI_HELPERS_PATH: pathlib.Path = ROOT / "crates/solid-light/src/cpi_helpers.rs"


def read_anchor_toml() -> Dict[str, Dict[str, str]]:
    """Return {cluster: {program_name: pubkey}} from `[programs.<cluster>]`."""
    text = ANCHOR_TOML.read_text()
    out: Dict[str, Dict[str, str]] = {}
    cur_cluster: str | None = None
    for line in text.splitlines():
        s = line.strip()
        m = re.match(r"\[programs\.([A-Za-z0-9_]+)\]", s)
        if m:
            cur_cluster = m.group(1)
            out.setdefault(cur_cluster, {})
            continue
        if s.startswith("["):
            cur_cluster = None
            continue
        if cur_cluster is None:
            continue
        m = re.match(r'([A-Za-z0-9_]+)\s*=\s*"([^"]+)"', s)
        if m:
            out[cur_cluster][m.group(1)] = m.group(2)
    return out


def read_declare_ids() -> Dict[str, str]:
    """Parse the `declare_id!("…")` literal from each program's lib.rs."""
    out: Dict[str, str] = {}
    for name, path in DECLARE_ID_PATHS.items():
        if not path.exists():
            sys.exit(f"[fatal] missing program source: {path}")
        for line in path.read_text().splitlines():
            m = re.match(r'\s*declare_id!\("([^"]+)"\)\s*;', line)
            if m:
                out[name] = m.group(1)
                break
        else:
            sys.exit(f"[fatal] no declare_id! in {path}")
    return out


def read_cpi_helpers_schema_registry_literal() -> str | None:
    """Return the base58 SCHEMA_REGISTRY_PROGRAM_ID literal from cpi_helpers.rs.

    None if the file is missing; empty string if the constant is not present
    in a recognized form (caller treats either as a failure).
    """
    if not CPI_HELPERS_PATH.exists():
        return None
    text = CPI_HELPERS_PATH.read_text()
    # Match: pub const SCHEMA_REGISTRY_PROGRAM_ID: &str = "DPk6…";
    m = re.search(
        r'pub\s+const\s+SCHEMA_REGISTRY_PROGRAM_ID\s*:\s*&str\s*=\s*"([^"]+)"\s*;',
        text,
    )
    return m.group(1) if m else ""


def read_deployments() -> Dict[str, Dict[str, str]]:
    """Return {cluster: {program_name: pubkey}} from deployments/*.json."""
    out: Dict[str, Dict[str, str]] = {}
    if not DEPLOYMENTS_DIR.is_dir():
        return out
    for path in sorted(DEPLOYMENTS_DIR.glob("*.json")):
        try:
            obj = json.loads(path.read_text())
        except Exception as exc:
            sys.exit(f"[fatal] {path}: invalid JSON: {exc}")
        cluster = obj.get("network") or path.stem
        progs = obj.get("programs", {}) or {}
        out[cluster] = {name: meta["program_id"] for name, meta in progs.items()}
    return out


def main() -> int:
    failures: list[str] = []

    anchor = read_anchor_toml()
    declared = read_declare_ids()
    deployments = read_deployments()

    # (1) declare_id!() must match Anchor.toml — for every cluster the ID has
    #     to be the same across clusters (it IS the same program binary).
    #     If Anchor.toml lists different IDs per cluster, we require declare_id
    #     to match ONE of them and all others to match the same.
    if not anchor:
        failures.append("Anchor.toml has no [programs.<cluster>] sections")
    else:
        for program in PROGRAM_KEYS:
            seen: set[str] = set()
            for cluster, prog_map in anchor.items():
                if program not in prog_map:
                    failures.append(f"Anchor.toml [programs.{cluster}] missing `{program}`")
                    continue
                seen.add(prog_map[program])
            if len(seen) > 1:
                failures.append(
                    f"Anchor.toml declares `{program}` with differing IDs across clusters: {sorted(seen)}"
                )
            if program in declared and seen and declared[program] not in seen:
                failures.append(
                    f"declare_id!() in {DECLARE_ID_PATHS[program].relative_to(ROOT)} "
                    f"= {declared[program]!r} but Anchor.toml says {sorted(seen)!r}"
                )

    # (1b) SOLID-SEC-032: the hardcoded SCHEMA_REGISTRY_PROGRAM_ID literal in
    # solid-light/src/cpi_helpers.rs must agree with the schema_registry ID
    # in Anchor.toml. If this drifts, the on-chain owner-check against
    # SCHEMA_REGISTRY_ID (ADR-0010) silently breaks.
    cpi_literal = read_cpi_helpers_schema_registry_literal()
    if cpi_literal is None:
        failures.append(
            f"missing {CPI_HELPERS_PATH.relative_to(ROOT)} -- "
            "cannot validate SCHEMA_REGISTRY_PROGRAM_ID literal"
        )
    elif cpi_literal == "":
        failures.append(
            f"{CPI_HELPERS_PATH.relative_to(ROOT)} does not define "
            "`pub const SCHEMA_REGISTRY_PROGRAM_ID: &str = \"...\";` -- "
            "required by SOLID-SEC-032"
        )
    else:
        schema_anchor_ids: set[str] = set()
        for cluster, prog_map in anchor.items():
            if "schema_registry" in prog_map:
                schema_anchor_ids.add(prog_map["schema_registry"])
        if schema_anchor_ids and cpi_literal not in schema_anchor_ids:
            failures.append(
                f"{CPI_HELPERS_PATH.relative_to(ROOT)} "
                f"SCHEMA_REGISTRY_PROGRAM_ID = {cpi_literal!r} "
                f"but Anchor.toml says {sorted(schema_anchor_ids)!r} -- "
                "re-derive the base58 string and the SCHEMA_REGISTRY_ID_BYTES "
                "array together (Rust tests in cpi_helpers.rs::id_bytes_tests "
                "gate the byte array)"
            )

    # (2) Every deployment manifest must agree with Anchor.toml for its cluster.
    for cluster, prog_map in deployments.items():
        anchor_for_cluster = anchor.get(cluster, {})
        for program, deployed_id in prog_map.items():
            expected = anchor_for_cluster.get(program)
            if expected is None:
                failures.append(
                    f"deployments/{cluster}.json has `{program}` = {deployed_id!r} "
                    f"but Anchor.toml [programs.{cluster}] has no entry"
                )
            elif expected != deployed_id:
                failures.append(
                    f"{cluster}: Anchor.toml `{program}` = {expected!r} "
                    f"≠ deployments/{cluster}.json `{program}` = {deployed_id!r}"
                )

    # (2b) Every non-local cluster declared in Anchor.toml must have a matching
    # deployment manifest.  Without this, deleting deployments/<cluster>.json
    # silently passes this gate even though CLAUDE.md's invariant is
    # "Anchor.toml, declare_id, and deployments all agree".  `localnet` is the
    # development stub and is not expected to have a published manifest.
    NON_LOCAL_CLUSTERS_REQUIRING_MANIFEST = {
        c for c in anchor.keys() if c != "localnet"
    }
    for cluster in NON_LOCAL_CLUSTERS_REQUIRING_MANIFEST:
        if cluster not in deployments:
            failures.append(
                f"Anchor.toml declares [programs.{cluster}] but "
                f"deployments/{cluster}.json is missing.  "
                "Either restore the manifest (via scripts/regen_devnet_manifest.py "
                f"or the cluster-specific equivalent) or drop the "
                f"[programs.{cluster}] section from Anchor.toml."
            )
        else:
            # Manifest exists; ensure every program from Anchor.toml appears.
            for program in PROGRAM_KEYS:
                if program not in deployments[cluster]:
                    failures.append(
                        f"deployments/{cluster}.json is missing `{program}`; "
                        f"Anchor.toml [programs.{cluster}] declares it."
                    )

    if failures:
        print("Program-ID consistency FAILED:", file=sys.stderr)
        for msg in failures:
            print(f"  - {msg}", file=sys.stderr)
        print(
            "\nFix procedure:\n"
            "  1. Decide the canonical IDs (usually the ones in Anchor.toml).\n"
            "  2. Close any stale deployments: `solana program close <ID> "
            "--lamports-recipient <authority>`.\n"
            "  3. Redeploy with the canonical IDs: `anchor deploy`.\n"
            "  4. Regenerate deployments/<cluster>.json to match.\n"
            "  5. Re-run scripts/store_vk.ts (VK PDA is keyed on program ID).",
            file=sys.stderr,
        )
        return 1

    print("Program IDs consistent across Anchor.toml, declare_id!, and deployments/.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
