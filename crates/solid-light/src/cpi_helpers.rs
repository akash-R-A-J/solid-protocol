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
pub const SCHEMA_REGISTRY_PROGRAM_ID: &str = "4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1";

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
/// `"4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1"`. Kept as a separate const
/// so the program-ID literal above is usable in `const` contexts without the
/// `pubkey!` proc-macro, which is not re-exported from `anchor_lang::prelude`.
const SCHEMA_REGISTRY_ID_BYTES: [u8; 32] = [
    52, 211, 14, 237, 78, 235, 34, 17, 230, 13, 239, 27, 51, 103, 62, 210, 206, 53, 34, 210, 147,
    198, 67, 29, 255, 178, 228, 216, 244, 2, 125, 52,
];

/// Hard-coded program ID for the `issuer-registry` program.
///
/// Introduced by ADR-0014.  The zk-verifier owner-checks
/// `issuer_tree_binding` against this constant before trusting any
/// byte of it -- same two-layer guard pattern as
/// `SCHEMA_REGISTRY_ID` (SOLID-SEC-032).
pub const ISSUER_REGISTRY_PROGRAM_ID: &str = "5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx";

/// Typed counterpart of [`ISSUER_REGISTRY_PROGRAM_ID`].
pub const ISSUER_REGISTRY_ID: Pubkey =
    anchor_lang::prelude::Pubkey::new_from_array(ISSUER_REGISTRY_ID_BYTES);

/// Raw bytes of `ISSUER_REGISTRY_ID`, base58-decoded at the source
/// location `"5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx"`.  The
/// drift test below is the local gate; `scripts/check_program_ids.py`
/// is the repo-wide gate.
const ISSUER_REGISTRY_ID_BYTES: [u8; 32] = [
    69, 105, 202, 235, 252, 94, 62, 176, 58, 15, 195, 222, 140, 226, 248, 106, 219, 80, 80, 11, 75,
    153, 89, 137, 211, 153, 71, 68, 205, 102, 24, 79,
];

// ─── Program-ID consistency tests (SOLID-SEC-032) ──────────────────────────
//
// The owner-check that anchors the whole trust model of zk-verifier
// (`ADR-0010`) compares `account.owner` to `SCHEMA_REGISTRY_ID`, which is
// built from `SCHEMA_REGISTRY_ID_BYTES` above. If the byte array ever
// drifts from the base58 literal in `SCHEMA_REGISTRY_PROGRAM_ID` (e.g.,
// someone updates the literal but forgets to re-transcribe the bytes, or
// redeploys to a new ID but updates only `Anchor.toml`), the owner-check
// silently starts accepting nothing OR accepting the wrong program, and
// the forged-trust-root attack surface reopens with zero test-failure
// signal. These tests are the local gate. `scripts/check_program_ids.py`
// is the repository-wide gate that cross-validates the literal against
// `Anchor.toml` and `declare_id!`.

#[cfg(test)]
mod id_bytes_tests {
    use super::*;
    use std::str::FromStr;

    /// SOLID-SEC-032: byte array must equal the base58-decoded string
    /// literal in the same file.
    #[test]
    fn schema_registry_id_bytes_matches_program_id_literal() {
        let decoded = Pubkey::from_str(SCHEMA_REGISTRY_PROGRAM_ID)
            .expect("SCHEMA_REGISTRY_PROGRAM_ID must be a valid base58 pubkey");
        assert_eq!(
            decoded.to_bytes(),
            SCHEMA_REGISTRY_ID_BYTES,
            "SCHEMA_REGISTRY_ID_BYTES drifted from SCHEMA_REGISTRY_PROGRAM_ID. \
             Re-derive bytes from the base58 literal via \
             `Pubkey::from_str(SCHEMA_REGISTRY_PROGRAM_ID).to_bytes()`."
        );
    }

    /// SOLID-SEC-032: the typed `Pubkey` built from the array must equal
    /// the `Pubkey` decoded from the string literal.
    #[test]
    fn schema_registry_id_typed_matches_program_id_literal() {
        let decoded = Pubkey::from_str(SCHEMA_REGISTRY_PROGRAM_ID)
            .expect("SCHEMA_REGISTRY_PROGRAM_ID must be a valid base58 pubkey");
        assert_eq!(
            SCHEMA_REGISTRY_ID, decoded,
            "SCHEMA_REGISTRY_ID (typed Pubkey) diverged from \
             SCHEMA_REGISTRY_PROGRAM_ID (base58 literal)."
        );
    }

    /// ADR-0014: identical drift guard for `ISSUER_REGISTRY_ID_BYTES`.
    /// The zk-verifier's owner-check on `issuer_tree_binding`
    /// (Phase 2 circuit rev) depends on this being correct.
    #[test]
    fn issuer_registry_id_bytes_matches_program_id_literal() {
        let decoded = Pubkey::from_str(ISSUER_REGISTRY_PROGRAM_ID)
            .expect("ISSUER_REGISTRY_PROGRAM_ID must be a valid base58 pubkey");
        assert_eq!(
            decoded.to_bytes(),
            ISSUER_REGISTRY_ID_BYTES,
            "ISSUER_REGISTRY_ID_BYTES drifted from ISSUER_REGISTRY_PROGRAM_ID. \
             Re-derive bytes via `Pubkey::from_str(...).to_bytes()`."
        );
    }

    #[test]
    fn issuer_registry_id_typed_matches_program_id_literal() {
        let decoded = Pubkey::from_str(ISSUER_REGISTRY_PROGRAM_ID)
            .expect("ISSUER_REGISTRY_PROGRAM_ID must be a valid base58 pubkey");
        assert_eq!(
            ISSUER_REGISTRY_ID, decoded,
            "ISSUER_REGISTRY_ID (typed Pubkey) diverged from ISSUER_REGISTRY_PROGRAM_ID."
        );
    }
}

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
    borsh::to_writer(&mut data, &identity).map_err(|_| error!(LightError::SerializationFailed))?;
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
    borsh::to_writer(&mut data, &issuer).map_err(|_| error!(LightError::SerializationFailed))?;
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

// ─── Schema-tree binding field accessors (SOLID-SEC-003) ───────────────────
//
// `issuer-registry::issue_credential` must anchor every append to a
// *registered* schema + tree pair. Before SOLID-SEC-003, the handler
// derived the tree-authority PDA from a caller-supplied `schema_hash`
// with no cross-check against schema_registry, which meant an approved
// issuer could spawn a rogue schema/tree universe by passing a hash
// they'd never registered. The fix reads the `SchemaTreeBinding` PDA
// and asserts (a) the discriminator is valid, (b) the embedded
// schema_hash matches the instruction argument, (c) the embedded
// tree_pubkey matches the `merkle_tree` account the handler is about
// to `append` into, and (d) the binding is not frozen.
//
// Keeping these accessors in `solid-light` keeps the byte layout
// owned by a single crate (the layout contract). Both `issuer-registry`
// and `zk-verifier` consume them, so no program parses raw bytes
// inline.

/// Extract `schema_hash` (bytes `[8..40)`) from a `SchemaTreeBinding`
/// account's data. Returns `None` if the buffer is too short or the
/// discriminator is not `b"schmtree"`.
pub fn schema_tree_binding_schema_hash(data: &[u8]) -> Option<[u8; 32]> {
    if data.len() < 40 || data[..8] != SCHEMA_TREE_DISCRIMINATOR {
        return None;
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&data[8..40]);
    Some(out)
}

/// Extract `tree_pubkey` (bytes `[40..72)`) from a `SchemaTreeBinding`
/// account's data. Returns `None` if the buffer is too short or the
/// discriminator is not `b"schmtree"`.
pub fn schema_tree_binding_tree_pubkey(data: &[u8]) -> Option<Pubkey> {
    if data.len() < 72 || data[..8] != SCHEMA_TREE_DISCRIMINATOR {
        return None;
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&data[40..72]);
    Some(Pubkey::new_from_array(out))
}

/// Extract the status byte (offset `112`) from a `SchemaTreeBinding`
/// account's data. Returns `None` if the buffer is too short or the
/// discriminator is not `b"schmtree"`.
pub fn schema_tree_binding_status(data: &[u8]) -> Option<u8> {
    if data.len() < 113 || data[..8] != SCHEMA_TREE_DISCRIMINATOR {
        return None;
    }
    Some(data[112])
}

/// Full `SchemaTreeBinding` gate used by `issue_credential` (SOLID-SEC-003).
///
/// Validates that the given account data:
///   1. Is at least 113 bytes (the parser window defined above).
///   2. Starts with the `SCHEMA_TREE_DISCRIMINATOR`.
///   3. Carries `schema_hash == expected_schema`.
///   4. Carries `tree_pubkey == expected_tree`.
///   5. Is active (status byte `0`).
///
/// Caller is still responsible for asserting `account.owner == SCHEMA_REGISTRY_ID`
/// before calling this function — the bytes alone cannot prove that the
/// account was written by `schema-registry`.  Keeping that check in the
/// caller lets this helper stay `Pubkey` / data only and trivially unit-
/// testable from the host side.
pub fn verify_schema_tree_binding_for_issue(
    data: &[u8],
    expected_schema: &[u8; 32],
    expected_tree: &Pubkey,
) -> std::result::Result<(), LightError> {
    let schema = schema_tree_binding_schema_hash(data).ok_or(LightError::InvalidSchemaBinding)?;
    if &schema != expected_schema {
        return Err(LightError::InvalidSchemaBinding);
    }
    let tree = schema_tree_binding_tree_pubkey(data).ok_or(LightError::InvalidSchemaBinding)?;
    if &tree != expected_tree {
        return Err(LightError::TreeBindingMismatch);
    }
    let status = schema_tree_binding_status(data).ok_or(LightError::InvalidSchemaBinding)?;
    if status != STATUS_ACTIVE_BYTE {
        return Err(LightError::SchemaTreeBindingFrozen);
    }
    Ok(())
}

/// Value of the `status` byte (offset 112 in `SchemaTreeBinding`)
/// that the layout treats as "active, accepting appends". Mirrors
/// `schema_registry::STATUS_ACTIVE`. Any change in schema-registry
/// must come with a matching update here -- the layout in this
/// file is the parser contract, and drift would silently
/// re-authorize frozen trees for issuance.
pub const STATUS_ACTIVE_BYTE: u8 = 0;

// ─── IssuerTreeBinding layout (ADR-0014; SOLID-SEC-004 prep) ───────────────
//
// Written by `issuer-registry` on `initialize_issuer_tree_binding` and
// updated by `update_issuer_tree_root`. The zk-verifier will owner-check
// the account against `ISSUER_REGISTRY_ID` (same two-layer guard as
// `SchemaTreeBinding`) and parse `current_root` from it to compare
// against the batch circuit's new public input.
//
//   ┌──────────── IssuerTreeBinding (113 bytes) ────────────┐
//   │  [0..  8)  discriminator = b"issrtree"                │
//   │  [8.. 40)  tree_pubkey      (SPL AC concurrent tree)  │
//   │ [40.. 72)  current_root                               │
//   │ [72.. 80)  last_updated_slot (u64 LE)                 │
//   │ [80.. 81)  status (0 = active, 1 = frozen)            │
//   │ [81..113)  authority (Pubkey)                         │
//   └────────────────────────────────────────────────────────┘
//
// Only one issuer tree exists per deployment -- no per-schema split --
// so there is no `schema_hash` field in the layout.  The PDA seed is
// likewise the literal `[b"issuer-tree-binding"]` with no parameter.
//
// Sized identically to the read-window of `SchemaTreeBinding` (113
// bytes) so the parser fast-path is shared.  If future fields are
// added, extend the total but keep [0..113) frozen as the parser
// contract.

pub const ISSUER_TREE_DISCRIMINATOR: [u8; 8] = *b"issrtree";

/// Extract `tree_pubkey` (bytes `[8..40)`) from an `IssuerTreeBinding`
/// account's data.  Returns `None` if the buffer is too short or the
/// discriminator is not `b"issrtree"`.
pub fn issuer_tree_binding_tree_pubkey(data: &[u8]) -> Option<Pubkey> {
    if data.len() < 40 || data[..8] != ISSUER_TREE_DISCRIMINATOR {
        return None;
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&data[8..40]);
    Some(Pubkey::new_from_array(out))
}

/// Extract `current_root` (bytes `[40..72)`) from an `IssuerTreeBinding`
/// account's data.  Returns `None` if the buffer is too short or the
/// discriminator is not `b"issrtree"`.
pub fn issuer_tree_binding_current_root(data: &[u8]) -> Option<[u8; 32]> {
    if data.len() < 72 || data[..8] != ISSUER_TREE_DISCRIMINATOR {
        return None;
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&data[40..72]);
    Some(out)
}

/// Extract the status byte (offset `80`) from an `IssuerTreeBinding`
/// account's data.  Returns `None` if the buffer is too short or the
/// discriminator is not `b"issrtree"`.
pub fn issuer_tree_binding_status(data: &[u8]) -> Option<u8> {
    if data.len() < 81 || data[..8] != ISSUER_TREE_DISCRIMINATOR {
        return None;
    }
    Some(data[80])
}

/// Full `IssuerTreeBinding` gate used by `verify_batch_proof`
/// (ADR-0014).  Validates that the given account data:
///   1. Is at least 81 bytes (the parser window).
///   2. Starts with `ISSUER_TREE_DISCRIMINATOR`.
///   3. Carries `current_root == expected_root`.
///   4. Is active (status byte `0`).
///
/// Caller is still responsible for asserting
/// `account.owner == ISSUER_REGISTRY_ID` before calling this helper --
/// the bytes alone cannot prove provenance.  Kept consistent with
/// `verify_schema_tree_binding_for_issue`.
pub fn verify_issuer_tree_binding_for_proof(
    data: &[u8],
    expected_root: &[u8; 32],
) -> std::result::Result<(), LightError> {
    let root =
        issuer_tree_binding_current_root(data).ok_or(LightError::InvalidIssuerTreeBinding)?;
    if &root != expected_root {
        return Err(LightError::IssuerTreeRootMismatch);
    }
    let status = issuer_tree_binding_status(data).ok_or(LightError::InvalidIssuerTreeBinding)?;
    if status != STATUS_ACTIVE_BYTE {
        return Err(LightError::IssuerTreeBindingFrozen);
    }
    Ok(())
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
    #[msg("SchemaTreeBinding tree_pubkey does not match the supplied merkle_tree account")]
    TreeBindingMismatch,
    #[msg("SchemaTreeBinding is frozen; cannot issue into this tree")]
    SchemaTreeBindingFrozen,
    #[msg("IssuerTreeBinding discriminator or layout is invalid (ADR-0014)")]
    InvalidIssuerTreeBinding,
    #[msg("IssuerTreeBinding.current_root does not match the proof's issuerTreeRoot public input")]
    IssuerTreeRootMismatch,
    #[msg("IssuerTreeBinding is frozen; cannot verify proofs under this root")]
    IssuerTreeBindingFrozen,
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_schema_account(
        schema: [u8; 32],
        tree_pk: [u8; 32],
        root: [u8; 32],
        status: u8,
    ) -> Vec<u8> {
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

    // ─── SOLID-SEC-003 regression gates ───────────────────────────────────
    //
    // These exercise the schema-tree binding gate used by
    // `issuer-registry::issue_credential`. Before SEC-003, a caller could
    // pass *any* `schema_hash` and *any* `merkle_tree` to the handler;
    // these tests are the host-side evidence that the helper now refuses
    // every mismatch axis (schema, tree, status, discriminator, short
    // data) with the specific error code downstream consumers need.

    fn make_tree_pk(b: u8) -> Pubkey {
        let mut bytes = [0u8; 32];
        bytes.fill(b);
        Pubkey::new_from_array(bytes)
    }

    fn tree_bytes(pk: &Pubkey) -> [u8; 32] {
        pk.to_bytes()
    }

    #[test]
    fn schema_tree_binding_issue_gate_happy_path() {
        let schema = [7u8; 32];
        let tree = make_tree_pk(3);
        let root = [9u8; 32];
        let data = make_schema_account(schema, tree_bytes(&tree), root, 0);
        assert!(verify_schema_tree_binding_for_issue(&data, &schema, &tree).is_ok());
    }

    #[test]
    fn schema_tree_binding_issue_gate_rejects_wrong_schema() {
        let schema = [7u8; 32];
        let wrong = [8u8; 32];
        let tree = make_tree_pk(3);
        let data = make_schema_account(schema, tree_bytes(&tree), [0u8; 32], 0);
        assert!(matches!(
            verify_schema_tree_binding_for_issue(&data, &wrong, &tree),
            Err(LightError::InvalidSchemaBinding)
        ));
    }

    #[test]
    fn schema_tree_binding_issue_gate_rejects_wrong_tree() {
        let schema = [7u8; 32];
        let tree = make_tree_pk(3);
        let wrong_tree = make_tree_pk(4);
        let data = make_schema_account(schema, tree_bytes(&tree), [0u8; 32], 0);
        assert!(matches!(
            verify_schema_tree_binding_for_issue(&data, &schema, &wrong_tree),
            Err(LightError::TreeBindingMismatch)
        ));
    }

    #[test]
    fn schema_tree_binding_issue_gate_rejects_frozen() {
        let schema = [7u8; 32];
        let tree = make_tree_pk(3);
        let data = make_schema_account(schema, tree_bytes(&tree), [0u8; 32], 1);
        assert!(matches!(
            verify_schema_tree_binding_for_issue(&data, &schema, &tree),
            Err(LightError::SchemaTreeBindingFrozen)
        ));
    }

    #[test]
    fn schema_tree_binding_issue_gate_rejects_bad_discriminator() {
        let data = vec![0u8; 113];
        let tree = make_tree_pk(3);
        assert!(matches!(
            verify_schema_tree_binding_for_issue(&data, &[0u8; 32], &tree),
            Err(LightError::InvalidSchemaBinding)
        ));
    }

    #[test]
    fn schema_tree_binding_issue_gate_rejects_short_data() {
        let data = vec![0u8; 40];
        let tree = make_tree_pk(3);
        assert!(matches!(
            verify_schema_tree_binding_for_issue(&data, &[0u8; 32], &tree),
            Err(LightError::InvalidSchemaBinding)
        ));
    }

    // ─── ADR-0014 IssuerTreeBinding parser tests ──────────────────────────
    //
    // The zk-verifier's owner-check + discriminator + root-match gate on
    // `IssuerTreeBinding` is what SEC-004 relies on for its on-chain
    // guarantee. These tests prove the parser rejects every mismatch axis
    // the on-chain handler must not accept.

    fn make_issuer_tree_binding(tree_pk: [u8; 32], root: [u8; 32], status: u8) -> Vec<u8> {
        let mut v = Vec::with_capacity(113);
        v.extend_from_slice(&ISSUER_TREE_DISCRIMINATOR);
        v.extend_from_slice(&tree_pk);
        v.extend_from_slice(&root);
        v.extend_from_slice(&0u64.to_le_bytes());
        v.push(status);
        // pad to full 113-byte layout (authority field) so the parser's
        // minimum-length assertions stay realistic.
        v.extend_from_slice(&[0u8; 32]);
        v
    }

    #[test]
    fn issuer_tree_binding_field_accessors_happy_path() {
        let tree = make_tree_pk(7);
        let root = [9u8; 32];
        let data = make_issuer_tree_binding(tree.to_bytes(), root, 0);
        assert_eq!(issuer_tree_binding_tree_pubkey(&data), Some(tree));
        assert_eq!(issuer_tree_binding_current_root(&data), Some(root));
        assert_eq!(issuer_tree_binding_status(&data), Some(0));
    }

    #[test]
    fn issuer_tree_binding_rejects_bad_discriminator() {
        let mut data = make_issuer_tree_binding([0u8; 32], [0u8; 32], 0);
        data[..8].copy_from_slice(b"schmtree");
        assert_eq!(issuer_tree_binding_tree_pubkey(&data), None);
        assert_eq!(issuer_tree_binding_current_root(&data), None);
        assert_eq!(issuer_tree_binding_status(&data), None);
    }

    #[test]
    fn issuer_tree_binding_rejects_short_data() {
        let data = vec![0u8; 30];
        assert_eq!(issuer_tree_binding_tree_pubkey(&data), None);
        assert_eq!(issuer_tree_binding_current_root(&data), None);
        assert_eq!(issuer_tree_binding_status(&data), None);
    }

    #[test]
    fn issuer_tree_proof_gate_happy_path() {
        let root = [9u8; 32];
        let data = make_issuer_tree_binding([0u8; 32], root, 0);
        assert!(verify_issuer_tree_binding_for_proof(&data, &root).is_ok());
    }

    #[test]
    fn issuer_tree_proof_gate_rejects_wrong_root() {
        let root = [9u8; 32];
        let wrong = [10u8; 32];
        let data = make_issuer_tree_binding([0u8; 32], root, 0);
        assert!(matches!(
            verify_issuer_tree_binding_for_proof(&data, &wrong),
            Err(LightError::IssuerTreeRootMismatch)
        ));
    }

    #[test]
    fn issuer_tree_proof_gate_rejects_frozen() {
        let root = [9u8; 32];
        let data = make_issuer_tree_binding([0u8; 32], root, 1);
        assert!(matches!(
            verify_issuer_tree_binding_for_proof(&data, &root),
            Err(LightError::IssuerTreeBindingFrozen)
        ));
    }

    #[test]
    fn issuer_tree_proof_gate_rejects_bad_discriminator() {
        let mut data = make_issuer_tree_binding([0u8; 32], [0u8; 32], 0);
        data[..8].copy_from_slice(b"schmtree");
        assert!(matches!(
            verify_issuer_tree_binding_for_proof(&data, &[0u8; 32]),
            Err(LightError::InvalidIssuerTreeBinding)
        ));
    }
}
