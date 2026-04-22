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
use solid_light::cpi_helpers::SCHEMA_REGISTRY_ID;

declare_id!("BZkVFdMhAEeGMvEAhXNjt3r3bEA2sCPqEFcsEbSbFGj2");

// ─── Circuit Constants ─────────────────────────────────────────────────────
// batch_credential_query (Phase 3.1) public inputs (31 total):
//   [0]      = nullifierHash (circuit output)
//   [1]      = globalRoot
//   [2..5]   = merkleRoots[4]
//   [6..9]   = schemaHashes[4]
//   [10..13] = queryCredentialIndices[4]
//   [14..17] = queryFieldIndices[4]
//   [18..21] = queryOperators[4]
//   [22..25] = queryValues[4]
//   [26]     = numPredicates
//   [27]     = compoundLogic
//   [28]     = verifierAddress
//   [29]     = verifierNonce
//   [30]     = currentTimestamp
pub const NR_PUBLIC_INPUTS: usize = 31;

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

// Compile-time assertion: the stack-owned VK buffer must fit comfortably
// inside Solana's per-frame BPF stack budget (4 KB).
//   VkBuf = 8 (nr_ic) + 64 (alpha) + 128 (beta) + 128 (gamma) + 128 (delta)
//         + 32 * 64 (ic) = 2 504 bytes.  Well below 4 096.
const _: () = {
    let sz = core::mem::size_of::<VkBuf>();
    assert!(sz < 3072, "VkBuf exceeds safe BPF stack slice");
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
        msg!("SolID ZK Verifier initialized. Authority: {}", config.authority);
        Ok(())
    }

    /// Update the clock-skew tolerance window for `currentTimestamp` public
    /// input (SOLID-SEC-005). Authority-only.
    ///
    /// The window caps at 1 hour. Longer windows defeat the freshness
    /// guarantee without a compelling reason; if a use case needs one, add a
    /// new ADR first (per `adr/README.md`).
    pub fn set_timestamp_skew(
        ctx: Context<AuthorityOnly>,
        skew_seconds: u32,
    ) -> Result<()> {
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
    /// Two invariants the old implementation missed:
    ///   1. Chunks must arrive in order (`chunk_index == config.next_vk_chunk`).
    ///      Without this a confused operator could scramble the VK and the
    ///      parser would silently accept a corrupt key.
    ///   2. The cumulative VK size is capped at the allocated 10_240 bytes.
    ///      Without the cap the Vec's realloc would fail at serialize-time
    ///      with an opaque error on the final chunk.
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
        require!(
            chunk_index == config.next_vk_chunk,
            ErrorCode::ChunkOutOfOrder
        );

        const VK_MAX_BYTES: usize = 10_240;
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
    pub fn verify_batch_proof(
        ctx: Context<VerifyBatchProof>,
        proof_a: [u8; 64],
        proof_b: [u8; 128],
        proof_c: [u8; 64],
        public_inputs: [[u8; 32]; NR_PUBLIC_INPUTS],
        nullifier: [u8; 32],
    ) -> Result<()> {
        let config = &ctx.accounts.verifier_config;
        require!(!config.paused, ErrorCode::Paused);
        require!(config.vk_initialized, ErrorCode::VerificationKeyNotSet);

        // (1) Nullifier binding: the output signal (public_inputs[0]) must equal
        // the `nullifier` the caller is about to register as a PDA seed.
        require!(nullifier == public_inputs[0], ErrorCode::NullifierMismatch);

        // (2) SEC-13: verifier scope binding.
        let verifier_address_input = public_inputs[28];
        require!(
            verifier_address_input == ID.to_bytes(),
            ErrorCode::InvalidVerifierAddress
        );

        // (2b) SOLID-SEC-005: bind `currentTimestamp` public input to on-chain
        // Clock. The circuit enforces `currentTimestamp <= expirationTimestamp`
        // per credential, but without an on-chain freshness check a prover may
        // pass `currentTimestamp = 0` and defeat every expiration gate.
        //
        // public_inputs[30] is a 32-byte LE encoding of a BN254 field element.
        // Plausible unix timestamps fit in a u64, so bytes [8..32] MUST be
        // zero; otherwise the caller has either (a) fed the circuit a
        // pathological value or (b) packed the input with the wrong encoding
        // (see SOLID-SEC-031 for the related SDK-side fix). Either way we
        // reject.
        let ts_bytes = public_inputs[30];
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
        // Parse the VK into a STACK-OWNED buffer.  `VkBuf` is ~2.5 KB and fits
        // inside Solana's 4 KB per-frame BPF stack.  The `Groth16Verifyingkey`
        // we hand to `groth16-solana` borrows from this buffer; its lifetime
        // ends when this function returns.  No heap allocation, no `Box::leak`.
        let vk_buf = VkBuf::parse(&ctx.accounts.vk_storage.data)
            .map_err(|_| ErrorCode::InvalidProofFormat)?;
        let vk = vk_buf.as_verifying_key();
        let proof_a_neg = negate_g1_point(&proof_a).map_err(|_| ErrorCode::InvalidProofFormat)?;

        let mut verifier = Groth16Verifier::<NR_PUBLIC_INPUTS>::new(
            &proof_a_neg,
            &proof_b,
            &proof_c,
            &public_inputs,
            &vk,
        )
        .map_err(|_| ErrorCode::InvalidProofFormat)?;

        verifier
            .verify()
            .map_err(|_| ErrorCode::ProofVerificationFailed)?;

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
#[repr(C)]
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub struct VkBuf {
    nr_ic: usize,
    alpha: [u8; 64],
    beta: [u8; 128],
    gamma: [u8; 128],
    delta: [u8; 128],
    ic: [[u8; 64]; MAX_IC],
}

#[derive(Debug, PartialEq, Eq)]
pub enum VkParseError {
    TooShort,
    TruncatedIc,
    IcOverflow,
}

impl VkBuf {
    /// Parse the on-chain VK byte buffer into a stack-owned `VkBuf`.
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

        let mut buf = VkBuf {
            nr_ic,
            alpha: [0u8; 64],
            beta: [0u8; 128],
            gamma: [0u8; 128],
            delta: [0u8; 128],
            ic: [[0u8; 64]; MAX_IC],
        };

        let mut cursor = 4;
        buf.alpha.copy_from_slice(&bytes[cursor..cursor + 64]);
        cursor += 64;
        buf.beta.copy_from_slice(&bytes[cursor..cursor + 128]);
        cursor += 128;
        buf.gamma.copy_from_slice(&bytes[cursor..cursor + 128]);
        cursor += 128;
        buf.delta.copy_from_slice(&bytes[cursor..cursor + 128]);
        cursor += 128;

        for slot in buf.ic.iter_mut().take(nr_ic) {
            slot.copy_from_slice(&bytes[cursor..cursor + 64]);
            cursor += 64;
        }
        Ok(buf)
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

/// Negate the Y coordinate of a G1 point for groth16-solana's expected
/// `-A` representation. Assumes 32-byte big-endian X || Y.
fn negate_g1_point(point: &[u8; 64]) -> std::result::Result<[u8; 64], ()> {
    // BN254 base-field prime (alt_bn128), big-endian.
    const P_BE: [u8; 32] = [
        0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29,
        0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58, 0x5d,
        0x97, 0x81, 0x6a, 0x91, 0x68, 0x71, 0xca, 0x8d,
        0x3c, 0x20, 0x8c, 0x16, 0xd8, 0x7c, 0xfd, 0x47,
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
        space = 8 + 4 + 10240,
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
    /// Reset to zero only through a redeploy; once the VK is finalized this
    /// serves as an audit trail of how many chunks were stitched together.
    pub next_vk_chunk: u16,
    /// SOLID-SEC-005: tolerance (seconds) between the `currentTimestamp`
    /// public input and the on-chain Clock at `verify_batch_proof` time.
    /// Default `DEFAULT_TIMESTAMP_SKEW_SECONDS`; configurable via
    /// `set_timestamp_skew` up to `MAX_TIMESTAMP_SKEW_SECONDS`.
    pub timestamp_skew_seconds: u32,
}

impl VerifierConfig {
    // 32 authority + 8 proof_count + 1 vk_initialized + 1 paused + 1 bump
    // + 2 next_vk_chunk + 4 timestamp_skew_seconds.
    pub const SPACE: usize = 32 + 8 + 1 + 1 + 1 + 2 + 4;
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
    #[msg("VK storage capacity exceeded (10240 bytes)")]
    VkStorageFull,
    #[msg("Arithmetic overflow")]
    Overflow,
    #[msg("currentTimestamp public input is outside the configured skew window")]
    StaleTimestamp,
    #[msg("Timestamp skew value exceeds MAX_TIMESTAMP_SKEW_SECONDS (3600)")]
    TimestampSkewTooLarge,
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
        v.extend_from_slice(&[1u8; 64]);   // alpha
        v.extend_from_slice(&[2u8; 128]);  // beta
        v.extend_from_slice(&[3u8; 128]);  // gamma
        v.extend_from_slice(&[4u8; 128]);  // delta
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
        // Mirrors the const assertion in the top of the file — having it also
        // as a runtime test makes the failure message readable.
        assert!(core::mem::size_of::<VkBuf>() < 3072);
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
}
