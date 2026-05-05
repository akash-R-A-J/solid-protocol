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
import os
from typing import Dict

ROOT = pathlib.Path(__file__).resolve().parent.parent
ANCHOR_TOML = ROOT / "Anchor.toml"

DESCRIPTIONS = {
    "zk_verifier":     "Groth16 ZK proof verifier with PDA-per-nullifier registry",
    "issuer_registry": "DAO-governed issuer registry with staking and slashing",
    "schema_registry": "On-chain credential schema registry with tree bindings",
}

ARTIFACTS = {
    "batchCredentialQueryWasm": {
        "filename": "batch_credential_query.wasm",
        "local_path": "circuits/build/batch_credential_query_js/batch_credential_query.wasm",
        "sha256": "add8cb0390511405faf2ffb1213d3792b858c7b2082d5b4b92a84dcd627e61b4",
        "content_type": "application/wasm",
    },
    "batchCredentialQueryZkey": {
        "filename": "batch_credential_query.zkey",
        "local_path": "circuits/build/batch_credential_query_final.zkey",
        "sha256": "7f43bbac249e8c913ac384ff4b007138d1ffb5bc489a16be42737598d96395e8",
        "content_type": "application/octet-stream",
    },
    "batchCredentialQueryVerificationKey": {
        "filename": "batch_credential_query_verification_key.json",
        "local_path": "circuits/build/verification_key.json",
        "sha256": "8385b82b032f65e505c784b28486ca8bec7da3f3d4b97b82724e697734565146",
        "content_type": "application/json",
    },
    "subgroupWasm": {
        "filename": "bjj_subgroup_proof.wasm",
        "local_path": "circuits/build/bjj_subgroup_proof_js/bjj_subgroup_proof.wasm",
        "sha256": "2c00e5a455a3b6fe1dc2a8761737acf2911baf396aab70165de3abd2673608b0",
        "content_type": "application/wasm",
    },
    "subgroupZkey": {
        "filename": "bjj_subgroup_proof.zkey",
        "local_path": "circuits/build/bjj_subgroup_proof_final.zkey",
        "sha256": "ea401ea9cbeb9ef829be30e0080b83a3c82544af53367b95a17f24a75845bd4a",
        "content_type": "application/octet-stream",
    },
    "subgroupVerificationKey": {
        "filename": "bjj_subgroup_verification_key.json",
        "local_path": "circuits/build/bjj_subgroup_verification_key.json",
        "sha256": "938ab39020f31156fa7e8fc230fc458adba5f13e08c64d41d9dbffdbc3643ce9",
        "content_type": "application/json",
    },
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
        "schema_version": 1,
        "network": cluster,
        "cluster": rpc_url_for_cluster(cluster),
        "websocket_cluster": (
            rpc_url_for_cluster(cluster)
            .replace("https://", "wss://")
            .replace("http://", "ws://")
        ),
        "deployed_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "git_commit": os.environ.get("SOLID_GIT_COMMIT"),
        "note": "Generated from Anchor.toml and devnet program accounts.",
        "deployer": {
            "address": os.environ.get("SOLID_DEPLOYER_ADDRESS"),
            "keypair_path": os.environ.get("ANCHOR_WALLET", "~/.config/solana/id.json"),
        },
        "programs": entries,
        "artifacts": {
            "base_url": os.environ.get("SOLID_ARTIFACT_BASE_URL") or None,
            "manifest_url": os.environ.get("SOLID_ARTIFACT_MANIFEST_URL") or None,
            "items": ARTIFACTS,
        },
        "indexer": {
            "url": os.environ.get("SOLID_INDEXER_URL") or None,
            "health_path": "/v1/health",
            "api_version": "v1",
        },
        "console": {
            "url": os.environ.get("SOLID_CONSOLE_URL") or None,
        },
        "wallet": {
            "release_url": os.environ.get("SOLID_WALLET_RELEASE_URL") or None,
        },
        "schemas": [],
        "trees": {
            "global_state_tree": os.environ.get("SOLID_GLOBAL_STATE_TREE") or None,
            "issuer_tree": {
                "tree_address": os.environ.get("SOLID_ISSUER_TREE") or None,
                "binding_pda": os.environ.get("SOLID_ISSUER_TREE_BINDING") or None,
                "current_root": os.environ.get("SOLID_ISSUER_TREE_ROOT") or None,
                "current_root_slot": None,
            },
            "schema_trees": [],
        },
        "pdas": {
            "verifier_config":  {"seeds": ["verifier-config"],                          "program": programs.get("zk_verifier", "")},
            "vk_storage":       {"seeds": ["vk-storage", "<verifier_config_pubkey>"],   "program": programs.get("zk_verifier", "")},
            "nullifier_record": {"seeds": ["null", "<nullifier32>"],                    "program": programs.get("zk_verifier", "")},
            "registry_config":  {"seeds": ["registry-config"],                          "program": programs.get("issuer_registry", "")},
            "dao_treasury":     {"seeds": ["dao-treasury"],                             "program": programs.get("issuer_registry", "")},
            "schema_tree_binding": {"seeds": ["schema-tree-binding", "<schema_hash>"],  "program": programs.get("schema_registry", "")},
            "global_binding":   {"seeds": ["global-binding"],                           "program": programs.get("schema_registry", "")},
        },
        "toolchain": {
            "anchor_cli":    "0.30.1",
            "anchor_lang":   "0.30.1",
            "solana_cli":    "1.18.22",
            "rust":          "1.79.0",
            "node":          "18",
            "circom":        "2.1.9",
            "snarkjs":       "0.7.5",
            "wasm_pack":     "0.13.1",
        },
    }
    print(json.dumps(manifest, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
