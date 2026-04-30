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

/// Extract `current_root` (bytes `[8..40)`) from a `GlobalStateBinding` PDA.
///
/// SOLID-SEC-054 / B13 reconstruction path: rather than the caller
/// asserting `globalRoot` on the wire, the handler reads the canonical
/// value directly from the on-chain account.  The owner-check is the
/// caller's responsibility -- the bytes alone cannot prove the account
/// was written by `schema-registry` (the same trust-boundary discipline
/// as `verify_schema_tree_binding_for_issue`).
///
/// Returns `Err(StateRootMismatch)` if the buffer is too short or the
/// discriminator is wrong.  Variant choice keeps the on-chain error
/// surface stable: pre-B13 callers used `ErrorCode::InvalidGlobalRoot`
/// and the discriminator-failure remap path is unchanged.
pub fn extract_global_state_root(
    tree_account_data: &[u8],
) -> std::result::Result<[u8; 32], LightError> {
    if tree_account_data.len() < 40 {
        return Err(LightError::StateRootMismatch);
    }
    if tree_account_data[..8] != GLOBAL_ROOT_DISCRIMINATOR {
        return Err(LightError::StateRootMismatch);
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&tree_account_data[8..40]);
    Ok(out)
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

/// Extract `current_root` (bytes `[72..104)`) from a `SchemaTreeBinding`
/// account's data. Returns `None` if the buffer is too short or the
/// discriminator is not `b"schmtree"`.
pub fn schema_tree_binding_current_root(data: &[u8]) -> Option<[u8; 32]> {
    if data.len() < 104 || data[..8] != SCHEMA_TREE_DISCRIMINATOR {
        return None;
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&data[72..104]);
    Some(out)
}

/// SOLID-SEC-054 / B13 reconstruction helper: extract the
/// `(current_root, schema_hash)` pair from an *active* `SchemaTreeBinding`.
///
/// This is the source-of-truth read path for slots `merkleRoots[i]`
/// (public-input slot `2 + i`) and `schemaHashes[i]` (slot `6 + i`)
/// when the on-chain handler reconstructs them from accounts rather
/// than trusting the caller's wire-supplied values.  Errors:
/// `InvalidSchemaBinding` for a malformed account; `SchemaTreeBindingFrozen`
/// for a binding whose status byte is not `STATUS_ACTIVE_BYTE`.
///
/// Caller is still responsible for asserting `account.owner ==
/// SCHEMA_REGISTRY_ID` -- bytes alone cannot prove provenance.
pub fn extract_active_schema_root_binding(
    data: &[u8],
) -> std::result::Result<([u8; 32], [u8; 32]), LightError> {
    let schema_hash =
        schema_tree_binding_schema_hash(data).ok_or(LightError::InvalidSchemaBinding)?;
    let current_root =
        schema_tree_binding_current_root(data).ok_or(LightError::InvalidSchemaBinding)?;
    let status = schema_tree_binding_status(data).ok_or(LightError::InvalidSchemaBinding)?;
    if status != STATUS_ACTIVE_BYTE {
        return Err(LightError::SchemaTreeBindingFrozen);
    }
    Ok((current_root, schema_hash))
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

/// SOLID-SEC-054 / B13 reconstruction helper: extract `current_root`
/// from an *active* `IssuerTreeBinding`.
///
/// Mirrors `extract_active_schema_root_binding` for the singleton
/// issuer-tree binding referenced at public-input slot
/// `ISSUER_TREE_ROOT_INPUT_INDEX = 10`.  Errors:
/// `InvalidIssuerTreeBinding` for a malformed account;
/// `IssuerTreeBindingFrozen` for a binding whose status byte is not
/// `STATUS_ACTIVE_BYTE`.  Caller is still responsible for asserting
/// `account.owner == ISSUER_REGISTRY_ID`.
pub fn extract_active_issuer_tree_root(data: &[u8]) -> std::result::Result<[u8; 32], LightError> {
    let root =
        issuer_tree_binding_current_root(data).ok_or(LightError::InvalidIssuerTreeBinding)?;
    let status = issuer_tree_binding_status(data).ok_or(LightError::InvalidIssuerTreeBinding)?;
    if status != STATUS_ACTIVE_BYTE {
        return Err(LightError::IssuerTreeBindingFrozen);
    }
    Ok(root)
}

// ─── On-chain Poseidon-Merkle recompute (SOLID-SEC-059 / H1 closure) ────
//
// The IssuerTreeBinding stores the *Poseidon* root of the issuer tree
// because the in-circuit `MerkleInclusion` template (circuits/lib/
// merkle_inclusion.circom) uses Poseidon(2) for path-recompute and
// the witness commits to a Poseidon root.  The on-chain SPL AC tree
// is Keccak-hashed and is the leaf-presence ledger; its root is NOT
// the canonical root for proof verification (mixing the two was the
// root cause of the LB4 hash-family mismatch surfaced 2026-04-30
// when B13 Option 2 closed the wire-size cap and the Groth16 verify
// path actually ran end-to-end).
//
// `update_issuer_tree_root` accepts a caller-supplied
// `(new_root, new_leaf, leaf_index, poseidon_proof_path)` and rejects
// any push whose recompute does not match `new_root`.  Off-chain
// callers derive the path via `LocalReplicaAdapter` (Poseidon-hashed)
// in `@solid-protocol/light`.  Soundness rests on Poseidon's
// collision resistance + the circuit consuming the same root the
// binding stored.

/// Recompute a Poseidon-Merkle root from a leaf, its index, and the
/// full proof path of sibling hashes.  Output is byte-identical to
/// the off-chain `LocalReplicaAdapter.getRoot()` produced by
/// `@solid-protocol/light` (which calls `poseidonHashPair`, which
/// is the same `Poseidon(2)` the circuit's `MerkleInclusion`
/// template uses).
///
/// Convention: `proof_path[i]` is the sibling at level `i` (level 0 =
/// leaf level).  The bit at position `i` of `leaf_index` selects
/// whether the running node is the LEFT or RIGHT child at that level
/// (matches circomlib `Mux1`-driven hashing in
/// `merkle_inclusion.circom`).
///
/// Caller is responsible for asserting `proof_path.len() == DEPTH`.
/// On BPF, dispatches to `solana_program::poseidon::hashv` (the
/// `sol_poseidon` syscall).  Cost: ~3K CU per Poseidon-2 + the
/// canonicalize-to-Fr round; ~50K CU for a depth-16 path.
pub fn compute_poseidon_merkle_root(
    leaf: &[u8; 32],
    leaf_index: u64,
    proof_path: &[[u8; 32]],
) -> Result<[u8; 32]> {
    let mut node: [u8; 32] = *leaf;
    let mut idx: u64 = leaf_index;
    for sibling in proof_path.iter() {
        let bit = idx & 1;
        let pair: [[u8; 32]; 2] = if bit == 0 {
            [node, *sibling]
        } else {
            [*sibling, node]
        };
        // solid_core::poseidon::hash_bytes is dual-target:
        // BPF -> sol_poseidon syscall; host -> light-poseidon.
        // Both produce byte-identical Poseidon(2) output matching
        // circomlib.
        node = solid_core::poseidon::hash_bytes(&pair)
            .map_err(|_| error!(LightError::InvalidIssuerTreeBinding))?;
        idx >>= 1;
    }
    Ok(node)
}

// ─── SPL AC ConcurrentMerkleTree v1 active-root parser ────────────────
//
// Kept as a utility for future use (e.g., when a Solana-native
// Poseidon-Merkle program replaces SPL AC for the issuer tree, or
// when a Keccak-side audit is needed).  No longer called in the
// binding-update path -- see compute_poseidon_merkle_root above.
//
// The atomic handlers (`revoke_issuer_atomic`, `request_withdrawal_atomic`)
// perform a `replace_leaf` CPI into SPL Account Compression and must then
// write the post-CPI active root into `IssuerTreeBinding.current_root` in
// the same ix.
//
// Pre-CRIT-2 design: recompute the new root client-side from
// `(new_leaf, leaf_index, supplied_path)`.  The 2026-04-29 synthesis audit
// (CRIT-2 / SEC-057) found this is unsound under SPL AC's *concurrent*
// semantics.  SPL AC's `set_leaf` validates the supplied proof against any
// root in the change_log ring buffer (default 64 entries), not only the
// active root.  When the supplied proof matches a stale root, SPL AC walks
// the change_log forward to fast-forward the proof's siblings to the live
// state, then commits an updated leaf.  The post-CPI active root is
// `change_log[new_active_index].root`, which depends on the in-buffer deltas
// -- not on the user-supplied path alone.  Two operators calling
// `revoke_issuer_atomic` back-to-back in the same block (both with paths
// against the same pre-CPI root) would produce a recomputed root the SPL AC
// tree never had -> phantom binding (DoS) or replay against a prior
// legitimate root.
//
// Robust fix: after the CPI succeeds, re-borrow the merkle_tree account
// data and read `change_logs[active_index].root` directly.  By definition,
// that IS the post-CPI active root, regardless of how many concurrent
// updates landed.

/// SPL Account Compression issuer-tree depth.  Matches
/// `scripts/backfill_issuer_tree.ts` (default 16, override via
/// `SOLID_ISSUER_TREE_DEPTH`).  The issuer tree is configured with
/// canopy depth 0 so the full path is always on-wire when SPL AC needs
/// it; this parser verifies the on-chain `max_depth` field matches.
pub const ISSUER_TREE_DEPTH: usize = 16;

/// SPL AC issuer-tree max_buffer_size (canonical default).  Matches
/// `scripts/backfill_issuer_tree.ts`.
pub const ISSUER_TREE_BUFFER_SIZE: usize = 64;

// SPL AC `ConcurrentMerkleTreeAccount` V1 byte layout for an issuer tree
// (max_depth=16, max_buffer_size=64, canopy=0).  Verified against the
// `@solana/spl-account-compression` 0.2.x reference implementation
// (`accounts/ConcurrentMerkleTreeAccount.js::deserializeConcurrentMerkleTree`
// + `types/ConcurrentMerkleTree.js::concurrentMerkleTreeBeetFactory`).
//
//   offset 0    account_type (CompressionAccountType::ConcurrentMerkleTree=1) : u8
//   offset 1    header_version (ConcurrentMerkleTreeHeaderData::V1=0)         : u8
//   offset 2    max_buffer_size                                               : u32 LE
//   offset 6    max_depth                                                     : u32 LE
//   offset 10   authority                                                     : Pubkey
//   offset 42   creation_slot                                                 : u64 LE
//   offset 50   _padding                                                      : [u8; 6]
//   offset 56   sequence_number                                               : u64 LE
//   offset 64   active_index                                                  : u64 LE
//   offset 72   buffer_size                                                   : u64 LE
//   offset 80   change_logs[0..MAX_BUFFER_SIZE]                               : (32 + max_depth*32 + 4 + 4) B each
//                 each ChangeLog<MAX_DEPTH>:
//                   +0     root                                       : [u8; 32]
//                   +32    path_nodes                                 : [Pubkey; MAX_DEPTH]
//                   +32+MAX_DEPTH*32   index                          : u32 LE
//                   +36+MAX_DEPTH*32   _pad                           : u32
//   offset 80+MAX_BUFFER_SIZE*CHANGE_LOG_SIZE   rightmost_proof       : (same layout as ChangeLog)

const SPL_AC_ACCOUNT_TYPE_OFFSET: usize = 0;
const SPL_AC_HEADER_VERSION_OFFSET: usize = 1;
const SPL_AC_MAX_BUFFER_SIZE_OFFSET: usize = 2;
const SPL_AC_MAX_DEPTH_OFFSET: usize = 6;
const SPL_AC_ACTIVE_INDEX_OFFSET: usize = 64;
const SPL_AC_CHANGE_LOGS_OFFSET: usize = 80;

const SPL_AC_ACCOUNT_TYPE_CMT: u8 = 1;
const SPL_AC_HEADER_DATA_V1_KIND: u8 = 0;

/// Per-entry size of `ChangeLog<MAX_DEPTH=16>`: 32 (root) + 16*32 (path) + 4
/// (index) + 4 (padding) = 552 bytes.
pub const SPL_AC_CHANGE_LOG_SIZE_DEPTH_16: usize = 32 + ISSUER_TREE_DEPTH * 32 + 4 + 4;

/// Minimum legal account size for an issuer-tree CMT (depth=16, buffer=64,
/// canopy=0).  Matches `getConcurrentMerkleTreeAccountSize(16, 64, 0)` in
/// `@solana/spl-account-compression`.  The trailing `+ SPL_AC_CHANGE_LOG_SIZE_DEPTH_16`
/// term is the rightmost_proof Path, which has the same byte size as a
/// ChangeLog (per SPL AC's `concurrentMerkleTreeBeetFactory`).
pub const SPL_AC_ISSUER_TREE_ACCOUNT_SIZE: usize = SPL_AC_CHANGE_LOGS_OFFSET
    + ISSUER_TREE_BUFFER_SIZE * SPL_AC_CHANGE_LOG_SIZE_DEPTH_16
    + SPL_AC_CHANGE_LOG_SIZE_DEPTH_16;

const _: () = assert!(SPL_AC_CHANGE_LOG_SIZE_DEPTH_16 == 552);
const _: () = assert!(SPL_AC_ISSUER_TREE_ACCOUNT_SIZE == 35960);

/// Read the post-CPI active Merkle root from an SPL AC
/// `ConcurrentMerkleTreeAccount` of (max_depth=16, max_buffer_size=64,
/// canopy=0) -- the dimensions used by SolID's issuer tree (ADR-0014).
///
/// MUST be called AFTER `invoke_signed(replace_leaf, ...)` returns Ok and
/// the `merkle_tree` account data has been re-borrowed from the runtime.
/// By definition, `change_logs[active_index].root` is the root SPL AC
/// just committed -- no recomputation needed and no soundness assumption
/// about whether the supplied proof was against the live or a stale root.
///
/// Validates the on-chain account dimensions (account_type, header
/// version, max_depth, max_buffer_size, active_index in range) BEFORE
/// reading any path-relative byte; refuses any account whose layout
/// drifts from the documented contract.  The fail-fast strategy avoids
/// the SOLID-SEC-031 byte-encoding-drift class permanently for this
/// surface.
///
/// Returns `LightError::InvalidIssuerTreeBinding` on any inconsistency
/// (re-using the existing error code keeps caller `?`-flow simple; the
/// failure reason is observable via the `msg!` log).
pub fn read_spl_ac_active_root_d16_b64(merkle_tree_data: &[u8]) -> Result<[u8; 32]> {
    if merkle_tree_data.len() < SPL_AC_ISSUER_TREE_ACCOUNT_SIZE {
        msg!(
            "read_spl_ac_active_root: short buffer ({} < {})",
            merkle_tree_data.len(),
            SPL_AC_ISSUER_TREE_ACCOUNT_SIZE
        );
        return Err(error!(LightError::InvalidIssuerTreeBinding));
    }
    if merkle_tree_data[SPL_AC_ACCOUNT_TYPE_OFFSET] != SPL_AC_ACCOUNT_TYPE_CMT {
        msg!(
            "read_spl_ac_active_root: wrong account_type ({})",
            merkle_tree_data[SPL_AC_ACCOUNT_TYPE_OFFSET]
        );
        return Err(error!(LightError::InvalidIssuerTreeBinding));
    }
    if merkle_tree_data[SPL_AC_HEADER_VERSION_OFFSET] != SPL_AC_HEADER_DATA_V1_KIND {
        msg!(
            "read_spl_ac_active_root: unsupported header version ({})",
            merkle_tree_data[SPL_AC_HEADER_VERSION_OFFSET]
        );
        return Err(error!(LightError::InvalidIssuerTreeBinding));
    }
    let max_buffer_size = u32::from_le_bytes(
        merkle_tree_data[SPL_AC_MAX_BUFFER_SIZE_OFFSET..SPL_AC_MAX_BUFFER_SIZE_OFFSET + 4]
            .try_into()
            .map_err(|_| error!(LightError::InvalidIssuerTreeBinding))?,
    );
    let max_depth = u32::from_le_bytes(
        merkle_tree_data[SPL_AC_MAX_DEPTH_OFFSET..SPL_AC_MAX_DEPTH_OFFSET + 4]
            .try_into()
            .map_err(|_| error!(LightError::InvalidIssuerTreeBinding))?,
    );
    if max_depth as usize != ISSUER_TREE_DEPTH {
        msg!(
            "read_spl_ac_active_root: max_depth={} (expected {})",
            max_depth,
            ISSUER_TREE_DEPTH
        );
        return Err(error!(LightError::InvalidIssuerTreeBinding));
    }
    if max_buffer_size as usize != ISSUER_TREE_BUFFER_SIZE {
        msg!(
            "read_spl_ac_active_root: max_buffer_size={} (expected {})",
            max_buffer_size,
            ISSUER_TREE_BUFFER_SIZE
        );
        return Err(error!(LightError::InvalidIssuerTreeBinding));
    }
    let active_index = u64::from_le_bytes(
        merkle_tree_data[SPL_AC_ACTIVE_INDEX_OFFSET..SPL_AC_ACTIVE_INDEX_OFFSET + 8]
            .try_into()
            .map_err(|_| error!(LightError::InvalidIssuerTreeBinding))?,
    );
    if active_index >= max_buffer_size as u64 {
        msg!(
            "read_spl_ac_active_root: active_index={} >= max_buffer_size={}",
            active_index,
            max_buffer_size
        );
        return Err(error!(LightError::InvalidIssuerTreeBinding));
    }
    let root_offset =
        SPL_AC_CHANGE_LOGS_OFFSET + (active_index as usize) * SPL_AC_CHANGE_LOG_SIZE_DEPTH_16;
    let root: [u8; 32] = merkle_tree_data[root_offset..root_offset + 32]
        .try_into()
        .map_err(|_| error!(LightError::InvalidIssuerTreeBinding))?;
    Ok(root)
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

    // ─── SPL AC ConcurrentMerkleTree active-root parser ──────────────────
    //
    // Regression gate for the CRIT-2 / SEC-057 fix.  Synthesises an SPL AC
    // V1 account (depth=16, buffer=64, canopy=0) at known offsets and
    // asserts the parser reads `change_logs[active_index].root` exactly --
    // and refuses every layout-drift axis (account_type, header version,
    // max_depth, max_buffer_size, oversized active_index, short buffer).

    /// Build a V1 SPL AC `ConcurrentMerkleTreeAccount` byte image with
    /// the given `active_index` and the root at that change-log slot
    /// set to `root`.  Other slots are filled with deterministic but
    /// distinct bytes so a buggy parser that reads the wrong slot has
    /// nowhere to hide.
    fn make_spl_ac_issuer_tree_account(active_index: u64, root: [u8; 32]) -> Vec<u8> {
        let mut buf = vec![0u8; SPL_AC_ISSUER_TREE_ACCOUNT_SIZE];
        // Header: account_type=1 (CMT), header_version=0 (V1).
        buf[SPL_AC_ACCOUNT_TYPE_OFFSET] = SPL_AC_ACCOUNT_TYPE_CMT;
        buf[SPL_AC_HEADER_VERSION_OFFSET] = SPL_AC_HEADER_DATA_V1_KIND;
        // V1 header data: max_buffer_size = 64, max_depth = 16.
        buf[SPL_AC_MAX_BUFFER_SIZE_OFFSET..SPL_AC_MAX_BUFFER_SIZE_OFFSET + 4]
            .copy_from_slice(&(ISSUER_TREE_BUFFER_SIZE as u32).to_le_bytes());
        buf[SPL_AC_MAX_DEPTH_OFFSET..SPL_AC_MAX_DEPTH_OFFSET + 4]
            .copy_from_slice(&(ISSUER_TREE_DEPTH as u32).to_le_bytes());
        // active_index slot.
        buf[SPL_AC_ACTIVE_INDEX_OFFSET..SPL_AC_ACTIVE_INDEX_OFFSET + 8]
            .copy_from_slice(&active_index.to_le_bytes());
        // Fill every change_log[i].root with a distinguishable but
        // deterministic decoy pattern, then overwrite the active slot.
        for i in 0..ISSUER_TREE_BUFFER_SIZE {
            let off = SPL_AC_CHANGE_LOGS_OFFSET + i * SPL_AC_CHANGE_LOG_SIZE_DEPTH_16;
            // decoy = [i+1; 32] (avoids zero so a "read all zeroes" bug fails).
            let decoy = [(i as u8).wrapping_add(0xC0); 32];
            buf[off..off + 32].copy_from_slice(&decoy);
        }
        let active_off = SPL_AC_CHANGE_LOGS_OFFSET
            + (active_index as usize) * SPL_AC_CHANGE_LOG_SIZE_DEPTH_16;
        buf[active_off..active_off + 32].copy_from_slice(&root);
        buf
    }

    #[test]
    fn read_spl_ac_active_root_happy_path_active_index_zero() {
        let root: [u8; 32] = [0xAA; 32];
        let buf = make_spl_ac_issuer_tree_account(0, root);
        assert_eq!(read_spl_ac_active_root_d16_b64(&buf).unwrap(), root);
    }

    #[test]
    fn read_spl_ac_active_root_happy_path_active_index_max() {
        // active_index = MAX_BUFFER_SIZE - 1 = 63 is still a valid slot.
        let root: [u8; 32] = [0xBB; 32];
        let buf = make_spl_ac_issuer_tree_account((ISSUER_TREE_BUFFER_SIZE - 1) as u64, root);
        assert_eq!(read_spl_ac_active_root_d16_b64(&buf).unwrap(), root);
    }

    #[test]
    fn read_spl_ac_active_root_returns_active_slot_not_decoy() {
        // Every other slot has a decoy; the parser MUST find the
        // active slot specifically.
        let root: [u8; 32] = [0x77; 32];
        for active in [0u64, 1, 5, 17, 32, 63] {
            let buf = make_spl_ac_issuer_tree_account(active, root);
            let got = read_spl_ac_active_root_d16_b64(&buf).unwrap();
            assert_eq!(got, root, "active_index={} returned wrong root", active);
            // None of the decoys equal `root`, so the assertion above
            // already rules out off-by-N reads.  Belt-and-suspenders:
            assert_ne!(got[0], 0xC0u8.wrapping_add(active as u8));
        }
    }

    #[test]
    fn read_spl_ac_active_root_rejects_wrong_account_type() {
        let mut buf = make_spl_ac_issuer_tree_account(0, [0xAA; 32]);
        buf[SPL_AC_ACCOUNT_TYPE_OFFSET] = 2; // not CMT
        assert!(read_spl_ac_active_root_d16_b64(&buf).is_err());
    }

    #[test]
    fn read_spl_ac_active_root_rejects_wrong_header_version() {
        let mut buf = make_spl_ac_issuer_tree_account(0, [0xAA; 32]);
        buf[SPL_AC_HEADER_VERSION_OFFSET] = 1; // V2 not yet supported
        assert!(read_spl_ac_active_root_d16_b64(&buf).is_err());
    }

    #[test]
    fn read_spl_ac_active_root_rejects_wrong_max_depth() {
        let mut buf = make_spl_ac_issuer_tree_account(0, [0xAA; 32]);
        buf[SPL_AC_MAX_DEPTH_OFFSET..SPL_AC_MAX_DEPTH_OFFSET + 4]
            .copy_from_slice(&20u32.to_le_bytes()); // not 16
        assert!(read_spl_ac_active_root_d16_b64(&buf).is_err());
    }

    #[test]
    fn read_spl_ac_active_root_rejects_wrong_max_buffer_size() {
        let mut buf = make_spl_ac_issuer_tree_account(0, [0xAA; 32]);
        buf[SPL_AC_MAX_BUFFER_SIZE_OFFSET..SPL_AC_MAX_BUFFER_SIZE_OFFSET + 4]
            .copy_from_slice(&128u32.to_le_bytes()); // not 64
        assert!(read_spl_ac_active_root_d16_b64(&buf).is_err());
    }

    #[test]
    fn read_spl_ac_active_root_rejects_oversized_active_index() {
        let mut buf = make_spl_ac_issuer_tree_account(0, [0xAA; 32]);
        // active_index = 64 violates `active_index < buffer_size` and
        // would otherwise read past the end of change_logs.
        buf[SPL_AC_ACTIVE_INDEX_OFFSET..SPL_AC_ACTIVE_INDEX_OFFSET + 8]
            .copy_from_slice(&(ISSUER_TREE_BUFFER_SIZE as u64).to_le_bytes());
        assert!(read_spl_ac_active_root_d16_b64(&buf).is_err());
    }

    #[test]
    fn read_spl_ac_active_root_rejects_short_buffer() {
        let buf = vec![0u8; SPL_AC_ISSUER_TREE_ACCOUNT_SIZE - 1];
        assert!(read_spl_ac_active_root_d16_b64(&buf).is_err());
    }

    #[test]
    fn read_spl_ac_active_root_layout_constants_pinned() {
        // The const_assert!s outside the test module already gate the
        // structural sizes (552, 35960).  This test pins the offsets so
        // a future maintainer reading just the test understands what we
        // depend on.
        assert_eq!(SPL_AC_ACCOUNT_TYPE_OFFSET, 0);
        assert_eq!(SPL_AC_HEADER_VERSION_OFFSET, 1);
        assert_eq!(SPL_AC_MAX_BUFFER_SIZE_OFFSET, 2);
        assert_eq!(SPL_AC_MAX_DEPTH_OFFSET, 6);
        assert_eq!(SPL_AC_ACTIVE_INDEX_OFFSET, 64);
        assert_eq!(SPL_AC_CHANGE_LOGS_OFFSET, 80);
        assert_eq!(SPL_AC_CHANGE_LOG_SIZE_DEPTH_16, 552);
        assert_eq!(SPL_AC_ISSUER_TREE_ACCOUNT_SIZE, 35960);
    }

    // ─── SOLID-SEC-054 / B13 reconstruction helpers ──────────────────────
    //
    // The on-chain `verify_batch_proof` handler reconstructs 11 of the 32
    // public-input slots from the accounts it already takes, so the wire
    // payload shrinks from 1324 bytes to 972 bytes (under Solana's
    // 1232-byte legacy-tx packet limit). The extract_* helpers below are
    // the source-of-truth read path for those slots; their byte-layout
    // contract is shared with the equality-style verify_* helpers above.
    //
    // These tests are the regression gate for the layout: every variant
    // (good, frozen, malformed, truncated) round-trips byte-identically
    // against the verify_* paths, so a future layout change cannot land
    // without breaking both helpers in the same commit.

    #[test]
    fn extract_global_state_root_happy_path() {
        let root = [42u8; 32];
        let data = make_global_root_account(root);
        let extracted = extract_global_state_root(&data).expect("happy path");
        assert_eq!(extracted, root);
        assert!(verify_state_root_matches(&data, &extracted));
    }

    #[test]
    fn extract_global_state_root_rejects_bad_discriminator() {
        let mut data = make_global_root_account([42u8; 32]);
        data[0] = 0;
        assert!(matches!(
            extract_global_state_root(&data),
            Err(LightError::StateRootMismatch)
        ));
    }

    #[test]
    fn extract_global_state_root_rejects_short_data() {
        let data = vec![0u8; 39];
        assert!(matches!(
            extract_global_state_root(&data),
            Err(LightError::StateRootMismatch)
        ));
    }

    #[test]
    fn schema_tree_binding_current_root_happy_path() {
        let schema = [7u8; 32];
        let root = [9u8; 32];
        let tree_pk = [3u8; 32];
        let data = make_schema_account(schema, tree_pk, root, 0);
        assert_eq!(schema_tree_binding_current_root(&data), Some(root));
    }

    #[test]
    fn schema_tree_binding_current_root_rejects_bad_discriminator() {
        let mut data = make_schema_account([0u8; 32], [0u8; 32], [0u8; 32], 0);
        data[0] = 0;
        assert_eq!(schema_tree_binding_current_root(&data), None);
    }

    #[test]
    fn schema_tree_binding_current_root_rejects_short_data() {
        let data = vec![0u8; 100];
        assert_eq!(schema_tree_binding_current_root(&data), None);
    }

    #[test]
    fn extract_active_schema_root_binding_happy_path() {
        let schema = [7u8; 32];
        let root = [9u8; 32];
        let tree_pk = [3u8; 32];
        let data = make_schema_account(schema, tree_pk, root, 0);
        let (extracted_root, extracted_schema) =
            extract_active_schema_root_binding(&data).expect("happy path");
        assert_eq!(extracted_root, root);
        assert_eq!(extracted_schema, schema);
        // Round-trip equivalence with the verify_* path.
        assert!(verify_schema_root_binding(
            &data,
            &extracted_root,
            &extracted_schema
        ));
    }

    #[test]
    fn extract_active_schema_root_binding_rejects_frozen() {
        let data = make_schema_account([7u8; 32], [3u8; 32], [9u8; 32], 1);
        assert!(matches!(
            extract_active_schema_root_binding(&data),
            Err(LightError::SchemaTreeBindingFrozen)
        ));
    }

    #[test]
    fn extract_active_schema_root_binding_rejects_bad_discriminator() {
        let mut data = make_schema_account([0u8; 32], [0u8; 32], [0u8; 32], 0);
        data[0] = 0;
        assert!(matches!(
            extract_active_schema_root_binding(&data),
            Err(LightError::InvalidSchemaBinding)
        ));
    }

    #[test]
    fn extract_active_schema_root_binding_rejects_short_data() {
        let data = vec![0u8; 100];
        assert!(matches!(
            extract_active_schema_root_binding(&data),
            Err(LightError::InvalidSchemaBinding)
        ));
    }

    #[test]
    fn extract_active_issuer_tree_root_happy_path() {
        let tree = make_tree_pk(7);
        let root = [9u8; 32];
        let data = make_issuer_tree_binding(tree.to_bytes(), root, 0);
        let extracted = extract_active_issuer_tree_root(&data).expect("happy path");
        assert_eq!(extracted, root);
        // Round-trip equivalence with the verify_* path.
        assert!(verify_issuer_tree_binding_for_proof(&data, &extracted).is_ok());
    }

    #[test]
    fn extract_active_issuer_tree_root_rejects_frozen() {
        let tree = make_tree_pk(7);
        let data = make_issuer_tree_binding(tree.to_bytes(), [9u8; 32], 1);
        assert!(matches!(
            extract_active_issuer_tree_root(&data),
            Err(LightError::IssuerTreeBindingFrozen)
        ));
    }

    #[test]
    fn extract_active_issuer_tree_root_rejects_bad_discriminator() {
        let tree = make_tree_pk(7);
        let mut data = make_issuer_tree_binding(tree.to_bytes(), [9u8; 32], 0);
        data[0] = 0;
        assert!(matches!(
            extract_active_issuer_tree_root(&data),
            Err(LightError::InvalidIssuerTreeBinding)
        ));
    }

    #[test]
    fn extract_active_issuer_tree_root_rejects_short_data() {
        let data = vec![0u8; 70];
        assert!(matches!(
            extract_active_issuer_tree_root(&data),
            Err(LightError::InvalidIssuerTreeBinding)
        ));
    }
}
