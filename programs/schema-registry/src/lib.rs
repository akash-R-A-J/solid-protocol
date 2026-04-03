use anchor_lang::prelude::*;

declare_id!("DPk6XUH6CArLWt4KMqJmpNBnPwQ3gG9P3dBd3MDVE3bT");

/// Schema Registry — modular schema management for credential verticals.
#[program]
pub mod schema_registry {
    use super::*;

    /// Register a new schema.
    pub fn register_schema(
        ctx: Context<RegisterSchema>,
        name: String,
        version: u8,
        category: String,
        field_names: Vec<String>,
        schema_hash: [u8; 32],
    ) -> Result<()> {
        require!(field_names.len() >= 1 && field_names.len() <= 8, ErrorCode::InvalidFieldCount);

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

    /// Increment usage counter (called via CPI during credential issuance).
    pub fn increment_usage(ctx: Context<IncrementUsage>) -> Result<()> {
        let schema = &mut ctx.accounts.schema_account;
        require!(!schema.deprecated, ErrorCode::SchemaDeprecated);
        schema.usage_count += 1;
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(name: String, version: u8)]
pub struct RegisterSchema<'info> {
    #[account(
        init, payer = authority,
        space = 8 + 32 + 64 + 1 + 64 + 256 + 32 + 1 + 8 + 8,
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
}
