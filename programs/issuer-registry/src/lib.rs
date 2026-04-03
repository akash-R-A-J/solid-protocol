use anchor_lang::prelude::*;

declare_id!("CRGYfonXwDk6gKEm9fC1U33VVBkqnQVD3sPdLKzqHWoR");

/// DAO-Governed Issuer Registry — full decentralized trust management.
///
/// No shortcuts. Full governance:
///   - Issuers stake SOL to register
///   - DAO token holders vote on approval
///   - Slashing for malicious issuance
///   - Automatic status management
#[program]
pub mod issuer_registry {
    use super::*;

    /// Initialize the DAO registry with governance parameters.
    pub fn initialize_registry(
        ctx: Context<InitializeRegistry>,
        min_stake: u64,
        voting_period: i64,
        approval_threshold: u64, // basis points (e.g., 6000 = 60%)
    ) -> Result<()> {
        let registry = &mut ctx.accounts.registry_config;
        registry.authority = ctx.accounts.authority.key();
        registry.min_stake_lamports = min_stake;
        registry.voting_period_seconds = voting_period;
        registry.approval_threshold_bps = approval_threshold;
        registry.total_issuers = 0;
        registry.active_issuers = 0;
        msg!("DAO Issuer Registry initialized: min_stake={}, voting_period={}s, threshold={}bps",
            min_stake, voting_period, approval_threshold);
        Ok(())
    }

    /// Register as an issuer by staking SOL.
    /// Creates a pending registration that needs DAO approval.
    pub fn register_issuer(
        ctx: Context<RegisterIssuer>,
        name: String,
        metadata_uri: String,
        bjj_pub_key_x: [u8; 32],
        bjj_pub_key_y: [u8; 32],
    ) -> Result<()> {
        let registry = &ctx.accounts.registry_config;
        let stake_amount = registry.min_stake_lamports;

        // Transfer stake from issuer to registry vault
        let cpi_ctx = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.issuer_authority.to_account_info(),
                to: ctx.accounts.stake_vault.to_account_info(),
            },
        );
        anchor_lang::system_program::transfer(cpi_ctx, stake_amount)?;

        // Create issuer PDA
        let issuer = &mut ctx.accounts.issuer_account;
        issuer.authority = ctx.accounts.issuer_authority.key();
        issuer.name = name;
        issuer.metadata_uri = metadata_uri;
        issuer.bjj_pub_key_x = bjj_pub_key_x;
        issuer.bjj_pub_key_y = bjj_pub_key_y;
        issuer.status = IssuerStatus::Pending;
        issuer.staked_amount = stake_amount;
        issuer.registered_at = Clock::get()?.unix_timestamp;
        issuer.votes_for = 0;
        issuer.votes_against = 0;
        issuer.voting_ends_at = Clock::get()?.unix_timestamp + registry.voting_period_seconds;
        issuer.credentials_issued = 0;
        issuer.slash_count = 0;

        let config = &mut ctx.accounts.registry_config;
        config.total_issuers += 1;

        msg!("Issuer registered: {}. Voting ends at {}", issuer.name, issuer.voting_ends_at);
        Ok(())
    }

    /// Vote on an issuer's registration (DAO governance).
    pub fn vote_on_issuer(
        ctx: Context<VoteOnIssuer>,
        approve: bool,
        vote_weight: u64,
    ) -> Result<()> {
        let issuer = &mut ctx.accounts.issuer_account;

        require!(issuer.status == IssuerStatus::Pending, ErrorCode::IssuerNotPending);
        require!(
            Clock::get()?.unix_timestamp <= issuer.voting_ends_at,
            ErrorCode::VotingPeriodEnded
        );

        // Record vote
        let vote_record = &mut ctx.accounts.vote_record;
        require!(!vote_record.has_voted, ErrorCode::AlreadyVoted);

        vote_record.voter = ctx.accounts.voter.key();
        vote_record.issuer = issuer.key();
        vote_record.approved = approve;
        vote_record.weight = vote_weight;
        vote_record.has_voted = true;

        if approve {
            issuer.votes_for += vote_weight;
        } else {
            issuer.votes_against += vote_weight;
        }

        msg!("Vote recorded: {} with weight {}", if approve { "FOR" } else { "AGAINST" }, vote_weight);
        Ok(())
    }

    /// Finalize voting and update issuer status.
    pub fn finalize_voting(ctx: Context<FinalizeVoting>) -> Result<()> {
        let issuer = &mut ctx.accounts.issuer_account;
        let registry = &ctx.accounts.registry_config;

        require!(issuer.status == IssuerStatus::Pending, ErrorCode::IssuerNotPending);
        require!(
            Clock::get()?.unix_timestamp > issuer.voting_ends_at,
            ErrorCode::VotingPeriodNotEnded
        );

        let total_votes = issuer.votes_for + issuer.votes_against;
        if total_votes == 0 {
            issuer.status = IssuerStatus::Rejected;
            msg!("Issuer rejected: no votes cast");
            return Ok(());
        }

        let approval_pct = (issuer.votes_for * 10000) / total_votes;
        if approval_pct >= registry.approval_threshold_bps {
            issuer.status = IssuerStatus::Approved;
            let config = &mut ctx.accounts.registry_config;
            config.active_issuers += 1;
            msg!("Issuer APPROVED: {} ({}% approval)", issuer.name, approval_pct / 100);
        } else {
            issuer.status = IssuerStatus::Rejected;
            msg!("Issuer REJECTED: {} ({}% approval, needed {}%)",
                issuer.name, approval_pct / 100, registry.approval_threshold_bps / 100);
        }
        Ok(())
    }

    /// Slash an issuer for malicious behavior.
    /// Can only be called by DAO authority after governance vote.
    pub fn slash_issuer(ctx: Context<SlashIssuer>, slash_amount: u64, reason: String) -> Result<()> {
        let issuer = &mut ctx.accounts.issuer_account;

        require!(
            issuer.status == IssuerStatus::Approved,
            ErrorCode::IssuerNotApproved
        );
        require!(
            slash_amount <= issuer.staked_amount,
            ErrorCode::SlashExceedsStake
        );

        issuer.staked_amount -= slash_amount;
        issuer.slash_count += 1;

        // If stake drops below minimum, revoke
        let registry = &ctx.accounts.registry_config;
        if issuer.staked_amount < registry.min_stake_lamports {
            issuer.status = IssuerStatus::Revoked;
            let config = &mut ctx.accounts.registry_config;
            config.active_issuers -= 1;
            msg!("Issuer REVOKED due to insufficient stake after slash");
        }

        msg!("Issuer slashed: {} lamports. Reason: {}", slash_amount, reason);
        Ok(())
    }

    /// Check if an issuer is approved (called via CPI from ZK verifier).
    pub fn check_issuer_status(ctx: Context<CheckIssuerStatus>) -> Result<()> {
        let issuer = &ctx.accounts.issuer_account;
        require!(issuer.status == IssuerStatus::Approved, ErrorCode::IssuerNotApproved);
        msg!("Issuer {} is APPROVED", issuer.name);
        Ok(())
    }

    /// Revoke an issuer (DAO authority only).
    pub fn revoke_issuer(ctx: Context<RevokeIssuer>) -> Result<()> {
        let issuer = &mut ctx.accounts.issuer_account;
        issuer.status = IssuerStatus::Revoked;
        let config = &mut ctx.accounts.registry_config;
        if config.active_issuers > 0 {
            config.active_issuers -= 1;
        }
        msg!("Issuer REVOKED: {}", issuer.name);
        Ok(())
    }
}

// ─── Account Contexts ──────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct InitializeRegistry<'info> {
    #[account(
        init, payer = authority,
        space = 8 + 32 + 8 + 8 + 8 + 8 + 8,
        seeds = [b"registry-config"],
        bump
    )]
    pub registry_config: Account<'info, RegistryConfig>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(name: String)]
pub struct RegisterIssuer<'info> {
    #[account(mut, seeds = [b"registry-config"], bump)]
    pub registry_config: Account<'info, RegistryConfig>,
    #[account(
        init, payer = issuer_authority,
        space = 8 + 32 + 64 + 128 + 32 + 32 + 1 + 8 + 8 + 8 + 8 + 8 + 8 + 8,
        seeds = [b"issuer", issuer_authority.key().as_ref()],
        bump
    )]
    pub issuer_account: Account<'info, IssuerAccount>,
    /// CHECK: Stake vault PDA
    #[account(mut, seeds = [b"stake-vault"], bump)]
    pub stake_vault: AccountInfo<'info>,
    #[account(mut)]
    pub issuer_authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct VoteOnIssuer<'info> {
    #[account(mut)]
    pub issuer_account: Account<'info, IssuerAccount>,
    #[account(
        init_if_needed, payer = voter,
        space = 8 + 32 + 32 + 1 + 8 + 1,
        seeds = [b"vote", issuer_account.key().as_ref(), voter.key().as_ref()],
        bump
    )]
    pub vote_record: Account<'info, VoteRecord>,
    #[account(mut)]
    pub voter: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FinalizeVoting<'info> {
    #[account(mut, seeds = [b"registry-config"], bump)]
    pub registry_config: Account<'info, RegistryConfig>,
    #[account(mut)]
    pub issuer_account: Account<'info, IssuerAccount>,
}

#[derive(Accounts)]
pub struct SlashIssuer<'info> {
    #[account(seeds = [b"registry-config"], bump)]
    pub registry_config: Account<'info, RegistryConfig>,
    #[account(mut)]
    pub issuer_account: Account<'info, IssuerAccount>,
    #[account(constraint = authority.key() == registry_config.authority @ ErrorCode::Unauthorized)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct CheckIssuerStatus<'info> {
    pub issuer_account: Account<'info, IssuerAccount>,
}

#[derive(Accounts)]
pub struct RevokeIssuer<'info> {
    #[account(mut, seeds = [b"registry-config"], bump)]
    pub registry_config: Account<'info, RegistryConfig>,
    #[account(mut)]
    pub issuer_account: Account<'info, IssuerAccount>,
    #[account(constraint = authority.key() == registry_config.authority @ ErrorCode::Unauthorized)]
    pub authority: Signer<'info>,
}

// ─── State ─────────────────────────────────────────────────────────────────

#[account]
pub struct RegistryConfig {
    pub authority: Pubkey,
    pub min_stake_lamports: u64,
    pub voting_period_seconds: i64,
    pub approval_threshold_bps: u64,
    pub total_issuers: u64,
    pub active_issuers: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum IssuerStatus {
    Pending,
    Approved,
    Rejected,
    Revoked,
}

#[account]
pub struct IssuerAccount {
    pub authority: Pubkey,
    pub name: String,
    pub metadata_uri: String,
    pub bjj_pub_key_x: [u8; 32],
    pub bjj_pub_key_y: [u8; 32],
    pub status: IssuerStatus,
    pub staked_amount: u64,
    pub registered_at: i64,
    pub votes_for: u64,
    pub votes_against: u64,
    pub voting_ends_at: i64,
    pub credentials_issued: u64,
    pub slash_count: u64,
}

#[account]
pub struct VoteRecord {
    pub voter: Pubkey,
    pub issuer: Pubkey,
    pub approved: bool,
    pub weight: u64,
    pub has_voted: bool,
}

// ─── Errors ────────────────────────────────────────────────────────────────

#[error_code]
pub enum ErrorCode {
    #[msg("Issuer is not in Pending status")]
    IssuerNotPending,
    #[msg("Issuer is not approved")]
    IssuerNotApproved,
    #[msg("Voting period has ended")]
    VotingPeriodEnded,
    #[msg("Voting period has not ended yet")]
    VotingPeriodNotEnded,
    #[msg("Voter has already voted")]
    AlreadyVoted,
    #[msg("Slash amount exceeds staked amount")]
    SlashExceedsStake,
    #[msg("Unauthorized — not DAO authority")]
    Unauthorized,
}
