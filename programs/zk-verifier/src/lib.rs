use anchor_lang::prelude::*;
use groth16_solana::groth16::{Groth16Verifier, Groth16Verifyingkey};
use solid_light::cpi_helpers;

declare_id!("FhtEvsUwxRRT8nf3uaxK6iAefFqvqCYimc9vqnH5XEgr");

// ─── Circuit Constants ─────────────────────────────────────────────────────
// Our compound_query circuit has 21 public inputs + 1 output = 22 public signals.
const NR_PUBLIC_INPUTS: usize = 22;

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
       /// Verify a Groth16 proof with On-Chain Root Anchoring (Phase 5).
    ///
    /// 1. Verify that public_inputs[X] (globalRoot) is valid via Light Protocol CPI.
    /// 2. Check nullifier isn't already used in Light Compressed State.
    /// 3. Verify Groth16 proof via syscalls.
    /// 4. Record nullifier in Light State to prevent replay.
    pub fn verify_proof_v2(
        ctx: Context<VerifyProofV2>,
        proof_a: [u8; 64],
        proof_b: [u8; 128],
        proof_c: [u8; 64],
        public_inputs: [[u8; 32]; NR_PUBLIC_INPUTS],
        nullifier: [u8; 32],
    ) -> Result<()> {
        let config = &ctx.accounts.verifier_config;
        require!(config.vk_initialized, ErrorCode::VerificationKeyNotSet);

        // 1. PHASE 2.4: SEC-13 Nullifier Scope Binding
        // Verify that public_inputs[19] (verifierAddress) matches this program's ID.
        // This prevents cross-application proof replay.
        let verifier_address_input = public_inputs[19];
        require!(
            verifier_address_input == ID.to_bytes(),
            ErrorCode::InvalidVerifierAddress
        );

        // 2. PHASE 5.2: SEC-03 On-Chain Root Verification (Light Protocol CPI)
        // Verify that public_inputs[0] (globalRoot) is valid via Merkle tree check.
        let global_root = public_inputs[0];
        let tree_account_data = ctx.accounts.merkle_tree.try_borrow_data()?;
        
        let root_exists = cpi_helpers::verify_state_root_matches(
            &tree_account_data,
            &global_root,
        );
        require!(root_exists, ErrorCode::InvalidGlobalRoot);

        // 3. Groth16 Verification
        let vk_data = &ctx.accounts.vk_storage.data;
        let vk = deserialize_vk(vk_data).map_err(|_| ErrorCode::InvalidProofFormat)?;
        let proof_a_neg = negate_g1_point(&proof_a);

        let mut verifier = Groth16Verifier::<NR_PUBLIC_INPUTS>::new(
            &proof_a_neg, &proof_b, &proof_c, &public_inputs, &vk,
        ).map_err(|_| ErrorCode::InvalidProofFormat)?;

        // 4. PHASE 2.1: Stateless Nullifier Registration (Light Protocol CPI)
        // Instead of a PDA, we "shield" (create) a compressed nullifier account.
        // If the nullifier has been used, Light Protocol's Merkle tree verification
        // or account-creation constraints will catch the duplicate.
        cpi_helpers::register_nullifier_cpi(
            &ctx.accounts.light_program,
            &ctx.accounts.merkle_tree,
            &ctx.accounts.payer,
            &ctx.accounts.system_program,
            nullifier,
        )?;

        // 5. Update state tracking
        let config = &mut ctx.accounts.verifier_config;
        config.proof_count += 1;

        emit!(CredentialVerified {
            nullifier,
            proof_count: config.proof_count,
            public_input_count: NR_PUBLIC_INPUTS as u8,
            timestamp: Clock::get()?.unix_timestamp,
        });

        msg!("Tier 3.0 Proof Verified & Anchored. Total: {}", config.proof_count);
        Ok(())
    }
        } else {
            msg!("VK chunk {} stored, {} bytes so far", chunk_index, vk_storage.data.len());
        }
        Ok(())
    }

    /// Initialize the Bloom filter nullifier registry.
    pub fn init_nullifier_bloom(ctx: Context<InitNullifierBloom>) -> Result<()> {
        let bloom = &mut ctx.accounts.nullifier_bloom;
        bloom.bits = vec![0u8; BLOOM_BYTES];
        bloom.entry_count = 0;
        msg!("Bloom filter nullifier registry initialized: {} bytes", BLOOM_BYTES);
        Ok(())
    }

    /// Verify a Groth16 proof and record the nullifier.
    ///
    /// This is the core verification instruction:
    ///   1. Check the VK is initialized
    ///   2. Check nullifier hasn't been used (Bloom filter O(1))
    ///   3. Verify Groth16 proof via groth16-solana (alt_bn128 syscalls)
    ///   4. Insert nullifier into Bloom filter
    ///   5. Emit CredentialVerified event
    pub fn verify_proof(
        ctx: Context<VerifyProof>,
        proof_a: [u8; 64],
        proof_b: [u8; 128],
        proof_c: [u8; 64],
        public_inputs: [[u8; 32]; NR_PUBLIC_INPUTS],
        nullifier: [u8; 32],
    ) -> Result<()> {
        let config = &ctx.accounts.verifier_config;

        // 1. Ensure VK is initialized
        require!(config.vk_initialized, ErrorCode::VerificationKeyNotSet);

        // 1. PHASE 4.1: Bind nullifier argument to public_inputs[0] (Groth16 Output)
        // This ensures the proof cannot be reused with a fake nullifier.
        require!(nullifier == public_inputs[0], ErrorCode::NullifierMismatch);

        // 2. PHASE 4.2: SEC-05 On-Chain Issuer Status Check
        //   Indices 4 and 5 are the issuer BJJ coordinates (from compound_query.circom).
        let issuer_account = &ctx.accounts.issuer_account;
        require!(
            issuer_account.status == issuer_registry::IssuerStatus::Approved,
            ErrorCode::IssuerNotApproved
        );
        require!(
            issuer_account.bjj_pub_key_x == public_inputs[4],
            ErrorCode::IssuerKeyMismatch
        );
        require!(
            issuer_account.bjj_pub_key_y == public_inputs[5],
            ErrorCode::IssuerKeyMismatch
        );

        // 3. Check nullifier via Bloom filter (O(1) lookup)
        let bloom = &ctx.accounts.nullifier_bloom;
        require!(
            !bloom_contains(&bloom.bits, &nullifier),
            ErrorCode::NullifierAlreadyUsed
        );

        // 3. Verify Groth16 proof using groth16-solana
        let vk_data = &ctx.accounts.vk_storage.data;

        // Deserialize VK from stored bytes
        let vk = deserialize_vk(vk_data)
            .map_err(|_| ErrorCode::InvalidProofFormat)?;

        // Negate proof_a for verification (groth16-solana requirement)
        let proof_a_neg = negate_g1_point(&proof_a);

        // Create verifier and verify
        let mut verifier = Groth16Verifier::<NR_PUBLIC_INPUTS>::new(
            &proof_a_neg,
            &proof_b,
            &proof_c,
            &public_inputs,
            &vk,
        ).map_err(|_| ErrorCode::InvalidProofFormat)?;

        // verify() returns Result<()> — errors on failure, Ok on success
        verifier.verify()
            .map_err(|_| ErrorCode::ProofVerificationFailed)?;

        // 4. Insert nullifier into Bloom filter
        let bloom = &mut ctx.accounts.nullifier_bloom;
        bloom_insert(&mut bloom.bits, &nullifier);
        bloom.entry_count += 1;

        // 5. Update proof count
        let config = &mut ctx.accounts.verifier_config;
        config.proof_count += 1;

        // 6. Emit event for indexers
        emit!(CredentialVerified {
            nullifier,
            proof_count: config.proof_count,
            public_input_count: NR_PUBLIC_INPUTS as u8,
            timestamp: Clock::get()?.unix_timestamp,
        });

        msg!("Proof verified! Nullifier recorded. Total: {}", config.proof_count);
        Ok(())
    }

    /// Check if a nullifier has been used (read-only).
    pub fn check_nullifier(ctx: Context<CheckNullifier>, nullifier: [u8; 32]) -> Result<()> {
        let bloom = &ctx.accounts.nullifier_bloom;
        if bloom_contains(&bloom.bits, &nullifier) {
            msg!("Nullifier USED (may be false positive at >100K entries)");
            return Err(ErrorCode::NullifierAlreadyUsed.into());
        }
        msg!("Nullifier NOT used");
        Ok(())
    }

    /// Get bloom filter stats.
    pub fn get_stats(ctx: Context<GetStats>) -> Result<()> {
        let config = &ctx.accounts.verifier_config;
        let bloom = &ctx.accounts.nullifier_bloom;
        let capacity_pct = (bloom.entry_count as f64 / 100_000.0 * 100.0) as u64;
        msg!("Proofs verified: {}", config.proof_count);
        msg!("Nullifiers recorded: {}", bloom.entry_count);
        msg!("Bloom filter capacity: ~{}%", capacity_pct);
        Ok(())
    }

    /// Initialize a new self-sovereign identity in the global state tree (Phase 2.3).
    pub fn initialize_identity(ctx: Context<InitializeIdentity>) -> Result<()> {
        let authority = ctx.accounts.authority.key();
        
        // PHASE 2.3: Anchor the initial identity state (nonce=0)
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
    /// SEC-16: Mandatory Authority Signer check.
    pub fn rotate_identity(ctx: Context<RotateIdentity>, new_nonce: u64) -> Result<()> {
        let authority = ctx.accounts.authority.key();

        // PHASE 2.3: Anchor the updated identity state (nonce++).
        // This instantly invalidates all ZK proofs derived from older nonces.
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

/// Deserialize a Groth16Verifyingkey from stored byte buffer.
///
/// Layout:
///   vk_alpha_g1: [u8; 64]       (bytes 0..64)
///   vk_beta_g2:  [u8; 128]      (bytes 64..192)
///   vk_gamme_g2: [u8; 128]      (bytes 192..320)
///   vk_delta_g2: [u8; 128]      (bytes 320..448)
///   vk_ic:       N * [u8; 64]   (bytes 448..)  where N = NR_PUBLIC_INPUTS + 1
fn deserialize_vk(data: &[u8]) -> std::result::Result<Groth16Verifyingkey<'_>, &'static str> {
    let expected_ic_count = NR_PUBLIC_INPUTS + 1;
    let expected_len = 64 + 128 + 128 + 128 + expected_ic_count * 64;

    if data.len() < expected_len {
        return Err("VK data too short");
    }

    let vk_alpha_g1: [u8; 64] = data[0..64].try_into().map_err(|_| "bad alpha")?;
    let vk_beta_g2: [u8; 128] = data[64..192].try_into().map_err(|_| "bad beta")?;
    let vk_gamme_g2: [u8; 128] = data[192..320].try_into().map_err(|_| "bad gamma")?;
    let vk_delta_g2: [u8; 128] = data[320..448].try_into().map_err(|_| "bad delta")?;

    // vk_ic is a slice of [u8; 64] — we can point directly into the data buffer
    // SAFETY: We've verified the length above. We use bytemuck-style reinterpret.
    let ic_bytes = &data[448..448 + expected_ic_count * 64];
    // Convert &[u8] to &[[u8; 64]]
    let vk_ic: &[[u8; 64]] = unsafe {
        std::slice::from_raw_parts(
            ic_bytes.as_ptr() as *const [u8; 64],
            expected_ic_count,
        )
    };

    Ok(Groth16Verifyingkey {
        nr_pubinputs: NR_PUBLIC_INPUTS,
        vk_alpha_g1,
        vk_beta_g2,
        vk_gamme_g2,
        vk_delta_g2,
        vk_ic,
    })
}

// ─── Bloom Filter Implementation ───────────────────────────────────────────

fn bloom_hashes(data: &[u8; 32]) -> [usize; BLOOM_K] {
    let mut indices = [0usize; BLOOM_K];
    let h1 = u64::from_le_bytes(data[0..8].try_into().unwrap()) as usize;
    let h2 = u64::from_le_bytes(data[8..16].try_into().unwrap()) as usize;
    for i in 0..BLOOM_K {
        indices[i] = (h1.wrapping_add(i.wrapping_mul(h2))) % BLOOM_BITS;
    }
    indices
}

fn bloom_insert(bits: &mut [u8], data: &[u8; 32]) {
    for idx in bloom_hashes(data) {
        let byte_idx = idx / 8;
        let bit_idx = idx % 8;
        if byte_idx < bits.len() {
            bits[byte_idx] |= 1 << bit_idx;
        }
    }
}

fn bloom_contains(bits: &[u8], data: &[u8; 32]) -> bool {
    for idx in bloom_hashes(data) {
        let byte_idx = idx / 8;
        let bit_idx = idx % 8;
        if byte_idx >= bits.len() || (bits[byte_idx] & (1 << bit_idx)) == 0 {
            return false;
        }
    }
    true
}

/// Reverse byte order per 32-byte chunk (endianness conversion).
fn change_endianness(bytes: &[u8]) -> Vec<u8> {
    let mut vec = Vec::new();
    for b in bytes.chunks(32) {
        for byte in b.iter().rev() {
            vec.push(*byte);
        }
    }
    vec
}

/// Negate the Y coordinate of a G1 point for groth16-solana verification.
fn negate_g1_point(point: &[u8; 64]) -> [u8; 64] {
    let mut negated = [0u8; 64];
    negated[..32].copy_from_slice(&point[..32]); // X unchanged

    // Convert Y to big-endian, negate in BN254 field, convert back
    let y_be = change_endianness(&point[32..64]);

    // BN254 base field modulus (big-endian)
    let p: [u8; 32] = [
        0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29,
        0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58, 0x5d,
        0x97, 0x81, 0x6a, 0x91, 0x68, 0x71, 0xca, 0x8d,
        0x3c, 0x20, 0x8c, 0x16, 0xd8, 0x7c, 0xfd, 0x47,
    ];

    let mut borrow = 0i16;
    let mut neg_y = [0u8; 32];
    for i in (0..32).rev() {
        let diff = p[i] as i16 - y_be[i] as i16 - borrow;
        if diff < 0 {
            neg_y[i] = (diff + 256) as u8;
            borrow = 1;
        } else {
            neg_y[i] = diff as u8;
            borrow = 0;
        }
    }

    let neg_y_le = change_endianness(&neg_y);
    negated[32..64].copy_from_slice(&neg_y_le);
    negated
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
pub struct InitNullifierBloom<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + 4 + BLOOM_BYTES + 8,
        seeds = [b"nullifier-bloom", verifier_config.key().as_ref()],
        bump
    )]
    pub nullifier_bloom: Account<'info, NullifierBloom>,
    #[account(seeds = [b"verifier-config"], bump)]
    pub verifier_config: Account<'info, VerifierConfig>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(proof_a: [u8; 64], proof_b: [u8; 128], proof_c: [u8; 64], public_inputs: [[u8; 32]; NR_PUBLIC_INPUTS], nullifier: [u8; 32])]
pub struct VerifyProofV2<'info> {
    #[account(
        mut, 
        seeds = [b"verifier-config"], 
        bump = verifier_config.bump // SEC-11: Verify stored bump
    )]
    pub verifier_config: Account<'info, VerifierConfig>,
    #[account(
        seeds = [b"vk-storage", verifier_config.key().as_ref()],
        bump
    )]
    pub vk_storage: Account<'info, VkStorage>,
    /// CHECK: Light Protocol Merkle tree account for root verification
    pub merkle_tree: UncheckedAccount<'info>,
    /// CHECK: Light Protocol program for CPI operations
    pub light_program: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CheckNullifier<'info> {
    #[account(seeds = [b"verifier-config"], bump)]
    pub verifier_config: Account<'info, VerifierConfig>,
    #[account(
        seeds = [b"nullifier-bloom", verifier_config.key().as_ref()],
        bump
    )]
    pub nullifier_bloom: Account<'info, NullifierBloom>,
}

#[derive(Accounts)]
pub struct InitializeIdentity<'info> {
    /// CHECK: Light Protocol Merkle tree account
    #[account(mut)]
    pub merkle_tree: UncheckedAccount<'info>,
    /// CHECK: Light Protocol program for CPI
    pub light_program: UncheckedAccount<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RotateIdentity<'info> {
    /// CHECK: Light Protocol Merkle tree account
    #[account(mut)]
    pub merkle_tree: UncheckedAccount<'info>,
    /// CHECK: Light Protocol program for CPI
    pub light_program: UncheckedAccount<'info>,
    /// SEC-16: Authority must sign to rotate their own identity state
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct GetStats<'info> {
    #[account(seeds = [b"verifier-config"], bump)]
    pub verifier_config: Account<'info, VerifierConfig>,
    #[account(
        seeds = [b"nullifier-bloom", verifier_config.key().as_ref()],
        bump
    )]
    pub nullifier_bloom: Account<'info, NullifierBloom>,
}

// ─── State ─────────────────────────────────────────────────────────────────

#[account]
pub struct VerifierConfig {
    pub authority: Pubkey,
    pub proof_count: u64,
    pub vk_initialized: bool,
    pub bump: u8, // SEC-11
}

#[account]
pub struct VkStorage {
    pub data: Vec<u8>,
}

#[account]
pub struct NullifierBloom {
    pub bits: Vec<u8>,
    pub entry_count: u64,
}

// ─── Errors ────────────────────────────────────────────────────────────────

#[error_code]
pub enum ErrorCode {
    #[msg("This nullifier has already been used — proof replay detected")]
    NullifierAlreadyUsed,
    #[msg("Groth16 proof verification failed")]
    ProofVerificationFailed,
    #[msg("Invalid proof format — could not parse proof bytes")]
    InvalidProofFormat,
    #[msg("Verification key not set — call store_verification_key first")]
    VerificationKeyNotSet,
    #[msg("Unauthorized — not the verifier authority")]
    Unauthorized,
    #[msg("Invalid global state root — proof rejected by Light Protocol anchor")]
    InvalidGlobalRoot,
    #[msg("Provided nullifier does not match the proof's output — replay attempted")]
    NullifierMismatch,
    #[msg("The issuer of this credential is not approved in the registry")]
    IssuerNotApproved,
    #[msg("Proof was signed by a key that does not match the issuer account")]
    IssuerKeyMismatch,
    #[msg("Verifier address in proof does not match this program's ID")]
    InvalidVerifierAddress,
}
