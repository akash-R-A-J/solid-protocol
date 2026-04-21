//! Data-layer helpers used by SolID on-chain programs.
//!
//! Design note (2026-04 remediation):
//!
//! The previous version of this file referenced `light_sdk::cpi::compressed_account_create`,
//! which does not exist in `light-sdk = "0.4.0"` (it was a placeholder). That
//! created a latent panic at deploy-time and a false sense of security.
//!
//! The production SolID protocol treats Light Protocol as one **of several**
//! backends for state storage. The on-chain verification path only needs two
//! things from the storage backend:
//!
//!   1. A way to read the *current* Merkle root the tree account advertises
//!      (`verify_state_root_matches`).
//!   2. A way to confirm that a specific `(schema_hash, merkle_root)` pair is
//!      the canonical pair registered by `schema-registry`
//!      (`verify_schema_root_binding`).
//!
//! Both functions are pure — they parse account data — so they work for
//! Light Protocol, for a SolID-native PDA tree, or for any other account shape
//! that follows the documented layout. The insertion path (publishing a new
//! credential into the tree) is handled off-chain by the holder SDK + indexer
//! against the specific storage backend chosen at deploy time, then surfaced
//! on-chain by `schema-registry::update_tree_root`.
//!
//! The account types defined in `credential_tree.rs` remain useful as the
//! canonical shape of the off-chain records.

use anchor_lang::prelude::*;

use crate::credential_tree::{
    CompressedCredential, CompressedIdentity, CompressedIssuer, CompressedNullifier,
};

/// Hard-coded program IDs the verifier trusts for schema-tree metadata. The
/// on-chain caller must still check that the supplied `schema_tree_N` account
/// is owned by this program; this constant is purely documentation.
pub const SCHEMA_REGISTRY_PROGRAM_ID: &str = "DPk6XUH6CArLWt4KMqJmpNBnPwQ3gG9P3dBd3MDVE3bT";

/// Typed counterpart of [`SCHEMA_REGISTRY_PROGRAM_ID`].
///
/// Callers use `SCHEMA_REGISTRY_ID` for `require_keys_eq!` checks against
/// `account.owner`. The zk-verifier must refuse any `global_tree` or
/// `schema_tree_N` whose owner is not this program: without the check an
/// attacker can craft a system-owned account with a valid `globroot` /
/// `schmtree` discriminator and forge trust-root bindings.
pub const SCHEMA_REGISTRY_ID: Pubkey =
    anchor_lang::prelude::Pubkey::new_from_array(SCHEMA_REGISTRY_ID_BYTES);

/// Raw bytes of `SCHEMA_REGISTRY_ID`, base58-decoded at the source location
/// `"DPk6XUH6CArLWt4KMqJmpNBnPwQ3gG9P3dBd3MDVE3bT"`. Kept as a separate const
/// so the program-ID literal above is usable in `const` contexts without the
/// `pubkey!` proc-macro, which is not re-exported from `anchor_lang::prelude`.
const SCHEMA_REGISTRY_ID_BYTES: [u8; 32] = [
    184, 31, 191, 183, 14, 126, 178, 219,
    191, 193, 249, 206, 232, 77, 185, 224,
    56, 51, 91, 209, 33, 205, 175, 183,
    155, 9, 46, 66, 147, 25, 1, 94,
];

// ─── Account-data builders (off-chain use) ─────────────────────────────────

/// Build the Borsh-encoded bytes for a new `CompressedCredential` leaf.
///
/// Consumed by off-chain tooling to deliver the leaf to whichever storage
/// backend the protocol is running against.
pub fn build_insert_credential_data(
    commitment: [u8; 32],
    schema_hash: [u8; 32],
    issuer: Pubkey,
) -> Result<Vec<u8>> {
    let credential = CompressedCredential::new(commitment, schema_hash, issuer.to_bytes());
    let mut data = Vec::with_capacity(CompressedCredential::size());
    data.extend_from_slice(&CompressedCredential::DISCRIMINATOR);
    borsh::to_writer(&mut data, &credential)
        .map_err(|_| error!(LightError::SerializationFailed))?;
    Ok(data)
}

/// Build the Borsh-encoded bytes for a `CompressedNullifier` leaf.
pub fn build_insert_nullifier_data(nullifier: [u8; 32]) -> Result<Vec<u8>> {
    let nullifier_acc = CompressedNullifier {
        nullifier,
        created_at: 0,
    };
    let mut data = Vec::with_capacity(CompressedNullifier::size());
    data.extend_from_slice(&CompressedNullifier::DISCRIMINATOR);
    borsh::to_writer(&mut data, &nullifier_acc)
        .map_err(|_| error!(LightError::SerializationFailed))?;
    Ok(data)
}

/// Build the Borsh-encoded bytes for a `CompressedIdentity` leaf.
pub fn build_insert_identity_data(owner: [u8; 32], revocation_nonce: u64) -> Result<Vec<u8>> {
    let identity = CompressedIdentity {
        owner,
        revocation_nonce,
    };
    let mut data = Vec::with_capacity(CompressedIdentity::size());
    data.extend_from_slice(&CompressedIdentity::DISCRIMINATOR);
    borsh::to_writer(&mut data, &identity)
        .map_err(|_| error!(LightError::SerializationFailed))?;
    Ok(data)
}

/// Build the Borsh-encoded bytes for a `CompressedIssuer` leaf.
pub fn build_insert_issuer_data(
    authority: [u8; 32],
    bjj_pub_key_x: [u8; 32],
    bjj_pub_key_y: [u8; 32],
    tier: u8,
) -> Result<Vec<u8>> {
    let issuer = CompressedIssuer {
        authority,
        bjj_pub_key_x,
        bjj_pub_key_y,
        tier,
        status: 0,
        revocation_nonce: 0,
    };
    let mut data = Vec::with_capacity(CompressedIssuer::size());
    data.extend_from_slice(&CompressedIssuer::DISCRIMINATOR);
    borsh::to_writer(&mut data, &issuer)
        .map_err(|_| error!(LightError::SerializationFailed))?;
    Ok(data)
}

/// Build the Borsh-encoded bytes for a revoked `CompressedCredential` leaf.
pub fn build_revoke_credential_data(
    commitment: [u8; 32],
    schema_hash: [u8; 32],
    issuer: Pubkey,
) -> Result<Vec<u8>> {
    let mut credential = CompressedCredential::new(commitment, schema_hash, issuer.to_bytes());
    credential.revoked = true;
    let mut data = Vec::with_capacity(CompressedCredential::size());
    data.extend_from_slice(&CompressedCredential::DISCRIMINATOR);
    borsh::to_writer(&mut data, &credential)
        .map_err(|_| error!(LightError::SerializationFailed))?;
    Ok(data)
}

// ─── On-chain parse helpers ────────────────────────────────────────────────

/// Layout of the `SchemaTreeBinding` PDA written by `schema-registry`.
///
/// ```text
/// offset  0..8     : Anchor discriminator ("schmtree")
/// offset  8..40    : schema_hash          (32 bytes LE)
/// offset 40..72    : tree_pubkey          (32 bytes)
/// offset 72..104   : current_root         (32 bytes LE)
/// offset 104..112  : last_updated_slot    (u64 LE)
/// offset 112..113  : status               (u8; 0 = active, 1 = frozen)
/// ```
pub const SCHEMA_TREE_DISCRIMINATOR: [u8; 8] = *b"schmtree";

/// Validate that the given PDA account is an *active* schema-tree binding whose
/// `schema_hash` equals `expected_schema` and whose `current_root` equals
/// `expected_root`.
///
/// Returns `true` only if every field matches and the status byte is `0`.
/// All parsing is bounds-checked; malformed accounts return `false` instead
/// of panicking.
pub fn verify_schema_root_binding(
    tree_account_data: &[u8],
    expected_root: &[u8; 32],
    expected_schema: &[u8; 32],
) -> bool {
    if tree_account_data.len() < 113 {
        return false;
    }
    // Discriminator gate
    if tree_account_data[..8] != SCHEMA_TREE_DISCRIMINATOR {
        return false;
    }
    // Schema match
    if &tree_account_data[8..40] != expected_schema.as_slice() {
        return false;
    }
    // Root match (offset 72..104)
    if &tree_account_data[72..104] != expected_root.as_slice() {
        return false;
    }
    // Status must be "active"
    tree_account_data[112] == 0
}

/// Validate that the given account is a `GlobalStateBinding` PDA whose current
/// root matches the expected value.
///
/// Layout:
/// ```text
/// offset  0..8    : Anchor discriminator ("globroot")
/// offset  8..40   : current_root         (32 bytes LE)
/// offset 40..48   : last_updated_slot    (u64 LE)
/// ```
pub const GLOBAL_ROOT_DISCRIMINATOR: [u8; 8] = *b"globroot";

pub fn verify_state_root_matches(tree_account_data: &[u8], expected_root: &[u8; 32]) -> bool {
    if tree_account_data.len() < 40 {
        return false;
    }
    if tree_account_data[..8] != GLOBAL_ROOT_DISCRIMINATOR {
        return false;
    }
    &tree_account_data[8..40] == expected_root.as_slice()
}

// ─── Errors ────────────────────────────────────────────────────────────────

#[error_code]
pub enum LightError {
    #[msg("Failed to serialize compressed account data")]
    SerializationFailed,
    #[msg("State root does not match on-chain Merkle tree")]
    StateRootMismatch,
    #[msg("Credential has been revoked")]
    CredentialRevoked,
    #[msg("Account data does not match the expected schema binding")]
    InvalidSchemaBinding,
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_schema_account(schema: [u8; 32], tree_pk: [u8; 32], root: [u8; 32], status: u8) -> Vec<u8> {
        let mut v = Vec::with_capacity(113);
        v.extend_from_slice(&SCHEMA_TREE_DISCRIMINATOR);
        v.extend_from_slice(&schema);
        v.extend_from_slice(&tree_pk);
        v.extend_from_slice(&root);
        v.extend_from_slice(&0u64.to_le_bytes());
        v.push(status);
        v
    }

    fn make_global_root_account(root: [u8; 32]) -> Vec<u8> {
        let mut v = Vec::with_capacity(48);
        v.extend_from_slice(&GLOBAL_ROOT_DISCRIMINATOR);
        v.extend_from_slice(&root);
        v.extend_from_slice(&0u64.to_le_bytes());
        v
    }

    #[test]
    fn root_binding_passes_happy_path() {
        let schema = [7u8; 32];
        let root = [9u8; 32];
        let tree_pk = [3u8; 32];
        let data = make_schema_account(schema, tree_pk, root, 0);
        assert!(verify_schema_root_binding(&data, &root, &schema));
    }

    #[test]
    fn root_binding_rejects_wrong_schema() {
        let schema = [7u8; 32];
        let wrong = [8u8; 32];
        let root = [9u8; 32];
        let data = make_schema_account(schema, [0u8; 32], root, 0);
        assert!(!verify_schema_root_binding(&data, &root, &wrong));
    }

    #[test]
    fn root_binding_rejects_wrong_root() {
        let schema = [7u8; 32];
        let root = [9u8; 32];
        let wrong = [10u8; 32];
        let data = make_schema_account(schema, [0u8; 32], root, 0);
        assert!(!verify_schema_root_binding(&data, &wrong, &schema));
    }

    #[test]
    fn root_binding_rejects_frozen() {
        let schema = [7u8; 32];
        let root = [9u8; 32];
        let data = make_schema_account(schema, [0u8; 32], root, 1);
        assert!(!verify_schema_root_binding(&data, &root, &schema));
    }

    #[test]
    fn root_binding_rejects_bad_discriminator() {
        let data = vec![0u8; 113];
        assert!(!verify_schema_root_binding(&data, &[0u8; 32], &[0u8; 32]));
    }

    #[test]
    fn root_binding_rejects_short_data() {
        let data = vec![0u8; 40];
        assert!(!verify_schema_root_binding(&data, &[0u8; 32], &[0u8; 32]));
    }

    #[test]
    fn global_root_happy_path() {
        let root = [42u8; 32];
        let data = make_global_root_account(root);
        assert!(verify_state_root_matches(&data, &root));
    }

    #[test]
    fn global_root_rejects_mismatch() {
        let root = [42u8; 32];
        let wrong = [43u8; 32];
        let data = make_global_root_account(root);
        assert!(!verify_state_root_matches(&data, &wrong));
    }
}
