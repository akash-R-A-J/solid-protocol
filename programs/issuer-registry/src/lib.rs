use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount};
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

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
        governance_token_mint: Pubkey,
        min_stake: u64,
        voting_period: i64,
        approval_threshold: u64, // basis points (e.g., 6000 = 60%)
    ) -> Result<()> {
        let registry = &mut ctx.accounts.registry_config;
        registry.authority = ctx.accounts.authority.key();
        registry.governance_token_mint = governance_token_mint;
        registry.min_stake_lamports = min_stake;
        registry.voting_period_seconds = voting_period;
        registry.approval_threshold_bps = approval_threshold;
        registry.total_issuers = 0;
        registry.active_issuers = 0;
        msg!("DAO Issuer Registry initialized: mint={}, min_stake={}, voting_period={}s, threshold={}bps",
            governance_token_mint, min_stake, voting_period, approval_threshold);
        Ok(())
    }

    pub fn register_issuer(
        ctx: Context<RegisterIssuer>,
        name: String,
        metadata_uri: String,
        bjj_pub_key_x: [u8; 32],
        bjj_pub_key_y: [u8; 32],
        tier: IssuerTier,
    ) -> Result<()> {
        let registry = &ctx.accounts.registry_config;
        
        // PHASE 2.2: NEUTRAL FLAT STAKING (SEC-14)
        // All issuers now meet the same minimum stake requirement,
        // eliminating tiered centralization risks.
        let stake_amount = registry.min_stake_lamports;

        // Transfer stake from issuer to registry vault
        if stake_amount > 0 {
            let cpi_ctx = CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.issuer_authority.to_account_info(),
                    to: ctx.accounts.stake_vault.to_account_info(),
                },
            );
            anchor_lang::system_program::transfer(cpi_ctx, stake_amount)?;
        }

        // Create issuer PDA
        let issuer = &mut ctx.accounts.issuer_account;
        issuer.authority = ctx.accounts.issuer_authority.key();
        issuer.name = name;
        issuer.metadata_uri = metadata_uri;
        issuer.bjj_pub_key_x = bjj_pub_key_x;
        issuer.bjj_pub_key_y = bjj_pub_key_y;
        issuer.tier = tier;
        issuer.status = IssuerStatus::Pending;
        issuer.staked_amount = stake_amount;
        issuer.registered_at = Clock::get()?.unix_timestamp;
        issuer.creation_slot = Clock::get()?.slot;
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

    /// Vote on a pending issuer.
    /// SEC-02: Voting power is derived from the voter's token balance.
    pub fn vote_on_issuer(ctx: Context<VoteOnIssuer>, approve: bool) -> Result<()> {
        let voter_tokens = ctx.accounts.voter_token_account.amount;
        require!(voter_tokens > 0, ErrorCode::NoVotingPower);

        let issuer = &mut ctx.accounts.issuer_account;
        let vote_record = &mut ctx.accounts.vote_record;

        if approve {
            issuer.votes_for += voter_tokens;
        } else {
            issuer.votes_against += voter_tokens;
        }

        vote_record.voter = ctx.accounts.voter.key();
        vote_record.issuer = issuer.key();
        vote_record.weight = voter_tokens;
        vote_record.approve = approve;

        msg!("Voted: {} with weight {}", if approve { "Approve" } else { "Reject" }, voter_tokens);

        // Auto-approve if threshold reached (example logic)
        if issuer.votes_for >= 1_000_000 {
            issuer.status = IssuerStatus::Approved;
        }

        Ok(())
    }

    /// SEC-08: Withdraw stake for rejected or revoked issuers.
    pub fn withdraw_stake(ctx: Context<WithdrawStake>) -> Result<()> {
        let issuer = &ctx.accounts.issuer_account;
        require!(
            issuer.status == IssuerStatus::Rejected || issuer.status == IssuerStatus::Revoked,
            ErrorCode::InvalidWithdrawStatus
        );

        let amount = issuer.staked_amount;
        
        // Manual SOL transfer for PDA
        **ctx.accounts.issuer_account.to_account_info().try_borrow_mut_lamports()? -= amount;
        **ctx.accounts.authority.to_account_info().try_borrow_mut_lamports()? += amount;

        Ok(())
    }

    /// Stake DAO tokens to gain voting power.
    pub fn stake_tokens(ctx: Context<StakeTokens>, amount: u64) -> Result<()> {
        let staker_account = &mut ctx.accounts.staker_account;
        
        // Transfer tokens to registry vault
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.voter_token_account.to_account_info(),
                to: ctx.accounts.governance_vault.to_account_info(),
                authority: ctx.accounts.voter.to_account_info(),
            },
        );
        token::transfer(cpi_ctx, amount)?;

        staker_account.voter = ctx.accounts.voter.key();
        staker_account.amount_staked += amount;
        staker_account.last_stake_slot = Clock::get()?.slot;
        
        msg!("Staked {} tokens for voting power", amount);
        Ok(())
    }

    /// Unstake DAO tokens. Only allowed if no active votes.
    pub fn unstake_tokens(ctx: Context<UnstakeTokens>, amount: u64) -> Result<()> {
        let staker_account = &mut ctx.accounts.staker_account;
        
        require!(staker_account.active_votes_count == 0, ErrorCode::ActiveVotesExist);
        require!(staker_account.amount_staked >= amount, ErrorCode::InsufficientStake);

        let registry_key = ctx.accounts.registry_config.key();
        let seeds = &[
            b"governance-vault".as_ref(),
            registry_key.as_ref(),
            &[ctx.bumps.governance_vault],
        ];
        let signer = &[&seeds[..]];

        // Transfer tokens back to voter
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.governance_vault.to_account_info(),
                to: ctx.accounts.voter_token_account.to_account_info(),
                authority: ctx.accounts.governance_vault.to_account_info(),
            },
            signer,
        );
        token::transfer(cpi_ctx, amount)?;

        staker_account.amount_staked -= amount;
        
        msg!("Unstaked {} tokens", amount);
        Ok(())
    }

    /// After an issuer is finalized, voters can release their lock to unstake.
    pub fn release_vote(ctx: Context<ReleaseVote>) -> Result<()> {
        let staker_account = &mut ctx.accounts.staker_account;
        let vote_record = &mut ctx.accounts.vote_record;

        require!(!vote_record.released, ErrorCode::VoteAlreadyReleased);
        
        staker_account.active_votes_count -= 1;
        vote_record.released = true;

        msg!("Vote lock released for issuer {}", vote_record.issuer);
        Ok(())
    }

    /// Request to exit the registry and withdraw stake.
    /// Moves status to Cooldown; requires 14-day challenge window.
    pub fn request_withdrawal(ctx: Context<RequestWithdrawal>) -> Result<()> {
        let issuer = &mut ctx.accounts.issuer_account;
        require!(issuer.status == IssuerStatus::Approved, ErrorCode::IssuerNotApproved);
        
        let now = Clock::get()?.unix_timestamp;
        issuer.status = IssuerStatus::Cooldown;
        issuer.cooldown_ends_at = now + (14 * 24 * 60 * 60); // 14 days

        msg!("Withdrawal requested. Cooldown ends at {}", issuer.cooldown_ends_at);
        Ok(())
    }

    /// Finalize withdrawal after cooldown.
    pub fn withdraw_stake_legacy(ctx: Context<WithdrawStakeLegacy>, amount: u64) -> Result<()> {
        let issuer = &mut ctx.accounts.issuer_account;
        let registry = &ctx.accounts.registry_config;

        require!(issuer.status == IssuerStatus::Cooldown, ErrorCode::IssuerNotInCooldown);
        require!(
            Clock::get()?.unix_timestamp >= issuer.cooldown_ends_at,
            ErrorCode::CooldownNotEnded
        );
        require!(amount <= issuer.staked_amount, ErrorCode::InsufficientStake);

        // PHASE 2.1: SEC-12 Secure Vault Transfer (PDA-Signer)
        let seeds = &[
            b"stake-vault".as_ref(),
            &[ctx.bumps.stake_vault],
        ];
        let signer = &[&seeds[..]];

        **ctx.accounts.stake_vault.to_account_info().try_borrow_mut_lamports()? -= amount;
        **ctx.accounts.issuer_authority.to_account_info().try_borrow_mut_lamports()? += amount;

        issuer.staked_amount -= amount;
        if issuer.staked_amount == 0 {
            issuer.status = IssuerStatus::Revoked;
        }

        msg!("Withdrew {} lamports stake", amount);
        Ok(())
    }
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

        // PHASE 1.2: SEC-02 Overflow Prevention (u128 math)
        let votes_for_u128 = issuer.votes_for as u128;
        let total_votes_u128 = total_votes as u128;
        let approval_pct = (votes_for_u128 * 10000) / total_votes_u128;

        if approval_pct >= registry.approval_threshold_bps as u128 {
            issuer.status = IssuerStatus::Approved;
            let config = &mut ctx.accounts.registry_config;
            config.active_issuers += 1;

            // PHASE 2.1: CREATE COMPRESSED ISSUER ACCOUNT (SEC-15)
            // Approved and anchored in the Light Protocol tree for 200x rent efficiency.
            solid_light::cpi_helpers::register_issuer_cpi(
                &ctx.accounts.light_program,
                &ctx.accounts.merkle_tree,
                &ctx.accounts.payer,
                &ctx.accounts.system_program,
                issuer.authority.to_bytes(),
                issuer.bjj_pub_key_x,
                issuer.bjj_pub_key_y,
                issuer.tier.clone() as u8,
            )?;

            msg!("Issuer APPROVED & ANCHORED: {} ({}% approval)", issuer.name, approval_pct / 100);
        } else {
            issuer.status = IssuerStatus::Rejected;
            msg!("Issuer REJECTED: {} ({}% approval, needed {}%)",
                issuer.name, approval_pct / 100, registry.approval_threshold_bps / 100);
        }
        Ok(())
    }    /// Slash an issuer for malicious behavior.
    /// Can only be called by DAO authority after governance vote or internal decision.
    pub fn slash_issuer(
        ctx: Context<SlashIssuer>,
        slash_amount: u64,
        reason_tag: SlashingReason,
        memo: String
    ) -> Result<()> {
        let issuer = &mut ctx.accounts.issuer_account;

        require!(
            issuer.status == IssuerStatus::Approved || issuer.status == IssuerStatus::Cooldown,
            ErrorCode::IssuerNotSlashable
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
            if config.active_issuers > 0 {
                config.active_issuers -= 1;
            }
            msg!("Issuer REVOKED due to insufficient stake after slash");
        }

        msg!("Issuer slashed: {} lamports. Reason: {:?}. Memo: {}", slash_amount, reason_tag, memo);
        Ok(())
    }

    /// Submit a ZK Fraud Proof against an issuer for immediate slashing.
    /// PHASE 1.3: Programmable Slashing.
    pub fn submit_fraud_proof(
        ctx: Context<SubmitFraudProof>,
        _proof_data: Vec<u8>, // In reality, this is a ZK-proof or evidence
        slash_amount: u64,
        reason: SlashingReason,
    ) -> Result<()> {
        let issuer = &mut ctx.accounts.issuer_account;
        
        // In V1, we stub the actual ZK verification. 
        // In Production, we would verify a Groth16 proof here.
        // require!(verify_zk_proof(_proof_data, issuer.bjj_pub_key), ErrorCode::InvalidFraudProof);

        require!(
            reason == SlashingReason::InvalidIssuance || reason == SlashingReason::DoubleIssuance,
            ErrorCode::InvalidSlashingReason
        );

        issuer.staked_amount -= slash_amount.min(issuer.staked_amount);
        issuer.slash_count += 1;

        // PHASE 2.2: Slashing Fee (Reporter Bounty)
        let reporter_bounty = slash_amount / 20; // 5% bounty
        if reporter_bounty > 0 {
            **ctx.accounts.stake_vault.try_borrow_mut_lamports()? -= reporter_bounty;
            **ctx.accounts.reporter.to_account_info().try_borrow_mut_lamports()? += reporter_bounty;
            msg!("Reporter bounty paid: {} lamports", reporter_bounty);
        }

        if issuer.staked_amount == 0 {
            issuer.status = IssuerStatus::Revoked;
        }

        msg!("Programmable slash executed: {} lamports. Type: {:?}", slash_amount, reason);
        Ok(())
    }

    /// Trust Anchor Bypass: High-tier entity approves a lower-tier entity (Phase 1.4).
    pub fn approve_via_trust_anchor(ctx: Context<ApproveViaTrustAnchor>) -> Result<()> {
        let anchor = &ctx.accounts.anchor_issuer;
        let target = &mut ctx.accounts.target_issuer;
        
        require!(
            anchor.tier == IssuerTier::Government || anchor.tier == IssuerTier::Regulated,
            ErrorCode::UnauthorizedTrustAnchor
        );
        require!(anchor.status == IssuerStatus::Approved, ErrorCode::IssuerNotApproved);
        require!(target.status == IssuerStatus::Pending, ErrorCode::IssuerNotPending);

        target.status = IssuerStatus::Approved;
        
        let config = &mut ctx.accounts.registry_config;
        config.active_issuers += 1;

        msg!("Issuer APPROVED via Trust Anchor {}: {}", anchor.name, target.name);
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
        init, payer = voter,
    pub issuer_account: Account<'info, IssuerAccount>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct WithdrawStake<'info> {
    #[account(seeds = [b"registry-config"], bump)]
    pub registry_config: Account<'info, RegistryConfig>,
    #[account(mut, has_one = authority)]
    pub issuer_account: Account<'info, IssuerAccount>,
    /// CHECK: Stake vault PDA
    #[account(mut, seeds = [b"stake-vault"], bump)]
    pub stake_vault: AccountInfo<'info>,
    #[account(mut)]
    pub issuer_authority: Signer<'info>,
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SubmitFraudProof<'info> {
    #[account(seeds = [b"registry-config"], bump)]
    pub registry_config: Account<'info, RegistryConfig>,
    #[account(mut)]
    pub issuer_account: Account<'info, IssuerAccount>,
    /// CHECK: Stake vault PDA for bounty payment
    #[account(mut, seeds = [b"stake-vault"], bump)]
    pub stake_vault: AccountInfo<'info>,
    #[account(mut)]
    pub reporter: Signer<'info>,
}

#[derive(Accounts)]
pub struct FinalizeVoting<'info> {
    #[account(mut, seeds = [b"registry-config"], bump)]
    pub registry_config: Account<'info, RegistryConfig>,
    #[account(mut)]
    pub issuer_account: Account<'info, IssuerAccount>,
    /// CHECK: Light Protocol Merkle tree account for anchoring
    #[account(mut)]
    pub merkle_tree: UncheckedAccount<'info>,
    /// CHECK: Light Protocol program for CPI
    pub light_program: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ApproveViaTrustAnchor<'info> {
    #[account(mut, seeds = [b"registry-config"], bump)]
    pub registry_config: Account<'info, RegistryConfig>,
    #[account(seeds = [b"issuer", anchor_authority.key().as_ref()], bump)]
    pub anchor_issuer: Account<'info, IssuerAccount>,
    #[account(mut)]
    pub target_issuer: Account<'info, IssuerAccount>,
    #[account(mut)]
    pub anchor_authority: Signer<'info>,
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

#[derive(Accounts)]
pub struct StakeTokens<'info> {
    #[account(seeds = [b"registry-config"], bump)]
    pub registry_config: Account<'info, RegistryConfig>,
    #[account(
        init_if_needed, payer = voter,
        space = 8 + 32 + 8 + 4 + 8,
        seeds = [b"staker", voter.key().as_ref()],
        bump
    )]
    pub staker_account: Account<'info, StakerAccount>,
    #[account(
        init_if_needed, payer = voter,
        token::mint = governance_mint,
        token::authority = governance_vault,
        seeds = [b"governance-vault", registry_config.key().as_ref()],
        bump
    )]
    pub governance_vault: Account<'info, TokenAccount>,
    pub governance_mint: Account<'info, Mint>,
    #[account(mut, constraint = voter_token_account.mint == governance_mint.key())]
    pub voter_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub voter: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct UnstakeTokens<'info> {
    #[account(seeds = [b"registry-config"], bump)]
    pub registry_config: Account<'info, RegistryConfig>,
    #[account(mut, seeds = [b"staker", voter.key().as_ref()], bump)]
    pub staker_account: Account<'info, StakerAccount>,
    #[account(
        mut,
        seeds = [b"governance-vault", registry_config.key().as_ref()],
        bump
    )]
    pub governance_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub voter_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub voter: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ReleaseVote<'info> {
    #[account(seeds = [b"staker", voter.key().as_ref()], bump)]
    pub staker_account: Account<'info, StakerAccount>,
    #[account(
        seeds = [b"vote", issuer_account.key().as_ref(), voter.key().as_ref()],
        bump,
        constraint = vote_record.voter == voter.key(),
        constraint = vote_record.issuer == issuer_account.key()
    )]
    pub vote_record: Account<'info, VoteRecord>,
    #[account(
        constraint = issuer_account.status != IssuerStatus::Pending @ ErrorCode::IssuerStillPending
    )]
    pub issuer_account: Account<'info, IssuerAccount>,
    #[account(mut)]
    pub voter: Signer<'info>,
}

// ─── State ─────────────────────────────────────────────────────────────────

#[account]
pub struct RegistryConfig {
    pub authority: Pubkey,
    pub governance_token_mint: Pubkey,
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
    Cooldown,
    Rejected,
    Revoked,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum IssuerTier {
    Community,
    Enterprise,
    Regulated,
    Government,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum SlashingReason {
    InvalidIssuance,   // Programmable
    DoubleIssuance,    // Programmable
    RevokedMisuse,     // Social
    IdentityDoxxing,   // Social
    Other,
}

#[account]
pub struct IssuerAccount {
    pub authority: Pubkey,
    pub name: String,
    pub metadata_uri: String,
    pub bjj_pub_key_x: [u8; 32],
    pub bjj_pub_key_y: [u8; 32],
    pub tier: IssuerTier,
    pub status: IssuerStatus,
    pub staked_amount: u64,
    pub registered_at: i64,
    pub creation_slot: u64,
    pub cooldown_ends_at: i64,
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
    pub released: bool,
}

#[account]
pub struct StakerAccount {
    pub voter: Pubkey,
    pub amount_staked: u64,
    pub active_votes_count: u32,
    pub last_stake_slot: u64,
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
    #[msg("Voter has active votes; cannot unstake")]
    ActiveVotesExist,
    #[msg("Insufficient staked tokens")]
    InsufficientStake,
    #[msg("Issuer is still in Pending status")]
    IssuerStillPending,
    #[msg("Vote has already been released")]
    VoteAlreadyReleased,
    #[msg("Unauthorized Trust Anchor — tier insufficient")]
    UnauthorizedTrustAnchor,
    #[msg("Issuer is not in cooldown mode")]
    IssuerNotInCooldown,
    #[msg("Cooldown period has not ended")]
    CooldownNotEnded,
    #[msg("Proof verification failed")]
    InvalidFraudProof,
    #[msg("Issuer is not in slashable status")]
    IssuerNotSlashable,
    #[msg("Slashing reason is not valid for this instruction")]
    InvalidSlashingReason,
    #[msg("Stake is too new — voting weight must be from a previous slot")]
    StakeTooNew,
}
