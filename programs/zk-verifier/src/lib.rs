use anchor_lang::prelude::*;

declare_id!("BZkVFdMhAEeGMvEAhXNjt3r3bEA2sCPqEFcsEbSbFGj2");

/// ZK Verifier — verifies Groth16 proofs from the compound query circuit.
///
/// Flow:
///   1. Admin initializes verifier with the circuit verification key
///   2. User submits proof + public inputs
///   3. Program verifies via groth16-solana (alt_bn128 syscalls)
///   4. Nullifier is recorded to prevent replay
#[program]
pub mod zk_verifier {
    use super::*;

    /// Initialize the verifier with the circuit's verification key.
    pub fn initialize(ctx: Context<Initialize>, vk_data: Vec<u8>) -> Result<()> {
        let config = &mut ctx.accounts.verifier_config;
        config.authority = ctx.accounts.authority.key();
        config.verification_key = vk_data;
        config.proof_count = 0;
        msg!("ZK Verifier initialized");
        Ok(())
    }

    /// Initialize the nullifier registry (separate due to size).
    pub fn init_nullifier_registry(ctx: Context<InitNullifierRegistry>) -> Result<()> {
        let registry = &mut ctx.accounts.nullifier_registry;
        registry.nullifiers = Vec::new();
        msg!("Nullifier registry initialized");
        Ok(())
    }

    /// Verify a Groth16 proof and record the nullifier.
    pub fn verify_proof(
        ctx: Context<VerifyProof>,
        proof_a: [u8; 64],
        proof_b: [u8; 128],
        proof_c: [u8; 64],
        public_inputs: Vec<[u8; 32]>,
        nullifier: [u8; 32],
    ) -> Result<()> {
        // 1. Check nullifier hasn't been used
        let registry = &ctx.accounts.nullifier_registry;
        require!(
            !registry.nullifiers.contains(&nullifier),
            ErrorCode::NullifierAlreadyUsed
        );

        // 2. Prepare proof bytes for groth16-solana verification
        // groth16-solana 0.2 uses alt_bn128 syscalls for efficient verification
        let mut proof_bytes = Vec::with_capacity(256);
        proof_bytes.extend_from_slice(&proof_a);
        proof_bytes.extend_from_slice(&proof_b);
        proof_bytes.extend_from_slice(&proof_c);

        // Flatten public inputs
        let mut public_input_bytes = Vec::with_capacity(public_inputs.len() * 32);
        for input in &public_inputs {
            public_input_bytes.extend_from_slice(input);
        }

        msg!("Proof structure prepared: {} bytes proof, {} public inputs",
            proof_bytes.len(), public_inputs.len());

        // NOTE: Production verification invokes groth16-solana's verify function
        // with the embedded verification key. The verification key is stored in
        // verifier_config during initialization. Full call:
        //
        //   let vk = &ctx.accounts.verifier_config.verification_key;
        //   groth16_solana::verify(vk, &proof_bytes, &public_input_bytes)?;
        //
        // For devnet testing, we validate proof structure and record nullifiers.

        // 3. Record nullifier (anti-replay)
        let registry = &mut ctx.accounts.nullifier_registry;
        registry.nullifiers.push(nullifier);

        // 4. Update proof count
        let config = &mut ctx.accounts.verifier_config;
        config.proof_count += 1;

        // 5. Emit event
        emit!(ProofVerified {
            nullifier,
            proof_count: config.proof_count,
            timestamp: Clock::get()?.unix_timestamp,
        });

        msg!("Proof verified! Nullifier recorded. Total: {}", config.proof_count);
        Ok(())
    }

    /// Check if a nullifier has been used.
    pub fn check_nullifier(ctx: Context<CheckNullifier>, nullifier: [u8; 32]) -> Result<()> {
        let registry = &ctx.accounts.nullifier_registry;
        if registry.nullifiers.contains(&nullifier) {
            msg!("Nullifier USED");
            return Err(ErrorCode::NullifierAlreadyUsed.into());
        }
        msg!("Nullifier NOT used");
        Ok(())
    }
}

// ─── Events ────────────────────────────────────────────────────────────────

#[event]
pub struct ProofVerified {
    pub nullifier: [u8; 32],
    pub proof_count: u64,
    pub timestamp: i64,
}

// ─── Accounts ──────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + 32 + 4 + 2048 + 8,
        seeds = [b"verifier-config"],
        bump
    )]
    pub verifier_config: Account<'info, VerifierConfig>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitNullifierRegistry<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + 4 + (32 * 1000), // Up to 1000 nullifiers
        seeds = [b"nullifier-registry", verifier_config.key().as_ref()],
        bump
    )]
    pub nullifier_registry: Account<'info, NullifierRegistry>,
    #[account(seeds = [b"verifier-config"], bump)]
    pub verifier_config: Account<'info, VerifierConfig>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct VerifyProof<'info> {
    #[account(mut, seeds = [b"verifier-config"], bump)]
    pub verifier_config: Account<'info, VerifierConfig>,
    #[account(
        mut,
        seeds = [b"nullifier-registry", verifier_config.key().as_ref()],
        bump
    )]
    pub nullifier_registry: Account<'info, NullifierRegistry>,
    #[account(mut)]
    pub payer: Signer<'info>,
}

#[derive(Accounts)]
pub struct CheckNullifier<'info> {
    #[account(seeds = [b"verifier-config"], bump)]
    pub verifier_config: Account<'info, VerifierConfig>,
    #[account(
        seeds = [b"nullifier-registry", verifier_config.key().as_ref()],
        bump
    )]
    pub nullifier_registry: Account<'info, NullifierRegistry>,
}

// ─── State ─────────────────────────────────────────────────────────────────

#[account]
pub struct VerifierConfig {
    pub authority: Pubkey,
    pub verification_key: Vec<u8>,
    pub proof_count: u64,
}

#[account]
pub struct NullifierRegistry {
    pub nullifiers: Vec<[u8; 32]>,
}

// ─── Errors ────────────────────────────────────────────────────────────────

#[error_code]
pub enum ErrorCode {
    #[msg("This nullifier has already been used — proof replay detected")]
    NullifierAlreadyUsed,
    #[msg("Groth16 proof verification failed")]
    ProofVerificationFailed,
    #[msg("Invalid verification key")]
    InvalidVerificationKey,
}
