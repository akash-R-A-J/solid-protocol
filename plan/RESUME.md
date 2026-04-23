# Resume -- where to pick up next session

Living handoff doc. Read this first when starting a new session.
Updated at the end of each session; the last-updated line is
authoritative.

- **Last updated:** 2026-04-24 end-of-session (Phase 2 CLOSED)
- **Current branch:** `main`
- **Current phase:** Phase 2 closed; Phase 3 open
- **Discipline in force:** root-cause only, no regressions, no doc
  lies, one source of truth per artifact (see
  `plan/IMPLEMENTATION_PLAN.md` Section 0).

---

## Where we left off

**Phase 2 is closed.**  Both load-bearing soundness items (SEC-004
+ SEC-008) shipped end-to-end: on-chain atomic status transitions,
circuit rev with compressed issuer tree + 6-input nullifier, SDK
migration across every package, backfill script, E2E pipeline
green through `npm run e2e`.  Snapshot:
`sec/audits/2026-04-24_v0.6_phase2_closeout.md`.

Registry state: **22 open / 20 fixed / 42 total**.
Severities: **CRITICAL 0 open**, HIGH 4 open, MEDIUM 9 open,
LOW 5 open, INFO 4 open.

### Commits landed this Phase 2 session

| Commit    | What                                                         |
|-----------|--------------------------------------------------------------|
| `ad944ff` | Prelude: doc drifts, script gaps, safety guards (SEC-039/-040/-042 fixed). |
| `a9f0b9d` | ADR-0014: compressed issuer tree with BJJ-binding leaf.      |
| `4fba821` | CI harness: circuit_witness_tests + e2e_localnet.            |
| `58afb93` | Impl 1: IssuerTreeBinding PDA + solid-light parser.          |
| `73871dc` | Impl 2: circuit rev (issuer-tree inclusion + 6-input nullifier) + zk-verifier. |
| `df33ffe` | Impl 3a: 6-input nullifier across Rust/WASM/TS + IssuerAccount fields. |
| `167138e` | Impl 3b: atomic status-transition hooks + leaf lifecycle.    |
| (this commit) | Impl 3c: SDK + E2E pipeline + backfill + Phase 2 close-out. |

### Host-side baseline (green at close-out)

```
cargo test -p solid-core  --lib    ->  44/44
cargo test -p solid-light --lib    ->  25/25
cargo test -p zk-verifier --lib    ->  11/11
python3 scripts/check_program_ids.py  -> consistent
```

### Open registry (by priority)

```
HIGH (4 open)
  SOLID-SEC-006  VK overwrite has no freeze-gate
  SOLID-SEC-007  BJJ pubkeys not subgroup-checked
  SOLID-SEC-010  Cross-language vectors narrow (covers 3 of ~10 primitives)
  SOLID-SEC-012  Trusted setup single-party (mainnet blocker)

MEDIUM (9 open)
  -013..-019, -021, -034  (governance + throughput + schema-hash
                            re-assertion + fraud-proof seed check)
LOW (5 open)   -022..-024, -035, -041
INFO (4 open)  -025, -026, -037, -038

See sec/SECURITY_REGISTRY.md for the full detail + remediation plan
on each.
```

---

## Next action (Phase 3 kick-off, sequenced)

### 1. SOLID-SEC-006 -- VK freeze-gate (MEDIUM size)

**Root cause.** `store_verification_key` with `chunk_index=0`
silently overwrites existing VK bytes.  Once the last chunk has
been stored with `finalize=true`, the VK should be immutable unless
the DAO votes to rotate.

**Change set.** Add `verifier_config.vk_finalized: bool` (set on
last chunk with `finalize=true`).  While true, `store_verification_key`
must refuse.  A separate `rotate_verification_key` ix (DAO signer +
48h timelock PDA) unfreezes for the rotation window.

### 2. SOLID-SEC-007 -- BJJ subgroup check at registration

**Root cause.** `register_issuer` accepts any `(bjj_x, bjj_y)` as
the issuer's BJJ pubkey.  A non-subgroup point breaks unlinkability
assumptions and lets an adversary control the small-subgroup
component of ephemeral derivations.

**Change set.** On-chain subgroup check in `register_issuer` via
the `arkworks` BabyJubJub curve primitives (same dep solid-core
already pulls in).  Reject with `InvalidBJJPubKey` if the supplied
point is not of prime-order subgroup `r`.  Rust-only change; no
circuit impact.

### 3. SOLID-SEC-010 -- Extend cross-language vectors

Primitives still missing from `tests/vectors/`:
- Poseidon raw (fields + bytes)
- BJJ keypair generation
- BJJ sign / verify
- `derive_credential_key`
- `computeIdentityState`
- `QueryBuilder._computeContextHash`
- ADR-0014 issuer leaf (already implicitly covered via the WASM
  round-trip, but vector it explicitly)

Extend `crates/solid-core/examples/gen_vectors.rs` +
`tests/vectors/check_vectors.ts`.  Straightforward; blocks external
audit readiness.

### 4. SOLID-SEC-041 -- Content-addressed VK artifact

Commit a `circuits/build/verification_key.sha256` OR require
`SOLID_VK_SHA256` env var for `initialize.ts`.  Either works; the
goal is to fail fast when an operator uploads an old VK against new
circuit code.  One-commit change; pair with the SOLID-SEC-012
multi-party-ceremony attestation work.

### 5. SOLID-SEC-012 -- Multi-party trusted setup

Mainnet blocker.  Must run a real ceremony with 10+ geographically
distributed contributors, each signing an attestation chain.
Artifacts (zkey, ptau, VK) hosted on IPFS + Arweave.  Not
one-sprint scope.

### 6. Governance cluster + LOW/INFO cleanup

SOLID-SEC-013..019, -034.  Squads 3-of-5 multisig, per-issuer stake
vaults, propose/accept authority transfer, etc.  Work separable
into small commits; good "warm-up" work while -012 is in flight.

### 7. External audit

After the items above close, schedule the external audit.  Per
Section 0 non-negotiable #5, mainnet slips one sprint after audit
close-out, not one sprint after submission.

---

## Verification steps on resume

```
cd /Users/rajakash/Desktop/testing/solid-protocol
git fetch origin
git log --oneline origin/main | head -10

cargo test -p solid-core  --lib           # expect 44/44
cargo test -p solid-light --lib           # expect 25/25
cargo test -p zk-verifier --lib           # expect 11/11
python3 scripts/check_program_ids.py      # expect "consistent"
head -3 Cargo.lock                        # must be version = 3
```

---

## Open decisions carried into Phase 3

1. **VK freeze-gate semantics.**  Should rotation require a DAO
   vote + 48h timelock (strongest), or registry-authority multisig
   only?  Ties into the SEC-013 Squads migration.  Decide in the
   ADR for SEC-006.

2. **Cooldown status semantics.**  Phase 2 treats
   `IssuerStatus::Cooldown` as Approved-equivalent for
   proof-verification purposes (tree leaf doesn't change on
   Approved -> Cooldown).  Phase 3 decision: should cooldown
   immediately disable proof verification (stronger guarantee)?
   If yes, `request_withdrawal` also needs to become atomic with a
   tree leaf replace.

3. **`scripts/e2e_localnet` CI gate.**  Landed as scaffolding in
   `4fba821`.  Should it be a hard gate on every push (current),
   or graduated (nightly only)?  Resolution depends on runtime
   cost once we measure it on CI.

---

## Rules of engagement (reprinted)

Full list in `plan/IMPLEMENTATION_PLAN.md` Section 9.  Quick
reference:

1. Root cause only.  No workarounds, no suppression, no
   `--no-verify`, no `#[cfg(feature = "unchecked")]`.
2. No regressions.  Every fix ships a regression gate.
3. No doc lies.  Either the doc matches the code or the doc moves
   to `docs/archive/` with a HISTORICAL banner.
4. One source of truth per artifact.
5. PR review checklist: root cause? regression test? doc truth?
   single source of truth? ADR compliance? registry aligned?

Phase 1 held to all six.  Phase 2 held to all six.  Phase 3 must
too.
