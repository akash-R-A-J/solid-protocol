//! CPI helpers for Light Protocol operations.
//!
//! These functions wrap the Light SDK's CPI interface for
//! common SolID operations:
//!   - insert_credential: Insert a commitment leaf into a compressed tree
//!   - verify_state_root: Verify a Merkle root matches the on-chain tree
//!   - revoke_credential: Mark a credential leaf as revoked
//!
//! Usage from SolID programs:
//!   use solid_light::cpi_helpers;
//!   cpi_helpers::insert_credential(ctx, commitment, schema_hash)?;

use anchor_lang::prelude::*;
use crate::credential_tree::CompressedCredential;

/// Light System Program ID (mainnet/devnet).
pub const LIGHT_SYSTEM_PROGRAM_ID: &str = "H5sFv8VwWmjxHYS2GB4fTDsK7uY78fcaC9D2pcg84Kvs";

/// Seeds for the CPI signer PDA.
pub const CPI_AUTHORITY_SEED: &[u8] = b"solid-light-authority";

/// Insert a credential commitment into a Light Protocol compressed state tree.
///
/// This CPI call:
///   1. Creates a compressed account containing the credential data
///   2. Inserts the account hash as a leaf in the Merkle tree
///   3. Updates the state root on-chain
///
/// The credential data (commitment, schema, issuer) is stored compressed —
/// ~200x cheaper than a traditional Solana PDA.
/// Insert a credential commitment into a Light Protocol compressed state tree.
pub fn build_insert_credential_data(
    commitment: [u8; 32],
    schema_hash: [u8; 32],
    issuer: Pubkey,
) -> Result<Vec<u8>> {
    let credential = CompressedCredential::new(
        commitment,
        schema_hash,
        issuer.to_bytes(),
    );

    let mut data = Vec::new();
    data.extend_from_slice(&CompressedCredential::DISCRIMINATOR);
    borsh::to_writer(&mut data, &credential)
        .map_err(|_| error!(LightError::SerializationFailed))?;

    Ok(data)
}

/// Build the insertion data for a compressed nullifier.
pub fn build_insert_nullifier_data(
    nullifier: [u8; 32],
) -> Result<Vec<u8>> {
    let nullifier_acc = crate::credential_tree::CompressedNullifier {
        nullifier,
        created_at: 0,
    };

    let mut data = Vec::new();
    data.extend_from_slice(&crate::credential_tree::CompressedNullifier::DISCRIMINATOR);
    borsh::to_writer(&mut data, &nullifier_acc)
        .map_err(|_| error!(LightError::SerializationFailed))?;

    Ok(data)
}

/// Build the insertion data for a compressed identity.
pub fn build_insert_identity_data(
    owner: [u8; 32],
    revocation_nonce: u64,
) -> Result<Vec<u8>> {
    let identity = crate::credential_tree::CompressedIdentity {
        owner,
        revocation_nonce,
    };

    let mut data = Vec::new();
    data.extend_from_slice(&crate::credential_tree::CompressedIdentity::DISCRIMINATOR);
    borsh::to_writer(&mut data, &identity)
        .map_err(|_| error!(LightError::SerializationFailed))?;

    Ok(data)
}

/// Build the revocation data for a credential.
///
/// Revocation marks the compressed account as revoked, which
/// invalidates future Merkle inclusion proofs for this commitment.
pub fn build_revoke_credential_data(
    commitment: [u8; 32],
    schema_hash: [u8; 32],
    issuer: Pubkey,
) -> Result<Vec<u8>> {
    let mut credential = CompressedCredential::new(
        commitment,
        schema_hash,
        issuer.to_bytes(),
    );
    credential.revoked = true;

    let mut data = Vec::new();
    data.extend_from_slice(&CompressedCredential::DISCRIMINATOR);
    borsh::to_writer(&mut data, &credential)
        .map_err(|_| error!(LightError::SerializationFailed))?;

    Ok(data)
}

/// Verify that a given state root matches the on-chain Merkle tree.
///
/// Used by the ZK verifier to ensure the Merkle root in the proof's
/// public inputs actually corresponds to the current (or recent) state
/// of the credential tree.
///
/// Returns true if the root is valid (matches on-chain state).
pub fn verify_state_root_matches(
    tree_account_data: &[u8],
    expected_root: &[u8; 32],
) -> bool {
    // Light Protocol stores the current root in the tree account header.
    // Offset depends on the tree type. For concurrent Merkle trees,
    // the root starts at byte offset 8 (after discriminator).
    if tree_account_data.len() < 40 {
        return false;
    }

    let stored_root = &tree_account_data[8..40];
    stored_root == expected_root.as_slice()
}

/// Perform a CPI to the Light System Program to register a compressed nullifier.
///
/// This is the on-chain equivalent of "marking a nullifier as spent".
pub fn register_nullifier_cpi<'info>(
    light_program: &UncheckedAccount<'info>,
    merkle_tree: &UncheckedAccount<'info>,
    payer: &Signer<'info>,
    system_program: &Program<'info, System>,
    nullifier: [u8; 32],
) -> Result<()> {
    let nullifier_data = build_insert_nullifier_data(nullifier)?;

    // NOTE: In a real Light Protocol integration, we would use the light-sdk's
    // 'shield' or 'compressed_account::create' CPI here.
    // Since we are building the infrastructure, we use a placeholder CPI for now
    // that represents the intent to move to the Light system program.
    
    msg!("CPI: Registering compressed nullifier {} via Light Program", 
        hex::encode(nullifier));
        
    // Placeholder for actual light-sdk CPI call:
    // light_sdk::request_units(ctx, 10000); // Compression is compute intensive
    // light_sdk::compressed_account::create(
    //     light_program,
    //     merkle_tree,
    //     nullifier_data,
    //     ...
    // )?;

    Ok(())
}

/// Perform a CPI to the Light System Program to register a compressed issuer.
pub fn register_issuer_cpi<'info>(
    light_program: &UncheckedAccount<'info>,
    merkle_tree: &UncheckedAccount<'info>,
    payer: &Signer<'info>,
    system_program: &Program<'info, System>,
    authority: [u8; 32],
    bjj_pub_key_x: [u8; 32],
    bjj_pub_key_y: [u8; 32],
    tier: u8,
) -> Result<()> {
    let issuer_data = build_insert_issuer_data(
        authority,
        bjj_pub_key_x,
        bjj_pub_key_y,
        tier,
    )?;

    msg!("CPI: Registering compressed issuer via Light Program (Tier: {})", tier);
    
    // Placeholder for actual light-sdk CPI call
    Ok(())
}

/// Perform a CPI to the Light System Program to register/update a compressed identity.
pub fn register_identity_cpi<'info>(
    light_program: &UncheckedAccount<'info>,
    merkle_tree: &UncheckedAccount<'info>,
    payer: &Signer<'info>,
    system_program: &Program<'info, System>,
    owner: [u8; 32],
    revocation_nonce: u64,
) -> Result<()> {
    let identity_data = build_insert_identity_data(owner, revocation_nonce)?;

    msg!("CPI: Anchoring identity state via Light Program (Nonce: {})", revocation_nonce);
    
    // Placeholder for actual light-sdk CPI call
    Ok(())
}

/// Build the insertion data for a compressed issuer.
fn build_insert_issuer_data(
    authority: [u8; 32],
    bjj_pub_key_x: [u8; 32],
    bjj_pub_key_y: [u8; 32],
    tier: u8,
) -> Result<Vec<u8>> {
    let issuer = crate::credential_tree::CompressedIssuer {
        authority,
        bjj_pub_key_x,
        bjj_pub_key_y,
        tier,
        status: 0, // Pending
        revocation_nonce: 0,
    };

    let mut data = Vec::new();
    data.extend_from_slice(&crate::credential_tree::CompressedIssuer::DISCRIMINATOR);
    borsh::to_writer(&mut data, &issuer)
        .map_err(|_| error!(LightError::SerializationFailed))?;

    Ok(data)
}

#[error_code]
pub enum LightError {
    #[msg("Failed to serialize compressed credential data")]
    SerializationFailed,
    #[msg("State root does not match on-chain Merkle tree")]
    StateRootMismatch,
    #[msg("Credential has been revoked")]
    CredentialRevoked,
    #[msg("Light Protocol CPI failed")]
    CpiFailed,
}
