use anchor_lang::prelude::*;
use groth16_solana::groth16::{Groth16Verifier, Groth16Verifyingkey};
use solid_light::cpi_helpers;

declare_id!("FhtEvsUwxRRT8nf3uaxK6iAefFqvqCYimc9vqnH5XEgr");

// ─── Circuit Constants ─────────────────────────────────────────────────────
// Our batch_credential_query circuit (Phase 3.1) has 30 public inputs.
// [0] = nullifierHash (output)
// [1] = globalRoot
// [2..5] = merkleRoots[4]
// [6..9] = schemaHashes[4]
// [10..13] = queryCredentialIndices[4]
// [14..17] = queryFieldIndices[4]
// [18..21] = queryOperators[4]
// [22..25] = queryValues[4]
// [26] = numPredicates
// [27] = compoundLogic
// [28] = verifierAddress
// [29] = verifierNonce
// [30] = currentTimestamp
const NR_PUBLIC_INPUTS: usize = 31; 

// ─── Bloom Filter Constants ────────────────────────────────────────────────
const BLOOM_BITS: usize = 262144; // 256 * 1024 bits
const BLOOM_BYTES: usize = BLOOM_BITS / 8; // 32768 bytes
const BLOOM_K: usize = 7; // Number of hash functions

/// ═══════════════════════════════════════════════════════════════════════════
/// ZK Verifier Program
/// ═══════════════════════════════════════════════════════════════════════════
///
/// Production Groth16 proof verification with:
///   - Real groth16-solana verification (alt_bn128 native syscalls)
///   - Bloom filter nullifier registry (~100K capacity, O(1) lookup)
///   - Verification key stored in separate PDA (serialized VK bytes)
///   - Events for indexer consumption
#[program]
pub mod zk_verifier {
    use super::*;

    /// Initialize the verifier config (authority only).
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let config = &mut ctx.accounts.verifier_config;
        config.authority = ctx.accounts.authority.key();
        config.proof_count = 0;
        config.vk_initialized = false;
        config.bump = ctx.bumps.verifier_config; // SEC-11: Store bump
        msg!("ZK Verifier initialized");
        Ok(())
    }

    /// Store the verification key (may require multiple calls for large VKs).
    /// The VK bytes are the raw concatenation of:
    ///   nr_pubinputs (8 bytes LE) || vk_alpha_g1 (64) || vk_beta_g2 (128) ||
    ///   vk_gamme_g2 (128) || vk_delta_g2 (128) || vk_ic (N * 64)
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
            msg!("Verification key stored: {} bytes", vk_storage.data.len());
    /// Verify a Groth16 proof with On-Chain Root Anchoring (Phase 3.1).
    ///
    /// 1. Verify that public_inputs[X] (globalRoot) is valid via Light Protocol CPI.
    /// 2. Check nullifier isn't already used in Light Compressed State.
    /// 3. Verify Groth16 proof via syscalls.
    /// 4. Record nullifier in Light State to prevent replay.
    pub fn verify_batch_proof(
        ctx: Context<VerifyBatchProof>,
        proof_a: [u8; 64],
        proof_b: [u8; 128],
        proof_c: [u8; 64],
        public_inputs: [[u8; 32]; NR_PUBLIC_INPUTS],
        nullifier: [u8; 32],
    ) -> Result<()> {
        let config = &ctx.accounts.verifier_config;
        require!(config.vk_initialized, ErrorCode::VerificationKeyNotSet);

        // 1. Nullifier Binding
        require!(nullifier == public_inputs[0], ErrorCode::NullifierMismatch);

        // 2. PHASE 3.1: SEC-13 Nullifier Scope Binding
        // verifierAddress is public_inputs[28]
        let verifier_address_input = public_inputs[28];
        require!(
            verifier_address_input == ID.to_bytes(),
            ErrorCode::InvalidVerifierAddress
        );

        // 3. PHASE 3.1: Shared Global Root Verification (Light Protocol CPI)
        // globalRoot is public_inputs[1]
        let global_root = public_inputs[1];
        let tree_account_data = ctx.accounts.merkle_tree.try_borrow_data()?;
        
        let global_root_exists = cpi_helpers::verify_state_root_matches(
            &tree_account_data,
            &global_root,
        );
        require!(global_root_exists, ErrorCode::InvalidGlobalRoot);

        // 4. PHASE 3.1: Schema-Root Binding (N=4) & Canonical Ordering (Phase 3.2)
        // merkleRoots[2..5], schemaHashes[6..9]
        for i in 0..4 {
            let merkle_root = public_inputs[2 + i];
            let schema_hash = public_inputs[6 + i];
            
            // Skip placeholders (zero hash)
            if merkle_root == [0u8; 32] {
                continue;
            }

            // PHASE 3.2: SEC-20 Canonical Ordering Check
            // Enforce strictly ascending order to prevent malleability.
            if i < 3 {
                let next_schema = public_inputs[6 + i + 1];
                if next_schema != [0u8; 32] {
                    require!(
                        schema_hash < next_schema,
                        ErrorCode::InvalidCredentialOrder
                    );
                }
            }

            // SEC-19: Verify root-schema binding.
            let root_valid = cpi_helpers::verify_schema_root_binding(
                &tree_account_data,
                &merkle_root,
                &schema_hash,
            );
            require!(root_valid, ErrorCode::InvalidSchemaRootBinding);
        }

        // 5. Groth16 Verification
        let vk_data = &ctx.accounts.vk_storage.data;
        let vk = deserialize_vk(vk_data).map_err(|_| ErrorCode::InvalidProofFormat)?;
        let proof_a_neg = negate_g1_point(&proof_a);

        let mut verifier = Groth16Verifier::<NR_PUBLIC_INPUTS>::new(
            &proof_a_neg, &proof_b, &proof_c, &public_inputs, &vk,
        ).map_err(|_| ErrorCode::InvalidProofFormat)?;

        verifier.verify().map_err(|_| ErrorCode::ProofVerificationFailed)?;

        // 6. PHASE 2.1: Stateless Nullifier Registration
        cpi_helpers::register_nullifier_cpi(
            &ctx.accounts.light_program,
            &ctx.accounts.merkle_tree,
            &ctx.accounts.payer,
            &ctx.accounts.system_program,
            nullifier,
        )?;

        // 7. Update state tracking
        let config_mut = &mut ctx.accounts.verifier_config;
        config_mut.proof_count += 1;

        emit!(CredentialVerified {
             nullifier,
             proof_count: config_mut.proof_count,
             public_input_count: NR_PUBLIC_INPUTS as u8,
             timestamp: Clock::get()?.unix_timestamp,
        });

        msg!("Phase 3 Batch Proof Verified. Credentials: 4. Total proofs: {}", config_mut.proof_count);
        Ok(())
    }

    /// Initialize a new self-sovereign identity in the global state tree (Phase 2.3).
    pub fn initialize_identity(ctx: Context<InitializeIdentity>) -> Result<()> {
        let authority = ctx.accounts.authority.key();
        
        cpi_helpers::register_identity_cpi(
            &ctx.accounts.light_program,
            &ctx.accounts.merkle_tree,
            &ctx.accounts.authority,
            &ctx.accounts.system_program,
            authority.to_bytes(),
            0, // Initial nonce
        )?;

        msg!("Identity initialized for owner: {}", authority);
        Ok(())
    }

    /// Rotate the identity state to revoke all previous proofs (Phase 2.3).
    pub fn rotate_identity(ctx: Context<RotateIdentity>, new_nonce: u64) -> Result<()> {
        let authority = ctx.accounts.authority.key();

        cpi_helpers::register_identity_cpi(
            &ctx.accounts.light_program,
            &ctx.accounts.merkle_tree,
            &ctx.accounts.authority,
            &ctx.accounts.system_program,
            authority.to_bytes(),
            new_nonce,
        )?;

        msg!("Identity rotated. Nonce: {}. All previous proofs are now REVOKED.", new_nonce);
        Ok(())
    }
}

// ─── VK Deserialization ────────────────────────────────────────────────────
// (Unchanged logic for deserialize_vk, bloom functions removed)

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
        init,
        payer = authority,
        space = 8 + 32 + 8 + 1,
        seeds = [b"verifier-config"],
        bump
    )]
    pub verifier_config: Account<'info, VerifierConfig>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct StoreVerificationKey<'info> {
    #[account(mut, seeds = [b"verifier-config"], bump)]
    pub verifier_config: Account<'info, VerifierConfig>,
    #[account(
        init_if_needed,
        payer = authority,
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
#[instruction(proof_a: [u8; 64], proof_b: [u8; 128], proof_c: [u8; 64], public_inputs: [[u8; 32]; NR_PUBLIC_INPUTS], nullifier: [u8; 32])]
pub struct VerifyBatchProof<'info> {
    #[account(
        mut, 
        seeds = [b"verifier-config"], 
        bump = verifier_config.bump
    )]
    pub verifier_config: Account<'info, VerifierConfig>,
    #[account(
        seeds = [b"vk-storage", verifier_config.key().as_ref()],
        bump
    )]
    pub vk_storage: Account<'info, VkStorage>,
    /// CHECK: Light Protocol Merkle tree account
    pub merkle_tree: UncheckedAccount<'info>,
    /// CHECK: Light Protocol program
    pub light_program: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeIdentity<'info> {
    #[account(mut)]
    pub merkle_tree: UncheckedAccount<'info>,
    pub light_program: UncheckedAccount<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RotateIdentity<'info> {
    #[account(mut)]
    pub merkle_tree: UncheckedAccount<'info>,
    pub light_program: UncheckedAccount<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// ─── State ─────────────────────────────────────────────────────────────────

#[account]
pub struct VerifierConfig {
    pub authority: Pubkey,
    pub proof_count: u64,
    pub vk_initialized: bool,
    pub bump: u8,
}

#[account]
pub struct VkStorage {
    pub data: Vec<u8>,
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
    #[msg("Nullifier mismatch")]
    NullifierMismatch,
    #[msg("Invalid verifier address")]
    InvalidVerifierAddress,
    #[msg("Invalid schema-root binding")]
    InvalidSchemaRootBinding,
    #[msg("Invalid credential order")]
    InvalidCredentialOrder,
}
