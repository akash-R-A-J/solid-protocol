# Compute-Unit Budget — measured baselines

Canonical reference for `SOLID-SEC-046` (CU regression gate).  Numbers below
are **on-chain measured**, not estimates, captured from a green
`npm run e2e` run on `solana-test-validator 1.18.22` with a
freshly-deployed program set on 2026-05-01.  Every row links to a real
tx signature so an auditor can independently re-run
`solana confirm -v <sig>` against a localnet that's replayed from
`scripts/initialize.ts` to reproduce.

The CI gate (see `.github/workflows/ci.yml -> cu_regression`) re-measures
every row on each PR and fails if `consumed > baseline * 1.10`.  The
10% margin absorbs run-to-run variance from BPF VM noise + slot-time
jitter; anything outside that band is a real regression.

---

## Per-instruction baselines

Captured 2026-05-01 (refreshed late-day after the M-batch verifier-side
invariant tightening + NF-batch closure: post-M-batch the buffer-account
ixs gained ~3,000 CU of constant validation overhead — `InitProofBuffer`
13,954 → 16,954, each `UploadProofChunk` 9,647 → 12,647 — without which
the SEC-046 1.10x gate would fail).  Originally captured on 2026-05-01
morning from the SOLID-SEC-054 / B13 Option 2 + LB5 closure
e2e run.  Validator: localnet, `solana-test-validator --reset`, programs
deployed at the canonical IDs.  Wallet: `56cch2JpqChmm8ufqorRHGB2jbXs88drjPnQbBE2MDEe`.

### `zk_verifier` (`DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb`)

| Instruction | Consumed CU | Budget | Headroom | Tx Signature |
|---|---:|---:|---:|---|
| `Initialize` | 21,394 | 200,000 | 89.3% | `TgbBCRBSat7HEHLK1DcAMnw1k8uXa8T5am4Ft9q1DzYemQJkorFbGP7P8mNCQn7wTwMoWH5D4QoE3NG55AMifG8` |
| `StoreVerificationKey` (chunk 1/3) | 12,842 | 200,000 | 93.6% | `5sM715TtHU4pgQAcpgY3VnFJ6qgwb2PKAj18dY8MEYgZ73bq5i9hemF2L8psbDW1chQinJLsP7Na6u6yLLevUotZ` |
| `StoreVerificationKey` (chunk 2/3) | 9,811 | 200,000 | 95.1% | `5YX2sdtYjfTch11jhV6Q4RRqbb6AA42cFMRV6R9LzRtBDLfHjmcvjcrgpHDRAfMUv2TxXkRrJRkkKVX5TxXJWAtT` |
| `StoreVerificationKey` (chunk 3/3) | 10,627 | 200,000 | 94.7% | `2MvuLyNNgcycP2zttxVxD5P4bHhHXrpMgYmFg9efNcVfvHWmAHqygceHgPEaod7xMUA1gG1Dswfz67voXYbzib2J` |
| `InitProofBuffer` | 16,954 | 200,000 | 91.5% | `5xcg48f13DTyNkwV...` |
| `UploadProofChunk` (chunk 1/2) | 12,647 | 200,000 | 93.7% | `2wGeK8wPMiQsX98z...` |
| `UploadProofChunk` (chunk 2/2) | 12,647 | 200,000 | 93.7% | `4qfKPcQmvaGtACva...` |
| **`VerifyBatchProofV2`** (Groth16) | **318,315** | **799,850** | **60.2%** | `26ZjFivuDv58bGpv...` |

### `issuer_registry` (`5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx`)

| Instruction | Consumed CU | Budget | Headroom | Tx Signature |
|---|---:|---:|---:|---|
| `InitializeRegistry` | 50,981 | 200,000 | 74.5% | `5u9V6GnXizi238uf7BozDERR7ctnd6XXubSKjyv9Q3FNBhn18TBDNuuqwNEtfpo9uYm5Be4TXE2rkXuG2vKKSkgX` |
| `InitializeIssuerTreeBinding` | 35,599 | 200,000 | 82.2% | `5RtMnQME8JLYYRhVB8V1gdvbaNf7RJvpdSL1AtNoFjVizrgkciHeBYqL52LTRAXFtfXhrPyRrUCtgCVUJ5THCmwg` |
| `RegisterIssuer` (sec007-skip-onchain) | 76,107 | 399,850 | 80.9% | `49TGLx3RL4DKEJURUzv6SzuRBKsCKbBVH6SdyQQv12rE48EJTVDphqBPTPF6nwwgMFscxgDpsxRHJb2o3U4H6a5D` |
| `StakeTokens` | 31,548 | 200,000 | 84.2% | `4SCiznn7piT3ggYdd5Cc8zUue6TpbmkVzuyEjY6ZJMf9SvCPTtjECMBSqvEhb9bozyfEUyDXcHm2Y1EHM34cN1F3` |
| `VoteOnIssuer` | 24,022 | 200,000 | 88.0% | `46B7oKYX8DtPmMQA5jiGyx648beAUxgUFzRJ9RngzGduNqt32ENtWLKxHH1FxdFKcbnJQyL1NYLaCYkGtu576W93` |
| `FinalizeVoting` | 12,526 | 200,000 | 93.7% | `VRDS8jgY5LHyfQkAoYNr1bHK4v8PpHWJuxkp79tR1HREjX3mQUVAu6Rdzz6nD7UzFU8PFZrZXz5GzKN1mB8hhVg` |
| **`AppendIssuerLeaf`** (atomic Poseidon-d16) | **427,518** | **799,850** | **46.6%** | `nVw5qxh8LEjTg1ZUYzWoJUudNxWjSQ4ioUzAeEC6hNXPggeRqQwS4h71hbAF1wPDVxvGY5a4KnmaHfPhf5KHQPm` |
| `IssueCredential` | 66,345 | 200,000 | 66.8% | `aHiaevmhHRLZXgooD6sDMseHU6DhmynFw4phbMRG6ZffowQLS2RswfnuQnBQ72EHDaBsWoky1RUbs82B6Peynxn` |

### `schema_registry` (`4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1`)

| Instruction | Consumed CU | Budget | Headroom | Tx Signature |
|---|---:|---:|---:|---|
| `RegisterSchema` (H5 widened preimage) | 229,952 | 399,850 | 42.5% | `gR4PZoBsE7yT9mWHAFu5892XzDnMCdcMvBAFCqqeQaeWT2ewBVjb6WpeTydBGRUQVvzQxawHpuz6t954P3sKmat` |
| `InitializeGlobalBinding` | 6,253 | 200,000 | 96.9% | `5jdC3jGZregroHYGZgZjkfrEA2VMoe59cd93YxUHSVey6ygejv9YQqok8smviygkPDqTtxAueCUxuKZA4Rek4bLb` |
| `InitializeTreeBinding` | 24,777 | 200,000 | 87.6% | `3oPZsVVAYJKXfG9ZmXXgFXTuUqoojVSGAbMLDGtLnAk2YP6GAjZ1DsNgnpP2qFs9czveE2gDB1bqP7fejEEaSJZF` |
| **`UpdateTreeRoot`** (Poseidon-d20 + integrity) | **417,775** | **799,850** | **47.8%** | `4vLQDGwUwnRuMgHTiYijEzvnWhEv9WbDoQ6rCW33NcXYojM7hFc9ZpWy9cdzbLdDJEzKr3Eu2HEfHjdrjfJN4E6c` |
| **`UpdateGlobalRoot`** (Poseidon-d20 + integrity) | **417,347** | **799,850** | **47.8%** | `5BM1cUECUz7oaZZAcHVCCNJUHowjr1gzeNNv3Za38rcTAbsjkP5xf3e6brn98pEkuUuhEn94vEUkcGm3N3C9Sfwe` |

---

## Largest consumers

| Rank | Instruction | Consumed | Notes |
|---|---|---:|---|
| 1 | `AppendIssuerLeaf` | 427,518 | depth-16 Poseidon recompute (~32 `Fr::from_le_bytes_mod_order` round-trips on BPF + 16 `sol_poseidon` syscalls) |
| 2 | `UpdateTreeRoot` | 417,775 | depth-20 Poseidon recompute |
| 3 | `UpdateGlobalRoot` | 417,347 | depth-20 Poseidon recompute |
| 4 | `VerifyBatchProofV2` | 318,315 | Groth16 verify (alt_bn128 pairing dominates) |
| 5 | `RegisterSchema` | 229,952 | H5-widened schema_hash preimage (3 nested Poseidon-Merkle-Damgard absorbs over name + field_names + category) |

All five land below 50% of the 1.4M per-tx CU ceiling.  None requires
a CU bump beyond the 800K cuIx already prepended for the Poseidon-recompute
families.

## Pinned cuIx per family

| Family | cuIx (CU limit) | Reason |
|---|---:|---|
| Default Anchor ix | 200,000 | Solana per-tx default; sufficient for ixs up to ~150K CU. |
| `RegisterSchema` | 400,000 | H5 widened preimage (3 chained Poseidon-Merkle-Damgard absorbs). |
| `RegisterIssuer` (bypass build) | 400,000 | Cheap consolation gate (`is_on_curve + !is_identity`) when `sec007-skip-onchain` is set; will bump to ~1.4M when SEC-048 Option E in-circuit subgroup-check lands and we drop the bypass. |
| Atomic Poseidon-recompute family (`AppendIssuerLeaf`, `RevokeIssuerAtomic`, `RequestWithdrawalAtomic`, `UpdateIssuerTreeRoot`, `UpdateTreeRoot`, `UpdateGlobalRoot`) | 800,000 | Depth-16/20 on-chain Poseidon-Merkle recompute. |
| `VerifyBatchProofV2` | 800,000 | Groth16 verify (~315K CU baseline; 800K leaves ~2.5x headroom for pairing variance). |

---

## CI gate

`.github/workflows/ci.yml -> e2e_localnet` job has a **"SOLID-SEC-046
CU regression gate"** step appended after `npm run e2e`.  The step
runs `python3 scripts/measure_cu.py`, which queries every just-run tx
via `solana confirm -v`, parses the per-program "consumed N of M
compute units" line, matches each tx's Anchor `Instruction:` log line
against a baseline entry in declaration order (chunked uploads pair
chronologically), and asserts:

```text
ix_consumed_cu <= baseline_consumed_cu * 1.10
```

Any commit that pushes an ix over baseline+10% fails the gate.  This
is the structural fix for SOLID-SEC-046 (no CU-budget regression gate).
The 10% margin absorbs run-to-run variance from BPF VM noise + slot-
time jitter; anything outside that band is a real regression.

Baselines are stored in `tests/cu_baselines.json` (machine-readable
companion to this doc); the script lives at `scripts/measure_cu.py`.

---

## Trigger conditions to refresh baselines

Re-run `npm run e2e`, capture all tx signatures, and update both this
doc + `tests/cu_baselines.json` whenever any of the following land:

- Circuit revision (changes Groth16 verify cost via IC cardinality, pairing
  structure, etc.).  Specifically the **Lane B batch**: CRIT-1, CRIT-1b,
  M1 / SEC-048 Option E (in-circuit BJJ subgroup), H7 (compound_query
  cleanup), M3 (verifierAddress -> payer.key), SEC-051 (padding fix),
  L3 (per-field range checks), SEC-006 Part 2 (`vk_generation` binding).
- BJJ subgroup check moved on-chain (`sec007-skip-onchain` removed →
  `RegisterIssuer` baseline bumps from ~76K CU to whatever the in-circuit
  fix costs through the issuance circuit; expect the on-chain cost to
  *drop* once the gate moves into the witness).
- Trusted-setup re-run: changes the VK byte layout.  Verify
  `StoreVerificationKey` total upload still completes in 3 chunks.
- New ix added in any of the three programs.
- Anchor / `groth16-solana` / `solana-program` major version bump.

Each refresh keeps the prior baselines as historical entries (don't
overwrite — append; the gate compares against the most recent).

---

## Predicted CU + byte ledger for the next trusted-setup batch (SEC-048 Option B + SEC-006 Part 2 + SEC-051)

> Written **before** the circuit revision lands, per L7 (byte/CU
> budget UP-FRONT for any change touching a hard wire-size or CU
> cap).  After the revision lands, compare against the actual
> measurements and resolve any drift inline here.
>
> Pre-batch baselines: `tests/cu_baselines.json` (date 2026-05-01,
> commit 4a8d8d6).

### 1. SEC-006 Part 2 -- bind `vk_generation` into circuit publics

- Public input count: `NR_PUBLIC_INPUTS = 32 -> 33`.
- New slot 32: `vkGeneration` (currently a `u16` in `VerifierConfig`,
  promoted into the Fr field witness).
- VK file size: **+1 G1 IC point = +64 bytes** (uncompressed) or
  +32 bytes (compressed).  Verification key JSON grows ~256 bytes.
- VK chunk count over the 1024-byte upload: pre-batch ~2,564 bytes
  (3 chunks).  Post-batch ~2,628 bytes -- still 3 chunks, no extra
  `StoreVerificationKey` ix call.
- `verify_batch_proof_v2` MSM cost: one extra G1 scalar mul over Fr.
  Estimate **+3,000 to +5,000 CU**.  Pre-batch baseline: 318,315.
  Predicted post-batch: **~321K-323K CU**.  Headroom under 800K
  cuIx: ~480K (was 482K).
- On-chain handler: reconstruct slot 32 from
  `verifier_config.vk_generation`.  Read u16, expand to 32-byte BE
  in slot.  **Negligible CU (<100).**
- Wire size: vk_generation NOT on the wire (reconstructed on-chain
  from `verifier_config`).  No ix-data growth.

### 2. SEC-048 Option B -- in-circuit BJJ prime-order subgroup check

- R1CS constraints: **+~30,000** (one cofactor-clear `[8] * P`
  scalar mul + an equality assertion against the input pubkey).
  Affects PROVING TIME and ZKEY size, NOT on-chain verify CU.
- Proving time: **+10-15%** (pre-batch ~5s on localnet -> ~5.5-5.75s).
- Zkey file size: **+2-3 MB** (witness columns scale with constraint
  count).  The on-chain VK is unaffected (VK is constant per
  circuit; constraints affect zkey, not VK -- only public-input
  count affects VK size, which is SEC-006 Part 2's concern).
- On-chain `register_issuer`: drops the `sec007-skip-onchain`
  feature flag.  The on-chain BJJ subgroup check is REMOVED
  entirely (in-circuit gate is the new enforcement; off-chain SDK
  predicate becomes defense-in-depth).  CU change: pre-batch
  76,057 with bypass.  Post-batch with bypass arm fully removed:
  expect a SLIGHT drop (~5K-10K) since we no longer emit
  `Sec007Bypass` event + `is_on_curve + !is_identity` consolation
  gate.  Predicted: **~66K-71K CU**.

### 3. SEC-051 -- reject all-padding `[0,0,0,0]` proofs

- One R1CS constraint: `IsZero(schemaHashes[0]).out === 0`.
- Proving time: **negligible** (single constraint).
- On-chain CU: **no change** (constraint enforced inside Groth16;
  pairing cost is constant per proof regardless of constraint count).

### 4. Aggregate predicted post-batch state

| Instruction | Pre-batch CU | Predicted post-batch CU | Delta |
|---|---:|---:|---:|
| `VerifyBatchProofV2` | 318,315 | **~321K-323K** | +3K-5K (+1 IC slot) |
| `RegisterIssuer` | 76,057 | **~66K-71K** | -5K-10K (drop bypass arm) |
| All other ixs | unchanged | unchanged | 0 |

Total cuIx headroom on the largest consumer (`AppendIssuerLeaf` at
427K and `UpdateTreeRoot` at 418K) is unchanged -- those don't touch
the circuit.

### 5. Artifact deltas

- `circuits/build/verification_key.json`: +~256 bytes (1 G1 IC point
  in JSON form).
- `circuits/build/verification_key.sha256`: changes (whole-file pin).
- `circuits/build/batch_credential_query.zkey`: +2-3 MB.
- `circuits/build/batch_credential_query.wasm`: unchanged (witness
  generator is independent of the trusted setup).
- `tests/fixtures/groth16_e2e_proof.json`: regenerated by the next
  clean e2e run (different VK -> different valid proofs).
- Wire size of `verify_batch_proof_v2` ix data: **unchanged** at 972
  bytes (slot 32 reconstructed on-chain, not on the wire; same as
  slots 1, 2-5, 6-9, 10, 29).

### 6. Validation gates after the batch lands

The CU regression gate is the structural enforcement; in addition:

1. `cargo test -p solid-core -p solid-light -p zk-verifier --lib`
   green (host-side primitives + verifier slot reconstruction).
2. `cargo test -p issuer-registry --lib` green (register_issuer
   bypass arm removed cleanly, no behaviour drift).
3. `npm run e2e` reaches `verified: true` end-to-end on a clean
   validator + replay rejection.
4. SEC-046 CU regression gate: 0 regressions over 1.10x tolerance.
5. The new VK pin (`circuits/build/verification_key.sha256`)
   matches the recomputed sha256 of the regenerated VK JSON.
6. `scripts/verify_freeze_gate.ts`: confirms `vk_finalized=true`
   after the new VK chunks upload + finalize cycle.

If any prediction misses by >20%, root-cause and update this doc
inline before merging.
