# Orchestrator hand-audit: zk-verifier (function level)

Date: 2026-04-26
Branch: main (HEAD: 59c99c2)
Auditor: Claude Opus 4.7 (orchestrator hand-check)
Scope: `programs/zk-verifier/src/lib.rs` (1261 LOC) + the `solid-light` helpers it consumes (`crates/solid-light/src/cpi_helpers.rs` 765 LOC, `credential_tree.rs` 112 LOC).

This file is the orchestrator's own per-function audit produced in parallel with the delegated specialist agent. It treats the source as the only authoritative artefact, and the delegated agent's output (under `01_modular/01_zk_verifier.md`) is reviewed against the standard demonstrated here.

The format for every Anchor handler (and every helper called from one) is the four-bucket dissection the user explicitly requested: **Inputs**, **Processing (correctness)**, **Outputs (return + side effects + events)**, **Error handling (every failure path enumerated)**. Anything subtle in any bucket is called out.

---

## Crate / build configuration

`programs/zk-verifier/Cargo.toml`:
- `crate-type = ["cdylib", "lib"]` — produces both the BPF program binary and a regular Rust library so other crates (none today, but the `cpi` feature hints at intent) can call into it.
- Features: `no-entrypoint`, `no-idl`, `no-log-ix-name`, `cpi = ["no-entrypoint"]`, `idl-build = ["anchor-lang/idl-build"]`, `default = []`. Standard Anchor surface.
- Direct deps:
  - `anchor-lang = workspace + ["init-if-needed"]` — required because `StoreVerificationKey` uses `init_if_needed` on `vk_storage`.
  - `groth16-solana = "0.2"` — alt_bn128 syscall-backed Groth16 verifier. No cargo workspace pin; this crate's MSRV must agree with the workspace MSRV (1.75).
  - `solid-light = path("../../crates/solid-light")` — supplies `cpi_helpers::*` (parsers + program-ID constants) and the borsh layout for compressed leaves.
- Notably absent: no direct `solana-program` dep. anchor-lang re-exports the relevant items via `prelude`. anchor-spl is also absent (this program never CPIs into SPL programs).

**Reading**: dep surface is minimal and proportionate. groth16-solana 0.2 is the load-bearing third-party crate; its update cadence and any open advisories should appear in the dependency-audit document.

---

## Module-level constants and compile-time guards

`programs/zk-verifier/src/lib.rs:22` — `declare_id!("DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb");`. Matches `Anchor.toml [programs.localnet/devnet] zk_verifier = "DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb"`. Verified by `scripts/check_program_ids.py`. ✓

`programs/zk-verifier/src/lib.rs:41` — `pub const NR_PUBLIC_INPUTS: usize = 32;`. Comment block at lines 25–40 enumerates the slot semantic:
- `[0]` nullifierHash
- `[1]` globalRoot
- `[2..6]` merkleRoots[4]      (note: comment uses `[2..5]` form)
- `[6..10]` schemaHashes[4]    (comment uses `[6..9]`)
- `[10]` issuerTreeRoot
- `[11..15]` queryCredentialIndices[4] (comment uses `[11..14]`)
- `[15..19]` queryFieldIndices[4]      (comment uses `[15..18]`)
- `[19..23]` queryOperators[4]         (comment uses `[19..22]`)
- `[23..27]` queryValues[4]            (comment uses `[23..26]`)
- `[27]` numPredicates
- `[28]` compoundLogic
- `[29]` verifierAddress
- `[30]` verifierNonce
- `[31]` currentTimestamp

The comment uses inclusive-end notation (e.g. `[2..5]` to mean indices 2,3,4,5). Reader must hold this convention in their head — minor cognitive trap. **Finding M01-L01**: the doc comment is ambiguous; future-you reading `[6..9]` may parse it as Rust half-open range and place `issuerTreeRoot` at `[9]`, off-by-one. Recommend rewriting as `[2..=5]` or the verbose form `[2], [3], [4], [5]`.

`programs/zk-verifier/src/lib.rs:47-53` — `ISSUER_TREE_ROOT_INPUT_INDEX = 10`, `VERIFIER_ADDRESS_INPUT_INDEX = 29`, `CURRENT_TIMESTAMP_INPUT_INDEX = 31`. These are the handler's source of truth; verified against the comment + against the circuit (`circuits/batch_credential_query.circom`) by the wave-1 circuit specialist.

`programs/zk-verifier/src/lib.rs:57` — `pub const MAX_IC: usize = NR_PUBLIC_INPUTS + 1;`. IC table count is one more than the number of public inputs by Groth16 construction. Used by `VkBuf::parse` as the upper bound on `nr_ic` to prevent unbounded heap allocation from a malformed VK. ✓

`programs/zk-verifier/src/lib.rs:59` — `pub const NULLIFIER_SEED: &[u8] = b"null";`. Used as the first PDA seed for `NullifierRecord`. The PDA seed-list is `[NULLIFIER_SEED, nullifier.as_ref()]` — total 4+32 = 36 bytes, two seeds, well within Solana's MAX_SEEDS=16 and MAX_SEED_LEN=32. ✓

`programs/zk-verifier/src/lib.rs:65` — `DEFAULT_TIMESTAMP_SKEW_SECONDS = 600` (10 min). `programs/zk-verifier/src/lib.rs:70` — `MAX_TIMESTAMP_SKEW_SECONDS = 3_600` (1 hour). The hard cap means a misconfiguration cannot disable freshness for an arbitrary horizon. ✓

`programs/zk-verifier/src/lib.rs:78` — `VK_ROTATION_TIMELOCK_SECONDS: i64 = 48 * 60 * 60`. Comment ties this to SOLID-SEC-006 + SOLID-SEC-043 (multisig replacement). 48 hours is conservative. The `i64` type (rather than u64) lets it interoperate with `Clock::unix_timestamp` (i64) directly without casts. ✓

`programs/zk-verifier/src/lib.rs:93-96` — compile-time assertion: `core::mem::size_of::<VkBuf>() < 1024`. Doc comments at lines 80–92 explain the BPF stack-frame budget reasoning. This is the structural fix for the 4 KB BPF per-frame overflow that bit the program before the IC table moved off the stack. **Finding M01-Info-01**: this `const _: () = { assert!(...) }` is excellent and uncommon practice. Worth cross-noting that an analogous compile-time guard for `VerifyBatchProof` accounts struct size (currently ~700 bytes per the workspace Cargo.toml comment) is missing — if future fields creep in, the `__global::verify_batch_proof` wrapper can again exceed 4 KB. See the integration audit for a recommended `static_assertions` crate import or hand-rolled `const _: () = { assert!(size_of::<VerifyBatchProof>() < N) }` guard.

---

## Handler 1: `initialize`

Source: `programs/zk-verifier/src/lib.rs:103-128`. Account context: `Initialize` at `programs/zk-verifier/src/lib.rs:798-810`.

**Inputs**:
- Accounts:
  - `verifier_config` (init, payer = authority, space = 8 + VerifierConfig::SPACE = 8 + 60 = 68 bytes, seeds = [b"verifier-config"], bump). The `init` constraint is the load-bearing replay protection: `initialize` cannot be called twice on the same deployment because the second call's PDA already exists and Anchor errors with `AccountAlreadyInitialized`.
  - `authority` (mut, Signer). Pays for rent. Becomes the on-chain authority recorded in `verifier_config.authority`.
  - `system_program` (Program<System>). Required for the create-account CPI inside `init`.
- Instruction args: none.

**Processing (correctness)**:
- Sets `authority`, zeroes `proof_count`, sets all flags to defaults, captures the canonical bump in `bump`. All fields explicitly initialized — no implicit zero relied upon (good practice, especially for `vk_finalized` and `rotate_request_ts` which gate later state transitions).
- `config.timestamp_skew_seconds = DEFAULT_TIMESTAMP_SKEW_SECONDS` (600). ✓
- `config.vk_finalized = false`, `config.vk_generation = 0`, `config.rotate_request_ts = 0`. The post-ADR-0015 fields are explicitly zeroed even though a freshly-allocated PDA's bytes are zero by default — defensive and matches the SPACE arithmetic.

**Outputs**:
- Returns `Ok(())`.
- Side effect: `verifier_config` PDA created and populated.
- Logs: `msg!("SolID ZK Verifier initialized. Authority: {}", config.authority);` — no privacy-sensitive data; the authority pubkey is on-chain anyway.

**Error handling**:
- The only failure modes are pre-init Anchor checks: PDA already exists → `AccountAlreadyInitialized`, or system_program create_account failure (rent-exempt amount unavailable, etc.). All recoverable / well-typed errors.

**Verdict**: clean. No findings.

---

## Handler 2: `set_timestamp_skew`

Source: `programs/zk-verifier/src/lib.rs:136-144`. Account context: `AuthorityOnly` at `programs/zk-verifier/src/lib.rs:812-821`.

**Inputs**:
- Accounts (`AuthorityOnly`):
  - `verifier_config` (mut, seeds=[b"verifier-config"], bump=verifier_config.bump, has_one = authority @ Unauthorized). `has_one` is the Anchor-level constraint that the `authority` Signer's pubkey equals `verifier_config.authority`. ✓
  - `authority` (Signer). The transaction signer.
- Instruction args: `skew_seconds: u32`.

**Processing**:
- `require!(skew_seconds <= MAX_TIMESTAMP_SKEW_SECONDS, ErrorCode::TimestampSkewTooLarge);` — bounded by the 3600-second cap. ✓
- Writes `verifier_config.timestamp_skew_seconds = skew_seconds`.

**Outputs**:
- Returns `Ok(())`. Side effect: skew updated. Logs: `Timestamp skew updated to {} seconds`.

**Error handling**:
- Caller is not authority: Anchor's `has_one` constraint fails with `Unauthorized`.
- skew exceeds cap: `TimestampSkewTooLarge`.

**Verdict**: clean. **Finding M01-Info-02**: the lower bound is 0 — a savvy authority could set skew to 0 effectively requiring `currentTimestamp == now` exactly, which would break valid proofs because of slot-time vs rpc-clock skew. This is acceptable (admin-only knob, recoverable) but worth documenting in `verifier-guide.md`.

---

## Handler 3: `store_verification_key`

Source: `programs/zk-verifier/src/lib.rs:163-222`. Account context: `StoreVerificationKey` at `programs/zk-verifier/src/lib.rs:823-840`.

**Inputs**:
- Accounts:
  - `verifier_config` (mut, seeds=[b"verifier-config"], bump=verifier_config.bump). Note: NOT decorated with `has_one = authority`; the authority check is done in-handler via `require!(config.authority == ctx.accounts.authority.key(), ...)`.
  - `vk_storage` (init_if_needed, payer = authority, space = 8 + 4 + 10228 = 10240, seeds=[b"vk-storage", verifier_config.key().as_ref()], bump). `init_if_needed` is required because the same handler bootstraps storage on first call AND appends on subsequent calls.
  - `authority` (mut, Signer, constraint = authority.key() == verifier_config.authority @ Unauthorized). The constraint here is duplicated by the in-body `require!`. Defense-in-depth.
  - `system_program`.
- Instruction args:
  - `chunk_index: u16` — must equal `config.next_vk_chunk` (in-order delivery).
  - `chunk_data: Vec<u8>` — the chunk bytes. No max length on the arg type itself; the handler bounds-checks total cumulative size against `VK_MAX_BYTES = 10_228`.
  - `is_final_chunk: bool` — flips `vk_initialized = true` and emits the cumulative-size log.

**Processing**:
1. `require!(config.authority == ctx.accounts.authority.key(), Unauthorized)` (line 172-175). The Accounts-level constraint at line 837 checks the same. Two layers of guard.
2. `require!(!config.vk_finalized, VerificationKeyFinalized)` (line 179). SOLID-SEC-006 freeze gate: a finalized VK refuses every write.
3. `require!(chunk_index == config.next_vk_chunk, ChunkOutOfOrder)` (line 180-183). Strict in-order delivery — without this, a malicious operator could scramble chunks and the parser could silently accept a corrupt VK.
4. If `chunk_index == 0`: replace `vk_storage.data = chunk_data` (line 197). Validates `incoming_len <= VK_MAX_BYTES` first (line 196). Note: this path means a partial-rotation accident on chunk 0 atomically replaces the buffer; safer than appending into a half-allocated state.
5. Else (chunk_index > 0): `checked_add` cumulative length (line 199-203), bounds-check against `VK_MAX_BYTES`, append (line 205).
6. `config.next_vk_chunk = config.next_vk_chunk.checked_add(1).ok_or(Overflow)?` (line 208-211). At u16 max (65 535) this errors; in practice `next_vk_chunk` is reset to 0 each rotation, so unreachable.
7. If `is_final_chunk`: `config.vk_initialized = true` (line 213-214). Logs cumulative size and chunk count.

**Outputs**:
- Returns `Ok(())`.
- Side effects: `vk_storage.data` mutated, `config.next_vk_chunk` incremented, possibly `config.vk_initialized = true`.
- Logs: chunk-count + total-size message on final chunk.

**Error handling** (every path enumerated):
- Unauthorized: in-body `require!` and Accounts-level `constraint` both fire.
- Already-finalized VK: `VerificationKeyFinalized`.
- Out-of-order chunk: `ChunkOutOfOrder`.
- Chunk-0 size > VK_MAX_BYTES: `VkStorageFull`.
- Cumulative size overflow on `checked_add`: `Overflow`.
- Cumulative size > VK_MAX_BYTES: `VkStorageFull`.
- Chunk-counter overflow: `Overflow` (unreachable in practice).
- system_program create_account failure on first call (init_if_needed): Anchor's bubbled error.

**Subtle observation** — `vk_storage` lacks any explicit authority check inside the constraint block. The `init_if_needed` is gated only by the fact that the *authority* signer pays. If a misconfigured permission system existed, this would be a finding; in the current model it does not. **Finding M01-Info-03**: `vk_storage` could in principle be initialized by a CPI from another program if that program could invoke `store_verification_key`. Anchor's signer model + the explicit signer check makes that effectively unreachable, but the doc-comment on `vk_storage` should call this out for future-proofing.

**Verdict**: clean. The two-layer authority guard (Accounts constraint + in-body require) is good practice.

---

## Handler 4: `finalize_verification_key`

Source: `programs/zk-verifier/src/lib.rs:231-247`.

**Inputs**:
- Accounts: `AuthorityOnly` (verifier_config + authority Signer with has_one).
- Instruction args: none.

**Processing**:
- `require!(config.vk_initialized, VerificationKeyNotSet)` — can't finalize an empty store.
- `require!(!config.vk_finalized, VerificationKeyAlreadyFinalized)` — idempotency guard. Repeated calls error rather than no-op; arguably a minor UX choice.
- `config.vk_finalized = true`.

**Outputs**: `Ok(())`. Side effect: `vk_finalized = true`. Log includes generation + timelock-seconds for operator visibility.

**Error handling**:
- Unauthorized: handled by `AuthorityOnly`'s `has_one`.
- VK not yet uploaded: `VerificationKeyNotSet`.
- Already finalized: `VerificationKeyAlreadyFinalized`.

**Verdict**: clean.

---

## Handler 5: `request_vk_rotation`

Source: `programs/zk-verifier/src/lib.rs:255-272`.

**Inputs**: `AuthorityOnly`, no args.

**Processing**:
1. `require!(config.vk_finalized, VerificationKeyNotFinalized)` — only frozen VKs need a timelock to rotate.
2. `require!(config.rotate_request_ts == 0, RotationAlreadyRequested)` — refuses double-requests.
3. `Clock::get()?.unix_timestamp` — read on-chain time.
4. `require!(now > 0, RotationClockInvalid)` — defense-in-depth against weird Clock states (boot edge cases).
5. `config.rotate_request_ts = now`.

**Outputs**: `Ok(())`. Side effect: timelock anchor recorded. Log includes earliest-rotation-time (`now + 48h`).

**Error handling**:
- Not finalized: `VerificationKeyNotFinalized`.
- Already pending: `RotationAlreadyRequested`.
- Clock <= 0: `RotationClockInvalid`.
- Clock sysvar unavailable: bubbled `?` error.

**Verdict**: clean.

---

## Handler 6: `cancel_vk_rotation`

Source: `programs/zk-verifier/src/lib.rs:278-284`.

**Inputs**: `AuthorityOnly`.

**Processing**: Sets `rotate_request_ts = 0` unconditionally; logs the prior value for audit.

**Outputs**: `Ok(())`. Side effect: timelock cleared.

**Error handling**: only `AuthorityOnly` has_one guard.

**Subtle observation**: idempotent — calling with no pending rotation is a successful no-op. The log line still fires (`prior request_ts=0`). Acceptable; explicitly documented in the comment at line 274-277.

**Verdict**: clean.

---

## Handler 7: `rotate_verification_key`

Source: `programs/zk-verifier/src/lib.rs:297-320`.

**Inputs**: `AuthorityOnly`.

**Processing**:
1. `require!(config.vk_finalized, VerificationKeyNotFinalized)`.
2. `require!(config.rotate_request_ts != 0, NoPendingRotation)`.
3. Fetch `now = Clock::get()?.unix_timestamp`.
4. `require!(vk_rotation_timelock_expired(config, now), RotationTimelockNotExpired)`. The helper is defined at line 948-956 and unit-tested.
5. Reset `vk_initialized = false`, `vk_finalized = false`, `next_vk_chunk = 0`, `rotate_request_ts = 0`.
6. `config.vk_generation = config.vk_generation.checked_add(1).ok_or(Overflow)?` — bumps generation. At u16 max (65 535) errors; with a 48h timelock it would take ~358 years to overflow. Acceptable in practice but worth noting that `vk_generation` is a u16 — if SOLID-SEC-006 part 2 plans to bind generation into a public input (per ADR-0015 + the field comment at lines 920–925), the on-circuit type must agree.

**Outputs**: `Ok(())`. Side effects: VK becomes mutable again (`vk_initialized=false`), rotation cleared. Logs new generation.

**Error handling**:
- Not finalized: `VerificationKeyNotFinalized`.
- No pending: `NoPendingRotation`.
- Timelock not expired: `RotationTimelockNotExpired`.
- Generation overflow: `Overflow` (effectively unreachable).
- Clock failure: bubbled.

**Subtle observation**: this handler does NOT clear `vk_storage.data`. The next chunk-0 write replaces the Vec; until then, the OLD VK bytes remain at rest. But because `vk_initialized = false`, `verify_batch_proof` will short-circuit with `VerificationKeyNotSet` before ever reading the stale buffer. Comment at lines 293-296 documents this. ✓

**Verdict**: clean. Critical flow correctly gated.

---

## Handler 8: `set_paused`

Source: `programs/zk-verifier/src/lib.rs:323-327`.

**Inputs**: `AuthorityOnly`, `paused: bool`.

**Processing**: Writes `verifier_config.paused = paused`. No idempotency check — calling twice with the same value is a benign no-op (and one log).

**Outputs**: `Ok(())`. Log: `Verifier paused = {true|false}`.

**Error handling**: only `AuthorityOnly` has_one.

**Verdict**: clean. Minimal surface.

---

## Handler 9: `transfer_authority`

Source: `programs/zk-verifier/src/lib.rs:330-336`.

**Inputs**: `AuthorityOnly`, `new_authority: Pubkey`.

**Processing**:
- `require!(new_authority != Pubkey::default(), Unauthorized)` — refuses transfer to all-zero pubkey, which is the only "obviously broken" target. Does NOT verify the new authority can sign (no two-step accept-transfer pattern). A typo here permanently locks the program.
- Writes `config.authority = new_authority`.

**Outputs**: `Ok(())`. Log: `Authority transferred to {pubkey}`.

**Error handling**: zero-pubkey rejection only.

**Subtle observation**: standard Solana ownership-transfer hazard. **Finding M01-M01 (Medium)**: no two-step authority handover. A misconfigured `new_authority` pubkey permanently locks the program (only redeploy can recover). Recommend adding a `pending_authority: Option<Pubkey>` field + an `accept_authority` handler that the new authority must sign. This is a defensive-design gap; not a present-day soundness break, but a real operational risk. Pre-mainnet item.

**Verdict**: see finding M01-M01.

---

## Handler 10: `verify_batch_proof` — the security-critical path

Source: `programs/zk-verifier/src/lib.rs:369-588`. Account context: `VerifyBatchProof` at `programs/zk-verifier/src/lib.rs:842-894`.

This is the soundness fulcrum of the entire system. Every fact a verifier acts on flows through this handler.

### Inputs

**Accounts** (every one inspected):

1. `verifier_config` (mut, seeds=[b"verifier-config"], bump=verifier_config.bump). Reads `paused`, `vk_initialized`, `timestamp_skew_seconds`. Mutated only at the end (`proof_count` increment). The bump reuse via `bump = verifier_config.bump` validates the canonical bump.
2. `vk_storage` (seeds=[b"vk-storage", verifier_config.key().as_ref()], bump). Read-only (no `mut`). Anchor's seed validation ensures only the canonical PDA can be passed.
3. `nullifier_record` (init, payer = payer, space = 8 + NullifierRecord::SPACE = 8+48=56, seeds=[NULLIFIER_SEED, nullifier.as_ref()], bump). The `init` constraint is the atomic replay-safety gate: the create_account CPI fails if the PDA already exists (ADR-0007).
4. `global_tree` (UncheckedAccount). Externally owned by `schema-registry`. The handler owner-checks at line 446-450.
5. `schema_tree_0..3` (UncheckedAccount each). Externally owned by `schema-registry`. Owner-checked inside the loop (line 523-527).
6. `issuer_tree_binding` (UncheckedAccount, seeds=[b"issuer-tree-binding"], bump, seeds::program=ISSUER_REGISTRY_ID). The `seeds::program` directive validates that the supplied account address equals the PDA derived under `ISSUER_REGISTRY_ID`. The handler additionally explicit-owner-checks at line 467-471 (defense-in-depth).
7. `payer` (mut, Signer). Anyone willing to pay rent for the nullifier PDA can submit a verifying proof — this is intentional (verifier scope binding via `verifierAddress` public input is the access gate).
8. `system_program` (Program<System>).

**Instruction args**:

1. `proof_a: [u8; 64]` — Groth16 G1 point (X || Y, big-endian).
2. `proof_b: [u8; 128]` — G2 point (X.c0 || X.c1 || Y.c0 || Y.c1, BE).
3. `proof_c: [u8; 64]` — G1 point.
4. `public_inputs: Vec<[u8; 32]>` — must validate to length 32. Type is Vec rather than `[[u8; 32]; NR_PUBLIC_INPUTS]` for stack-frame reasons documented at lines 354-368 (~1 312 bytes inline would exceed BPF 4 KB frame).
5. `nullifier: [u8; 32]` — used as PDA seed; cross-checked against `public_inputs[0]`.

### Processing (numbered against handler structure)

**(0) Pre-flight gates** (lines 378-380):
- `require!(!config.paused, Paused)`. Single-toggle pause. ✓
- `require!(config.vk_initialized, VerificationKeyNotSet)`. Refuses verification before VK upload. ✓

**Public-input arity gate** (lines 389-392):
- `let public_inputs: &[[u8; 32]; NR_PUBLIC_INPUTS] = public_inputs.as_slice().try_into().map_err(|_| InvalidProofFormat)?`. Converts the heap `Vec` to a fixed-size array reference. Without this, every later indexed access (`public_inputs[10]`, `public_inputs[29]`, `public_inputs[31]`, the `2..6` and `6..10` ranges in the loop) could panic on a length mismatch. The `try_into` failure mode is a typed `InvalidProofFormat` rather than a panic. ✓

**(1) Nullifier-output binding** (line 396):
- `require!(nullifier == public_inputs[0], NullifierMismatch)`. The circuit's nullifier output signal (slot 0) must equal the `nullifier` argument used as a PDA seed. Without this, a prover could decouple nullifier from the proof: register PDA seeded by an attacker-chosen nullifier while the real proof's nullifier remained free for replay.

**(2) Verifier scope binding** (lines 399-404):
- `let verifier_address_input = public_inputs[VERIFIER_ADDRESS_INPUT_INDEX];` — slot 29.
- `require!(verifier_address_input == ID.to_bytes(), InvalidVerifierAddress);`. Ties the proof to *this* program's deployment. Without this, a proof generated for verifier deployment A would replay against deployment B (cross-deployment replay).

**(2b) Timestamp freshness** (lines 406-436):
- Reads `public_inputs[31]` (slot CURRENT_TIMESTAMP_INPUT_INDEX).
- `for i in 8..32 { require!(ts_bytes[i] == 0, StaleTimestamp); }` — bytes 8..32 must be zero, asserting the value fits in u64. **Subtle correctness check**: this enforces that the prover passed a u64 in the slot, not an arbitrary 32-byte field element. Without this, a prover could forge `currentTimestamp` to an arbitrary BN254 element that happens to satisfy the in-circuit `currentTimestamp <= expirationTimestamp` constraints when reduced mod r.
- `claimed_ts = u64::from_le_bytes(ts_bytes[0..8].try_into()?)`. LE encoding to match the SDK's witness packing.
- `now_i64 = Clock::get()?.unix_timestamp; require!(now_i64 >= 0, StaleTimestamp);` — defense-in-depth.
- `now = now_i64 as u64;` — safe after the >=0 check.
- `skew = config.timestamp_skew_seconds as u64;`.
- `lower = now.saturating_sub(skew); upper = now.saturating_add(skew);`. Saturating arithmetic prevents underflow on early-mainnet-time edge cases (`now < skew`) and overflow on i64::MAX-near values (irrelevant in practice).
- `require!(claimed_ts >= lower && claimed_ts <= upper, StaleTimestamp);`. Two-sided window. Without this, a prover could pass `currentTimestamp = 0` and defeat the in-circuit expiration gate (cf. SOLID-SEC-005).

**(3) Global-root binding** (lines 446-456):
- `require_keys_eq!(*ctx.accounts.global_tree.owner, SCHEMA_REGISTRY_ID, InvalidGlobalRoot);` — owner-check against `schema-registry` (ADR-0010). **This is load-bearing**: without it, an attacker can submit a system-owned account containing forged `globroot` discriminator + arbitrary root bytes, and the parser-level check would succeed against fabricated data.
- `let global_root = public_inputs[1];`.
- `try_borrow_data` to read the AccountInfo's data lifetime-correctly. ✓
- `cpi_helpers::verify_state_root_matches(&tree_account_data, &global_root)` — cf. `crates/solid-light/src/cpi_helpers.rs:289-297`. Checks: data.len() >= 40, discriminator equals `b"globroot"`, bytes [8..40] equal `expected_root`. Returns bool; bool false → `InvalidGlobalRoot`. ✓

**(3b) Issuer-tree binding** (lines 467-483):
- Owner check against `ISSUER_REGISTRY_ID`. Two-layer guard: Accounts derive uses `seeds::program = ISSUER_REGISTRY_ID` (validates address); explicit `require_keys_eq!` validates `account.owner`. The two are belt-and-suspenders because (a) Anchor's seeds::program enforces address derivation but does NOT enforce ownership of an existing account, and (b) a system-owned account cannot exist at a PDA address derived under another program (Solana invariants), but if a future Anchor bug or a runtime-edge-case violated that, the explicit owner check still holds. ✓
- `cpi_helpers::verify_issuer_tree_binding_for_proof(&issuer_binding_data, &issuer_tree_root_input)`. Cf. `crates/solid-light/src/cpi_helpers.rs:466-480`: reads `current_root` from offset [40..72], reads status from offset [80], checks discriminator = `b"issrtree"`. Returns `Result<(), LightError>`. The handler maps `IssuerTreeRootMismatch` → `IssuerTreeRootMismatch`, `IssuerTreeBindingFrozen` → `IssuerTreeBindingFrozen`, anything else → `InvalidIssuerTreeBinding`.
- `drop(issuer_binding_data);` (line 483) — explicitly drops the borrow before any subsequent mutable account access. The handler doesn't actually take a mut borrow on issuer_tree_binding later, so this drop is defensive. Acceptable.

**(4) Schema ↔ root binding loop** (lines 495-533):
- Tracks `last_schema: Option<[u8; 32]>` for canonical-ordering enforcement.
- For `i in 0..4`:
  - `merkle_root = public_inputs[2 + i]`, `schema_hash = public_inputs[6 + i]`.
  - **Padding-slot skip** (line 500-502): if both are zero, `continue`. The circuit's padding-slot test (`circuits/test/padding_slot.test.js`) is the matching off-chain proof that the padding slot doesn't impose unsatisfiable constraints.
  - **Canonical ordering** (lines 504-507): `require!(schema_hash > prev, InvalidCredentialOrder)`. Strict ascending. Prevents duplicate-schema attacks (same credential twice in different slots).
  - `last_schema = Some(schema_hash);`.
  - **Slot dispatch** (lines 517-522): match on `i` to pick the right `schema_tree_N` AccountInfo.
  - **Owner check** on the picked account (lines 523-527): `require_keys_eq!(*tree_info.owner, SCHEMA_REGISTRY_ID, InvalidSchemaRootBinding);`. Same load-bearing reasoning as global_tree.
  - **Parse + match**: `cpi_helpers::verify_schema_root_binding(&data, &merkle_root, &schema_hash)`. Cf. `crates/solid-light/src/cpi_helpers.rs:254-276`: data.len() >= 113, discriminator `b"schmtree"`, schema_hash match, current_root match, status==0 (active). Returns bool; false → `InvalidSchemaRootBinding`.

**Subtle correctness checks (loop)**:
- The padding-slot skip means a caller that wants to query `k < 4` schemas MUST place padding (`[0u8; 32]`, `[0u8; 32]`) in the unused slots. The circuit must produce `merkle_root == 0 && schema_hash == 0` for those slots; if the circuit doesn't, the on-chain check would fire spuriously. The matching circuit invariant is enforced by `circuits/test/padding_slot.test.js`. ✓
- For skipped slots, the supplied `schema_tree_i` AccountInfo is NOT examined — anyone, including a system-owned dummy. This is the natural consequence of UncheckedAccount; correct.

**(5) Groth16 verification** (lines 557-563):
- Delegates to `verify_groth16_proof` at line 727. Heavy locals (parsed VkBuf, by-value Groth16Verifyingkey, proof_a_neg, Groth16Verifier) live in that helper's frame; `#[inline(never)]` ensures LTO doesn't fold them into the wrapper.
- `verify_groth16_proof` returns `InvalidProofFormat` on parse / G1 negation failure, or `ProofVerificationFailed` on actual pairing-check failure.

**(6) Nullifier PDA materialization** (lines 567-570):
- The `init` constraint already created the PDA atomically. Now the handler populates: `nullifier`, `created_at = Clock::get()?.unix_timestamp`, `slot = Clock::get()?.slot`. Two extra Clock syscalls; not free but not significant.

**(7) Metrics + emit** (lines 572-587):
- `proof_count.saturating_add(1)`. Won't error at u64::MAX (saturates).
- `emit!(CredentialVerified { nullifier, proof_count, public_input_count: NR_PUBLIC_INPUTS as u8, timestamp: Clock::get()?.unix_timestamp });`. **Third Clock::get() call** — the handler reads Clock three times in (5), (6), (7). Each is ~1500 CU per Solana's syscall pricing; total ~4500 CU spent on syscalls that could be one local variable. Minor optimization.
- `msg!("Batch proof verified...")`.

### Outputs

- Returns `Ok(())` on success.
- Side effects: `nullifier_record` PDA created and populated, `verifier_config.proof_count` incremented, `CredentialVerified` event emitted, log line emitted.
- The created `nullifier_record` PDA's lamport balance (rent-exempt for 56 bytes) is paid by `payer`. Closed-account considerations: a future `close` instruction would reopen the replay window — currently no such instruction exists, which is correct.

### Error handling — every path enumerated

| Path | Trigger | Error |
|------|---------|-------|
| paused | `config.paused == true` | `Paused` |
| vk not set | `config.vk_initialized == false` | `VerificationKeyNotSet` |
| arity mismatch | `public_inputs.len() != 32` | `InvalidProofFormat` |
| nullifier mismatch | `nullifier != public_inputs[0]` | `NullifierMismatch` |
| wrong verifier addr | `public_inputs[29] != program ID bytes` | `InvalidVerifierAddress` |
| timestamp non-u64 | bytes [8..32] of slot 31 not zero | `StaleTimestamp` |
| timestamp parse | `try_into` on bytes [0..8] fails | `StaleTimestamp` |
| Clock <0 | `now_i64 < 0` | `StaleTimestamp` |
| timestamp out of window | `claimed_ts < lower OR > upper` | `StaleTimestamp` |
| global_tree wrong owner | `*global_tree.owner != SCHEMA_REGISTRY_ID` | `InvalidGlobalRoot` |
| global_tree borrow | `try_borrow_data` fails | bubbled `?` |
| global root mismatch | parser returns false | `InvalidGlobalRoot` |
| issuer_tree_binding wrong owner | `*issuer_tree_binding.owner != ISSUER_REGISTRY_ID` | `InvalidIssuerTreeBinding` |
| issuer_tree_binding borrow | `try_borrow_data` fails | bubbled |
| issuer_tree root mismatch | parser returns `IssuerTreeRootMismatch` | `IssuerTreeRootMismatch` |
| issuer_tree frozen | parser returns `IssuerTreeBindingFrozen` | `IssuerTreeBindingFrozen` |
| issuer_tree malformed | parser returns other error | `InvalidIssuerTreeBinding` |
| schema not ascending | `schema_hash <= prev` | `InvalidCredentialOrder` |
| schema_tree_N wrong owner | `*tree_info.owner != SCHEMA_REGISTRY_ID` | `InvalidSchemaRootBinding` |
| schema_tree_N parse fail | parser returns false | `InvalidSchemaRootBinding` |
| VK parse fail | `VkBuf::parse` errors | `InvalidProofFormat` |
| G1 negate fail | input Y > P | `InvalidProofFormat` |
| Groth16Verifier::new error | format check fails | `InvalidProofFormat` |
| Groth16 pairing fail | `verifier.verify()` error | `ProofVerificationFailed` |
| nullifier replay | PDA already exists | Anchor `AccountAlreadyInitialized` (init constraint) |
| Clock failure | sysvar read fails | bubbled `?` |

**No unwrap/expect anywhere on user-supplied data.** Every `?` is on a `Result` whose error type is sound. No panic vectors observed.

### Subtle handler-level findings

**Finding M01-Info-04**: `Clock::get()?` is invoked three times within `verify_batch_proof` (timestamp skew check, nullifier_record.created_at + .slot, and event timestamp). On BPF this is approximately 3 × 1500 CU ≈ 4500 CU of avoidable cost. Recommend caching `let now = Clock::get()?` once and reusing `.unix_timestamp` and `.slot` from the cached value.

**Finding M01-Info-05**: the public-input slot comments at lines 25-40 use Python-style inclusive-end ranges (`[2..5]` for indices 2..5). The constants at lines 47-53 are correct (10, 29, 31). The downstream specialist agent should confirm the comment-to-constant correspondence is the only source of truth here — the agent's audit of the circuit slot order will validate or invalidate.

**Finding M01-L02 (Low)**: schema_tree_N AccountInfo references are passed even when the slot is "padded" (zero schema_hash). A caller could pass arbitrary garbage AccountInfos in the padded slots — this is benign because they're never read. But it's a tiny operational footgun: error logs from validators may be confusing if a user passes a wrong account in slot 3 expecting it to be ignored. Consider documenting in `verifier-guide.md`.

**Finding M01-Info-06**: SOLID-SEC-006 part 2 — binding `vk_generation` into the public-input contract — is deferred to the next trusted-setup cycle. Today, the residual risk is theoretical: a proof generated against VK generation N can never verify against generation N+1's IC, and the nullifier registry is generation-agnostic. The deferred fix tightens the formal proof rather than closing a real hole, but it is the right pre-mainnet move.

---

## Helper: `vk_rotation_timelock_expired`

Source: `programs/zk-verifier/src/lib.rs:948-956`. Pure fn, host-testable.

**Inputs**: `&VerifierConfig`, `now_unix_ts: i64`.

**Processing**:
- If `config.rotate_request_ts == 0` → `false` (no pending rotation).
- Else `now >= config.rotate_request_ts.saturating_add(VK_ROTATION_TIMELOCK_SECONDS)`.

**Outputs**: bool.

**Error handling**: pure, no panics, saturating arithmetic.

**Coverage**: tests `vk_rotation_not_expired_when_no_pending_request`, `vk_rotation_not_expired_inside_window`, `vk_rotation_expired_at_and_beyond_timelock`, `vk_rotation_handles_saturation_safely` (all at lines 1150-1215). Comprehensive boundary coverage. ✓

**Verdict**: clean. Excellent test coverage of edge cases (saturation, boundary).

---

## Helper: `VkBuf::parse` and `as_verifying_key`

Source: `programs/zk-verifier/src/lib.rs:644-708`.

**Inputs (parse)**: `&[u8]` (raw VK bytes from `vk_storage.data`).

**Processing (parse)**:
1. Length check: `bytes.len() >= HEADER (= 4 + 64 + 128*3 = 452)`. Returns `TooShort` else.
2. Read `nr_ic` from bytes[0..4] LE.
3. Validate `nr_ic > 0 && nr_ic <= MAX_IC` → returns `IcOverflow`.
4. Validate `bytes.len() >= HEADER + nr_ic * 64` → returns `TruncatedIc`.
5. Copy fixed scalars: alpha [4..68], beta [68..196], gamma [196..324], delta [324..452]. **Hand-verified**: cursor advances 4 + 64 + 128 + 128 + 128 = 452 == HEADER. ✓
6. Allocate `Vec<[u8; 64]>` with capacity exactly `nr_ic`.
7. Copy each IC point.

**Outputs**: `VkBuf` or `VkParseError`.

**Error handling**: `TooShort`, `IcOverflow`, `TruncatedIc`. No panics; every slice copy is preceded by a length check.

**Subtle correctness checks**:
- The cursor arithmetic is hand-rolled rather than using `std::io::Cursor` or similar. Hand-verified above. ✓
- `Vec::with_capacity(nr_ic)` — exactly the right size; no over-allocation. ✓
- The IC table on the heap means `VkBuf` shell stays small (~480 bytes including the Vec header), preserving stack budget.

**Coverage**: tests `vk_parse_round_trips_minimum_size`, `vk_parse_round_trips_max_ic`, `vk_parse_rejects_too_short_header`, `vk_parse_rejects_truncated_ic_region`, `vk_parse_rejects_zero_ic`, `vk_parse_rejects_ic_overflow`, `vk_view_borrows_from_buffer`, `vk_buf_fits_in_stack_budget`. All boundary cases. ✓

**`as_verifying_key`** (lines 696-708):
- Returns a `Groth16Verifyingkey` borrowing from `self`.
- Fields: `nr_pubinputs = nr_ic.saturating_sub(1)` — IC count = pubinputs + 1.
- The `vk_gamme_g2` field name in `Groth16Verifyingkey` is a typo in the upstream `groth16-solana` crate (should be `gamma_g2`). The handler uses it correctly. **Finding M01-Info-07**: upstream typo not in our control; document in dependency notes.
- `vk_ic: &self.ic[..self.nr_ic]` — slices the heap Vec. Length is invariant by construction.

**Verdict**: clean. Robust parser, well-tested, stack-conscious.

---

## Helper: `verify_groth16_proof`

Source: `programs/zk-verifier/src/lib.rs:727-751`.

**Inputs**: `&[u8]` (vk bytes), `&[u8; 64]` proof_a, `&[u8; 128]` proof_b, `&[u8; 64]` proof_c, `&[[u8; 32]; NR_PUBLIC_INPUTS]` public_inputs.

**Processing**:
1. `VkBuf::parse(vk_storage_data)` → `InvalidProofFormat` on error.
2. `VkBuf::as_verifying_key()` → borrowed view.
3. `negate_g1_point(proof_a)` → `InvalidProofFormat` on borrow.
4. `Groth16Verifier::<NR_PUBLIC_INPUTS>::new(...)` → `InvalidProofFormat` on construction error.
5. `verifier.verify()` → `ProofVerificationFailed`.

**Outputs**: `Result<()>`. Pure.

**Error handling**: each call site maps to a typed `ErrorCode`. No panics.

**`#[inline(never)]` discipline**: documented at lines 711-725. Without it, the heavy locals merge into the wrapper frame and exceed BPF's 4 KB. ✓

---

## Helper: `negate_g1_point`

Source: `programs/zk-verifier/src/lib.rs:755-784`.

**Inputs**: `&[u8; 64]` (a G1 point in 32-byte BE X || 32-byte BE Y form).

**Processing**:
- BN254 base prime `P_BE` hardcoded as `[u8; 32]`. ✓ (Cross-checked against the BN254 spec.)
- Out X = X (unchanged).
- Out Y = P - Y, computed byte-by-byte big-endian with explicit borrow tracking.

**Outputs**: `[u8; 64]` or `()` error (caller maps to `InvalidProofFormat`).

**Error handling**: returns `Err(())` if final borrow is non-zero (Y > P, indicating non-canonical input). No panics.

**Subtle correctness checks**:
- Hand-rolled big-int subtraction in 32 BE bytes. The borrow propagation `borrow = 1 if diff < 0 else 0` is the textbook recipe; visually verified.
- The `for i in (0..32).rev()` traversal is BE-correct.
- The diff computation uses `i16` to absorb the borrow; max negative is `-1 - 255 = -256`, fits in i16; max positive is `255`. After `+= 256`, `diff in [0, 255]`, cast to u8 is well-defined. ✓

**Coverage**: tests `negate_g1_zero_y_is_field_prime`, `negate_g1_is_involutive_modulo_p` (lines 1238-1260). Involution test is clever: pick a random Y, negate twice, expect the input. ✓

**Verdict**: clean. Hand-rolled but correct; tests prove involution.

---

## State accounts

### `VerifierConfig` (lines 898-942)

| Field | Type | Bytes | Purpose |
|-------|------|-------|---------|
| authority | Pubkey | 32 | upgrade authority |
| proof_count | u64 | 8 | metrics |
| vk_initialized | bool | 1 | gate for `verify_batch_proof` |
| paused | bool | 1 | emergency pause |
| bump | u8 | 1 | canonical PDA bump |
| next_vk_chunk | u16 | 2 | chunked-upload cursor |
| timestamp_skew_seconds | u32 | 4 | SEC-005 freshness window |
| vk_finalized | bool | 1 | SEC-006 freeze gate |
| vk_generation | u16 | 2 | SEC-006 monotonic counter |
| rotate_request_ts | i64 | 8 | SEC-006 timelock anchor |

Total = 60 bytes. `SPACE` constant at line 941 sums to 60. Verified by unit test `verifier_config_space_matches_layout` at line 1218-1227. ✓

**Hand-verified**: 32 + 8 + 1 + 1 + 1 + 2 + 4 + 1 + 2 + 8 = 60. ✓

The `Initialize` context (line 802) allocates `8 + VerifierConfig::SPACE = 68` bytes (8-byte Anchor discriminator + 60 fields). ✓

### `VkStorage` (lines 958-961)

```rust
pub struct VkStorage {
    pub data: Vec<u8>,
}
```

Borsh serializes `Vec<u8>` as `4-byte LE length + N bytes`. Account allocated at `8 + 4 + 10228 = 10240` (Anchor disc + Vec len prefix + max payload). 10240 == `MAX_PERMITTED_DATA_INCREASE` per CPI. The handler's `VK_MAX_BYTES = 10_228` keeps cumulative payload within the allocation. ✓

### `NullifierRecord` (lines 963-972)

| Field | Type | Bytes |
|-------|------|-------|
| nullifier | [u8; 32] | 32 |
| created_at | i64 | 8 |
| slot | u64 | 8 |

`SPACE = 48`, allocation = `8 + 48 = 56`. ✓

---

## Errors (line 976-1030)

26 variants. Every code path observed in handlers maps to one of these. No undocumented error paths.

A few that warrant a closer look:
- `Unauthorized` — overloaded: used both for the wrong-signer case AND the "transfer to default pubkey" case. Two different conditions sharing one error code is a minor UX gap. **Finding M01-Info-08**: split into `UnauthorizedSigner` and `InvalidNewAuthority` for better caller diagnosability.
- `StaleTimestamp` — overloaded for four conditions: bytes 8..32 non-zero, parse failure, Clock <0, out-of-window. Acceptable; off-chain logs make the distinction observable.

---

## Tests (lines 1038-1261)

Coverage:
- VK parser (6 tests): minimum, max, too short, truncated IC, zero IC, IC overflow, view borrow, stack budget.
- Timelock helper (4 tests): no pending, inside window, at boundary, beyond, saturation.
- VerifierConfig SPACE invariant (1 test).
- Timelock constant (1 test, doc-as-test).
- G1 negation (2 tests): zero-Y, involution.

What's NOT covered (gap):
- The `verify_batch_proof` body itself. End-to-end proof verification requires either a localnet validator (not host-testable) or a stub Groth16 verifier. Currently exercised by the integration test plan but only `01_registry_init.test.ts` is implemented.
- The `set_paused` / `transfer_authority` handlers (trivial; low priority).

**Finding M01-M02 (Medium)**: missing host-testable property tests for `verify_batch_proof`'s pre-Groth16 gates (arity, nullifier match, verifier address, timestamp window, owner checks, schema ordering). All eight gates are pure-function-ish (Clock-mockable) and could be lifted into a host-testable harness using `solana-program-test` or by hand-mocking AccountInfo bytes. This would close the gap between the parser tests (which exist) and the handler tests (which don't).

---

## Cross-references to solid-light helpers used

The handler depends on these `cpi_helpers` items (file: `crates/solid-light/src/cpi_helpers.rs`):

1. `SCHEMA_REGISTRY_ID` (line 47) — typed Pubkey, drift-tested at lines 102-126.
2. `ISSUER_REGISTRY_ID` (line 68) — typed Pubkey, drift-tested at lines 130-150.
3. `verify_state_root_matches` (line 289) — pure parser; tests at lines 584-595.
4. `verify_schema_root_binding` (line 254) — pure parser; tests at lines 537-581.
5. `verify_issuer_tree_binding_for_proof` (line 466) — pure parser; tests at lines 729-764.

All five helpers are extensively unit-tested with mismatch-axis coverage. **No findings on this surface from the zk-verifier perspective**; the wave-1 solid-light specialist owns deeper analysis.

**Cross-cutting observation**: the program-ID byte arrays at `cpi_helpers.rs:54-57` and `:75-78` are hand-transcribed from base58 literals. The four drift tests (`schema_registry_id_bytes_matches_program_id_literal`, `schema_registry_id_typed_matches_program_id_literal`, `issuer_registry_id_bytes_matches_program_id_literal`, `issuer_registry_id_typed_matches_program_id_literal`) gate against silent transcription drift. ✓

**Doc-drift item observed in passing**: `crates/solid-light/src/credential_tree.rs:62` claims nullifier = `Poseidon(masterKey, verifierAddress, schemaHash)` — a 3-input formulation. Post ADR-0006 revision, it is a 6-input Poseidon (identityCommitment, secret, schemaId, credentialIndex, currentTimestamp, issuerTreeRoot). The struct itself is just a leaf-data type for `build_insert_nullifier_data`; the comment is stale documentation. **Finding M01-L03 (Low)**: stale doc comment in `credential_tree.rs:62`. To be folded into the docs-drift wave-2 doc.

---

## Findings summary (orchestrator hand-audit only; agent findings synthesized later)

| ID | Severity | Location | Issue |
|----|----------|----------|-------|
| M01-M01 | Medium | `lib.rs:330-336` (transfer_authority) | No two-step authority handover; typo locks program until redeploy. Pre-mainnet item. |
| M01-M02 | Medium | `lib.rs:369-588` (verify_batch_proof) | No host-testable harness for the eight pre-Groth16 gates; integration tests cover only `01_registry_init`. |
| M01-L01 | Low | `lib.rs:25-40` (slot doc-comment) | Doc comment uses inclusive-end range notation that conflicts with Rust's half-open ranges. |
| M01-L02 | Low | `lib.rs:842-894` (VerifyBatchProof accounts) | Padded `schema_tree_N` slots accept any AccountInfo with no validation; potential operational footgun. |
| M01-L03 | Low | `cpi_helpers.rs / credential_tree.rs:62` | Stale comment claims 3-input nullifier; actual is 6-input post ADR-0006 rev. |
| M01-Info-01 | Info | `lib.rs:93-96` | The compile-time `VkBuf` size guard is excellent; analogous guard for `VerifyBatchProof` accounts struct missing. |
| M01-Info-02 | Info | `lib.rs:136-144` | `set_timestamp_skew` accepts 0 (admin-only knob; could break valid proofs if abused). Document. |
| M01-Info-03 | Info | `lib.rs:823-840` | `vk_storage.init_if_needed` is signer-gated but worth doc-noting for future auditors. |
| M01-Info-04 | Info | `lib.rs:557-587` | Three `Clock::get()?` calls in `verify_batch_proof`; ~4500 CU saving by caching once. |
| M01-Info-05 | Info | `lib.rs:25-40` | Slot doc comments are the source-of-truth shadow of the constants at lines 47-53; the circuit specialist's audit confirms or invalidates. |
| M01-Info-06 | Info | `lib.rs:920-925` | SOLID-SEC-006 part 2 deferred; no current soundness break, but pre-mainnet rigor item. |
| M01-Info-07 | Info | `lib.rs:704` (`vk_gamme_g2`) | Upstream typo in `groth16-solana`; not our concern but noted for dependency-audit. |
| M01-Info-08 | Info | `lib.rs:984` | `Unauthorized` overloaded across two distinct conditions. |

**Critical / High findings**: none in this hand-audit. The handler's load-bearing checks (owner-checks, arity, nullifier binding, verifier address, timestamp window, schema ordering, atomic replay, VK lifecycle) are all in place and correctly implemented.

---

## CU envelope (orchestrator's quick estimate)

`verify_batch_proof` cost decomposition (rough, pre-CI-instrumentation):

| Phase | Estimated CU | Notes |
|-------|--------------|-------|
| Anchor wrapper deser of args (~1312 bytes) | ~5 000 | borsh deser of proof + vec + nullifier |
| Account validation (init nullifier_record) | ~10 000 | system_program create_account CPI |
| Public-input arity gate (try_into) | ~500 | trivial |
| Nullifier match | ~100 | byte cmp |
| Verifier address match | ~100 | byte cmp |
| Timestamp window check | ~3 000 | Clock::get + 24-byte loop + arith |
| Global root: borrow + parse + cmp | ~2 000 | discriminator+root cmp |
| Issuer tree binding: borrow + parse + cmp | ~2 000 | discriminator+root cmp |
| Schema loop (×4 iterations × parse) | ~10 000 | discriminator+root+schema+status |
| VkBuf::parse | ~5 000 | header+IC copies |
| negate_g1_point | ~1 000 | hand-rolled bigint |
| Groth16Verifier::new + verify | ~250 000 — 320 000 | alt_bn128 syscalls |
| Nullifier PDA write + Clock + Slot | ~5 000 | 2 syscalls + write |
| Event emit + log | ~3 000 | borsh + msg! |
| **Total**                                  | **~300 000 — 370 000** | well under 1.4M |

Headroom against 1.4M ceiling: ~1.0M CU. Comfortable.

The dominant cost is Groth16 verification itself; nothing in the handler's pre-Groth16 surface is wasteful enough to merit immediate optimization. Three `Clock::get` calls are the only realistic micro-optimization; one cached read saves ~3K CU.

---

## Suggested next actions (orchestrator-level)

1. (Low effort, immediate): cache `Clock::get()?` once in `verify_batch_proof`. ~3 KB CU saving.
2. (Low effort): split `Unauthorized` into `UnauthorizedSigner` + `InvalidNewAuthority`. Caller diagnosability.
3. (Medium effort): add two-step authority handover (`pending_authority` + `accept_authority`). Pre-mainnet operational risk reduction. Tracks toward SOLID-SEC-043's spirit.
4. (Medium effort): host-testable harness for `verify_batch_proof` pre-Groth16 gates. Closes the test-coverage gap between parser tests and integration tests.
5. (Low effort): rewrite slot doc comments at lines 25-40 with explicit indices (`[2], [3], [4], [5]`) or `[2..=5]`.
6. (Low effort): fix stale comment at `credential_tree.rs:62` (folded into docs-drift wave-2).
7. (Medium effort): add a compile-time `static_assert!(size_of::<VerifyBatchProof>() < N)` guard analogous to `VkBuf`. Prevents silent BPF stack regressions.
8. (No effort, just record): SOLID-SEC-006 part 2 stays in the roadmap; ship with the next trusted-setup cycle.

---

## End of orchestrator hand-audit

This file is the standard against which the delegated specialist agent's `01_modular/01_zk_verifier.md` is reviewed in wave 2. Findings above are merged with the agent's findings (deduplicated by location) into the master roadmap.
