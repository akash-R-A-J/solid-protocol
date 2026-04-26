# CU Budget Audit — solid-protocol v0.6.1

Date: 2026-04-26 | HEAD: 59c99c2 | Auditor: delegated BPF compute-unit specialist (agent), persisted by orchestrator

Scope: `programs/zk-verifier/src/lib.rs` (1261), `programs/issuer-registry/src/lib.rs` (2497), `programs/schema-registry/src/lib.rs` (599), `crates/solid-core/src/babyjubjub.rs` (694), `crates/solid-core/src/poseidon.rs` (394), `crates/solid-core/src/nullifier.rs` (153), `crates/solid-light/src/cpi_helpers.rs` (765). Total 6,363 LOC.

## 1. CU model — canonical costs on Solana BPF

**Per-tx ceiling: 1,400,000 CU** (in-repo cite: `programs/issuer-registry/Cargo.toml:23`, `sec/SECURITY_REGISTRY.md:1414-1420`). **Default per-ix: 200,000 CU** (`scripts/bootstrap_issuer.ts:256`). `ComputeBudgetProgram::set_compute_unit_limit` caps at 1.4M.

| Op | Cost | Notes |
|---|---:|---|
| `sol_alt_bn128_g1_addition` | ~3,000 CU | Solana 1.18 fee table |
| `sol_alt_bn128_g1_multiplication` | ~180,000 CU | per scalar mul (groth16-solana uses ~33 of these for 32 inputs) |
| `sol_alt_bn128_pairing` | ~36,000 CU base + ~80,000/Miller-loop | one 3-pair pairing per verify |
| `sol_poseidon` (`solana_program::poseidon::hashv`) | 3,000-5,000 CU per call (arity-independent) | dispatch at `crates/solid-core/src/poseidon.rs:140-148` |
| Pre-Poseidon canonicalization | ~500-1,500 CU per input | `bytes_le_to_fr -> fr_to_bytes_le` round-trip at `crates/solid-core/src/poseidon.rs:118-122` |
| `sol_log` (msg!) | ~100 + 1/byte | every `msg!` |
| `sol_log_data` (`emit!`) | ~100 + 1/byte payload | events |
| Base CPI (`sol_invoke_signed_*`) | ~3,000-5,000 CU | before callee cost |
| SPL AC `append` | ~14K-25K CU | depth-20 tree |
| SPL AC `replace_leaf` | ~50K-150K CU | depth- and canopy-dependent |
| `system_program::create_account` CPI | ~3,000 CU | |
| `system_program::transfer` CPI | ~2,500 CU | |
| `spl_token::transfer` CPI | ~5,500-8,000 CU | |
| Anchor `init` PDA | ~5K-7K alloc + ~3K disc stamp | |
| Anchor `init_if_needed` (existing) | ~1,500 CU | |
| Borsh deser of typed `Account<'info,T>` | ~1K-3K CU per account, prop. to size | |
| `Clock::get` | ~350-500 CU | |
| `Rent::get` | ~100 CU | |
| `find_program_address` | ~5,000 CU typical | up to 256 iterations of `create_program_address` |
| Anchor `__global::*` wrapper | ~2K-4K CU base, +~2K for many-account contexts | argument deser + ctx |

ed25519 is not used on-chain; the Ed-equivalent is verified inside the Groth16 circuit.

## 2. Per-handler CU envelope

Status: GREEN <400K, YELLOW 400K-800K, RED >800K. Headroom vs 1.4M.

### 2.1 zk-verifier (`programs/zk-verifier/src/lib.rs`)

| Handler | Lines | Estimated CU | Status |
|---|---|---:|---|
| `initialize` | 103-128 | ~12K | GREEN |
| `set_timestamp_skew` | 136-144 | ~3K | GREEN |
| `store_verification_key` | 163-222 | ~15K-30K per chunk | GREEN |
| `finalize_verification_key` | 231-247 | ~3K | GREEN |
| `request_vk_rotation` | 255-272 | ~3.5K | GREEN |
| `cancel_vk_rotation` | 278-284 | ~2.5K | GREEN |
| `rotate_verification_key` | 297-320 | ~5K | GREEN |
| `set_paused` | 323-327 | ~2K | GREEN |
| `transfer_authority` | 330-336 | ~2.5K | GREEN |
| **`verify_batch_proof`** | 370-588 | **~280K-345K** | GREEN (~75% headroom) |

`verify_batch_proof` decomposition: arity gate ~50, nullifier byte-eq ~150, verifier-address bind ~250, timestamp guard (24-iter loop + Clock::get) ~1.5K, global root ~3.5K, issuer-tree binding ~3.5K, per-slot schema bind ~3.5K-14K (1-4 active slots), Groth16 verify ~250K-300K (33 G1 muls + 3-pair Miller loop + final exp), nullifier `init` PDA ~10K-15K, metrics+emit ~3K, plus Anchor wrapper ~10K-15K and account loading ~25K-40K (VkStorage Borsh dominates this).

### 2.2 issuer-registry (`programs/issuer-registry/src/lib.rs`)

| Handler | Lines | Estimated CU | Status |
|---|---|---:|---|
| `initialize_registry` | 124-143 | ~12K | GREEN |
| **`register_issuer` (no feature)** | 145-282 | **~1.6M-2.0M** | **RED — BROKEN (SEC-048)** |
| `register_issuer` (`sec007-skip-onchain`) | 145-282 | ~110K-150K | GREEN (interim only) |
| `vote_on_issuer` | 295-357 | ~15K-22K | GREEN |
| `withdraw_stake` | 361-381 | ~5K | GREEN |
| `stake_tokens` | 384-404 | ~15K-25K | GREEN |
| `unstake_tokens` | 407-443 | ~15K-25K | GREEN |
| `release_vote` | 452-465 | ~6K | GREEN |
| `request_withdrawal` | 478-501 | ~5K | GREEN |
| `withdraw_after_cooldown` | 504-547 | ~8K | GREEN |
| `finalize_voting` | 549-612 | ~10K-15K | GREEN |
| `slash_issuer` | 621-696 | ~10K-15K | GREEN |
| `submit_fraud_proof` | 709-777 | ~10K-15K | GREEN |
| `approve_via_trust_anchor` | 788-838 | ~8K-12K | GREEN |
| `check_issuer_status` | 841-849 | ~3.5K | GREEN |
| `revoke_issuer` (legacy) | 859-877 | ~5K | GREEN |
| `initialize_issuer_tree_binding` | 905-956 | ~12K-18K | GREEN |
| `update_issuer_tree_root` | 966-1004 | ~5K-7K | GREEN |
| `set_issuer_tree_binding_status` | 1010-1036 | ~4K-5K | GREEN |
| `append_issuer_leaf` | 1090-1178 | ~50K-80K | GREEN |
| `revoke_issuer_atomic` | 1208-1324 | ~80K-160K (depth-dependent) | GREEN |
| `request_withdrawal_atomic` | 1366-1484 | ~80K-160K | GREEN |
| `issue_credential` | 1509-1640 | ~50K-80K | GREEN |

### 2.3 schema-registry (`programs/schema-registry/src/lib.rs`)

| Handler | Lines | CU | Status |
|---|---|---:|---|
| `register_schema` | 86-137 | ~15K-25K | GREEN |
| `deprecate_schema` | 140-145 | ~3K | GREEN |
| `increment_usage` | 153-166 | ~4K | GREEN |
| `initialize_tree_binding` | 189-249 | ~12K-18K | GREEN |
| `update_tree_root` | 263-309 | ~6K-8K | GREEN |
| `set_binding_status` | 314-341 | ~5K | GREEN |
| `initialize_global_binding` | 347-378 | ~12K-18K | GREEN |
| `update_global_root` | 382-413 | ~6K-8K | GREEN |
| `transfer_tree_binding_authority` | 420-444 | ~5K-6K | GREEN |
| `transfer_global_binding_authority` | 447-470 | ~5K-6K | GREEN |

All schema-registry handlers are well under any threshold.

**Summary:** only `register_issuer` (no feature) is RED. All other handlers GREEN with >50% headroom. No YELLOW handlers. Hot paths CU-wise: (1) `register_issuer` no-bypass (>1.4M, broken), (2) `verify_batch_proof` (~285-345K, dominated by Groth16 syscalls; will grow with NR_PUBLIC_INPUTS), (3) atomic SPL AC handlers (50-160K each, depth-dependent).

## 3. Hot-path drilldowns

**`verify_batch_proof`** (`programs/zk-verifier/src/lib.rs:370-588`): Anchor wrapper deserializes 1312-byte ix args (~10K-15K) -> account loading (VkStorage Borsh ~5K-20K is the dominant non-syscall cost) -> arity gate -> nullifier eq -> verifier-address eq -> timestamp guard -> global root parse (`crates/solid-light/src/cpi_helpers.rs:289-297`) -> issuer-tree binding parse (`cpi_helpers.rs:466-480`) -> per-slot schema binding parse (`cpi_helpers.rs:254-276`, up to 4x) -> `verify_groth16_proof` helper at `lib.rs:727-751` (`#[inline(never)]` is the load-bearing fix for SEC-047 stack pressure) -> `VkBuf::parse` at `lib.rs:644-692` (~10K-15K) -> `negate_g1_point` at `lib.rs:755-784` (~1K) -> `Groth16Verifier::new` (~5K) -> `verifier.verify()` (33 G1 muls + 3-pair pairing, ~250K-300K) -> nullifier PDA `init` (~10K-15K) -> emit + msg! (~3K).

**`register_issuer` (no feature)**: the entire >1.4M cost is in `solid_core::babyjubjub::require_in_prime_order_subgroup(&bjj_pub_key)` at `programs/issuer-registry/src/lib.rs:189`, which calls `EdwardsAffine::is_in_correct_subgroup_assuming_on_curve` at `crates/solid-core/src/babyjubjub.rs:229`. That runs `r * P == O` over a ~251-bit BJJ subgroup-prime scalar; ~251 doublings + ~125 conditional adds; per-doubling ~3K-5K CU and per-add ~5K-8K CU on BPF arkworks; aggregate ~1.75M CU (matches in-repo SEC-048 evidence at `sec/SECURITY_REGISTRY.md:1426-1432`).

**`revoke_issuer_atomic`** (`lib.rs:1208-1324`): Anchor wrapper + 8-account ctx (~5K) + authority eq (~300) + 2 status `require!` (~500) + compute OLD leaf at `lib.rs:1226` (5-input Poseidon ~6K-12K) + counter bump (~500) + status flip (~50) + compute NEW leaf at `lib.rs:1239` (5-input Poseidon ~6K-12K) + `find_program_address` at `lib.rs:1261-1262` (~5K) + 3 `require_keys_eq!` (~750) + Vec marshaling (~3K) + account-meta build (~3K-5K) + **CPI `replace_leaf` at `lib.rs:1305` (~50K-150K depth/canopy-dependent)** + bookkeeping + emit (~3K). Total ~85K-180K. Same shape for `request_withdrawal_atomic`.

**`append_issuer_leaf`/`issue_credential`** (`lib.rs:1090-1178`, `1509-1640`): single SPL AC `append` CPI (~14K-25K). `append_issuer_leaf` additionally pays ~6K-12K for the 5-input `compute_issuer_leaf_bytes` Poseidon at `lib.rs:63-77`. `issue_credential` does NOT recompute (treats `commitment` as opaque off-chain Poseidon output).

## 4. SEC-007 / SEC-048 deep dive: BJJ subgroup check on BPF

**Source:** `crates/solid-core/src/babyjubjub.rs:218-233` (`require_in_prime_order_subgroup`).

Cost decomposition: `bytes_to_fq` x 2 (~600 each), `EdwardsAffine::new_unchecked` (~50), `is_on_curve` evaluating `-x^2 + y^2 = 1 + d*x^2*y^2` = 4 Fq muls + 2 Fq adds (~3K), `is_zero` (~100), then the **dominant** `is_in_correct_subgroup_assuming_on_curve` at line 229 = scalar mul `r*P` over ~251-bit BJJ subgroup prime via arkworks BPF-portable backend, ~251 doublings + ~125 conditional adds. Per-doubling on BPF: 4 Fq muls + 4 Fq adds + 4 Fq subs, each Fq mul ~600-1,000 CU (4-limb bigint mul + Montgomery reduction), so ~3K-5K CU per doubling. Per add: 8 Fq muls + 4 Fq adds = ~5K-8K CU. Aggregate **~1.75M CU**, matches in-repo measurement of "1.6M-2.0M CU" at `sec/SECURITY_REGISTRY.md:1426-1432`. The 1.4M ceiling cuts it short -> `exceeded CUs meter` runtime error.

**Consolation gate** (currently shipped under `sec007-skip-onchain` at `programs/issuer-registry/src/lib.rs:200-207`): `is_on_curve + !is_identity` = ~3.5K CU. Catches off-curve garbage and identity but **does NOT catch cofactor-8 torsion** (the residual SEC-048 attack — security parameter for affected issuer's credentials drops from 251 to 3 bits).

**Alternatives ranked by soundness:**
- **Option A** (current bypass + off-chain TS predicate `isInPrimeOrderSubgroup`): 3.5K CU on-chain; trusts off-chain caller; `Sec007Bypass` event allows post-fact monitoring. Localnet/devnet only.
- **Option B** (cofactor-clear in SDK: pre-multiply candidate by 8 off-chain, on-chain stays at consolation gate): 3.5K CU on-chain, 3 doublings (~3 ms) on caller; strictly improves on Option A because the trusted off-chain operation shrinks from "remember to call the predicate" to "call .mul(8)". Sibling-equivalent detection profile. Recommended interim. Cite: `sec/SECURITY_REGISTRY.md:1471-1477`.
- **Option C** (precomputed-table check on-chain): REJECTED — table derivation requires another scalar mul of equivalent scale.
- **Option D** (SD-EdDSA torsion-killer multiply): REJECTED — this IS the operation that costs 1.6M CU.
- **Option E** (in-circuit subgroup constraint in `circuits/batch_credential_query.circom`): 3.5K CU on-chain (just the consolation gate, possibly droppable) + ~30K extra circuit constraints (one EdDSA-style scalar mul; ~250 ms additional proving cost). **Strongest soundness.** Requires trusted-setup re-run; batches with SEC-006 Part 2 + SEC-012. Cite: `sec/SECURITY_REGISTRY.md:1478-1485`.
- **Option F** (Solana `sol_babyjubjub_*` syscall): ~3K CU on-chain; multi-quarter timeline; depends on validator buy-in. Cite: `sec/SECURITY_REGISTRY.md:1486-1489`.

**Recommended path:** ship Option B as interim (improves over today's bypass without trusted-setup work); ship Option E as the canonical fix (next trusted-setup cycle).

## 5. SEC-046 CU regression gate design

**Today:** no CU gate exists. Compile-time `assert!(sz < 1024, ...)` at `programs/zk-verifier/src/lib.rs:93-96` catches stack-frame growth (closes SEC-047) but says nothing about CU. Future bumps to NR_PUBLIC_INPUTS (SEC-006 Part 2 will do this), additional IC points, or Solana minor-version syscall-cost drift can silently push `verify_batch_proof` past 1.4M with only a runtime "exceeded maximum number of instructions" error.

**Proposed CI job `cu_baseline`:**

1. `bash scripts/sync_program_keypairs.sh && anchor build` — `--release` (debug-vs-release CU costs differ radically on BPF).
2. `solana-test-validator --reset` in background.
3. Deterministic init: `initialize.ts -> backfill_issuer_tree.ts -> bootstrap_issuer.ts -> issue.ts`.
4. Drive `prove.ts` with a checked-in deterministic test vector at `tests/vectors/cu_baseline_proof.json` (precomputed `(proof_a, proof_b, proof_c, public_inputs, nullifier)` tuple).
5. Submit with `ComputeBudgetProgram.setComputeUnitLimit({ units: 800_000 })` (~2.5x v0.6.1 baseline; catches 250% regressions instantly).
6. `solana confirm -v <sig>` log includes `Program <id> consumed X of 800000 compute units`. Parse `X`.
7. Compare against `docs/CU_BUDGET.md`. Fail if `X > baseline * 1.10` (10% slop for run-to-run variance + minor-version drift).
8. Repeat for `register_issuer` (with `sec007-skip-onchain` build), `revoke_issuer_atomic`, `request_withdrawal_atomic`, `append_issuer_leaf`, `issue_credential`.
9. On sanctioned bumps: PR commits new baseline in `docs/CU_BUDGET.md`; the baseline bump is its own reviewable signal.

**`docs/CU_BUDGET.md` schema:**

```
## Per-handler baselines
| Handler | Program | Build flags | Baseline CU | Tolerance | Last bumped | Reason |
| verify_batch_proof          | zk-verifier      | --release                                   |  330_000 | +10% | 2026-04-26 | NR_PUBLIC_INPUTS=32 |
| register_issuer             | issuer-registry  | --release --features sec007-skip-onchain    |  150_000 | +10% | 2026-04-26 | SEC-048 bypass |
| append_issuer_leaf          | issuer-registry  | --release                                   |   80_000 | +10% | 2026-04-26 | depth-20, canopy-14 |
| revoke_issuer_atomic        | issuer-registry  | --release                                   |  170_000 | +10% | 2026-04-26 | depth-20, canopy-14 |
| request_withdrawal_atomic   | issuer-registry  | --release                                   |  170_000 | +10% | 2026-04-26 | mirrors revoke |
| issue_credential            | issuer-registry  | --release                                   |   75_000 | +10% | 2026-04-26 | depth-20 |

## Triggering constants
- NR_PUBLIC_INPUTS                  (programs/zk-verifier/src/lib.rs:41)
- VK IC count                       (= NR_PUBLIC_INPUTS + 1)
- ISSUER_TREE_DEPTH                 (deployment-time SPL AC config)
- ISSUER_TREE_CANOPY_DEPTH          (deployment-time SPL AC config)
```

**Trip wires:** any handler exceeds baseline by 10%; the 800K cap is hit at submit time (250% regression); any GREEN handler crosses 800K.

**Failure mode:** PR check fails with `verify_batch_proof: 388_000 CU > 363_000 (baseline 330_000 + 10%)`.

**Stretch:** `solana_program::log::sol_log_compute_units()` at each phase boundary inside `verify_batch_proof` for per-phase regression localization. ~100 CU per call, 6 phases = ~600 CU. Negligible. Recommended Phase 4.

## 6. Stack-frame pressure (4 KB BPF per-frame audit)

`Cargo.toml:93` pins `lto = "thin"` (with extensive in-file note at `Cargo.toml:62-92` explaining why "fat" must NOT be re-enabled), and `programs/zk-verifier/src/lib.rs:369, 726` carry `#[inline(never)]` on `verify_batch_proof` and `verify_groth16_proof`. The compile-time assert at `lib.rs:93-96` bounds `VkBuf` shell at <1024B. All three together are the durable SEC-047 fix.

| Function | Largest locals | Frame | Free |
|---|---|---:|---:|
| `verify_batch_proof` (user fn) | Context (~900B), proof_a/b/c (256B), Vec descriptor (24B), nullifier (32B), public_inputs by-ref (24B) | ~1.5-1.8KB | ~2.2KB |
| `verify_groth16_proof` helper | VkBuf (480B), Groth16Verifyingkey view (480B), proof_a_neg (64B), Groth16Verifier (~120B) | ~1.2-1.4KB | ~2.6KB |
| `__global::verify_batch_proof` Anchor wrapper | deser ix args (post-fix Vec descriptor: 312B), VerifyBatchProof accounts (~900B), ctx (~150B), outgoing-arg slots (~200B), saved registers + spill (~600B) | ~2.2-2.4KB | **~1.6-1.8KB** |
| `register_issuer` | Context (~300B), bjj_pub_key (64B), CPI args (~200B); name/metadata_uri heap | ~700B | ~3.3KB |
| `revoke_issuer_atomic` | Context (~250B), old_root + leaves (96B), Vec descriptors (heap payload), signer_seeds (~50B) | ~600B | ~3.4KB |
| `request_withdrawal_atomic` | identical | ~600B | ~3.4KB |
| `append_issuer_leaf` | smaller than revoke (no proof staging) | ~500B | ~3.5KB |
| `issue_credential` | Context (~350B), schema_hash + commitment (64B), Vecs | ~600B | ~3.4KB |
| `store_verification_key` | mut borrows (~60B), chunk_data Vec descriptor (24B) | ~200B | ~3.8KB |
| All schema-registry handlers | All under ~500B | ~500B | ~3.5KB |
| `compute_issuer_leaf_bytes` | status_epoch_fr/rev_nonce_fr (~64B), 5 inputs by-ref (160B) | ~250B | ~3.75KB |

**Smallest margin: `__global::verify_batch_proof` wrapper at ~1.6-1.8KB free.** No function approaching 4KB ceiling. The Vec-descriptor change (post-SEC-047) bought the wrapper its margin — the prior `[[u8; 32]; 32]` direct argument cost 1024B inline + another 1024B in BPF outgoing-arg slots = the 456B overflow described at `sec/SECURITY_REGISTRY.md:1289-1304`.

## 7. Findings

**CU-H01 (HIGH) — `register_issuer` exceeds 1.4M CU on BPF (SEC-048).** Location: `programs/issuer-registry/src/lib.rs:189` -> `crates/solid-core/src/babyjubjub.rs:218-233` -> `EdwardsAffine::is_in_correct_subgroup_assuming_on_curve`. Status: open with `sec007-skip-onchain` interim bypass for localnet/devnet. Mainnet deploy-blocker. Bypass build's consolation gate does NOT catch cofactor-8 torsion. Recommendation: ship Option E (in-circuit subgroup constraint) as canonical; ship Option B (off-chain cofactor-clear) as interim. Tracking: `sec/SECURITY_REGISTRY.md:1373-1523`.

**CU-M01 (MEDIUM) — No CU regression gate in CI (SEC-046).** Location: `.github/workflows/ci.yml`. Any future bump to NR_PUBLIC_INPUTS or VK IC count silently shrinks `verify_batch_proof` headroom; SEC-006 Part 2 will trigger this. Recommendation: ship Section 5 spec verbatim.

**CU-M02 (MEDIUM) — No explicit BPF stack-frame size CI gate.** SEC-047 already happened once. Today the linker error is the implicit gate but it surfaces under generic "build failed". Recommendation: add a CI step that runs `cargo-build-sbf --release` for each program, parses linker output for `Stack offset of N exceeded max offset of 4096`, emits structured failure. Pair with CU-M01.

**CU-L01 (LOW) — `verify_batch_proof` double-parses the VK.** Saves 5K-20K per proof if `vk_storage` becomes `UncheckedAccount` with hand-rolled discriminator + length check.

**CU-L02 (LOW) — Per-call canonicalization in `hash_bytes` runs N round-trips even for already-canonical inputs.** Saves ~3K-7K per 5-input Poseidon if a fast-path branch checks MSB < 0x30 and skips the round-trip.

**CU-L03 (LOW) — Atomic handlers re-run `find_program_address` defensively.** ~5K CU per call. Trusting `ctx.bumps.<name>` would save 5K per call. Audit carefully before removing.

**CU-I01 (INFO) — Atomic-handler CU is depth-of-tree dependent and undocumented.** Document `(issuer_tree_depth, issuer_tree_canopy_depth)` in `docs/CU_BUDGET.md` + `docs/DEPLOYMENT_AND_TESTING.md`.

## 8. Suggested next actions, ordered

**Wave A (mainnet blockers):**
1. Close SEC-048 (CU-H01): ship Option E (in-circuit subgroup constraint) batched with SEC-006 Part 2 + SEC-012 ceremony.
2. Ship Option B (cofactor-clear in SDK) as interim improvement over today's bypass.
3. Land SEC-046 CU baseline gate (CU-M01). Required before SEC-006 Part 2 ships.

**Wave B (before next external audit):**
4. Add explicit BPF stack-frame CI gate (CU-M02).
5. Document `(tree_depth, canopy_depth)` in `docs/CU_BUDGET.md` + runbook (CU-I01).

**Wave C (only if CU-M01 shows shrinking headroom):**
6. CU-L01: strip duplicate VK parse. ~5K-20K CU saved per proof.
7. CU-L02: fast-path canonical Poseidon inputs. ~3K-7K per call.
8. CU-L03: trust Anchor bumps. ~5K per atomic ix.
9. Phase 4: in-handler `sol_log_compute_units()` at phase boundaries.

**Wave D (long-term):**
10. Solana `sol_babyjubjub_*` syscall (Option F): multi-quarter; would let the protocol drop the in-circuit constraint after Option E ships.

---

## Summary

- Only `register_issuer` (no feature) is RED (~1.6M-2.0M CU vs 1.4M ceiling); SEC-048 is structural and there is no buildable on-chain alternative under current Solana CU rules. Section 4 walks through five fix options ranked by soundness.
- All other handlers are GREEN with >50% headroom. `verify_batch_proof` at ~285-345K is the second-largest, dominated by Groth16 alt_bn128 syscalls. Headroom shrinks linearly with each new public input.
- Stack-frame audit clean: smallest margin is ~1.6-1.8 KB free in the Anchor `__global::verify_batch_proof` wrapper. `lto = "thin"` + two `#[inline(never)]` annotations + the `VkBuf < 1024` compile-time assert are the durable SEC-047 fix.
- SEC-046 gate design is concrete and ready to implement; spec includes the test vector at `tests/vectors/cu_baseline_proof.json`, the `docs/CU_BUDGET.md` schema, the 800K compute-budget cap (catches 250% regressions), and the 10% tolerance.
