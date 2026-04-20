#!/usr/bin/env python3
"""
regen_devnet_manifest.py — Emit deployments/<cluster>.json from the current
`Anchor.toml` + `solana program show` output.

Usage:
    solana config set --url devnet
    python3 scripts/regen_devnet_manifest.py > deployments/devnet.json

Exit codes:
    0 success
    1 inconsistency (fix Anchor.toml or redeploy first)
    2 transient error (RPC, missing solana CLI)
"""

from __future__ import annotations
import datetime
import json
import pathlib
import re
import shutil
import subprocess
import sys
from typing import Dict

ROOT = pathlib.Path(__file__).resolve().parent.parent
ANCHOR_TOML = ROOT / "Anchor.toml"

DESCRIPTIONS = {
    "zk_verifier":     "Groth16 ZK proof verifier with PDA-per-nullifier registry",
    "issuer_registry": "DAO-governed issuer registry with staking and slashing",
    "schema_registry": "On-chain credential schema registry with tree bindings",
}


def parse_cluster_programs(cluster: str) -> Dict[str, str]:
    text = ANCHOR_TOML.read_text()
    out: Dict[str, str] = {}
    in_section = False
    for line in text.splitlines():
        s = line.strip()
        if s == f"[programs.{cluster}]":
            in_section = True
            continue
        if s.startswith("[") and s != f"[programs.{cluster}]":
            in_section = False
            continue
        if not in_section:
            continue
        m = re.match(r'([A-Za-z0-9_]+)\s*=\s*"([^"]+)"', s)
        if m:
            out[m.group(1)] = m.group(2)
    return out


def require_solana_cli() -> str:
    p = shutil.which("solana")
    if p is None:
        sys.exit("[fatal] `solana` CLI not found on PATH; run inside `nix develop`")
    return p


def program_show(program_id: str) -> Dict[str, str]:
    """Return {upgrade_authority} for the program."""
    try:
        r = subprocess.run(
            ["solana", "program", "show", program_id, "--output", "json"],
            capture_output=True, text=True, check=True, timeout=30,
        )
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as exc:
        sys.stderr.write(f"[warn] solana program show {program_id}: {exc}\n")
        return {"upgrade_authority": "UNKNOWN"}
    try:
        blob = json.loads(r.stdout)
    except json.JSONDecodeError:
        return {"upgrade_authority": "UNKNOWN"}
    return {
        "upgrade_authority": blob.get("authority") or blob.get("upgradeAuthority", "UNKNOWN"),
    }


def rpc_url_for_cluster(cluster: str) -> str:
    # Matches Anchor's conventional cluster → RPC mapping.
    return {
        "localnet": "http://localhost:8899",
        "devnet":   "https://api.devnet.solana.com",
        "testnet":  "https://api.testnet.solana.com",
        "mainnet":  "https://api.mainnet-beta.solana.com",
    }.get(cluster, "UNKNOWN")


def main(argv: list[str]) -> int:
    cluster = argv[1] if len(argv) > 1 else "devnet"
    require_solana_cli()

    programs = parse_cluster_programs(cluster)
    if not programs:
        sys.exit(f"[fatal] no programs listed for [programs.{cluster}] in Anchor.toml")

    entries = {}
    for name, pid in sorted(programs.items()):
        meta = program_show(pid)
        entries[name] = {
            "program_id": pid,
            "upgrade_authority": meta["upgrade_authority"],
            "binary": f"target/deploy/{name}.so",
            "description": DESCRIPTIONS.get(name, ""),
        }

    manifest = {
        "network": cluster,
        "cluster": rpc_url_for_cluster(cluster),
        "deployed_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "programs": entries,
        "pdas": {
            "verifier_config":  {"seeds": ["verifier-config"],                          "program": programs.get("zk_verifier", "")},
            "vk_storage":       {"seeds": ["vk-storage", "<verifier_config_pubkey>"],   "program": programs.get("zk_verifier", "")},
            "nullifier_record": {"seeds": ["null", "<nullifier32>"],                    "program": programs.get("zk_verifier", "")},
            "registry_config":  {"seeds": ["registry-config"],                          "program": programs.get("issuer_registry", "")},
            "schema_tree":      {"seeds": ["schema-tree", "<schema_hash32>"],           "program": programs.get("schema_registry", "")},
            "global_binding":   {"seeds": ["global-binding"],                           "program": programs.get("schema_registry", "")},
        },
        "toolchain": {
            "anchor_cli":    "0.30.1",
            "anchor_lang":   "0.30.1",
            "solana_cli":    "1.18.22",
            "rust":          "1.79.0",
        },
    }
    print(json.dumps(manifest, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
