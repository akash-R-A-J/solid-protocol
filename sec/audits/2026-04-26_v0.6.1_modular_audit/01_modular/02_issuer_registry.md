# Modular audit -- programs/issuer-registry

| Field | Value |
| --- | --- |
| File | `programs/issuer-registry/src/lib.rs` |
| LOC | 2497 (lib.rs) + 53 (Cargo.toml) |
| Crate | `issuer-registry` (cdylib + lib), `name = "issuer_registry"` |
| Declared program ID | `5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx` |
| Anchor.toml localnet/devnet | `5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx` |
| Date | 2026-04-26 |
| Auditor | delegated issuer-registry specialist (re-dispatch); persisted by orchestrator |

Scope: every `#[program]` handler; every `#[derive(Accounts)]` struct; every `#[account]` state struct; the `compute_issuer_leaf_bytes` helper; the `transfer_slashed_lamports` helper; the `sec007-skip-onchain` feature flag; the SPL Account Compression CPIs that drive issuer-tree mutation; cross-program reads of `schema-registry`'s `SchemaAccount` and `SchemaTreeBinding`.

INVARIANT 1 (program ID consistency) is GREEN: Anchor.toml [programs.localnet] line 11, [programs.devnet] line 16, deployments/devnet.json lines 26/49/53, and the `declare_id!` literal at lib.rs:17 all agree on `5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx`.

## 2. Crate / build configuration audit

`programs/issuer-registry/Cargo.toml` (53 LOC).

```
[lib]
crate-type = ["cdylib", "lib"]
name = "issuer_registry"
```

This is the standard Anchor program shape. Comment block at Cargo.toml:17-37 documents the `sec007-skip-onchain` feature flag explicitly and links it to docs/E2E_BLOCKERS.md B9, sec/SECURITY_REGISTRY.md SEC-048, docs/IMPROVEMENTS_ROADMAP.md (P0). The flag must NOT be set on mainnet builds; localnet/devnet/CI builds enable it because the on-chain BJJ prime-order check exceeds the BPF 1.4M CU per-tx ceiling.

Dependencies (Cargo.toml:39-52):

- `anchor-lang` workspace, with the `init-if-needed` feature -- used by `StakeTokens.staker_account`, `SlashIssuer.dao_treasury`, `SubmitFraudProof.dao_treasury`. NB: `init-if-needed` historically caused B10 access-violation on `governance_vault`; that path was split into `init_governance_vault`, but the flag remains required for the three remaining `init_if_needed` sites.
- `anchor-spl` workspace -- used for `Mint`, `Token`, `TokenAccount`, `Transfer`.
- `solid-light = path` -- imports `verify_schema_tree_binding_for_issue`, `LightError`, `SCHEMA_REGISTRY_ID`. Used by `issue_credential`. solid-light is the BPF-compatible parser crate.
- `schema-registry = path` (`no-entrypoint`) -- imports `SchemaAccount` so Anchor auto-verifies the cross-program PDA's discriminator + Borsh layout (SOLID-SEC-003). The `no-entrypoint` feature gate prevents pulling the BPF entry of schema-registry into this binary.
- `solid-core = path` -- used for `babyjubjub::{is_on_curve, is_identity, require_in_prime_order_subgroup, BJJPublicKey}` and `poseidon::{u64_to_fr, fr_to_bytes_le, hash_bytes}`.

Build profile / linker flags are NOT overridden in this Cargo.toml; the workspace Cargo.toml controls those.

## 3. State accounts

### 3.1 `RegistryConfig` (lib.rs:2230-2244)

| Field | Type | Bytes |
| --- | --- | --- |
| `authority` | `Pubkey` | 32 |
| `governance_token_mint` | `Pubkey` | 32 |
| `min_stake_lamports` | `u64` | 8 |
| `voting_period_seconds` | `i64` | 8 |
| `approval_threshold_bps` | `u64` | 8 |
| `total_issuers` | `u64` | 8 |
| `active_issuers` | `u64` | 8 |
| `next_issuer_leaf_index` | `u64` | 8 |

Total payload = 112 bytes; with 8-byte Anchor discriminator = 120 bytes. The `InitializeRegistry` account uses `space = 120`. GREEN.

`next_issuer_leaf_index` (ADR-0014) is the singleton counter for `append_issuer_leaf`. Strictly monotone -- doc string says "Never decrements (revocation replaces the leaf in place, it does not free the index)." Verified by handler -- `revoke_issuer_atomic` does NOT touch this counter.

### 3.2 `IssuerAccount` (lib.rs:2272-2329)

`SPACE = 32 + (4 + 64) + (4 + 128) + 32 + 32 + 1 + 1 + 8 * 12 + 1 = 395` (lib.rs:2317-2328). With +8 Anchor discriminator the on-chain account is 403 B.

Twelve numeric fields: `staked_amount`, `registered_at`, `creation_slot`, `cooldown_ends_at`, `votes_for`, `votes_against`, `voting_ends_at`, `credentials_issued`, `slash_count`, `revocation_nonce`, `status_epoch`, `issuer_tree_leaf_index` (each 8 B).

### 3.3 `VoteRecord` (lib.rs:2331-2339)

`voter(32) + issuer(32) + approved(1) + weight(8) + has_voted(1) + released(1)` = 75 B; +8 disc = 83. GREEN.

### 3.4 `StakerAccount` (lib.rs:2341-2347)

`voter(32) + amount_staked(8) + active_votes_count(4) + last_stake_slot(8)` = 52 B; +8 disc = 60. GREEN.

### 3.5 `IssuerTreeBinding` -- raw 113-byte layout (lib.rs:84-102)

NOT an Anchor `#[account]` -- this is a hand-rolled `UncheckedAccount` whose layout is parsed in-handler. Layout documented in lib.rs:91-96:

```
[0..  8)  discriminator = b"issrtree"
[8.. 40)  tree_pubkey      (SPL AC concurrent tree)        -- 32 B
[40.. 72)  current_root                                     -- 32 B
[72.. 80)  last_updated_slot (u64 LE)                       --  8 B
[80.. 81)  status (0 = active, 1 = frozen)                  --  1 B
[81..113)  authority (Pubkey)                               -- 32 B
```

Constants (lib.rs:98-102): `ISSUER_TREE_BINDING_SEED = b"issuer-tree-binding"`, `ISSUER_TREE_BINDING_SIZE = 113`, `ISSUER_TREE_DISCRIMINATOR = *b"issrtree"`, `ISSUER_TREE_STATUS_ACTIVE = 0`, `ISSUER_TREE_STATUS_FROZEN = 1`.

INVARIANT 2 (parser layout) is GREEN: zk-verifier's owner check reads the same offsets via `solid-light`.

`ISSUER_TREE_AUTHORITY_SEED = b"issuer-tree-authority"` (lib.rs:110) is the SINGLETON PDA seed used by `append_issuer_leaf` and the two atomic `replace_leaf` handlers.

### 3.6 Enums

- `IssuerStatus` (lib.rs:2246-2253): `Pending, Approved, Cooldown, Rejected, Revoked`.
- `IssuerTier` (lib.rs:2255-2261): `Community, Enterprise, Regulated, Government`.
- `SlashingReason` (lib.rs:2263-2270): `InvalidIssuance, DoubleIssuance, RevokedMisuse, IdentityDoxxing, Other`.
- `RevokeReason` (lib.rs:2576-2588): `Revoked, Slashed, FraudConfirmed, CooldownRequested`.

## 4. Issuer-leaf hash structure + ADR-0014 cross-reference

`compute_issuer_leaf_bytes` (lib.rs:63-77):

```rust
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
```

INVARIANT 3 (issuer-leaf hash structure) is GREEN against ADR-0014: `Poseidon5(authority, bjj_x, bjj_y, status_epoch, revocation_nonce)`. Field ordering matches.

The leaf is consumed by:
- `batch_credential_query.circom` STEP 0.75 -- the in-circuit Merkle membership proof recomputes this exact Poseidon5 from private inputs; if the field order or the byte encoding here drifts from the circuit, every issuer-tree membership proof silently fails.
- `compute_issuer_leaf_bytes` is called four times: `append_issuer_leaf` (lib.rs:1159), `revoke_issuer_atomic` lines 1275 (OLD) and 1288 (NEW), `request_withdrawal_atomic` lines 1435 (OLD) and 1451 (NEW).

## 5. Status state machine deep dive

Detailed transition matrix:

| From | To | Handler | Bumps `status_epoch`? | Bumps `revocation_nonce`? | Replaces tree leaf? | Updates `IssuerTreeBinding.current_root`? |
| --- | --- | --- | --- | --- | --- | --- |
| Pending | Approved | `finalize_voting` | yes | no | no | n/a |
| Pending | Approved | `approve_via_trust_anchor` | yes | no | no | n/a |
| Pending | Rejected | `finalize_voting` | no | no | no | n/a |
| Approved | Cooldown | `request_withdrawal` (LEGACY) | no | no | no | n/a -- refuses if `is_tree_enrolled` |
| Approved | Cooldown | `request_withdrawal_atomic` | yes | yes | yes via `replace_leaf` | **NO -- caller must `update_issuer_tree_root` separately (SOLID-SEC-045)** |
| Cooldown | Revoked | `withdraw_after_cooldown` (zero stake path) | yes | yes | NO | n/a -- refuses if `is_tree_enrolled` |
| Approved | Revoked | `revoke_issuer` (LEGACY) | yes | yes | NO | n/a -- refuses if `is_tree_enrolled` |
| Approved/Cooldown | Revoked | `revoke_issuer_atomic` | yes | yes | yes via `replace_leaf` | **NO -- caller must `update_issuer_tree_root` separately (SOLID-SEC-045)** |
| Approved/Cooldown | Revoked | `slash_issuer` (when stake < min) | yes | yes | no | n/a -- refuses path if would_revoke && enrolled |
| Approved/Cooldown | Revoked | `submit_fraud_proof` (when stake = 0) | yes | yes | no | n/a -- refuses path if would_revoke && enrolled |

INVARIANT 4 status state machine: GREEN that `Approved <-> Cooldown <-> Revoked` flips status AND replaces tree leaf in atomic handlers. RED on the second half (the SOLID-SEC-045 ask): atomic handlers do NOT update `IssuerTreeBinding.current_root` in the same ix. The handlers CPI `replace_leaf` into the SPL AC tree, which DOES update the on-chain Merkle root inside the tree account, but the `IssuerTreeBinding` PDA's `current_root` cache is only refreshed by the separate `update_issuer_tree_root` ix.

There is a window between the atomic ix and the binding-root push where:
- `replace_leaf` succeeded -> the SPL AC tree's authoritative root has rotated.
- `IssuerTreeBinding.current_root` still holds the pre-revocation root.
- Off-chain, a holder generates a proof against the OLD root (since that is what zk-verifier reads).
- Until `update_issuer_tree_root` lands, the proof verifies despite the issuer being revoked.

This is FINDING M02-H01 (HIGH), tracked upstream as SOLID-SEC-045 / NEW-01 in the v0.6.1 audit. The fix per the open-work list in CLAUDE.md is "write the new root directly into the binding PDA inside the atomic ix."

## 6. BJJ subgroup workaround: full feature-flag analysis

### 6.1 The workaround

Cargo.toml:37 declares the feature `sec007-skip-onchain = []`. lib.rs:221-252 contains the cfg-gated guard:

```rust
let bjj_pub_key = solid_core::babyjubjub::BJJPublicKey {
    x: bjj_pub_key_x,
    y: bjj_pub_key_y,
};
#[cfg(not(feature = "sec007-skip-onchain"))]
{
    solid_core::babyjubjub::require_in_prime_order_subgroup(&bjj_pub_key)
        .map_err(|_| ErrorCode::InvalidBJJPubKey)?;
}
#[cfg(feature = "sec007-skip-onchain")]
{
    require!(
        solid_core::babyjubjub::is_on_curve(&bjj_pub_key),
        ErrorCode::InvalidBJJPubKey
    );
    require!(
        !solid_core::babyjubjub::is_identity(&bjj_pub_key),
        ErrorCode::InvalidBJJPubKey
    );
    msg!(
        "SEC-048: sec007-skip-onchain active; off-chain SDK predicate is enforcement point"
    );
    emit!(Sec007Bypass {
        issuer_authority: ctx.accounts.issuer_authority.key(),
        slot: Clock::get()?.slot,
    });
    let _ = &bjj_pub_key;
}
```

### 6.2 Why the workaround exists

- `require_in_prime_order_subgroup` does a full `r * P == O` cofactor-8 scalar multiplication via arkworks.
- Off-chain (host x86) this costs a few ms.
- On-chain (BPF + arkworks) this exceeds the per-transaction CU ceiling (~1.4M CU).
- Without the bypass, `register_issuer` cannot land at all, blocking every downstream e2e step.

### 6.3 Threat model when feature is ON

- An attacker who registers a small-order pubkey (cofactor-8 torsion point) can forge BJJ-EdDSA signatures within that subgroup.
- Every credential signed by that key would verify with compromised soundness in the in-circuit `EdDSAVerify` template.
- The in-circuit issuer-pubkey gate (Merkle-membership against the issuer tree, ADR-0014) does NOT defend against this -- the issuer tree just contains the bad pubkey as a "valid" leaf.
- The off-chain TS predicate `isInPrimeOrderSubgroup` in `ts-sdk/packages/core/src/index.ts` is the load-bearing gate. If a caller skips it, registration succeeds with a bad key and downstream cryptography is compromised.

### 6.4 When feature is OFF (mainnet target)

- The full `require_in_prime_order_subgroup` runs on-chain.
- BPF cost > 1.4M CU -> the entire transaction fails -> no `register_issuer` succeeds.
- Conclusion: the protocol cannot be deployed to mainnet with the feature-OFF code path until SEC-048 ships a cheaper on-chain replacement.

INVARIANT 7 (BJJ subgroup check): VERIFIED. There is no static/CI gate enforcing "mainnet builds must compile without this feature" beyond the comment block in Cargo.toml. This is FINDING M02-H02 (HIGH).

### 6.5 Telemetry

The `Sec007Bypass` event (lib.rs:2566-2570) emits `(issuer_authority, slot)` on every bypass execution. The doc comment says off-chain monitors should subscribe to this and alert. There is no on-chain enforcement of cluster-id, so the event is a strictly off-chain signal.

### 6.6 Consolation gates in the bypass arm

In the OFF-chain-skip path:
- `is_on_curve(&bjj_pub_key)` must be true (rejects off-curve points).
- `!is_identity(&bjj_pub_key)` must be true (rejects neutral element).

These two gates fit in the BPF budget. They reject the obvious failures but a cofactor-8 torsion point still passes here.

## 7. Per-handler deep dive

### 7.1 `initialize_registry` (lib.rs:124-143)

INPUTS: 4 args (`governance_token_mint`, `min_stake`, `voting_period`, `approval_threshold`); accounts (`registry_config` init PDA, `authority` Signer, `system_program`).

PROCESSING: All eight `RegistryConfig` fields written exactly once. Counters initialised to 0. `msg!` logs the four configurable params.

OUTPUTS: `Ok(())`. No event.

ERROR HANDLING: Anchor-builtin only (init constraint). Zero unwraps. No bounds check on `voting_period_seconds` (i64; negative values produce already-elapsed voting -> auto-rejection). FINDING M02-L01.

### 7.2 `init_governance_vault` (lib.rs:160-177)

Splits SPL TokenAccount init from `stake_tokens` (B10 fix). `require_keys_eq!(governance_mint == registry.governance_token_mint, Unauthorized)` guards against pre-creation under sock-puppet mint.

ERROR HANDLING: `Unauthorized`, Anchor `init` errors.

### 7.3 `register_issuer` (lib.rs:179-316)

INPUTS: `name` (cap 64), `metadata_uri` (cap 128), `bjj_pub_key_{x,y}`, `tier`. Accounts: `registry_config` (mut), `issuer_account` (init, seeds=[b"issuer", authority]), `stake_vault`, `issuer_authority` Signer.

PROCESSING:
1. Length checks (lib.rs:187-188).
2. cfg-gated subgroup gate (Section 6).
3. Tier multiplier: Community=1, Enterprise=10, Regulated=5, Government=0.
4. `stake_amount = min_stake.checked_mul(multiplier).ok_or(Overflow)?`. GREEN.
5. If stake_amount > 0: System Transfer to stake_vault.
6. Field-by-field write of IssuerAccount.
7. `config.total_issuers += 1` (NOT checked_add; M02-INF02).

OUTPUTS: `msg!`. Conditional `Sec007Bypass` event when feature ON.

ERROR HANDLING: `NameTooLong`, `MetadataTooLong`, `InvalidBJJPubKey`, `Overflow`. Zero unwraps.

### 7.4 `vote_on_issuer` (lib.rs:329-391)

PROCESSING:
1. `now_slot >= last_stake_slot + 100` (flash-loan protection).
2. `voter_weight = staker.amount_staked > 0`.
3. `now_ts < voting_ends_at`.
4. `status == Pending`.
5. Conditional `votes_{for,against} += weight` with checked_add.
6. Write VoteRecord.
7. `staker.active_votes_count = checked_add(1)` -- locks stake.

ERROR HANDLING: `StakeTooNew`, `NoVotingPower`, `VotingPeriodEnded`, `IssuerNotPending`, `Overflow`.

### 7.5 `withdraw_stake` (lib.rs:395-415)

Refunds rejected issuers. Direct lamport manipulation: `**stake_vault.lamports -= amount`, `**issuer_authority.lamports += amount`. NOT checked_sub.

FINDING M02-M01: route through `transfer_slashed_lamports` for parity with slash path.

### 7.6 `stake_tokens` (lib.rs:426-453)

SPL Token CPI Transfer from `voter_token_account` to `governance_vault`. `voter_token_account.mint == governance_mint.key()` constraint enforced.

### 7.7 `unstake_tokens` (lib.rs:456-492)

NB: NO `voter_token_account.mint` constraint! FINDING M02-M02. Compare to `stake_tokens` which has it. Asymmetric.

PROCESSING: `active_votes_count == 0`, `amount_staked >= amount`, CPI Transfer signed by governance_vault PDA, `staked_amount -= amount`.

### 7.8 `release_vote` (lib.rs:501-514)

`!released`, `staker.active_votes_count = checked_sub(1)`, `released = true`. Constraint: `issuer.status != Pending`.

### 7.9 `request_withdrawal` (LEGACY) (lib.rs:527-550)

`status == Approved`, `!is_tree_enrolled` (else `IssuerTreeUpdateRequired`), `status = Cooldown`, `cooldown_ends_at = unix_timestamp + 14 days`. Does NOT bump `revocation_nonce` or `status_epoch`. Legacy path for pre-tree-backfill issuers only.

### 7.10 `withdraw_after_cooldown` (lib.rs:553-596)

`status == Cooldown`, `unix_timestamp >= cooldown_ends_at`, `amount <= staked_amount`, ADR-0014 enrollment guard if `amount == staked_amount && is_tree_enrolled`. Direct lamport ops. If `staked_amount == 0` after: status = Revoked, status_epoch = slot, revocation_nonce += 1.

### 7.11 `finalize_voting` (lib.rs:598-661)

Permissionless after voting deadline. u128 percentage math: `approval_pct = (votes_for_u128 * 10000) / total_votes_u128`. If `approval_pct >= approval_threshold_bps`: status = Approved, emit `IssuerApproved`. Else Rejected.

`payer: mut Signer` is never charged -- M02-INF04.

### 7.12 `slash_issuer` (lib.rs:670-745)

DAO authority only. `slash_amount <= staked_amount`. ADR-0014 enrollment guard. `staked_amount = checked_sub(slash_amount)`, `slash_count = checked_add(1)`. Routes through `transfer_slashed_lamports` (rent-floor guarded). If staked_amount < min_stake: status = Revoked, status_epoch = slot, revocation_nonce = checked_add(1), active_issuers -= 1.

NB: `issuer_account` lacks seed constraint (M02-L02).

### 7.13 `submit_fraud_proof` (lib.rs:758-826)

`reporter == registry_config.authority` (closes free stake-drain attack). `proof_data.len() <= 4096`. `reason in {InvalidIssuance, DoubleIssuance}`. Same shape as slash. `proof_data` is currently ignored (forward-compat).

### 7.14 `approve_via_trust_anchor` (lib.rs:837-887)

`anchor.tier in {Government, Regulated}`, `anchor.status == Approved`, `target.status == Pending`. `target_authority` is instruction arg; PDA seeded by it. `target.status_epoch = slot`, `active_issuers += 1`. Emit `IssuerApproved`.

### 7.15 `check_issuer_status` (lib.rs:890-898)

Read-only CPI target. Returns Ok if `status == Approved`, else `IssuerNotApproved`.

### 7.16 `revoke_issuer` (LEGACY) (lib.rs:908-926)

`!is_tree_enrolled`. `status = Revoked`, `status_epoch = slot`, `revocation_nonce = checked_add(1)`, `active_issuers -= 1`.

### 7.17 `initialize_issuer_tree_binding` (lib.rs:954-1005)

`require_keys_eq!(authority == registry_config.authority, Unauthorized)`. System CreateAccount CPI with PDA self-signing. Owner = `crate::ID`. Writes 113 bytes including discriminator, tree_pubkey, zero root, slot, status=ACTIVE, authority pubkey.

### 7.18 `update_issuer_tree_root` (lib.rs:1015-1053)

Owner check -> length check -> discriminator -> status_active -> stored authority equal to signer -> slot strictly monotonic -> write root + slot.

TWO `try_into().unwrap()` calls (lines 1039, 1045). Statically infallible by surrounding length guards but cosmetically violates "zero unwraps" rule. M02-INF05.

INVARIANT 5 (issuer_tree_operator authority): SINGLE-SIGNER. M02-M03 (SOLID-SEC-043).

### 7.19 `set_issuer_tree_binding_status` (lib.rs:1059-1085)

Reuses `UpdateIssuerTreeRoot` accounts. `status in {ACTIVE, FROZEN}`, owner+disc+length, stored_authority equals signer, write status byte. Same `try_into().unwrap()` pattern.

### 7.20 `append_issuer_leaf` (lib.rs:1139-1227)

`authority == registry.authority`, `status == Approved`, `!is_tree_enrolled`, compute leaf via `compute_issuer_leaf_bytes`, validate compression_program/log_wrapper/tree_authority IDs, build SPL AC `append` ix data (40 bytes), `invoke_signed`. Updates `next_issuer_leaf_index` (checked_add), `issuer.issuer_tree_leaf_index`, `is_tree_enrolled = true`. Emits `IssuerLeafAppended`.

### 7.21 `revoke_issuer_atomic` (lib.rs:1257-1373)

Strict order: authority gate -> enrollment gate -> source-status gate -> compute OLD leaf -> bump revocation_nonce + status_epoch -> flip to Revoked -> compute NEW leaf -> u32 cast (try_into) -> CPI replace_leaf with old_root + old_leaf + new_leaf + leaf_index_u32 -> active_issuers -= 1 -> emit `IssuerLeafReplaced`.

CRITICAL FINDING M02-H01: `IssuerTreeBinding.current_root` NOT updated here.

### 7.22 `request_withdrawal_atomic` (lib.rs:1415-1533)

Self-only (`issuer_authority == issuer.authority`). `is_tree_enrolled`, `status == Approved`. Strict order: OLD leaf -> bump nonce + epoch -> status = Cooldown, cooldown_ends_at -> NEW leaf -> CPI replace_leaf -> emit `IssuerLeafReplaced { reason: CooldownRequested }`. NO `active_issuers -= 1` (Cooldown still counts active).

CRITICAL FINDING M02-H01 reiterated.

### 7.23 `issue_credential` (lib.rs:1558-1689)

`status == Approved`, `issuer.authority == issuer_authority`, `commitment != [0;32] && commitment != [0xFF;32]`. SOLID-SEC-003 cross-program checks: `!schema_account.deprecated`, `schema_account.schema_hash == schema_hash`, `*schema_tree_binding.owner == SCHEMA_REGISTRY_ID`, `verify_schema_tree_binding_for_issue` returns Ok. CPI invoke_signed for SPL AC append. `credentials_issued = checked_add(1)`. Emit `CredentialIssued`.

## 8. Per-error catalogue (lib.rs:2351-2453)

49 distinct error variants. Severity mapping:

| Error | Severity | Notes |
| --- | --- | --- |
| `Unauthorized` | high | All auth-vs-recorded gates |
| `Overflow` | high | `checked_*` failure |
| `InvalidBJJPubKey` | high | Section 6 |
| `InvalidIssuerTreeBindingOwner` | high | Invariant 2 |
| `IssuerTreeRootNotMonotonic` | high | Replay protection |
| `IssuerTreeUpdateRequired` | high | Atomic-flow gate |
| `PoseidonFailed` | medium | Should never fire |
| `InvalidCompressionProgram`, `InvalidNoopProgram`, `InvalidTreeAuthority`, `InvalidIssuerTreeAuthority` | medium | CPI gate |
| `SchemaHashMismatch`, `SchemaDeprecated`, `InvalidSchemaTreeBindingOwner`, `TreeBindingMismatch`, `SchemaTreeBindingFrozen`, `InvalidSchemaTreeBinding` | medium | SOLID-SEC-003 |
| `StakeVaultWouldGoBelow` | medium | SOLID-SEC-030 |

`unwrap()` audit: only 3 calls, all in `update_issuer_tree_root`/`set_issuer_tree_binding_status` (`try_into().unwrap()` on length-guaranteed slices). Statically infallible. Cosmetic.

`expect()` count: zero.

Casts: u64 -> u128 widening (GREEN). u128 -> u64 truncating in `IssuerApproved.approval_bps` where value is bounded <= 10000. u64 -> u32 via explicit `try_into` for leaf_index. No `as` narrowing.

## 9. Findings (severity-sorted)

### HIGH

**M02-H01: Atomic handlers do not update `IssuerTreeBinding.current_root` in the same ix.**
- Paths: `revoke_issuer_atomic` (lib.rs:1257-1373), `request_withdrawal_atomic` (lib.rs:1415-1533).
- Impact: between the atomic ix and the operator's separate `update_issuer_tree_root` call, `IssuerTreeBinding.current_root` is stale. Pre-revocation proofs continue to verify in that window.
- Status: SOLID-SEC-045 / NEW-01. Fix: write the post-replace SPL AC root into `IssuerTreeBinding[40..72]` and bump `[72..80]` inside the atomic ix.

**M02-H02: `sec007-skip-onchain` feature flag has no static / CI enforcement against mainnet builds.**
- Cargo.toml comment is documentation, not a gate. A misbuild pushed to mainnet would silently allow registration of cofactor-8 torsion BJJ keys.
- Fix: extend `scripts/check_program_ids.py` (or the release-build script) to inspect compiled feature set and reject mainnet builds with `sec007-skip-onchain` active.

### MEDIUM

**M02-M01: `withdraw_stake` uses unchecked lamport arithmetic.**
- lib.rs:404-413. Lacks `checked_sub` and rent-floor guard. Same gap in `withdraw_after_cooldown` (lib.rs:573-582).
- Fix: route through `transfer_slashed_lamports` for parity.

**M02-M02: `unstake_tokens` does not constrain `voter_token_account.mint`.**
- Asymmetric vs `StakeTokens`. Fix: add `governance_mint` to `UnstakeTokens` struct and the same constraint.

**M02-M03: `issuer_tree_operator` is a single-signer authority (SOLID-SEC-043).**
- Every gated handler checks `authority == registry_config.authority`. Single key is the trust root.

### LOW

**M02-L01: `voting_period_seconds: i64` accepts negative values without bounds check.** Fix: `require!(voting_period > 0)`.

**M02-L02: `slash_issuer.issuer_account` and `submit_fraud_proof.issuer_account` lack seed constraints.** Hygiene; pin seeds like `RevokeIssuerAtomic` does.

### INFORMATIONAL

**M02-INF01**: `approval_threshold_bps` has no upper bound. Add `require!(approval_threshold <= 10000)`.

**M02-INF02**: `total_issuers += 1` and `active_issuers += 1` use unchecked add (lines 308, 630, 731-733).

**M02-INF03**: `voter_token_account.mint` constraint in `StakeTokens` lacks `@ ErrorCode::...`.

**M02-INF04**: `FinalizeVoting.payer` is a Signer that pays nothing. Rename to clarify intent.

**M02-INF05**: Three `try_into().unwrap()` calls in `update_issuer_tree_root` and `set_issuer_tree_binding_status`. Statically infallible.

**M02-INF06**: Tier multiplier ordering is non-monotonic (Community=1 < Regulated=5 < Enterprise=10, Government=0). Document rationale.

**M02-INF07**: Tier rationale not in code. Add to `docs/MODULE_CONTRACTS.md`.

## 10. Compute notes

- `register_issuer` with `sec007-skip-onchain` ON: ~120-180k CU.
- `register_issuer` with `sec007-skip-onchain` OFF: > 1.4M CU (DOES NOT LAND).
- `append_issuer_leaf`: SPL AC `append` dominant, ~80-120k CU.
- `revoke_issuer_atomic` / `request_withdrawal_atomic`: SPL AC `replace_leaf` plus two Poseidon hashes; total ~700k-1.0M CU range with proof-path nodes.
- `issue_credential`: SPL AC `append` plus schema-binding parse; ~150-200k CU.

SOLID-SEC-046 (CU-budget regression gate) is OPEN.

## 11. Dependency notes

- `solid-core`: `babyjubjub::*`, `poseidon::*`. BPF-compatible. Cross-language vector parity in `tests/vectors/`.
- `solid-light`: `verify_schema_tree_binding_for_issue`, `LightError`, `SCHEMA_REGISTRY_ID`. Used by `issue_credential` only.
- `schema-registry` (`no-entrypoint`): typed `SchemaAccount` for SOLID-SEC-003.
- `anchor-lang` workspace + `init-if-needed`.
- `anchor-spl` workspace.
- SPL Account Compression: hand-rolled CPI -- not a Cargo dep.
- SPL Noop: hand-rolled CPI target.

## 12. Open questions / TODOs

1. **M02-H01 (SOLID-SEC-045)**: confirm with verifier that writing post-replace SPL AC root directly into `IssuerTreeBinding[40..72]` from the atomic ix matches what zk-verifier reads.
2. **M02-H02 (build-side enforcement)**: investigate cleanest CI gate for "no `sec007-skip-onchain` on mainnet".
3. **M02-M03 (SOLID-SEC-043)**: design Squads 3-of-5 PDA wiring.
4. ADR-0014 `revocation_nonce` re-approval semantics: re-registration creates a new IssuerAccount via `init`; lifecycle "previously revoked re-approved in place" cannot occur. Verify doc matches.
5. `IssueCredential.merkle_tree` verified against `schema_tree_binding.tree_pubkey` via `verify_schema_tree_binding_for_issue`. Confirm helper enforces equality (it does per solid-light).

## 13. Suggested next actions

1. **Land SOLID-SEC-045 fix (M02-H01).** Both atomic handlers should write post-replace root + slot into binding PDA.
2. **Add CI gate against `sec007-skip-onchain` on mainnet builds (M02-H02).**
3. **M02-M01**: route `withdraw_stake` and `withdraw_after_cooldown` through `transfer_slashed_lamports`.
4. **M02-M02**: add `governance_mint` and mint constraint to `UnstakeTokens`.
5. **M02-M03 / SOLID-SEC-043** before external audit ships.
6. **M02-L01**: bound `voting_period_seconds > 0`.
7. **M02-INF01..INF07**: cosmetic / hygiene.
8. **Cross-language vector regression (SOLID-SEC-010)**: extend gen_vectors.rs to cover `compute_issuer_leaf_bytes`.
9. **CU-budget regression gate (SOLID-SEC-046)**.

---

End of audit. 23 handlers covered (sections 7.1 through 7.23). Twelve specific findings issued (2 HIGH, 3 MEDIUM, 2 LOW, 7 INFORMATIONAL).
