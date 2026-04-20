#!/usr/bin/env python3
"""
check_program_ids.py — Hard-gate against Anchor.toml ↔ deployments drift.

Runs in CI and locally.  Fails fast when the program IDs declared in
`Anchor.toml` (the source of truth consumed by the circuits, the TS SDK, and
`declare_id!` macros in the on-chain programs) diverge from the IDs actually
recorded in `deployments/<cluster>.json`.

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
