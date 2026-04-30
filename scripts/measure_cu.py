#!/usr/bin/env python3
"""SOLID-SEC-046 CU regression gate.

Re-measures the on-chain compute-unit consumption for every instruction
exercised by `npm run e2e` and asserts that none exceeds its baseline by
more than `tolerance_factor` (default 1.10 = +10%).

Workflow assumption:
    1. A localnet validator has the three SolID programs deployed.
    2. `npm run e2e` has just been run successfully against this validator
       (so the most-recent txs per program are the e2e txs).
    3. This script then queries `solana transaction-history` per program
       and `solana confirm -v` per tx to extract the consumed CU + the
       Anchor `Instruction:` log line, matches against the baseline, and
       fails the build if any ix is over baseline * tolerance.

Baseline source: `tests/cu_baselines.json` (machine-readable companion
to `docs/CU_BUDGET.md`).  Every entry is anchored to a real tx
signature so an auditor can reproduce.

Exit codes:
    0  -- every measured ix at or below baseline * tolerance
    1  -- at least one ix exceeded the tolerance
    2  -- runtime error (validator unreachable, missing tx, etc.)

Usage:
    # CI: assumes solana CLI on PATH + validator at $SOLID_RPC_URL
    python3 scripts/measure_cu.py
    # Override tolerance (e.g. tighter gate for release builds)
    python3 scripts/measure_cu.py --tolerance 1.05
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path
from typing import Dict, List, Optional, Tuple


REPO_ROOT = Path(__file__).resolve().parent.parent
BASELINE_PATH = REPO_ROOT / "tests" / "cu_baselines.json"

# Regex: program <id> consumed <N> of <M> compute units
RE_CONSUMED = re.compile(
    r"Program\s+([1-9A-HJ-NP-Za-km-z]{32,44})\s+consumed\s+(\d+)\s+of\s+(\d+)\s+compute units"
)
# Regex: Program log: Instruction: <CamelCaseName>
RE_INSTRUCTION = re.compile(r"Program log:\s+Instruction:\s+(\w+)")


def run_solana(*args: str) -> str:
    """Invoke `solana` and return stdout.  Raises on non-zero exit."""
    result = subprocess.run(
        ["solana", *args],
        capture_output=True,
        text=True,
        timeout=60,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"solana {' '.join(args)} failed (exit {result.returncode}): "
            f"stderr={result.stderr.strip()!r}"
        )
    return result.stdout


def transaction_history(program_id: str, limit: int = 30) -> List[str]:
    """Return up to `limit` most-recent tx signatures for `program_id`."""
    out = run_solana("transaction-history", program_id, "--limit", str(limit))
    sigs = []
    for line in out.splitlines():
        line = line.strip()
        if not line or line.startswith(("TS:", "Block")) or "transactions found" in line:
            continue
        # Signatures are 87-88 base58 chars.  Be liberal but reject obvious
        # noise (e.g. error lines).
        if 64 <= len(line) <= 100 and re.fullmatch(r"[1-9A-HJ-NP-Za-km-z]+", line):
            sigs.append(line)
    return sigs


def confirm_tx(sig: str) -> Tuple[Optional[str], Dict[str, Tuple[int, int]]]:
    """Parse `solana confirm -v <sig>`.

    Returns `(top_level_instruction_name, {program_id: (consumed, limit)})`.
    The top-level instruction name is the FIRST `Program log: Instruction:`
    line in the log block; the per-program CU map is the LAST `consumed X
    of Y compute units` line per program (deepest CPI return last).
    """
    out = run_solana("confirm", "-v", sig)
    instr: Optional[str] = None
    cu_map: Dict[str, Tuple[int, int]] = {}
    for line in out.splitlines():
        line = line.strip()
        m_ix = RE_INSTRUCTION.search(line)
        if m_ix and instr is None:
            instr = m_ix.group(1)
        m_cu = RE_CONSUMED.search(line)
        if m_cu:
            program_id = m_cu.group(1)
            consumed = int(m_cu.group(2))
            limit = int(m_cu.group(3))
            cu_map[program_id] = (consumed, limit)
    return instr, cu_map


def load_baselines() -> Dict:
    if not BASELINE_PATH.exists():
        print(f"ERROR: baseline file missing at {BASELINE_PATH}", file=sys.stderr)
        sys.exit(2)
    return json.loads(BASELINE_PATH.read_text())


def measure_program(
    program_name: str,
    program_id: str,
    expected_ixs: Dict[str, Dict],
    history_limit: int = 30,
) -> List[Tuple[str, str, int, int, str]]:
    """Return a list of (ix_name, baseline_key, measured_cu, baseline_cu, sig)
    matched by walking the program's recent tx history and pairing each
    Instruction occurrence with the corresponding baseline entry in
    declaration order.  Multiple chunks of the same Instruction name (e.g.
    StoreVerificationKey_chunk_1/2/3) are matched in order of appearance
    (oldest -> newest)."""
    sigs = transaction_history(program_id, history_limit)

    # Group baseline keys by their Instruction name.  e.g.
    # "StoreVerificationKey_chunk_1" -> Instruction name "StoreVerificationKey".
    name_to_keys: Dict[str, List[str]] = defaultdict(list)
    for key in expected_ixs:
        # Strip a trailing "_chunk_N" suffix if present.
        m = re.match(r"^(.+?)(?:_chunk_\d+)?$", key)
        ix_name = m.group(1) if m else key
        # If the key has _sec007_skip_onchain or other suffixes, treat as
        # a single Instruction (the on-chain name is just RegisterIssuer).
        # Strip any "_<lowercased_descriptor>" tail that doesn't look like
        # a CamelCase ix name.
        if "_" in ix_name and not ix_name[ix_name.rindex("_") + 1].isupper():
            ix_name = ix_name.split("_", 1)[0]
        name_to_keys[ix_name].append(key)

    # Walk sigs from OLDEST to NEWEST so chunked uploads pair in order.
    sigs_chrono = list(reversed(sigs))

    results: List[Tuple[str, str, int, int, str]] = []
    cursor: Dict[str, int] = defaultdict(int)
    for sig in sigs_chrono:
        ix_name, cu_map = confirm_tx(sig)
        if ix_name is None:
            continue
        if program_id not in cu_map:
            continue
        consumed, _limit = cu_map[program_id]

        # Match against baseline entries for this Instruction name.
        # Order-preserving: chunk_1 first, chunk_2 second, etc.
        keys = name_to_keys.get(ix_name, [])
        idx = cursor[ix_name]
        if idx >= len(keys):
            continue  # Already matched all expected occurrences for this name.
        baseline_key = keys[idx]
        cursor[ix_name] = idx + 1

        baseline_cu = expected_ixs[baseline_key]["consumed_cu"]
        results.append((ix_name, baseline_key, consumed, baseline_cu, sig))

    return results


def main() -> int:
    parser = argparse.ArgumentParser(description="SEC-046 CU regression gate")
    parser.add_argument(
        "--tolerance",
        type=float,
        default=None,
        help="multiplicative tolerance (default: from baseline JSON, typically 1.10)",
    )
    parser.add_argument(
        "--rpc-url",
        default=os.environ.get("SOLID_RPC_URL", "http://127.0.0.1:8899"),
        help="Solana RPC URL (default: http://127.0.0.1:8899 or $SOLID_RPC_URL)",
    )
    args = parser.parse_args()

    # Tell solana CLI which cluster to talk to.
    os.environ["SOLANA_URL"] = args.rpc_url

    baselines = load_baselines()
    tolerance = args.tolerance or float(baselines.get("_tolerance_factor", 1.10))

    print(f"SOLID-SEC-046 CU regression gate")
    print(f"  RPC URL  : {args.rpc_url}")
    print(f"  Tolerance: x{tolerance}")
    print(f"  Baselines: {BASELINE_PATH.relative_to(REPO_ROOT)}")
    print()

    failures: List[str] = []
    measured_total = 0
    for prog_name, prog_data in baselines["programs"].items():
        prog_id = prog_data["program_id"]
        expected = prog_data["instructions"]
        print(f"== {prog_name} ({prog_id}) ==")
        try:
            results = measure_program(prog_name, prog_id, expected)
        except Exception as e:
            print(f"  ERROR: {e}")
            return 2
        seen_keys = set()
        for ix, baseline_key, measured, baseline, sig in results:
            seen_keys.add(baseline_key)
            measured_total += 1
            ratio = measured / baseline if baseline > 0 else float("inf")
            limit = expected[baseline_key]["cu_limit"]
            status = "OK" if ratio <= tolerance else "REGRESSION"
            print(
                f"  {ix:<32}  consumed={measured:>7}  baseline={baseline:>7}  "
                f"limit={limit:>7}  ratio={ratio:.3f}  [{status}]  {sig[:16]}..."
            )
            if ratio > tolerance:
                failures.append(
                    f"{prog_name}::{baseline_key} consumed {measured} CU, "
                    f"baseline {baseline}, ratio {ratio:.3f} > tolerance {tolerance}"
                )
        # Were any baseline ixs not exercised?
        missing = set(expected.keys()) - seen_keys
        if missing:
            for key in sorted(missing):
                msg = (
                    f"  MISSING: {key} -- no recent tx matched.  "
                    f"Did `npm run e2e` complete?  Reference sig: "
                    f"{expected[key]['tx_signature']}"
                )
                print(msg)
                failures.append(msg)
        print()

    print(f"Measured {measured_total} ixs; {len(failures)} regression(s).")
    if failures:
        print()
        print("FAILED:")
        for f in failures:
            print(f"  - {f}")
        return 1
    print("OK -- every ix within tolerance.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
