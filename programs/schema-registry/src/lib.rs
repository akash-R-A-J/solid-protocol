// SOLID-SEC-A5 (2026-05-28): no production-code panic surfaces.
// See programs/zk-verifier/src/lib.rs for the full rationale. Tests are
// explicitly allowed because `unwrap`/`expect`/`panic!` are the standard
// assertion mechanism in `#[test]` functions.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use anchor_lang::prelude::*;
use anchor_lang::solana_program::system_instruction;

declare_id!("4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1");

// ─── Byte-level binding layouts ────────────────────────────────────────────
//
// The `solid-light` crate parses these accounts from raw bytes (see
// `crates/solid-light/src/cpi_helpers.rs`), so their layout is a
// *protocol-level* contract — not just an in-program struct.  We therefore
// allocate and write the PDAs by hand instead of going through `#[account]`
// (which would use a sha256-derived discriminator that does not match the
// literal 8-byte tags the parser expects).
//
//   ┌──────────── SchemaTreeBinding (144 bytes) ────────────┐
//   │  [0..  8)  discriminator = b"schmtree"                │
//   │  [8.. 40)  schema_hash                                │
//   │ [40.. 72)  tree_pubkey      (SPL AC concurrent tree)  │
//   │ [72..104)  current_root                               │
//   │ [104..112) last_updated_slot (u64 LE)                 │
//   │ [112..113) status (0 = active, 1 = frozen)            │
//   │ [113..145) authority (Pubkey)                         │
//   └────────────────────────────────────────────────────────┘
//
//   ┌──────────── GlobalStateBinding (80 bytes) ────────────┐
//   │  [0..  8)  discriminator = b"globroot"                │
//   │  [8.. 40)  current_root                               │
//   │ [40.. 48)  last_updated_slot (u64 LE)                 │
//   │ [48.. 80)  authority (Pubkey)                         │
//   └────────────────────────────────────────────────────────┘
//
// The `solid-light` parser only reads bytes [0..113) of SchemaTreeBinding
// and [0..40) of GlobalStateBinding, so the trailing authority field is
// forward-compatible.

pub const SCHEMA_TREE_DISCRIMINATOR: [u8; 8] = *b"schmtree";
pub const SCHEMA_TREE_BINDING_SIZE: usize = 145;

pub const GLOBAL_ROOT_DISCRIMINATOR: [u8; 8] = *b"globroot";
pub const GLOBAL_STATE_BINDING_SIZE: usize = 80;

pub const STATUS_ACTIVE: u8 = 0;
pub const STATUS_FROZEN: u8 = 1;

// Length ceilings for register_schema.  These must match the space allocation
// of SchemaAccount exactly; the handler enforces them so Borsh serialization
// never overflows the allocated PDA.
pub const SCHEMA_MAX_NAME_LEN: usize = 64;
pub const SCHEMA_MAX_CATEGORY_LEN: usize = 64;
pub const SCHEMA_MAX_FIELDS: usize = 8;
pub const SCHEMA_MAX_FIELD_NAME_LEN: usize = 32;

/// Exact Borsh-encoded size of a SchemaAccount, matching the field order in
/// the struct definition below.
///
///   32 authority
/// + (4 + SCHEMA_MAX_NAME_LEN)       name (len-prefixed String)
/// +  1                              version
/// + (4 + SCHEMA_MAX_CATEGORY_LEN)   category
/// + (4 + SCHEMA_MAX_FIELDS * (4 + SCHEMA_MAX_FIELD_NAME_LEN))
///                                   field_names (Vec<String>)
/// + 32                              schema_hash
/// +  1                              deprecated (bool)
/// +  8                              created_at (i64)
/// +  8                              usage_count (u64)
pub const SCHEMA_ACCOUNT_SPACE: usize = 32
    + (4 + SCHEMA_MAX_NAME_LEN)
    + 1
    + (4 + SCHEMA_MAX_CATEGORY_LEN)
    + (4 + SCHEMA_MAX_FIELDS * (4 + SCHEMA_MAX_FIELD_NAME_LEN))
    + 32
    + 1
    + 8
    + 8;

// ─── Host-testable raw-bytes write helpers ─────────────────────────────────
//
// These helpers encapsulate the byte-level write contract for the two
// custom-layout PDAs (`SchemaTreeBinding`, `GlobalStateBinding`) so the
// inline ix bodies stay short AND so the layout checks can be unit-
// tested with synthetic buffers.  The handler is responsible for the
// owner-check on the AccountInfo BEFORE calling these (the byte-level
// helpers cannot prove provenance).

/// Apply a new root + current slot to a `SchemaTreeBinding` raw buffer.
/// Caller must already have asserted `account.owner == crate::ID`.
pub fn apply_schema_tree_root_update(
    data: &mut [u8],
    expected_schema_hash: &[u8; 32],
    signer_pubkey: &[u8; 32],
    new_root: &[u8; 32],
    now_slot: u64,
) -> Result<()> {
    require!(
        data.len() >= SCHEMA_TREE_BINDING_SIZE,
        ErrorCode::MalformedBinding
    );
    require!(
        data[0..8] == SCHEMA_TREE_DISCRIMINATOR,
        ErrorCode::MalformedBinding
    );
    require!(
        &data[8..40] == expected_schema_hash.as_slice(),
        ErrorCode::SchemaHashMismatch
    );
    require!(data[112] == STATUS_ACTIVE, ErrorCode::BindingFrozen);
    // SOLID-SEC-A5 (2026-05-28): the prior `data.len() >= SCHEMA_TREE_BINDING_SIZE`
    // require! makes both slices statically safe today, but we route through
    // `get(..).ok_or(..)? + try_into().map_err(..)?` so a future refactor of
    // the size check cannot silently re-introduce a panic on a too-short buffer.
    let stored_authority: [u8; 32] = data
        .get(113..145)
        .ok_or(error!(ErrorCode::MalformedBinding))?
        .try_into()
        .map_err(|_| error!(ErrorCode::MalformedBinding))?;
    require!(
        &stored_authority == signer_pubkey,
        ErrorCode::UnauthorizedTreeBinding
    );
    let last_slot_bytes: [u8; 8] = data
        .get(104..112)
        .ok_or(error!(ErrorCode::MalformedBinding))?
        .try_into()
        .map_err(|_| error!(ErrorCode::MalformedBinding))?;
    let last_slot = u64::from_le_bytes(last_slot_bytes);
    require!(now_slot > last_slot, ErrorCode::RootSlotNotMonotonic);

    data[72..104].copy_from_slice(new_root);
    data[104..112].copy_from_slice(&now_slot.to_le_bytes());
    Ok(())
}

/// Apply a status byte to a `SchemaTreeBinding` raw buffer.  Caller
/// must owner-check first.
pub fn apply_schema_tree_status_update(
    data: &mut [u8],
    signer_pubkey: &[u8; 32],
    new_status: u8,
) -> Result<()> {
    require!(
        new_status == STATUS_ACTIVE || new_status == STATUS_FROZEN,
        ErrorCode::InvalidStatus
    );
    require!(
        data.len() >= SCHEMA_TREE_BINDING_SIZE && data[0..8] == SCHEMA_TREE_DISCRIMINATOR,
        ErrorCode::MalformedBinding
    );
    // SOLID-SEC-A5 (2026-05-28): see write_schema_tree_binding_root for rationale.
    let stored_authority: [u8; 32] = data
        .get(113..145)
        .ok_or(error!(ErrorCode::MalformedBinding))?
        .try_into()
        .map_err(|_| error!(ErrorCode::MalformedBinding))?;
    require!(
        &stored_authority == signer_pubkey,
        ErrorCode::UnauthorizedTreeBinding
    );
    data[112] = new_status;
    Ok(())
}

/// Rotate authority on a `SchemaTreeBinding` raw buffer.  Caller must
/// owner-check first.
pub fn apply_schema_tree_authority_rotation(
    data: &mut [u8],
    signer_pubkey: &[u8; 32],
    new_authority: &[u8; 32],
) -> Result<()> {
    require!(
        data.len() >= SCHEMA_TREE_BINDING_SIZE && data[0..8] == SCHEMA_TREE_DISCRIMINATOR,
        ErrorCode::MalformedBinding
    );
    // SOLID-SEC-A5 (2026-05-28): see write_schema_tree_binding_root for rationale.
    let stored_authority: [u8; 32] = data
        .get(113..145)
        .ok_or(error!(ErrorCode::MalformedBinding))?
        .try_into()
        .map_err(|_| error!(ErrorCode::MalformedBinding))?;
    require!(
        &stored_authority == signer_pubkey,
        ErrorCode::UnauthorizedTreeBinding
    );
    data[113..145].copy_from_slice(new_authority);
    Ok(())
}

/// Apply a new root + current slot to the singleton `GlobalStateBinding`
/// raw buffer.  Caller must owner-check first.
pub fn apply_global_root_update(
    data: &mut [u8],
    signer_pubkey: &[u8; 32],
    new_root: &[u8; 32],
    now_slot: u64,
) -> Result<()> {
    require!(
        data.len() >= GLOBAL_STATE_BINDING_SIZE,
        ErrorCode::MalformedBinding
    );
    require!(
        data[0..8] == GLOBAL_ROOT_DISCRIMINATOR,
        ErrorCode::MalformedBinding
    );
    // SOLID-SEC-A5 (2026-05-28): see write_schema_tree_binding_root for rationale.
    let stored_authority: [u8; 32] = data
        .get(48..80)
        .ok_or(error!(ErrorCode::MalformedBinding))?
        .try_into()
        .map_err(|_| error!(ErrorCode::MalformedBinding))?;
    require!(
        &stored_authority == signer_pubkey,
        ErrorCode::UnauthorizedTreeBinding
    );
    let last_slot_bytes: [u8; 8] = data
        .get(40..48)
        .ok_or(error!(ErrorCode::MalformedBinding))?
        .try_into()
        .map_err(|_| error!(ErrorCode::MalformedBinding))?;
    let last_slot = u64::from_le_bytes(last_slot_bytes);
    require!(now_slot > last_slot, ErrorCode::RootSlotNotMonotonic);

    data[8..40].copy_from_slice(new_root);
    data[40..48].copy_from_slice(&now_slot.to_le_bytes());
    Ok(())
}

/// Rotate authority on the singleton `GlobalStateBinding` raw buffer.
/// Caller must owner-check first.
pub fn apply_global_authority_rotation(
    data: &mut [u8],
    signer_pubkey: &[u8; 32],
    new_authority: &[u8; 32],
) -> Result<()> {
    require!(
        data.len() >= GLOBAL_STATE_BINDING_SIZE && data[0..8] == GLOBAL_ROOT_DISCRIMINATOR,
        ErrorCode::MalformedBinding
    );
    // SOLID-SEC-A5 (2026-05-28): see write_schema_tree_binding_root for rationale.
    let stored_authority: [u8; 32] = data
        .get(48..80)
        .ok_or(error!(ErrorCode::MalformedBinding))?
        .try_into()
        .map_err(|_| error!(ErrorCode::MalformedBinding))?;
    require!(
        &stored_authority == signer_pubkey,
        ErrorCode::UnauthorizedTreeBinding
    );
    data[48..80].copy_from_slice(new_authority);
    Ok(())
}

/// Schema Registry — modular schema management + credential-tree bindings.
#[program]
pub mod schema_registry {
    use super::*;

    /// Register a new schema.
    ///
    /// The length caps on `name`, `category`, and `field_names` must match
    /// the SCHEMA_ACCOUNT_SPACE constant: overflowing any of them would push
    /// the Borsh-serialized layout past the allocated PDA size at write-time.
    pub fn register_schema(
        ctx: Context<RegisterSchema>,
        name: String,
        version: u8,
        category: String,
        field_names: Vec<String>,
        schema_hash: [u8; 32],
    ) -> Result<()> {
        require!(
            !field_names.is_empty() && field_names.len() <= SCHEMA_MAX_FIELDS,
            ErrorCode::InvalidFieldCount
        );
        require!(
            name.len() <= SCHEMA_MAX_NAME_LEN,
            ErrorCode::MetadataTooLong
        );
        require!(
            category.len() <= SCHEMA_MAX_CATEGORY_LEN,
            ErrorCode::MetadataTooLong
        );
        for fname in field_names.iter() {
            require!(
                fname.len() <= SCHEMA_MAX_FIELD_NAME_LEN,
                ErrorCode::MetadataTooLong
            );
        }

        // SEC-06 / SOLID-SEC-002 + SOLID-SEC-063 / H5: verify schema_hash
        // against the WIDENED preimage that binds field_names + category,
        // not just (name, version, field_count).  Two registrations
        // differing only in field semantics or category previously
        // collided to the same hash; verifiers accepting a proof under
        // schema A would treat it as schema B.  Shared derivation with
        // the off-chain SDK so the on-chain check cannot drift.  See
        // `solid_core::schema::tests::test_compute_schema_hash_binds_field_names_and_category`
        // for the regression gate.
        let computed_hash = solid_core::schema::compute_schema_hash_from_parts(
            &name,
            version,
            &field_names,
            &category,
        )
        .map_err(|_| error!(ErrorCode::PoseidonFailed))?;
        require!(computed_hash == schema_hash, ErrorCode::InvalidSchemaHash);

        let schema = &mut ctx.accounts.schema_account;
        schema.authority = ctx.accounts.authority.key();
        schema.name = name;
        schema.version = version;
        schema.category = category;
        schema.field_names = field_names;
        schema.schema_hash = schema_hash;
        schema.deprecated = false;
        schema.created_at = Clock::get()?.unix_timestamp;
        schema.usage_count = 0;

        msg!("Schema registered: {} v{}", schema.name, schema.version);
        Ok(())
    }

    /// Deprecate a schema (only authority).
    pub fn deprecate_schema(ctx: Context<DeprecateSchema>) -> Result<()> {
        let schema = &mut ctx.accounts.schema_account;
        schema.deprecated = true;
        msg!("Schema deprecated: {}", schema.name);
        Ok(())
    }

    /// Increment the schema's usage counter.
    ///
    /// Access control: only the schema authority may call this. The previous
    /// implementation had no signer constraint, which let anyone inflate a
    /// schema's `usage_count` to `u64::MAX` and DoS any downstream consumer
    /// that read the counter (analytics, governance weight, fee calculations).
    pub fn increment_usage(ctx: Context<IncrementUsage>) -> Result<()> {
        let schema = &mut ctx.accounts.schema_account;
        require!(!schema.deprecated, ErrorCode::SchemaDeprecated);
        require_keys_eq!(
            schema.authority,
            ctx.accounts.authority.key(),
            ErrorCode::UnauthorizedSchemaAuthority
        );
        schema.usage_count = schema
            .usage_count
            .checked_add(1)
            .ok_or(ErrorCode::Overflow)?;
        Ok(())
    }

    // ─── Tree-binding lifecycle (2026-04 R-2 remediation) ─────────────────

    /// Allocate and initialize a `SchemaTreeBinding` PDA for a specific schema.
    ///
    /// The binding records *which SPL Account-Compression tree* holds
    /// credentials issued under this schema, plus the most recently published
    /// Merkle root.  The on-chain ZK verifier consults this PDA via
    /// `solid_light::cpi_helpers::verify_schema_root_binding` to confirm that
    /// the `schemaHash` public input of a proof matches the tree whose
    /// membership path the proof attests to.
    ///
    /// Authorities:
    ///   * `schema_account.authority` (the owner of the schema) must sign.
    ///     This is the same party that registered the schema, so no new
    ///     permission surface is introduced.
    ///
    /// Invariants enforced:
    ///   * The account is created with the canonical seeds
    ///     `[b"schema-tree-binding", schema_hash]`.
    ///   * The 8-byte literal discriminator `b"schmtree"` is written, matching
    ///     the off-chain parser contract.
    pub fn initialize_tree_binding(
        ctx: Context<InitializeTreeBinding>,
        schema_hash: [u8; 32],
        tree_pubkey: Pubkey,
    ) -> Result<()> {
        // Authority is the schema owner.
        require_keys_eq!(
            ctx.accounts.schema_account.authority,
            ctx.accounts.authority.key(),
            ErrorCode::UnauthorizedTreeBinding
        );
        require!(
            ctx.accounts.schema_account.schema_hash == schema_hash,
            ErrorCode::SchemaHashMismatch
        );

        // Allocate the PDA via a System CreateAccount ix, owned by this program.
        let binding_info = ctx.accounts.schema_tree_binding.to_account_info();
        let rent = Rent::get()?;
        let lamports = rent.minimum_balance(SCHEMA_TREE_BINDING_SIZE);

        let signer_seeds: &[&[u8]] = &[
            b"schema-tree-binding",
            schema_hash.as_ref(),
            &[ctx.bumps.schema_tree_binding],
        ];
        let signer_seeds_all: &[&[&[u8]]] = &[signer_seeds];

        anchor_lang::solana_program::program::invoke_signed(
            &system_instruction::create_account(
                &ctx.accounts.authority.key(),
                &binding_info.key(),
                lamports,
                SCHEMA_TREE_BINDING_SIZE as u64,
                &crate::ID,
            ),
            &[
                ctx.accounts.authority.to_account_info(),
                binding_info.clone(),
                ctx.accounts.system_program.to_account_info(),
            ],
            signer_seeds_all,
        )?;

        // Write the binding contents.
        let mut data = binding_info.try_borrow_mut_data()?;
        data[0..8].copy_from_slice(&SCHEMA_TREE_DISCRIMINATOR);
        data[8..40].copy_from_slice(&schema_hash);
        data[40..72].copy_from_slice(&tree_pubkey.to_bytes());
        data[72..104].copy_from_slice(&[0u8; 32]); // root starts at 0
        data[104..112].copy_from_slice(&Clock::get()?.slot.to_le_bytes());
        data[112] = STATUS_ACTIVE;
        data[113..145].copy_from_slice(&ctx.accounts.authority.key().to_bytes());

        msg!(
            "SchemaTreeBinding initialised: schema={:?} tree={}",
            &schema_hash[..4],
            tree_pubkey
        );
        Ok(())
    }

    /// Mirror the current SPL AC Merkle root into the binding PDA.
    ///
    /// V1 uses an authority-gated push model: the party that runs the
    /// credential-issuance pipeline (the same `authority` recorded at
    /// `initialize_tree_binding` time) pushes the new root here as a
    /// follow-up to its `append`-into-the-SPL-AC-tree call, or an indexer
    /// pushes on its behalf after watching `CredentialIssued` events.
    ///
    /// This is the "event-driven indexer hand-off" the architecture doc
    /// describes.  A future v1.1 can replace this with a permissionless
    /// refresh that parses the SPL AC tree header directly; that is tracked
    /// in `docs/REVOCATION_DESIGN.md` alongside R-3.
    /// Update the schema-tree binding's Poseidon `current_root`.
    ///
    /// SOLID-SEC-059 sibling for the schema (credential) tree
    /// (closes 2026-04-30): pre-fix, this ix accepted any caller-supplied
    /// `new_root: [u8; 32]` from the binding's authority -- a single-key
    /// root-injection primitive that combined with SEC-043 yielded full
    /// proof-forging.  Robust fix: caller supplies (new_root, new_leaf,
    /// leaf_index, poseidon_proof_path) and the handler runs an on-chain
    /// Poseidon-Merkle recompute, refusing any push whose recompute does
    /// not equal `new_root`.  Mirrors the issuer-side
    /// `update_issuer_tree_root` integrity check.
    ///
    /// Architecture (LB4 closure 2026-04-30): the binding stores the
    /// POSEIDON root because the in-circuit `MerkleInclusion` template
    /// uses Poseidon(2) for path-recompute.  SPL AC's Keccak root is the
    /// leaf-presence ledger and is NOT the canonical root for proof
    /// verification.
    ///
    /// `poseidon_proof_path` is sent as a flat `Vec<u8>` of `TREE_DEPTH * 32 = 640`
    /// bytes (TREE_DEPTH=20, matching the in-circuit `merkleSiblings[i][TREE_DEPTH]`).
    pub fn update_tree_root(
        ctx: Context<UpdateTreeRoot>,
        schema_hash: [u8; 32],
        new_root: [u8; 32],
        old_leaf: [u8; 32],
        new_leaf: [u8; 32],
        leaf_index: u64,
        poseidon_proof_path: Vec<u8>,
    ) -> Result<()> {
        let binding_info = ctx.accounts.schema_tree_binding.to_account_info();
        require_keys_eq!(
            *binding_info.owner,
            crate::ID,
            ErrorCode::InvalidBindingOwner
        );

        require!(
            poseidon_proof_path.len() == solid_core::TREE_DEPTH * 32,
            ErrorCode::InvalidProofPathLength
        );
        let mut path: Vec<[u8; 32]> = Vec::with_capacity(solid_core::TREE_DEPTH);
        for i in 0..solid_core::TREE_DEPTH {
            let mut s = [0u8; 32];
            s.copy_from_slice(&poseidon_proof_path[i * 32..(i + 1) * 32]);
            path.push(s);
        }

        // SOLID-SEC-080 (NF-03, closed 2026-05-01): pre-update binding-root
        // anchor.  Pre-fix, this ix accepted a (path, new_leaf, new_root)
        // triple that was self-consistent but unconstrained against the
        // BINDING's notion of current state -- a buggy off-chain pipeline
        // could push a path consistent with new_root yet inconsistent with
        // the binding's live root, corrupting subsequent reads.  Now the
        // anchor recomputes (old_leaf, leaf_index, path) against
        // binding.current_root and refuses any mismatch.
        //
        // Fresh-binding sentinel: when binding.current_root == [0; 32] (the
        // post-`initialize_tree_binding` placeholder before any update has
        // landed), the anchor short-circuits Ok() because the empty-tree
        // Poseidon root is non-zero and would otherwise reject every first
        // update.  The downstream self-consistency check below is then the
        // only gate on the first update; subsequent updates have a real
        // root and the anchor arms.  See
        // `solid_light::cpi_helpers::verify_binding_root_anchor` doc.
        {
            let binding_data = binding_info.try_borrow_data()?;
            require!(
                binding_data.len() >= SCHEMA_TREE_BINDING_SIZE
                    && binding_data[0..8] == SCHEMA_TREE_DISCRIMINATOR,
                ErrorCode::InvalidBindingOwner
            );
            let mut current_root = [0u8; 32];
            current_root.copy_from_slice(&binding_data[72..104]);
            solid_light::cpi_helpers::verify_binding_root_anchor(
                &current_root,
                &old_leaf,
                leaf_index,
                &path,
            )
            .map_err(|_| error!(ErrorCode::BindingRootStale))?;
        }

        // SEC-059 self-consistency check (existing): the new state must
        // recompute to new_root from the same path that anchored the old
        // state above.
        let computed_root =
            solid_light::cpi_helpers::compute_poseidon_merkle_root(&new_leaf, leaf_index, &path)
                .map_err(|_| error!(ErrorCode::TreeRootMismatch))?;
        require!(computed_root == new_root, ErrorCode::TreeRootMismatch);

        let mut data = binding_info.try_borrow_mut_data()?;
        let signer_bytes = ctx.accounts.authority.key().to_bytes();
        apply_schema_tree_root_update(
            &mut data,
            &schema_hash,
            &signer_bytes,
            &new_root,
            Clock::get()?.slot,
        )
    }

    /// Allow the authority to freeze the binding (emergency stop).  A frozen
    /// binding causes every verifier proof that references it to fail
    /// `verify_schema_root_binding`.
    pub fn set_binding_status(
        ctx: Context<UpdateTreeRoot>,
        _schema_hash: [u8; 32],
        status: u8,
    ) -> Result<()> {
        let binding_info = ctx.accounts.schema_tree_binding.to_account_info();
        require_keys_eq!(
            *binding_info.owner,
            crate::ID,
            ErrorCode::InvalidBindingOwner
        );
        let mut data = binding_info.try_borrow_mut_data()?;
        let signer_bytes = ctx.accounts.authority.key().to_bytes();
        apply_schema_tree_status_update(&mut data, &signer_bytes, status)
    }

    /// Allocate and initialize the singleton `GlobalStateBinding` PDA.
    ///
    /// The global binding mirrors the root of the identity-state tree
    /// consumed by the verifier as the `globalStateRoot` public input.
    pub fn initialize_global_binding(ctx: Context<InitializeGlobalBinding>) -> Result<()> {
        let binding_info = ctx.accounts.global_binding.to_account_info();
        let rent = Rent::get()?;
        let lamports = rent.minimum_balance(GLOBAL_STATE_BINDING_SIZE);

        let signer_seeds: &[&[u8]] = &[b"global-binding", &[ctx.bumps.global_binding]];
        let signer_seeds_all: &[&[&[u8]]] = &[signer_seeds];

        anchor_lang::solana_program::program::invoke_signed(
            &system_instruction::create_account(
                &ctx.accounts.authority.key(),
                &binding_info.key(),
                lamports,
                GLOBAL_STATE_BINDING_SIZE as u64,
                &crate::ID,
            ),
            &[
                ctx.accounts.authority.to_account_info(),
                binding_info.clone(),
                ctx.accounts.system_program.to_account_info(),
            ],
            signer_seeds_all,
        )?;

        let mut data = binding_info.try_borrow_mut_data()?;
        data[0..8].copy_from_slice(&GLOBAL_ROOT_DISCRIMINATOR);
        data[8..40].copy_from_slice(&[0u8; 32]);
        data[40..48].copy_from_slice(&Clock::get()?.slot.to_le_bytes());
        data[48..80].copy_from_slice(&ctx.accounts.authority.key().to_bytes());
        msg!("GlobalStateBinding initialised");
        Ok(())
    }

    /// Update the singleton global-state root with on-chain Poseidon
    /// integrity check.  Mirrors `update_tree_root`'s SEC-059-sibling
    /// closure (2026-04-30) for the credential tree.  Caller supplies
    /// (new_root, new_leaf, leaf_index, poseidon_proof_path); on-chain
    /// Poseidon-Merkle recompute refuses any push that does not equal
    /// the recomputed root.
    ///
    /// Authority-gated (matches the authority recorded at
    /// `initialize_global_binding`); the integrity check is a NEW gate
    /// closing the SEC-059 root-injection vector for the global tree.
    pub fn update_global_root(
        ctx: Context<UpdateGlobalRoot>,
        new_root: [u8; 32],
        old_leaf: [u8; 32],
        new_leaf: [u8; 32],
        leaf_index: u64,
        poseidon_proof_path: Vec<u8>,
    ) -> Result<()> {
        let binding_info = ctx.accounts.global_binding.to_account_info();
        require_keys_eq!(
            *binding_info.owner,
            crate::ID,
            ErrorCode::InvalidBindingOwner
        );

        require!(
            poseidon_proof_path.len() == solid_core::TREE_DEPTH * 32,
            ErrorCode::InvalidProofPathLength
        );
        let mut path: Vec<[u8; 32]> = Vec::with_capacity(solid_core::TREE_DEPTH);
        for i in 0..solid_core::TREE_DEPTH {
            let mut s = [0u8; 32];
            s.copy_from_slice(&poseidon_proof_path[i * 32..(i + 1) * 32]);
            path.push(s);
        }

        // SOLID-SEC-080 (NF-03, closed 2026-05-01): pre-update binding-root
        // anchor.  See `update_tree_root` above for the full rationale; the
        // global binding's current_root lives at bytes [8..40) in this PDA
        // (vs [72..104) for the schema_tree binding).
        {
            let binding_data = binding_info.try_borrow_data()?;
            require!(
                binding_data.len() >= GLOBAL_STATE_BINDING_SIZE
                    && binding_data[0..8] == GLOBAL_ROOT_DISCRIMINATOR,
                ErrorCode::InvalidBindingOwner
            );
            let mut current_root = [0u8; 32];
            current_root.copy_from_slice(&binding_data[8..40]);
            solid_light::cpi_helpers::verify_binding_root_anchor(
                &current_root,
                &old_leaf,
                leaf_index,
                &path,
            )
            .map_err(|_| error!(ErrorCode::BindingRootStale))?;
        }

        let computed_root =
            solid_light::cpi_helpers::compute_poseidon_merkle_root(&new_leaf, leaf_index, &path)
                .map_err(|_| error!(ErrorCode::GlobalRootMismatch))?;
        require!(computed_root == new_root, ErrorCode::GlobalRootMismatch);

        let mut data = binding_info.try_borrow_mut_data()?;
        let signer_bytes = ctx.accounts.authority.key().to_bytes();
        apply_global_root_update(&mut data, &signer_bytes, &new_root, Clock::get()?.slot)
    }

    /// Transfer `authority` on a `SchemaTreeBinding` PDA to a new Pubkey.
    ///
    /// Closes the operational-key-rotation gap: without this instruction the
    /// original schema-binding authority was immutable, so a compromised or
    /// lost key would brick the binding (and every proof against that schema).
    pub fn transfer_tree_binding_authority(
        ctx: Context<UpdateTreeRoot>,
        _schema_hash: [u8; 32],
        new_authority: Pubkey,
    ) -> Result<()> {
        let binding_info = ctx.accounts.schema_tree_binding.to_account_info();
        require_keys_eq!(
            *binding_info.owner,
            crate::ID,
            ErrorCode::InvalidBindingOwner
        );
        let mut data = binding_info.try_borrow_mut_data()?;
        let signer_bytes = ctx.accounts.authority.key().to_bytes();
        apply_schema_tree_authority_rotation(&mut data, &signer_bytes, &new_authority.to_bytes())?;
        msg!("SchemaTreeBinding authority rotated to {}", new_authority);
        Ok(())
    }

    /// Transfer the global-state binding authority.
    pub fn transfer_global_binding_authority(
        ctx: Context<UpdateGlobalRoot>,
        new_authority: Pubkey,
    ) -> Result<()> {
        let binding_info = ctx.accounts.global_binding.to_account_info();
        require_keys_eq!(
            *binding_info.owner,
            crate::ID,
            ErrorCode::InvalidBindingOwner
        );
        let mut data = binding_info.try_borrow_mut_data()?;
        let signer_bytes = ctx.accounts.authority.key().to_bytes();
        apply_global_authority_rotation(&mut data, &signer_bytes, &new_authority.to_bytes())?;
        msg!("GlobalStateBinding authority rotated to {}", new_authority);
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(name: String, version: u8)]
pub struct RegisterSchema<'info> {
    #[account(
        init, payer = authority,
        space = 8 + SCHEMA_ACCOUNT_SPACE,
        seeds = [b"schema", name.as_bytes(), &[version]],
        bump
    )]
    pub schema_account: Account<'info, SchemaAccount>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DeprecateSchema<'info> {
    #[account(mut, constraint = schema_account.authority == authority.key())]
    pub schema_account: Account<'info, SchemaAccount>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct IncrementUsage<'info> {
    #[account(mut)]
    pub schema_account: Account<'info, SchemaAccount>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(schema_hash: [u8; 32], tree_pubkey: Pubkey)]
pub struct InitializeTreeBinding<'info> {
    /// The schema this binding refers to.
    pub schema_account: Account<'info, SchemaAccount>,

    /// The binding PDA — uninitialized, allocated by this ix.
    /// We use `UncheckedAccount` because we write a custom 145-byte layout
    /// with literal discriminator bytes, NOT an Anchor-hash discriminator.
    /// CHECK: seed-constrained and program-owned on write.
    #[account(
        mut,
        seeds = [b"schema-tree-binding", schema_hash.as_ref()],
        bump,
    )]
    pub schema_tree_binding: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(schema_hash: [u8; 32])]
pub struct UpdateTreeRoot<'info> {
    /// CHECK: We parse raw bytes; ownership checked in-handler.
    #[account(
        mut,
        seeds = [b"schema-tree-binding", schema_hash.as_ref()],
        bump,
    )]
    pub schema_tree_binding: UncheckedAccount<'info>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct InitializeGlobalBinding<'info> {
    /// CHECK: Seed-constrained; written by this ix.
    #[account(mut, seeds = [b"global-binding"], bump)]
    pub global_binding: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateGlobalRoot<'info> {
    /// CHECK: parsed from raw bytes.
    #[account(mut, seeds = [b"global-binding"], bump)]
    pub global_binding: UncheckedAccount<'info>,
    pub authority: Signer<'info>,
}

#[account]
pub struct SchemaAccount {
    pub authority: Pubkey,
    pub name: String,
    pub version: u8,
    pub category: String,
    pub field_names: Vec<String>,
    pub schema_hash: [u8; 32],
    pub deprecated: bool,
    pub created_at: i64,
    pub usage_count: u64,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Schema must have 1-8 fields")]
    InvalidFieldCount,
    #[msg("Schema is deprecated")]
    SchemaDeprecated,
    #[msg("Provided schema hash does not match the metadata — registry integrity error")]
    InvalidSchemaHash,
    #[msg("Supplied schema_hash does not match schema_account.schema_hash")]
    SchemaHashMismatch,
    #[msg("Caller is not the tree-binding authority")]
    UnauthorizedTreeBinding,
    #[msg("Binding account is not owned by schema_registry")]
    InvalidBindingOwner,
    #[msg("Binding account is malformed or uninitialized")]
    MalformedBinding,
    #[msg("Binding is frozen; no further updates accepted")]
    BindingFrozen,
    #[msg("Invalid status byte (must be 0 or 1)")]
    InvalidStatus,
    #[msg("Arithmetic overflow")]
    Overflow,
    #[msg("Caller is not the schema authority")]
    UnauthorizedSchemaAuthority,
    #[msg("New root slot must strictly exceed previous last_updated_slot")]
    RootSlotNotMonotonic,
    #[msg("Schema name, category, or field name exceeds permitted length")]
    MetadataTooLong,
    #[msg("On-chain Poseidon evaluation failed")]
    PoseidonFailed,
    #[msg("poseidon_proof_path length must equal solid_core::TREE_DEPTH * 32 (SEC-059 sibling)")]
    InvalidProofPathLength,
    #[msg("Submitted new_root does not match the on-chain Poseidon-Merkle recompute against (new_leaf, leaf_index, poseidon_proof_path) for the schema tree (SEC-059 sibling)")]
    TreeRootMismatch,
    #[msg("Submitted new_root does not match the on-chain Poseidon-Merkle recompute for the global tree (SEC-059 sibling)")]
    GlobalRootMismatch,
    #[msg(
        "Caller-supplied (old_leaf, leaf_index, poseidon_proof_path) does \
         not recompute to the binding's current_root -- stale path or \
         wrong old leaf (SOLID-SEC-080).  Re-fetch the binding state and \
         derive the path against the live root."
    )]
    BindingRootStale,
}

// ─── Host-side tests ───────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn anchor_error_code(err: anchor_lang::error::Error) -> u32 {
        match err {
            anchor_lang::error::Error::AnchorError(ae) => ae.error_code_number,
            other => panic!("expected AnchorError, got: {:?}", other),
        }
    }

    /// Disambiguates `ErrorCode -> u32` (Anchor's `error_code` macro
    /// generates `From<ErrorCode>` for `u32`, `ProgramError`, and
    /// `anchor_lang::error::Error`, so an inline `.into()` won't infer).
    fn ec(code: ErrorCode) -> u32 {
        code.into()
    }

    fn make_schema_tree_binding(
        schema_hash: [u8; 32],
        tree_pubkey: [u8; 32],
        root: [u8; 32],
        slot: u64,
        status: u8,
        authority: [u8; 32],
    ) -> Vec<u8> {
        let mut v = Vec::with_capacity(SCHEMA_TREE_BINDING_SIZE);
        v.extend_from_slice(&SCHEMA_TREE_DISCRIMINATOR);
        v.extend_from_slice(&schema_hash);
        v.extend_from_slice(&tree_pubkey);
        v.extend_from_slice(&root);
        v.extend_from_slice(&slot.to_le_bytes());
        v.push(status);
        v.extend_from_slice(&authority);
        assert_eq!(v.len(), SCHEMA_TREE_BINDING_SIZE);
        v
    }

    fn make_global_binding(root: [u8; 32], slot: u64, authority: [u8; 32]) -> Vec<u8> {
        let mut v = Vec::with_capacity(GLOBAL_STATE_BINDING_SIZE);
        v.extend_from_slice(&GLOBAL_ROOT_DISCRIMINATOR);
        v.extend_from_slice(&root);
        v.extend_from_slice(&slot.to_le_bytes());
        v.extend_from_slice(&authority);
        assert_eq!(v.len(), GLOBAL_STATE_BINDING_SIZE);
        v
    }

    // ─── Layout invariants ─────────────────────────────────────────────────

    #[test]
    fn schema_tree_binding_size_is_145() {
        assert_eq!(SCHEMA_TREE_BINDING_SIZE, 145);
    }

    #[test]
    fn schema_tree_discriminator_is_schmtree() {
        assert_eq!(SCHEMA_TREE_DISCRIMINATOR, *b"schmtree");
    }

    #[test]
    fn global_state_binding_size_is_80() {
        assert_eq!(GLOBAL_STATE_BINDING_SIZE, 80);
    }

    #[test]
    fn global_root_discriminator_is_globroot() {
        assert_eq!(GLOBAL_ROOT_DISCRIMINATOR, *b"globroot");
    }

    #[test]
    fn status_constants_are_distinct_and_zero_active() {
        assert_eq!(STATUS_ACTIVE, 0);
        assert_eq!(STATUS_FROZEN, 1);
        assert_ne!(STATUS_ACTIVE, STATUS_FROZEN);
    }

    #[test]
    fn schema_account_space_is_borsh_consistent() {
        // 32 + 4+64 + 1 + 4+64 + 4 + 8*(4+32) + 32 + 1 + 8 + 8 = 510.
        assert_eq!(SCHEMA_ACCOUNT_SPACE, 510);
    }

    // ─── apply_schema_tree_root_update: positive paths ─────────────────────

    #[test]
    fn schema_tree_root_update_happy_path() {
        let schema = [7u8; 32];
        let auth = [3u8; 32];
        let mut buf =
            make_schema_tree_binding(schema, [0u8; 32], [0u8; 32], 100, STATUS_ACTIVE, auth);
        let new_root = [9u8; 32];
        apply_schema_tree_root_update(&mut buf, &schema, &auth, &new_root, 200).unwrap();
        assert_eq!(&buf[72..104], &new_root[..]);
        assert_eq!(u64::from_le_bytes(buf[104..112].try_into().unwrap()), 200);
        assert_eq!(&buf[8..40], &schema[..]);
        assert_eq!(buf[112], STATUS_ACTIVE);
        assert_eq!(&buf[113..145], &auth[..]);
    }

    // ─── apply_schema_tree_root_update: negative paths ─────────────────────

    #[test]
    fn schema_tree_root_update_rejects_short_buffer() {
        let mut buf = vec![0u8; SCHEMA_TREE_BINDING_SIZE - 1];
        buf[0..8].copy_from_slice(&SCHEMA_TREE_DISCRIMINATOR);
        let err = apply_schema_tree_root_update(&mut buf, &[0u8; 32], &[0u8; 32], &[1u8; 32], 1)
            .unwrap_err();
        assert_eq!(anchor_error_code(err), ec(ErrorCode::MalformedBinding));
    }

    #[test]
    fn schema_tree_root_update_rejects_bad_discriminator() {
        let auth = [3u8; 32];
        let mut buf =
            make_schema_tree_binding([0u8; 32], [0u8; 32], [0u8; 32], 100, STATUS_ACTIVE, auth);
        buf[0..8].copy_from_slice(b"globroot");
        let err = apply_schema_tree_root_update(&mut buf, &[0u8; 32], &auth, &[1u8; 32], 200)
            .unwrap_err();
        assert_eq!(anchor_error_code(err), ec(ErrorCode::MalformedBinding));
    }

    #[test]
    fn schema_tree_root_update_rejects_schema_mismatch() {
        let auth = [3u8; 32];
        let real_schema = [7u8; 32];
        let wrong_schema = [8u8; 32];
        let mut buf =
            make_schema_tree_binding(real_schema, [0u8; 32], [0u8; 32], 100, STATUS_ACTIVE, auth);
        let err = apply_schema_tree_root_update(&mut buf, &wrong_schema, &auth, &[1u8; 32], 200)
            .unwrap_err();
        assert_eq!(anchor_error_code(err), ec(ErrorCode::SchemaHashMismatch));
    }

    #[test]
    fn schema_tree_root_update_rejects_frozen() {
        let auth = [3u8; 32];
        let schema = [7u8; 32];
        let mut buf =
            make_schema_tree_binding(schema, [0u8; 32], [0u8; 32], 100, STATUS_FROZEN, auth);
        let err =
            apply_schema_tree_root_update(&mut buf, &schema, &auth, &[1u8; 32], 200).unwrap_err();
        assert_eq!(anchor_error_code(err), ec(ErrorCode::BindingFrozen));
    }

    #[test]
    fn schema_tree_root_update_rejects_wrong_authority() {
        let auth = [3u8; 32];
        let imposter = [9u8; 32];
        let schema = [7u8; 32];
        let mut buf =
            make_schema_tree_binding(schema, [0u8; 32], [0u8; 32], 100, STATUS_ACTIVE, auth);
        let err = apply_schema_tree_root_update(&mut buf, &schema, &imposter, &[1u8; 32], 200)
            .unwrap_err();
        assert_eq!(
            anchor_error_code(err),
            ec(ErrorCode::UnauthorizedTreeBinding)
        );
    }

    #[test]
    fn schema_tree_root_update_rejects_non_monotonic_slot_equal() {
        let auth = [3u8; 32];
        let schema = [7u8; 32];
        let mut buf =
            make_schema_tree_binding(schema, [0u8; 32], [0u8; 32], 100, STATUS_ACTIVE, auth);
        let err =
            apply_schema_tree_root_update(&mut buf, &schema, &auth, &[1u8; 32], 100).unwrap_err();
        assert_eq!(anchor_error_code(err), ec(ErrorCode::RootSlotNotMonotonic));
    }

    #[test]
    fn schema_tree_root_update_rejects_non_monotonic_slot_lesser() {
        let auth = [3u8; 32];
        let schema = [7u8; 32];
        let mut buf =
            make_schema_tree_binding(schema, [0u8; 32], [0u8; 32], 100, STATUS_ACTIVE, auth);
        let err =
            apply_schema_tree_root_update(&mut buf, &schema, &auth, &[1u8; 32], 50).unwrap_err();
        assert_eq!(anchor_error_code(err), ec(ErrorCode::RootSlotNotMonotonic));
    }

    // ─── apply_schema_tree_status_update ──────────────────────────────────

    #[test]
    fn schema_tree_status_update_freeze_then_unfreeze() {
        let auth = [3u8; 32];
        let schema = [7u8; 32];
        let mut buf =
            make_schema_tree_binding(schema, [0u8; 32], [0u8; 32], 1, STATUS_ACTIVE, auth);
        apply_schema_tree_status_update(&mut buf, &auth, STATUS_FROZEN).unwrap();
        assert_eq!(buf[112], STATUS_FROZEN);
        apply_schema_tree_status_update(&mut buf, &auth, STATUS_ACTIVE).unwrap();
        assert_eq!(buf[112], STATUS_ACTIVE);
    }

    #[test]
    fn schema_tree_status_update_rejects_invalid_status_byte() {
        let auth = [3u8; 32];
        let mut buf =
            make_schema_tree_binding([0u8; 32], [0u8; 32], [0u8; 32], 1, STATUS_ACTIVE, auth);
        for bad in [2u8, 0xFF, 0x10] {
            let err = apply_schema_tree_status_update(&mut buf, &auth, bad).unwrap_err();
            assert_eq!(
                anchor_error_code(err),
                ec(ErrorCode::InvalidStatus),
                "status byte {} must be rejected",
                bad
            );
        }
    }

    #[test]
    fn schema_tree_status_update_rejects_imposter_authority() {
        let auth = [3u8; 32];
        let imposter = [9u8; 32];
        let mut buf =
            make_schema_tree_binding([0u8; 32], [0u8; 32], [0u8; 32], 1, STATUS_ACTIVE, auth);
        let err = apply_schema_tree_status_update(&mut buf, &imposter, STATUS_FROZEN).unwrap_err();
        assert_eq!(
            anchor_error_code(err),
            ec(ErrorCode::UnauthorizedTreeBinding)
        );
        assert_eq!(buf[112], STATUS_ACTIVE);
    }

    // ─── apply_schema_tree_authority_rotation ─────────────────────────────

    #[test]
    fn schema_tree_authority_rotation_happy_path() {
        let old_auth = [3u8; 32];
        let new_auth = [7u8; 32];
        let mut buf =
            make_schema_tree_binding([0u8; 32], [0u8; 32], [0u8; 32], 1, STATUS_ACTIVE, old_auth);
        apply_schema_tree_authority_rotation(&mut buf, &old_auth, &new_auth).unwrap();
        assert_eq!(&buf[113..145], &new_auth[..]);
    }

    #[test]
    fn schema_tree_authority_rotation_rejects_imposter() {
        let old_auth = [3u8; 32];
        let imposter = [9u8; 32];
        let new_auth = [7u8; 32];
        let mut buf =
            make_schema_tree_binding([0u8; 32], [0u8; 32], [0u8; 32], 1, STATUS_ACTIVE, old_auth);
        let err = apply_schema_tree_authority_rotation(&mut buf, &imposter, &new_auth).unwrap_err();
        assert_eq!(
            anchor_error_code(err),
            ec(ErrorCode::UnauthorizedTreeBinding)
        );
        assert_eq!(&buf[113..145], &old_auth[..]);
    }

    #[test]
    fn schema_tree_authority_rotation_then_old_signer_loses_access() {
        let old_auth = [3u8; 32];
        let new_auth = [7u8; 32];
        let third = [11u8; 32];
        let mut buf =
            make_schema_tree_binding([0u8; 32], [0u8; 32], [0u8; 32], 1, STATUS_ACTIVE, old_auth);
        apply_schema_tree_authority_rotation(&mut buf, &old_auth, &new_auth).unwrap();
        let err = apply_schema_tree_authority_rotation(&mut buf, &old_auth, &third).unwrap_err();
        assert_eq!(
            anchor_error_code(err),
            ec(ErrorCode::UnauthorizedTreeBinding)
        );
        apply_schema_tree_authority_rotation(&mut buf, &new_auth, &third).unwrap();
        assert_eq!(&buf[113..145], &third[..]);
    }

    // ─── apply_global_root_update ─────────────────────────────────────────

    #[test]
    fn global_root_update_happy_path() {
        let auth = [3u8; 32];
        let mut buf = make_global_binding([0u8; 32], 100, auth);
        let new_root = [42u8; 32];
        apply_global_root_update(&mut buf, &auth, &new_root, 200).unwrap();
        assert_eq!(&buf[8..40], &new_root[..]);
        assert_eq!(u64::from_le_bytes(buf[40..48].try_into().unwrap()), 200);
        assert_eq!(&buf[48..80], &auth[..]);
    }

    #[test]
    fn global_root_update_rejects_short_buffer() {
        let mut buf = vec![0u8; GLOBAL_STATE_BINDING_SIZE - 1];
        buf[0..8].copy_from_slice(&GLOBAL_ROOT_DISCRIMINATOR);
        let err = apply_global_root_update(&mut buf, &[0u8; 32], &[1u8; 32], 200).unwrap_err();
        assert_eq!(anchor_error_code(err), ec(ErrorCode::MalformedBinding));
    }

    #[test]
    fn global_root_update_rejects_bad_discriminator() {
        let auth = [3u8; 32];
        let mut buf = make_global_binding([0u8; 32], 100, auth);
        buf[0..8].copy_from_slice(b"schmtree");
        let err = apply_global_root_update(&mut buf, &auth, &[1u8; 32], 200).unwrap_err();
        assert_eq!(anchor_error_code(err), ec(ErrorCode::MalformedBinding));
    }

    #[test]
    fn global_root_update_rejects_imposter() {
        let auth = [3u8; 32];
        let imposter = [9u8; 32];
        let mut buf = make_global_binding([0u8; 32], 100, auth);
        let err = apply_global_root_update(&mut buf, &imposter, &[1u8; 32], 200).unwrap_err();
        assert_eq!(
            anchor_error_code(err),
            ec(ErrorCode::UnauthorizedTreeBinding)
        );
    }

    #[test]
    fn global_root_update_rejects_non_monotonic_slot() {
        let auth = [3u8; 32];
        let mut buf = make_global_binding([0u8; 32], 100, auth);
        let err = apply_global_root_update(&mut buf, &auth, &[1u8; 32], 100).unwrap_err();
        assert_eq!(anchor_error_code(err), ec(ErrorCode::RootSlotNotMonotonic));
    }

    // ─── apply_global_authority_rotation ──────────────────────────────────

    #[test]
    fn global_authority_rotation_happy_path() {
        let old_auth = [3u8; 32];
        let new_auth = [7u8; 32];
        let mut buf = make_global_binding([0u8; 32], 100, old_auth);
        apply_global_authority_rotation(&mut buf, &old_auth, &new_auth).unwrap();
        assert_eq!(&buf[48..80], &new_auth[..]);
    }

    #[test]
    fn global_authority_rotation_rejects_imposter() {
        let old_auth = [3u8; 32];
        let imposter = [9u8; 32];
        let new_auth = [7u8; 32];
        let mut buf = make_global_binding([0u8; 32], 100, old_auth);
        let err = apply_global_authority_rotation(&mut buf, &imposter, &new_auth).unwrap_err();
        assert_eq!(
            anchor_error_code(err),
            ec(ErrorCode::UnauthorizedTreeBinding)
        );
        assert_eq!(&buf[48..80], &old_auth[..]);
    }

    // ─── Cross-pollution sanity ───────────────────────────────────────────

    #[test]
    fn schema_helpers_reject_global_layout() {
        let auth = [3u8; 32];
        let mut buf = make_global_binding([0u8; 32], 100, auth);
        let err = apply_schema_tree_root_update(&mut buf, &[0u8; 32], &auth, &[1u8; 32], 200)
            .unwrap_err();
        assert_eq!(anchor_error_code(err), ec(ErrorCode::MalformedBinding));
        let err2 = apply_schema_tree_status_update(&mut buf, &auth, STATUS_FROZEN).unwrap_err();
        assert_eq!(anchor_error_code(err2), ec(ErrorCode::MalformedBinding));
    }

    #[test]
    fn global_helpers_reject_schema_layout() {
        let auth = [3u8; 32];
        let mut buf =
            make_schema_tree_binding([0u8; 32], [0u8; 32], [0u8; 32], 100, STATUS_ACTIVE, auth);
        let err = apply_global_root_update(&mut buf, &auth, &[1u8; 32], 200).unwrap_err();
        assert_eq!(anchor_error_code(err), ec(ErrorCode::MalformedBinding));
    }
}
