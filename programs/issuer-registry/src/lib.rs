use anchor_lang::prelude::*;
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
        require!(name.len() <= 64, ErrorCode::NameTooLong);
        require!(metadata_uri.len() <= 128, ErrorCode::MetadataTooLong);

        // PHASE 5: TIERED STAKING (Risk 3 Mitigation)
        // Graduated skin-in-the-game based on authority level.
        let stake_multiplier: u64 = match tier {
            IssuerTier::Community => 1,
            IssuerTier::Enterprise => 10,
            IssuerTier::Regulated => 5,
            IssuerTier::Government => 0, // Government is exempt from SOL stake
        };
        let registry = &ctx.accounts.registry_config;
        let stake_amount = registry
            .min_stake_lamports
            .checked_mul(stake_multiplier)
            .ok_or(ErrorCode::Overflow)?;

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
    /// SEC-23: Flash-loan protection. Voting power is derived from staked tokens
    /// that have been held for at least 100 slots to prevent one-block hacks.
    pub fn vote_on_issuer(ctx: Context<VoteOnIssuer>, approve: bool) -> Result<()> {
        let staker = &ctx.accounts.staker_account;
        let now_slot = Clock::get()?.slot;

        // SEC-23: Ensure stake is at least 100 slots old
        require!(
            now_slot >= staker.last_stake_slot + 100,
            ErrorCode::StakeTooNew
        );

        let voter_weight = staker.amount_staked;
        require!(voter_weight > 0, ErrorCode::NoVotingPower);

        let issuer = &mut ctx.accounts.issuer_account;
        let vote_record = &mut ctx.accounts.vote_record;

        if approve {
            issuer.votes_for += voter_weight;
        } else {
            issuer.votes_against += voter_weight;
        }

        vote_record.voter = ctx.accounts.voter.key();
        vote_record.issuer = issuer.key();
        vote_record.weight = voter_weight;
        vote_record.approved = approve;
        vote_record.has_voted = true;

        msg!("Voted: {} with weight {} (Stake from slot {})", 
            if approve { "Approve" } else { "Reject" }, voter_weight, staker.last_stake_slot);
        // Auto-approval is handled by `finalize_voting` once the voting period
        // ends — we do NOT short-circuit here to avoid malleability around the
        // configured threshold.
        Ok(())
    }

    /// SEC-08: Refund stake for rejected issuers (no slashing occurred).
    /// Revoked issuers must use `withdraw_after_cooldown` after slashing settles.
    pub fn withdraw_stake(ctx: Context<WithdrawStake>) -> Result<()> {
        let issuer = &mut ctx.accounts.issuer_account;
        require!(
            issuer.status == IssuerStatus::Rejected,
            ErrorCode::InvalidWithdrawStatus
        );
        let amount = issuer.staked_amount;
        issuer.staked_amount = 0;

        **ctx.accounts.stake_vault.to_account_info().try_borrow_mut_lamports()? -= amount;
        **ctx.accounts.issuer_authority.to_account_info().try_borrow_mut_lamports()? += amount;
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

    /// Request to exit the registry. Moves status to Cooldown and starts the
    /// 14-day challenge window before the issuer can withdraw their stake.
    pub fn request_withdrawal(ctx: Context<RequestWithdrawal>) -> Result<()> {
        let issuer = &mut ctx.accounts.issuer_account;
        require!(issuer.status == IssuerStatus::Approved, ErrorCode::IssuerNotApproved);

        let now = Clock::get()?.unix_timestamp;
        issuer.status = IssuerStatus::Cooldown;
        issuer.cooldown_ends_at = now + 14 * 24 * 60 * 60;

        msg!("Withdrawal requested. Cooldown ends at {}", issuer.cooldown_ends_at);
        Ok(())
    }

    /// Finalize withdrawal of `amount` lamports after the cooldown expires.
    pub fn withdraw_after_cooldown(
        ctx: Context<WithdrawAfterCooldown>,
        amount: u64,
    ) -> Result<()> {
        let issuer = &mut ctx.accounts.issuer_account;
        require!(issuer.status == IssuerStatus::Cooldown, ErrorCode::IssuerNotInCooldown);
        require!(
            Clock::get()?.unix_timestamp >= issuer.cooldown_ends_at,
            ErrorCode::CooldownNotEnded
        );
        require!(amount <= issuer.staked_amount, ErrorCode::InsufficientStake);

        **ctx.accounts.stake_vault.to_account_info().try_borrow_mut_lamports()? -= amount;
        **ctx.accounts.issuer_authority.to_account_info().try_borrow_mut_lamports()? += amount;

        issuer.staked_amount = issuer.staked_amount.saturating_sub(amount);
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

            // The off-chain indexer watches `IssuerApproved` and publishes the
            // `CompressedIssuer` leaf to whichever storage backend the protocol
            // is configured with (Light Protocol, SolID-native tree, etc.). This
            // keeps the on-chain path backend-agnostic.
            emit!(IssuerApproved {
                issuer: issuer.key(),
                authority: issuer.authority,
                bjj_pub_key_x: issuer.bjj_pub_key_x,
                bjj_pub_key_y: issuer.bjj_pub_key_y,
                tier: issuer.tier.clone(),
                approval_bps: approval_pct as u64,
                timestamp: Clock::get()?.unix_timestamp,
            });

            msg!("Issuer APPROVED: {} ({}% approval)", issuer.name, approval_pct / 100);
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

    /// Submit a fraud finding against an issuer.
    ///
    /// SECURITY (2026-04 remediation): gated behind DAO `authority`. A signed
    /// attestation from the registry authority (the DAO council multisig) is
    /// required. The previous implementation accepted arbitrary evidence from
    /// any signer, allowing a free stake-drain attack.
    ///
    /// The `proof_data` argument is preserved for forward-compatibility: when
    /// the circuit-backed fraud-proof verifier ships, this function will grow
    /// an alternative authorization path that verifies a Groth16 fraud proof
    /// and then allows *any* reporter to execute the slash.
    pub fn submit_fraud_proof(
        ctx: Context<SubmitFraudProof>,
        proof_data: Vec<u8>,
        slash_amount: u64,
        reason: SlashingReason,
    ) -> Result<()> {
        // Authority-gated path.
        require!(
            ctx.accounts.registry_config.authority == ctx.accounts.reporter.key(),
            ErrorCode::UnauthorizedFraudReporter
        );
        // Defensive cap on the size of supplied evidence.
        require!(proof_data.len() <= 4096, ErrorCode::FraudProofTooLarge);

        let issuer = &mut ctx.accounts.issuer_account;
        require!(
            reason == SlashingReason::InvalidIssuance || reason == SlashingReason::DoubleIssuance,
            ErrorCode::InvalidSlashingReason
        );
        require!(
            slash_amount <= issuer.staked_amount,
            ErrorCode::SlashExceedsStake
        );

        issuer.staked_amount = issuer.staked_amount.saturating_sub(slash_amount);
        issuer.slash_count = issuer.slash_count.saturating_add(1);

        // Reporter bounty is only paid when the reporter is NOT the authority
        // itself (the DAO treasury already controls those funds). Left as 0 here.
        if issuer.staked_amount == 0 {
            issuer.status = IssuerStatus::Revoked;
        }

        msg!(
            "Fraud slash executed on {} — {} lamports. Reason: {:?}",
            issuer.authority,
            slash_amount,
            reason
        );
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
        space = 8 + IssuerAccount::SPACE,
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
    #[account(seeds = [b"registry-config"], bump)]
    pub registry_config: Account<'info, RegistryConfig>,
    #[account(mut)]
    pub issuer_account: Account<'info, IssuerAccount>,
    #[account(
        init, payer = voter,
        space = 8 + 32 + 32 + 1 + 8 + 1 + 1,
        seeds = [b"vote", issuer_account.key().as_ref(), voter.key().as_ref()],
        bump
    )]
    pub vote_record: Account<'info, VoteRecord>,
    #[account(seeds = [b"staker", voter.key().as_ref()], bump)]
    pub staker_account: Account<'info, StakerAccount>,
    #[account(mut)]
    pub voter: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct WithdrawStake<'info> {
    #[account(seeds = [b"registry-config"], bump)]
    pub registry_config: Account<'info, RegistryConfig>,
    #[account(
        mut,
        seeds = [b"issuer", issuer_authority.key().as_ref()], bump,
        constraint = issuer_account.authority == issuer_authority.key() @ ErrorCode::Unauthorized,
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
pub struct RequestWithdrawal<'info> {
    #[account(mut, seeds = [b"issuer", issuer_authority.key().as_ref()], bump)]
    pub issuer_account: Account<'info, IssuerAccount>,
    #[account(mut)]
    pub issuer_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct WithdrawAfterCooldown<'info> {
    #[account(mut, seeds = [b"issuer", issuer_authority.key().as_ref()], bump)]
    pub issuer_account: Account<'info, IssuerAccount>,
    /// CHECK: Stake vault PDA
    #[account(mut, seeds = [b"stake-vault"], bump)]
    pub stake_vault: AccountInfo<'info>,
    #[account(mut)]
    pub issuer_authority: Signer<'info>,
    pub system_program: Program<'info, System>,
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

impl IssuerAccount {
    /// 32 authority
    /// + (4 + 64) name (len-prefixed String, capped by NameTooLong)
    /// + (4 + 128) metadata_uri (len-prefixed String, capped by MetadataTooLong)
    /// + 32 bjj_x + 32 bjj_y
    /// + 1 tier + 1 status
    /// + 8 × 9 numeric fields (staked_amount, registered_at, creation_slot,
    ///    cooldown_ends_at, votes_for, votes_against, voting_ends_at,
    ///    credentials_issued, slash_count)
    pub const SPACE: usize = 32 + (4 + 64) + (4 + 128) + 32 + 32 + 1 + 1 + 8 * 9;
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
    #[msg("No voting power: voter has zero staked tokens")]
    NoVotingPower,
    #[msg("Invalid issuer status for withdrawal")]
    InvalidWithdrawStatus,
    #[msg("Fraud proof submitter is not the DAO authority")]
    UnauthorizedFraudReporter,
    #[msg("Supplied fraud evidence exceeds the 4 KB ceiling")]
    FraudProofTooLarge,
    #[msg("Name too long (max 64 bytes)")]
    NameTooLong,
    #[msg("Metadata URI too long (max 128 bytes)")]
    MetadataTooLong,
    #[msg("Arithmetic overflow")]
    Overflow,
}

// ─── Events ────────────────────────────────────────────────────────────────

#[event]
pub struct IssuerApproved {
    pub issuer: Pubkey,
    pub authority: Pubkey,
    pub bjj_pub_key_x: [u8; 32],
    pub bjj_pub_key_y: [u8; 32],
    pub tier: IssuerTier,
    pub approval_bps: u64,
    pub timestamp: i64,
}
