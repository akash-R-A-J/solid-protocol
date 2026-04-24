use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::{invoke, invoke_signed},
    pubkey::Pubkey as SolPubkey,
    system_instruction,
};
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use solid_light::cpi_helpers::{
    verify_schema_tree_binding_for_issue, LightError, SCHEMA_REGISTRY_ID,
};
// SOLID-SEC-003: bring in the typed `SchemaAccount` from schema-registry
// so Anchor auto-verifies the PDA's discriminator, owner program, and
// Borsh layout -- no hand-parsing, no drift.
use schema_registry::SchemaAccount;

declare_id!("CRGYfonXwDk6gKEm9fC1U33VVBkqnQVD3sPdLKzqHWoR");

// ─── SPL Account Compression integration (R-2) ─────────────────────────────
//
// We integrate with SPL Account Compression by hand-rolling the `append` CPI
// rather than importing the `spl-account-compression` crate.  Two reasons:
//
//   1. Dep-graph hygiene — SPL AC transitively pulls a large tree of
//      `borsh`, `bytemuck`, and `solana-program` versions that often fight
//      with Anchor's own pins.  A 40-byte instruction is not worth that risk.
//   2. Upgrade resilience — the on-wire instruction (discriminator + 32-byte
//      leaf + 3 accounts) has been stable since v0.4 of spl-account-compression
//      and is unlikely to change.  Pinning the wire format in one place is
//      easier to audit than pinning a transitive crate.
//
// Program IDs (same on mainnet, devnet, localnet):

/// `cmtDvXumGCrqC1Age74AVPhSRVXJMd8PJS91L8KbNCK`
pub mod spl_account_compression_id {
    anchor_lang::declare_id!("cmtDvXumGCrqC1Age74AVPhSRVXJMd8PJS91L8KbNCK");
}

/// `noopb9bkMVfRPU8AsbpTUg8AQkHtKwMYZiFUjNRtMmV`
pub mod spl_noop_id {
    anchor_lang::declare_id!("noopb9bkMVfRPU8AsbpTUg8AQkHtKwMYZiFUjNRtMmV");
}

/// Anchor discriminator for `spl_account_compression::append`:
/// `sha256("global:append")[..8]`.  Pinning here rather than recomputing on
/// every call keeps the CPI allocation-free.
pub const SPL_AC_APPEND_DISCRIMINATOR: [u8; 8] =
    [0x95, 0x78, 0x12, 0xde, 0xec, 0xe1, 0x58, 0xcb];

/// Anchor discriminator for `spl_account_compression::replace_leaf`:
/// `sha256("global:replace_leaf")[..8]`.  Used by
/// `revoke_issuer_atomic` (ADR-0014).
pub const SPL_AC_REPLACE_LEAF_DISCRIMINATOR: [u8; 8] =
    [0xe3, 0x88, 0x6a, 0x74, 0x10, 0xe4, 0xe8, 0x2c];

/// Compute the ADR-0014 issuer-tree leaf for a given `IssuerAccount`:
/// `Poseidon(5)(authority, bjj_x, bjj_y, status_epoch, revocation_nonce)`.
///
/// The byte-order convention (raw 32-byte arrays fed directly into
/// `solid_core::poseidon::hash_bytes`) matches the holder SDK's wire
/// format for the circuit's private inputs.  A mismatch here silently
/// breaks the in-circuit Merkle-membership check for every proof;
/// cross-language vectors cover the regression.
fn compute_issuer_leaf_bytes(issuer: &IssuerAccount) -> Result<[u8; 32]> {
    let status_epoch_fr = solid_core::poseidon::u64_to_fr(issuer.status_epoch);
    let status_epoch_bytes = solid_core::poseidon::fr_to_bytes_le(&status_epoch_fr);
    let rev_nonce_fr = solid_core::poseidon::u64_to_fr(issuer.revocation_nonce);
    let rev_nonce_bytes = solid_core::poseidon::fr_to_bytes_le(&rev_nonce_fr);

    solid_core::poseidon::hash_bytes(&[
        issuer.authority.to_bytes(),
        issuer.bjj_pub_key_x,
        issuer.bjj_pub_key_y,
        status_epoch_bytes,
        rev_nonce_bytes,
    ])
    .map_err(|_| error!(ErrorCode::PoseidonFailed))
}

/// Seed for the PDA that signs `append` CPIs on behalf of the issuer.
/// The PDA is unique per (schema_hash): one authority per tree.  This means
/// a single schema's tree cannot be appended to by arbitrary callers.
pub const TREE_AUTHORITY_SEED: &[u8] = b"tree-authority";

// ─── Issuer-tree binding (ADR-0014; SEC-004 setup) ─────────────────────────
//
// See `crates/solid-light/src/cpi_helpers.rs` for the canonical byte
// layout and the parser contract.  The binding is a singleton (one
// issuer tree per deployment); the PDA seed is the literal
// `[b"issuer-tree-binding"]`.
//
//   [0..  8)  discriminator = b"issrtree"
//   [8.. 40)  tree_pubkey      (SPL AC concurrent tree)
//   [40.. 72)  current_root
//   [72.. 80)  last_updated_slot (u64 LE)
//   [80.. 81)  status (0 = active, 1 = frozen)
//   [81..113)  authority (Pubkey)

pub const ISSUER_TREE_BINDING_SEED: &[u8] = b"issuer-tree-binding";
pub const ISSUER_TREE_BINDING_SIZE: usize = 113;
pub const ISSUER_TREE_DISCRIMINATOR: [u8; 8] = *b"issrtree";
pub const ISSUER_TREE_STATUS_ACTIVE: u8 = 0;
pub const ISSUER_TREE_STATUS_FROZEN: u8 = 1;

/// Seed for the PDA that will sign `append` / `replace_leaf` CPIs into
/// the issuer-tree's SPL AC account.  Unlike `TREE_AUTHORITY_SEED`
/// (which is per-schema), this seed is a singleton -- there is exactly
/// one issuer tree.  Used from issuer-registry by the status-transition
/// hooks (Phase 2 impl 3); declared here so the address is stable from
/// day one of the scaffold.
pub const ISSUER_TREE_AUTHORITY_SEED: &[u8] = b"issuer-tree-authority";

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
        registry.next_issuer_leaf_index = 0;
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

        // SOLID-SEC-007.  Reject BJJ public keys that are not in the
        // prime-order subgroup (cofactor-8 torsion), off the curve,
        // or the Edwards neutral element.  Without this gate an
        // attacker who registers a small-order pubkey can forge
        // signatures over a tiny subgroup; every downstream
        // credential signed by that key verifies with compromised
        // soundness.  `solid_core::babyjubjub::require_in_prime_
        // order_subgroup` does a full `r * P == O` scalar
        // multiplication.  This runs once per issuer registration;
        // it is a setup-time cost, not a hot path.
        let bjj_pub_key = solid_core::babyjubjub::BJJPublicKey {
            x: bjj_pub_key_x,
            y: bjj_pub_key_y,
        };
        solid_core::babyjubjub::require_in_prime_order_subgroup(&bjj_pub_key)
            .map_err(|_| ErrorCode::InvalidBJJPubKey)?;

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
        // ADR-0014: both counters start at zero.  `status_epoch` bumps
        // to the finalisation slot at first approval (finalize_voting
        // or approve_via_trust_anchor); `revocation_nonce` bumps only
        // on revoke_issuer / slash_issuer / submit_fraud_proof and on
        // re-approval of a previously-revoked issuer.
        issuer.revocation_nonce = 0;
        issuer.status_epoch = 0;
        issuer.issuer_tree_leaf_index = 0;
        issuer.is_tree_enrolled = false;

        let config = &mut ctx.accounts.registry_config;
        config.total_issuers += 1;

        msg!("Issuer registered: {}. Voting ends at {}", issuer.name, issuer.voting_ends_at);
        Ok(())
    }

    /// Vote on a pending issuer.
    ///
    /// Governance invariants (2026-04 remediation):
    ///   - Stake must be at least 100 slots old (flash-loan protection).
    ///   - Current unix time must be strictly less than issuer.voting_ends_at
    ///     (otherwise votes would be cast after the window closed, enabling
    ///      late-vote stuffing before a caller runs finalize_voting).
    ///   - Voter's active_votes_count is incremented so the tokens that backed
    ///     this vote cannot be unstaked until release_vote is called. Without
    ///     this increment a voter could stake, vote, and immediately unstake
    ///     at zero economic cost, making the DAO economically meaningless.
    pub fn vote_on_issuer(ctx: Context<VoteOnIssuer>, approve: bool) -> Result<()> {
        let staker = &ctx.accounts.staker_account;
        let now_slot = Clock::get()?.slot;
        let now_ts = Clock::get()?.unix_timestamp;

        // Flash-loan protection: stake age minimum.
        require!(
            now_slot >= staker.last_stake_slot + 100,
            ErrorCode::StakeTooNew
        );

        let voter_weight = staker.amount_staked;
        require!(voter_weight > 0, ErrorCode::NoVotingPower);

        let issuer = &mut ctx.accounts.issuer_account;
        let vote_record = &mut ctx.accounts.vote_record;

        // Reject late votes. finalize_voting already enforces the other side
        // (cannot finalize before voting_ends_at), so this check closes the
        // symmetric window.
        require!(
            now_ts < issuer.voting_ends_at,
            ErrorCode::VotingPeriodEnded
        );
        require!(
            issuer.status == IssuerStatus::Pending,
            ErrorCode::IssuerNotPending
        );

        if approve {
            issuer.votes_for = issuer
                .votes_for
                .checked_add(voter_weight)
                .ok_or(ErrorCode::Overflow)?;
        } else {
            issuer.votes_against = issuer
                .votes_against
                .checked_add(voter_weight)
                .ok_or(ErrorCode::Overflow)?;
        }

        vote_record.voter = ctx.accounts.voter.key();
        vote_record.issuer = issuer.key();
        vote_record.weight = voter_weight;
        vote_record.approved = approve;
        vote_record.has_voted = true;
        vote_record.released = false;

        // Lock the voter's stake until release_vote is called.
        let staker_mut = &mut ctx.accounts.staker_account;
        staker_mut.active_votes_count = staker_mut
            .active_votes_count
            .checked_add(1)
            .ok_or(ErrorCode::Overflow)?;

        msg!(
            "Voted: {} with weight {} (stake slot {})",
            if approve { "Approve" } else { "Reject" },
            voter_weight,
            staker_mut.last_stake_slot
        );
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
    ///
    /// Preconditions:
    ///   - vote_record.released == false (checked)
    ///   - issuer_account.status != Pending (enforced in context)
    ///   - staker_account.active_votes_count >= 1 (defensive; should always
    ///     hold because vote_on_issuer incremented it)
    pub fn release_vote(ctx: Context<ReleaseVote>) -> Result<()> {
        let staker_account = &mut ctx.accounts.staker_account;
        let vote_record = &mut ctx.accounts.vote_record;

        require!(!vote_record.released, ErrorCode::VoteAlreadyReleased);
        staker_account.active_votes_count = staker_account
            .active_votes_count
            .checked_sub(1)
            .ok_or(ErrorCode::Overflow)?;
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

        // ADR-0014: if this withdrawal would zero the stake and thus
        // flip status to Revoked, enrolled issuers must route through
        // `revoke_issuer_atomic` instead.  Unenrolled issuers pre-tree
        // can continue using this legacy path.
        if amount == issuer.staked_amount && issuer.is_tree_enrolled {
            return Err(error!(ErrorCode::IssuerTreeUpdateRequired));
        }

        **ctx.accounts.stake_vault.to_account_info().try_borrow_mut_lamports()? -= amount;
        **ctx.accounts.issuer_authority.to_account_info().try_borrow_mut_lamports()? += amount;

        issuer.staked_amount = issuer.staked_amount.saturating_sub(amount);
        if issuer.staked_amount == 0 {
            issuer.status = IssuerStatus::Revoked;
            issuer.status_epoch = Clock::get()?.slot;
            issuer.revocation_nonce = issuer
                .revocation_nonce
                .checked_add(1)
                .ok_or(ErrorCode::Overflow)?;
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
            // ADR-0014: record the finalisation slot so `append_issuer_leaf`
            // (which the operator calls next) picks up the correct
            // `status_epoch` for the leaf preimage.
            issuer.status_epoch = Clock::get()?.slot;
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
    }

    /// Slash an issuer for malicious behavior.
    /// Can only be called by DAO authority after governance vote or internal decision.
    ///
    /// Lamports move atomically from the shared `stake_vault` PDA to the
    /// `dao_treasury` PDA. Without this transfer (pre-remediation behavior
    /// only decremented accounting), slashed SOL was effectively orphaned in
    /// the vault and the DAO never actually captured the penalty.
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

        // ADR-0014: once the issuer has been enrolled in the issuer
        // tree, any status flip to Revoked MUST flow through
        // `revoke_issuer_atomic` so the on-chain status and the tree
        // leaf stay in lockstep.  Non-atomic revocation would leave the
        // pre-revocation leaf in the tree and let revoked proofs keep
        // verifying -- exactly the SOLID-SEC-004 attack the compressed-
        // tree pattern is designed to close.  The slash still happens
        // (lamports + counters), but we refuse to cross the Approved/
        // Cooldown -> Revoked boundary here.
        let registry = &ctx.accounts.registry_config;
        let would_revoke =
            issuer.staked_amount.saturating_sub(slash_amount) < registry.min_stake_lamports;
        if would_revoke && issuer.is_tree_enrolled {
            return Err(error!(ErrorCode::IssuerTreeUpdateRequired));
        }

        issuer.staked_amount = issuer
            .staked_amount
            .checked_sub(slash_amount)
            .ok_or(ErrorCode::Overflow)?;
        issuer.slash_count = issuer
            .slash_count
            .checked_add(1)
            .ok_or(ErrorCode::Overflow)?;

        // Move lamports: stake_vault -> dao_treasury.
        transfer_slashed_lamports(
            &ctx.accounts.stake_vault,
            &ctx.accounts.dao_treasury,
            slash_amount,
        )?;

        // If stake drops below minimum, revoke.  The guard above has
        // already refused the non-atomic path for enrolled issuers, so
        // this branch only flips status for pre-enrolment (bootstrap)
        // issuers.
        let registry = &ctx.accounts.registry_config;
        if issuer.staked_amount < registry.min_stake_lamports {
            issuer.status = IssuerStatus::Revoked;
            issuer.status_epoch = Clock::get()?.slot;
            issuer.revocation_nonce = issuer
                .revocation_nonce
                .checked_add(1)
                .ok_or(ErrorCode::Overflow)?;
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

        // ADR-0014: same enrollment guard as `slash_issuer`.  If the
        // slash would zero the stake AND the issuer is tree-enrolled,
        // refuse here so the revocation must go through
        // `revoke_issuer_atomic`.
        let would_revoke = issuer
            .staked_amount
            .saturating_sub(slash_amount)
            == 0;
        if would_revoke && issuer.is_tree_enrolled {
            return Err(error!(ErrorCode::IssuerTreeUpdateRequired));
        }

        issuer.staked_amount = issuer
            .staked_amount
            .checked_sub(slash_amount)
            .ok_or(ErrorCode::Overflow)?;
        issuer.slash_count = issuer
            .slash_count
            .checked_add(1)
            .ok_or(ErrorCode::Overflow)?;

        // Move the slashed lamports from stake_vault into the DAO treasury.
        // Reporter bounty is paid by a separate follow-on instruction when the
        // reporter is external — gating that to a circuit-verified fraud proof
        // is tracked as a P2 item in docs/IMPROVEMENTS_ROADMAP.md.
        transfer_slashed_lamports(
            &ctx.accounts.stake_vault,
            &ctx.accounts.dao_treasury,
            slash_amount,
        )?;

        if issuer.staked_amount == 0 {
            issuer.status = IssuerStatus::Revoked;
            issuer.status_epoch = Clock::get()?.slot;
            issuer.revocation_nonce = issuer
                .revocation_nonce
                .checked_add(1)
                .ok_or(ErrorCode::Overflow)?;
        }

        msg!(
            "Fraud slash executed on {} — {} lamports. Reason: {:?}",
            issuer.authority,
            slash_amount,
            reason
        );
        Ok(())
    }

    /// Trust Anchor Bypass: High-tier entity approves a lower-tier entity.
    ///
    /// The `target_authority` argument is used to derive the canonical
    /// `[b"issuer", target_authority]` PDA seed, so a malicious trust anchor
    /// cannot pass an arbitrary IssuerAccount as target_issuer.
    ///
    /// Emits `IssuerApproved` so off-chain indexers stay in sync whether the
    /// approval came through DAO voting (finalize_voting) or through this
    /// trust-anchor bypass.
    pub fn approve_via_trust_anchor(
        ctx: Context<ApproveViaTrustAnchor>,
        target_authority: Pubkey,
    ) -> Result<()> {
        let anchor = &ctx.accounts.anchor_issuer;
        let target = &mut ctx.accounts.target_issuer;

        require!(
            anchor.tier == IssuerTier::Government || anchor.tier == IssuerTier::Regulated,
            ErrorCode::UnauthorizedTrustAnchor
        );
        require!(anchor.status == IssuerStatus::Approved, ErrorCode::IssuerNotApproved);
        require!(target.status == IssuerStatus::Pending, ErrorCode::IssuerNotPending);
        require_keys_eq!(
            target.authority,
            target_authority,
            ErrorCode::Unauthorized
        );

        target.status = IssuerStatus::Approved;
        // ADR-0014: same status-epoch hand-off as finalize_voting; the
        // operator's next `append_issuer_leaf` picks this up.
        target.status_epoch = Clock::get()?.slot;

        let config = &mut ctx.accounts.registry_config;
        config.active_issuers = config
            .active_issuers
            .checked_add(1)
            .ok_or(ErrorCode::Overflow)?;

        emit!(IssuerApproved {
            issuer: target.key(),
            authority: target.authority,
            bjj_pub_key_x: target.bjj_pub_key_x,
            bjj_pub_key_y: target.bjj_pub_key_y,
            tier: target.tier.clone(),
            // Trust-anchor approvals are binary rather than percentage-based;
            // surface this to downstream consumers as 10_000 bps = 100 pct.
            approval_bps: 10_000,
            timestamp: Clock::get()?.unix_timestamp,
        });

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
    ///
    /// ADR-0014: once an issuer has been enrolled in the issuer tree,
    /// revocation MUST go through `revoke_issuer_atomic` so the status
    /// flip and the tree replace_leaf happen in the same tx.  This
    /// legacy path stays wired for pre-enrolment bootstrap scenarios
    /// (where the tree does not yet contain the issuer) and for test
    /// clusters that run without the tree enabled.
    pub fn revoke_issuer(ctx: Context<RevokeIssuer>) -> Result<()> {
        let issuer = &mut ctx.accounts.issuer_account;
        require!(
            !issuer.is_tree_enrolled,
            ErrorCode::IssuerTreeUpdateRequired
        );
        issuer.status = IssuerStatus::Revoked;
        issuer.status_epoch = Clock::get()?.slot;
        issuer.revocation_nonce = issuer
            .revocation_nonce
            .checked_add(1)
            .ok_or(ErrorCode::Overflow)?;
        let config = &mut ctx.accounts.registry_config;
        if config.active_issuers > 0 {
            config.active_issuers -= 1;
        }
        msg!("Issuer REVOKED: {}", issuer.name);
        Ok(())
    }

    // ─── Issuer-tree binding (ADR-0014; SEC-004 setup) ────────────────────
    //
    // The issuer tree backs the SEC-004 in-circuit issuer-pubkey binding.
    // These three instructions mirror schema-registry's tree-binding
    // lifecycle one-to-one: initialise + update_root + set_status.  The
    // issuer tree is a singleton -- one per deployment -- so the PDA seed
    // carries no parameter and there is no schema_hash to track.
    //
    // This scaffold lands additively.  Until Phase 2 impl 2 wires the
    // status-transition hooks (register_issuer etc.), the binding is
    // initialised once at deploy time and holds an all-zero root.  The
    // circuit-rev commit will consume `current_root` as a public input;
    // until then these instructions are dormant but deployable.

    /// Allocate and initialise the singleton `IssuerTreeBinding` PDA.
    ///
    /// Guards (mirrors `schema_registry::initialize_tree_binding`):
    ///   * PDA seed is exactly `[b"issuer-tree-binding"]`.
    ///   * Literal 8-byte discriminator `b"issrtree"` is written.
    ///   * The caller's key is recorded as the binding's authority and is
    ///     the only signer that can call `update_issuer_tree_root` or
    ///     `set_issuer_tree_binding_status` afterwards.
    ///
    /// The caller MUST be `registry_config.authority` (the DAO / upgrade
    /// authority) to prevent an arbitrary signer from installing a
    /// competing tree_pubkey.
    pub fn initialize_issuer_tree_binding(
        ctx: Context<InitializeIssuerTreeBinding>,
        tree_pubkey: Pubkey,
    ) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.authority.key(),
            ctx.accounts.registry_config.authority,
            ErrorCode::Unauthorized
        );

        let binding_info = ctx.accounts.issuer_tree_binding.to_account_info();
        let rent = Rent::get()?;
        let lamports = rent.minimum_balance(ISSUER_TREE_BINDING_SIZE);

        let signer_seeds: &[&[u8]] = &[
            ISSUER_TREE_BINDING_SEED,
            &[ctx.bumps.issuer_tree_binding],
        ];
        let signer_seeds_all: &[&[&[u8]]] = &[signer_seeds];

        invoke_signed(
            &system_instruction::create_account(
                &ctx.accounts.authority.key(),
                &binding_info.key(),
                lamports,
                ISSUER_TREE_BINDING_SIZE as u64,
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
        data[0..8].copy_from_slice(&ISSUER_TREE_DISCRIMINATOR);
        data[8..40].copy_from_slice(&tree_pubkey.to_bytes());
        data[40..72].copy_from_slice(&[0u8; 32]); // root starts at 0 (empty tree)
        data[72..80].copy_from_slice(&Clock::get()?.slot.to_le_bytes());
        data[80] = ISSUER_TREE_STATUS_ACTIVE;
        data[81..113].copy_from_slice(&ctx.accounts.authority.key().to_bytes());

        msg!(
            "IssuerTreeBinding initialised: tree={} authority={}",
            tree_pubkey,
            ctx.accounts.authority.key()
        );
        // Avoid "unused" warning: `invoke` is imported for forward-compat
        // with the Phase 2 impl 2 status-transition hooks that will use
        // it for `append` CPIs that do not need signer seeds.
        let _ = invoke;
        Ok(())
    }

    /// Mirror the current SPL AC Merkle root into the issuer-tree binding.
    ///
    /// Same authority-gated push model as schema-registry's
    /// `update_tree_root`: the recorded authority (or an indexer acting on
    /// its behalf) pushes the new root after CPIing into the SPL AC tree.
    /// Monotonicity is enforced on `last_updated_slot`; without it a
    /// compromised authority (or a replayed tx) could regress the root to
    /// a pre-revocation value, re-admitting a revoked issuer's old proofs.
    pub fn update_issuer_tree_root(
        ctx: Context<UpdateIssuerTreeRoot>,
        new_root: [u8; 32],
    ) -> Result<()> {
        let binding_info = ctx.accounts.issuer_tree_binding.to_account_info();
        require_keys_eq!(
            *binding_info.owner,
            crate::ID,
            ErrorCode::InvalidIssuerTreeBindingOwner
        );
        let mut data = binding_info.try_borrow_mut_data()?;
        require!(
            data.len() >= ISSUER_TREE_BINDING_SIZE,
            ErrorCode::InvalidIssuerTreeBinding
        );
        require!(
            data[0..8] == ISSUER_TREE_DISCRIMINATOR,
            ErrorCode::InvalidIssuerTreeBinding
        );
        require!(
            data[80] == ISSUER_TREE_STATUS_ACTIVE,
            ErrorCode::IssuerTreeBindingFrozen
        );

        let stored_authority: [u8; 32] = data[81..113].try_into().unwrap();
        require!(
            stored_authority == ctx.accounts.authority.key().to_bytes(),
            ErrorCode::Unauthorized
        );

        let last_slot_bytes: [u8; 8] = data[72..80].try_into().unwrap();
        let last_slot = u64::from_le_bytes(last_slot_bytes);
        let now_slot = Clock::get()?.slot;
        require!(now_slot > last_slot, ErrorCode::IssuerTreeRootNotMonotonic);

        data[40..72].copy_from_slice(&new_root);
        data[72..80].copy_from_slice(&now_slot.to_le_bytes());
        Ok(())
    }

    /// Freeze / unfreeze the issuer-tree binding.  A frozen binding makes
    /// every subsequent `verify_batch_proof` fail the issuer-root gate,
    /// which is the nuclear option for halting proof verification in case
    /// of incident response.  Only the recorded authority can call this.
    pub fn set_issuer_tree_binding_status(
        ctx: Context<UpdateIssuerTreeRoot>,
        status: u8,
    ) -> Result<()> {
        require!(
            status == ISSUER_TREE_STATUS_ACTIVE || status == ISSUER_TREE_STATUS_FROZEN,
            ErrorCode::InvalidIssuerTreeBindingStatus
        );
        let binding_info = ctx.accounts.issuer_tree_binding.to_account_info();
        require_keys_eq!(
            *binding_info.owner,
            crate::ID,
            ErrorCode::InvalidIssuerTreeBindingOwner
        );
        let mut data = binding_info.try_borrow_mut_data()?;
        require!(
            data.len() >= ISSUER_TREE_BINDING_SIZE
                && data[0..8] == ISSUER_TREE_DISCRIMINATOR,
            ErrorCode::InvalidIssuerTreeBinding
        );
        let stored_authority: [u8; 32] = data[81..113].try_into().unwrap();
        require!(
            stored_authority == ctx.accounts.authority.key().to_bytes(),
            ErrorCode::Unauthorized
        );
        data[80] = status;
        Ok(())
    }

    // ─── Issuer-tree leaf lifecycle (ADR-0014; SEC-004 / SEC-008) ─────────
    //
    // Two instructions wrap the SPL AC CPIs that actually mutate the
    // issuer tree:
    //
    //   append_issuer_leaf      -- first-time enrolment of an approved
    //                              issuer.  CPIs SPL AC `append`, assigns
    //                              the issuer's permanent
    //                              `issuer_tree_leaf_index`, flips
    //                              `is_tree_enrolled = true`.
    //   revoke_issuer_atomic    -- atomic revoke + tree replace.  Bumps
    //                              revocation_nonce + status_epoch AND
    //                              CPIs SPL AC `replace_leaf` in a single
    //                              instruction.  Required path for any
    //                              enrolled issuer's Revoked transition;
    //                              the legacy `revoke_issuer` (DAO) /
    //                              `slash_issuer` / `submit_fraud_proof`
    //                              paths REFUSE when `is_tree_enrolled`
    //                              is true (see the top of each handler).
    //
    // The atomicity is load-bearing: without it, there is a window where
    // on-chain status is Revoked but the issuer-tree root still reflects
    // the pre-revocation leaf.  A proof generated in that window would
    // pass the circuit's Merkle-membership check against the stale root
    // AND be accepted by `verify_batch_proof` (which reads the stale
    // root from `IssuerTreeBinding.current_root`).  Atomicity closes the
    // window to zero txs.

    /// First-time enrolment of an issuer into the singleton issuer tree.
    ///
    /// Preconditions:
    ///   * Issuer status must be `Approved`.
    ///   * Issuer must not already be enrolled (`is_tree_enrolled == false`).
    ///   * Caller signs as `registry_config.authority` (the DAO / tree
    ///     operator); this is the same authority that runs
    ///     `initialize_issuer_tree_binding` and `update_issuer_tree_root`.
    ///
    /// Behaviour:
    ///   1. Computes the issuer leaf on-chain via Poseidon(5) of
    ///      (authority, bjj_x, bjj_y, status_epoch, revocation_nonce).
    ///   2. CPIs `spl_account_compression::append` signed by the
    ///      `[b"issuer-tree-authority"]` PDA; SPL AC appends the leaf
    ///      and emits its `ChangeLog` through `spl-noop`.
    ///   3. Bumps `registry_config.next_issuer_leaf_index` by 1 and
    ///      records the assigned index in `issuer.issuer_tree_leaf_index`.
    ///   4. Flips `issuer.is_tree_enrolled = true`.
    ///
    /// The caller is expected to invoke `update_issuer_tree_root` in the
    /// same transaction (either directly or via the tree-authority
    /// operator); `IssuerTreeBinding.current_root` is the on-chain gate
    /// the verifier reads, and only the binding update makes this
    /// enrolment effective for proofs.
    pub fn append_issuer_leaf(ctx: Context<AppendIssuerLeaf>) -> Result<()> {
        // Authority gate mirrors the binding lifecycle instructions.
        require_keys_eq!(
            ctx.accounts.authority.key(),
            ctx.accounts.registry_config.authority,
            ErrorCode::Unauthorized
        );

        let issuer = &mut ctx.accounts.issuer_account;
        require!(
            issuer.status == IssuerStatus::Approved,
            ErrorCode::IssuerNotApproved
        );
        require!(
            !issuer.is_tree_enrolled,
            ErrorCode::IssuerAlreadyEnrolled
        );

        // Compute the leaf that `batch_credential_query.circom` STEP 0.75
        // would compute for this issuer.  The input ordering MUST match
        // the circuit's `Poseidon(5)` exactly; a swap breaks the
        // Merkle-membership check on every proof without a clear error
        // on-chain.
        let leaf = compute_issuer_leaf_bytes(issuer)?;

        // ─── CPI into SPL AC `append` ─────────────────────────────────
        let spl_ac: SolPubkey = spl_account_compression_id::ID;
        let spl_noop: SolPubkey = spl_noop_id::ID;
        require_keys_eq!(
            ctx.accounts.compression_program.key(),
            spl_ac,
            ErrorCode::InvalidCompressionProgram
        );
        require_keys_eq!(
            ctx.accounts.log_wrapper.key(),
            spl_noop,
            ErrorCode::InvalidNoopProgram
        );

        let (tree_authority_key, tree_authority_bump) = Pubkey::find_program_address(
            &[ISSUER_TREE_AUTHORITY_SEED],
            &crate::ID,
        );
        require_keys_eq!(
            ctx.accounts.issuer_tree_authority.key(),
            tree_authority_key,
            ErrorCode::InvalidIssuerTreeAuthority
        );

        let mut ix_data = Vec::with_capacity(40);
        ix_data.extend_from_slice(&SPL_AC_APPEND_DISCRIMINATOR);
        ix_data.extend_from_slice(&leaf);

        let cpi_ix = Instruction {
            program_id: spl_ac,
            accounts: vec![
                AccountMeta::new(ctx.accounts.merkle_tree.key(), false),
                AccountMeta::new_readonly(tree_authority_key, true),
                AccountMeta::new_readonly(spl_noop, false),
            ],
            data: ix_data,
        };
        let signer_seeds: &[&[u8]] = &[
            ISSUER_TREE_AUTHORITY_SEED,
            &[tree_authority_bump],
        ];
        invoke_signed(
            &cpi_ix,
            &[
                ctx.accounts.merkle_tree.to_account_info(),
                ctx.accounts.issuer_tree_authority.to_account_info(),
                ctx.accounts.log_wrapper.to_account_info(),
                ctx.accounts.compression_program.to_account_info(),
            ],
            &[signer_seeds],
        )?;

        // ─── Bump counter + record assignment ─────────────────────────
        let config = &mut ctx.accounts.registry_config;
        let assigned_index = config.next_issuer_leaf_index;
        config.next_issuer_leaf_index = config
            .next_issuer_leaf_index
            .checked_add(1)
            .ok_or(ErrorCode::Overflow)?;
        issuer.issuer_tree_leaf_index = assigned_index;
        issuer.is_tree_enrolled = true;

        emit!(IssuerLeafAppended {
            issuer: issuer.authority,
            leaf,
            leaf_index: assigned_index,
            status_epoch: issuer.status_epoch,
            revocation_nonce: issuer.revocation_nonce,
            merkle_tree: ctx.accounts.merkle_tree.key(),
        });
        Ok(())
    }

    /// Atomic revoke: bump holder state + CPI `replace_leaf` in one ix.
    ///
    /// Preconditions:
    ///   * Caller signs as `registry_config.authority`.
    ///   * Issuer is `is_tree_enrolled` and is currently in `Approved`
    ///     or `Cooldown` status -- revoking a `Pending`, `Rejected`, or
    ///     already-`Revoked` issuer is a no-op this ix refuses.
    ///
    /// Behaviour (in strict order; load-bearing):
    ///   1. Compute OLD leaf from pre-bump on-chain state.
    ///   2. Bump `issuer.revocation_nonce += 1` and
    ///      `issuer.status_epoch = Clock::slot`.
    ///   3. Set `issuer.status = IssuerStatus::Revoked`.
    ///   4. Compute NEW leaf from post-bump state.
    ///   5. CPI `spl_account_compression::replace_leaf` with
    ///      `(old_root, old_leaf, new_leaf, leaf_index)` and the Merkle
    ///      proof provided by the caller in `remaining_accounts`.
    ///   6. Decrement `registry_config.active_issuers`.
    ///
    /// The caller supplies the proof nodes (top-of-path first, root-of-
    /// tree last; minus any canopy the tree has) in `remaining_accounts`.
    /// SPL AC itself validates the proof against the supplied `old_root`
    /// and the live tree state; a stale proof -> CPI failure -> the
    /// whole tx rolls back (atomicity).  The caller must also invoke
    /// `update_issuer_tree_root` in the same tx to keep
    /// `IssuerTreeBinding.current_root` in lockstep; the verifier reads
    /// that binding for the gate, and only its update makes the
    /// revocation visible to proof-verification.
    pub fn revoke_issuer_atomic<'info>(
        ctx: Context<'_, '_, '_, 'info, RevokeIssuerAtomic<'info>>,
        old_root: [u8; 32],
    ) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.authority.key(),
            ctx.accounts.registry_config.authority,
            ErrorCode::Unauthorized
        );

        let issuer = &mut ctx.accounts.issuer_account;
        require!(issuer.is_tree_enrolled, ErrorCode::IssuerNotEnrolled);
        require!(
            issuer.status == IssuerStatus::Approved
                || issuer.status == IssuerStatus::Cooldown,
            ErrorCode::InvalidRevokeSourceStatus
        );

        // (1) OLD leaf.
        let old_leaf = compute_issuer_leaf_bytes(issuer)?;

        // (2) Bump counters BEFORE computing the new leaf.
        issuer.revocation_nonce = issuer
            .revocation_nonce
            .checked_add(1)
            .ok_or(ErrorCode::Overflow)?;
        issuer.status_epoch = Clock::get()?.slot;

        // (3) Flip status.
        issuer.status = IssuerStatus::Revoked;

        // (4) NEW leaf (reflects bumped state).
        let new_leaf = compute_issuer_leaf_bytes(issuer)?;

        // Captured now because step 6 borrows config mutably again.
        let leaf_index = issuer.issuer_tree_leaf_index;
        let leaf_index_u32: u32 = leaf_index
            .try_into()
            .map_err(|_| error!(ErrorCode::IssuerTreeLeafIndexTooLarge))?;

        // (5) CPI replace_leaf.
        let spl_ac: SolPubkey = spl_account_compression_id::ID;
        let spl_noop: SolPubkey = spl_noop_id::ID;
        require_keys_eq!(
            ctx.accounts.compression_program.key(),
            spl_ac,
            ErrorCode::InvalidCompressionProgram
        );
        require_keys_eq!(
            ctx.accounts.log_wrapper.key(),
            spl_noop,
            ErrorCode::InvalidNoopProgram
        );

        let (tree_authority_key, tree_authority_bump) = Pubkey::find_program_address(
            &[ISSUER_TREE_AUTHORITY_SEED],
            &crate::ID,
        );
        require_keys_eq!(
            ctx.accounts.issuer_tree_authority.key(),
            tree_authority_key,
            ErrorCode::InvalidIssuerTreeAuthority
        );

        // ix_data layout for SPL AC replace_leaf:
        //   discriminator(8) | old_root(32) | prev_leaf(32) | new_leaf(32) | index(u32 LE)
        let mut ix_data = Vec::with_capacity(8 + 32 + 32 + 32 + 4);
        ix_data.extend_from_slice(&SPL_AC_REPLACE_LEAF_DISCRIMINATOR);
        ix_data.extend_from_slice(&old_root);
        ix_data.extend_from_slice(&old_leaf);
        ix_data.extend_from_slice(&new_leaf);
        ix_data.extend_from_slice(&leaf_index_u32.to_le_bytes());

        let mut accounts = vec![
            AccountMeta::new(ctx.accounts.merkle_tree.key(), false),
            AccountMeta::new_readonly(tree_authority_key, true),
            AccountMeta::new_readonly(spl_noop, false),
        ];
        // Proof nodes supplied by the caller as readonly remaining
        // accounts -- SPL AC validates them against the tree itself.
        for proof_node in ctx.remaining_accounts.iter() {
            accounts.push(AccountMeta::new_readonly(proof_node.key(), false));
        }

        let cpi_ix = Instruction {
            program_id: spl_ac,
            accounts,
            data: ix_data,
        };

        let signer_seeds: &[&[u8]] = &[
            ISSUER_TREE_AUTHORITY_SEED,
            &[tree_authority_bump],
        ];
        let mut invoke_accounts = vec![
            ctx.accounts.merkle_tree.to_account_info(),
            ctx.accounts.issuer_tree_authority.to_account_info(),
            ctx.accounts.log_wrapper.to_account_info(),
            ctx.accounts.compression_program.to_account_info(),
        ];
        for proof_node in ctx.remaining_accounts.iter() {
            invoke_accounts.push(proof_node.clone());
        }
        invoke_signed(&cpi_ix, &invoke_accounts, &[signer_seeds])?;

        // (6) Registry bookkeeping.
        let config = &mut ctx.accounts.registry_config;
        if config.active_issuers > 0 {
            config.active_issuers = config.active_issuers - 1;
        }

        emit!(IssuerLeafReplaced {
            issuer: issuer.authority,
            old_leaf,
            new_leaf,
            leaf_index,
            new_status_epoch: issuer.status_epoch,
            new_revocation_nonce: issuer.revocation_nonce,
            reason: RevokeReason::Revoked,
            merkle_tree: ctx.accounts.merkle_tree.key(),
        });
        Ok(())
    }

    // ─── Credential issuance (R-2) ────────────────────────────────────────

    /// Publish a new credential commitment into the SPL Account-Compression
    /// tree bound to `schema_hash`.
    ///
    /// Only an *Approved* issuer may call this.  The instruction:
    ///   1. Bounds-checks the issuer and the supplied commitment.
    ///   2. Builds the SPL AC `append` instruction by hand and invokes it
    ///      with the `(b"tree-authority", schema_hash)` PDA signing as the
    ///      tree authority.  SPL AC enforces that the caller IS that
    ///      authority.
    ///   3. Increments `issuer.credentials_issued` and emits the
    ///      `CredentialIssued` event.
    ///
    /// What this instruction deliberately does NOT do:
    ///   * It does not update `SchemaTreeBinding.current_root`.  That lives
    ///     in `schema-registry` and is written by a separate
    ///     `update_tree_root` call (either directly by the tree-authority
    ///     keypair or by an indexer that watches `CredentialIssued`).  This
    ///     split is what lets the on-chain path work with ANY SPL AC tree
    ///     shape without parsing the concurrent-merkle-tree header on-chain.
    ///   * It does not verify the off-chain EdDSA signature on the credential
    ///     — that's a Circom-circuit concern at proof-generation time.
    pub fn issue_credential(
        ctx: Context<IssueCredential>,
        schema_hash: [u8; 32],
        commitment: [u8; 32],
    ) -> Result<()> {
        let issuer = &mut ctx.accounts.issuer_account;
        require!(
            issuer.status == IssuerStatus::Approved,
            ErrorCode::IssuerNotApproved
        );
        // The issuer's authority must sign.
        require_keys_eq!(
            issuer.authority,
            ctx.accounts.issuer_authority.key(),
            ErrorCode::Unauthorized
        );

        // Reject the trivial (all-zero / all-one) commitments that can only
        // be produced by mis-use of the SDK — every honest commitment is a
        // Poseidon output and effectively collision-free with these patterns.
        require!(
            commitment != [0u8; 32] && commitment != [0xFFu8; 32],
            ErrorCode::InvalidCommitment
        );

        // SOLID-SEC-003: bind this issuance to a *registered* schema + tree.
        // Without these checks, an approved issuer could pass any
        // `schema_hash` and any `merkle_tree` and append credentials to
        // a rogue schema/tree universe.  The seed constraints on the
        // context accounts prove they are PDAs derived under
        // schema-registry; the in-handler checks then tie the bytes
        // together.
        let schema_acct = &ctx.accounts.schema_account;
        require!(!schema_acct.deprecated, ErrorCode::SchemaDeprecated);
        require!(
            schema_acct.schema_hash == schema_hash,
            ErrorCode::SchemaHashMismatch
        );
        require_keys_eq!(
            *ctx.accounts.schema_tree_binding.owner,
            SCHEMA_REGISTRY_ID,
            ErrorCode::InvalidSchemaTreeBindingOwner
        );
        {
            let binding_data = ctx.accounts.schema_tree_binding.try_borrow_data()?;
            verify_schema_tree_binding_for_issue(
                &binding_data,
                &schema_hash,
                &ctx.accounts.merkle_tree.key(),
            )
            .map_err(|e| match e {
                LightError::TreeBindingMismatch => ErrorCode::TreeBindingMismatch,
                LightError::SchemaTreeBindingFrozen => ErrorCode::SchemaTreeBindingFrozen,
                _ => ErrorCode::InvalidSchemaTreeBinding,
            })?;
        }

        // Derive and verify the tree-authority PDA.
        let (tree_authority_key, tree_authority_bump) = Pubkey::find_program_address(
            &[TREE_AUTHORITY_SEED, schema_hash.as_ref()],
            &crate::ID,
        );
        require_keys_eq!(
            ctx.accounts.tree_authority.key(),
            tree_authority_key,
            ErrorCode::InvalidTreeAuthority
        );

        // Build the SPL AC `append` instruction manually.
        let mut ix_data = Vec::with_capacity(40);
        ix_data.extend_from_slice(&SPL_AC_APPEND_DISCRIMINATOR);
        ix_data.extend_from_slice(&commitment);

        let spl_ac: SolPubkey = spl_account_compression_id::ID;
        let spl_noop: SolPubkey = spl_noop_id::ID;
        require_keys_eq!(
            ctx.accounts.compression_program.key(),
            spl_ac,
            ErrorCode::InvalidCompressionProgram
        );
        require_keys_eq!(ctx.accounts.log_wrapper.key(), spl_noop, ErrorCode::InvalidNoopProgram);

        let cpi_ix = Instruction {
            program_id: spl_ac,
            accounts: vec![
                AccountMeta::new(ctx.accounts.merkle_tree.key(), false),
                AccountMeta::new_readonly(tree_authority_key, true),
                AccountMeta::new_readonly(spl_noop, false),
            ],
            data: ix_data,
        };

        let signer_seeds: &[&[u8]] = &[
            TREE_AUTHORITY_SEED,
            schema_hash.as_ref(),
            &[tree_authority_bump],
        ];
        invoke_signed(
            &cpi_ix,
            &[
                ctx.accounts.merkle_tree.to_account_info(),
                ctx.accounts.tree_authority.to_account_info(),
                ctx.accounts.log_wrapper.to_account_info(),
                ctx.accounts.compression_program.to_account_info(),
            ],
            &[signer_seeds],
        )?;

        issuer.credentials_issued = issuer
            .credentials_issued
            .checked_add(1)
            .ok_or(ErrorCode::Overflow)?;

        emit!(CredentialIssued {
            issuer: issuer.authority,
            schema_hash,
            commitment,
            merkle_tree: ctx.accounts.merkle_tree.key(),
            slot: Clock::get()?.slot,
            timestamp: Clock::get()?.unix_timestamp,
        });

        msg!(
            "CredentialIssued: issuer={} schema_hash[..4]={:?} tree={}",
            issuer.authority,
            &schema_hash[..4],
            ctx.accounts.merkle_tree.key()
        );
        Ok(())
    }
}

// ─── Account Contexts ──────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct InitializeRegistry<'info> {
    #[account(
        init, payer = authority,
        // 8 disc + 32 authority + 32 governance_token_mint + 8 min_stake
        // + 8 voting_period + 8 approval_threshold + 8 total_issuers
        // + 8 active_issuers + 8 next_issuer_leaf_index (ADR-0014)
        // = 120 bytes.
        space = 8 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8,
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
    // `mut` is required: vote_on_issuer increments active_votes_count.
    // Without mut, Anchor silently discards the write and the DAO
    // governance lock is unenforceable.
    #[account(mut, seeds = [b"staker", voter.key().as_ref()], bump)]
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
    /// CHECK: Shared stake vault PDA (source of slashed lamports).
    #[account(mut, seeds = [b"stake-vault"], bump)]
    pub stake_vault: AccountInfo<'info>,
    /// CHECK: DAO treasury PDA (destination of slashed lamports).
    /// Initialised on first slash via init_if_needed so the registry is
    /// deployable without a separate bootstrapping instruction.
    #[account(
        init_if_needed,
        payer = reporter,
        space = 0,
        seeds = [b"dao-treasury"],
        bump,
        owner = system_program.key()
    )]
    pub dao_treasury: AccountInfo<'info>,
    #[account(mut)]
    pub reporter: Signer<'info>,
    pub system_program: Program<'info, System>,
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
#[instruction(target_authority: Pubkey)]
pub struct ApproveViaTrustAnchor<'info> {
    #[account(mut, seeds = [b"registry-config"], bump)]
    pub registry_config: Account<'info, RegistryConfig>,
    #[account(seeds = [b"issuer", anchor_authority.key().as_ref()], bump)]
    pub anchor_issuer: Account<'info, IssuerAccount>,
    /// Seed-constrained to the canonical issuer PDA of the supplied
    /// target_authority. This prevents a malicious trust anchor from handing
    /// us an arbitrary off-canon IssuerAccount.
    #[account(
        mut,
        seeds = [b"issuer", target_authority.as_ref()],
        bump,
    )]
    pub target_issuer: Account<'info, IssuerAccount>,
    #[account(mut)]
    pub anchor_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct SlashIssuer<'info> {
    #[account(mut, seeds = [b"registry-config"], bump)]
    pub registry_config: Account<'info, RegistryConfig>,
    #[account(mut)]
    pub issuer_account: Account<'info, IssuerAccount>,
    /// CHECK: Shared stake vault PDA (source of slashed lamports).
    #[account(mut, seeds = [b"stake-vault"], bump)]
    pub stake_vault: AccountInfo<'info>,
    /// CHECK: DAO treasury PDA (destination of slashed lamports).
    #[account(
        init_if_needed,
        payer = authority,
        space = 0,
        seeds = [b"dao-treasury"],
        bump,
        owner = system_program.key()
    )]
    pub dao_treasury: AccountInfo<'info>,
    #[account(
        mut,
        constraint = authority.key() == registry_config.authority @ ErrorCode::Unauthorized
    )]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
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

/// ADR-0014.  Accounts for `initialize_issuer_tree_binding`.
///
/// `authority` must equal `registry_config.authority` (handler check);
/// the seed constraint proves the binding PDA is derived under this
/// program; the system-program CPI inside the handler writes the
/// initial 113-byte payload.
#[derive(Accounts)]
pub struct InitializeIssuerTreeBinding<'info> {
    #[account(seeds = [b"registry-config"], bump)]
    pub registry_config: Account<'info, RegistryConfig>,

    /// CHECK: seeds-constrained; written + owned by this program via a
    /// system CreateAccount CPI inside the handler.
    #[account(mut, seeds = [ISSUER_TREE_BINDING_SEED], bump)]
    pub issuer_tree_binding: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// ADR-0014.  Accounts for `update_issuer_tree_root` + reused by
/// `set_issuer_tree_binding_status`.  The PDA seed proves program
/// provenance; the stored-authority byte range inside the binding
/// (offset [81..113)) is what the handler actually enforces as the
/// gate.
#[derive(Accounts)]
pub struct UpdateIssuerTreeRoot<'info> {
    /// CHECK: parsed raw; owner + discriminator + stored-authority
    /// checked in-handler.
    #[account(mut, seeds = [ISSUER_TREE_BINDING_SEED], bump)]
    pub issuer_tree_binding: UncheckedAccount<'info>,
    pub authority: Signer<'info>,
}

/// ADR-0014.  Accounts for `append_issuer_leaf`.
///
/// `issuer_account` is mutated (assigns `issuer_tree_leaf_index`,
/// flips `is_tree_enrolled`); `registry_config` is mutated (bumps
/// `next_issuer_leaf_index`).  The SPL AC accounts are validated
/// inline in the handler.
#[derive(Accounts)]
pub struct AppendIssuerLeaf<'info> {
    #[account(mut, seeds = [b"registry-config"], bump)]
    pub registry_config: Account<'info, RegistryConfig>,

    /// The issuer being enrolled.  `issuer_authority` is NOT a signer
    /// here -- enrolment is driven by the registry authority (and the
    /// issuer already signed their own registration).
    #[account(
        mut,
        seeds = [b"issuer", issuer_account.authority.as_ref()],
        bump
    )]
    pub issuer_account: Account<'info, IssuerAccount>,

    /// CHECK: Issuer-tree PDA that signs the SPL AC `append` CPI.
    /// Seed is the singleton `[b"issuer-tree-authority"]`; rederived
    /// + compared in-handler via `find_program_address`.
    #[account(seeds = [ISSUER_TREE_AUTHORITY_SEED], bump)]
    pub issuer_tree_authority: UncheckedAccount<'info>,

    /// CHECK: SPL AC concurrent merkle tree account (writable); SPL AC
    /// validates ownership + shape on CPI.
    #[account(mut)]
    pub merkle_tree: UncheckedAccount<'info>,

    /// CHECK: Must be `spl_noop_id::ID`; validated in-handler.
    pub log_wrapper: UncheckedAccount<'info>,

    /// CHECK: Must be `spl_account_compression_id::ID`; validated in-handler.
    pub compression_program: UncheckedAccount<'info>,

    /// The registry authority -- same authority that governs
    /// `IssuerTreeBinding`.  Signs the ix.
    pub authority: Signer<'info>,
}

/// ADR-0014.  Accounts for `revoke_issuer_atomic`.
///
/// Proof nodes are supplied as `remaining_accounts` (readonly).  The
/// caller must include every sibling on the path from the leaf to the
/// root, minus any canopy the backing SPL AC tree already holds
/// on-chain.
#[derive(Accounts)]
pub struct RevokeIssuerAtomic<'info> {
    #[account(mut, seeds = [b"registry-config"], bump)]
    pub registry_config: Account<'info, RegistryConfig>,

    #[account(
        mut,
        seeds = [b"issuer", issuer_account.authority.as_ref()],
        bump
    )]
    pub issuer_account: Account<'info, IssuerAccount>,

    /// CHECK: Issuer-tree PDA that signs the SPL AC `replace_leaf` CPI.
    #[account(seeds = [ISSUER_TREE_AUTHORITY_SEED], bump)]
    pub issuer_tree_authority: UncheckedAccount<'info>,

    /// CHECK: SPL AC concurrent merkle tree account; SPL AC validates
    /// ownership + shape on CPI.
    #[account(mut)]
    pub merkle_tree: UncheckedAccount<'info>,

    /// CHECK: Must be `spl_noop_id::ID`; validated in-handler.
    pub log_wrapper: UncheckedAccount<'info>,

    /// CHECK: Must be `spl_account_compression_id::ID`; validated in-handler.
    pub compression_program: UncheckedAccount<'info>,

    pub authority: Signer<'info>,
}

/// Accounts for `issue_credential`.
///
/// * `issuer_account`    — PDA recording the approved issuer; must match
///                         `issuer_authority`.
/// * `issuer_authority`  — signer (the issuer's Solana key).
/// * `tree_authority`    — PDA `(b"tree-authority", schema_hash)` that SPL AC
///                         will see as the authorized appender.  Signed by
///                         this program via `invoke_signed`.
/// * `merkle_tree`       — the concurrent-merkle-tree account owned by SPL AC.
/// * `log_wrapper`       — `spl-noop` program.
/// * `compression_program` — SPL Account Compression program.
#[derive(Accounts)]
#[instruction(schema_hash: [u8; 32], commitment: [u8; 32])]
pub struct IssueCredential<'info> {
    #[account(
        mut,
        seeds = [b"issuer", issuer_authority.key().as_ref()],
        bump
    )]
    pub issuer_account: Account<'info, IssuerAccount>,

    pub issuer_authority: Signer<'info>,

    /// SOLID-SEC-003.  Typed `SchemaAccount` PDA from schema-registry.
    /// Seed-constrained + `seeds::program` so Anchor:
    ///   - verifies the account is owned by schema-registry;
    ///   - verifies its discriminator matches `SchemaAccount`;
    ///   - verifies the PDA was derived with schema-registry's ID
    ///     and the `(b"schema", name, version)` seeds.
    /// The handler additionally requires
    /// `schema_account.schema_hash == schema_hash` and
    /// `!schema_account.deprecated`.
    #[account(
        seeds = [
            b"schema",
            schema_account.name.as_bytes(),
            core::slice::from_ref(&schema_account.version),
        ],
        bump,
        seeds::program = SCHEMA_REGISTRY_ID,
    )]
    pub schema_account: Account<'info, SchemaAccount>,

    /// SOLID-SEC-003.  Raw 145-byte `SchemaTreeBinding` PDA (custom
    /// layout; see `programs/schema-registry/src/lib.rs` header).  The
    /// seeds prove this PDA was derived from `schema_hash` under
    /// schema-registry.  The handler uses the `solid-light` parser to
    /// cross-check `tree_pubkey == merkle_tree.key()` and that the
    /// binding is not frozen; it also asserts the runtime owner is
    /// `SCHEMA_REGISTRY_ID` (layout alone cannot prove provenance).
    /// CHECK: parsed + owner-verified in-handler.
    #[account(
        seeds = [b"schema-tree-binding", schema_hash.as_ref()],
        bump,
        seeds::program = SCHEMA_REGISTRY_ID,
    )]
    pub schema_tree_binding: UncheckedAccount<'info>,

    /// CHECK: Derived + verified in-handler against
    /// `(b"tree-authority", schema_hash)`.  The PDA is a signer via
    /// `invoke_signed`; it never needs to be writable.
    #[account(seeds = [TREE_AUTHORITY_SEED, schema_hash.as_ref()], bump)]
    pub tree_authority: UncheckedAccount<'info>,

    /// CHECK: SPL Account Compression tree account (writable).  Its
    /// pubkey is tied to `schema_tree_binding.tree_pubkey` in-handler;
    /// the SPL AC program validates ownership and shape during CPI.
    #[account(mut)]
    pub merkle_tree: UncheckedAccount<'info>,

    /// CHECK: Must be `spl_noop_id::ID`; validated in-handler.
    pub log_wrapper: UncheckedAccount<'info>,

    /// CHECK: Must be `spl_account_compression_id::ID`; validated in-handler.
    pub compression_program: UncheckedAccount<'info>,
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
    // `mut`: handler decrements active_votes_count.
    #[account(mut, seeds = [b"staker", voter.key().as_ref()], bump)]
    pub staker_account: Account<'info, StakerAccount>,
    // `mut`: handler flips `released = true`.
    #[account(
        mut,
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
    /// ADR-0014: strictly monotone counter; the value read here is the
    /// index assigned to the NEXT `append_issuer_leaf` call, after which
    /// the counter bumps by 1.  Never decrements (revocation replaces
    /// the leaf in place, it does not free the index).
    pub next_issuer_leaf_index: u64,
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
    /// ADR-0014: monotonic counter, bumped on every revoke / re-approve.
    /// Feeds the issuer-tree leaf's 5th Poseidon input.  Starts at 0 on
    /// `register_issuer`.  Strictly monotone -- re-approval of a
    /// previously-revoked issuer MUST bump again so their post-re-approval
    /// leaf is distinct from the pre-revocation leaf; otherwise the
    /// in-circuit nullifier universe wouldn't rotate and SEC-008 would
    /// re-open.
    pub revocation_nonce: u64,
    /// ADR-0014: slot at which the issuer was last flipped to
    /// `IssuerStatus::Approved`.  Feeds the leaf's 4th Poseidon input so
    /// status transitions (Approved -> Cooldown -> Revoked -> re-Approved)
    /// each change the leaf regardless of whether `revocation_nonce` bumps.
    pub status_epoch: u64,
    /// ADR-0014: 0-based index of this issuer's leaf in the issuer tree.
    /// Assigned by `append_issuer_leaf` when the issuer is first enrolled;
    /// stable thereafter (every `replace_issuer_leaf_*` ix writes back to
    /// the same index).
    pub issuer_tree_leaf_index: u64,
    /// ADR-0014: true once `append_issuer_leaf` has succeeded for this
    /// issuer.  Gates the atomic status-transition paths -- once enrolled,
    /// every Revoked transition MUST flow through `revoke_issuer_atomic`
    /// so the on-chain status and the tree leaf stay in lockstep (closes
    /// the tree-update lag window that would otherwise let a revoked
    /// issuer's pre-revocation proofs keep verifying).
    pub is_tree_enrolled: bool,
}

impl IssuerAccount {
    /// 32 authority
    /// + (4 + 64) name (len-prefixed String, capped by NameTooLong)
    /// + (4 + 128) metadata_uri (len-prefixed String, capped by MetadataTooLong)
    /// + 32 bjj_x + 32 bjj_y
    /// + 1 tier + 1 status
    /// + 8 * 12 numeric fields (staked_amount, registered_at, creation_slot,
    ///    cooldown_ends_at, votes_for, votes_against, voting_ends_at,
    ///    credentials_issued, slash_count, revocation_nonce, status_epoch,
    ///    issuer_tree_leaf_index)
    /// + 1 is_tree_enrolled (bool)
    pub const SPACE: usize = 32 + (4 + 64) + (4 + 128) + 32 + 32 + 1 + 1 + 8 * 12 + 1;
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
    #[msg("Credential commitment is degenerate (all-zero or all-one bytes)")]
    InvalidCommitment,
    #[msg("Derived tree-authority PDA does not match supplied account")]
    InvalidTreeAuthority,
    #[msg("Supplied compression_program account is not SPL Account Compression")]
    InvalidCompressionProgram,
    #[msg("Supplied log_wrapper account is not SPL Noop")]
    InvalidNoopProgram,
    #[msg("Slash would drop stake_vault below the rent-exempt minimum (SOLID-SEC-030)")]
    StakeVaultWouldGoBelow,
    #[msg("Supplied schema_hash does not match schema_account.schema_hash (SOLID-SEC-003)")]
    SchemaHashMismatch,
    #[msg("Schema is marked deprecated; issuing against it is disallowed (SOLID-SEC-003)")]
    SchemaDeprecated,
    #[msg("SchemaTreeBinding account is not owned by schema-registry (SOLID-SEC-003)")]
    InvalidSchemaTreeBindingOwner,
    #[msg("SchemaTreeBinding discriminator or layout is invalid (SOLID-SEC-003)")]
    InvalidSchemaTreeBinding,
    #[msg("SchemaTreeBinding.tree_pubkey does not match merkle_tree (SOLID-SEC-003)")]
    TreeBindingMismatch,
    #[msg("SchemaTreeBinding is frozen; cannot issue into this tree (SOLID-SEC-003)")]
    SchemaTreeBindingFrozen,
    #[msg("IssuerTreeBinding account is not owned by issuer-registry (ADR-0014)")]
    InvalidIssuerTreeBindingOwner,
    #[msg("IssuerTreeBinding discriminator or layout is invalid (ADR-0014)")]
    InvalidIssuerTreeBinding,
    #[msg("IssuerTreeBinding is frozen; cannot update root or verify proofs (ADR-0014)")]
    IssuerTreeBindingFrozen,
    #[msg("IssuerTreeBinding root update is non-monotonic (ADR-0014)")]
    IssuerTreeRootNotMonotonic,
    #[msg("IssuerTreeBinding status byte is invalid; must be 0 (active) or 1 (frozen)")]
    InvalidIssuerTreeBindingStatus,
    #[msg("IssuerTreeAuthority PDA does not match the canonical derivation of [b\"issuer-tree-authority\"]")]
    InvalidIssuerTreeAuthority,
    #[msg("Issuer is already enrolled in the issuer tree (ADR-0014)")]
    IssuerAlreadyEnrolled,
    #[msg("Issuer is not yet enrolled in the issuer tree (ADR-0014); call append_issuer_leaf first")]
    IssuerNotEnrolled,
    #[msg("revoke_issuer_atomic requires the issuer to be in Approved or Cooldown status")]
    InvalidRevokeSourceStatus,
    #[msg("Issuer has been enrolled in the issuer tree; revocation must flow through revoke_issuer_atomic (ADR-0014)")]
    IssuerTreeUpdateRequired,
    #[msg("issuer_tree_leaf_index does not fit in u32 (ADR-0014 hard cap at 2^32 issuers)")]
    IssuerTreeLeafIndexTooLarge,
    #[msg("Poseidon hash failed during issuer-leaf computation")]
    PoseidonFailed,
    #[msg("BJJ public key is not in the prime-order subgroup; cofactor-8 torsion rejected (SOLID-SEC-007)")]
    InvalidBJJPubKey,
}

// ─── Internal helpers ──────────────────────────────────────────────────────

/// Move `amount` lamports from `from` (a program-owned PDA) to `to` (also a
/// PDA that we control). Direct lamport manipulation is safe when:
///   1. The source account is program-owned and we are the program.
///   2. We do not violate rent-exemption for the source.
///
/// Both stake_vault and dao_treasury are system-owned (non-data) lamport
/// holders; manipulating their lamport field directly is the standard
/// pattern (see withdraw_stake / withdraw_after_cooldown in this file).
fn transfer_slashed_lamports<'info>(
    from: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    let from_balance = **from.try_borrow_lamports()?;
    require!(from_balance >= amount, ErrorCode::InsufficientStake);
    let remaining = from_balance
        .checked_sub(amount)
        .ok_or(ErrorCode::Overflow)?;

    // SOLID-SEC-030: the shared `stake_vault` PDA is a 0-byte lamport-only
    // account. If a full slash drops its balance to zero (or below the
    // rent-exempt minimum), the Solana runtime garbage-collects the PDA and
    // every future `register_issuer` deposit fails with "account not found".
    //
    // Enforce the rent floor on the source account. This is safe for any
    // data size: the runtime's rent-exempt minimum is a function of
    // `data_len()` so we compute it per-call rather than hardcoding a
    // lamport amount that would drift across epochs.
    //
    // `to` can only grow, so no rent check is needed on the receiver.
    let min_rent = Rent::get()?.minimum_balance(from.data_len());
    require!(
        remaining >= min_rent,
        ErrorCode::StakeVaultWouldGoBelow
    );

    **from.try_borrow_mut_lamports()? = remaining;
    let to_balance = **to.try_borrow_lamports()?;
    **to.try_borrow_mut_lamports()? = to_balance
        .checked_add(amount)
        .ok_or(ErrorCode::Overflow)?;
    Ok(())
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

/// Emitted on every successful `issue_credential`.
///
/// Off-chain indexers subscribe to this to:
///   * Refresh the `SchemaTreeBinding` PDA via
///     `schema_registry::update_tree_root` once the tree's new root is
///     observable.
///   * Populate a searchable holder-facing index of issued-credential
///     metadata (without the credential plaintext).
#[event]
pub struct CredentialIssued {
    pub issuer: Pubkey,
    pub schema_hash: [u8; 32],
    pub commitment: [u8; 32],
    pub merkle_tree: Pubkey,
    pub slot: u64,
    pub timestamp: i64,
}

/// Emitted on every successful `append_issuer_leaf` (ADR-0014).
/// Off-chain indexers subscribe to this to:
///   * Maintain their replicated issuer-tree state (leaf index ->
///     bytes map).
///   * Push the refreshed SPL AC root into `IssuerTreeBinding` via
///     `update_issuer_tree_root`.
#[event]
pub struct IssuerLeafAppended {
    pub issuer: Pubkey,
    pub leaf: [u8; 32],
    pub leaf_index: u64,
    pub status_epoch: u64,
    pub revocation_nonce: u64,
    pub merkle_tree: Pubkey,
}

/// Emitted on every successful `revoke_issuer_atomic` (ADR-0014).
/// Downstream consumers: same indexer as above, plus any UI that
/// wants to surface a revocation notice to holders.
#[event]
pub struct IssuerLeafReplaced {
    pub issuer: Pubkey,
    pub old_leaf: [u8; 32],
    pub new_leaf: [u8; 32],
    pub leaf_index: u64,
    pub new_status_epoch: u64,
    pub new_revocation_nonce: u64,
    pub reason: RevokeReason,
    pub merkle_tree: Pubkey,
}

/// Classifies the status transition that drove a
/// `replace_leaf` event.  Kept distinct from `SlashingReason` since
/// not every revoke comes from a slash (e.g. voluntary cooldown ->
/// revoke path).
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum RevokeReason {
    Revoked,
    Slashed,
    FraudConfirmed,
}
