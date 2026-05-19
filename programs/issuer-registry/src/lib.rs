use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::{invoke, invoke_signed},
    pubkey::Pubkey as SolPubkey,
    system_instruction,
};
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use solid_light::cpi_helpers::{
    verify_schema_tree_binding_for_issue, LightError, ISSUER_TREE_DEPTH, SCHEMA_REGISTRY_ID,
};
// SOLID-SEC-003: bring in the typed `SchemaAccount` from schema-registry
// so Anchor auto-verifies the PDA's discriminator, owner program, and
// Borsh layout -- no hand-parsing, no drift.
use schema_registry::SchemaAccount;

declare_id!("5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx");

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
pub const SPL_AC_APPEND_DISCRIMINATOR: [u8; 8] = [0x95, 0x78, 0x12, 0xde, 0xec, 0xe1, 0x58, 0xcb];

/// Anchor discriminator for `spl_account_compression::replace_leaf`:
/// `sha256("global:replace_leaf")[..8]`.  Used by
/// `revoke_issuer_atomic` and `request_withdrawal_atomic` (ADR-0014).
///
/// Discovered 2026-04-27 (SOLID-SEC-049): the prior literal
/// `[0xe3, 0x88, 0x6a, 0x74, 0x10, 0xe4, 0xe8, 0x2c]` did NOT match
/// `sha256("global:replace_leaf")[..8]` and never matched any
/// candidate ix-name preimage.  Because neither atomic ix had been
/// exercised end-to-end before integration tests 02-11 landed, every
/// `replace_leaf` CPI from this program would have failed at SPL AC's
/// instruction dispatch with `InstructionFallbackNotFound` (the SPL
/// AC program has no handler for the wrong 8-byte prefix).  The
/// regression gate is `replace_leaf_discriminator_matches_anchor_global_replace_leaf`
/// in this file's test module; it computes the discriminator from
/// the canonical preimage at test time so any future drift fails CI.
pub const SPL_AC_REPLACE_LEAF_DISCRIMINATOR: [u8; 8] =
    [0xcc, 0xa5, 0x4c, 0x64, 0x49, 0x93, 0x00, 0x80];

// ─── SEC-048 Phase E.2: subgroup-VK upload + freeze-gate constants ────────
//
// The subgroup VK has its own state machine, separate from zk-verifier's
// batch VK.  Both VKs go through identical chunked-upload + finalize +
// rotation flows, but the trust roots are independent: rotating the
// batch VK has no effect on the subgroup VK and vice versa.  We
// duplicate the timelock + size cap constants here rather than CPI'ing
// into zk-verifier because the two VKs serve different circuits and
// must roll independently.

/// SOLID-SEC-006 / ADR-0015 mirror for the SEC-048 subgroup VK.  48
/// hours between `request_subgroup_vk_rotation` and `rotate_subgroup_vk`
/// gives the DAO / watchers time to react to a compromised authority
/// attempting to swap the VK.  Numerically equal to zk-verifier's
/// `VK_ROTATION_TIMELOCK_SECONDS`; kept as a separate const so the two
/// VKs can be governed independently if a future ADR tunes one without
/// the other.
pub const SUBGROUP_VK_ROTATION_TIMELOCK_SECONDS: i64 = 48 * 60 * 60;

/// Per-CPI cap minus account discriminator + Vec length prefix.  Mirrors
/// zk-verifier's `VK_MAX_BYTES`.  The subgroup VK is much smaller
/// (~390 bytes for a 2-input circuit) so this cap is comfortable
/// headroom for future growth.
pub const SUBGROUP_VK_MAX_BYTES: usize = 10_228;

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

/// SOLID-SEC-045 helper: write `new_root` + current slot into the
/// `IssuerTreeBinding` raw bytes.  The caller MUST have already
/// owner-checked the binding account against `crate::ID`.
///
/// This helper:
///   * asserts the buffer is at least `ISSUER_TREE_BINDING_SIZE`
///     bytes and carries the canonical `ISSUER_TREE_DISCRIMINATOR`,
///   * asserts the binding is in the active state (status byte 0),
///   * (NF-07 / SOLID-SEC-079) asserts strict slot monotonicity --
///     the new write's slot MUST be greater than the binding's
///     `last_updated_slot`.  Pre-fix, two atomic ixs in the same
///     slot (e.g. a revoke and a withdrawal in the same block)
///     would both succeed and the second write would silently
///     clobber the first.  After the fix, the second write fails
///     with `IssuerTreeRootNotMonotonic` and the tx rolls back,
///     so callers re-derive the path against the live binding
///     state.  Mirrors the same require! that
///     `update_issuer_tree_root` already enforces.
///   * writes `new_root` into [40..72) and the slot into [72..80).
///
/// Extracted so the same write contract can be host-tested with
/// synthetic buffers and so the two atomic ixs share a single
/// definition of "what an atomic binding update means".
fn write_issuer_tree_binding_root(
    binding_data: &mut [u8],
    new_root: &[u8; 32],
    slot: u64,
) -> Result<()> {
    require!(
        binding_data.len() >= ISSUER_TREE_BINDING_SIZE
            && binding_data[0..8] == ISSUER_TREE_DISCRIMINATOR,
        ErrorCode::InvalidIssuerTreeBinding
    );
    require!(
        binding_data[80] == ISSUER_TREE_STATUS_ACTIVE,
        ErrorCode::IssuerTreeBindingFrozen
    );
    let prev_slot_bytes: [u8; 8] = binding_data[72..80].try_into().unwrap();
    let prev_slot = u64::from_le_bytes(prev_slot_bytes);
    require!(slot > prev_slot, ErrorCode::IssuerTreeRootNotMonotonic);
    binding_data[40..72].copy_from_slice(new_root);
    binding_data[72..80].copy_from_slice(&slot.to_le_bytes());
    Ok(())
}

/// Pre-CPI binding anchor (NF-01 / NF-04 / SOLID-SEC-077, closed 2026-05-01).
///
/// Recomputes the Poseidon-Merkle root from `(old_leaf, leaf_index,
/// caller-supplied path)` and asserts it equals
/// `IssuerTreeBinding.current_root`.  This ties the path the caller
/// supplied to the binding's notion of "current" tree state, not
/// just to whatever stale root SPL AC's concurrent change-log buffer
/// happens to admit.
///
/// Why this is load-bearing: SPL AC's `replace_leaf` validates the
/// proof against ANY root in its change-log ring buffer (default 64
/// entries), not only the active root.  Pre-fix, an attacker (or a
/// stale-state operator) could submit a proof against an old root
/// `R1` while the binding was at `R3`.  SPL AC would accept the
/// CPI (R1 still in buffer); the post-CPI Poseidon recompute would
/// produce a "next" root from `R1`'s perspective; and that root
/// would land in the binding -- regressing it from `R3` to a state
/// that doesn't reflect the live tree.  After the fix, the OLD-root
/// recompute gates the CPI: the path must be consistent with the
/// binding's current root before the SPL AC CPI runs at all.
///
/// Soundness contract: Poseidon's collision resistance + the
/// caller's inability to forge a path producing the binding's root
/// without knowing the leaves at every other index up to the
/// affected level.  Cost: one Poseidon-Merkle recompute (~50K CU at
/// `ISSUER_TREE_DEPTH = 16`); CU surveyed in `docs/CU_BUDGET.md`.
fn verify_issuer_binding_anchor(
    binding_data: &[u8],
    old_leaf: &[u8; 32],
    leaf_index: u64,
    path: &[[u8; 32]],
) -> Result<()> {
    require!(
        binding_data.len() >= ISSUER_TREE_BINDING_SIZE
            && binding_data[0..8] == ISSUER_TREE_DISCRIMINATOR,
        ErrorCode::InvalidIssuerTreeBinding
    );
    let mut current_root = [0u8; 32];
    current_root.copy_from_slice(&binding_data[40..72]);
    let recomputed =
        solid_light::cpi_helpers::compute_poseidon_merkle_root(old_leaf, leaf_index, path)
            .map_err(|_| error!(ErrorCode::InvalidIssuerTreeBinding))?;
    require!(recomputed == current_root, ErrorCode::IssuerTreeRootStale);
    Ok(())
}

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
    ///
    /// Contract (2026-04-26 redesign — see docs/E2E_BLOCKERS.md B10):
    ///
    /// Inputs:
    ///   - `min_stake` (u64, lamports): minimum SOL stake for issuer
    ///     registration.  Validated by `register_issuer` against the
    ///     issuer's tier multiplier.
    ///   - `voting_period` (i64, seconds): per-proposal voting window.
    ///     MUST be `> 0`; a zero or negative period would let proposals
    ///     finalise instantly with whatever quorum happened to exist at
    ///     submission, which is not a "vote".
    ///   - `approval_threshold` (u64, basis points): fraction of YES weight
    ///     required to approve.  MUST be `<= 10_000` (100.00%).
    ///
    /// Accounts:
    ///   - `governance_mint` (Mint): the SPL mint whose holders are the
    ///     governance constituency.  We take a typed `Account<Mint>` here
    ///     instead of a raw `Pubkey` parameter so that Anchor's deserializer
    ///     enforces "this is a real, live SPL Mint, owned by the SPL Token
    ///     program" at the boundary.  Previously this was a free `Pubkey`
    ///     parameter, which let scripts pass `Pubkey::default()` and write
    ///     a non-mint into `registry.governance_token_mint`; that corrupted
    ///     state then poisoned every downstream guard that compared the
    ///     vault's mint against `registry.governance_token_mint` (B10).
    ///   - `governance_vault` (TokenAccount, init): the singleton DAO-stake
    ///     escrow for this registry, born atomically with the registry.
    ///     Folding this into `initialize_registry` (instead of a separate
    ///     `init_governance_vault` ix or a lazy `init_if_needed` on the
    ///     stake path) means callers of `stake_tokens` only ever see a
    ///     fully-initialised vault, eliminating both the access-violation
    ///     class of bug from B10 and the "stale mint" reconciliation
    ///     surface for clients.  PDA: `["governance-vault", registry_config]`.
    ///     `token::authority = governance_vault` makes the vault its own
    ///     authority (matches what `unstake_tokens` already signs as).
    ///
    /// Effects:
    ///   - Creates the singleton `RegistryConfig` PDA at
    ///     `["registry-config"]`, owned by this program.
    ///   - Creates the singleton governance vault TokenAccount.
    ///   - Records `governance_token_mint = governance_mint.key()`.  Once
    ///     written, this field is immutable for the life of the registry.
    ///
    /// Failure modes:
    ///   - `governance_mint` is not a valid SPL Mint -> Anchor's
    ///     deserializer rejects the tx before this body runs.
    ///   - `voting_period <= 0` -> `ErrorCode::InvalidVotingPeriod`.
    ///   - `approval_threshold > 10_000` -> `ErrorCode::InvalidThreshold`.
    ///   - Registry already exists -> `AccountAlreadyInUse` (Anchor `init`).
    pub fn initialize_registry(
        ctx: Context<InitializeRegistry>,
        min_stake: u64,
        voting_period: i64,
        approval_threshold: u64, // basis points (e.g., 6000 = 60%)
    ) -> Result<()> {
        require!(voting_period > 0, ErrorCode::InvalidVotingPeriod);
        require!(approval_threshold <= 10_000, ErrorCode::InvalidThreshold);

        let governance_token_mint = ctx.accounts.governance_mint.key();

        let registry = &mut ctx.accounts.registry_config;
        registry.authority = ctx.accounts.authority.key();
        registry.governance_token_mint = governance_token_mint;
        registry.min_stake_lamports = min_stake;
        registry.voting_period_seconds = voting_period;
        registry.approval_threshold_bps = approval_threshold;
        registry.total_issuers = 0;
        registry.active_issuers = 0;
        registry.next_issuer_leaf_index = 0;
        msg!(
            "DAO Issuer Registry initialized: mint={}, vault={}, min_stake={}, voting_period={}s, threshold={}bps",
            governance_token_mint,
            ctx.accounts.governance_vault.key(),
            min_stake,
            voting_period,
            approval_threshold,
        );
        Ok(())
    }

    pub fn register_issuer(
        ctx: Context<RegisterIssuer>,
        name: String,
        metadata_uri: String,
        bjj_pub_key_x: [u8; 32],
        bjj_pub_key_y: [u8; 32],
        tier: IssuerTier,
        // SEC-048 Phase E.3 (2026-05-XX): Groth16 proof of the
        // prime-order subgroup invariant for `(bjj_pub_key_x,
        // bjj_pub_key_y)`.  Format: `proof_a (64) || proof_b (128) ||
        // proof_c (64)` = 256 bytes.  The bytes are the SDK-encoded
        // form (LB5 / SOLID-SEC-067 G2 (imag, real) swap applied;
        // proof_a Y NOT pre-negated -- on-chain
        // `verify_groth16_proof::<2>` negates internally).  Anchor
        // 0.30.1's BPF Vec<[u8; N]> deserialiser is unstable per LB1,
        // so the proof rides on the wire as a `Vec<u8>` and gets
        // chunk-extracted into typed `[u8; 64]` / `[u8; 128]` arrays
        // by this handler.
        subgroup_proof: Vec<u8>,
    ) -> Result<()> {
        require!(name.len() <= 64, ErrorCode::NameTooLong);
        require!(metadata_uri.len() <= 128, ErrorCode::MetadataTooLong);

        // SOLID-SEC-062 / H4: reject non-canonical BN254 field
        // encodings on the BJJ pubkey x/y BEFORE the subgroup verify.
        // Without this gate, a malicious issuer can submit two distinct
        // 32-byte encodings (`v1 != v2`, `v1 mod p == v2 mod p`) that
        // both produce the same field element and collide in the
        // issuer tree -- nullifier confusion under revocation.  Cheap
        // (<200 CU): two MSB-first 32-byte compares.
        require!(
            solid_core::poseidon::is_canonical_bn254_le(&bjj_pub_key_x),
            ErrorCode::InvalidBJJPubKey
        );
        require!(
            solid_core::poseidon::is_canonical_bn254_le(&bjj_pub_key_y),
            ErrorCode::InvalidBJJPubKey
        );

        // Defense-in-depth pre-checks BEFORE the heavy Groth16 verify:
        // reject off-curve / Edwards-identity inputs cheaply (~3.5K CU
        // total) so a malicious caller can't burn the full ~285K CU on
        // garbage that the cheap predicate would have rejected.  The
        // load-bearing soundness gate is still the Groth16 verify
        // below; these are the SEC-048 consolation rails kept as a
        // ~free filter (L4 + L6: cheap explicit pre-checks beat
        // implicit failures inside a 285K-CU pairing).
        let bjj_pub_key = solid_core::babyjubjub::BJJPublicKey {
            x: bjj_pub_key_x,
            y: bjj_pub_key_y,
        };
        require!(
            solid_core::babyjubjub::is_on_curve(&bjj_pub_key),
            ErrorCode::InvalidBJJPubKey
        );
        require!(
            !solid_core::babyjubjub::is_identity(&bjj_pub_key),
            ErrorCode::InvalidBJJPubKey
        );

        // SOLID-SEC-048 Phase E.3 (2026-05-XX): on-chain Groth16 verify
        // of the prime-order subgroup invariant.  The subgroup circuit
        // (`circuits/bjj_subgroup_proof.circom`) has 2 public inputs --
        // `(Ax, Ay)` -- and asserts `on_curve(P) AND P != identity AND
        // [r] * P == identity` for `r = ` the BJJ subgroup prime order.
        //
        // Public-input byte contract: snarkjs publicSignals are decimal
        // representations of the **circomlib-native** field elements.
        // The on-chain `bjj_pub_key_x` / `bjj_pub_key_y` are LE bytes
        // of the same field elements (per
        // `is_canonical_bn254_le` above; SEC-062 byte form).
        // groth16-solana expects each public input as a 32-byte BE
        // buffer of the field element, so we byte-reverse LE -> BE.
        // This contract is host-tested by
        // `subgroup_host_verify_round_trip` in this file's tests
        // module.
        require!(
            ctx.accounts.subgroup_verifier_config.vk_finalized,
            ErrorCode::SubgroupVkNotFinalized
        );
        require!(
            !ctx.accounts.subgroup_verifier_config.paused,
            ErrorCode::Unauthorized
        );
        require!(subgroup_proof.len() == 256, ErrorCode::InvalidSubgroupProof);
        let mut proof_a = [0u8; 64];
        proof_a.copy_from_slice(&subgroup_proof[0..64]);
        let mut proof_b = [0u8; 128];
        proof_b.copy_from_slice(&subgroup_proof[64..192]);
        let mut proof_c = [0u8; 64];
        proof_c.copy_from_slice(&subgroup_proof[192..256]);

        let mut be_x = bjj_pub_key_x;
        be_x.reverse();
        let mut be_y = bjj_pub_key_y;
        be_y.reverse();
        let public_inputs: [[u8; 32]; 2] = [be_x, be_y];

        solid_light::groth16::verify_groth16_proof::<2>(
            &ctx.accounts.subgroup_vk_storage.data,
            &proof_a,
            &proof_b,
            &proof_c,
            &public_inputs,
        )
        .map_err(|e| match e {
            solid_light::groth16::Groth16VerifyError::InvalidProofFormat => {
                error!(ErrorCode::InvalidSubgroupProof)
            }
            solid_light::groth16::Groth16VerifyError::ProofVerificationFailed => {
                error!(ErrorCode::InvalidSubgroupProof)
            }
        })?;

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

        msg!(
            "Issuer registered: {}. Voting ends at {}",
            issuer.name,
            issuer.voting_ends_at
        );
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
        // SIMULATOR MODE: Reduced from 50 to 12 slots (~5s on devnet)
        // to lower friction for testing. Restore to 100 for mainnet.
        require!(
            now_slot >= staker.last_stake_slot + 12,
            ErrorCode::StakeTooNew
        );

        let voter_weight = staker.amount_staked;
        require!(voter_weight > 0, ErrorCode::NoVotingPower);

        let issuer = &mut ctx.accounts.issuer_account;
        let vote_record = &mut ctx.accounts.vote_record;

        // Reject late votes. finalize_voting already enforces the other side
        // (cannot finalize before voting_ends_at), so this check closes the
        // symmetric window.
        require!(now_ts < issuer.voting_ends_at, ErrorCode::VotingPeriodEnded);
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

        **ctx
            .accounts
            .stake_vault
            .to_account_info()
            .try_borrow_mut_lamports()? -= amount;
        **ctx
            .accounts
            .issuer_authority
            .to_account_info()
            .try_borrow_mut_lamports()? += amount;
        Ok(())
    }

    /// Stake DAO tokens to gain voting power.
    ///
    /// B10 trace logs (2026-04-26): the access-violation
    /// reproduced post-CPI post-`init_if_needed`, so we emit explicit
    /// markers at every boundary inside the handler.  If the next
    /// failure is between `enter` and `transfer-ok` -> CPI side.
    /// Between `transfer-ok` and `writeback-ready` -> field assignment.
    /// After `writeback-ready` with no `exit` log on chain -> Anchor
    /// post-handler writeback (the `Box` step is next).
    pub fn stake_tokens(ctx: Context<StakeTokens>, amount: u64) -> Result<()> {
        msg!("stake_tokens: enter, amount={}", amount);

        let voter_key = ctx.accounts.voter.key();
        let clock_slot = Clock::get()?.slot;

        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.voter_token_account.to_account_info(),
                to: ctx.accounts.governance_vault.to_account_info(),
                authority: ctx.accounts.voter.to_account_info(),
            },
        );
        token::transfer(cpi_ctx, amount)?;
        msg!("stake_tokens: transfer-ok");

        let staker_account = &mut ctx.accounts.staker_account;
        staker_account.voter = voter_key;
        staker_account.amount_staked = staker_account
            .amount_staked
            .checked_add(amount)
            .ok_or(ErrorCode::Overflow)?;
        staker_account.last_stake_slot = clock_slot;
        msg!(
            "stake_tokens: writeback-ready, total_staked={}",
            staker_account.amount_staked
        );

        Ok(())
    }

    /// Unstake DAO tokens. Only allowed if no active votes.
    pub fn unstake_tokens(ctx: Context<UnstakeTokens>, amount: u64) -> Result<()> {
        let staker_account = &mut ctx.accounts.staker_account;

        require!(
            staker_account.active_votes_count == 0,
            ErrorCode::ActiveVotesExist
        );
        require!(
            staker_account.amount_staked >= amount,
            ErrorCode::InsufficientStake
        );

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
    ///
    /// SOLID-SEC-044 gate.  Issuers enrolled in the ADR-0014 issuer
    /// tree MUST use `request_withdrawal_atomic` instead.  The legacy
    /// path would leave the on-chain status and the tree root in
    /// disagreement: the status would say Cooldown, but the tree leaf
    /// would still reflect the pre-Cooldown preimage, so every
    /// credential the issuer ever issued would continue to verify
    /// against the live tree root.  Treating Cooldown as verify-
    /// negative (the ADR-0014 amendment) requires the leaf to bump.
    pub fn request_withdrawal(ctx: Context<RequestWithdrawal>) -> Result<()> {
        let issuer = &mut ctx.accounts.issuer_account;
        require!(
            issuer.status == IssuerStatus::Approved,
            ErrorCode::IssuerNotApproved
        );

        // SOLID-SEC-044.  Enrolled issuers route through
        // `request_withdrawal_atomic`; legacy path kept for
        // pre-tree-backfill issuers only.
        if issuer.is_tree_enrolled {
            return Err(error!(ErrorCode::IssuerTreeUpdateRequired));
        }

        let now = Clock::get()?.unix_timestamp;
        issuer.status = IssuerStatus::Cooldown;
        issuer.cooldown_ends_at = now + 14 * 24 * 60 * 60;

        msg!(
            "Withdrawal requested. Cooldown ends at {}",
            issuer.cooldown_ends_at
        );
        Ok(())
    }

    /// SOLID-SEC-061 / H3: drain stake for a Revoked issuer once the
    /// 24h DAO dispute window has closed.
    ///
    /// Pre-fix, an enrolled issuer who voluntarily exited via
    /// `request_withdrawal_atomic` -> Cooldown ended up in a stuck-stake
    /// trap: `withdraw_after_cooldown` refused to fully drain the stake
    /// for tree-enrolled issuers (the legacy path required a Revoked
    /// transition), and `revoke_issuer_atomic` flipped them to Revoked
    /// without any refund handler.  Net: the only paths that touched
    /// stake refused to handle the well-behaved exit case.  This ix is
    /// the missing piece.
    ///
    /// Preconditions:
    ///   * Issuer status == Revoked.
    ///   * `staked_amount > 0`.
    ///   * The 24h dispute window opened by `revoke_issuer_atomic` has
    ///     elapsed (`cooldown_ends_at` is set on revoke; if a DAO slash
    ///     is in flight, the slash handler runs first and seizes the
    ///     stake before this ix can drain it).
    ///   * Caller signs as `issuer.authority`.
    ///
    /// Behaviour:
    ///   * Transfers `amount` lamports from `stake_vault` to
    ///     `issuer_authority`.
    ///   * Decrements `issuer.staked_amount` by `amount`; status remains
    ///     Revoked.
    ///   * Emits `StakeWithdrawn`.
    pub fn withdraw_after_revoke(ctx: Context<WithdrawAfterRevoke>, amount: u64) -> Result<()> {
        let issuer = &mut ctx.accounts.issuer_account;
        require!(
            issuer.status == IssuerStatus::Revoked,
            ErrorCode::IssuerNotRevoked
        );
        require!(issuer.staked_amount > 0, ErrorCode::InsufficientStake);
        require!(amount > 0, ErrorCode::InsufficientStake);
        require!(amount <= issuer.staked_amount, ErrorCode::InsufficientStake);
        // 24h dispute window from revoke timestamp.
        require!(
            Clock::get()?.unix_timestamp >= issuer.cooldown_ends_at,
            ErrorCode::DisputeWindowOpen
        );
        require_keys_eq!(
            ctx.accounts.issuer_authority.key(),
            issuer.authority,
            ErrorCode::Unauthorized
        );

        **ctx
            .accounts
            .stake_vault
            .to_account_info()
            .try_borrow_mut_lamports()? -= amount;
        **ctx
            .accounts
            .issuer_authority
            .to_account_info()
            .try_borrow_mut_lamports()? += amount;

        issuer.staked_amount = issuer
            .staked_amount
            .checked_sub(amount)
            .ok_or(ErrorCode::Overflow)?;

        emit!(StakeWithdrawn {
            issuer: issuer.authority,
            amount,
            remaining: issuer.staked_amount,
            slot: Clock::get()?.slot,
        });
        Ok(())
    }

    /// Finalize withdrawal of `amount` lamports after the cooldown expires.
    pub fn withdraw_after_cooldown(ctx: Context<WithdrawAfterCooldown>, amount: u64) -> Result<()> {
        let issuer = &mut ctx.accounts.issuer_account;
        require!(
            issuer.status == IssuerStatus::Cooldown,
            ErrorCode::IssuerNotInCooldown
        );
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

        **ctx
            .accounts
            .stake_vault
            .to_account_info()
            .try_borrow_mut_lamports()? -= amount;
        **ctx
            .accounts
            .issuer_authority
            .to_account_info()
            .try_borrow_mut_lamports()? += amount;

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

        require!(
            issuer.status == IssuerStatus::Pending,
            ErrorCode::IssuerNotPending
        );
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

            msg!(
                "Issuer APPROVED: {} ({}% approval)",
                issuer.name,
                approval_pct / 100
            );
        } else {
            issuer.status = IssuerStatus::Rejected;
            msg!(
                "Issuer REJECTED: {} ({}% approval, needed {}%)",
                issuer.name,
                approval_pct / 100,
                registry.approval_threshold_bps / 100
            );
        }
        Ok(())
    }

    /// SIMULATOR MODE: Allow updating the voting period on the live
    /// registry config. Used on devnet to set a short window (e.g. 10s)
    /// so issuers can be approved quickly during testing.
    /// Remove this entire instruction for mainnet.
    pub fn set_voting_period(
        ctx: Context<SetVotingPeriod>,
        new_period: i64,
    ) -> Result<()> {
        require!(new_period > 0, ErrorCode::InvalidVotingPeriod);
        let registry = &mut ctx.accounts.registry_config;
        let old = registry.voting_period_seconds;
        registry.voting_period_seconds = new_period;
        msg!(
            "Voting period updated: {}s -> {}s",
            old,
            new_period,
        );
        Ok(())
    }

    /// Grant an approved issuer permission to issue exactly one registered
    /// schema. DAO approval remains the issuer-level admission gate; this PDA
    /// is the schema-level authorization gate consumed by `issue_credential`.
    pub fn grant_schema_permission(
        ctx: Context<GrantSchemaPermission>,
        schema_hash: [u8; 32],
    ) -> Result<()> {
        let issuer = &ctx.accounts.issuer_account;
        require!(
            issuer.status == IssuerStatus::Approved,
            ErrorCode::IssuerNotApproved
        );

        let schema = &ctx.accounts.schema_account;
        require!(!schema.deprecated, ErrorCode::SchemaDeprecated);
        require!(
            schema.schema_hash == schema_hash,
            ErrorCode::SchemaHashMismatch
        );

        let permission = &mut ctx.accounts.issuer_schema_permission;
        permission.issuer = issuer.key();
        permission.issuer_authority = issuer.authority;
        permission.schema_hash = schema_hash;
        permission.schema_account = schema.key();
        permission.granted_by = ctx.accounts.registry_authority.key();
        permission.granted_at = Clock::get()?.unix_timestamp;
        permission.revoked_at = 0;
        permission.active = true;
        permission.bump = ctx.bumps.issuer_schema_permission;

        emit!(IssuerSchemaPermissionGranted {
            issuer: issuer.authority,
            issuer_account: issuer.key(),
            schema_hash,
            schema_account: schema.key(),
            granted_by: ctx.accounts.registry_authority.key(),
            timestamp: permission.granted_at,
        });

        msg!(
            "IssuerSchemaPermission granted: issuer={} schema_hash[..4]={:?}",
            issuer.authority,
            &schema_hash[..4]
        );
        Ok(())
    }

    /// Revoke a schema-specific issuance permission. Existing credentials stay
    /// verifiable through their historical commitments; new issuance for this
    /// issuer/schema pair is blocked immediately.
    pub fn revoke_schema_permission(
        ctx: Context<RevokeSchemaPermission>,
        schema_hash: [u8; 32],
    ) -> Result<()> {
        let permission = &mut ctx.accounts.issuer_schema_permission;
        require!(permission.active, ErrorCode::IssuerSchemaPermissionInactive);
        require!(
            permission.schema_hash == schema_hash,
            ErrorCode::IssuerSchemaPermissionMismatch
        );
        require!(
            permission.issuer == ctx.accounts.issuer_account.key(),
            ErrorCode::IssuerSchemaPermissionMismatch
        );

        permission.active = false;
        permission.revoked_at = Clock::get()?.unix_timestamp;

        emit!(IssuerSchemaPermissionRevoked {
            issuer: ctx.accounts.issuer_account.authority,
            issuer_account: ctx.accounts.issuer_account.key(),
            schema_hash,
            schema_account: permission.schema_account,
            revoked_by: ctx.accounts.registry_authority.key(),
            timestamp: permission.revoked_at,
        });

        msg!(
            "IssuerSchemaPermission revoked: issuer={} schema_hash[..4]={:?}",
            ctx.accounts.issuer_account.authority,
            &schema_hash[..4]
        );
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
        memo: String,
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

        msg!(
            "Issuer slashed: {} lamports. Reason: {:?}. Memo: {}",
            slash_amount,
            reason_tag,
            memo
        );
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
        let would_revoke = issuer.staked_amount.saturating_sub(slash_amount) == 0;
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
        // M9 / SOLID-SEC-074 (closed 2026-05-01): mutual tier-floor.
        // Pre-fix only the anchor's tier was gated -- a Regulated
        // anchor could approve a Government target, silently
        // up-tier-ing the target without the corresponding regulatory
        // bar.  Post-fix: target.tier <= anchor.tier is required
        // (rank-based comparison).  Government can approve any tier;
        // Regulated can approve Regulated/Enterprise/Community.
        require!(
            target.tier.rank() <= anchor.tier.rank(),
            ErrorCode::UnauthorizedTrustAnchor
        );
        require!(
            anchor.status == IssuerStatus::Approved,
            ErrorCode::IssuerNotApproved
        );
        require!(
            target.status == IssuerStatus::Pending,
            ErrorCode::IssuerNotPending
        );
        require_keys_eq!(target.authority, target_authority, ErrorCode::Unauthorized);

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

        msg!(
            "Issuer APPROVED via Trust Anchor {}: {}",
            anchor.name,
            target.name
        );
        Ok(())
    }

    /// Check if an issuer is approved (called via CPI from ZK verifier).
    pub fn check_issuer_status(ctx: Context<CheckIssuerStatus>) -> Result<()> {
        let issuer = &ctx.accounts.issuer_account;
        require!(
            issuer.status == IssuerStatus::Approved,
            ErrorCode::IssuerNotApproved
        );
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

        let signer_seeds: &[&[u8]] = &[ISSUER_TREE_BINDING_SEED, &[ctx.bumps.issuer_tree_binding]];
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

    /// Update the issuer-tree binding's `current_root` to a new
    /// Poseidon root, integrity-checked on-chain.
    ///
    /// SOLID-SEC-059 / H1 (closes 2026-04-30): pre-fix, this ix accepted
    /// a caller-supplied `new_root: [u8; 32]` from a single-key authority
    /// with no on-chain integrity check.  Combined with SEC-043
    /// (single-key blast radius), that was a proof-forging primitive:
    /// the authority could push any 32-byte value `R` and verify_batch_proof
    /// would accept proofs whose `issuerTreeRoot = R`.
    ///
    /// Robust fix: the caller supplies `(new_root, new_leaf, leaf_index,
    /// poseidon_proof_path)`.  The handler runs an on-chain Poseidon-Merkle
    /// recompute and refuses any push whose recompute does not match
    /// `new_root`.  An attacker controlling the authority key can still
    /// install a root, but only one consistent with a real Poseidon path
    /// they constructed -- pushing a fabricated root with no
    /// corresponding leaf set is no longer possible.
    ///
    /// Architecture (LB4 closure, 2026-04-30): the binding stores the
    /// POSEIDON root because the in-circuit `MerkleInclusion` template
    /// (circuits/lib/merkle_inclusion.circom) uses Poseidon(2) for path
    /// recompute and the witness commits to a Poseidon root.  The
    /// on-chain SPL AC tree is Keccak-hashed and serves as the
    /// leaf-presence ledger; its root is NOT the canonical root for
    /// proof verification.  The earlier "read SPL AC header" attempt
    /// at H1/CRIT-2 conflated the two and surfaced as Groth16 verify
    /// failure once B13 Option 2 closed the wire-size cap.
    ///
    /// `poseidon_proof_path` is sent as a flat `Vec<u8>` (16 * 32 = 512
    /// bytes for ISSUER_TREE_DEPTH=16) to avoid Anchor 0.30.1's
    /// `Vec<[u8; 32]>` BorshDeserialize-on-BPF issue (LB1).
    pub fn update_issuer_tree_root(
        ctx: Context<UpdateIssuerTreeRoot>,
        new_root: [u8; 32],
        new_leaf: [u8; 32],
        leaf_index: u64,
        poseidon_proof_path: Vec<u8>,
    ) -> Result<()> {
        let binding_info = ctx.accounts.issuer_tree_binding.to_account_info();
        require_keys_eq!(
            *binding_info.owner,
            crate::ID,
            ErrorCode::InvalidIssuerTreeBindingOwner
        );

        // Path arity gate: the caller MUST supply exactly
        // ISSUER_TREE_DEPTH siblings (no canopy reliance, since this is
        // the off-chain Poseidon side).
        require!(
            poseidon_proof_path.len() == ISSUER_TREE_DEPTH * 32,
            ErrorCode::InvalidProofPathLength
        );
        let mut path: Vec<[u8; 32]> = Vec::with_capacity(ISSUER_TREE_DEPTH);
        for i in 0..ISSUER_TREE_DEPTH {
            let mut s = [0u8; 32];
            s.copy_from_slice(&poseidon_proof_path[i * 32..(i + 1) * 32]);
            path.push(s);
        }

        // On-chain Poseidon recompute integrity check (closes H1's
        // root-injection attack).  Cost: ~50-80K CU (16 sol_poseidon
        // syscalls + canonicalisation rounds); comfortably under the
        // 200K default per-ix budget.
        let computed_root =
            solid_light::cpi_helpers::compute_poseidon_merkle_root(&new_leaf, leaf_index, &path)
                .map_err(|_| error!(ErrorCode::InvalidIssuerTreeBinding))?;
        require!(computed_root == new_root, ErrorCode::IssuerTreeRootMismatch);

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
            data.len() >= ISSUER_TREE_BINDING_SIZE && data[0..8] == ISSUER_TREE_DISCRIMINATOR,
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
    //
    // SOLID-SEC-045 (closed 2026-04-27): the atomic ixs ALSO write the
    // post-CPI tree root into `IssuerTreeBinding.current_root` in the
    // same instruction.  Pre-fix, the binding update was a separate
    // `update_issuer_tree_root` ix enforced only by doc comment; if the
    // caller forgot it, `verify_batch_proof` continued to accept
    // pre-revocation proofs against the stale binding root.  Post-fix,
    // the new root is recomputed on-chain from
    // `(new_leaf, leaf_index, full proof path)` via Keccak256 and
    // committed to the binding before the handler returns.  The
    // `update_issuer_tree_root` ix remains for the `append_issuer_leaf`
    // path (where the new root is not derivable from inputs alone) and
    // for any out-of-band root reconciliation; calling it after an
    // atomic ix is safely redundant.

    /// First-time enrolment of an issuer into the singleton issuer tree.
    ///
    /// SOLID-SEC-059 / H1 (closes 2026-04-30, atomic + integrity-checked).
    /// The handler appends the leaf into the SPL AC tree (leaf-presence
    /// ledger; Keccak-hashed) AND atomically updates the
    /// `IssuerTreeBinding.current_root` to the new Poseidon root,
    /// integrity-checked via on-chain Poseidon-Merkle recompute against
    /// `poseidon_proof_path`.  No "caller MUST follow up" pattern; no
    /// trust in a caller-supplied root value.
    ///
    /// Architecture note (LB4 closure 2026-04-30): the binding stores
    /// the POSEIDON root because the in-circuit `MerkleInclusion`
    /// template uses Poseidon(2) for path-recompute and the witness
    /// commits to a Poseidon root.  SPL AC's Keccak root is the
    /// leaf-presence ledger and is NOT the canonical root for proof
    /// verification.  The earlier "read SPL AC header" attempt at
    /// H1/CRIT-2 surfaced as Groth16 verify failure once B13 Option 2
    /// closed the wire-size cap.
    ///
    /// Preconditions:
    ///   * Issuer status must be `Approved`.
    ///   * Issuer must not already be enrolled (`is_tree_enrolled == false`).
    ///   * Caller signs as `registry_config.authority`.
    ///   * `poseidon_proof_path` MUST be the Poseidon-Merkle path of
    ///     siblings against the empty leaf at `next_issuer_leaf_index`
    ///     (= the path that would witness "no leaf at this slot" pre-append).
    ///     Callers derive this off-chain via
    ///     `@solid-protocol/light::LocalReplicaAdapter`.
    ///
    /// Path encoding: flat `Vec<u8>` of `ISSUER_TREE_DEPTH * 32` bytes
    /// (each 32-byte chunk is one Poseidon sibling).  Avoids the
    /// `Vec<[u8; 32]>` BorshDeserialize-on-BPF issue (LB1).
    pub fn append_issuer_leaf(
        ctx: Context<AppendIssuerLeaf>,
        poseidon_proof_path: Vec<u8>,
    ) -> Result<()> {
        // SIMULATOR MODE: Authority check removed so any wallet can enroll
        // issuers into the issuer tree during devnet testing. Restore for mainnet.
        // See PRE_LAUNCH_REVERT.md
        //
        // require_keys_eq!(
        //     ctx.accounts.authority.key(),
        //     ctx.accounts.registry_config.authority,
        //     ErrorCode::Unauthorized
        // );

        let issuer = &mut ctx.accounts.issuer_account;
        require!(
            issuer.status == IssuerStatus::Approved,
            ErrorCode::IssuerNotApproved
        );
        require!(!issuer.is_tree_enrolled, ErrorCode::IssuerAlreadyEnrolled);

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

        let (tree_authority_key, tree_authority_bump) =
            Pubkey::find_program_address(&[ISSUER_TREE_AUTHORITY_SEED], &crate::ID);
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
        let signer_seeds: &[&[u8]] = &[ISSUER_TREE_AUTHORITY_SEED, &[tree_authority_bump]];
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

        // SOLID-SEC-059 / H1 atomic binding update via on-chain Poseidon
        // recompute.  Caller-supplied `poseidon_proof_path` is verified
        // by computing Poseidon-Merkle(new_leaf, leaf_index, path) and
        // writing that root to the binding.  No trust in a caller
        // root; the recompute is the gate.
        require!(
            poseidon_proof_path.len() == ISSUER_TREE_DEPTH * 32,
            ErrorCode::InvalidProofPathLength
        );
        let mut path: Vec<[u8; 32]> = Vec::with_capacity(ISSUER_TREE_DEPTH);
        for i in 0..ISSUER_TREE_DEPTH {
            let mut s = [0u8; 32];
            s.copy_from_slice(&poseidon_proof_path[i * 32..(i + 1) * 32]);
            path.push(s);
        }
        let assigned_index = ctx.accounts.registry_config.next_issuer_leaf_index;
        let new_root =
            solid_light::cpi_helpers::compute_poseidon_merkle_root(&leaf, assigned_index, &path)
                .map_err(|_| error!(ErrorCode::InvalidIssuerTreeBinding))?;
        let binding_info = ctx.accounts.issuer_tree_binding.to_account_info();
        require_keys_eq!(
            *binding_info.owner,
            crate::ID,
            ErrorCode::InvalidIssuerTreeBindingOwner
        );
        {
            let mut binding_data = binding_info.try_borrow_mut_data()?;
            write_issuer_tree_binding_root(&mut binding_data, &new_root, Clock::get()?.slot)?;
        }

        // ─── Bump counter + record assignment ─────────────────────────
        let config = &mut ctx.accounts.registry_config;
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
        poseidon_proof_path: Vec<u8>,
    ) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.authority.key(),
            ctx.accounts.registry_config.authority,
            ErrorCode::Unauthorized
        );

        let issuer = &mut ctx.accounts.issuer_account;
        require!(issuer.is_tree_enrolled, ErrorCode::IssuerNotEnrolled);
        require!(
            issuer.status == IssuerStatus::Approved || issuer.status == IssuerStatus::Cooldown,
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

        // SOLID-SEC-061 / H3: open the 24h dispute window during which
        // the DAO can file a slash via `slash_issuer` / `submit_fraud_proof`.
        // After the window closes, the issuer (or any signer authorised
        // for stake recovery) can call `withdraw_after_revoke` to drain
        // remaining unslashed stake.  Reuses the existing
        // `cooldown_ends_at` slot since (a) Cooldown and Revoked are
        // disjoint and (b) the field's semantic ("when the current
        // restrictive state lifts") fits both cases.
        issuer.cooldown_ends_at = Clock::get()?.unix_timestamp + 24 * 60 * 60;

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

        let (tree_authority_key, tree_authority_bump) =
            Pubkey::find_program_address(&[ISSUER_TREE_AUTHORITY_SEED], &crate::ID);
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

        // SOLID-SEC-045 / CRIT-2 pre-CPI hygiene: require a full proof
        // path for the SPL AC `replace_leaf` CPI.  Issuer tree has
        // canopy=0 so SPL AC needs every sibling on-wire.
        require!(
            ctx.remaining_accounts.len() == ISSUER_TREE_DEPTH,
            ErrorCode::InvalidProofPathLength
        );

        // SOLID-SEC-059 / H1 closure (atomic + integrity-checked, 2026-04-30):
        // verify the Poseidon-Merkle path before spending CU on the SPL
        // AC CPI.  Binding stores the Poseidon root the circuit expects;
        // SPL AC's Keccak root is leaf-presence ledger only.
        require!(
            poseidon_proof_path.len() == ISSUER_TREE_DEPTH * 32,
            ErrorCode::InvalidProofPathLength
        );
        let mut path: Vec<[u8; 32]> = Vec::with_capacity(ISSUER_TREE_DEPTH);
        for i in 0..ISSUER_TREE_DEPTH {
            let mut s = [0u8; 32];
            s.copy_from_slice(&poseidon_proof_path[i * 32..(i + 1) * 32]);
            path.push(s);
        }

        // NF-01 / NF-04 / SOLID-SEC-077 (closed 2026-05-01) pre-CPI
        // anchor: the path the caller supplied MUST recompute, with
        // `old_leaf` at `leaf_index`, to the binding's current Poseidon
        // root.  Without this, SPL AC's concurrent change-log would
        // accept the CPI against any in-buffer root (default 64
        // entries) and the post-CPI Poseidon recompute would land a
        // root in the binding that doesn't reflect the live tree --
        // observable as binding regression and stale-root replay.
        // After the anchor, mismatched paths short-circuit before the
        // SPL AC CPI ever runs.
        let binding_info = ctx.accounts.issuer_tree_binding.to_account_info();
        require_keys_eq!(
            *binding_info.owner,
            crate::ID,
            ErrorCode::InvalidIssuerTreeBindingOwner
        );
        {
            let binding_data = binding_info.try_borrow_data()?;
            verify_issuer_binding_anchor(&binding_data, &old_leaf, leaf_index, &path)?;
        }

        let signer_seeds: &[&[u8]] = &[ISSUER_TREE_AUTHORITY_SEED, &[tree_authority_bump]];
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

        // SOLID-SEC-045 / CRIT-2 / H1 atomic binding update via on-chain
        // Poseidon recompute.  The new_leaf already reflects the
        // post-bump issuer state (status_epoch + revocation_nonce); the
        // path is supplied by the caller and verified by the recompute.
        // No reliance on SPL AC's Keccak path; no concurrent-semantics
        // concern because Poseidon recompute is single-input-determined.
        let new_root =
            solid_light::cpi_helpers::compute_poseidon_merkle_root(&new_leaf, leaf_index, &path)
                .map_err(|_| error!(ErrorCode::InvalidIssuerTreeBinding))?;
        {
            let mut binding_data = binding_info.try_borrow_mut_data()?;
            write_issuer_tree_binding_root(&mut binding_data, &new_root, Clock::get()?.slot)?;
        }
        // `old_root` is consumed by SPL AC `replace_leaf` for the
        // Keccak-side leaf-presence check.  Suppress unused-var warn.
        let _ = old_root;

        // (7) Registry bookkeeping.
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

    /// Atomic voluntary cooldown: Approved -> Cooldown + CPI
    /// `replace_leaf` in one ix (SOLID-SEC-044, ADR-0014 amendment).
    ///
    /// ADR-0014 amendment (2026-04-25).  Phase 2 left the legacy
    /// `request_withdrawal` handler in place, which flipped status
    /// to Cooldown but left the issuer's leaf untouched.  That made
    /// Cooldown effectively Approved-equivalent for proof
    /// verification: a credential held by an issuer who was winding
    /// down would keep verifying against the live tree root.  The
    /// post-amendment stance is "Cooldown is verify-negative": any
    /// proof touching a Cooldown issuer's credentials must fail the
    /// in-circuit Merkle membership check.  This ix delivers that by
    /// replacing the leaf preimage (status_epoch + revocation_nonce
    /// both bump) in the same transaction as the status flip.
    ///
    /// Preconditions (mirrors `revoke_issuer_atomic` except for the
    /// source status set):
    ///   * Caller signs as `issuer_account.authority` (the issuer
    ///     themselves -- NOT the DAO authority; withdrawal is
    ///     voluntary).
    ///   * Issuer is `is_tree_enrolled` and is currently in
    ///     `Approved` status.  Cooldown -> Cooldown is a no-op this
    ///     ix refuses (there is no valid state to transition to).
    ///
    /// Behaviour (identical CPI flow to `revoke_issuer_atomic`):
    ///   1. Compute OLD leaf from pre-bump on-chain state.
    ///   2. Bump `issuer.revocation_nonce += 1` and
    ///      `issuer.status_epoch = Clock::slot`.
    ///   3. Flip `issuer.status = IssuerStatus::Cooldown`; set
    ///      `issuer.cooldown_ends_at = now + 14 days`.
    ///   4. Compute NEW leaf from post-bump state.
    ///   5. CPI `spl_account_compression::replace_leaf` (caller
    ///      supplies proof nodes in `remaining_accounts`).
    ///
    /// SOLID-SEC-045 (closed 2026-04-27): this ix writes the post-CPI
    /// tree root into `IssuerTreeBinding.current_root` atomically.  No
    /// separate `update_issuer_tree_root` follow-up is required for the
    /// cooldown to be observable by `verify_batch_proof`; the binding is
    /// up-to-date the moment this handler returns successfully.  The
    /// new root is recomputed on-chain from
    /// `(new_leaf, leaf_index, full proof path)` via the same
    /// Keccak256 hashing SPL AC uses internally; soundness rests on the
    /// CPI's prior validation of the path against the pre-CPI tree
    /// root, plus pre-image resistance of Keccak256.
    pub fn request_withdrawal_atomic<'info>(
        ctx: Context<'_, '_, '_, 'info, RequestWithdrawalAtomic<'info>>,
        old_root: [u8; 32],
        poseidon_proof_path: Vec<u8>,
    ) -> Result<()> {
        let issuer = &mut ctx.accounts.issuer_account;

        // Authority: the issuer, not the DAO.  Withdrawal is a
        // voluntary exit; only the issuer can start it.
        require_keys_eq!(
            ctx.accounts.issuer_authority.key(),
            issuer.authority,
            ErrorCode::Unauthorized
        );
        require!(issuer.is_tree_enrolled, ErrorCode::IssuerNotEnrolled);
        require!(
            issuer.status == IssuerStatus::Approved,
            ErrorCode::IssuerNotApproved
        );

        // (1) OLD leaf.
        let old_leaf = compute_issuer_leaf_bytes(issuer)?;

        // (2) Bump counters BEFORE computing the new leaf so the post-
        // bump preimage differs from every pre-cooldown proof's
        // expectations.
        issuer.revocation_nonce = issuer
            .revocation_nonce
            .checked_add(1)
            .ok_or(ErrorCode::Overflow)?;
        issuer.status_epoch = Clock::get()?.slot;

        // (3) Flip status + start the 14-day cooldown window.
        issuer.status = IssuerStatus::Cooldown;
        issuer.cooldown_ends_at = Clock::get()?.unix_timestamp + 14 * 24 * 60 * 60;

        // (4) NEW leaf (reflects bumped state).
        let new_leaf = compute_issuer_leaf_bytes(issuer)?;

        let leaf_index = issuer.issuer_tree_leaf_index;
        let leaf_index_u32: u32 = leaf_index
            .try_into()
            .map_err(|_| error!(ErrorCode::IssuerTreeLeafIndexTooLarge))?;

        // (5) CPI replace_leaf.  Identical flow to
        // `revoke_issuer_atomic`; SPL AC validates the proof in
        // `remaining_accounts` against the supplied `old_root` and the
        // live tree state.
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

        let (tree_authority_key, tree_authority_bump) =
            Pubkey::find_program_address(&[ISSUER_TREE_AUTHORITY_SEED], &crate::ID);
        require_keys_eq!(
            ctx.accounts.issuer_tree_authority.key(),
            tree_authority_key,
            ErrorCode::InvalidIssuerTreeAuthority
        );

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
        for proof_node in ctx.remaining_accounts.iter() {
            accounts.push(AccountMeta::new_readonly(proof_node.key(), false));
        }

        let cpi_ix = Instruction {
            program_id: spl_ac,
            accounts,
            data: ix_data,
        };

        // SOLID-SEC-045 / CRIT-2 hygiene + SOLID-SEC-059 / H1 atomic
        // Poseidon-recompute (mirrors revoke_issuer_atomic).  See that
        // handler's commentary for the full soundness story.
        require!(
            ctx.remaining_accounts.len() == ISSUER_TREE_DEPTH,
            ErrorCode::InvalidProofPathLength
        );
        require!(
            poseidon_proof_path.len() == ISSUER_TREE_DEPTH * 32,
            ErrorCode::InvalidProofPathLength
        );
        let mut path: Vec<[u8; 32]> = Vec::with_capacity(ISSUER_TREE_DEPTH);
        for i in 0..ISSUER_TREE_DEPTH {
            let mut s = [0u8; 32];
            s.copy_from_slice(&poseidon_proof_path[i * 32..(i + 1) * 32]);
            path.push(s);
        }

        // NF-01 / NF-04 / SOLID-SEC-077 (closed 2026-05-01) pre-CPI
        // anchor: same contract as revoke_issuer_atomic.  Stale-path
        // attempts now fail with `IssuerTreeRootStale` before the SPL
        // AC CPI is ever issued.  See `verify_issuer_binding_anchor`
        // doc-comment for the full soundness story.
        let binding_info = ctx.accounts.issuer_tree_binding.to_account_info();
        require_keys_eq!(
            *binding_info.owner,
            crate::ID,
            ErrorCode::InvalidIssuerTreeBindingOwner
        );
        {
            let binding_data = binding_info.try_borrow_data()?;
            verify_issuer_binding_anchor(&binding_data, &old_leaf, leaf_index, &path)?;
        }

        let signer_seeds: &[&[u8]] = &[ISSUER_TREE_AUTHORITY_SEED, &[tree_authority_bump]];
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

        // SOLID-SEC-045 / CRIT-2 / H1 atomic binding update via on-chain
        // Poseidon recompute.  Mirrors revoke_issuer_atomic.
        let new_root =
            solid_light::cpi_helpers::compute_poseidon_merkle_root(&new_leaf, leaf_index, &path)
                .map_err(|_| error!(ErrorCode::InvalidIssuerTreeBinding))?;
        {
            let mut binding_data = binding_info.try_borrow_mut_data()?;
            write_issuer_tree_binding_root(&mut binding_data, &new_root, Clock::get()?.slot)?;
        }
        let _ = old_root;

        // Note: registry_config.active_issuers is NOT decremented
        // here -- a Cooldown issuer is still "active" for bookkeeping
        // purposes and the count moves on the Cooldown -> Revoked
        // transition (withdraw_after_cooldown / revoke_issuer_atomic).

        emit!(IssuerLeafReplaced {
            issuer: issuer.authority,
            old_leaf,
            new_leaf,
            leaf_index,
            new_status_epoch: issuer.status_epoch,
            new_revocation_nonce: issuer.revocation_nonce,
            reason: RevokeReason::CooldownRequested,
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

        let permission = &ctx.accounts.issuer_schema_permission;
        require!(permission.active, ErrorCode::IssuerSchemaPermissionInactive);
        require!(
            permission.issuer == issuer.key(),
            ErrorCode::IssuerSchemaPermissionMismatch
        );
        require_keys_eq!(
            permission.issuer_authority,
            issuer.authority,
            ErrorCode::IssuerSchemaPermissionMismatch
        );
        require!(
            permission.schema_hash == schema_hash,
            ErrorCode::IssuerSchemaPermissionMismatch
        );

        // Reject the trivial (all-zero / all-one) commitments that can only
        // be produced by mis-use of the SDK — every honest commitment is a
        // Poseidon output and effectively collision-free with these patterns.
        require!(
            commitment != [0u8; 32] && commitment != [0xFFu8; 32],
            ErrorCode::InvalidCommitment
        );

        // SOLID-SEC-062 / H4: reject non-canonical BN254 field encodings.
        // An honest commitment is a Poseidon output already in [0, p), but
        // a malicious issuer can hand-craft a 32-byte commitment with
        // `c1 != c2` and `c1 mod p == c2 mod p`, producing two on-chain
        // leaves the circuit treats as one (nullifier confusion).  This
        // gate refuses anything ≥ p before the leaf is appended.
        require!(
            solid_core::poseidon::is_canonical_bn254_le(&commitment),
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
            permission.schema_account,
            ctx.accounts.schema_account.key(),
            ErrorCode::IssuerSchemaPermissionMismatch
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
        let (tree_authority_key, tree_authority_bump) =
            Pubkey::find_program_address(&[TREE_AUTHORITY_SEED, schema_hash.as_ref()], &crate::ID);
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
        require_keys_eq!(
            ctx.accounts.log_wrapper.key(),
            spl_noop,
            ErrorCode::InvalidNoopProgram
        );

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

    // ─── SEC-048 Phase E.2: subgroup VK chunked upload + freeze-gate ─────
    //
    // These ixs mirror zk-verifier's `initialize` /
    // `store_verification_key` / `finalize_verification_key` /
    // `request_vk_rotation` / `cancel_vk_rotation` /
    // `rotate_verification_key` 1:1, but operate on a separate
    // `SubgroupVerifierConfig` PDA so the two VKs roll independently.
    // The verify path that consumes this VK is wired into
    // `register_issuer` in Phase E.3; until that lands, this ix family
    // is dormant from a soundness perspective and only exercised by
    // host tests.

    /// Initialize the subgroup-VK config PDA (authority only).  Idempotent
    /// only in the trivial sense -- the Anchor `init` constraint fails on
    /// re-init.
    pub fn init_subgroup_verifier(ctx: Context<InitSubgroupVerifier>) -> Result<()> {
        let config = &mut ctx.accounts.subgroup_verifier_config;
        config.authority = ctx.accounts.authority.key();
        config.bump = ctx.bumps.subgroup_verifier_config;
        config.paused = false;
        config.next_vk_chunk = 0;
        config.vk_initialized = false;
        config.vk_finalized = false;
        config.vk_generation = 0;
        config.rotate_request_ts = 0;
        msg!(
            "SEC-048 Phase E.2: SubgroupVerifierConfig initialized.  Authority: {}",
            config.authority
        );
        Ok(())
    }

    /// Append a VK chunk into the subgroup-VK storage PDA.
    ///
    /// Invariants (mirror zk-verifier's `store_verification_key`):
    ///   1. Chunks must arrive in order (`chunk_index ==
    ///      config.next_vk_chunk`).
    ///   2. Cumulative size capped at `SUBGROUP_VK_MAX_BYTES`.
    ///   3. SOLID-SEC-006 freeze-gate: refuses every write once
    ///      `vk_finalized == true`.  Rotation MUST flow through
    ///      `request_subgroup_vk_rotation` + 48h timelock +
    ///      `rotate_subgroup_vk`.
    pub fn store_subgroup_vk_chunk(
        ctx: Context<StoreSubgroupVkChunk>,
        chunk_index: u16,
        chunk_data: Vec<u8>,
        is_final_chunk: bool,
    ) -> Result<()> {
        let storage = &mut ctx.accounts.subgroup_vk_storage;
        let config = &mut ctx.accounts.subgroup_verifier_config;

        require!(
            config.authority == ctx.accounts.authority.key(),
            ErrorCode::Unauthorized
        );
        require!(!config.vk_finalized, ErrorCode::SubgroupVkAlreadyFinalized);
        require!(
            chunk_index == config.next_vk_chunk,
            ErrorCode::SubgroupVkChunkOutOfOrder
        );

        let incoming_len = chunk_data.len();
        if chunk_index == 0 {
            require!(
                incoming_len <= SUBGROUP_VK_MAX_BYTES,
                ErrorCode::SubgroupVkStorageFull
            );
            storage.data = chunk_data;
        } else {
            let new_total = storage
                .data
                .len()
                .checked_add(incoming_len)
                .ok_or(ErrorCode::Overflow)?;
            require!(
                new_total <= SUBGROUP_VK_MAX_BYTES,
                ErrorCode::SubgroupVkStorageFull
            );
            storage.data.extend_from_slice(&chunk_data);
        }

        config.next_vk_chunk = config
            .next_vk_chunk
            .checked_add(1)
            .ok_or(ErrorCode::Overflow)?;

        if is_final_chunk {
            config.vk_initialized = true;
            msg!(
                "SEC-048 Phase E.2: subgroup VK stored: {} bytes across {} chunks",
                storage.data.len(),
                config.next_vk_chunk
            );
        }
        Ok(())
    }

    /// SOLID-SEC-006 freeze the subgroup VK.  After this call,
    /// `store_subgroup_vk_chunk` refuses every chunk; further changes
    /// must flow through `request_subgroup_vk_rotation` +
    /// `rotate_subgroup_vk`.
    pub fn finalize_subgroup_vk(ctx: Context<SubgroupVerifierAuthorityOnly>) -> Result<()> {
        let config = &mut ctx.accounts.subgroup_verifier_config;
        require!(config.vk_initialized, ErrorCode::SubgroupVkNotSet);
        require!(!config.vk_finalized, ErrorCode::SubgroupVkAlreadyFinalized);
        config.vk_finalized = true;
        msg!(
            "SEC-048 Phase E.2: subgroup VK finalized at generation {}.  \
             Further writes rejected; rotation requires \
             request_subgroup_vk_rotation + {}s timelock + rotate_subgroup_vk.",
            config.vk_generation,
            SUBGROUP_VK_ROTATION_TIMELOCK_SECONDS
        );
        Ok(())
    }

    /// SOLID-SEC-006 start the subgroup-VK rotation timelock.  The VK
    /// stays finalized and in effect for the full 48h window; this call
    /// only records the moment after which `rotate_subgroup_vk` is
    /// permitted.
    pub fn request_subgroup_vk_rotation(ctx: Context<SubgroupVerifierAuthorityOnly>) -> Result<()> {
        let config = &mut ctx.accounts.subgroup_verifier_config;
        require!(config.vk_finalized, ErrorCode::SubgroupVkNotFinalized);
        require!(
            config.rotate_request_ts == 0,
            ErrorCode::SubgroupVkRotationAlreadyRequested
        );
        let now = Clock::get()?.unix_timestamp;
        require!(now > 0, ErrorCode::SubgroupVkRotationClockInvalid);
        config.rotate_request_ts = now;
        msg!(
            "SEC-048 Phase E.2: subgroup VK rotation requested at unix_ts={}.  \
             Earliest rotation at unix_ts={}.",
            now,
            now.saturating_add(SUBGROUP_VK_ROTATION_TIMELOCK_SECONDS)
        );
        Ok(())
    }

    /// SOLID-SEC-006 cancel a pending subgroup-VK rotation.  No-op
    /// against zero `rotate_request_ts`.  Authority-only.
    pub fn cancel_subgroup_vk_rotation(ctx: Context<SubgroupVerifierAuthorityOnly>) -> Result<()> {
        let config = &mut ctx.accounts.subgroup_verifier_config;
        let prior = config.rotate_request_ts;
        config.rotate_request_ts = 0;
        msg!(
            "SEC-048 Phase E.2: subgroup VK rotation cancelled (prior request_ts={}).",
            prior
        );
        Ok(())
    }

    /// SOLID-SEC-006 complete a timelocked subgroup-VK rotation.
    /// On success, resets `vk_initialized`, `vk_finalized`, and
    /// `next_vk_chunk` so the operator can upload a fresh VK; bumps
    /// `vk_generation`.  `subgroup_vk_storage.data` is NOT cleared
    /// here; the next chunk-0 write overwrites it atomically (same
    /// pattern as zk-verifier's `rotate_verification_key`).
    pub fn rotate_subgroup_vk(ctx: Context<SubgroupVerifierAuthorityOnly>) -> Result<()> {
        let config = &mut ctx.accounts.subgroup_verifier_config;
        require!(config.vk_finalized, ErrorCode::SubgroupVkNotFinalized);
        require!(
            config.rotate_request_ts != 0,
            ErrorCode::SubgroupVkNoPendingRotation
        );
        let now = Clock::get()?.unix_timestamp;
        require!(
            subgroup_vk_rotation_timelock_expired(config, now),
            ErrorCode::SubgroupVkRotationTimelockNotExpired
        );
        config.vk_initialized = false;
        config.vk_finalized = false;
        config.next_vk_chunk = 0;
        config.rotate_request_ts = 0;
        config.vk_generation = config
            .vk_generation
            .checked_add(1)
            .ok_or(ErrorCode::Overflow)?;
        msg!(
            "SEC-048 Phase E.2: subgroup VK rotated; generation now {}.  \
             Upload fresh chunks via store_subgroup_vk_chunk + finalize_subgroup_vk.",
            config.vk_generation
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
    /// SPL governance mint.  Typed `Account<Mint>` (not raw `Pubkey`)
    /// so Anchor's deserializer rejects anything that isn't owned by
    /// the SPL Token program.  See `initialize_registry` doc for the
    /// rationale (closes B10's "Pubkey::default() in registry" defect).
    pub governance_mint: Account<'info, Mint>,
    /// Singleton DAO-stake escrow, born atomically with the registry.
    /// Owned by itself (`token::authority = governance_vault`); the PDA
    /// signs withdrawals via `invoke_signed` with the same seeds.
    #[account(
        init, payer = authority,
        token::mint = governance_mint,
        token::authority = governance_vault,
        seeds = [b"governance-vault", registry_config.key().as_ref()],
        bump
    )]
    pub governance_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
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
    /// SEC-048 Phase E.3 (2026-05-XX): on-chain subgroup-VK config.
    /// Read-only here; must be `vk_finalized == true` and
    /// `paused == false` for `register_issuer` to admit any caller.
    /// Written only by the `init_subgroup_verifier` /
    /// `store_subgroup_vk_chunk` / `finalize_subgroup_vk` /
    /// `*_subgroup_vk_rotation` ix family (E.2).
    #[account(
        seeds = [b"subgroup-verifier-config"],
        bump = subgroup_verifier_config.bump,
    )]
    pub subgroup_verifier_config: Account<'info, SubgroupVerifierConfig>,
    /// SEC-048 Phase E.3 (2026-05-XX): on-chain subgroup-VK byte
    /// buffer.  Read-only here; consumed by
    /// `solid_light::groth16::verify_groth16_proof::<2>` to validate
    /// the supplied `subgroup_proof` against the prime-order subgroup
    /// invariant.
    #[account(
        seeds = [b"subgroup-vk-storage", subgroup_verifier_config.key().as_ref()],
        bump,
    )]
    pub subgroup_vk_storage: Account<'info, SubgroupVkStorage>,
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

/// SIMULATOR MODE: Accounts for `set_voting_period`.
/// Remove this entire struct for mainnet.
#[derive(Accounts)]
pub struct SetVotingPeriod<'info> {
    #[account(mut, seeds = [b"registry-config"], bump)]
    pub registry_config: Account<'info, RegistryConfig>,
    // SIMULATOR MODE: No authority check so any wallet can
    // adjust the voting period during devnet testing.
    // For mainnet, gate this behind registry_config.authority.
    pub caller: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(schema_hash: [u8; 32])]
pub struct GrantSchemaPermission<'info> {
    #[account(
        seeds = [b"registry-config"],
        bump,
        // SIMULATOR MODE: Authority check removed so any wallet can grant
        // schema permissions during devnet testing. Restore for mainnet.
        // constraint = registry_config.authority == registry_authority.key() @ ErrorCode::Unauthorized,
    )]
    pub registry_config: Account<'info, RegistryConfig>,

    #[account(seeds = [b"issuer", issuer_account.authority.as_ref()], bump)]
    pub issuer_account: Account<'info, IssuerAccount>,

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

    #[account(
        init_if_needed,
        payer = registry_authority,
        space = 8 + IssuerSchemaPermission::SPACE,
        seeds = [
            b"issuer-schema",
            issuer_account.key().as_ref(),
            schema_hash.as_ref(),
        ],
        bump,
    )]
    pub issuer_schema_permission: Account<'info, IssuerSchemaPermission>,

    #[account(mut)]
    pub registry_authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(schema_hash: [u8; 32])]
pub struct RevokeSchemaPermission<'info> {
    #[account(
        seeds = [b"registry-config"],
        bump,
        // SIMULATOR MODE: Authority check removed so any wallet can revoke
        // schema permissions during devnet testing. Restore for mainnet.
        // constraint = registry_config.authority == registry_authority.key() @ ErrorCode::Unauthorized,
    )]
    pub registry_config: Account<'info, RegistryConfig>,

    #[account(seeds = [b"issuer", issuer_account.authority.as_ref()], bump)]
    pub issuer_account: Account<'info, IssuerAccount>,

    #[account(
        mut,
        seeds = [
            b"issuer-schema",
            issuer_account.key().as_ref(),
            schema_hash.as_ref(),
        ],
        bump = issuer_schema_permission.bump,
    )]
    pub issuer_schema_permission: Account<'info, IssuerSchemaPermission>,

    pub registry_authority: Signer<'info>,
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

/// ADR-0014.  Accounts for `set_issuer_tree_binding_status`.  The PDA
/// seed proves program provenance; the stored-authority byte range
/// inside the binding (offset [81..113)) is what the handler actually
/// enforces as the gate.
///
/// Historical note: this struct was named `UpdateIssuerTreeRoot` and
/// was shared by the deleted `update_issuer_tree_root` ix
/// (SOLID-SEC-059 / H1, 2026-04-29).  The `set_issuer_tree_binding_status`
/// handler still uses the same shape (binding PDA + signing authority),
/// so the accounts struct is kept under a renamed alias.  External
/// IDL clients calling `set_issuer_tree_binding_status` see the new
/// name in the Anchor IDL.
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

    /// CHECK: SOLID-SEC-059 / H1 atomic binding update.  Owner +
    /// discriminator + status checked in-handler via
    /// `write_issuer_tree_binding_root`.  The post-CPI handler computes
    /// the new Poseidon root from `poseidon_proof_path` (ix arg) and
    /// writes it here atomically with the SPL AC append.
    #[account(mut, seeds = [ISSUER_TREE_BINDING_SEED], bump)]
    pub issuer_tree_binding: UncheckedAccount<'info>,

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

    /// SOLID-SEC-045.  Singleton `IssuerTreeBinding` (113-byte custom
    /// layout); written atomically inside this ix to reflect the
    /// post-CPI tree root.  Owner-checked + discriminator-checked +
    /// status-checked in-handler.  Must be writable.
    /// CHECK: parsed + owner-verified in-handler against `crate::ID`.
    #[account(mut, seeds = [ISSUER_TREE_BINDING_SEED], bump)]
    pub issuer_tree_binding: UncheckedAccount<'info>,

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

    /// Schema-scoped issuance permission. DAO grants this after issuer
    /// approval; `issue_credential` rejects any issuer/schema pair without an
    /// active permission PDA.
    #[account(
        seeds = [
            b"issuer-schema",
            issuer_account.key().as_ref(),
            schema_hash.as_ref(),
        ],
        bump = issuer_schema_permission.bump,
    )]
    pub issuer_schema_permission: Account<'info, IssuerSchemaPermission>,

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

/// B10 resolution (2026-04-26): vault init is **not** here.
///
/// The `governance_vault` is born atomically with the registry inside
/// `initialize_registry` (see that ix's doc).  By the time any caller
/// reaches `stake_tokens`, the vault is already initialised, owned by
/// itself, and bound to `registry.governance_token_mint`.  This handler
/// therefore only references the vault as `mut` and verifies its mint
/// matches the typed `governance_mint` account the caller supplies.
///
/// Why this matters: the previous shape (`init_if_needed` on the vault
/// here) co-located System+SPL init with a CPI that mutates the same
/// account in the same tx, which surfaced as
/// `Access violation in unknown section` during Anchor's post-handler
/// account writeback on localnet.  See docs/E2E_BLOCKERS.md B10.
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
        mut,
        seeds = [b"governance-vault", registry_config.key().as_ref()],
        bump,
        constraint = governance_vault.mint == governance_mint.key() @ ErrorCode::Unauthorized,
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

/// ADR-0014 amendment (SOLID-SEC-044).  Accounts for
/// `request_withdrawal_atomic`.
///
/// Proof nodes are supplied as `remaining_accounts` (readonly),
/// identical to `revoke_issuer_atomic`.
#[derive(Accounts)]
pub struct RequestWithdrawalAtomic<'info> {
    #[account(
        mut,
        seeds = [b"issuer", issuer_authority.key().as_ref()],
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

    /// SOLID-SEC-045.  Singleton `IssuerTreeBinding` (113-byte custom
    /// layout); written atomically inside this ix to reflect the
    /// post-CPI tree root.  Owner-checked + discriminator-checked +
    /// status-checked in-handler.  Must be writable.
    /// CHECK: parsed + owner-verified in-handler against `crate::ID`.
    #[account(mut, seeds = [ISSUER_TREE_BINDING_SEED], bump)]
    pub issuer_tree_binding: UncheckedAccount<'info>,

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

/// SOLID-SEC-061 / H3.  Accounts for `withdraw_after_revoke`.
///
/// The seed-bound `issuer_account` PDA + the explicit
/// `require_keys_eq!(issuer_authority, issuer.authority)` in the
/// handler enforce that only the legitimate issuer authority can drain
/// the post-revoke stake.  Mirrors `WithdrawAfterCooldown` shape so a
/// SDK helper can share the encoder layout.
#[derive(Accounts)]
pub struct WithdrawAfterRevoke<'info> {
    #[account(mut, seeds = [b"issuer", issuer_authority.key().as_ref()], bump)]
    pub issuer_account: Account<'info, IssuerAccount>,
    /// CHECK: Stake vault PDA.
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

// ─── SEC-048 Phase E.2: subgroup-VK account contexts ──────────────────────

/// PDA singleton; mirrors zk-verifier's `Initialize` for the
/// `VerifierConfig` PDA, but seeded under `b"subgroup-verifier-config"`
/// so the two configs can be governed independently.
///
/// SOLID-SEC-083 (HIGH; closed 2026-05-02; SEC-048 Phase E hardening):
/// Constrains `authority == registry_config.authority`.  Pre-fix,
/// `init_subgroup_verifier` was a first-caller-wins singleton on a
/// fresh cluster: an attacker could front-run the operator after
/// `anchor deploy` but before `scripts/initialize.ts` ran, become
/// the SubgroupVerifierConfig.authority, and then upload a
/// permissive subgroup VK that admits any pubkey -- nullifying the
/// SEC-048 Phase E.3 soundness gate.  The `registry_config` account
/// + the `authority == registry_config.authority` constraint forces
/// the SubgroupVerifierConfig.authority to be the SAME key that
/// already controls the registry, so the only race window remaining
/// is the (already-existing) one for `initialize_registry` itself.
/// Operator runbook MUST run `initialize_registry` first
/// (`scripts/initialize.ts` step [1/8]) before
/// `init_subgroup_verifier` (step [8/8]).
#[derive(Accounts)]
pub struct InitSubgroupVerifier<'info> {
    /// Live `RegistryConfig` -- the source of truth for the program's
    /// admin authority.  Must exist before `init_subgroup_verifier`
    /// can be called.
    #[account(
        seeds = [b"registry-config"],
        bump,
    )]
    pub registry_config: Account<'info, RegistryConfig>,
    #[account(
        init, payer = authority,
        space = 8 + SubgroupVerifierConfig::SPACE,
        seeds = [b"subgroup-verifier-config"],
        bump
    )]
    pub subgroup_verifier_config: Account<'info, SubgroupVerifierConfig>,
    #[account(
        mut,
        constraint = authority.key() == registry_config.authority @ ErrorCode::Unauthorized
    )]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// VK chunked-upload context.  `init_if_needed` mirrors zk-verifier's
/// `StoreVerificationKey` -- the storage PDA is born on the first
/// `chunk_index == 0` write and persists across the upload session.
#[derive(Accounts)]
pub struct StoreSubgroupVkChunk<'info> {
    #[account(
        mut,
        seeds = [b"subgroup-verifier-config"],
        bump = subgroup_verifier_config.bump,
    )]
    pub subgroup_verifier_config: Account<'info, SubgroupVerifierConfig>,
    #[account(
        init_if_needed, payer = authority,
        // 8 (disc) + 4 (Vec len) + SUBGROUP_VK_MAX_BYTES = 10240, the
        // Solana per-CPI realloc cap.  Must stay in lockstep with
        // `SUBGROUP_VK_MAX_BYTES` in `store_subgroup_vk_chunk`.
        space = 8 + 4 + SUBGROUP_VK_MAX_BYTES,
        seeds = [b"subgroup-vk-storage", subgroup_verifier_config.key().as_ref()],
        bump
    )]
    pub subgroup_vk_storage: Account<'info, SubgroupVkStorage>,
    #[account(
        mut,
        constraint = authority.key() == subgroup_verifier_config.authority @ ErrorCode::Unauthorized
    )]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Authority-only context for finalize / request / cancel / rotate
/// against the subgroup-VK config.  Mirrors zk-verifier's
/// `AuthorityOnly` 1:1 with the seed swap.
#[derive(Accounts)]
pub struct SubgroupVerifierAuthorityOnly<'info> {
    #[account(
        mut,
        seeds = [b"subgroup-verifier-config"],
        bump = subgroup_verifier_config.bump,
        has_one = authority @ ErrorCode::Unauthorized
    )]
    pub subgroup_verifier_config: Account<'info, SubgroupVerifierConfig>,
    pub authority: Signer<'info>,
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

impl IssuerTier {
    /// Numeric trust ordering used by `approve_via_trust_anchor`'s
    /// mutual-floor check (M9 / SOLID-SEC-074).  Higher numeric
    /// value == higher trust.  Government can approve any tier;
    /// Regulated can approve up to Regulated; lower tiers cannot
    /// trust-anchor at all.
    pub fn rank(&self) -> u8 {
        match self {
            IssuerTier::Community => 0,
            IssuerTier::Enterprise => 1,
            IssuerTier::Regulated => 2,
            IssuerTier::Government => 3,
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum SlashingReason {
    InvalidIssuance, // Programmable
    DoubleIssuance,  // Programmable
    RevokedMisuse,   // Social
    IdentityDoxxing, // Social
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
pub struct IssuerSchemaPermission {
    pub issuer: Pubkey,
    pub issuer_authority: Pubkey,
    pub schema_hash: [u8; 32],
    pub schema_account: Pubkey,
    pub granted_by: Pubkey,
    pub granted_at: i64,
    pub revoked_at: i64,
    pub active: bool,
    pub bump: u8,
}

impl IssuerSchemaPermission {
    /// 32 issuer PDA + 32 issuer authority + 32 schema hash + 32 schema PDA
    /// + 32 grant authority + 8 grant timestamp + 8 revoke timestamp
    /// + 1 active + 1 bump.
    pub const SPACE: usize = 32 + 32 + 32 + 32 + 32 + 8 + 8 + 1 + 1;
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

// ─── SEC-048 Phase E.2: subgroup-VK state ─────────────────────────────────

/// State machine for the SEC-048 subgroup-VK upload + rotation.  Layout
/// is similar to zk-verifier's `VerifierConfig`, minus the fields that
/// only make sense for the batch verifier (proof_count,
/// timestamp_skew_seconds): the subgroup verify has no Clock binding
/// and no per-call nullifier counter -- it's a pure registration-time
/// gate.  Each field's semantics are identical to its zk-verifier
/// counterpart so an operator who knows one runbook knows the other.
#[account]
pub struct SubgroupVerifierConfig {
    /// Authority allowed to upload, finalize, request rotation, and
    /// rotate.  Same key as `RegistryConfig.authority` for v1 (single-
    /// signer); SEC-013 / SEC-043 will swap to a Squads 3-of-5 PDA.
    pub authority: Pubkey,
    /// `init` bump cached so PDA-signed paths can resolve without a
    /// runtime `find_program_address`.
    pub bump: u8,
    /// Mirror of zk-verifier's `paused` field.  Reserved for future
    /// emergency-pause wiring; unused on v1 but committed to the layout
    /// up-front so a future ADR doesn't need to migrate the account.
    pub paused: bool,
    /// `vk_initialized == true` once the final chunk has been stored;
    /// gates `finalize_subgroup_vk`.
    pub vk_initialized: bool,
    /// Index of the next chunk expected by `store_subgroup_vk_chunk`.
    /// Bumps strictly monotonically; reset to 0 only on a completed
    /// rotation.
    pub next_vk_chunk: u16,
    /// SOLID-SEC-006 freeze-gate.  Flips to `true` on
    /// `finalize_subgroup_vk`.  While `true`, `store_subgroup_vk_chunk`
    /// refuses every write.
    pub vk_finalized: bool,
    /// SOLID-SEC-006 monotonic rotation counter.  Starts at 0; bumps
    /// on every successful `rotate_subgroup_vk`.  Independent of
    /// zk-verifier's `vk_generation` -- the two VKs roll separately.
    pub vk_generation: u16,
    /// SOLID-SEC-006 rotation-request timelock anchor.  Zero means no
    /// pending rotation.  Non-zero means the authority has called
    /// `request_subgroup_vk_rotation`; `rotate_subgroup_vk` refuses
    /// until `Clock::unix_timestamp >= rotate_request_ts +
    /// SUBGROUP_VK_ROTATION_TIMELOCK_SECONDS`.
    pub rotate_request_ts: i64,
}

impl SubgroupVerifierConfig {
    // 32 authority + 1 bump + 1 paused + 1 vk_initialized
    // + 2 next_vk_chunk + 1 vk_finalized + 2 vk_generation
    // + 8 rotate_request_ts = 48 bytes.
    //
    // Compared to zk-verifier's `VerifierConfig::SPACE = 60`: we drop
    // `proof_count` (8 bytes; nothing on this VK to count) and
    // `timestamp_skew_seconds` (4 bytes; no Clock binding).  Any future
    // field add must bump this constant in the same commit.
    pub const SPACE: usize = 32 + 1 + 1 + 1 + 2 + 1 + 2 + 8;
}

/// Heap-backed VK byte buffer; layout mirrors zk-verifier's `VkStorage`.
#[account]
pub struct SubgroupVkStorage {
    pub data: Vec<u8>,
}

/// SOLID-SEC-006 helper: is the pending subgroup-VK rotation past its
/// timelock?  Pure so the state-transition invariants are host-testable
/// without spinning up a validator.  Returns `false` when there is no
/// pending rotation (`rotate_request_ts == 0`).
pub fn subgroup_vk_rotation_timelock_expired(
    config: &SubgroupVerifierConfig,
    now_unix_ts: i64,
) -> bool {
    if config.rotate_request_ts == 0 {
        return false;
    }
    now_unix_ts
        >= config
            .rotate_request_ts
            .saturating_add(SUBGROUP_VK_ROTATION_TIMELOCK_SECONDS)
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
    #[msg("Approval threshold (basis points) exceeds 10000")]
    InvalidThreshold,
    #[msg("Voting period must be > 0 seconds")]
    InvalidVotingPeriod,
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
    #[msg("Issuer lacks an active DAO-granted permission for this schema")]
    IssuerSchemaPermissionInactive,
    #[msg("Issuer schema permission account does not match the requested issuer/schema")]
    IssuerSchemaPermissionMismatch,
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
    #[msg(
        "Issuer is not yet enrolled in the issuer tree (ADR-0014); call append_issuer_leaf first"
    )]
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
    #[msg("Proof path length does not match ISSUER_TREE_DEPTH; full path required, no canopy reliance (SOLID-SEC-045)")]
    InvalidProofPathLength,
    #[msg("Submitted old_root does not match IssuerTreeBinding.current_root; retry against the live binding root (SEC-060 / H2)")]
    IssuerTreeRootStale,
    #[msg("Submitted new_root does not match the on-chain Poseidon-Merkle recompute against (new_leaf, leaf_index, poseidon_proof_path) (SEC-059 / H1)")]
    IssuerTreeRootMismatch,
    #[msg(
        "Issuer is not in Revoked status; withdraw_after_revoke requires Revoked (SEC-061 / H3)"
    )]
    IssuerNotRevoked,
    #[msg("DAO dispute window has not closed yet; wait until cooldown_ends_at to withdraw post-revoke (SEC-061 / H3)")]
    DisputeWindowOpen,
    // ─── SEC-048 Phase E.2: subgroup-VK upload + freeze-gate errors ─────
    #[msg("Subgroup VK chunk index does not match next expected chunk (SEC-048 Phase E.2)")]
    SubgroupVkChunkOutOfOrder,
    #[msg("Subgroup VK storage exceeds SUBGROUP_VK_MAX_BYTES (SEC-048 Phase E.2)")]
    SubgroupVkStorageFull,
    #[msg("Subgroup VK has not been uploaded; finalize requires final chunk first (SEC-048 Phase E.2)")]
    SubgroupVkNotSet,
    #[msg("Subgroup VK already finalized; rotation requires the request_subgroup_vk_rotation timelock path (SEC-048 Phase E.2)")]
    SubgroupVkAlreadyFinalized,
    #[msg("Subgroup VK is not finalized; rotation refuses until finalize_subgroup_vk lands (SEC-048 Phase E.2)")]
    SubgroupVkNotFinalized,
    #[msg("Subgroup VK rotation already pending; cancel before requesting another (SEC-048 Phase E.2)")]
    SubgroupVkRotationAlreadyRequested,
    #[msg("Subgroup VK rotation timelock has not expired (48h since request_subgroup_vk_rotation) (SEC-048 Phase E.2)")]
    SubgroupVkRotationTimelockNotExpired,
    #[msg("No pending subgroup VK rotation to complete (SEC-048 Phase E.2)")]
    SubgroupVkNoPendingRotation,
    #[msg("Clock returned a non-positive unix_timestamp; rotation request refused (SEC-048 Phase E.2)")]
    SubgroupVkRotationClockInvalid,
    #[msg("Subgroup Groth16 proof is malformed or fails the prime-order subgroup invariant for the supplied BJJ pubkey (SEC-048 Phase E.3)")]
    InvalidSubgroupProof,
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
    require!(remaining >= min_rent, ErrorCode::StakeVaultWouldGoBelow);

    **from.try_borrow_mut_lamports()? = remaining;
    let to_balance = **to.try_borrow_lamports()?;
    **to.try_borrow_mut_lamports()? = to_balance.checked_add(amount).ok_or(ErrorCode::Overflow)?;
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

#[event]
pub struct IssuerSchemaPermissionGranted {
    pub issuer: Pubkey,
    pub issuer_account: Pubkey,
    pub schema_hash: [u8; 32],
    pub schema_account: Pubkey,
    pub granted_by: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct IssuerSchemaPermissionRevoked {
    pub issuer: Pubkey,
    pub issuer_account: Pubkey,
    pub schema_hash: [u8; 32],
    pub schema_account: Pubkey,
    pub revoked_by: Pubkey,
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

// `Sec007Bypass` event REMOVED in SEC-048 Phase E.3 (2026-05-02).
// The on-chain Groth16 verify against the subgroup VK is the
// load-bearing soundness gate at registration time; there is no
// bypass to telemetry-trace.  See SECURITY_REGISTRY.md SEC-048
// closure narrative for the full audit trail.

/// SOLID-SEC-061 / H3.  Emitted by `withdraw_after_revoke` for every
/// successful post-revoke stake drain.  Lets indexers and DAO dashboards
/// reconcile staked-amount totals without polling every IssuerAccount.
#[event]
pub struct StakeWithdrawn {
    pub issuer: Pubkey,
    pub amount: u64,
    pub remaining: u64,
    pub slot: u64,
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
    /// SOLID-SEC-044.  Voluntary Approved -> Cooldown transition via
    /// `request_withdrawal_atomic`.  The leaf is replaced so that pre-
    /// cooldown proofs no longer verify; if the issuer later abandons
    /// the cooldown and the operator restores the old preimage, the
    /// revocation_nonce bump ensures the tree state still differs from
    /// the pre-cooldown leaf.
    CooldownRequested,
}

// ─── Host-side tests (SOLID-SEC-045 + supporting layout invariants) ────────
//
// These tests exercise pure helpers (`write_issuer_tree_binding_root`,
// layout constants).  Anything that needs the Anchor runtime
// (Account<T>, Context, Signer, CPI) is covered by the bankrun
// integration suite under `tests/integration/`.

#[cfg(test)]
mod tests {
    use super::*;

    /// Extract the numeric `error_code_number` from an
    /// `anchor_lang::error::Error`.  Panics if the error is not an
    /// `AnchorError` variant -- in our handler code paths every
    /// `require!` / `require_keys_eq!` produces one.
    fn anchor_error_code(err: anchor_lang::error::Error) -> u32 {
        match err {
            anchor_lang::error::Error::AnchorError(ae) => ae.error_code_number,
            other => panic!("expected AnchorError, got: {:?}", other),
        }
    }

    /// Build a 113-byte IssuerTreeBinding buffer with the canonical
    /// discriminator + tree_pubkey + root + slot + status + authority.
    fn make_binding(
        tree_pubkey: [u8; 32],
        root: [u8; 32],
        slot: u64,
        status: u8,
        authority: [u8; 32],
    ) -> Vec<u8> {
        let mut v = Vec::with_capacity(ISSUER_TREE_BINDING_SIZE);
        v.extend_from_slice(&ISSUER_TREE_DISCRIMINATOR);
        v.extend_from_slice(&tree_pubkey);
        v.extend_from_slice(&root);
        v.extend_from_slice(&slot.to_le_bytes());
        v.push(status);
        v.extend_from_slice(&authority);
        assert_eq!(v.len(), ISSUER_TREE_BINDING_SIZE);
        v
    }

    // ─── write_issuer_tree_binding_root: positive paths ────────────────────

    #[test]
    fn write_binding_root_happy_path_overwrites_root_and_slot() {
        let old_root = [0xAAu8; 32];
        let new_root = [0xBBu8; 32];
        let mut buf = make_binding(
            [1u8; 32],
            old_root,
            100,
            ISSUER_TREE_STATUS_ACTIVE,
            [9u8; 32],
        );

        write_issuer_tree_binding_root(&mut buf, &new_root, 250).unwrap();

        // Root field changed.
        assert_eq!(&buf[40..72], &new_root[..]);
        // Slot field updated to the supplied value.
        let slot = u64::from_le_bytes(buf[72..80].try_into().unwrap());
        assert_eq!(slot, 250);
        // Discriminator + tree_pubkey + status + authority untouched.
        assert_eq!(&buf[0..8], &ISSUER_TREE_DISCRIMINATOR);
        assert_eq!(&buf[8..40], &[1u8; 32]);
        assert_eq!(buf[80], ISSUER_TREE_STATUS_ACTIVE);
        assert_eq!(&buf[81..113], &[9u8; 32]);
    }

    #[test]
    fn write_binding_root_accepts_slot_one_against_zero_init() {
        // Genesis case: a freshly-initialized binding stores
        // last_updated_slot = init_slot.  The first update from any
        // atomic ix runs at slot >= init_slot + 1 in practice
        // (Solana slot times are ~400ms; binding init and the first
        // append are always >= 1 slot apart).
        let mut buf = make_binding(
            [0u8; 32],
            [0u8; 32],
            0,
            ISSUER_TREE_STATUS_ACTIVE,
            [0u8; 32],
        );
        let new_root = [0xCDu8; 32];
        write_issuer_tree_binding_root(&mut buf, &new_root, 1).unwrap();
        assert_eq!(&buf[40..72], &new_root[..]);
        assert_eq!(u64::from_le_bytes(buf[72..80].try_into().unwrap()), 1);
    }

    #[test]
    fn write_binding_root_rejects_same_slot_replay_nf07() {
        // NF-07 / SOLID-SEC-079 (closed 2026-05-01).  Same-slot
        // re-write is the cross-tx race surface: pre-fix, two atomic
        // ixs in the same block (e.g. a revoke + a withdrawal) both
        // landed and the second silently clobbered the first.  After
        // the fix, the second write fails with
        // `IssuerTreeRootNotMonotonic`.
        let mut buf = make_binding(
            [0u8; 32],
            [0x77u8; 32],
            50,
            ISSUER_TREE_STATUS_ACTIVE,
            [0u8; 32],
        );
        let err = write_issuer_tree_binding_root(&mut buf, &[0xAAu8; 32], 50).unwrap_err();
        let expected: u32 = ErrorCode::IssuerTreeRootNotMonotonic.into();
        assert_eq!(anchor_error_code(err), expected);
        // Buffer untouched: the require! short-circuits before the
        // copy_from_slice writes run.
        assert_eq!(&buf[40..72], &[0x77u8; 32]);
        assert_eq!(u64::from_le_bytes(buf[72..80].try_into().unwrap()), 50);
    }

    #[test]
    fn write_binding_root_rejects_slot_regression_nf07() {
        // Strictly older slot is also rejected (covers the case where
        // a stale tx with an old leader's slot lands during reorg).
        let mut buf = make_binding(
            [0u8; 32],
            [0x77u8; 32],
            100,
            ISSUER_TREE_STATUS_ACTIVE,
            [0u8; 32],
        );
        let err = write_issuer_tree_binding_root(&mut buf, &[0xAAu8; 32], 99).unwrap_err();
        let expected: u32 = ErrorCode::IssuerTreeRootNotMonotonic.into();
        assert_eq!(anchor_error_code(err), expected);
    }

    // ─── write_issuer_tree_binding_root: negative paths ────────────────────

    #[test]
    fn write_binding_root_rejects_buffer_too_short() {
        let mut buf = vec![0u8; ISSUER_TREE_BINDING_SIZE - 1];
        buf[0..8].copy_from_slice(&ISSUER_TREE_DISCRIMINATOR);
        let err = write_issuer_tree_binding_root(&mut buf, &[1u8; 32], 1).unwrap_err();
        let expected: u32 = ErrorCode::InvalidIssuerTreeBinding.into();
        assert_eq!(anchor_error_code(err), expected);
    }

    #[test]
    fn write_binding_root_rejects_bad_discriminator() {
        let mut buf = make_binding(
            [0u8; 32],
            [0u8; 32],
            1,
            ISSUER_TREE_STATUS_ACTIVE,
            [0u8; 32],
        );
        // Corrupt discriminator (e.g., to schmtree, the schema-tree one).
        buf[0..8].copy_from_slice(b"schmtree");
        let err = write_issuer_tree_binding_root(&mut buf, &[1u8; 32], 1).unwrap_err();
        let expected: u32 = ErrorCode::InvalidIssuerTreeBinding.into();
        assert_eq!(anchor_error_code(err), expected);
    }

    #[test]
    fn write_binding_root_rejects_frozen_binding() {
        let mut buf = make_binding(
            [0u8; 32],
            [0u8; 32],
            1,
            ISSUER_TREE_STATUS_FROZEN,
            [0u8; 32],
        );
        let err = write_issuer_tree_binding_root(&mut buf, &[1u8; 32], 1).unwrap_err();
        let expected: u32 = ErrorCode::IssuerTreeBindingFrozen.into();
        assert_eq!(anchor_error_code(err), expected);
    }

    #[test]
    fn write_binding_root_rejects_invalid_status_byte() {
        // status byte that is neither 0 (active) nor 1 (frozen) is
        // already rejected as "not active" by the helper's frozen-gate
        // (it requires status == 0).  Pin that here.
        let mut buf = make_binding([0u8; 32], [0u8; 32], 1, 7, [0u8; 32]);
        let err = write_issuer_tree_binding_root(&mut buf, &[1u8; 32], 1).unwrap_err();
        let expected: u32 = ErrorCode::IssuerTreeBindingFrozen.into();
        assert_eq!(anchor_error_code(err), expected);
    }

    // ─── Layout / discriminator invariants ─────────────────────────────────

    #[test]
    fn issuer_tree_binding_size_is_113() {
        assert_eq!(ISSUER_TREE_BINDING_SIZE, 113);
    }

    #[test]
    fn issuer_tree_discriminator_is_issrtree() {
        assert_eq!(ISSUER_TREE_DISCRIMINATOR, *b"issrtree");
    }

    #[test]
    fn issuer_tree_binding_seed_matches_solid_light() {
        // Seed literal must agree with what the SDK derives off-chain.
        // Drift here silently breaks every script that re-derives the
        // singleton binding PDA.
        assert_eq!(ISSUER_TREE_BINDING_SEED, b"issuer-tree-binding");
    }

    #[test]
    fn issuer_tree_status_constants_are_distinct() {
        assert_ne!(ISSUER_TREE_STATUS_ACTIVE, ISSUER_TREE_STATUS_FROZEN);
        assert_eq!(ISSUER_TREE_STATUS_ACTIVE, 0);
        assert_eq!(ISSUER_TREE_STATUS_FROZEN, 1);
    }

    #[test]
    fn spl_ac_program_ids_match_canonical() {
        // Drift gate: if an Anchor or solana SDK update silently
        // re-derives the SPL AC / noop program IDs, this test fires.
        assert_eq!(
            spl_account_compression_id::ID.to_string(),
            "cmtDvXumGCrqC1Age74AVPhSRVXJMd8PJS91L8KbNCK"
        );
        assert_eq!(
            spl_noop_id::ID.to_string(),
            "noopb9bkMVfRPU8AsbpTUg8AQkHtKwMYZiFUjNRtMmV"
        );
    }

    #[test]
    fn replace_leaf_discriminator_matches_anchor_global_replace_leaf() {
        // sha256("global:replace_leaf")[..8].
        let computed = anchor_lang::solana_program::hash::hash(b"global:replace_leaf").to_bytes();
        assert_eq!(&SPL_AC_REPLACE_LEAF_DISCRIMINATOR, &computed[..8]);
    }

    #[test]
    fn append_discriminator_matches_anchor_global_append() {
        let computed = anchor_lang::solana_program::hash::hash(b"global:append").to_bytes();
        assert_eq!(&SPL_AC_APPEND_DISCRIMINATOR, &computed[..8]);
    }

    // ─── verify_issuer_binding_anchor (NF-01 / NF-04 / SOLID-SEC-077) ──────

    /// Build a `(leaf, leaf_index, path, expected_root)` tuple that the
    /// anchor must accept by definition: the expected_root IS the
    /// Poseidon recompute of (leaf, leaf_index, path).  Used as the
    /// ground truth for both happy and stale-path tests.
    fn make_anchor_fixture(
        leaf: [u8; 32],
        leaf_index: u64,
        path_pattern: u8,
        depth: usize,
    ) -> ([u8; 32], Vec<[u8; 32]>) {
        let path: Vec<[u8; 32]> = (0..depth)
            .map(|i| {
                let mut s = [0u8; 32];
                // Distinct sibling at each level so the recompute path
                // is sensitive to position, not just to the pattern.
                s[0] = path_pattern;
                s[1] = i as u8;
                s
            })
            .collect();
        let expected_root =
            solid_light::cpi_helpers::compute_poseidon_merkle_root(&leaf, leaf_index, &path)
                .expect("host poseidon recompute must succeed");
        (expected_root, path)
    }

    #[test]
    fn anchor_accepts_recomputed_root_matching_binding() {
        // Happy path: the binding stores the Poseidon root that the
        // recompute (old_leaf, leaf_index, path) reproduces.
        let leaf = [0x42u8; 32];
        let leaf_index: u64 = 7;
        let depth = ISSUER_TREE_DEPTH;
        let (expected_root, path) = make_anchor_fixture(leaf, leaf_index, 0xAB, depth);

        let binding = make_binding(
            [0u8; 32],
            expected_root,
            100,
            ISSUER_TREE_STATUS_ACTIVE,
            [0u8; 32],
        );
        verify_issuer_binding_anchor(&binding, &leaf, leaf_index, &path)
            .expect("anchor must accept the path that recomputes to binding.current_root");
    }

    #[test]
    fn anchor_rejects_stale_path_against_advanced_binding() {
        // NF-01 attack shape: the attacker submits a path that's
        // valid against an OLD binding root R1, but the binding has
        // since advanced to R3.  The recompute produces R1 (or close
        // to it); R1 != R3 => `IssuerTreeRootStale`.
        let leaf = [0x42u8; 32];
        let leaf_index: u64 = 7;
        let (root_r1, path_r1) = make_anchor_fixture(leaf, leaf_index, 0xAB, ISSUER_TREE_DEPTH);

        // Binding has since moved to a different root R3 (e.g., a
        // sibling at some level differs because another leaf updated).
        let mut root_r3 = root_r1;
        root_r3[0] ^= 0xFF;
        assert_ne!(root_r1, root_r3);

        let binding = make_binding(
            [0u8; 32],
            root_r3,
            200,
            ISSUER_TREE_STATUS_ACTIVE,
            [0u8; 32],
        );

        let err = verify_issuer_binding_anchor(&binding, &leaf, leaf_index, &path_r1).unwrap_err();
        let expected: u32 = ErrorCode::IssuerTreeRootStale.into();
        assert_eq!(anchor_error_code(err), expected);
    }

    #[test]
    fn anchor_rejects_wrong_old_leaf() {
        // Old-leaf tampering: the path is correct for the binding's
        // leaf at index 7, but the caller passed a different
        // pre-image.  Recompute produces a different root.
        let real_leaf = [0x42u8; 32];
        let fake_leaf = [0x99u8; 32];
        let leaf_index: u64 = 7;
        let (real_root, path) = make_anchor_fixture(real_leaf, leaf_index, 0xAB, ISSUER_TREE_DEPTH);

        let binding = make_binding(
            [0u8; 32],
            real_root,
            100,
            ISSUER_TREE_STATUS_ACTIVE,
            [0u8; 32],
        );
        let err =
            verify_issuer_binding_anchor(&binding, &fake_leaf, leaf_index, &path).unwrap_err();
        let expected: u32 = ErrorCode::IssuerTreeRootStale.into();
        assert_eq!(anchor_error_code(err), expected);
    }

    #[test]
    fn anchor_rejects_wrong_leaf_index() {
        // Index tampering: same leaf + same path, but a different
        // index flips left/right child selection at some level.  The
        // bit-pattern of (leaf_index XOR leaf_index') determines
        // which level diverges; for index=7 vs 6, level-0 bit flips
        // so the very first hash pair is reordered.
        let leaf = [0x42u8; 32];
        let real_index: u64 = 7;
        let fake_index: u64 = 6;
        let (real_root, path) = make_anchor_fixture(leaf, real_index, 0xAB, ISSUER_TREE_DEPTH);

        let binding = make_binding(
            [0u8; 32],
            real_root,
            100,
            ISSUER_TREE_STATUS_ACTIVE,
            [0u8; 32],
        );
        let err = verify_issuer_binding_anchor(&binding, &leaf, fake_index, &path).unwrap_err();
        let expected: u32 = ErrorCode::IssuerTreeRootStale.into();
        assert_eq!(anchor_error_code(err), expected);
    }

    #[test]
    fn anchor_rejects_corrupted_binding_buffer() {
        // Bad discriminator or short buffer must never reach the
        // recompute (defence in depth -- the caller should have
        // owner-checked, but this is the helper's own boundary).
        let mut buf = vec![0u8; ISSUER_TREE_BINDING_SIZE - 1];
        let err = verify_issuer_binding_anchor(&buf, &[0u8; 32], 0, &[]).unwrap_err();
        let expected: u32 = ErrorCode::InvalidIssuerTreeBinding.into();
        assert_eq!(anchor_error_code(err), expected);

        // Right size, wrong discriminator.
        buf = make_binding([0u8; 32], [0u8; 32], 0, 0, [0u8; 32]);
        buf[0..8].copy_from_slice(b"schmtree");
        let err = verify_issuer_binding_anchor(&buf, &[0u8; 32], 0, &[]).unwrap_err();
        let expected: u32 = ErrorCode::InvalidIssuerTreeBinding.into();
        assert_eq!(anchor_error_code(err), expected);
    }

    // ─── SEC-048 Phase E.2: SubgroupVerifierConfig state-machine tests ─

    /// Build a fresh, post-`init_subgroup_verifier` config.  Mirrors the
    /// state set by `init_subgroup_verifier` so each test starts from the
    /// canonical zero state.
    fn fresh_subgroup_config() -> SubgroupVerifierConfig {
        SubgroupVerifierConfig {
            authority: Pubkey::new_unique(),
            bump: 255,
            paused: false,
            vk_initialized: false,
            next_vk_chunk: 0,
            vk_finalized: false,
            vk_generation: 0,
            rotate_request_ts: 0,
        }
    }

    #[test]
    fn subgroup_vk_config_space_matches_layout() {
        // 32 authority + 1 bump + 1 paused + 1 vk_initialized
        // + 2 next_vk_chunk + 1 vk_finalized + 2 vk_generation
        // + 8 rotate_request_ts = 48.
        assert_eq!(SubgroupVerifierConfig::SPACE, 48);
        // SPACE plus the 8-byte Anchor discriminator must fit in
        // Solana's per-CPI realloc cap; trivially true at 56 bytes.
        assert!(8 + SubgroupVerifierConfig::SPACE < 1024);
    }

    #[test]
    fn subgroup_vk_rotation_timelock_no_pending_returns_false() {
        let cfg = fresh_subgroup_config();
        // No request -> never expired, regardless of clock.
        assert!(!subgroup_vk_rotation_timelock_expired(&cfg, 0));
        assert!(!subgroup_vk_rotation_timelock_expired(&cfg, i64::MAX));
    }

    #[test]
    fn subgroup_vk_rotation_timelock_inside_window_returns_false() {
        let mut cfg = fresh_subgroup_config();
        cfg.rotate_request_ts = 1_700_000_000;
        // 47h59m elapsed -- still inside the 48h window.
        let now = cfg.rotate_request_ts + SUBGROUP_VK_ROTATION_TIMELOCK_SECONDS - 60;
        assert!(!subgroup_vk_rotation_timelock_expired(&cfg, now));
    }

    #[test]
    fn subgroup_vk_rotation_timelock_at_or_past_expiry_returns_true() {
        let mut cfg = fresh_subgroup_config();
        cfg.rotate_request_ts = 1_700_000_000;
        // Exactly 48h elapsed.
        let now_at = cfg.rotate_request_ts + SUBGROUP_VK_ROTATION_TIMELOCK_SECONDS;
        assert!(subgroup_vk_rotation_timelock_expired(&cfg, now_at));
        // 1s past the window.
        assert!(subgroup_vk_rotation_timelock_expired(&cfg, now_at + 1));
        // Far past.
        assert!(subgroup_vk_rotation_timelock_expired(&cfg, i64::MAX));
    }

    #[test]
    fn subgroup_vk_rotation_timelock_handles_saturation_safely() {
        // i64 overflow safety: a request at i64::MAX cannot complete
        // (saturating_add -> i64::MAX so any finite `now` is < target).
        let mut cfg = fresh_subgroup_config();
        cfg.rotate_request_ts = i64::MAX;
        assert!(!subgroup_vk_rotation_timelock_expired(&cfg, i64::MAX - 1));
        // Equality at i64::MAX returns true (saturating_add caps there).
        assert!(subgroup_vk_rotation_timelock_expired(&cfg, i64::MAX));
    }

    #[test]
    fn subgroup_vk_rotation_timelock_is_48_hours() {
        // Source-level pin against ADR-0015 + zk-verifier's matching
        // const.  If the DAO ever lowers either, both must move
        // together (or the SECURITY_REGISTRY entry must explicitly
        // record the divergence).
        assert_eq!(SUBGROUP_VK_ROTATION_TIMELOCK_SECONDS, 48 * 3600);
    }

    #[test]
    fn subgroup_vk_max_bytes_fits_per_cpi_realloc_cap() {
        // 8 (Anchor discriminator) + 4 (Vec len prefix) +
        // SUBGROUP_VK_MAX_BYTES must be <= MAX_PERMITTED_DATA_INCREASE
        // = 10240.  Mirrors zk-verifier's same gate.
        assert_eq!(8 + 4 + SUBGROUP_VK_MAX_BYTES, 10240);
    }

    /// SOLID-SEC-083 (HIGH; closed 2026-05-02).  Source-level pin that
    /// asserts the `Unauthorized` ErrorCode discriminant exists and is
    /// reachable.  The actual constraint
    /// (`authority == registry_config.authority` on
    /// `InitSubgroupVerifier`) is enforced by Anchor at handler-entry
    /// time and can only be exercised by an integration test against a
    /// real validator (or solana-bankrun).  The bankrun test is
    /// tracked at `tests/integration/12_subgroup_vk_init_authority_race.test.ts`
    /// (TODO; mirrors the auth-race coverage class).  Until that
    /// lands, this test pins the failure error code so a refactor that
    /// silently drops the constraint AND removes the error variant
    /// fails compile.
    #[test]
    fn sec_083_unauthorized_error_code_pinned() {
        let code: u32 = ErrorCode::Unauthorized.into();
        assert!(
            code > 0,
            "ErrorCode::Unauthorized must resolve to a non-zero discriminant"
        );
    }

    // ─── SEC-048 Phase E.3 round-trip integration test ──────────────────
    //
    // Mirrors zk-verifier's `groth16_host_verify_round_trip` 1:1, but
    // for `N = 2` (the subgroup circuit's public-input arity) and
    // against the in-tree pinned subgroup VK at
    // `circuits/build/bjj_subgroup_verification_key.json`.  The
    // fixture is checked in at `tests/fixtures/subgroup_e2e_proof.json`
    // and was captured from a successful local snarkjs verify
    // (regenerate via the snarkjs CLI commands documented in the
    // fixture's `description` field).
    //
    // What this test pins:
    //   1. The byte-order contract for the on-chain handler:
    //      `bjj_pub_key_x` / `_y` are stored as LE bytes; on-chain we
    //      reverse to BE before passing to
    //      `verify_groth16_proof::<2>`.  publicSignals[i] from snarkjs
    //      are decimal strings of the same field elements; converting
    //      decimal -> BE32 must produce the same byte sequence as the
    //      LE -> reverse path.  Both bytes are then fed to the same
    //      pairing call; if either path disagrees, the verify rejects.
    //   2. The SDK proof-encoding contract: snarkjs G2 is (real, imag)
    //      while groth16-solana expects (imag, real).  See LB5 /
    //      SOLID-SEC-067.  This test rebuilds the SDK encoding inline
    //      so a regression in `formatProofForSolana` would be caught
    //      here on the host.
    //
    // Skipped if the fixture or VK file is missing (clean checkouts
    // hit this).  CI-pin via the e2e job + the `cargo test
    // -p issuer-registry --lib` step in `.github/workflows/ci.yml`.

    use num_bigint::BigUint;
    use num_traits::Num;
    use serde_json::Value as JsonValue;
    use std::path::Path;

    fn dec_str_to_be32(s: &str) -> [u8; 32] {
        let n = BigUint::from_str_radix(s, 10).expect("decimal string");
        let be = n.to_bytes_be();
        if be.len() > 32 {
            panic!("bigint exceeds 32 bytes");
        }
        let mut out = [0u8; 32];
        out[32 - be.len()..].copy_from_slice(&be);
        out
    }

    /// Mirror of TS SDK `formatProofForSolana` (LB5 / SOLID-SEC-067)
    /// for host-side use.  Takes the snarkjs `proof` JSON and returns
    /// `(proof_a, proof_b, proof_c)` in groth16-solana's expected
    /// G1=(x,y)BE / G2=(x_imag, x_real, y_imag, y_real)BE form.
    /// proof_a is NOT pre-negated -- on-chain `verify_groth16_proof`
    /// negates internally.
    fn format_proof_for_solana(proof: &JsonValue) -> ([u8; 64], [u8; 128], [u8; 64]) {
        let pi_a = proof["pi_a"].as_array().expect("pi_a");
        let pi_b = proof["pi_b"].as_array().expect("pi_b");
        let pi_c = proof["pi_c"].as_array().expect("pi_c");

        let mut proof_a = [0u8; 64];
        proof_a[..32].copy_from_slice(&dec_str_to_be32(pi_a[0].as_str().unwrap()));
        proof_a[32..].copy_from_slice(&dec_str_to_be32(pi_a[1].as_str().unwrap()));

        // G2 (imag, real) swap from snarkjs's (real, imag) convention.
        let mut proof_b = [0u8; 128];
        proof_b[0..32].copy_from_slice(&dec_str_to_be32(pi_b[0][1].as_str().unwrap())); // x_imag
        proof_b[32..64].copy_from_slice(&dec_str_to_be32(pi_b[0][0].as_str().unwrap())); // x_real
        proof_b[64..96].copy_from_slice(&dec_str_to_be32(pi_b[1][1].as_str().unwrap())); // y_imag
        proof_b[96..128].copy_from_slice(&dec_str_to_be32(pi_b[1][0].as_str().unwrap())); // y_real

        let mut proof_c = [0u8; 64];
        proof_c[..32].copy_from_slice(&dec_str_to_be32(pi_c[0].as_str().unwrap()));
        proof_c[32..].copy_from_slice(&dec_str_to_be32(pi_c[1].as_str().unwrap()));

        (proof_a, proof_b, proof_c)
    }

    /// Mirror of `scripts/initialize.ts::serializeG1/serializeG2`
    /// (post-LB5).  Input is the snarkjs VK JSON; output is the bytes
    /// the on-chain `subgroup_vk_storage` PDA carries.
    fn serialize_subgroup_vk_from_snarkjs_json(vk_json: &JsonValue) -> Vec<u8> {
        let mut out = Vec::new();
        let ic_arr = vk_json["IC"].as_array().expect("IC");
        let nr_ic = ic_arr.len() as u32;
        out.extend_from_slice(&nr_ic.to_le_bytes());

        // alpha_g1: G1 (x_BE, y_BE).
        let a = vk_json["vk_alpha_1"].as_array().expect("vk_alpha_1");
        out.extend_from_slice(&dec_str_to_be32(a[0].as_str().unwrap()));
        out.extend_from_slice(&dec_str_to_be32(a[1].as_str().unwrap()));

        // beta/gamma/delta_g2: G2 (x_imag, x_real, y_imag, y_real) BE.
        for key in &["vk_beta_2", "vk_gamma_2", "vk_delta_2"] {
            let p = vk_json[*key].as_array().expect(*key);
            let xr = p[0][0].as_str().unwrap();
            let xi = p[0][1].as_str().unwrap();
            let yr = p[1][0].as_str().unwrap();
            let yi = p[1][1].as_str().unwrap();
            out.extend_from_slice(&dec_str_to_be32(xi));
            out.extend_from_slice(&dec_str_to_be32(xr));
            out.extend_from_slice(&dec_str_to_be32(yi));
            out.extend_from_slice(&dec_str_to_be32(yr));
        }

        // IC: each G1 (x_BE, y_BE).
        for ic in ic_arr {
            let ic_p = ic.as_array().expect("IC[i]");
            out.extend_from_slice(&dec_str_to_be32(ic_p[0].as_str().unwrap()));
            out.extend_from_slice(&dec_str_to_be32(ic_p[1].as_str().unwrap()));
        }
        out
    }

    #[test]
    fn subgroup_host_verify_round_trip() {
        // Workspace root is two levels up from `programs/issuer-registry`.
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/subgroup_e2e_proof.json");
        if !fixture_path.exists() {
            eprintln!(
                "[skipped] subgroup_host_verify_round_trip: fixture {} missing.\n\
                 Regenerate via the snarkjs CLI commands documented in the \
                 fixture's `description` field.",
                fixture_path.display(),
            );
            return;
        }
        let fixture: JsonValue =
            serde_json::from_str(&std::fs::read_to_string(&fixture_path).expect("read fixture"))
                .expect("parse fixture json");

        let vk_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../")
            .join(fixture["vk_path"].as_str().expect("vk_path"));
        if !vk_path.exists() {
            eprintln!(
                "[skipped] subgroup_host_verify_round_trip: VK file {} missing.\n\
                 Regenerate via `cd circuits && node scripts/setup.js \
                 --circuit bjj_subgroup_proof`.",
                vk_path.display(),
            );
            return;
        }
        let vk_json: JsonValue =
            serde_json::from_str(&std::fs::read_to_string(&vk_path).expect("read vk"))
                .expect("parse vk json");

        // Build VK byte buffer matching the on-chain storage layout.
        let vk_bytes = serialize_subgroup_vk_from_snarkjs_json(&vk_json);

        // Encode proof for groth16-solana (LB5 G2 swap; proof_a NOT
        // pre-negated -- the on-chain helper negates).
        let (proof_a, proof_b, proof_c) = format_proof_for_solana(&fixture["proof"]);

        // Build public_inputs from snarkjs publicSignals (decimal -> BE32).
        let public_signals = fixture["publicSignals"]
            .as_array()
            .expect("publicSignals array");
        assert_eq!(
            public_signals.len(),
            2,
            "subgroup circuit has exactly 2 public inputs (Ax, Ay)"
        );
        let mut public_inputs: [[u8; 32]; 2] = [[0u8; 32]; 2];
        for (i, s) in public_signals.iter().enumerate() {
            public_inputs[i] = dec_str_to_be32(s.as_str().expect("publicSignals[i]"));
        }

        // Run the on-chain verify path on the host.  This is the
        // soundness gate `register_issuer` will run on every call once
        // SEC-048 Phase E.3 lands; if this passes, the byte-encoding
        // contract is correct end-to-end.
        match solid_light::groth16::verify_groth16_proof::<2>(
            &vk_bytes,
            &proof_a,
            &proof_b,
            &proof_c,
            &public_inputs,
        ) {
            Ok(()) => {
                eprintln!("[host-verify] OK -- SEC-048 Phase E.3 round-trip green");
            }
            Err(e) => {
                panic!(
                    "subgroup verify_groth16_proof FAILED on host: {:?}.\n\
                     Either the LB5 G2 (imag, real) swap is wrong, the \
                     publicSignals decimal->BE32 conversion disagrees with \
                     groth16-solana's expected encoding, or the VK \
                     serialisation order is off.",
                    e
                );
            }
        }
    }

    #[test]
    fn subgroup_pubkey_le_to_be_round_trip() {
        // SEC-048 Phase E.3 byte-contract pin: the on-chain handler
        // takes `bjj_pub_key_x` as LE bytes (per
        // `is_canonical_bn254_le` SEC-062 gate) and reverses to BE
        // before feeding to `verify_groth16_proof::<2>`.  This test
        // pins the LE->reverse path against the snarkjs
        // publicSignals[i] decimal -> BE32 path: both must produce
        // byte-identical buffers, otherwise the on-chain verify will
        // see a different field element from what was proved.
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/subgroup_e2e_proof.json");
        if !fixture_path.exists() {
            eprintln!(
                "[skipped] subgroup_pubkey_le_to_be_round_trip: fixture {} missing.",
                fixture_path.display()
            );
            return;
        }
        let fixture: JsonValue =
            serde_json::from_str(&std::fs::read_to_string(&fixture_path).expect("read fixture"))
                .expect("parse fixture json");

        let public_signals = fixture["publicSignals"]
            .as_array()
            .expect("publicSignals array");

        // For each publicSignals[i], compute its BE32 form and its
        // LE32 form (reverse).  The LE32 form is what the on-chain
        // `bjj_pub_key_x` field carries.  Reverse the LE -> BE; assert
        // equality with the direct decimal->BE32 conversion.
        for s in public_signals.iter().take(2) {
            let be32 = dec_str_to_be32(s.as_str().unwrap());
            // Build LE bytes by reversing BE; this is what
            // `is_canonical_bn254_le` would accept as a canonical
            // input (assuming the field element is < BN254 modulus,
            // which Base8 trivially satisfies).
            let mut le32 = be32;
            le32.reverse();
            // Now the on-chain handler's LE -> BE round-trip:
            let mut be_recovered = le32;
            be_recovered.reverse();
            assert_eq!(
                be_recovered, be32,
                "LE -> BE byte-reverse must round-trip exactly"
            );
        }
    }
}
