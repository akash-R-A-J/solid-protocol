# Session Log -- 2026-04-30 → 2026-05-01 (e2e first verified: true; LB4/LB5 closures; B13 Option 2)

The session that closed B13 Option 2 + nine P0/P1 audit findings + two
"caller MUST" architectural mistakes (LB4 + LB5).  `npm run e2e` now
prints `verified: true` followed by `ok (replay rejected by nullifier
PDA init constraint)` -- the **first** end-to-end on-chain Groth16
verify in the protocol's history.

Companion documents: `plan/RESUME.md` (forward-looking handoff),
`docs/CU_BUDGET.md` (measured CU baselines + tx hashes for explorer
verification), `sec/SECURITY_REGISTRY.md` (closure entries), this file.

---

## 0. Where we ended

- **HEAD**: `fb447ff` -- "SEC-054 + LB1..LB5: e2e green end-to-end + 9 audit
  findings (P0/P1) closed".  211/211 host tests green.
- **Live edge**: `npm run e2e` returns `verified: true` on a fresh
  localnet validator.  Tx signature for the verify:
  `tVYvkyTt55r8RCf3LhVMTmBrKzr5HFtDJcwKM5tFHXerUaxmQSMsZQoKLR8HXbDMu2gYBjNwxC9gaSpdA1oMmCX`.
- **CU**: every ix lands under its cuIx; largest is `AppendIssuerLeaf`
  at 427,518 CU (47% headroom under the 800K cuIx and ~30% of the 1.4M
  per-tx ceiling).  Full table at `docs/CU_BUDGET.md`.
- **Stack**: no .so frame regressed; heap-allocated `Vec<[u8; 32]>` paths
  use 24-byte descriptors.

---

## 1. P0 / P1 audit findings closed

Each closure ships a regression gate that pins it (CLAUDE.md
non-negotiable #2).  Listed in source-of-truth order.

| ID | Title | Closure shape | Regression gate |
|---|---|---|---|
| SOLID-SEC-054 / B13 Option 2 | `verify_batch_proof` ix data > 1232-byte legacy-tx packet limit | New ix trio: `init_proof_buffer` + `upload_proof_chunk` + `verify_batch_proof_v2` | 6 ProofBuffer slot-partition tests in `programs/zk-verifier/src/lib.rs::tests`; full e2e walks the 4-tx flow |
| SOLID-SEC-058 / CRIT-3 | Off-chain prover artifacts (.wasm/.zkey) fetched from CDN with no SHA-256 gate | New `ts-sdk/packages/sdk/src/artifact_integrity.ts` + sidecar `.sha256` files | 14 unit tests in `tests/unit/artifact_integrity.test.ts` |
| SOLID-SEC-059 / H1 | `update_issuer_tree_root` accepted any caller-pushed root (root-injection primitive) | On-chain Poseidon-Merkle recompute against caller-supplied path; integrity check refuses any mismatch | Atomic-update variant in `append_issuer_leaf` / `revoke_issuer_atomic` / `request_withdrawal_atomic`; e2e regression |
| SEC-059 sibling (schema + global) | Same root-injection vector for `update_tree_root` and `update_global_root` | Same on-chain Poseidon-recompute integrity gate, depth=20 | e2e regression |
| SOLID-SEC-061 / H3 | Stuck-stake post-Cooldown trap for tree-enrolled issuers | New `withdraw_after_revoke` ix with 24h dispute window via `cooldown_ends_at` reuse; new `StakeWithdrawn` event | Anchor account-struct + error-code added |
| SOLID-SEC-062 / H4 | BN254 canonicality on commitments + BJJ x/y | New `is_canonical_bn254_le` in solid-core::poseidon | 10 host tests covering modulus edges + padding-shift |
| SOLID-SEC-063 / H5 | Schema-hash preimage too narrow (collisions on field_names + category) | Widened to 5-input Poseidon over (name_h, version_e, count_e, fnames_h, cat_h) via Merkle-Damgard absorbs (`poseidon_compress_bytes`) | 4 host tests + TS mirror in `@solid-protocol/core::computeSchemaHash` |
| SOLID-SEC-064 / H6 | `verifyOnChain` v0/ALT path returned `verified: true` on reverted tx | Check `confirmTransaction(...).value.err`; throw on revert; defense-in-depth nullifier-PDA assert | Code path + e2e |
| SOLID-SEC-066 / H8 | `SOLID_DEBUG_CIRCUIT_INPUT=1` leaks holder master BJJ key to /tmp | Default redacts secret-bearing fields; opt-in via `SOLID_DEBUG_CIRCUIT_INPUT_INCLUDE_SECRETS=DANGER_I_UNDERSTAND`; mkdtemp 0700 dir + 0600 file | 24 unit tests in `tests/unit/holder_debug_redaction.test.ts` |
| **SOLID-SEC-067 / LB5 (NEW)** | G2 byte-order mismatch: snarkjs JSON dumps F_q² in (real, imag) order; alt_bn128 syscall expects (imag, real). Pre-fix every G2 component (VK β/γ/δ + proof B) was on the wrong field-extension basis | Swap on encode in `serializeG2` (initialize.ts) + `formatProofForSolana` (holder/src/index.ts) | New host integration test `programs/zk-verifier/src/lib.rs::tests::groth16_host_verify_round_trip` consumes `tests/fixtures/groth16_e2e_proof.json` and runs `verify_groth16_proof` on host |

The atomic Poseidon-recompute design (LB4) folded SEC-045 (CRIT-2) +
SEC-059 (H1) + SEC-060 (H2) into a single architectural fix: the
binding stores the **Poseidon root** the in-circuit `MerkleInclusion`
template (`Poseidon(2)`) consumes; SPL AC's Keccak tree stays as the
leaf-presence ledger only.  The original audit's "read SPL AC header"
recommendation was the wrong hash family for this protocol.

---

## 2. Latent bugs surfaced by reaching on-chain Groth16 (LB1 .. LB5)

Each was dormant because pre-B13 no proof had ever reached the
`verify_batch_proof` handler.  Listed in order of discovery.

- **LB1**: Anchor 0.30.1's `Vec<[u8; 32]>` BorshDeserialize fails on BPF.
  Fix: change ix arg to `Vec<u8>`, chunk in handler.  Original 2026-04-29
  session log §1.3.
- **LB2**: SEC-005 timestamp slot 31 read as LE u64 from bytes [0..8],
  but the SDK BE-encodes every public input.  Handler now reads
  `from_be_bytes(ts_bytes[24..32])`, zero-checks [0..24].  2026-04-29
  session log §1.3.
- **LB3**: Reconstructed Merkle slots (1, 2..5, 6..9, 10) stored on-chain
  in LE byte form (matching SDK `bufToDecimal` LE-decode) but Groth16
  expects BE.  Handler byte-reverses on copy.  `verifierAddress` (29) is
  the exception (BE on both sides per SOLID-SEC-031).  2026-04-29
  session log §1.3.
- **LB4 (this arc)**: The IssuerTreeBinding (and SchemaTreeBinding,
  GlobalStateBinding) must store the **Poseidon root** the circuit's
  `MerkleInclusion` consumes, NOT the SPL AC tree's Keccak root.  The
  earlier SEC-045 / CRIT-2 fix mistakenly read the SPL AC active root
  via `read_spl_ac_active_root_d16_b64` and wrote it to the binding;
  on-chain Groth16 then operated on a Keccak-derived root the witness
  never committed to.  Closed via the atomic Poseidon-recompute design:
  each state-change ix (`append_issuer_leaf`, atomic revoke /
  withdrawal, `update_*_root`) computes the new Poseidon root on-chain
  from a caller-supplied path and verifies via the existing in-circuit
  template's Poseidon family.
- **LB5 / SOLID-SEC-067 (this arc)**: groth16-solana's G2 byte order is
  (imag, real) per Solana's alt_bn128 syscall convention; snarkjs JSON
  uses (real, imag).  The pre-fix `serializeG2` and `formatProofForSolana`
  used snarkjs ordering -- every G2 component was on the wrong basis;
  on-chain pairing always rejected.  Confirmed against
  `groth16-solana 0.2.0`'s own canonical reference parser
  `parse_vk_to_rust.js`, which does
  `Array.from(le(c0)).concat(Array.from(le(c1))).reverse()` (the
  `.reverse()` over the concatenation produces `[c1_BE, c0_BE]` =
  imag-first).  Fix lands in `scripts/initialize.ts::serializeG2` +
  `ts-sdk/packages/holder/src/index.ts::formatProofForSolana`.  Host
  regression gate is the new `groth16_host_verify_round_trip`
  integration test which consumes a captured fixture and runs
  `verify_groth16_proof` (the same fn the on-chain handler calls)
  on host.

---

## 3. Architectural decisions ledger

Decisions made this session that should NOT be re-litigated without
explicit reason.

### 3.1 The IssuerTreeBinding stores the Poseidon root, not SPL AC's Keccak

The circuit's `MerkleInclusion` template (`circuits/lib/merkle_inclusion.circom`)
uses `Poseidon(2)` for level-pair hashing.  The on-chain SPL AC tree
uses Keccak for its concurrent-merkle-tree header.  These are
fundamentally different hash families.  The IssuerTreeBinding can
honestly track **one** of them.

**Decision**: it tracks the Poseidon root, computed on-chain via
`solid_light::cpi_helpers::compute_poseidon_merkle_root` from a
caller-supplied path.  The integrity check refuses any caller-pushed
root that doesn't recompute from a real Poseidon path.  SPL AC stays
as the on-chain leaf-presence ledger; its root is opaque to the
verify path.

**Why**: the circuit is the source of truth for soundness (CLAUDE.md
L2).  The audit's "read SPL AC header" recommendation conflated two
trees that share leaves but live in different hash families.

**Trade**: caller has to compute the Poseidon path off-chain and
supply it both as ix-arg path (for our on-chain recompute) AND as
SPL AC path siblings via `remaining_accounts` (for the SPL AC CPI).
Two paths, sibling-by-sibling different (Poseidon vs Keccak).
`@solid-protocol/light::LocalReplicaAdapter` produces the Poseidon
path; the SPL AC path is fetched via `getProof` RPC.

**If reopened**: the only architectural alternative is to drop SPL AC
entirely and use a Solana-program-managed Poseidon-Merkle tree.  That's
a significant rework; not in scope for this audit close-out.

### 3.2 cuIx pinned per ix family, not per-call

Three distinct cuIx tiers based on ix workload:

- **200K (Anchor default)**: light ixs (init, status flips, vote, etc.).
- **400K**: ixs with 1-2 nested Poseidon absorbs (`RegisterSchema`,
  `RegisterIssuer` bypass-build).
- **800K**: ixs with depth-16/20 Poseidon-Merkle recompute or Groth16
  pairing (the atomic family + `VerifyBatchProofV2`).

The CI gate (SEC-046) re-measures every ix on each PR and fails on
regression vs `docs/CU_BUDGET.md` baselines.

### 3.3 The SDK no longer accepts caller-trusted roots

Pre-fix, the SDK pushed a Poseidon root via `update_issuer_tree_root`
(or its schema/global siblings) and the on-chain handler accepted it
verbatim from a single-key authority.  Post-fix, the on-chain handler
runs Poseidon recompute against a caller-supplied path and refuses any
mismatch.  Single-key compromise can still produce a CONSISTENT push
(an attacker controlling the authority key can compute a path against
a leaf they control), but a fabricated root unattached to any leaf
state is no longer possible.  SEC-043 (single-key blast radius) is
still open; the multi-party authority refactor is queued.

---

## 4. What was learned (M3 carryover from 2026-04-29)

This session re-ran into the same anti-patterns the 2026-04-29 session
flagged.  Concrete corrections applied:

### 4.1 Don't speculate; isolate empirically

When the on-chain Groth16 first failed, I jumped to "G2 swap is the
fix" without proving it.  The user pushed back twice before I built
the host-side `groth16_host_verify_round_trip` integration test.  The
right move from minute one would have been: capture the fixture,
build the host test, and let it tell me if the fix is right.

**New rule for this codebase**: when on-chain Groth16 fails, the
**first** action is always: dump the fixture from a successful local
verify, run the host test, observe the result, then fix.  Do NOT
modify on-chain serialisation without running the host test first.

The host test is at
`programs/zk-verifier/src/lib.rs::tests::groth16_host_verify_round_trip`.
Run with:
```
cargo test -p zk-verifier --lib groth16_host_verify_round_trip -- --nocapture
```

### 4.2 Don't fight validator state -- restart it cleanly

Several arcs this session failed because the on-chain state was
stale from a prior partially-completed run (VK chunks at
`next_vk_chunk > 0`, phantom verifier_config, etc.).  Each time I
tried to "work around" it (re-run init, etc.) I lost ~10 minutes.

**New rule**: when the validator state is uncertain, kill it, wipe
`test-ledger`, restart with `--reset`.  A clean validator costs ~14
seconds; chasing dirty-state ghosts costs ~10 minutes per cycle.

### 4.3 Run long ops in the background, not under a timeout

`npm run e2e` takes ~5 minutes (100-slot flash-loan cool-off + 120s
voting period + 4-tx verify chain).  A 10-minute Bash timeout cut
off every run that included redeploy + e2e in the same chain.

**New rule**: `npm run e2e` always runs via Bash `run_in_background`
plus a `Monitor` on the log file with a filter for the success
markers.  Never under a foreground timeout.

---

## 5. State of the working tree

Committed to `fb447ff`:
- 28 modified source files + 8 new files (audit doc archive, fixture,
  artifact-integrity module, two unit test files, two new sec audits,
  options archive doc, session log).
- Removed 7 obsolete tests (`compute_concurrent_merkle_root_keccak`
  helper deleted; the function was on the wrong hash family for the
  binding).

Untracked / out-of-scope:
- `solid/` -- empty nested `.git` directory; accidental from an earlier
  session.  Recommend `rm -rf solid/` once verified safe to delete.

---

## 6. Pickup tomorrow

- **Task #12: M2-M11 batch**.  No-ceremony correctness fixes from the
  2026-04-29 synthesis audit:
  - M2: `set_issuer_tree_binding_status` instant-freeze gap → 1h
    timelock + emergency-signer split.
  - M4: `global_tree` field lacks `seeds::program` constraint.
  - M5: VK parser tolerates `nr_ic != MAX_IC`; tighten to exact match.
  - M6: symmetric timestamp clamp `now ± skew` → one-sided + drift.
  - M7: `checkIssuerStatus` walks Borsh by hand → use Anchor
    `BorshAccountsCoder.decode`.
  - M8: `ISSUE_CREDENTIAL_DISCRIMINATOR` hardcoded with no recompute
    test (sibling class to SEC-049) → add gate.
  - M9: SEC-015 trust-anchor tier asymmetry → mutual floor.
  - M10: `SOLID_CONFIG.PUBLIC_INPUTS = 31` stale post ADR-0014 → 32.
  - M11: `parseCredentialIssuedEvent` skips event-discriminator
    validation.
  - (M3 batches with the Lane B circuit revision; not in M2-M11.)
- **Task #14: Lane B circuit revision** (CRIT-1, CRIT-1b, M1 / SEC-048
  Option E, H7, M3, SEC-051, L3, SEC-006 Part 2).  One ceremony.
- **Task #15**: drop `sec007-skip-onchain` once SEC-048 Option E lands.
- **Task #16**: re-validate e2e post-circuit revision; re-baseline CU.
- **CI gate (SEC-046)**: `.github/workflows/ci.yml::cu_regression`
  re-measures every ix per PR vs `tests/cu_baselines.json` (1.10x
  tolerance).
