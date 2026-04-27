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
// ADR-0012, which previously pinned 31):
//   [0]      = nullifierHash (circuit output; 6-input Poseidon post ADR-0014)
//   [1]      = globalRoot
//   [2..5]   = merkleRoots[4]
//   [6..9]   = schemaHashes[4]
//   [10]     = issuerTreeRoot                  (NEW; SEC-004 / SEC-008)
//   [11..14] = queryCredentialIndices[4]
//   [15..18] = queryFieldIndices[4]
//   [19..22] = queryOperators[4]
//   [23..26] = queryValues[4]
//   [27]     = numPredicates
//   [28]     = compoundLogic
//   [29]     = verifierAddress
//   [30]     = verifierNonce
//   [31]     = currentTimestamp
pub const NR_PUBLIC_INPUTS: usize = 32;

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
    /// `public_inputs` is `Vec<[u8; 32]>` (heap-resident) rather than
    /// `[[u8; 32]; NR_PUBLIC_INPUTS]` (1 024 bytes inline) by design.
    /// Anchor's `__global` wrapper deserializes the args struct, then
    /// passes args by value into this fn -- on BPF the arg copy lives in
    /// the wrapper's outgoing-args stack slots, doubling the array's
    /// stack footprint.  At `NR_PUBLIC_INPUTS = 32` that pushed the
    /// wrapper 456 bytes past the 4 KB per-frame BPF budget.  A `Vec`
    /// is a 24-byte fat pointer regardless of length, so the wrapper
    /// only holds one cheap copy and the underlying 1 024-byte buffer
    /// stays on the heap.  Length is validated equal to
    /// `NR_PUBLIC_INPUTS` before any indexed access; conversion to the
    /// `&[[u8; 32]; NR_PUBLIC_INPUTS]` shape that `Groth16Verifier`
    /// expects is a single zero-cost `TryInto` on the slice.  See
    /// `ts-sdk/packages/verifier/src/index.ts` for the matching wire
    /// encoding (4-byte LE length prefix before the 32x32-byte payload).
    #[inline(never)]
    pub fn verify_batch_proof(
        ctx: Context<VerifyBatchProof>,
        proof_a: [u8; 64],
        proof_b: [u8; 128],
        proof_c: [u8; 64],
        public_inputs: Vec<[u8; 32]>,
        nullifier: [u8; 32],
    ) -> Result<()> {
        let config = &ctx.accounts.verifier_config;
        require!(!config.paused, ErrorCode::Paused);
        require!(config.vk_initialized, ErrorCode::VerificationKeyNotSet);

        // (0) Public-input arity gate.  Every downstream slot index in this
        // handler is a compile-time constant against `NR_PUBLIC_INPUTS`
        // (e.g. `VERIFIER_ADDRESS_INPUT_INDEX`, `CURRENT_TIMESTAMP_INPUT_INDEX`,
        // and the schema/merkle range `[2..10]`).  A Vec gives a malicious
        // caller the freedom to send the wrong length and crash the program
        // on the first out-of-bounds access; converting to a fixed-size
        // reference here turns that into a typed error.
        let public_inputs: &[[u8; 32]; NR_PUBLIC_INPUTS] = public_inputs
            .as_slice()
            .try_into()
            .map_err(|_| ErrorCode::InvalidProofFormat)?;

        // (1) Nullifier binding: the output signal (public_inputs[0]) must equal
        // the `nullifier` the caller is about to register as a PDA seed.
        require!(nullifier == public_inputs[0], ErrorCode::NullifierMismatch);

        // (2) SEC-13: verifier scope binding.  Index shifted to 29
        // by ADR-0014 (issuerTreeRoot inserted at [10]).
        let verifier_address_input = public_inputs[VERIFIER_ADDRESS_INPUT_INDEX];
        require!(
            verifier_address_input == ID.to_bytes(),
            ErrorCode::InvalidVerifierAddress
        );

        // (2b) SOLID-SEC-005: bind `currentTimestamp` public input to on-chain
        // Clock. The circuit enforces `currentTimestamp <= expirationTimestamp`
        // per credential, but without an on-chain freshness check a prover may
        // pass `currentTimestamp = 0` and defeat every expiration gate.
        //
        // public_inputs[CURRENT_TIMESTAMP_INPUT_INDEX] is a 32-byte LE
        // encoding of a BN254 field element.  Plausible unix timestamps
        // fit in a u64, so bytes [8..32] MUST be zero; otherwise the
        // caller has either (a) fed the circuit a pathological value
        // or (b) packed the input with the wrong encoding (see
        // SOLID-SEC-031 for the related SDK-side fix). Either way we
        // reject.
        let ts_bytes = public_inputs[CURRENT_TIMESTAMP_INPUT_INDEX];
        for i in 8..32 {
            require!(ts_bytes[i] == 0, ErrorCode::StaleTimestamp);
        }
        let claimed_ts = u64::from_le_bytes(
            ts_bytes[0..8]
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

        // (3) Global-root verification.
        //
        // The `global_tree` account is the `GlobalStateBinding` PDA owned by
        // `schema-registry`. Without the owner check below an attacker can
        // pass a system-owned account with a forged `globroot` discriminator
        // and any root bytes: the parse-level verify_state_root_matches would
        // succeed against fabricated data and the whole ZK path becomes
        // bypassable.
        require_keys_eq!(
            *ctx.accounts.global_tree.owner,
            SCHEMA_REGISTRY_ID,
            ErrorCode::InvalidGlobalRoot
        );
        let global_root = public_inputs[1];
        let tree_account_data = ctx.accounts.global_tree.try_borrow_data()?;
        require!(
            cpi_helpers::verify_state_root_matches(&tree_account_data, &global_root),
            ErrorCode::InvalidGlobalRoot
        );

        // (3b) ADR-0014: issuer-tree root binding.
        //
        // Cross-check `public_inputs[ISSUER_TREE_ROOT_INPUT_INDEX]`
        // against the singleton `IssuerTreeBinding.current_root`.  The
        // owner check is load-bearing for the same reason as (3) -- a
        // system-owned account with a forged `issrtree` discriminator
        // would otherwise parse cleanly.  Bundled with the SOLID-SEC-008
        // epoch nullifier (the root is already in `public_inputs[0]`'s
        // preimage), this closes the post-revocation replay window.
        require_keys_eq!(
            *ctx.accounts.issuer_tree_binding.owner,
            ISSUER_REGISTRY_ID,
            ErrorCode::InvalidIssuerTreeBinding
        );
        let issuer_tree_root_input = public_inputs[ISSUER_TREE_ROOT_INPUT_INDEX];
        let issuer_binding_data = ctx.accounts.issuer_tree_binding.try_borrow_data()?;
        cpi_helpers::verify_issuer_tree_binding_for_proof(
            &issuer_binding_data,
            &issuer_tree_root_input,
        )
        .map_err(|e| match e {
            cpi_helpers::LightError::IssuerTreeRootMismatch => ErrorCode::IssuerTreeRootMismatch,
            cpi_helpers::LightError::IssuerTreeBindingFrozen => ErrorCode::IssuerTreeBindingFrozen,
            _ => ErrorCode::InvalidIssuerTreeBinding,
        })?;
        drop(issuer_binding_data);

        // (4) Schema ↔ root binding + canonical ordering.
        //
        // Each non-zero slot `i` has:
        //   merkle_roots[i] = public_inputs[2 + i]
        //   schema_hashes[i] = public_inputs[6 + i]
        //
        // For every active slot we require:
        //   (a) the schema must be strictly greater than the previous active schema
        //   (b) the registered credential-tree PDA for that schema must exist
        //       and its stored root must equal `merkle_roots[i]`.
        let mut last_schema: Option<[u8; 32]> = None;
        for i in 0..4usize {
            let merkle_root = public_inputs[2 + i];
            let schema_hash = public_inputs[6 + i];

            if merkle_root == [0u8; 32] && schema_hash == [0u8; 32] {
                continue;
            }

            // (4a) Canonical ordering: schemas strictly ascending.
            if let Some(prev) = last_schema {
                require!(schema_hash > prev, ErrorCode::InvalidCredentialOrder);
            }
            last_schema = Some(schema_hash);

            // (4b) Registered tree lookup via schema-registry PDA.
            // The caller must pass 4 `schema_tree_info[i]` accounts, one per
            // active slot (zero slots allow the default `Pubkey::default()`).
            //
            // The owner check here is load-bearing for the same reason as the
            // global_tree check above: without it, a crafted system-owned
            // account carrying a `schmtree` discriminator would be accepted.
            let tree_info = match i {
                0 => &ctx.accounts.schema_tree_0,
                1 => &ctx.accounts.schema_tree_1,
                2 => &ctx.accounts.schema_tree_2,
                _ => &ctx.accounts.schema_tree_3,
            };
            require_keys_eq!(
                *tree_info.owner,
                SCHEMA_REGISTRY_ID,
                ErrorCode::InvalidSchemaRootBinding
            );
            let data = tree_info.try_borrow_data()?;
            require!(
                cpi_helpers::verify_schema_root_binding(&data, &merkle_root, &schema_hash),
                ErrorCode::InvalidSchemaRootBinding
            );
        }

        // (5) Groth16 verification.
        //
        // Delegate to a separate, NEVER-INLINED helper.  The BPF target
        // is a flat 4 KB per-frame stack and `lto=fat` aggressively
        // inlines the user handler into Anchor's `__global::*` wrapper.
        // After the merge, the wrapper holds Anchor's deserialized
        // argument struct (`proof_a` 64 + `proof_b` 128 + `proof_c` 64
        // + `public_inputs` 32×32 = 1024 + `nullifier` 32 = 1 312 bytes)
        // AND a second moved copy of the same 1 312 bytes when those
        // args are passed by value into the user fn — plus `vk_buf`
        // (~480 bytes after the IC table moved to the heap), the
        // by-value `Groth16Verifyingkey` returned by `as_verifying_key`
        // (~480 bytes), `proof_a_neg` (64 bytes), and the
        // `Groth16Verifier` (~120 bytes).  That tipped
        // `__global::verify_batch_proof` 456 bytes past the 4 KB BPF
        // budget.
        //
        // Confining the heavy Groth16 locals to a `#[inline(never)]`
        // helper keeps them in their own stack frame, well isolated
        // from the wrapper's argument-deserialization frame.  This is
        // the durable fix: it does not depend on LTO's inlining
        // heuristics and survives compiler upgrades.
        verify_groth16_proof(
            &ctx.accounts.vk_storage.data,
            &proof_a,
            &proof_b,
            &proof_c,
            public_inputs,
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
}

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
    public_inputs: [[u8; 32]; NR_PUBLIC_INPUTS], nullifier: [u8; 32]
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
}
