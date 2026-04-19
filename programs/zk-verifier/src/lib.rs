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

pub const NULLIFIER_SEED: &[u8] = b"null";

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
        msg!("SolID ZK Verifier initialized. Authority: {}", config.authority);
        Ok(())
    }

    /// Store the verification key in chunks (may require multiple calls for large VKs).
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

        if chunk_index == 0 {
            vk_storage.data = chunk_data;
        } else {
            vk_storage.data.extend_from_slice(&chunk_data);
        }

        if is_final_chunk {
            config.vk_initialized = true;
            msg!(
                "Verification key stored: {} bytes total",
                vk_storage.data.len()
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

        // (3) Global-root verification.
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
            let tree_info = match i {
                0 => &ctx.accounts.schema_tree_0,
                1 => &ctx.accounts.schema_tree_1,
                2 => &ctx.accounts.schema_tree_2,
                _ => &ctx.accounts.schema_tree_3,
            };
            let data = tree_info.try_borrow_data()?;
            require!(
                cpi_helpers::verify_schema_root_binding(&data, &merkle_root, &schema_hash),
                ErrorCode::InvalidSchemaRootBinding
            );
        }

        // (5) Groth16 verification.
        let vk_data = &ctx.accounts.vk_storage.data;
        let vk = deserialize_vk(vk_data).map_err(|_| ErrorCode::InvalidProofFormat)?;
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

/// Deserialize the VK bytes stored via `store_verification_key` into a
/// `Groth16Verifyingkey`.
///
/// Expected byte layout (little-endian counts, big-endian curve points):
///   u32 nr_ic            — number of IC points
///   [u8; 64]  alpha_g1
///   [u8; 128] beta_g2
///   [u8; 128] gamma_g2
///   [u8; 128] delta_g2
///   repeated [u8; 64] ic[0..nr_ic]
fn deserialize_vk(bytes: &[u8]) -> std::result::Result<Groth16Verifyingkey<'static>, ()> {
    use std::convert::TryInto;
    if bytes.len() < 4 + 64 + 128 * 3 {
        return Err(());
    }
    let mut cursor = 0usize;

    let nr_ic = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().map_err(|_| ())?) as usize;
    cursor += 4;

    let alpha: [u8; 64] = bytes[cursor..cursor + 64].try_into().map_err(|_| ())?;
    cursor += 64;
    let beta: [u8; 128] = bytes[cursor..cursor + 128].try_into().map_err(|_| ())?;
    cursor += 128;
    let gamma: [u8; 128] = bytes[cursor..cursor + 128].try_into().map_err(|_| ())?;
    cursor += 128;
    let delta: [u8; 128] = bytes[cursor..cursor + 128].try_into().map_err(|_| ())?;
    cursor += 128;

    if bytes.len() < cursor + nr_ic * 64 {
        return Err(());
    }

    // The Groth16Verifyingkey type holds references; we need the IC points to
    // live for the duration of verification. We leak a Box to produce a
    // 'static reference — this is called exactly once per proof and the memory
    // is reclaimed when the program returns. (Solana programs are short-lived
    // processes, so the total allocation is bounded.)
    let mut ic: Vec<[u8; 64]> = Vec::with_capacity(nr_ic);
    for _ in 0..nr_ic {
        let point: [u8; 64] = bytes[cursor..cursor + 64].try_into().map_err(|_| ())?;
        cursor += 64;
        ic.push(point);
    }
    let ic_static: &'static [[u8; 64]] = Box::leak(ic.into_boxed_slice());

    let alpha_static: &'static [u8; 64] = Box::leak(Box::new(alpha));
    let beta_static: &'static [u8; 128] = Box::leak(Box::new(beta));
    let gamma_static: &'static [u8; 128] = Box::leak(Box::new(gamma));
    let delta_static: &'static [u8; 128] = Box::leak(Box::new(delta));

    Ok(Groth16Verifyingkey {
        nr_pubinputs: nr_ic.saturating_sub(1), // IC count = nr_pubinputs + 1
        vk_alpha_g1: *alpha_static,
        vk_beta_g2: *beta_static,
        vk_gamme_g2: *gamma_static,
        vk_delta_g2: *delta_static,
        vk_ic: ic_static,
    })
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
}

impl VerifierConfig {
    pub const SPACE: usize = 32 + 8 + 1 + 1 + 1;
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
}
