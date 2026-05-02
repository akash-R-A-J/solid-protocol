# Session log -- 2026-05-02 (SEC-048 Phase E close + SEC-083 hardening)

## TL;DR

SOLID-SEC-048 closed end-to-end via Phase E (Option B): subgroup-circuit Groth16 verify wired into `register_issuer`; `sec007-skip-onchain` Cargo feature DELETED; `Sec007Bypass` event DELETED; e2e green on the no-bypass build with `verified: true` + replay-rejection. Sibling SOLID-SEC-083 (HIGH; auth-race hardening on `init_subgroup_verifier`) discovered + closed in the same arc.

## Commits landed (local main, NOT YET PUSHED)

| Sha       | What                                                                                                                                                |
|-----------|-----------------------------------------------------------------------------------------------------------------------------------------------------|
| `cbbe088` | E.1 Lift Groth16 verify primitives to `solid-light::groth16` const-generic over N (public-input count)                                              |
| `87cdd34` | E.2 `SubgroupVerifierConfig` PDA + chunked-upload + 48h-timelock-rotation ix family in issuer-registry                                              |
| `0a97099` | E.3 `register_issuer` consumes 256-byte subgroup_proof + on-chain Groth16 verify; `sec007-skip-onchain` Cargo feature + `Sec007Bypass` event DELETED |
| `30343b5` | E.4 SDK + scripts: `generateSubgroupProof` helper; `scripts/initialize.ts` step [8/8] uploads + finalizes subgroup VK; `scripts/bootstrap_issuer.ts` generates + passes the proof |
| (pending) | E.6 SOLID-SEC-083 fix + process.exit fixes + cu_baselines refresh + docs sweep                                                                       |

## E2E receipt

```
$ npm run e2e   # exit 0
... full pipeline ...
[4/4] Submitting verify_batch_proof...
   ix.data.length = 972 (expected 972)
   ix.keys.length = 11 (expected 11)
   verified: true
   tx:       3NkZqYjYDdSwrEveqYJMZaDPuadMiQoJ79bwjbFzPs9B1CUHWZTqeUyehJW99mTmMRaxgfzm4NnM1iYmYtE1G5k4
Replay test (should fail)...
   ok (replay rejected by nullifier PDA init constraint)

Done.
$ solana confirm 3NkZqYjYDdSwrEveqYJMZaDPuadMiQoJ79bwjbFzPs9B1CUHWZTqeUyehJW99mTmMRaxgfzm4NnM1iYmYtE1G5k4 \
                  --url http://127.0.0.1:8899
Finalized
```

## CU baselines refreshed (post no-bypass build)

| Instruction        | Pre (bypass)   | Post (Phase E) | Note                                                                                   |
|--------------------|----------------|----------------|----------------------------------------------------------------------------------------|
| `RegisterIssuer`   | 76,107 CU      | **180,037 CU** | +103,930 CU for on-chain Groth16 N=2 verify (subgroup); 35% of 500K cuIx               |
| `VerifyBatchProofV2`| 319,815 CU    | 322,356 CU     | 1.008x baseline; clean within 1.10x tolerance                                           |
| All 21 ixs         | -              | within 1.10x   | SOLID-SEC-046 CI gate clean; baselines pinned in `tests/cu_baselines.json` (2026-05-02) |

The `RegisterIssuer` CU is much lower than the predicted 365-400K because:
1. The cheap `is_on_curve + !is_identity` pre-filter (~3.5K CU) catches obvious-bad inputs before the heavy Groth16 verify fires.
2. The N=2 subgroup-VK Groth16 verify is much cheaper than the N=32 batch-VK verify -- fewer IC scalar muls (subgroup VK has 3 IC points vs batch VK's 33).

## Findings discovered + closed THIS arc (not just SEC-048)

### SOLID-SEC-083 (HIGH; closed 2026-05-02 in the same arc)

`init_subgroup_verifier` was a first-caller-wins singleton with no constraint tying its authority to `registry_config.authority`. On a fresh public cluster, an attacker could front-run the operator's `scripts/initialize.ts` step [8/8], become the `SubgroupVerifierConfig.authority`, then upload a permissive subgroup VK that admits any pubkey -- nullifying the SEC-048 Phase E.3 soundness gate. Closed by adding `registry_config: Account<'info, RegistryConfig>` to `InitSubgroupVerifier` + the constraint `authority.key() == registry_config.authority @ ErrorCode::Unauthorized`.

Source-level pin in `programs/issuer-registry/src/lib.rs::tests::sec_083_unauthorized_error_code_pinned`. Bankrun integration test (`tests/integration/12_subgroup_vk_init_authority_race.test.ts`) is a TODO under BACKLOG #7.

### Three workflow root causes diagnosed empirically (saved as memory entries)

These are NOT code bugs; they're discipline fixes earned by paying for the same kind of confusion repeatedly. Each has its own memory entry under `~/.claude/projects/.../memory/`.

1. **`| tail -N` buffers stdin until EOF.** Piping a long-running command through `tail -120` produces an EMPTY output file for the entire run -- tail buffers everything until upstream EOF. I mistook this for "silent crash" twice this session. Saved as `feedback_pipe_tail_buffering.md`.

2. **e2e needs a CLEAN slate after any program-code change.** Three independent stores (validator persistence in `test-ledger/`, cached `state.json`, IDL JSONs in `target/idl/`) can each silently desync from new program code. After my SOLID-SEC-083 fix added `registry_config` to `InitSubgroupVerifier`, I forgot to regen the IDL -- the TS client serialized accounts in OLD order, the on-chain handler read the wrong account at the wrong index, the constraint failed, and `isAlreadyInitialised` misclassified the resulting `Unauthorized` error as "already active". Saved as `feedback_e2e_clean_slate.md`.

3. **snarkjs leaves worker_threads alive.** `snarkjs.groth16.fullProve` spawns N worker_threads (one per CPU core) and does NOT terminate them after proof generation. Combined with @solana/web3.js Connection's HTTP-keepalive socket, the Node event loop never drains -- `bootstrap_issuer.ts` hung at 0% CPU for 27+ minutes after `Done. Summary:`. Diagnosed via `process._getActiveHandles()` showing 10 MessagePort + 1 TCP Socket. Fix: `process.exit(0)` after `main()` succeeds (standard pattern for snarkjs-using one-shot scripts; same fix applied to `prove.ts`). Saved as `feedback_snarkjs_workers_hang.md`.

## Test counts at session close

| Crate / package         | Tests | Delta vs pre-Phase-E |
|-------------------------|-------|----------------------|
| `solid-core` --lib      | 73    | unchanged            |
| `solid-light` --lib     | 68    | +14 (Groth16 module + cross-instantiation gates) |
| `zk-verifier` --lib     | 30    | -12 (moved to solid-light) + 1 round-trip kept   |
| `issuer-registry` --lib | 33    | +10 (E.2 SubgroupVerifierConfig + E.3 round-trip + SEC-083 pin) |
| `schema-registry` --lib | 30    | unchanged            |
| Circuits                | 45    | unchanged            |
| **Total**               | **279** | **+12** (after the moves)                  |

All 279 host + circuit tests green.

## What's NOT done this session (deferred)

- **Multi-party trusted setup ceremony for the subgroup VK** (SOLID-SEC-012 + SEC-048-mainnet). The local single-party ceremony at `circuits/trusted_setup/` is testnet-only; mainnet ships AFTER a multi-party ceremony.
- **Bankrun integration test for SEC-083** (`tests/integration/12_subgroup_vk_init_authority_race.test.ts`). Source-level pin landed; live constraint test joins BACKLOG #7 02-11.
- **Extend `circuits/scripts/setup.js` to write `<circuit>.{wasm,zkey}.sha256` sidecars automatically.** Currently only the VK sidecar is written; the wasm/zkey sidecars must be generated manually (or by `shasum -a 256 ... > ....sha256`). This affects both batch + subgroup circuits. Tracked as task #8 in the session task list.
- **`@solid-protocol/holder`'s `generateBatchProof` and `prove.ts` already had the same snarkjs hang.** Fixed `prove.ts` here; `generateBatchProof` is library code (no `process.exit` -- it's the caller's responsibility).

## Pickup next session

1. **Push the local commits** (`cbbe088` + `87cdd34` + `0a97099` + `30343b5` + the pending E.6 commit) once the next session reviewer signs off.
2. **Multi-party ceremony coordination** for both the batch circuit and the subgroup circuit. Same shared PTAU + same contributor pool, two consecutive Phase-2s in one session.
3. **SEC-006 Part 2 + SEC-051 batch-circuit changes** (one-line constraint additions) batched with the multi-party ceremony.
4. **ts-sdk jest baseline** (BACKLOG Tier 2 #11). Auditors will flag jest configured but zero `.test.ts` files; ~1 week of focused work.
5. **Integration tests 02..11** (BACKLOG Tier 2 #7), including the SEC-083 bankrun test.
6. **`circuits/scripts/setup.js` sidecar emission for wasm + zkey** (task #8 above).

## Discipline reminders enforced this session

- L1 (cascade is the diagnostic): when fixing the IDL drift surfaced 3+ apparent failures (account constraint, "Account does not exist", false "already active"), I stopped editing and went up the stack -- found ONE missing `node scripts/build_idls.mjs`.
- L4 (regression gate FIRST): every Phase E commit landed its tests + size-pin assertions in the same commit.
- L7 (CU/byte ledger up-front): the predicted 365-400K CU for `RegisterIssuer` was used to size the 500K cuIx before measure_cu confirmed the actual 180K.
- L8 (don't speculate; observe): when bootstrap_issuer.ts hung, I instrumented `process._getActiveHandles()` instead of guessing snarkjs vs websocket. Got the answer empirically: 10 MessagePort + 1 Socket.
- L9 (atomic per-finding commits): SEC-048 Phase E shipped as 5 separate commits (E.1..E.4 + the pending E.6); SEC-083 will land in its own commit alongside the docs sweep.
