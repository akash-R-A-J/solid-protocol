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

Captured 2026-05-01 from the SOLID-SEC-054 / B13 Option 2 + LB5 closure
e2e run.  Validator: localnet, `solana-test-validator --reset`, programs
deployed at the canonical IDs.  Wallet: `56cch2JpqChmm8ufqorRHGB2jbXs88drjPnQbBE2MDEe`.

### `zk_verifier` (`DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb`)

| Instruction | Consumed CU | Budget | Headroom | Tx Signature |
|---|---:|---:|---:|---|
| `Initialize` | 21,394 | 200,000 | 89.3% | `TgbBCRBSat7HEHLK1DcAMnw1k8uXa8T5am4Ft9q1DzYemQJkorFbGP7P8mNCQn7wTwMoWH5D4QoE3NG55AMifG8` |
| `StoreVerificationKey` (chunk 1/3) | 12,842 | 200,000 | 93.6% | `5sM715TtHU4pgQAcpgY3VnFJ6qgwb2PKAj18dY8MEYgZ73bq5i9hemF2L8psbDW1chQinJLsP7Na6u6yLLevUotZ` |
| `StoreVerificationKey` (chunk 2/3) | 9,811 | 200,000 | 95.1% | `5YX2sdtYjfTch11jhV6Q4RRqbb6AA42cFMRV6R9LzRtBDLfHjmcvjcrgpHDRAfMUv2TxXkRrJRkkKVX5TxXJWAtT` |
| `StoreVerificationKey` (chunk 3/3) | 10,627 | 200,000 | 94.7% | `2MvuLyNNgcycP2zttxVxD5P4bHhHXrpMgYmFg9efNcVfvHWmAHqygceHgPEaod7xMUA1gG1Dswfz67voXYbzib2J` |
| `InitProofBuffer` | 13,954 | 200,000 | 93.0% | `2MttWiFoCM8PVzwaeDsHEwuQzsm7FGFyES9ky1NRAMZ2aCRGugPsZbSftAabANrexwRzeUHprC1aK42TfvjwRGdc` |
| `UploadProofChunk` (chunk 1/2) | 9,647 | 200,000 | 95.2% | `5J2D5QvuR6MvseuvDiHwTYjJQ4Meod1fuXkSRFGaJS5H45P6zbC8TdeNSYaZXnmerXrnraWHowjsf29VJNyfCpVv` |
| `UploadProofChunk` (chunk 2/2) | 9,647 | 200,000 | 95.2% | `5ND6nvhMdnYhQ4AmFGjM6qkZYTFBDD1rDRf6FF6VUJ4f5z1Ub3vj24MmntzSHpqDvpFYXPRAs8skA3UEXYjZtGXi` |
| **`VerifyBatchProofV2`** (Groth16) | **315,406** | **799,850** | **60.6%** | `tVYvkyTt55r8RCf3LhVMTmBrKzr5HFtDJcwKM5tFHXerUaxmQSMsZQoKLR8HXbDMu2gYBjNwxC9gaSpdA1oMmCX` |

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
| 4 | `VerifyBatchProofV2` | 315,406 | Groth16 verify (alt_bn128 pairing dominates) |
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

`.github/workflows/ci.yml :: cu_regression` job re-runs `npm run e2e` on a
fresh validator, parses the CU consumption from each tx, and asserts:

```text
ix_consumed_cu <= baseline_consumed_cu * 1.10
```

Any commit that pushes an ix over baseline+10% fails the gate.  This is
the structural fix for SOLID-SEC-046 (no CU-budget regression gate).

The script is at `scripts/measure_cu.py`; baselines are stored in
`tests/cu_baselines.json` (machine-readable companion to this doc).

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
