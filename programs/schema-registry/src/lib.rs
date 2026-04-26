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

        // SEC-06 / SOLID-SEC-002: Verify schema_hash against metadata.
        //
        // Shared derivation with `solid_core::schema::SchemaDefinition::compute_hash`
        // so the on-chain check cannot drift from the off-chain SDK. Any
        // change to the preimage layout must land in BOTH places in the same
        // PR (regression test: `solid_core::schema::tests::test_compute_schema_hash_parts_matches_definition`).
        let computed_hash =
            solid_core::schema::compute_schema_hash_from_parts(&name, version, field_names.len())
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
    pub fn update_tree_root(
        ctx: Context<UpdateTreeRoot>,
        schema_hash: [u8; 32],
        new_root: [u8; 32],
    ) -> Result<()> {
        let binding_info = ctx.accounts.schema_tree_binding.to_account_info();
        require_keys_eq!(
            *binding_info.owner,
            crate::ID,
            ErrorCode::InvalidBindingOwner
        );
        let mut data = binding_info.try_borrow_mut_data()?;
        require!(
            data.len() >= SCHEMA_TREE_BINDING_SIZE,
            ErrorCode::MalformedBinding
        );
        require!(
            data[0..8] == SCHEMA_TREE_DISCRIMINATOR,
            ErrorCode::MalformedBinding
        );
        // Schema-hash sanity — prevents refreshing the wrong binding.
        require!(
            &data[8..40] == schema_hash.as_slice(),
            ErrorCode::SchemaHashMismatch
        );
        // Status must be active.
        require!(data[112] == STATUS_ACTIVE, ErrorCode::BindingFrozen);
        // Authority gate.
        let stored_authority: [u8; 32] = data[113..145].try_into().unwrap();
        require!(
            stored_authority == ctx.accounts.authority.key().to_bytes(),
            ErrorCode::UnauthorizedTreeBinding
        );

        // Monotonicity: a binding's root may only advance in slot time.
        // Without this, a compromised authority (or a replayed tx) could
        // regress the root back to a value that predates a revocation,
        // allowing a revoked credential's inclusion proof to pass again.
        let last_slot_bytes: [u8; 8] = data[104..112].try_into().unwrap();
        let last_slot = u64::from_le_bytes(last_slot_bytes);
        let now_slot = Clock::get()?.slot;
        require!(now_slot > last_slot, ErrorCode::RootSlotNotMonotonic);

        data[72..104].copy_from_slice(&new_root);
        data[104..112].copy_from_slice(&now_slot.to_le_bytes());
        Ok(())
    }

    /// Allow the authority to freeze the binding (emergency stop).  A frozen
    /// binding causes every verifier proof that references it to fail
    /// `verify_schema_root_binding`.
    pub fn set_binding_status(
        ctx: Context<UpdateTreeRoot>,
        _schema_hash: [u8; 32],
        status: u8,
    ) -> Result<()> {
        require!(
            status == STATUS_ACTIVE || status == STATUS_FROZEN,
            ErrorCode::InvalidStatus
        );
        let binding_info = ctx.accounts.schema_tree_binding.to_account_info();
        require_keys_eq!(
            *binding_info.owner,
            crate::ID,
            ErrorCode::InvalidBindingOwner
        );
        let mut data = binding_info.try_borrow_mut_data()?;
        require!(
            data.len() >= SCHEMA_TREE_BINDING_SIZE && data[0..8] == SCHEMA_TREE_DISCRIMINATOR,
            ErrorCode::MalformedBinding
        );
        let stored_authority: [u8; 32] = data[113..145].try_into().unwrap();
        require!(
            stored_authority == ctx.accounts.authority.key().to_bytes(),
            ErrorCode::UnauthorizedTreeBinding
        );
        data[112] = status;
        Ok(())
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

    /// Update the singleton global-state root.  Authority-gated (matches the
    /// authority recorded at `initialize_global_binding`).
    pub fn update_global_root(ctx: Context<UpdateGlobalRoot>, new_root: [u8; 32]) -> Result<()> {
        let binding_info = ctx.accounts.global_binding.to_account_info();
        require_keys_eq!(
            *binding_info.owner,
            crate::ID,
            ErrorCode::InvalidBindingOwner
        );
        let mut data = binding_info.try_borrow_mut_data()?;
        require!(
            data.len() >= GLOBAL_STATE_BINDING_SIZE,
            ErrorCode::MalformedBinding
        );
        require!(
            data[0..8] == GLOBAL_ROOT_DISCRIMINATOR,
            ErrorCode::MalformedBinding
        );
        let stored_authority: [u8; 32] = data[48..80].try_into().unwrap();
        require!(
            stored_authority == ctx.accounts.authority.key().to_bytes(),
            ErrorCode::UnauthorizedTreeBinding
        );

        // Same monotonicity guarantee as update_tree_root.
        let last_slot_bytes: [u8; 8] = data[40..48].try_into().unwrap();
        let last_slot = u64::from_le_bytes(last_slot_bytes);
        let now_slot = Clock::get()?.slot;
        require!(now_slot > last_slot, ErrorCode::RootSlotNotMonotonic);

        data[8..40].copy_from_slice(&new_root);
        data[40..48].copy_from_slice(&now_slot.to_le_bytes());
        Ok(())
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
        require!(
            data.len() >= SCHEMA_TREE_BINDING_SIZE && data[0..8] == SCHEMA_TREE_DISCRIMINATOR,
            ErrorCode::MalformedBinding
        );
        let stored_authority: [u8; 32] = data[113..145].try_into().unwrap();
        require!(
            stored_authority == ctx.accounts.authority.key().to_bytes(),
            ErrorCode::UnauthorizedTreeBinding
        );
        data[113..145].copy_from_slice(&new_authority.to_bytes());
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
        require!(
            data.len() >= GLOBAL_STATE_BINDING_SIZE && data[0..8] == GLOBAL_ROOT_DISCRIMINATOR,
            ErrorCode::MalformedBinding
        );
        let stored_authority: [u8; 32] = data[48..80].try_into().unwrap();
        require!(
            stored_authority == ctx.accounts.authority.key().to_bytes(),
            ErrorCode::UnauthorizedTreeBinding
        );
        data[48..80].copy_from_slice(&new_authority.to_bytes());
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
}
