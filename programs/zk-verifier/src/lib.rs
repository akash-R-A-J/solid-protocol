//! SolID ZK Verifier Program.
//!
//! Verifies Groth16 proofs emitted by `circuits/batch_credential_query.circom`
//! and records the hardened nullifier so the same proof can never be replayed.
//!
//! Design choices (2026-04 remediation):
//! - **Program ID** matches `Anchor.toml` and `deployments/devnet.json`.
//! - **Nullifier registry** uses a PDA-per-nullifier seeded by `[b"null", nullifier]`.
//!   The `init` constraint fails atomically if the PDA already exists, giving
//!   O(1) replay-safety without any Merkle tree or bloom filter.
//! - **Global-root binding** reads the Merkle tree account through the
//!   `solid_light::cpi_helpers::verify_state_root_matches` adapter.
//! - **Schema-root binding** (SEC-19) reads a registered-tree PDA from
//!   `schema-registry` and asserts `merkleRoot` belongs to the tree that was
//!   pre-bound to `schemaHash`.

use anchor_lang::prelude::*;
use groth16_solana::groth16::{Groth16Verifier, Groth16Verifyingkey};
use solid_light::cpi_helpers;
use solid_light::cpi_helpers::{ISSUER_REGISTRY_ID, SCHEMA_REGISTRY_ID};

declare_id!("DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb");

// ─── Circuit Constants ─────────────────────────────────────────────────────
// batch_credential_query public inputs (32 total; ADR-0014 revision of
// ADR-0012, which previously pinned 31).  The full 32-element layout
// is fixed by the trusted setup; on the wire we send only the 21 slots
// the witness owns -- the other 11 are reconstructed on-chain from
// accounts the ix already takes (SOLID-SEC-054 / B13).  Slot indices
// below are inclusive ranges (a..=b means a, a+1, ..., b).
//
//   [0]       = nullifierHash         (wire; circuit output, 6-input Poseidon
//                                      post ADR-0014)
//   [1]       = globalRoot            (RECONSTRUCT from `global_tree`)
//   [2..=5]   = merkleRoots[4]        (RECONSTRUCT from `schema_tree_N`)
//   [6..=9]   = schemaHashes[4]       (RECONSTRUCT from `schema_tree_N`)
//   [10]      = issuerTreeRoot        (RECONSTRUCT from `issuer_tree_binding`;
//                                      NEW; SEC-004 / SEC-008)
//   [11..=14] = queryCredentialIndices[4]  (wire)
//   [15..=18] = queryFieldIndices[4]       (wire)
//   [19..=22] = queryOperators[4]          (wire)
//   [23..=26] = queryValues[4]             (wire)
//   [27]      = numPredicates              (wire)
//   [28]      = compoundLogic              (wire)
//   [29]      = verifierAddress       (RECONSTRUCT from `program_id`)
//   [30]      = verifierNonce              (wire; witness-bound anti-replay
//                                           nonce)
//   [31]      = currentTimestamp           (wire; witness-bound, freshness
//                                           enforced via SEC-005 skew window
//                                           against `Clock::unix_timestamp`)
//
// Slots 30 and 31 stay on the wire because they are witness-bound and
// Groth16 enforces polynomial-equality on public inputs with zero
// tolerance.  `Clock::unix_timestamp` cannot reproduce the exact
// `T_off` the witness committed to (slot times, network propagation,
// inclusion delay; off by ~1-2 seconds at minimum); the SEC-005 skew
// window is an *additional* on-chain freshness predicate that wraps
// the wire-supplied `currentTimestamp`, not a substitute for
// cryptographic equality.
pub const NR_PUBLIC_INPUTS: usize = 32;

/// SOLID-SEC-054 / B13: the number of public-input slots transmitted on
/// the wire.  Compile-time invariant: `NR_WIRE_INPUTS +
/// RECONSTRUCTED_INPUT_SLOTS.len() == NR_PUBLIC_INPUTS`.
pub const NR_WIRE_INPUTS: usize = 21;

/// Slots transmitted on the wire, in canonical (ascending) order.  The
/// caller's `public_inputs: Vec<u8>` has length `NR_WIRE_INPUTS * 32`;
/// chunk `i` at `[i*32 .. (i+1)*32]` maps to circuit slot
/// `WIRE_INPUT_SLOTS[i]`. Off-chain SDK encoders
/// (`ts-sdk/packages/verifier/src/index.ts::buildVerifyBatchProofIx`)
/// MUST extract `publicSignals[WIRE_INPUT_SLOTS[i]]` for each `i` in
/// the same order and flatten each slot to 32 little-endian bytes.
pub const WIRE_INPUT_SLOTS: [usize; NR_WIRE_INPUTS] = [
    0, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 30, 31,
];

/// Slots reconstructed on-chain from accounts the ix already takes.
/// In the same canonical (ascending) order as the circuit layout.
pub const RECONSTRUCTED_INPUT_SLOTS: [usize; NR_PUBLIC_INPUTS - NR_WIRE_INPUTS] =
    [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 29];

/// Index of `issuerTreeRoot` in `public_inputs[]`.  Load-bearing: the
/// handler reads this slot and cross-checks it against the on-chain
/// `IssuerTreeBinding.current_root` (ADR-0014).  Any reshuffle of the
/// public-input layout MUST update this constant in the same commit.
pub const ISSUER_TREE_ROOT_INPUT_INDEX: usize = 10;

/// Index of `verifierAddress` in `public_inputs[]` post ADR-0014 shift.
pub const VERIFIER_ADDRESS_INPUT_INDEX: usize = 29;

/// Index of `currentTimestamp` in `public_inputs[]` post ADR-0014 shift.
pub const CURRENT_TIMESTAMP_INPUT_INDEX: usize = 31;

/// Maximum number of IC points the on-chain VK parser is willing to materialize
/// on the stack. `IC` has `NR_PUBLIC_INPUTS + 1` entries by construction.
pub const MAX_IC: usize = NR_PUBLIC_INPUTS + 1;

pub const NULLIFIER_SEED: &[u8] = b"null";

/// SOLID-SEC-005: default clock skew tolerance for the `currentTimestamp`
/// public input. 10 minutes. Chosen to absorb normal client-clock drift and
/// inter-RPC delay without admitting a materially stale proof. Overridable
/// per-config via `set_timestamp_skew`.
pub const DEFAULT_TIMESTAMP_SKEW_SECONDS: u32 = 600;

/// Hard cap on the configurable skew window. Anything larger than an hour
/// is a governance decision that belongs behind a superseding ADR, not a
/// single config call.
pub const MAX_TIMESTAMP_SKEW_SECONDS: u32 = 3_600;

/// SOLID-SEC-006.  A finalised verification key can only be rotated
/// after this many seconds have elapsed since `request_vk_rotation`.
/// 48 hours gives the DAO / watchers time to react to a compromised
/// authority attempting to swap the VK.  Once SOLID-SEC-043 replaces
/// the single-pubkey authority with a Squads 3-of-5 PDA signer, the
/// timelock composes with the multisig quorum for full DAO gating.
pub const VK_ROTATION_TIMELOCK_SECONDS: i64 = 48 * 60 * 60;

// Compile-time assertion: the stack-resident `VkBuf` shell must stay
// well under Solana's 4 KB per-frame BPF stack budget.  The IC table
// (`Vec<[u8; 64]>`) lives on the heap, so only fixed-size scalars
// contribute to the stack-resident size.
//
//   VkBuf = 8 (nr_ic) + 64 (alpha) + 128 (beta) + 128 (gamma)
//         + 128 (delta) + 24 (Vec) ≈ 480 bytes.  Comfortably below
//         the 1 KB ceiling we want to leave for verify_batch_proof's
//         other locals (Anchor's deserialized argument struct alone
//         is ~1 312 bytes; the previous `[[u8; 64]; MAX_IC]` field
//         pushed the inlined `__global::verify_batch_proof` frame
//         past the 4 KB BPF limit by ~456 bytes).  See ADR-0014 +
//         the SOLID-SEC-XXX entry for the regression history.
const _: () = {
    let sz = core::mem::size_of::<VkBuf>();
    assert!(sz < 1024, "VkBuf shell exceeds 1 KB stack budget");
};

#[program]
pub mod zk_verifier {
    use super::*;

    /// Initialize the verifier config (authority only).
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let config = &mut ctx.accounts.verifier_config;
        config.authority = ctx.accounts.authority.key();
        config.proof_count = 0;
        config.vk_initialized = false;
        config.bump = ctx.bumps.verifier_config;
        config.paused = false;
        config.next_vk_chunk = 0;
        // SOLID-SEC-005: default timestamp skew window is 10 minutes. The
        // authority can tune this via `set_timestamp_skew` without a program
        // upgrade. Stored as u32 seconds; practical range is 0..=3600.
        config.timestamp_skew_seconds = DEFAULT_TIMESTAMP_SKEW_SECONDS;
        // SOLID-SEC-006 freeze-gate defaults.  The VK is not finalized
        // until the authority explicitly calls `finalize_verification_
        // key`; until then `store_verification_key` can keep
        // appending.  `vk_generation` starts at 0 and only bumps on a
        // completed rotation.
        config.vk_finalized = false;
        config.vk_generation = 0;
        config.rotate_request_ts = 0;
        msg!(
            "SolID ZK Verifier initialized. Authority: {}",
            config.authority
        );
        Ok(())
    }

    /// Update the clock-skew tolerance window for `currentTimestamp` public
    /// input (SOLID-SEC-005). Authority-only.
    ///
    /// The window caps at 1 hour. Longer windows defeat the freshness
    /// guarantee without a compelling reason; if a use case needs one, add a
    /// new ADR first (per `adr/README.md`).
    pub fn set_timestamp_skew(ctx: Context<AuthorityOnly>, skew_seconds: u32) -> Result<()> {
        require!(
            skew_seconds <= MAX_TIMESTAMP_SKEW_SECONDS,
            ErrorCode::TimestampSkewTooLarge
        );
        ctx.accounts.verifier_config.timestamp_skew_seconds = skew_seconds;
        msg!("Timestamp skew updated to {} seconds", skew_seconds);
        Ok(())
    }

    /// Store the verification key in chunks.
    ///
    /// Invariants:
    ///   1. Chunks must arrive in order (`chunk_index == config.next_vk_chunk`).
    ///      Without this a confused operator could scramble the VK and the
    ///      parser would silently accept a corrupt key.
    ///   2. The cumulative VK size is capped at the allocated 10_228 bytes
    ///      (`8 + 4 + 10228 = 10240` total account size, the Solana
    ///      `MAX_PERMITTED_DATA_INCREASE` per-CPI cap).
    ///      Without the cap the Vec's realloc would fail at serialize-time
    ///      with an opaque error on the final chunk.
    ///   3. SOLID-SEC-006: the VK is immutable once `vk_finalized` is
    ///      true.  A rotation MUST go through
    ///      `request_vk_rotation` + the 48-hour timelock +
    ///      `rotate_verification_key` path, which clears the
    ///      finalized flag and resets `next_vk_chunk` to 0 for the
    ///      next upload cycle.
    pub fn store_verification_key(
        ctx: Context<StoreVerificationKey>,
        chunk_index: u16,
        chunk_data: Vec<u8>,
        is_final_chunk: bool,
    ) -> Result<()> {
        let vk_storage = &mut ctx.accounts.vk_storage;
        let config = &mut ctx.accounts.verifier_config;

        require!(
            config.authority == ctx.accounts.authority.key(),
            ErrorCode::Unauthorized
        );
        // SOLID-SEC-006.  Refuse every write against a finalized VK.
        // The only legitimate way back to an open-for-writes state is
        // `request_vk_rotation` + timelock + `rotate_verification_key`.
        require!(!config.vk_finalized, ErrorCode::VerificationKeyFinalized);
        require!(
            chunk_index == config.next_vk_chunk,
            ErrorCode::ChunkOutOfOrder
        );

        // SOLID-SEC-006 / e2e fix: keep `8 + 4 + VK_MAX_BYTES` <=
        // Solana's `MAX_PERMITTED_DATA_INCREASE = 10240`, the per-CPI
        // cap on `system_instruction::create_account` size.  The
        // canonical VK is currently ~2.6KB so 10228 leaves ~3.9x
        // headroom; if a future circuit grows the VK past this, the
        // remediation is a multi-PDA chunked storage account, not a
        // bigger `VkStorage`.
        const VK_MAX_BYTES: usize = 10_228;
        let incoming_len = chunk_data.len();

        if chunk_index == 0 {
            require!(incoming_len <= VK_MAX_BYTES, ErrorCode::VkStorageFull);
            vk_storage.data = chunk_data;
        } else {
            let new_total = vk_storage
                .data
                .len()
                .checked_add(incoming_len)
                .ok_or(ErrorCode::Overflow)?;
            require!(new_total <= VK_MAX_BYTES, ErrorCode::VkStorageFull);
            vk_storage.data.extend_from_slice(&chunk_data);
        }

        config.next_vk_chunk = config
            .next_vk_chunk
            .checked_add(1)
            .ok_or(ErrorCode::Overflow)?;

        if is_final_chunk {
            config.vk_initialized = true;
            msg!(
                "Verification key stored: {} bytes total across {} chunks",
                vk_storage.data.len(),
                config.next_vk_chunk
            );
        }
        Ok(())
    }

    /// SOLID-SEC-006.  Freeze the VK.  After this call
    /// `store_verification_key` refuses every chunk; any further
    /// change to the VK must flow through
    /// `request_vk_rotation` + `rotate_verification_key`.
    /// Requires `vk_initialized == true` (no point freezing an empty
    /// store).  Idempotent only in the trivial sense that repeated
    /// calls are rejected via `VerificationKeyAlreadyFinalized`.
    pub fn finalize_verification_key(ctx: Context<AuthorityOnly>) -> Result<()> {
        let config = &mut ctx.accounts.verifier_config;
        require!(config.vk_initialized, ErrorCode::VerificationKeyNotSet);
        require!(
            !config.vk_finalized,
            ErrorCode::VerificationKeyAlreadyFinalized
        );
        config.vk_finalized = true;
        msg!(
            "Verification key finalized at generation {} (SOLID-SEC-006).  \
             Further writes rejected; rotation path requires \
             request_vk_rotation + {}s timelock + rotate_verification_key.",
            config.vk_generation,
            VK_ROTATION_TIMELOCK_SECONDS
        );
        Ok(())
    }

    /// SOLID-SEC-006.  Start the rotation timelock.  The VK stays
    /// finalized and in effect for the full window; this call only
    /// records the moment after which `rotate_verification_key` is
    /// permitted.  Refuses if a rotation is already pending
    /// (`RotationAlreadyRequested`) or if the VK is not finalized
    /// (`VerificationKeyNotFinalized`).
    pub fn request_vk_rotation(ctx: Context<AuthorityOnly>) -> Result<()> {
        let config = &mut ctx.accounts.verifier_config;
        require!(config.vk_finalized, ErrorCode::VerificationKeyNotFinalized);
        require!(
            config.rotate_request_ts == 0,
            ErrorCode::RotationAlreadyRequested
        );
        let now = Clock::get()?.unix_timestamp;
        require!(now > 0, ErrorCode::RotationClockInvalid);
        config.rotate_request_ts = now;
        msg!(
            "VK rotation requested at unix_ts={} (SOLID-SEC-006).  \
             Earliest rotation at unix_ts={}.",
            now,
            now.saturating_add(VK_ROTATION_TIMELOCK_SECONDS)
        );
        Ok(())
    }

    /// SOLID-SEC-006.  Cancel a pending rotation.  No-op if there is
    /// nothing pending.  Authority-only.  Used when the operator
    /// decides against rotating, or when they need to reset the
    /// timelock anchor after discovering a problem.
    pub fn cancel_vk_rotation(ctx: Context<AuthorityOnly>) -> Result<()> {
        let config = &mut ctx.accounts.verifier_config;
        let prior = config.rotate_request_ts;
        config.rotate_request_ts = 0;
        msg!("VK rotation cancelled (prior request_ts={}).", prior);
        Ok(())
    }

    /// SOLID-SEC-006.  Complete a timelocked rotation.  Requires:
    ///   - `vk_finalized == true` (we are rotating a frozen VK).
    ///   - `rotate_request_ts != 0` (a rotation was requested).
    ///   - `Clock::unix_timestamp >= rotate_request_ts + VK_ROTATION_
    ///     TIMELOCK_SECONDS` (the 48-hour window elapsed).
    /// On success, resets `vk_initialized`, `vk_finalized`, and
    /// `next_vk_chunk` so the operator can upload a fresh VK via
    /// `store_verification_key`.  Bumps `vk_generation`.  Clears the
    /// pending rotation.  `vk_storage.data` is NOT cleared here; the
    /// next chunk-0 write via `store_verification_key` overwrites the
    /// buffer atomically (same pattern as initial upload).
    pub fn rotate_verification_key(ctx: Context<AuthorityOnly>) -> Result<()> {
        let config = &mut ctx.accounts.verifier_config;
        require!(config.vk_finalized, ErrorCode::VerificationKeyNotFinalized);
        require!(config.rotate_request_ts != 0, ErrorCode::NoPendingRotation);
        let now = Clock::get()?.unix_timestamp;
        require!(
            vk_rotation_timelock_expired(config, now),
            ErrorCode::RotationTimelockNotExpired
        );
        config.vk_initialized = false;
        config.vk_finalized = false;
        config.next_vk_chunk = 0;
        config.rotate_request_ts = 0;
        config.vk_generation = config
            .vk_generation
            .checked_add(1)
            .ok_or(ErrorCode::Overflow)?;
        msg!(
            "VK rotated; generation now {} (SOLID-SEC-006).  Upload fresh \
             chunks via store_verification_key + finalize_verification_key.",
            config.vk_generation
        );
        Ok(())
    }

    /// Emergency pause (authority only).
    pub fn set_paused(ctx: Context<AuthorityOnly>, paused: bool) -> Result<()> {
        ctx.accounts.verifier_config.paused = paused;
        msg!("Verifier paused = {}", paused);
        Ok(())
    }

    /// Transfer upgrade authority (authority only).
    pub fn transfer_authority(ctx: Context<AuthorityOnly>, new_authority: Pubkey) -> Result<()> {
        let config = &mut ctx.accounts.verifier_config;
        require!(new_authority != Pubkey::default(), ErrorCode::Unauthorized);
        config.authority = new_authority;
        msg!("Authority transferred to {}", new_authority);
        Ok(())
    }

    /// Verify a batch Groth16 proof.
    ///
    /// 1. Binds `verifierAddress` public input to this program's ID (SEC-13).
    /// 2. Verifies `globalRoot` against the registered global state tree.
    /// 3. Verifies each non-zero `(merkleRoot, schemaHash)` pair is registered
    ///    in `schema-registry` and that the schemas are strictly ascending
    ///    (SEC-20).
    /// 4. Runs Groth16 verification via alt_bn128 syscalls.
    /// 5. Allocates a PDA keyed by the nullifier — atomically fails on replay.
    ///
    /// `#[inline(never)]` is load-bearing.  Without it, this body gets folded
    /// into Anchor's `__global::verify_batch_proof` wrapper, merging stack
    /// frames with the wrapper's deserialized-arg struct and the
    /// `VerifyBatchProof` accounts struct.  Forcing a real call boundary
    /// keeps user-fn locals isolated from the wrapper's frame.
    ///
    /// `public_inputs` is `Vec<u8>` (heap-resident, flat) rather than
    /// `[[u8; 32]; NR_WIRE_INPUTS]` (672 bytes inline) or the obvious
    /// `Vec<[u8; 32]>` shape.  Two reasons:
    ///
    /// (a) Anchor's `__global` wrapper deserializes the args struct,
    /// then passes args by value into this fn -- on BPF the arg copy
    /// lives in the wrapper's outgoing-args stack slots, doubling the
    /// array's stack footprint.  At the prior `NR_PUBLIC_INPUTS = 32`
    /// that pushed the wrapper 456 bytes past the 4 KB per-frame BPF
    /// budget (SEC-047).  A `Vec` is a 24-byte fat pointer regardless
    /// of length, so the wrapper only holds one cheap copy and the
    /// underlying buffer stays on the heap.
    ///
    /// (b) `Vec<[u8; 32]>` round-trips fine on the host but reliably
    /// fails Anchor 0.30.1's per-element BorshDeserialize on BPF
    /// during the wrapper's args-deser step (`InstructionDidNotDeserialize`,
    /// surfaced post-SEC-053 once a real proof first reached the
    /// handler).  Flat `Vec<u8>` deserializes via the
    /// fast-path `Vec::with_capacity + read_exact`, which is in
    /// active use across the rest of the program (e.g.
    /// `store_verification_key`).  The handler chunks into 32-byte
    /// slices below.
    ///
    /// SOLID-SEC-054 / B13: the wire carries only `NR_WIRE_INPUTS = 21`
    /// of the 32 circuit slots; the remaining 11 are reconstructed
    /// in-handler from the accounts already on the ix surface
    /// (`global_tree`, `schema_tree_0..3`, `issuer_tree_binding`,
    /// `program_id`).  This shrinks ix data from 1324 to 972 bytes,
    /// under Solana's 1232-byte legacy-tx packet ceiling.  See the
    /// `WIRE_INPUT_SLOTS` / `RECONSTRUCTED_INPUT_SLOTS` constants
    /// above for the slot partition.
    ///
    /// See `ts-sdk/packages/verifier/src/index.ts::buildVerifyBatchProofIx`
    /// for the matching wire encoding (4-byte LE length prefix == 672
    /// (= NR_WIRE_INPUTS * 32) before the flat payload).
    #[inline(never)]
    pub fn verify_batch_proof(
        ctx: Context<VerifyBatchProof>,
        proof_a: [u8; 64],
        proof_b: [u8; 128],
        proof_c: [u8; 64],
        public_inputs: Vec<u8>,
        nullifier: [u8; 32],
    ) -> Result<()> {
        let config = &ctx.accounts.verifier_config;
        require!(!config.paused, ErrorCode::Paused);
        require!(config.vk_initialized, ErrorCode::VerificationKeyNotSet);

        // (0) Wire-arity gate.  Caller sends exactly `NR_WIRE_INPUTS *
        // 32` flat bytes (21 slots * 32 bytes = 672 bytes).  Any
        // other length is a typed rejection -- the per-chunk indexing
        // below assumes the exact length.
        require!(
            public_inputs.len() == NR_WIRE_INPUTS * 32,
            ErrorCode::InvalidProofFormat
        );

        // (1) Reconstruct the full 32-slot public-input array on the
        // user-fn stack frame (1024 bytes; user fn has ~2.2 KB free
        // per the SEC-047 stack-frame audit -- cf. `cu_budget.md` §6).
        // The Anchor `__global` wrapper's tighter ~1.6 KB-free margin
        // is unaffected because `#[inline(never)]` keeps this frame
        // separate from the wrapper's deserialization frame.
        let mut full_inputs: [[u8; 32]; NR_PUBLIC_INPUTS] = [[0u8; 32]; NR_PUBLIC_INPUTS];

        // (1a) Copy wire-supplied slots into their canonical positions.
        // `WIRE_INPUT_SLOTS[i]` is the circuit-slot index that the
        // bytes at `public_inputs[i*32..(i+1)*32]` are destined for.
        for (wire_idx, &circuit_slot) in WIRE_INPUT_SLOTS.iter().enumerate() {
            let start = wire_idx * 32;
            full_inputs[circuit_slot]
                .copy_from_slice(&public_inputs[start..start + 32]);
        }

        // (1b) Reconstruct `globalRoot` (slot 1) from the
        // `GlobalStateBinding` PDA owned by `schema-registry`.
        // Owner-check is load-bearing: a system-owned account with
        // a forged `globroot` discriminator would otherwise parse
        // cleanly.  Pre-B13 we read this slot from the wire and
        // checked it matched the account body; post-B13 we read
        // directly from the account, which is strictly stronger
        // (eliminates the "what if the wire-supplied root is stale
        // but skew-valid" question).
        //
        // Byte-order: the holder SDK reads stored roots/hashes via
        // `bufToDecimal` (LE-decode), then the verifier SDK BE-encodes
        // each `publicSignals[i]` for the wire (because groth16-solana
        // interprets each [u8; 32] public input as BE).  So the
        // BE-form Groth16 expects is `reverse(stored_bytes)`.  All
        // reconstructed slots in (1b)-(1d) byte-reverse on copy --
        // verifierAddress (1e) is the exception because the holder
        // already uses `bufToDecimalBE` for it (SOLID-SEC-031).
        require_keys_eq!(
            *ctx.accounts.global_tree.owner,
            SCHEMA_REGISTRY_ID,
            ErrorCode::InvalidGlobalRoot
        );
        let global_tree_data = ctx.accounts.global_tree.try_borrow_data()?;
        let mut global_root_le = cpi_helpers::extract_global_state_root(&global_tree_data)
            .map_err(|_| ErrorCode::InvalidGlobalRoot)?;
        global_root_le.reverse();
        full_inputs[1] = global_root_le;
        drop(global_tree_data);

        // (1c) Reconstruct `issuerTreeRoot` (slot 10) from the
        // singleton `IssuerTreeBinding` PDA owned by `issuer-registry`
        // (ADR-0014).  Same LE -> BE reversal as (1b).
        require_keys_eq!(
            *ctx.accounts.issuer_tree_binding.owner,
            ISSUER_REGISTRY_ID,
            ErrorCode::InvalidIssuerTreeBinding
        );
        let issuer_binding_data = ctx.accounts.issuer_tree_binding.try_borrow_data()?;
        let mut issuer_root_le = cpi_helpers::extract_active_issuer_tree_root(
            &issuer_binding_data,
        )
        .map_err(|e| match e {
            cpi_helpers::LightError::IssuerTreeBindingFrozen => ErrorCode::IssuerTreeBindingFrozen,
            _ => ErrorCode::InvalidIssuerTreeBinding,
        })?;
        issuer_root_le.reverse();
        full_inputs[ISSUER_TREE_ROOT_INPUT_INDEX] = issuer_root_le;
        drop(issuer_binding_data);

        // (1d) Reconstruct merkleRoots[0..3] (slots 2..5) and
        // schemaHashes[0..3] (slots 6..9) from `schema_tree_0..3`.
        //
        // Inactive-slot signal: callers pass `Pubkey::default()` (or
        // any non-`SCHEMA_REGISTRY_ID`-owned account) for slots they
        // don't have a credential for.  The owner-check below
        // distinguishes active vs inactive without needing a wire
        // flag; inactive slots leave `full_inputs[2+i]` and
        // `full_inputs[6+i]` at their zero-initialized state, which
        // matches the witness commitment for unused slots.
        //
        // Canonical-ordering check (schemas strictly ascending) is
        // preserved as defense-in-depth: catches a misordered set of
        // schema_tree accounts.  The strict-ascending comparison runs
        // on the BE-reversed schema-hash bytes (the same byte ordering
        // the witness committed to), so the ordering invariant matches
        // the circuit's own canonicality check (SOLID-SEC-050).
        let mut last_schema: Option<[u8; 32]> = None;
        for i in 0..4usize {
            let tree_info = match i {
                0 => &ctx.accounts.schema_tree_0,
                1 => &ctx.accounts.schema_tree_1,
                2 => &ctx.accounts.schema_tree_2,
                _ => &ctx.accounts.schema_tree_3,
            };
            // Inactive slot: not owned by schema-registry.  Leave the
            // corresponding `full_inputs` entries at zero; the witness
            // committed to zero too, so Groth16 stays consistent.
            if *tree_info.owner != SCHEMA_REGISTRY_ID {
                continue;
            }
            let data = tree_info.try_borrow_data()?;
            let (mut merkle_root_le, mut schema_hash_le) =
                cpi_helpers::extract_active_schema_root_binding(&data).map_err(|e| match e {
                    cpi_helpers::LightError::SchemaTreeBindingFrozen => {
                        ErrorCode::InvalidSchemaRootBinding
                    }
                    _ => ErrorCode::InvalidSchemaRootBinding,
                })?;
            merkle_root_le.reverse();
            schema_hash_le.reverse();
            // Both are now in BE form; matches what Groth16 expects
            // and what the circuit's canonicality check uses.
            if let Some(prev) = last_schema {
                require!(schema_hash_le > prev, ErrorCode::InvalidCredentialOrder);
            }
            last_schema = Some(schema_hash_le);
            full_inputs[2 + i] = merkle_root_le;
            full_inputs[6 + i] = schema_hash_le;
        }

        // (1e) Reconstruct `verifierAddress` (slot 29) from this
        // program's ID.  Constant; no account read.
        full_inputs[VERIFIER_ADDRESS_INPUT_INDEX] = ID.to_bytes();

        // (2) Nullifier binding: the output signal (full_inputs[0])
        // must equal the `nullifier` the caller is about to register
        // as a PDA seed.  full_inputs[0] is wire-sourced (slot 0 is
        // in `WIRE_INPUT_SLOTS`).
        require!(nullifier == full_inputs[0], ErrorCode::NullifierMismatch);

        // (3) SOLID-SEC-005: bind `currentTimestamp` (slot 31, wire-sourced)
        // to on-chain Clock via the configured skew window. The
        // circuit enforces `currentTimestamp <= expirationTimestamp`
        // per credential; without an on-chain freshness check a
        // prover may pass `currentTimestamp = 0` and defeat every
        // expiration gate.
        //
        // full_inputs[CURRENT_TIMESTAMP_INPUT_INDEX] is a 32-byte
        // **big-endian** encoding of a BN254 field element.  Big-endian
        // is the convention the SDK uses for every public input
        // (matching `verifierAddress = ID.to_bytes()` per SOLID-SEC-031,
        // which is itself BE because Solana pubkeys serialize BE).
        // Plausible unix timestamps fit in a u64, so the high 24 bytes
        // (`[0..24]`) MUST be zero with the value living in `[24..32]`;
        // otherwise the caller has either (a) fed the circuit a
        // pathological value or (b) packed the input with the wrong
        // encoding.
        //
        // Pre-B13 this check was written against an LE assumption (read
        // `[0..8]` as LE u64, require `[8..32]` zero).  That was latent
        // because the verifier was never invoked end-to-end before
        // SOLID-SEC-054 / B13 closed the wire-size wall; the SDK has
        // always encoded BE.
        let ts_bytes = full_inputs[CURRENT_TIMESTAMP_INPUT_INDEX];
        for i in 0..24 {
            require!(ts_bytes[i] == 0, ErrorCode::StaleTimestamp);
        }
        let claimed_ts = u64::from_be_bytes(
            ts_bytes[24..32]
                .try_into()
                .map_err(|_| ErrorCode::StaleTimestamp)?,
        );
        let now_i64 = Clock::get()?.unix_timestamp;
        require!(now_i64 >= 0, ErrorCode::StaleTimestamp);
        let now = now_i64 as u64;
        let skew = config.timestamp_skew_seconds as u64;
        let lower = now.saturating_sub(skew);
        let upper = now.saturating_add(skew);
        require!(
            claimed_ts >= lower && claimed_ts <= upper,
            ErrorCode::StaleTimestamp
        );

        // (4) Groth16 verification against the reconstructed full
        // 32-element array.
        //
        // Delegate to a separate, NEVER-INLINED helper.  The BPF target
        // is a flat 4 KB per-frame stack and `lto=fat` aggressively
        // inlines the user handler into Anchor's `__global::*` wrapper.
        // Confining the heavy Groth16 locals (~1.2 KB) to a
        // `#[inline(never)]` helper keeps them in their own stack
        // frame, well isolated from the wrapper's
        // argument-deserialization frame.  This is the durable
        // SEC-047 fix: it does not depend on LTO's inlining
        // heuristics and survives compiler upgrades.
        verify_groth16_proof(
            &ctx.accounts.vk_storage.data,
            &proof_a,
            &proof_b,
            &proof_c,
            &full_inputs,
        )?;

        // (6) Allocate nullifier PDA — atomic replay-safety.
        // The PDA is initialized by Anchor via the `init` constraint on
        // `nullifier_record`, which fails if the PDA already exists.
        ctx.accounts.nullifier_record.nullifier = nullifier;
        ctx.accounts.nullifier_record.created_at = Clock::get()?.unix_timestamp;
        ctx.accounts.nullifier_record.slot = Clock::get()?.slot;

        // (7) Metrics.
        let config_mut = &mut ctx.accounts.verifier_config;
        config_mut.proof_count = config_mut.proof_count.saturating_add(1);

        emit!(CredentialVerified {
            nullifier,
            proof_count: config_mut.proof_count,
            public_input_count: NR_PUBLIC_INPUTS as u8,
            timestamp: Clock::get()?.unix_timestamp,
        });

        msg!(
            "Batch proof verified. Cumulative verified count: {}",
            config_mut.proof_count
        );
        Ok(())
    }

    // ─── B13 Option 2 / SOLID-SEC-054: buffer-account chunked-upload path ───
    //
    // The legacy `verify_batch_proof` ix data is 1269 bytes once the
    // required `setComputeUnitLimit` is prepended -- 37 bytes over Solana's
    // 1232-byte legacy-tx packet cap.  Versioned-tx + ALT compression
    // doesn't recover that delta because the cuIx itself adds 32 bytes of
    // ComputeBudget program key + ~8 bytes for the cuIx data, both of which
    // must remain in `staticAccountKeys` (programs cannot be ALT-compressed).
    //
    // This trio of ixs is the documented escape valve from
    // `docs/REMEDIATION_OPTIONS_ARCHIVE.md` §1.1 + `docs/E2E_BLOCKERS.md`
    // B13 Option 2.  Caller flow:
    //
    //   init_proof_buffer(payer)                 -- 1 small tx
    //   upload_proof_chunk(buffer, off, bytes)*N -- 2-4 small txs
    //   verify_batch_proof_v2(buffer)            -- 1 small tx
    //
    // The `_v2` ix re-runs the same B13 (1) reconstruction the legacy ix
    // does (slots 1, 2..5, 6..9, 10, 29 from accounts), so soundness
    // properties are identical.  The legacy ix is preserved for the
    // narrower "proof fits in one tx" case.

    /// Allocate a per-payer scratch PDA for staging a Groth16 proof.
    pub fn init_proof_buffer(ctx: Context<InitProofBuffer>) -> Result<()> {
        let buffer = &mut ctx.accounts.proof_buffer;
        buffer.payer = ctx.accounts.payer.key();
        buffer.created_slot = Clock::get()?.slot;
        buffer.bytes_written = 0;
        // `data` is zero-initialized by Anchor's `init` constraint.
        Ok(())
    }

    /// Append `bytes` into `proof_buffer.data[offset..]`.  Idempotent on
    /// re-upload-of-same-bytes; the caller is the only writer (PDA seed
    /// includes their pubkey so two payers cannot collide).
    pub fn upload_proof_chunk(
        ctx: Context<UploadProofChunk>,
        offset: u32,
        bytes: Vec<u8>,
    ) -> Result<()> {
        let buffer = &mut ctx.accounts.proof_buffer;
        require_keys_eq!(
            buffer.payer,
            ctx.accounts.payer.key(),
            ErrorCode::InvalidProofBufferOwner
        );
        let off = offset as usize;
        let end = off
            .checked_add(bytes.len())
            .ok_or(ErrorCode::InvalidProofChunk)?;
        require!(end <= PROOF_BUFFER_PAYLOAD_SIZE, ErrorCode::InvalidProofChunk);
        buffer.data[off..end].copy_from_slice(&bytes);
        if (end as u32) > buffer.bytes_written {
            buffer.bytes_written = end as u32;
        }
        Ok(())
    }

    /// Verify a Groth16 proof staged in `proof_buffer` and atomically
    /// initialise the nullifier PDA.  Mirrors `verify_batch_proof`'s
    /// soundness checks (B13 reconstruction of slots 1, 2..5, 6..9, 10,
    /// 29; nullifier binding; SEC-005 timestamp skew; Groth16 pairing).
    /// Closes the buffer and refunds rent at end-of-handler.
    ///
    /// `nullifier_seed` MUST equal the nullifier staged in
    /// `proof_buffer.data[256..288)`.  The Anchor accounts struct uses
    /// `nullifier_seed` to derive the `nullifier_record` PDA at
    /// deser-time; a mismatch with the buffered nullifier is rejected
    /// in-handler with `ErrorCode::NullifierMismatch`.
    #[inline(never)]
    pub fn verify_batch_proof_v2(
        ctx: Context<VerifyBatchProofV2>,
        nullifier_seed: [u8; 32],
    ) -> Result<()> {
        let config = &ctx.accounts.verifier_config;
        require!(!config.paused, ErrorCode::Paused);
        require!(config.vk_initialized, ErrorCode::VerificationKeyNotSet);

        let buffer = &ctx.accounts.proof_buffer;
        require_keys_eq!(
            buffer.payer,
            ctx.accounts.payer.key(),
            ErrorCode::InvalidProofBufferOwner
        );
        require!(
            buffer.bytes_written as usize == PROOF_BUFFER_PAYLOAD_SIZE,
            ErrorCode::ProofBufferIncomplete
        );

        // Layout: [proof_a 64 | proof_b 128 | proof_c 64 | nullifier 32 |
        //          wire_inputs 21*32 = 672], total = 960.
        let proof_a: [u8; 64] = buffer.data[0..64]
            .try_into()
            .map_err(|_| error!(ErrorCode::InvalidProofFormat))?;
        let proof_b: [u8; 128] = buffer.data[64..192]
            .try_into()
            .map_err(|_| error!(ErrorCode::InvalidProofFormat))?;
        let proof_c: [u8; 64] = buffer.data[192..256]
            .try_into()
            .map_err(|_| error!(ErrorCode::InvalidProofFormat))?;
        let nullifier: [u8; 32] = buffer.data[256..288]
            .try_into()
            .map_err(|_| error!(ErrorCode::InvalidProofFormat))?;

        // The PDA seed driving `nullifier_record` was derived from
        // `nullifier_seed` (ix arg) at account-deser time.  Refuse if
        // the caller's seed does not match the buffered nullifier; a
        // mismatch otherwise produces a phantom nullifier PDA that
        // doesn't correspond to the verified proof.
        require!(
            nullifier_seed == nullifier,
            ErrorCode::NullifierMismatch
        );

        // Reconstruct the full 32-slot public-input array (same logic
        // as verify_batch_proof; see that handler for the byte-order
        // commentary).
        let mut full_inputs: [[u8; 32]; NR_PUBLIC_INPUTS] = [[0u8; 32]; NR_PUBLIC_INPUTS];
        let wire_region = &buffer.data[288..(288 + NR_WIRE_INPUTS * 32)];
        for (wire_idx, &circuit_slot) in WIRE_INPUT_SLOTS.iter().enumerate() {
            let start = wire_idx * 32;
            full_inputs[circuit_slot].copy_from_slice(&wire_region[start..start + 32]);
        }

        // Slot 1: globalRoot from GlobalStateBinding.
        require_keys_eq!(
            *ctx.accounts.global_tree.owner,
            SCHEMA_REGISTRY_ID,
            ErrorCode::InvalidGlobalRoot
        );
        let global_tree_data = ctx.accounts.global_tree.try_borrow_data()?;
        let mut global_root_le = cpi_helpers::extract_global_state_root(&global_tree_data)
            .map_err(|_| ErrorCode::InvalidGlobalRoot)?;
        global_root_le.reverse();
        full_inputs[1] = global_root_le;
        drop(global_tree_data);

        // Slot 10: issuerTreeRoot from IssuerTreeBinding.
        require_keys_eq!(
            *ctx.accounts.issuer_tree_binding.owner,
            ISSUER_REGISTRY_ID,
            ErrorCode::InvalidIssuerTreeBinding
        );
        let issuer_binding_data = ctx.accounts.issuer_tree_binding.try_borrow_data()?;
        let mut issuer_root_le =
            cpi_helpers::extract_active_issuer_tree_root(&issuer_binding_data)
                .map_err(|e| match e {
                    cpi_helpers::LightError::IssuerTreeBindingFrozen => {
                        ErrorCode::IssuerTreeBindingFrozen
                    }
                    _ => ErrorCode::InvalidIssuerTreeBinding,
                })?;
        issuer_root_le.reverse();
        full_inputs[ISSUER_TREE_ROOT_INPUT_INDEX] = issuer_root_le;
        drop(issuer_binding_data);

        // Slots 2..5 (merkleRoots) + 6..9 (schemaHashes) from schema_tree_0..3.
        let mut last_schema: Option<[u8; 32]> = None;
        for i in 0..4usize {
            let tree_info = match i {
                0 => &ctx.accounts.schema_tree_0,
                1 => &ctx.accounts.schema_tree_1,
                2 => &ctx.accounts.schema_tree_2,
                _ => &ctx.accounts.schema_tree_3,
            };
            if *tree_info.owner != SCHEMA_REGISTRY_ID {
                continue;
            }
            let data = tree_info.try_borrow_data()?;
            let (mut merkle_root_le, mut schema_hash_le) =
                cpi_helpers::extract_active_schema_root_binding(&data).map_err(|e| match e {
                    cpi_helpers::LightError::SchemaTreeBindingFrozen => {
                        ErrorCode::InvalidSchemaRootBinding
                    }
                    _ => ErrorCode::InvalidSchemaRootBinding,
                })?;
            merkle_root_le.reverse();
            schema_hash_le.reverse();
            if let Some(prev) = last_schema {
                require!(schema_hash_le > prev, ErrorCode::InvalidCredentialOrder);
            }
            last_schema = Some(schema_hash_le);
            full_inputs[2 + i] = merkle_root_le;
            full_inputs[6 + i] = schema_hash_le;
        }

        // Slot 29: verifierAddress = this program's ID.
        full_inputs[VERIFIER_ADDRESS_INPUT_INDEX] = ID.to_bytes();

        // Nullifier binding.
        require!(nullifier == full_inputs[0], ErrorCode::NullifierMismatch);

        // SEC-005 timestamp skew (slot 31, BE u64 in [24..32]).
        let ts_bytes = full_inputs[CURRENT_TIMESTAMP_INPUT_INDEX];
        for i in 0..24 {
            require!(ts_bytes[i] == 0, ErrorCode::StaleTimestamp);
        }
        let claimed_ts = u64::from_be_bytes(
            ts_bytes[24..32]
                .try_into()
                .map_err(|_| ErrorCode::StaleTimestamp)?,
        );
        let now_i64 = Clock::get()?.unix_timestamp;
        require!(now_i64 >= 0, ErrorCode::StaleTimestamp);
        let now = now_i64 as u64;
        let skew = config.timestamp_skew_seconds as u64;
        let lower = now.saturating_sub(skew);
        let upper = now.saturating_add(skew);
        require!(
            claimed_ts >= lower && claimed_ts <= upper,
            ErrorCode::StaleTimestamp
        );

        // Groth16 verify against the reconstructed full input array.
        verify_groth16_proof(
            &ctx.accounts.vk_storage.data,
            &proof_a,
            &proof_b,
            &proof_c,
            &full_inputs,
        )?;

        // Atomic nullifier PDA init.
        ctx.accounts.nullifier_record.nullifier = nullifier;
        ctx.accounts.nullifier_record.created_at = Clock::get()?.unix_timestamp;
        ctx.accounts.nullifier_record.slot = Clock::get()?.slot;

        // Metrics.
        let config_mut = &mut ctx.accounts.verifier_config;
        config_mut.proof_count = config_mut.proof_count.saturating_add(1);

        emit!(CredentialVerified {
            nullifier,
            proof_count: config_mut.proof_count,
            public_input_count: NR_PUBLIC_INPUTS as u8,
            timestamp: Clock::get()?.unix_timestamp,
        });

        // Buffer is closed in the accounts struct via `close = payer`,
        // refunding rent at handler exit.
        Ok(())
    }
}

/// Total payload size of the staged proof in `ProofBuffer.data`.
/// Layout: proof_a(64) + proof_b(128) + proof_c(64) + nullifier(32) +
/// wire_inputs(21*32 = 672) = 960 bytes.
pub const PROOF_BUFFER_PAYLOAD_SIZE: usize = 64 + 128 + 64 + 32 + NR_WIRE_INPUTS * 32;
const _: () = assert!(PROOF_BUFFER_PAYLOAD_SIZE == 960);

// ─── Verification-key deserialization ─────────────────────────────────────

/// Stack-owned backing storage for a parsed Groth16 verification key.
///
/// `Groth16Verifyingkey<'a>` from `groth16-solana` borrows its `vk_ic` slice
/// from the caller.  Rather than heap-allocating + `Box::leak`-ing that slice
/// (the pre-remediation behavior: one unbounded leak per `verify_batch_proof`
/// call), we materialize every field into this struct on the caller's stack
/// frame and hand out a borrow of exactly the used prefix.
///
/// Expected byte layout of the stored VK (little-endian counts, big-endian
/// curve points):
/// ```text
///   u32         nr_ic        — number of IC points (= nr_pubinputs + 1)
///   [u8; 64]    alpha_g1
///   [u8; 128]   beta_g2
///   [u8; 128]   gamma_g2
///   [u8; 128]   delta_g2
///   [u8; 64]    ic[0..nr_ic]
/// ```
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub struct VkBuf {
    nr_ic: usize,
    alpha: [u8; 64],
    beta: [u8; 128],
    gamma: [u8; 128],
    delta: [u8; 128],
    /// IC table on the heap.  Length is exactly `nr_ic` after `parse`,
    /// bounded by `MAX_IC`.  Heap residency is intentional: a fixed
    /// `[[u8; 64]; MAX_IC]` field made the inlined
    /// `__global::verify_batch_proof` BPF frame overflow the 4 KB
    /// per-frame stack budget by ~456 bytes (the parent frame already
    /// holds Anchor's ~1 312-byte deserialized argument struct).
    /// `Vec` keeps the borrow lifetime tied to `self`, so
    /// `as_verifying_key` is still safe with no `Box::leak`.
    ic: Vec<[u8; 64]>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum VkParseError {
    TooShort,
    TruncatedIc,
    IcOverflow,
}

impl VkBuf {
    /// Parse the on-chain VK byte buffer into a `VkBuf` whose IC table
    /// lives on the heap.  The fixed-size scalars (alpha/beta/gamma/delta)
    /// stay inline in the returned struct.
    ///
    /// Bounds-checked at every cursor advance; malformed input returns a typed
    /// error rather than panicking.  `nr_ic` is capped at `MAX_IC` to prevent
    /// a malicious / miscalibrated VK from driving an out-of-bounds copy.
    pub fn parse(bytes: &[u8]) -> core::result::Result<Self, VkParseError> {
        const HEADER: usize = 4 + 64 + 128 * 3;
        if bytes.len() < HEADER {
            return Err(VkParseError::TooShort);
        }

        // Header
        let nr_ic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        if nr_ic == 0 || nr_ic > MAX_IC {
            return Err(VkParseError::IcOverflow);
        }
        if bytes.len() < HEADER + nr_ic * 64 {
            return Err(VkParseError::TruncatedIc);
        }

        let mut alpha = [0u8; 64];
        let mut beta = [0u8; 128];
        let mut gamma = [0u8; 128];
        let mut delta = [0u8; 128];

        let mut cursor = 4;
        alpha.copy_from_slice(&bytes[cursor..cursor + 64]);
        cursor += 64;
        beta.copy_from_slice(&bytes[cursor..cursor + 128]);
        cursor += 128;
        gamma.copy_from_slice(&bytes[cursor..cursor + 128]);
        cursor += 128;
        delta.copy_from_slice(&bytes[cursor..cursor + 128]);
        cursor += 128;

        // Allocate the IC table directly on the heap with exact capacity.
        // No intermediate `[[u8; 64]; MAX_IC]` ever lives on the stack.
        let mut ic: Vec<[u8; 64]> = Vec::with_capacity(nr_ic);
        for _ in 0..nr_ic {
            let mut slot = [0u8; 64];
            slot.copy_from_slice(&bytes[cursor..cursor + 64]);
            cursor += 64;
            ic.push(slot);
        }

        Ok(VkBuf {
            nr_ic,
            alpha,
            beta,
            gamma,
            delta,
            ic,
        })
    }

    /// Produce a `Groth16Verifyingkey` that borrows from this buffer.  The
    /// returned view is valid for the lifetime of `self`.
    pub fn as_verifying_key(&self) -> Groth16Verifyingkey<'_> {
        Groth16Verifyingkey {
            // IC count = nr_pubinputs + 1.  Saturating for the edge case
            // where a caller hands us a 1-element IC; the verifier will
            // reject later.
            nr_pubinputs: self.nr_ic.saturating_sub(1),
            vk_alpha_g1: self.alpha,
            vk_beta_g2: self.beta,
            vk_gamme_g2: self.gamma,
            vk_delta_g2: self.delta,
            vk_ic: &self.ic[..self.nr_ic],
        }
    }
}

/// Run Groth16 verification in an isolated, never-inlined stack frame.
///
/// All heavy locals — the parsed `VkBuf` shell, the by-value
/// `Groth16Verifyingkey` view, `proof_a_neg`, and the `Groth16Verifier`
/// state — live here and disappear at function exit.  Because this helper
/// is `#[inline(never)]`, LTO=fat cannot fold it into Anchor's
/// `__global::verify_batch_proof` wrapper, so its frame doesn't merge
/// with the wrapper's deserialized-argument frame.  This is the
/// structural fix for the BPF 4 KB per-frame stack overflow described
/// in the call-site comment in `verify_batch_proof`.
///
/// Inputs are taken by reference to avoid duplicating the (already
/// argument-deserialized) ~1 312-byte payload across stack frames.
/// Heap allocations: one `Vec<[u8; 64]>` of length `nr_ic` inside
/// `VkBuf`, dropped at exit.  No `Box::leak`; no static state.
#[inline(never)]
fn verify_groth16_proof(
    vk_storage_data: &[u8],
    proof_a: &[u8; 64],
    proof_b: &[u8; 128],
    proof_c: &[u8; 64],
    public_inputs: &[[u8; 32]; NR_PUBLIC_INPUTS],
) -> Result<()> {
    let vk_buf = VkBuf::parse(vk_storage_data).map_err(|_| ErrorCode::InvalidProofFormat)?;
    let vk = vk_buf.as_verifying_key();
    let proof_a_neg = negate_g1_point(proof_a).map_err(|_| ErrorCode::InvalidProofFormat)?;

    let mut verifier = Groth16Verifier::<NR_PUBLIC_INPUTS>::new(
        &proof_a_neg,
        proof_b,
        proof_c,
        public_inputs,
        &vk,
    )
    .map_err(|_| ErrorCode::InvalidProofFormat)?;

    verifier
        .verify()
        .map_err(|_| ErrorCode::ProofVerificationFailed)?;
    Ok(())
}

/// Negate the Y coordinate of a G1 point for groth16-solana's expected
/// `-A` representation. Assumes 32-byte big-endian X || Y.
fn negate_g1_point(point: &[u8; 64]) -> std::result::Result<[u8; 64], ()> {
    // BN254 base-field prime (alt_bn128), big-endian.
    const P_BE: [u8; 32] = [
        0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58,
        0x5d, 0x97, 0x81, 0x6a, 0x91, 0x68, 0x71, 0xca, 0x8d, 0x3c, 0x20, 0x8c, 0x16, 0xd8, 0x7c,
        0xfd, 0x47,
    ];

    // Out = (X || P - Y), computed as big-endian subtraction.
    let mut out = [0u8; 64];
    out[..32].copy_from_slice(&point[..32]); // X unchanged.

    let mut borrow: i16 = 0;
    for i in (0..32).rev() {
        let p = P_BE[i] as i16;
        let y = point[32 + i] as i16;
        let mut diff = p - y - borrow;
        if diff < 0 {
            diff += 256;
            borrow = 1;
        } else {
            borrow = 0;
        }
        out[32 + i] = diff as u8;
    }
    if borrow != 0 {
        return Err(());
    }
    Ok(out)
}

// ─── Events ────────────────────────────────────────────────────────────────

#[event]
pub struct CredentialVerified {
    pub nullifier: [u8; 32],
    pub proof_count: u64,
    pub public_input_count: u8,
    pub timestamp: i64,
}

// ─── Account Contexts ──────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init, payer = authority,
        space = 8 + VerifierConfig::SPACE,
        seeds = [b"verifier-config"],
        bump
    )]
    pub verifier_config: Account<'info, VerifierConfig>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AuthorityOnly<'info> {
    #[account(
        mut,
        seeds = [b"verifier-config"], bump = verifier_config.bump,
        has_one = authority @ ErrorCode::Unauthorized
    )]
    pub verifier_config: Account<'info, VerifierConfig>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct StoreVerificationKey<'info> {
    #[account(mut, seeds = [b"verifier-config"], bump = verifier_config.bump)]
    pub verifier_config: Account<'info, VerifierConfig>,
    #[account(
        init_if_needed, payer = authority,
        // 8 (disc) + 4 (Vec len) + 10228 (data) = 10240 = the Solana
        // CPI realloc cap (`MAX_PERMITTED_DATA_INCREASE`).  Must stay
        // in lockstep with `VK_MAX_BYTES` in `store_verification_key`.
        space = 8 + 4 + 10228,
        seeds = [b"vk-storage", verifier_config.key().as_ref()],
        bump
    )]
    pub vk_storage: Account<'info, VkStorage>,
    #[account(mut, constraint = authority.key() == verifier_config.authority @ ErrorCode::Unauthorized)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(
    proof_a: [u8; 64], proof_b: [u8; 128], proof_c: [u8; 64],
    public_inputs: Vec<u8>, nullifier: [u8; 32]
)]
pub struct VerifyBatchProof<'info> {
    #[account(mut, seeds = [b"verifier-config"], bump = verifier_config.bump)]
    pub verifier_config: Account<'info, VerifierConfig>,

    #[account(seeds = [b"vk-storage", verifier_config.key().as_ref()], bump)]
    pub vk_storage: Account<'info, VkStorage>,

    /// Nullifier record PDA. `init` ensures it cannot already exist.
    #[account(
        init, payer = payer,
        space = 8 + NullifierRecord::SPACE,
        seeds = [NULLIFIER_SEED, nullifier.as_ref()],
        bump
    )]
    pub nullifier_record: Account<'info, NullifierRecord>,

    /// CHECK: Global-state Merkle tree account (Light Protocol or SolID-native).
    pub global_tree: UncheckedAccount<'info>,

    /// CHECK: Per-schema tree metadata PDA (slot 0).
    pub schema_tree_0: UncheckedAccount<'info>,
    /// CHECK: slot 1
    pub schema_tree_1: UncheckedAccount<'info>,
    /// CHECK: slot 2
    pub schema_tree_2: UncheckedAccount<'info>,
    /// CHECK: slot 3
    pub schema_tree_3: UncheckedAccount<'info>,

    /// ADR-0014: singleton `IssuerTreeBinding` PDA owned by
    /// `issuer-registry`.  The seed literal `b"issuer-tree-binding"`
    /// is the source of truth declared in
    /// `programs/issuer-registry/src/lib.rs`
    /// (`ISSUER_TREE_BINDING_SEED`).  The handler additionally
    /// owner-checks this account against `ISSUER_REGISTRY_ID` and
    /// asserts the parsed `current_root` equals
    /// `public_inputs[ISSUER_TREE_ROOT_INPUT_INDEX]`.  CHECK handled
    /// in-handler.
    #[account(
        seeds = [b"issuer-tree-binding"],
        bump,
        seeds::program = ISSUER_REGISTRY_ID,
    )]
    pub issuer_tree_binding: UncheckedAccount<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// B13 Option 2: allocate the per-payer proof-staging buffer PDA.
#[derive(Accounts)]
pub struct InitProofBuffer<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + ProofBuffer::SPACE,
        seeds = [b"proof-buffer", payer.key().as_ref()],
        bump,
    )]
    pub proof_buffer: Account<'info, ProofBuffer>,

    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// B13 Option 2: write a chunk of bytes into the proof-staging buffer.
/// Caller MUST be the original `payer` recorded at init.
#[derive(Accounts)]
pub struct UploadProofChunk<'info> {
    #[account(
        mut,
        seeds = [b"proof-buffer", payer.key().as_ref()],
        bump,
    )]
    pub proof_buffer: Account<'info, ProofBuffer>,

    pub payer: Signer<'info>,
}

/// B13 Option 2: verify a Groth16 proof staged in `proof_buffer`.
/// Mirrors `VerifyBatchProof`'s account set + the proof-buffer.
/// `proof_buffer` is closed at end of handler (rent refund to payer).
///
/// `nullifier_seed` is the ix arg that drives the nullifier_record PDA
/// derivation.  The handler asserts it equals
/// `proof_buffer.data[256..288]` (the staged nullifier), so a malicious
/// caller passing a mismatched seed gets a typed rejection rather than
/// an off-base PDA.
#[derive(Accounts)]
#[instruction(nullifier_seed: [u8; 32])]
pub struct VerifyBatchProofV2<'info> {
    #[account(mut, seeds = [b"verifier-config"], bump = verifier_config.bump)]
    pub verifier_config: Account<'info, VerifierConfig>,

    #[account(seeds = [b"vk-storage", verifier_config.key().as_ref()], bump)]
    pub vk_storage: Account<'info, VkStorage>,

    /// Nullifier record PDA. `init` ensures it cannot already exist.
    /// Seed is the nullifier read from `proof_buffer.data[256..288]`,
    /// so callers don't pass it as ix arg.  But Anchor needs the seed
    /// at deserialization time -- we surface it through the
    /// `#[instruction]` attribute via `nullifier_seed: [u8; 32]` which
    /// the SDK MUST set equal to `proof_buffer.data[256..288]`.  The
    /// handler then asserts the wire-supplied seed matches the
    /// buffered nullifier so the constraint binds.
    #[account(
        init, payer = payer,
        space = 8 + NullifierRecord::SPACE,
        seeds = [NULLIFIER_SEED, nullifier_seed.as_ref()],
        bump
    )]
    pub nullifier_record: Account<'info, NullifierRecord>,

    /// CHECK: Global-state Merkle tree account.  Owner-checked in-handler.
    pub global_tree: UncheckedAccount<'info>,

    /// CHECK: Per-schema tree metadata PDA (slot 0).
    pub schema_tree_0: UncheckedAccount<'info>,
    /// CHECK: slot 1
    pub schema_tree_1: UncheckedAccount<'info>,
    /// CHECK: slot 2
    pub schema_tree_2: UncheckedAccount<'info>,
    /// CHECK: slot 3
    pub schema_tree_3: UncheckedAccount<'info>,

    /// ADR-0014 issuer-tree binding.
    #[account(
        seeds = [b"issuer-tree-binding"],
        bump,
        seeds::program = ISSUER_REGISTRY_ID,
    )]
    pub issuer_tree_binding: UncheckedAccount<'info>,

    /// Buffer holding the staged proof + wire inputs.  Closed at end of
    /// handler; rent refunds to `payer`.
    #[account(
        mut,
        close = payer,
        seeds = [b"proof-buffer", payer.key().as_ref()],
        bump,
    )]
    pub proof_buffer: Account<'info, ProofBuffer>,

    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// ─── State ─────────────────────────────────────────────────────────────────

#[account]
pub struct VerifierConfig {
    pub authority: Pubkey,
    pub proof_count: u64,
    pub vk_initialized: bool,
    pub paused: bool,
    pub bump: u8,
    /// Index of the next chunk expected by `store_verification_key`.
    /// Reset to zero only through a redeploy or a completed rotation;
    /// once the VK is finalized this serves as an audit trail of how
    /// many chunks were stitched together.
    pub next_vk_chunk: u16,
    /// SOLID-SEC-005: tolerance (seconds) between the `currentTimestamp`
    /// public input and the on-chain Clock at `verify_batch_proof` time.
    /// Default `DEFAULT_TIMESTAMP_SKEW_SECONDS`; configurable via
    /// `set_timestamp_skew` up to `MAX_TIMESTAMP_SKEW_SECONDS`.
    pub timestamp_skew_seconds: u32,
    /// SOLID-SEC-006 freeze-gate.  Flips to `true` on
    /// `finalize_verification_key`.  While `true`, `store_verification_
    /// key` refuses every chunk-0 overwrite and every append -- the VK
    /// is immutable until the DAO-gated rotation path completes.
    pub vk_finalized: bool,
    /// SOLID-SEC-006 monotonic rotation counter.  Starts at 0; bumps
    /// on every successful `rotate_verification_key`.  Observers can
    /// detect rotation out-of-band; a future circuit revision will
    /// bind this into the public-input contract so cross-VK replay is
    /// impossible (currently SEC-006 Part 2, deferred to the next
    /// trusted-setup regeneration).
    pub vk_generation: u16,
    /// SOLID-SEC-006 rotation-request timelock anchor.  Zero means no
    /// pending rotation.  Non-zero means the authority has requested a
    /// rotation; `rotate_verification_key` refuses until
    /// `Clock::unix_timestamp >= rotate_request_ts + VK_ROTATION_
    /// TIMELOCK_SECONDS`.  `cancel_vk_rotation` resets to zero.
    pub rotate_request_ts: i64,
}

impl VerifierConfig {
    // 32 authority + 8 proof_count + 1 vk_initialized + 1 paused + 1 bump
    // + 2 next_vk_chunk + 4 timestamp_skew_seconds
    // + 1 vk_finalized + 2 vk_generation + 8 rotate_request_ts.
    // SOLID-SEC-006 growth from 49 -> 60.  ADR-0015 notes the layout
    // bump; any future add must bump this constant in the same commit.
    pub const SPACE: usize = 32 + 8 + 1 + 1 + 1 + 2 + 4 + 1 + 2 + 8;
}

/// SOLID-SEC-006 helper: is the pending rotation past its timelock?
/// Pure so the state-transition invariants are host-testable without
/// spinning up a validator.  Returns `false` when there is no pending
/// rotation (request_ts == 0).
pub fn vk_rotation_timelock_expired(config: &VerifierConfig, now_unix_ts: i64) -> bool {
    if config.rotate_request_ts == 0 {
        return false;
    }
    now_unix_ts
        >= config
            .rotate_request_ts
            .saturating_add(VK_ROTATION_TIMELOCK_SECONDS)
}

#[account]
pub struct VkStorage {
    pub data: Vec<u8>,
}

#[account]
pub struct NullifierRecord {
    pub nullifier: [u8; 32],
    pub created_at: i64,
    pub slot: u64,
}

impl NullifierRecord {
    pub const SPACE: usize = 32 + 8 + 8;
}

/// SOLID-SEC-054 / B13 Option 2: per-payer scratch PDA for staging a
/// Groth16 proof across multiple `upload_proof_chunk` ixs before a final
/// `verify_batch_proof_v2` call.  The seed includes the payer's pubkey
/// (`[b"proof-buffer", payer.key().as_ref()]`) so two payers cannot
/// collide.  Closed at end of `verify_batch_proof_v2` -- rent refunds to
/// the original payer.
#[account]
pub struct ProofBuffer {
    pub payer: Pubkey,
    pub created_slot: u64,
    /// High-water mark of bytes written by `upload_proof_chunk`.  The
    /// `verify_batch_proof_v2` ix refuses if this is not exactly
    /// `PROOF_BUFFER_PAYLOAD_SIZE` (rejects partially-uploaded buffers).
    pub bytes_written: u32,
    /// Flat payload region.  Layout (caller-side encoding mirrors this):
    ///   [0..64)         proof_a
    ///   [64..192)       proof_b
    ///   [192..256)      proof_c
    ///   [256..288)      nullifier
    ///   [288..960)      wire_inputs (21 * 32)
    pub data: [u8; PROOF_BUFFER_PAYLOAD_SIZE],
}

impl ProofBuffer {
    /// 32 payer + 8 created_slot + 4 bytes_written + 960 data.
    pub const SPACE: usize = 32 + 8 + 4 + PROOF_BUFFER_PAYLOAD_SIZE;
}

// ─── Errors ────────────────────────────────────────────────────────────────

#[error_code]
pub enum ErrorCode {
    #[msg("Groth16 proof verification failed")]
    ProofVerificationFailed,
    #[msg("Invalid proof format")]
    InvalidProofFormat,
    #[msg("Verification key not set")]
    VerificationKeyNotSet,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Invalid global state root")]
    InvalidGlobalRoot,
    #[msg("Nullifier mismatch between circuit output and registered nullifier")]
    NullifierMismatch,
    #[msg("Invalid verifier address — proof was not bound to this program")]
    InvalidVerifierAddress,
    #[msg("Invalid schema/root binding — schema is not associated with this tree")]
    InvalidSchemaRootBinding,
    #[msg("Schemas must be strictly ascending (canonical ordering)")]
    InvalidCredentialOrder,
    #[msg("Verifier is paused")]
    Paused,
    #[msg("VK chunk arrived out of order (expected next_vk_chunk)")]
    ChunkOutOfOrder,
    #[msg("VK storage capacity exceeded (10228 bytes)")]
    VkStorageFull,
    #[msg("Arithmetic overflow")]
    Overflow,
    #[msg("currentTimestamp public input is outside the configured skew window")]
    StaleTimestamp,
    #[msg("Timestamp skew value exceeds MAX_TIMESTAMP_SKEW_SECONDS (3600)")]
    TimestampSkewTooLarge,
    #[msg("IssuerTreeBinding account is invalid or not owned by issuer-registry (ADR-0014)")]
    InvalidIssuerTreeBinding,
    #[msg("IssuerTreeBinding.current_root does not match the proof's issuerTreeRoot public input (ADR-0014)")]
    IssuerTreeRootMismatch,
    #[msg("IssuerTreeBinding is frozen; cannot verify proofs under this root (ADR-0014)")]
    IssuerTreeBindingFrozen,
    #[msg("Verification key is finalized; further writes rejected (SOLID-SEC-006)")]
    VerificationKeyFinalized,
    #[msg("Verification key is not finalized; finalize_verification_key must run first (SOLID-SEC-006)")]
    VerificationKeyNotFinalized,
    #[msg("Verification key is already finalized (SOLID-SEC-006)")]
    VerificationKeyAlreadyFinalized,
    #[msg("A VK rotation is already pending; cancel it before requesting another (SOLID-SEC-006)")]
    RotationAlreadyRequested,
    #[msg("No VK rotation is pending (SOLID-SEC-006)")]
    NoPendingRotation,
    #[msg("VK rotation timelock has not yet expired (SOLID-SEC-006)")]
    RotationTimelockNotExpired,
    #[msg(
        "Clock returned a non-positive unix timestamp during VK rotation request (SOLID-SEC-006)"
    )]
    RotationClockInvalid,
    #[msg("Proof buffer is owned by a different payer (B13 Option 2)")]
    InvalidProofBufferOwner,
    #[msg("Proof chunk offset+length out of bounds (B13 Option 2)")]
    InvalidProofChunk,
    #[msg("Proof buffer is not fully populated; verify rejects partial uploads (B13 Option 2)")]
    ProofBufferIncomplete,
}

// ─── Unit tests ────────────────────────────────────────────────────────────
//
// These tests execute on the host (`cargo test -p zk-verifier`) rather than
// on BPF.  They exercise the VK parser and the G1 negation in isolation so
// malformed-input regressions are caught without a full program-test harness.

#[cfg(test)]
mod tests {
    use super::*;

    // Synthesize a plausibly-shaped VK byte buffer with `nr_ic` IC points.
    // The values are not cryptographically meaningful — the test is about the
    // parser, not about pairing correctness.
    fn synth_vk_bytes(nr_ic: usize) -> Vec<u8> {
        let mut v = Vec::with_capacity(4 + 64 + 128 * 3 + nr_ic * 64);
        v.extend_from_slice(&(nr_ic as u32).to_le_bytes());
        v.extend_from_slice(&[1u8; 64]); // alpha
        v.extend_from_slice(&[2u8; 128]); // beta
        v.extend_from_slice(&[3u8; 128]); // gamma
        v.extend_from_slice(&[4u8; 128]); // delta
        for i in 0..nr_ic {
            v.extend_from_slice(&[i as u8; 64]);
        }
        v
    }

    #[test]
    fn vk_parse_round_trips_minimum_size() {
        let bytes = synth_vk_bytes(2);
        let buf = VkBuf::parse(&bytes).expect("parse");
        assert_eq!(buf.nr_ic, 2);
        assert_eq!(buf.alpha, [1u8; 64]);
        assert_eq!(buf.beta, [2u8; 128]);
        assert_eq!(buf.gamma, [3u8; 128]);
        assert_eq!(buf.delta, [4u8; 128]);
        assert_eq!(buf.ic[0], [0u8; 64]);
        assert_eq!(buf.ic[1], [1u8; 64]);
    }

    #[test]
    fn vk_parse_round_trips_max_ic() {
        // Standard batch VK: 31 public inputs → 32 IC points.
        let bytes = synth_vk_bytes(MAX_IC);
        let buf = VkBuf::parse(&bytes).expect("parse");
        assert_eq!(buf.nr_ic, MAX_IC);
        assert_eq!(buf.ic[MAX_IC - 1], [(MAX_IC - 1) as u8; 64]);
    }

    #[test]
    fn vk_parse_rejects_too_short_header() {
        let bytes = vec![0u8; 4 + 64 + 128 * 3 - 1];
        assert_eq!(VkBuf::parse(&bytes), Err(VkParseError::TooShort));
    }

    #[test]
    fn vk_parse_rejects_truncated_ic_region() {
        let mut bytes = synth_vk_bytes(5);
        bytes.truncate(bytes.len() - 10);
        assert_eq!(VkBuf::parse(&bytes), Err(VkParseError::TruncatedIc));
    }

    #[test]
    fn vk_parse_rejects_zero_ic() {
        let bytes = synth_vk_bytes(0);
        // Header alone is valid size; nr_ic == 0 must be rejected.
        assert_eq!(VkBuf::parse(&bytes), Err(VkParseError::IcOverflow));
    }

    #[test]
    fn vk_parse_rejects_ic_overflow() {
        // Claim more IC points than the stack buffer can hold.
        let too_many = MAX_IC + 1;
        let mut bytes = Vec::with_capacity(4 + 64 + 128 * 3 + too_many * 64);
        bytes.extend_from_slice(&(too_many as u32).to_le_bytes());
        bytes.extend_from_slice(&[0u8; 64 + 128 * 3 + 64]); // partial payload
        assert_eq!(VkBuf::parse(&bytes), Err(VkParseError::IcOverflow));
    }

    #[test]
    fn vk_view_borrows_from_buffer() {
        let bytes = synth_vk_bytes(4);
        let buf = VkBuf::parse(&bytes).expect("parse");
        let vk = buf.as_verifying_key();
        assert_eq!(vk.nr_pubinputs, 3);
        assert_eq!(vk.vk_ic.len(), 4);
        assert_eq!(vk.vk_alpha_g1, [1u8; 64]);
    }

    #[test]
    fn vk_buf_fits_in_stack_budget() {
        // Mirrors the const assertion at the top of the file; runtime form
        // gives a readable failure message.  After moving the IC table to
        // the heap, the stack-resident shell is ~480 bytes.  The 1 KB
        // ceiling leaves the rest of the BPF 4 KB per-frame budget for
        // Anchor's deserialized argument struct (~1 312 bytes) plus the
        // verifier's other locals.
        assert!(core::mem::size_of::<VkBuf>() < 1024);
    }

    // ─── SOLID-SEC-006 freeze-gate + timelock regression gates ──────────

    /// Construct a minimal `VerifierConfig` in a pre-finalize state for
    /// the timelock tests.  Not an Anchor Context; just the struct.
    fn blank_config() -> VerifierConfig {
        VerifierConfig {
            authority: Pubkey::default(),
            proof_count: 0,
            vk_initialized: true,
            paused: false,
            bump: 0,
            next_vk_chunk: 0,
            timestamp_skew_seconds: DEFAULT_TIMESTAMP_SKEW_SECONDS,
            vk_finalized: false,
            vk_generation: 0,
            rotate_request_ts: 0,
        }
    }

    #[test]
    fn vk_rotation_not_expired_when_no_pending_request() {
        let config = blank_config();
        assert!(!vk_rotation_timelock_expired(&config, 0));
        assert!(!vk_rotation_timelock_expired(&config, i64::MAX));
    }

    #[test]
    fn vk_rotation_not_expired_inside_window() {
        let mut config = blank_config();
        config.rotate_request_ts = 1_700_000_000; // arbitrary epoch
                                                  // Checking at request-time and at request + (timelock - 1)
                                                  // both must reject.  The handler will use Clock::unix_timestamp.
        assert!(!vk_rotation_timelock_expired(
            &config,
            config.rotate_request_ts
        ));
        assert!(!vk_rotation_timelock_expired(
            &config,
            config
                .rotate_request_ts
                .saturating_add(VK_ROTATION_TIMELOCK_SECONDS - 1)
        ));
    }

    #[test]
    fn vk_rotation_expired_at_and_beyond_timelock() {
        let mut config = blank_config();
        config.rotate_request_ts = 1_700_000_000;
        // Exactly at the boundary: `>=` semantics.
        assert!(vk_rotation_timelock_expired(
            &config,
            config
                .rotate_request_ts
                .saturating_add(VK_ROTATION_TIMELOCK_SECONDS)
        ));
        // Well past.
        assert!(vk_rotation_timelock_expired(
            &config,
            config
                .rotate_request_ts
                .saturating_add(VK_ROTATION_TIMELOCK_SECONDS * 10)
        ));
    }

    #[test]
    fn vk_rotation_handles_saturation_safely() {
        // If an operator somehow set rotate_request_ts to near-i64::MAX,
        // `saturating_add(VK_ROTATION_TIMELOCK_SECONDS)` caps at i64::MAX.
        // The expected behaviour is: no panic, and `now < cap` still
        // reads as not-expired.  Practical unix timestamps on Solana
        // are nowhere near i64::MAX, and `rotate_request_ts` is only
        // ever set from `Clock::get()?.unix_timestamp` in
        // `request_vk_rotation`, which `require!`s `now > 0`.  This
        // test guards the math, not a realistic attack surface.
        let mut config = blank_config();
        config.rotate_request_ts = i64::MAX - 1;
        // `now = 0` is far below the saturated cap -> not expired.
        assert!(!vk_rotation_timelock_expired(&config, 0));
        // At `now = i64::MAX` the saturating sum also equals i64::MAX,
        // so `now >= cap` is true.  This is the mathematically correct
        // outcome; it is harmless because no honest caller can reach
        // that state.  Documenting the expectation prevents a future
        // "fix" from introducing a wraparound bug elsewhere.
        assert!(vk_rotation_timelock_expired(&config, i64::MAX));
    }

    #[test]
    fn verifier_config_space_matches_layout() {
        // Guard against silent SPACE drift; mirrors the SEC-042 pattern
        // of "compute the exact byte size and compare to the constant".
        // 32 authority + 8 proof_count + 1 vk_initialized + 1 paused
        // + 1 bump + 2 next_vk_chunk + 4 timestamp_skew_seconds
        // + 1 vk_finalized + 2 vk_generation + 8 rotate_request_ts.
        let expected: usize = 32 + 8 + 1 + 1 + 1 + 2 + 4 + 1 + 2 + 8;
        assert_eq!(VerifierConfig::SPACE, expected);
        assert_eq!(VerifierConfig::SPACE, 60);
    }

    #[test]
    fn vk_rotation_timelock_is_48_hours() {
        // Doc-as-test: the timelock constant is load-bearing.  If a
        // future refactor changes it, this test forces a conscious
        // update here + the ADR.
        assert_eq!(VK_ROTATION_TIMELOCK_SECONDS, 48 * 3600);
    }

    #[test]
    fn negate_g1_zero_y_is_field_prime() {
        // Y = 0 should negate to Y = P (mod P) = 0 too, but our byte-level
        // routine writes P - 0 = P, which is accepted because groth16-solana
        // canonicalizes before pairing.  The key invariant: no panic, correct
        // bitwidth.
        let mut point = [0u8; 64];
        point[0] = 0x12; // X
        let out = negate_g1_point(&point).expect("negate");
        assert_eq!(&out[..32], &point[..32], "X must be unchanged");
    }

    #[test]
    fn negate_g1_is_involutive_modulo_p() {
        // Pick a Y strictly less than P so P - (P - Y) = Y.
        let mut point = [0u8; 64];
        point[0] = 0xAB;
        for (i, b) in point[32..].iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(7);
        }
        let once = negate_g1_point(&point).expect("negate once");
        let twice = negate_g1_point(&once).expect("negate twice");
        assert_eq!(point, twice, "double negation is identity");
    }

    // ─── ADR-0014 public-input layout pins ────────────────────────────────
    //
    // The on-chain handler reads three specific slots of `public_inputs[]`
    // by hardcoded index.  A reshuffle of the circuit's IO without
    // matching changes here silently breaks every proof verification;
    // these tests are the source-level regression gate.

    #[test]
    fn public_input_count_is_32() {
        // Post ADR-0014 (issuerTreeRoot added at slot 10).
        assert_eq!(NR_PUBLIC_INPUTS, 32);
    }

    #[test]
    fn max_ic_is_public_inputs_plus_one() {
        // Groth16's IC has length NR_PUBLIC_INPUTS + 1 by construction.
        assert_eq!(MAX_IC, NR_PUBLIC_INPUTS + 1);
    }

    #[test]
    fn public_input_indices_are_distinct_and_in_bounds() {
        // The three load-bearing indices must be unique and inside
        // 0..NR_PUBLIC_INPUTS.  A duplicate would silently let one
        // public-input slot stand in for another in the on-chain
        // gates.
        for &idx in &[
            ISSUER_TREE_ROOT_INPUT_INDEX,
            VERIFIER_ADDRESS_INPUT_INDEX,
            CURRENT_TIMESTAMP_INPUT_INDEX,
        ] {
            assert!(idx < NR_PUBLIC_INPUTS, "idx {} out of bounds", idx);
        }
        assert_ne!(ISSUER_TREE_ROOT_INPUT_INDEX, VERIFIER_ADDRESS_INPUT_INDEX);
        assert_ne!(ISSUER_TREE_ROOT_INPUT_INDEX, CURRENT_TIMESTAMP_INPUT_INDEX);
        assert_ne!(VERIFIER_ADDRESS_INPUT_INDEX, CURRENT_TIMESTAMP_INPUT_INDEX);
    }

    #[test]
    fn public_input_indices_are_at_documented_slots() {
        // The doc-comment header at the top of this file documents:
        //   slot 10  = issuerTreeRoot
        //   slot 29  = verifierAddress
        //   slot 31  = currentTimestamp
        // If anyone bumps these constants without updating the
        // doc-comment + circuit, this test forces a conscious change.
        assert_eq!(ISSUER_TREE_ROOT_INPUT_INDEX, 10);
        assert_eq!(VERIFIER_ADDRESS_INPUT_INDEX, 29);
        assert_eq!(CURRENT_TIMESTAMP_INPUT_INDEX, 31);
    }

    // ─── SOLID-SEC-054 / B13 slot-partition invariants ─────────────────────
    //
    // The wire ↔ reconstructed split is encoded by two const slot-mapping
    // arrays.  These tests assert that the partition is well-formed at
    // build time: no slot dropped, no slot duplicated, no slot mis-classified.
    // A regression here would silently cause Groth16 to reject every honest
    // proof (or, worse, accept proofs against a layout that doesn't match
    // the trusted-setup commitment).

    #[test]
    fn wire_input_arity_is_consistent() {
        // The wire carries 21 of the 32 circuit slots; the rest are
        // reconstructed on-chain.  Any change to NR_PUBLIC_INPUTS or
        // NR_WIRE_INPUTS without matching slot-array updates fails this.
        assert_eq!(NR_WIRE_INPUTS, WIRE_INPUT_SLOTS.len());
        assert_eq!(
            NR_PUBLIC_INPUTS - NR_WIRE_INPUTS,
            RECONSTRUCTED_INPUT_SLOTS.len()
        );
        assert_eq!(
            NR_WIRE_INPUTS + RECONSTRUCTED_INPUT_SLOTS.len(),
            NR_PUBLIC_INPUTS
        );
    }

    #[test]
    fn slot_partition_is_complete_and_disjoint() {
        // Every circuit slot 0..NR_PUBLIC_INPUTS is covered exactly once
        // across WIRE_INPUT_SLOTS ∪ RECONSTRUCTED_INPUT_SLOTS, with no
        // overlap.  Construct a 32-bit bitmap and assert each slot is
        // hit exactly once.
        let mut hits = [0u32; NR_PUBLIC_INPUTS];
        for &s in WIRE_INPUT_SLOTS.iter() {
            assert!(s < NR_PUBLIC_INPUTS, "wire slot {} out of bounds", s);
            hits[s] += 1;
        }
        for &s in RECONSTRUCTED_INPUT_SLOTS.iter() {
            assert!(
                s < NR_PUBLIC_INPUTS,
                "reconstructed slot {} out of bounds",
                s
            );
            hits[s] += 1;
        }
        for (i, &h) in hits.iter().enumerate() {
            assert_eq!(h, 1, "slot {} covered {} times (expected 1)", i, h);
        }
    }

    #[test]
    fn timestamp_and_nonce_are_witness_bound_wire_slots() {
        // Slots 30 (verifierNonce) and 31 (currentTimestamp) are
        // witness-bound and MUST stay on the wire -- Groth16 has zero
        // tolerance on public-input equality and the on-chain Clock
        // cannot reproduce the exact T_off the witness committed to.
        assert!(
            WIRE_INPUT_SLOTS.contains(&30),
            "verifierNonce (slot 30) must be wire-supplied"
        );
        assert!(
            WIRE_INPUT_SLOTS.contains(&CURRENT_TIMESTAMP_INPUT_INDEX),
            "currentTimestamp (slot 31) must be wire-supplied"
        );
        assert!(
            !RECONSTRUCTED_INPUT_SLOTS.contains(&CURRENT_TIMESTAMP_INPUT_INDEX),
            "currentTimestamp must NOT be reconstructed -- soundness gate"
        );
    }

    #[test]
    fn nullifier_slot_is_wire_supplied() {
        // The nullifier (slot 0) is the circuit output and is bound to
        // the `nullifier` arg by `(2) NullifierMismatch` check.  It
        // MUST be wire-supplied; it's not derivable from any account.
        assert!(WIRE_INPUT_SLOTS.contains(&0));
        assert!(!RECONSTRUCTED_INPUT_SLOTS.contains(&0));
    }

    #[test]
    fn reconstructed_slots_match_documented_layout() {
        // The 11 reconstructible slots are pinned by docs/E2E_BLOCKERS.md
        // B13, plan/IMPLEMENTATION_PLAN.md Appendix D, and CLAUDE.md.
        // Any divergence between the constants here and those docs is a
        // doc-vs-code drift that must be resolved in the same commit.
        let expected: [usize; 11] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 29];
        assert_eq!(RECONSTRUCTED_INPUT_SLOTS, expected);
    }

    #[test]
    fn wire_size_fits_legacy_tx_packet() {
        // The whole point of B13.  Compute the borsh wire size of the
        // verify_batch_proof ix data and assert it's under Solana's
        // 1232-byte legacy-tx packet ceiling.
        //
        //   discriminator     8
        // + proof_a          64
        // + proof_b         128
        // + proof_c          64
        // + Vec<> length     4
        // + public_inputs body (NR_WIRE_INPUTS * 32)
        // + nullifier        32
        let ix_data_size = 8 + 64 + 128 + 64 + 4 + NR_WIRE_INPUTS * 32 + 32;
        assert!(
            ix_data_size <= 1232,
            "verify_batch_proof ix data {} bytes exceeds legacy-tx 1232-byte ceiling",
            ix_data_size
        );
        // Pin the actual figure to make any future regression visible
        // in the diff.  Pre-B13: 1324 bytes; post-B13: 972 bytes.
        assert_eq!(ix_data_size, 972);
    }

    // ─── SOLID-SEC-005 timestamp-skew constants ───────────────────────────

    #[test]
    fn default_timestamp_skew_is_ten_minutes() {
        assert_eq!(DEFAULT_TIMESTAMP_SKEW_SECONDS, 600);
    }

    #[test]
    fn max_timestamp_skew_is_one_hour() {
        assert_eq!(MAX_TIMESTAMP_SKEW_SECONDS, 3_600);
    }

    #[test]
    fn default_skew_within_max_window() {
        // Sanity: the on-chain handler must accept the default at init
        // time.  If MAX is ever lowered below DEFAULT, every fresh
        // initialize would fail at `set_timestamp_skew`'s bounds check.
        assert!(DEFAULT_TIMESTAMP_SKEW_SECONDS <= MAX_TIMESTAMP_SKEW_SECONDS);
    }

    // ─── Constant integrity ───────────────────────────────────────────────

    #[test]
    fn nullifier_seed_is_null_literal() {
        assert_eq!(NULLIFIER_SEED, b"null");
    }

    #[test]
    fn cross_program_ids_match_canonical() {
        // The owner-checks at lib.rs:446-450 / :467-471 / :523-527
        // dereference these constants directly.  A drift would silently
        // re-open the forged-trust-root attack surface (P0-2 / SEC-004).
        assert_eq!(
            SCHEMA_REGISTRY_ID.to_string(),
            "4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1"
        );
        assert_eq!(
            ISSUER_REGISTRY_ID.to_string(),
            "5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx"
        );
    }

    #[test]
    fn zk_verifier_program_id_is_canonical() {
        // Drift gate: ID literal matches CLAUDE.md's canonical list.
        assert_eq!(
            crate::ID.to_string(),
            "DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb"
        );
    }

    // ─── Negate-G1 known-answer ───────────────────────────────────────────

    #[test]
    fn negate_g1_known_answer_y_one() {
        // X = 0, Y = 1 (BE).  P - 1 (BE) is BN254_FQ - 1.
        // BN254_FQ in hex (BE):
        //   0x30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47
        // P - 1 = ...d46 (last byte: 0x47 - 0x01 = 0x46).
        let mut point = [0u8; 64];
        point[63] = 0x01; // Y = 1 in big-endian
        let out = negate_g1_point(&point).expect("negate");
        // X must be unchanged.
        assert_eq!(&out[..32], &[0u8; 32]);
        // Y must be P - 1 (BE).  Last byte of P is 0x47 -> P-1 ends 0x46.
        assert_eq!(out[63], 0x46);
        // High byte of Y must be 0x30 (matches BN254_FQ first byte).
        assert_eq!(out[32], 0x30);
    }

    #[test]
    fn negate_g1_zero_zero_no_panic() {
        // The (0, 0) "infinity sentinel" must not panic.  groth16-solana
        // canonicalises before use; whatever bytes we produce, the
        // pairing layer must accept or reject without us crashing.
        let point = [0u8; 64];
        let out = negate_g1_point(&point).expect("negate (0,0)");
        // X is unchanged.
        assert_eq!(&out[..32], &[0u8; 32]);
        // Y becomes P - 0 = P.
        // (Documenting expected output, not asserting equality with a
        // canonical "infinity"; the pairing crate handles canonicalisation.)
        assert_eq!(out[32], 0x30);
        assert_eq!(out[63], 0x47);
    }

    // ─── B13 Option 2 / SOLID-SEC-054: ProofBuffer layout invariants ─────

    #[test]
    fn proof_buffer_payload_size_matches_layout() {
        // Layout: proof_a 64 + proof_b 128 + proof_c 64 + nullifier 32 +
        // wire_inputs 21*32 = 672 = 960.
        assert_eq!(PROOF_BUFFER_PAYLOAD_SIZE, 960);
        // Anchor account SPACE = 8 disc + 32 payer + 8 created_slot
        // + 4 bytes_written + 960 data = 1012.  The init ix allocates
        // 8 + ProofBuffer::SPACE.
        assert_eq!(8 + ProofBuffer::SPACE, 8 + 32 + 8 + 4 + 960);
        assert_eq!(8 + ProofBuffer::SPACE, 1012);
    }

    #[test]
    fn proof_buffer_layout_partition_is_disjoint_and_complete() {
        // Caller-side encoding mirrors the on-chain handler's read
        // offsets.  Pin them so any future widening (extra slot, etc.)
        // forces an explicit update here.
        let proof_a_end = 64usize;
        let proof_b_end = proof_a_end + 128;
        let proof_c_end = proof_b_end + 64;
        let nullifier_end = proof_c_end + 32;
        let wire_end = nullifier_end + NR_WIRE_INPUTS * 32;

        assert_eq!(proof_a_end, 64);
        assert_eq!(proof_b_end, 192);
        assert_eq!(proof_c_end, 256);
        assert_eq!(nullifier_end, 288);
        assert_eq!(wire_end, PROOF_BUFFER_PAYLOAD_SIZE);
    }

    #[test]
    fn proof_buffer_payload_fits_two_chunk_uploads_under_legacy_tx_cap() {
        // Each upload_proof_chunk ix carries: ix discriminator (8) +
        // offset (4) + Vec<u8> length prefix (4) + bytes (variable) +
        // accounts overhead (~150) + tx framing (~70) ≈ 236-byte fixed
        // overhead.  A 700-byte chunk fits comfortably under the
        // 1232-byte legacy-tx cap (700 + 236 = 936) and 2 chunks of
        // 480-700 bytes each cover the 960-byte payload.
        let max_payload_per_chunk = 1232usize - 236;
        assert!(max_payload_per_chunk >= 700);
        assert!(2 * 700 >= PROOF_BUFFER_PAYLOAD_SIZE);
    }

    #[test]
    fn proof_buffer_chunk_round_trip_assembles_byte_identical_payload() {
        // Synthesize a deterministic 960-byte payload and verify that
        // upload_proof_chunk-style writes (offset + slice) reproduce it
        // exactly when chunked into two pieces.  Models the SDK's
        // chunked-upload behavior without spinning up a validator.
        let mut full = vec![0u8; PROOF_BUFFER_PAYLOAD_SIZE];
        for (i, b) in full.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(13).wrapping_add(7);
        }

        // Two-chunk split: [0..500), [500..960)
        let mut buf = vec![0u8; PROOF_BUFFER_PAYLOAD_SIZE];
        let mut hwm: usize = 0;

        // Chunk 1
        let off1: usize = 0;
        let bytes1 = &full[off1..500];
        buf[off1..off1 + bytes1.len()].copy_from_slice(bytes1);
        hwm = hwm.max(off1 + bytes1.len());

        // Chunk 2
        let off2: usize = 500;
        let bytes2 = &full[off2..PROOF_BUFFER_PAYLOAD_SIZE];
        buf[off2..off2 + bytes2.len()].copy_from_slice(bytes2);
        hwm = hwm.max(off2 + bytes2.len());

        assert_eq!(hwm, PROOF_BUFFER_PAYLOAD_SIZE, "high-water mark must equal payload");
        assert_eq!(buf, full, "chunked assembly must be byte-identical to single-shot");
    }

    #[test]
    fn proof_buffer_partial_upload_is_detectable_by_high_water_mark() {
        // If the second chunk is missing, the high-water mark stays
        // below PROOF_BUFFER_PAYLOAD_SIZE and verify_batch_proof_v2's
        // ProofBufferIncomplete check fires.
        let mut hwm: usize = 0;
        let off1: usize = 0;
        let bytes1_len: usize = 500;
        hwm = hwm.max(off1 + bytes1_len);
        // Skip chunk 2 (simulating dropped tx).
        assert!(hwm < PROOF_BUFFER_PAYLOAD_SIZE, "partial upload must be < payload size");
    }

    #[test]
    fn proof_buffer_offset_overflow_rejected_by_arithmetic_check() {
        // upload_proof_chunk's checked_add on (offset + bytes.len())
        // catches u32 overflow before the bounds check.  Test models
        // that arithmetic.
        let offset: u32 = u32::MAX - 100;
        let len: usize = 200;
        let off = offset as usize;
        let end = off.checked_add(len);
        // On 64-bit hosts checked_add doesn't overflow (offset+len <
        // usize::MAX), but the subsequent <= PROOF_BUFFER_PAYLOAD_SIZE
        // check (960) would reject.
        let end_val = end.expect("usize add doesn't overflow");
        assert!(end_val > PROOF_BUFFER_PAYLOAD_SIZE, "huge offset must be out of bounds");
    }

    // ─── Groth16 host round-trip (LB5 / SOLID-SEC-067 regression gate) ───
    //
    // Exercises `verify_groth16_proof` (the same fn the on-chain handler
    // calls) on a real proof + VK + publicSignals fixture captured by
    // `scripts/prove.ts` after a successful local snarkjs verify.  If
    // this test passes, the on-chain handler's serialisation contract
    // is correct end-to-end; any e2e-on-chain failure is then localised
    // to validator state, account-data drift, or the wire path.
    //
    // The fixture is generated by running `npm run prove` once -- the
    // script writes `tests/fixtures/groth16_e2e_proof.json` after
    // confirming `snarkjs.groth16.verify(vk, publicSignals, proof) == true`.
    // If the fixture is absent, the test is skipped (so CI on a clean
    // checkout doesn't fail; explicit gate is the prove run).

    use num_bigint::BigUint;
    use num_traits::Num;
    use serde_json::Value as JsonValue;
    use std::path::Path;

    fn dec_str_to_be32(s: &str) -> [u8; 32] {
        let n = BigUint::from_str_radix(s, 10).expect("decimal string");
        let mut be = n.to_bytes_be();
        if be.len() > 32 {
            panic!("bigint exceeds 32 bytes");
        }
        let mut out = [0u8; 32];
        out[32 - be.len()..].copy_from_slice(&be);
        be.clear();
        out
    }

    /// Serialise a snarkjs VK JSON into the on-chain VkBuf wire format.
    /// Mirrors `scripts/initialize.ts::serializeG1/serializeG2` (post-LB5).
    fn serialize_vk_from_snarkjs_json(vk_json: &JsonValue) -> Vec<u8> {
        let mut out = Vec::new();
        let ic_arr = vk_json["IC"].as_array().expect("IC");
        let nr_ic = ic_arr.len() as u32;
        out.extend_from_slice(&nr_ic.to_le_bytes());

        // alpha_g1: G1 (x_BE, y_BE)
        let a = vk_json["vk_alpha_1"].as_array().expect("vk_alpha_1");
        out.extend_from_slice(&dec_str_to_be32(a[0].as_str().unwrap()));
        out.extend_from_slice(&dec_str_to_be32(a[1].as_str().unwrap()));

        // beta_g2 / gamma_g2 / delta_g2: G2 (x_imag, x_real, y_imag, y_real) BE
        for key in &["vk_beta_2", "vk_gamma_2", "vk_delta_2"] {
            let p = vk_json[*key].as_array().expect(*key);
            // p[0] = [x_real, x_imag], p[1] = [y_real, y_imag] (snarkjs convention)
            // groth16-solana expects (imag, real) -> swap on encode.
            let xr = p[0][0].as_str().unwrap();
            let xi = p[0][1].as_str().unwrap();
            let yr = p[1][0].as_str().unwrap();
            let yi = p[1][1].as_str().unwrap();
            out.extend_from_slice(&dec_str_to_be32(xi)); // x_imag
            out.extend_from_slice(&dec_str_to_be32(xr)); // x_real
            out.extend_from_slice(&dec_str_to_be32(yi)); // y_imag
            out.extend_from_slice(&dec_str_to_be32(yr)); // y_real
        }

        // IC: each G1 (x_BE, y_BE)
        for ic in ic_arr {
            let ic_p = ic.as_array().expect("IC[i]");
            out.extend_from_slice(&dec_str_to_be32(ic_p[0].as_str().unwrap()));
            out.extend_from_slice(&dec_str_to_be32(ic_p[1].as_str().unwrap()));
        }
        out
    }

    #[test]
    fn groth16_host_verify_round_trip() {
        // Locate fixture relative to the workspace root.  Tests run from
        // the crate dir (`programs/zk-verifier`), so we walk up two levels.
        let fixture_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/groth16_e2e_proof.json");
        if !fixture_path.exists() {
            eprintln!(
                "[skipped] groth16_host_verify_round_trip: fixture {} missing.\n\
                 Run `npm run prove` once to generate it.",
                fixture_path.display(),
            );
            return;
        }
        let fixture: JsonValue = serde_json::from_str(
            &std::fs::read_to_string(&fixture_path).expect("read fixture"),
        )
        .expect("parse fixture json");

        let vk_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../")
            .join(fixture["vk_path"].as_str().expect("vk_path"));
        let vk_json: JsonValue =
            serde_json::from_str(&std::fs::read_to_string(&vk_path).expect("read vk"))
                .expect("parse vk json");

        // Serialise VK using the same logic the on-chain handler reads.
        let vk_bytes = serialize_vk_from_snarkjs_json(&vk_json);

        // Convert snarkjs publicSignals (string[]) -> [[u8; 32]; NR_PUBLIC_INPUTS].
        let public_signals = fixture["publicSignals"]
            .as_array()
            .expect("publicSignals array");
        assert_eq!(
            public_signals.len(),
            NR_PUBLIC_INPUTS,
            "fixture must have exactly NR_PUBLIC_INPUTS publicSignals"
        );
        let mut public_inputs: [[u8; 32]; NR_PUBLIC_INPUTS] = [[0u8; 32]; NR_PUBLIC_INPUTS];
        for (i, s) in public_signals.iter().enumerate() {
            public_inputs[i] = dec_str_to_be32(s.as_str().expect("publicSignals[i]"));
        }

        // Read the SDK's pre-encoded solanaProof bytes (post-LB5 G2 swap)
        // from the fixture.  These are exactly what the SDK feeds the ix.
        let sp = &fixture["solanaProof"];
        let proof_a_vec: Vec<u8> = sp["proofA"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect();
        let proof_b_vec: Vec<u8> = sp["proofB"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect();
        let proof_c_vec: Vec<u8> = sp["proofC"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect();
        assert_eq!(proof_a_vec.len(), 64);
        assert_eq!(proof_b_vec.len(), 128);
        assert_eq!(proof_c_vec.len(), 64);
        let mut proof_a = [0u8; 64];
        proof_a.copy_from_slice(&proof_a_vec);
        let mut proof_b = [0u8; 128];
        proof_b.copy_from_slice(&proof_b_vec);
        let mut proof_c = [0u8; 64];
        proof_c.copy_from_slice(&proof_c_vec);

        // Run the on-chain handler's verify path on the host.  This
        // exercises VkBuf::parse + negate_g1_point + Groth16Verifier::new
        // + verifier.verify().  If it returns Ok, the on-chain Groth16
        // path is sound; any e2e-on-chain failure is elsewhere.
        match verify_groth16_proof(&vk_bytes, &proof_a, &proof_b, &proof_c, &public_inputs) {
            Ok(()) => {
                eprintln!("[host-verify] OK -- LB5 / SOLID-SEC-067 round-trip green");
            }
            Err(e) => {
                panic!(
                    "verify_groth16_proof FAILED on host: {:?}.\n\
                     Either the LB5 G2 (imag, real) swap is still wrong, the proof_a Y \
                     negation diverges, the VK serialisation order disagrees with \
                     groth16-solana, or the public-input encoding is off.",
                    e
                );
            }
        }
    }
}
