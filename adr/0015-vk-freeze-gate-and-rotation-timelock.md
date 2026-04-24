# ADR 0015: VK freeze-gate with 48-hour rotation timelock (SOLID-SEC-006)

- **Status:** Accepted (Phase 3 impl 2 landed on-chain 2026-04-25;
  Part 2 circuit binding deferred to the next trusted-setup cycle)
- **Date:** 2026-04-25
- **Deciders:** founding team; Phase 3 entry review
- **Extends:** ADR-0008 (chunked VK upload).  The chunk pipeline is
  unchanged; ADR-0015 adds a finalize gate and a timelocked rotation
  path on top of it.
- **Affects:** `programs/zk-verifier/src/lib.rs` (`VerifierConfig`
  layout, `store_verification_key`, new instructions
  `finalize_verification_key`, `request_vk_rotation`,
  `cancel_vk_rotation`, `rotate_verification_key`).  No circuit
  change; no trusted-setup regeneration.

## Context

SOLID-SEC-006 (HIGH, open since the v0.3 audit).  Before this ADR,
`store_verification_key` would accept any `chunk_index = 0` write
from the authority and silently overwrite the live VK.  Two concrete
consequences:

1. **Single-key VK swap.**  A compromise of the lone `authority`
   keypair lets the attacker install an arbitrary VK between
   `vk_initialized = true` and the next `verify_batch_proof`.  No
   DAO, no timelock, no observability.  The protocol's entire
   soundness guarantee collapses to "the authority key is not
   compromised".

2. **Truncated-VK finalize.**  The `is_final_chunk` flag on the
   last chunk gated `vk_initialized`, but nothing stopped the
   authority from re-calling `store_verification_key(chunk_index =
   0, ..., is_final_chunk = true)` with a short byte string, leaving
   the VK parsed-correct-but-wrong.  This is strictly worse than a
   full swap because the parser does not reject a short VK that
   happens to pass sanity checks.

The v0.6 deep audit listed SEC-006 as one of two HIGHs that closed
"at the root rather than papered over them" blocks external-audit
readiness (the other being SEC-007, landed 2026-04-25 in Phase 3 impl
1).

## Decision

Two-layer fix: on-chain immutability gate now (Part 1), circuit-bound
rotation counter at the next trusted-setup regeneration (Part 2).

### Part 1 -- on-chain immutability gate (landed 2026-04-25)

Extend `VerifierConfig`:

- `vk_finalized: bool` -- `false` at init; flips to `true` on a
  `finalize_verification_key` call that requires `vk_initialized`.
- `vk_generation: u16` -- starts at 0; bumps atomically on a
  completed `rotate_verification_key`.
- `rotate_request_ts: i64` -- 0 means no pending rotation; non-zero
  is the `Clock::unix_timestamp` at the moment `request_vk_rotation`
  was called.

`store_verification_key` gains a hard guard:

```rust
require!(!config.vk_finalized, ErrorCode::VerificationKeyFinalized);
```

Four new instructions:

- `finalize_verification_key` (authority-only).  Requires
  `vk_initialized && !vk_finalized`.  Sets `vk_finalized = true`.
  Idempotent in the trivial sense: a second call returns
  `VerificationKeyAlreadyFinalized`.

- `request_vk_rotation` (authority-only).  Requires
  `vk_finalized && rotate_request_ts == 0`.  Records
  `Clock::unix_timestamp` into `rotate_request_ts`.  The live VK
  continues to serve verifications during the window.

- `cancel_vk_rotation` (authority-only).  Zeros
  `rotate_request_ts`.  No-op if nothing is pending.

- `rotate_verification_key` (authority-only).  Requires
  `vk_finalized`, `rotate_request_ts != 0`, and
  `Clock::unix_timestamp >= rotate_request_ts +
  VK_ROTATION_TIMELOCK_SECONDS`.  On success, clears
  `vk_initialized` and `vk_finalized`, resets `next_vk_chunk = 0`,
  clears `rotate_request_ts`, bumps `vk_generation`.  `vk_storage.
  data` is NOT wiped here -- the next chunk-0 `store_verification_
  key` write overwrites it atomically in one transaction.

`VK_ROTATION_TIMELOCK_SECONDS = 48 * 3600 = 172_800`.  48 hours
chosen as the industry standard minimum for "compromised authority,
watchers need time to react".  Shorter undermines the observability
argument; longer slows legitimate rotation without adding security.

The timelock alone is not the full defence.  Once SOLID-SEC-043
replaces the single-pubkey authority with a Squads 3-of-5 PDA
signer, the rotation path is (multisig quorum) * (48-hour window).
ADR-0015 composes additively with SEC-043; nothing in this ADR
blocks that upgrade.

### Part 2 -- circuit-bound `vk_generation` (deferred)

Part 1 closes the *silent swap* attack: any swap is now visibly
timelocked.  It does not close the *cross-VK replay* subclass.  An
attacker who captured proofs under VK generation N cannot replay
them against VK generation N+1 because the VK bytes differ, but the
protocol has no cryptographic tie from a proof back to its intended
generation -- any proof that happens to verify under the current VK
is accepted.

The complete fix binds `vk_generation` into the circuit's public-
input contract as an additional input index.  `zk-verifier` reads
its local `VerifierConfig.vk_generation` and rejects any proof whose
`public_inputs[GENERATION_INPUT_INDEX]` does not match.  Holders
query the current generation from on-chain when generating a proof.

This is a circuit change and therefore a new trusted-setup cycle.
Per the v0.6 audit's Phase 3 sequencing, Part 2 lands paired with
the next planned trusted-setup work so only one multi-party ceremony
is required across SEC-006 Part 2 + SEC-010 expansion + any further
constraint additions.

## Consequences

- **Positive (Part 1).**  A compromised authority can no longer
  silently swap the VK.  Any rotation announces itself through
  `request_vk_rotation` 48 hours ahead; watchers / indexers /
  end-users have a window to react.  The `vk_generation` counter
  gives observers a durable anchor for "which VK is live right now".
- **Positive.**  Reuses the existing chunk pipeline.  No VK artifact
  layout change, no circuit change, no trusted-setup change.  Pure
  on-chain addition; deployment is a redeploy of `zk-verifier` with
  a `VerifierConfig` resize migration (pre-mainnet: re-init is
  sufficient since `deployments/devnet.json` has no live bind).
- **Negative (closed by Part 2).**  Cross-VK replay is still
  possible between consecutive generations.  Mitigated in practice
  by the 48-hour window (any captured pre-rotation proof has 48h
  worth of "window of replay" before the old VK is removed), but
  not structurally closed until Part 2 ships.
- **Negative.**  `VerifierConfig::SPACE` grows 49 -> 60 bytes.  Any
  live deployment needs a `resize` migration before upgrading.
  `deployments/devnet.json.deployed_at` is still `null` at the time
  of this ADR, so this is a no-op for now; the mainnet pattern will
  be documented in `docs/UPGRADE_GUIDE.md` before first mainnet
  deploy.
- **Negative.**  A malicious authority can spam
  `request_vk_rotation` + `cancel_vk_rotation` indefinitely without
  cost.  This is not a soundness gap (it only delays a legitimate
  rotation by the attacker), but SOLID-SEC-043's multisig gate
  removes the ability by construction.

## Alternatives considered

- **Redeploy-only rotation.**  Ship without a rotation path and
  require a full program redeploy for any VK swap.  Rejected:
  redeploy needs the program upgrade authority, which is a separate
  concern, and moving VK rotation behind program upgrade conflates
  "cryptographic VK change" with "Anchor bytecode change".  The
  latter is infrequent; the former may need to run on its own
  cadence.

- **DAO-vote gating without timelock.**  A DAO proposal + vote path
  instead of a 48h fixed window.  Rejected for v0.6: there is no
  on-chain DAO for the zk-verifier authority yet (the issuer-
  registry has a DAO but it's scoped to issuer approvals).  Once
  SEC-043 lands and authority becomes a Squads PDA, the timelock +
  quorum combination is the DAO-equivalent gate.  Revisit in SEC-
  013 close-out.

- **Shorter timelock (6h / 12h / 24h).**  Considered; rejected
  because the primary defence is observability, and observability
  needs hours not minutes.  Ethereum DAOs that host similar rotation
  paths (Gnosis Safe, MakerDAO) universally use 24h or longer.

- **Circuit-first (Part 2) immediately.**  Considered; rejected
  because it requires a trusted-setup regeneration that should be
  batched with SEC-010 + any other constraint additions, not spent
  on one finding.  Part 1 closes the silent-swap attack cleanly
  today; Part 2 lands at the next ceremony.

## Regression gates

In `programs/zk-verifier/src/lib.rs` host tests:

- `vk_rotation_not_expired_when_no_pending_request` -- zero-anchor
  state is never "expired".
- `vk_rotation_not_expired_inside_window` -- strict-less-than
  request + (timelock - 1) rejects.
- `vk_rotation_expired_at_and_beyond_timelock` -- boundary and past
  accept.
- `vk_rotation_handles_saturation_safely` -- math does not panic at
  near-i64::MAX request timestamps (practically unreachable but
  guards against future refactor bugs).
- `verifier_config_space_matches_layout` -- SPACE constant equals
  the exact byte count; catches layout drift of the SEC-042 class.
- `vk_rotation_timelock_is_48_hours` -- doc-as-test for the
  constant; future changes are a conscious choice here + ADR.

Integration tests -- full transaction-level coverage of the four
handlers -- will land in `tests/integration/12_vk_rotation.test.ts`
as part of SOLID-SEC-B5 (integration suite 02..11 expansion).

## References

- SOLID-SEC-006 detail in `sec/SECURITY_REGISTRY.md`.
- SOLID-SEC-042 (the doc-drift sibling on the same account type).
- ADR-0008 (chunked upload; extended by this ADR).
- `sec/audits/2026-04-24_v0.6_deep_comprehensive_audit.md` Section
  7.1 item 1 ("VK freeze-gate ... MUST close before external
  audit").
