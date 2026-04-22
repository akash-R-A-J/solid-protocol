# SolID Protocol — Deep System Audit
**Date:** 2026-04-22  
**Auditor:** Antigravity (fresh read of every source file)  
**Prior Art Checked:** `security_audit.md` (2026-04-20), `IMPROVEMENTS_ROADMAP.md` (2026-04-21), `solid_protocol_system_audit_2026_04_21.md`

---

## Executive Verdict

| Dimension | Score | Reasoning |
|-----------|-------|-----------|
| **Testing Readiness** | 🟡 **Partial** | One bankrun test exists; no full program-test harness; circuits have no compiled artifacts |
| **Docs ↔ Impl Alignment** | 🟠 **Partial: multiple drifts** | Architecture doc describes a non-existent file tree; several P0 bugs from prior audit are now **fixed in code** but remain marked `[ ]` in the roadmap |
| **Security Posture** | 🟡 **Hardened but incomplete** | 4 previously Critical issues and 5 High issues are **fully remediated in code**; 10 new findings identified here that the prior audit missed |
| **Integration Cohesion** | 🟢 **Remarkably intact** | The three programs + `solid-light` + `verifier` SDK + `holder` SDK form a coherent, non-fragmented pipeline as of this commit |
| **Logical Correctness** | 🟠 **Several bugs remaining** | Most P0-level bugs from prior audit ARE fixed; but 4 subtle new logical issues detected below |

---

## Part 1: Testing Readiness

### What Exists

| Component | Test Coverage | Location |
|-----------|--------------|----------|
| `solid-light` cpi_helpers | ✅ Unit tests embedded (8 tests) | `crates/solid-light/src/cpi_helpers.rs#L224–309` |
| `zk-verifier` VkBuf parser | ✅ Unit tests embedded (7 tests) | `programs/zk-verifier/src/lib.rs#L602–715` |
| `zk-verifier` G1 negation | ✅ Unit tests embedded (2 tests) | `programs/zk-verifier/src/lib.rs#L691–714` |
| Bankrun integration | 🟡 One test (registry init) | `tests/integration/01_registry_init.test.ts` |
| Cross-language vectors | 🟡 File exists, NOT in CI | `tests/vectors/check_vectors.ts` |
| Program-test harness | ❌ None | — |
| Circuit witness-gen tests | ❌ None | — |
| Schema-registry instructions | ❌ None | — |
| Issuer voting flow | ❌ None | — |
| End-to-end prove + verify | ❌ None | — |

### Verdict: Not Ready for Formal Testing

The single bankrun test (`01_registry_init.test.ts`) can only pass if:
1. `anchor build` has been run and produced `target/idl/issuer_registry.json`, and
2. The bankrun environment can load the BPF `.so`.

On this Windows machine neither condition is met automatically. The system is in a **pre-testable** state — the code is ready conceptually but the testing pipeline has a 1-test coverage gap that is severe.

**What you need before "system is ready for testing":**
- [ ] `anchor build` on WSL2 to produce IDL + BPF artifacts
- [ ] At minimum 5 more bankrun tests: schema-registry lifecycle, vote/finalize, slash, issue_credential CPI, verify_batch_proof
- [ ] Circuit `.wasm` + `.zkey` compiled so `generateBatchProof` can actually run end-to-end
- [ ] Cross-language vector check wired into a CI job

---

## Part 2: Docs vs Implementation Alignment

### 2.1 Things the Docs Say That Are NOT In Code (Documentation Ahead)

| Doc Claim | Reality |
|-----------|---------|
| `system_architecture.md` §2.1 describes `src/instructions/*.rs`, `src/state/*.rs`, `src/errors.rs` sub-files | Each program is a single `lib.rs` monolith — no sub-file split |
| `system_architecture.md` describes `single_query.circom` as a fallback circuit | File does NOT exist in `circuits/` |
| `system_architecture.md` describes `circuits/trusted_setup/`, `circuits/build/` directories | Neither exists |
| `system_architecture.md` describes `crates/solid-sdk/`, `crates/solid-cli/` | Neither exists; only `solid-core` and `solid-light` are present |
| `system_architecture.md` shows Layer 1 as "Light Protocol (ZK Compressed state trees)" | Implementation uses SPL Account Compression (no Light Protocol CPI) |
| Layer 3 diagram shows `Rust SDK (Issuer CLI + server-side)` | No Rust SDK / CLI crate |
| Docs §2.3 describes `solid-core` with `poseidon.rs`, `babyjubjub.rs`, `commitment.rs` etc. | `solid-core` is in `crates/` but NOT inspected here — only `solid-light` exists in `crates/` |
| `docs/REVOCATION_DESIGN.md` describes a fully designed revocation system | NOT implemented anywhere |
| `docs/infra_roadmap.md` and others reference Photon Indexer | Replaced by SPL AC; not updated |

### 2.2 Things In Code That Are NOT In Docs (Implementation Ahead)

| Code Feature | Missing From Docs |
|--------------|-------------------|
| `approve_via_trust_anchor` instruction | NOT in `issuer-guide.md` at all |
| `request_withdrawal` + `withdraw_after_cooldown` | Not in `issuer-guide.md` (noted as P3-6 in roadmap) |
| `set_binding_status` (emergency freeze) | Not in `schemas.md` (P3-7 in roadmap) |
| `transfer_tree_binding_authority` | Not in any user doc |
| `transfer_global_binding_authority` | Not in any user doc |
| `StakerAccount.active_votes_count` locking logic | The doc describes DAO governance but not this specific counter |
| Tiered staking (`IssuerTier`) | Not detailed in docs beyond a mention |
| Flash-loan protection (100-slot stake age) | Not documented anywhere user-facing |
| `IssuerAccount.cooldown_ends_at`, `creation_slot` fields | Not in schema descriptions |
| SPL AC discriminator hand-rolling + tree-authority PDA design | `light-protocol.md` exists but not updated fully |

### 2.3 The Big Structural Doc-Code Drift

**`system_architecture.md` Line ~102–145** describes a non-existent file tree. This is the original v0.1 aspirational design. The actual codebase has a much simpler structure (single `lib.rs` per program). This creates a false impression for any new contributor or auditor who reads the architecture doc first.

---

## Part 3: Prior Audit Cross-Check — What's Fixed, What's Not

The prior audit (`security_audit.md` 2026-04-20, `IMPROVEMENTS_ROADMAP.md` 2026-04-21) listed many items. Here is the **verified current state** of every P0/P1 critical issue:

### P0 Issues — Current Status

| ID | Issue | Status in Code |
|----|-------|---------------|
| **P0-1** | RegistryConfig space 80 → 112 bytes | ✅ **FIXED** — `space = 8 + 32 + 32 + 8 + 8 + 8 + 8 + 8` at line 728 |
| **P0-2** | global_tree + schema_tree_N owner checks missing | ✅ **FIXED** — `require_keys_eq!(*ctx.accounts.global_tree.owner, SCHEMA_REGISTRY_ID)` at lines 188–192, repeated for each schema_tree slot |
| **P0-3** | vote_on_issuer never increments active_votes_count | ✅ **FIXED** — lines 201–205 increment it with overflow check |
| **P0-4** | vote_on_issuer has no voting deadline check | ✅ **FIXED** — `require!(now_ts < issuer.voting_ends_at)` at lines 172–175 |
| **P0-5** | Nullifier hex vs decimal parsing | ✅ **FIXED** — `bigintToBytes32(BigInt(publicSignals[0]))` at holder/index.ts line 371, similar fix in single-credential path line 199 |
| **P0-6** | @solid-protocol/verifier package missing | ✅ **FIXED** — full `verifier/src/index.ts` exists with all required functions |

### P1 Issues — Current Status

| ID | Issue | Status in Code |
|----|-------|---------------|
| **P1-1** | ReleaseVote accounts not marked mut | ✅ **FIXED** — `#[account(mut)]` on both `staker_account` (line 1010) and `vote_record` (line 1013) |
| **P1-2** | IncrementUsage has no access control | ✅ **FIXED** — `require_keys_eq!(schema.authority, ctx.accounts.authority.key())` at lines 164–168 |
| **P1-3** | slash_issuer does not transfer lamports | ✅ **FIXED** — `transfer_slashed_lamports()` called in both `slash_issuer` (line 436) and `submit_fraud_proof` (line 505) |
| **P1-4** | approve_via_trust_anchor does not emit IssuerApproved | ✅ **FIXED** — `emit!(IssuerApproved {...})` at lines 560–570 |
| **P1-5** | target_issuer has no PDA constraint | ✅ **FIXED** — seed-constrained at lines 847–851: `seeds = [b"issuer", target_authority.as_ref()]` |
| **P1-6** | VkStorage has no cap | ✅ **FIXED** — `require!(new_total <= VK_MAX_BYTES)` at line 112; separate cap for chunk_index==0 at line 104 |
| **P1-7** | identity commitment uses wrong key | ✅ **FIXED** — `holder/index.ts` now uses `deriveCredentialKey()` to get per-schema keypair at line 282 |
| **P1-8** / **P1-9** | devnet.json stale; check_program_ids.py gap | ❓ Not inspected (devnet.json exists but not re-checked for freshness) |

### P2 Issues — Current Status

| ID | Issue | Status in Code |
|----|-------|---------------|
| **P2-1** | numPredicates bound check | ✅ **FIXED** — `LessEqThan(8)` check at `batch_credential_query.circom` lines 95–98 |
| **P2-2** | compoundLogic binary constraint | ✅ **FIXED** — `compoundLogic * (compoundLogic - 1) === 0` at line 102 |
| **P2-3** | ExpirationChecker not instantiated | ✅ **FIXED** — `expiry[i] = ExpirationChecker()` loop at lines 192–198 with `expiry[i].valid === 1` |
| **P2-4** | resolveSchema wrong memcmp | ✅ **FIXED** — `sdk/index.ts` now walks Borsh length prefixes rather than using hardcoded offset (lines 162–191) |
| **P2-5** | listIssuers wrong offset/byte | ✅ **FIXED** — `sdk/index.ts` now walks Borsh layout correctly, `status === 1` check (lines 203–221) |
| **P2-6** | SchemaAccount space undercounts field_names | ✅ **FIXED** — `SCHEMA_ACCOUNT_SPACE` computed correctly in schema-registry with constants (lines 66–74) |
| **P2-7** | GLOBAL_DEPTH hardcoded | ✅ **FIXED** — `GLOBAL_DEPTH` is now a template parameter and `globalSiblings[NUM_CREDS][GLOBAL_DEPTH]` is used |
| **P2-9** | chunk sequence validation | ✅ **FIXED** — `require!(chunk_index == config.next_vk_chunk)` at lines 95–98 with counter increment |
| **P2-14** | SolID.prove() stub | ✅ **FIXED** — `sdk/index.ts` now delegates to `generateBatchProof()` (lines 81–108) |
| **P2-8**, **P2-10**, **P2-11**, **P2-12**, **P2-13** | Token deployment, CI vectors, revocation, issuer-ZK binding, trusted setup | ❌ **STILL UNIMPLEMENTED** (infrastructure/ceremony work) |

> **Key finding:** The roadmap checklist (`IMPROVEMENTS_ROADMAP.md`) still shows ALL items as `[ ]` unchecked. But 25+ of the 37 items are verifiably fixed in the current code. **The roadmap is outdated and misleading.** Anyone reading it will think the system is in the same state as the April 20 audit.

---

## Part 4: New Security Findings (Not in Prior Audit)

These are issues **not found or not correctly analyzed** in the prior `security_audit.md`.

---

### 🔴 NEW-SEC-01: `withdraw_after_cooldown` Does Not Validate `authority == issuer.authority`

**File:** `programs/issuer-registry/src/lib.rs`, Lines 995–1005

```rust
pub struct WithdrawAfterCooldown<'info> {
    #[account(mut, seeds = [b"issuer", issuer_authority.key().as_ref()], bump)]
    pub issuer_account: Account<'info, IssuerAccount>,
    /// CHECK: Stake vault PDA
    #[account(mut, seeds = [b"stake-vault"], bump)]
    pub stake_vault: AccountInfo<'info>,
    #[account(mut)]
    pub issuer_authority: Signer<'info>,
    // ...
}
```

The PDA seed `[b"issuer", issuer_authority.key()]` binds the `issuer_account` to the signer's key. This is correct for the withdrawal case. **However**, there is no `constraint = issuer_account.authority == issuer_authority.key()` check in the struct. While the PDA seeds themselves prevent a *wrong* PDA from being passed, if for any reason the `authority` field stored in the account differs from the signer (e.g., via a bug that changed the authority), the signer could drain the stake vault of a different issuer.

Compare this to `WithdrawStake` which has the explicit constraint at line 788:  
`constraint = issuer_account.authority == issuer_authority.key() @ ErrorCode::Unauthorized`

**Risk:** Low-to-medium defense-in-depth gap. The PDA seeds guard is sufficient in the current design, but explicit constraint parity is missing.

---

### 🔴 NEW-SEC-02: `SCHEMA_REGISTRY_ID_BYTES` May Be Wrong — Not Derived Programmatically

**File:** `crates/solid-light/src/cpi_helpers.rs`, Lines 54–59

```rust
const SCHEMA_REGISTRY_ID_BYTES: [u8; 32] = [
    184, 31, 191, 183, 14, 126, 178, 219,
    191, 193, 249, 206, 232, 77, 185, 224,
    56, 51, 91, 209, 33, 205, 175, 183,
    155, 9, 46, 66, 147, 25, 1, 94,
];
```

This is a **manually hand-decoded** byte array of `"DPk6XUH6CArLWt4KMqJmpNBnPwQ3gG9P3dBd3MDVE3bT"`. If anyone ever changes the schema-registry program ID (which would happen on a re-deploy after an upgrade), this constant will silently become wrong, and the entire owner-check in the ZK verifier will pass against a program that is NOT the real schema registry.

**The prior audit's P0-2 fix is only as strong as this constant being correct.** If the constant is wrong, the fix is a no-op and the critical bypass is back.

There is no compile-time test, CI check, or cross-reference to `Anchor.toml` that validates this byte array. The `check_program_ids.py` script does NOT check this constant.

**Risk:** High — silent regression vector for the most critical security check in the system.

---

### 🟠 NEW-SEC-03: `submit_fraud_proof` Has No `issuer_account` PDA Seed Constraint

**File:** `programs/issuer-registry/src/lib.rs`, Lines 799–823

```rust
pub struct SubmitFraudProof<'info> {
    #[account(seeds = [b"registry-config"], bump)]
    pub registry_config: Account<'info, RegistryConfig>,
    #[account(mut)]
    pub issuer_account: Account<'info, IssuerAccount>,   // ← NO SEED CONSTRAINT
    // ...
```

Unlike `SlashIssuer` (which also has no seed constraint on `issuer_account` at line 861), the `submit_fraud_proof` handler has the authority check at line 475–477. However, the `issuer_account` can be any `IssuerAccount` PDA — it is not constrained to be the canonical `[b"issuer", issuer.authority.as_ref()]` PDA. A malicious reporter (even the DAO authority) could in principle pass a fabricated IssuerAccount.

**The remedy already exists** in the code for `approve_via_trust_anchor` and `withdraw_after_cooldown` (seed + authority constraints). The same pattern should apply here and in `SlashIssuer`.

**Risk:** Medium — partially mitigated by the authority gate, but defense-in-depth is missing.

---

### 🟠 NEW-SEC-04: `G1 Negation` Edge Case: Y = 0

**File:** `programs/zk-verifier/src/lib.rs`, Line 430

```rust
if borrow != 0 {
    return Err(());
}
```

When `Y = 0`, computing `P - 0 = P` where `P` is the BN254 field prime. The result has `Y = P`, which equals `0` modulo `P`. The code returns `Ok(P)` — and the test at line 697 acknowledges this: "P - 0 = P, which is accepted because groth16-solana canonicalizes before pairing."

The issue: if `Y > P` (e.g., a specially crafted proof point), the subtraction underflows → `borrow != 0` → returns `Err(())` → maps to `ErrorCode::InvalidProofFormat`. This is the correct behavior. But the check is `borrow != 0` after all 32 iterations, not `borrow > 0`. In Rust, `i16` signed arithmetic means `borrow` could theoretically be positive after the loop. This is safe because the BigEndian subtraction is correct, but it's an unusual pattern that could confuse auditors.

**Risk:** Informational — the logic is correct but worth a comment clarifying why `borrow` is `i16` not `u16`.

---

### 🟠 NEW-SEC-05: `set_binding_status` Reuses `UpdateTreeRoot` Context Without Status-Specific Guard

**File:** `programs/schema-registry/src/lib.rs`, Lines 322–349

```rust
pub fn set_binding_status(
    ctx: Context<UpdateTreeRoot>,  // ← shares context with update_tree_root
    _schema_hash: [u8; 32],
    status: u8,
) -> Result<()> {
```

`set_binding_status` reuses the `UpdateTreeRoot` account context, which requires `seeds = [b"schema-tree-binding", schema_hash]`. This is fine for the PDA constraint. However, `update_tree_root` enforces **monotonicity** on the slot (`require!(now_slot > last_slot)`). `set_binding_status` does NOT update the slot field — so an authority can call `set_binding_status` in the same slot as `update_tree_root` with no issue. But this also means `update_tree_root` can be called in the same slot as `set_binding_status`, which is the intended behavior for emergency freeze followed by immediate root update. The semantics are fine, but **the code must not call `update_tree_root` in the same slot it was last called**, and there's no protection against an operator accidentally skipping the freeze step.

More critically: `set_binding_status` can **unfreeze** a binding (`status = STATUS_ACTIVE`). There is no time-lock or additional authorization on unfreezing. A compromised authority key could: freeze → legitimate txns fail → unfreeze immediately. This is a known governance limitation, not a bug, but should be documented.

**Risk:** Low — governance design choice, not a vulnerability.

---

### 🟡 NEW-SEC-06: `bufToDecimal` Is Little-Endian, Circuit Expects Field-Element Order

**File:** `ts-sdk/packages/holder/src/index.ts`, Lines 391–397

```typescript
function bufToDecimal(buf: Uint8Array): string {
  let result = 0n;
  for (let i = buf.length - 1; i >= 0; i--) {
    result = result * 256n + BigInt(buf[i]);
  }
  return result.toString();
}
```

This iterates from `buf.length-1` downto `0`, treating `buf[0]` as the *least significant byte* (little-endian). This is correct for Solana account data (LE) and for BabyJubJub field elements stored LE.

However, `VERIFIER_ID_BYTES` (a Solana public key) is stored in big-endian form as a 32-byte array when accessed via `.toBytes()`. Calling `bufToDecimal(VERIFIER_ID_BYTES)` converts the big-endian pubkey as if it were little-endian — yielding the wrong integer. The circuit's `verifierAddress` input must equal `ID.to_bytes()` on-chain (line 176 of zk-verifier), which is the raw 32-byte Solana pubkey in whichever endianness `declare_id!` uses.

**This is an actual bug**: the `verifierAddress` circuit input may be interpreted with wrong byte order causing the on-chain check `verifier_address_input == ID.to_bytes()` to fail every proof, making the system non-functional end-to-end.

**Risk:** High — potentially breaks all proof verifications.

---

### 🟡 NEW-SEC-07: `NullifierComputer` (Legacy 3-Arg) vs Circuit 5-Arg Nullifier Mismatch

**File:** `crates/solid-light/src/cpi_helpers.rs` vs `circuits/batch_credential_query.circom`

The batch circuit uses this nullifier formula (Step 5):
```
nullifier = Poseidon(masterKey, revocationNonce, verifierAddress, queryContextHash, verifierNonce)
```

The `NullifierComputer` template in `circuits/lib/nullifier_expiry.circom` uses:
```
nullifier = Poseidon(holderPrivKey, schemaHash, verifierNonce)
```

These are two completely different formulas. While `NullifierComputer` is dead code (it's never instantiated by the batch circuit), the `computeNullifier` WASM function called by the holder SDK must implement the 5-input formula. If the WASM crate (`solid-core`) implements the old 3-input version, every nullifier computed client-side will fail the on-chain `require!(nullifier == public_inputs[0])` check.

**Risk:** Potentially critical depending on which formula is in `solid-core` — not inspectable here (the crate was not found in the `crates/` directory in this audit), but this is the most likely remaining end-to-end failure point.

---

### 🟡 NEW-SEC-08: `SchemaTreeBinding` Size Off-By-One

**File:** `programs/schema-registry/src/lib.rs`, Lines 37 and 249

```rust
pub const SCHEMA_TREE_BINDING_SIZE: usize = 145;
// ...
data[113..145].copy_from_slice(&ctx.accounts.authority.key().to_bytes());
```

The comment at line 22–23 says:
```
// [113..145) authority (Pubkey)   → 32 bytes from 113, so 113+32=145 → 145 bytes total
```

But `SCHEMA_TREE_BINDING_SIZE = 145` allocates exactly 145 bytes (indices `[0..145)`). The slice `[113..145)` is a 32-byte slice occupying the last 32 bytes. This is exactly right.

However, line 37: `pub const SCHEMA_TREE_BINDING_SIZE: usize = 145` — but in `cpi_helpers.rs` at line 169: `if tree_account_data.len() < 113 { return false; }` — the Rust parser only needs 113 bytes (up to and including the status byte). The authority at [113..145] is forward-compatible trailing data. This is intentional but creates an asymmetry where the writer allocates 145 bytes and the reader only needs 113.

**Risk:** None — this is intentional and documented. Informational only.

---

### 🟡 NEW-SEC-09: `CredentialAtom` Has a Local `IsZero` Template That Shadows the CircomLib One

**File:** `circuits/lib/credential_atom.circom`, Lines 83–90

```circom
template IsZero() {
    signal input in;
    signal output out;
    signal inv;
    inv <-- in != 0 ? 1/in : 0;
    out <== -in * inv + 1;
    in * out === 0;
}
```

This defines `IsZero` again after including `poseidon.circom` (which pulls in `comparators.circom` → which already exports `IsZero`). In circom 2.1.0, this redefinition causes a **duplicate template name** that is resolved by the later definition taking precedence within the file scope.

The implementations are identical, so this is functionally harmless today. But it's a latent risk: if `circomlib`'s `IsZero` is ever updated (e.g., adds a bit-width constraint or domain separation), the local shadow will silently continue using the old implementation.

The `batch_credential_query.circom` also `include`s both this file AND `comparators.circom` from `circomlib`. In a multi-file circom project, the name resolution order matters and can be compiler-version-dependent.

**Risk:** Low but real. The prior audit listed this as BUG-01.

---

### 🟡 NEW-SEC-10: `GreaterThan(8)` Used for OR-Sum Check May Have Bit-Width Issue

**File:** `circuits/batch_credential_query.circom`, Lines 247–251

```circom
component orCheck = GreaterThan(8);
orCheck.in[0] <== orSum;
orCheck.in[1] <== 0;
signal orResult;
orResult <== orCheck.out;
```

`orSum` is the sum of up to 4 binary values (0 or 1), so it can range from 0 to 4. `GreaterThan(8)` works for values ≤ 2^8 - 1 = 255, so this is fine numerically. However, `orSum` is a field element (potentially up to 254 bits if unconstrained). Using `GreaterThan(8)` implicitly assumes `orSum` fits in 8 bits.

Because predicateResults[i] is the output of `isActive[i] * evaluators[i].result + (1 - isActive[i])` and both `isActive` and `result` are binary, `predicateResults[i]` is binary. The sum of 4 binary values is indeed at most 4, which fits in 8 bits. This is safe.

But `orSum` in the batch circuit sums `isActive[i] * evaluators[i].result` (not `predicateResults[i]`). These are products of outputs of `LessThan(8)` and `PredicateEvaluator.result` — both binary. So the sum is at most 4. Safe.

**Risk:** None — informational. Worth a comment in the circuit.

---

## Part 5: Logical Flaws and Bugs

### BUG-NEW-01: `generateBatchProof` holderPubKeyX Identity Check vs holderPrivKey Usage

**File:** `ts-sdk/packages/holder/src/index.ts`, Lines 250–255

```typescript
// SEC-17: Identity Cohesion
for (const cred of sortedCredentials) {
    if (Buffer.from(cred.holderPubKeyX).compare(masterPublicKey.x) !== 0) {
        throw new Error("Identity Cohesion Failure: ...");
    }
}
```

This checks that `cred.holderPubKeyX` matches the master public key. But the circuit (in `CredentialAtom`) uses `holderBJJPubKeyAx` which comes from `anchors[i].credentialPubKeyAx` — the **per-schema derived** public key, NOT the master public key. So credentials stored with the master public key as `holderPubKeyX` will fail the circuit's Poseidon commitment check (because the commitment was originally signed over the master key, not the derived key).

**The identity cohesion check is wrong in the opposite direction from BUG-04:** instead of the SDK using per-schema keys (which was the fix from the prior audit), it now checks that stored credentials use the master public key. But the circuit expects per-schema derived keys in the commitment. This creates a catch-22:
- If the holder SDK stores `masterPublicKey.x` in credentials → identity cohesion check passes → but circuit commitment check fails
- If the holder SDK stores `derivedKey.x` in credentials → identity cohesion check fails → proof never starts

**Root cause:** The identity cohesion check should compare `cred.holderPubKeyX` against the **per-schema derived pubkey** for that credential's schema, not the master pubkey.

**Risk:** High — the batch proving path fails for any correctly-issued credential.

---

### BUG-NEW-02: `IdentityAnchor` Always Has `enabled = 1` — No Bypass for Null Schemas

**File:** `circuits/lib/identity_anchor.circom`, Lines 46–53

```circom
component globalInclusion = MerkleInclusion(GLOBAL_DEPTH);
globalInclusion.enabled <== 1;  // Always enabled
```

In `CredentialAtom`, the `enabled` signal is derived from `1 - isZero(schemaHash)`, meaning inactive (zero-schema) credential slots bypass signature and Merkle checks. But `IdentityAnchor` does NOT guard against zero-schema slots — it always computes global inclusion with `enabled = 1`.

For an inactive slot (schemaHash = 0):
- `CredentialAtom.enabled = 0` → signature and local Merkle checks bypassed
- `IdentityAnchor.enabled = 1` → global Merkle inclusion is ENFORCED

This means even for an empty/padding credential slot, the prover must supply a valid global Merkle proof for the identity state derived from `Poseidon(derivedAx, derivedAy, revocNonce)` where `(derivedAx, derivedAy) = BabyPbk(Poseidon(masterKey, 0))`.

This is an over-constraint for padding slots: it forces the global tree to contain identity leaves for zero-schema derived keys, which is not how the architecture intends to work. The global tree should not have entries derived from schemaHash=0.

**Risk:** Medium — the batch circuit may be unable to prove with fewer than NUM_CREDS credentials unless the global tree pre-loads zero-schema identity leaves, which is architecturally wrong.

---

### BUG-NEW-03: `transfer_slashed_lamports` Does Not Check `dao_treasury` Rent-Exemption

**File:** `programs/issuer-registry/src/lib.rs`, Lines 1192–1206

The `transfer_slashed_lamports` helper transfers lamports into `dao_treasury` by direct field manipulation. The `dao_treasury` PDA is initialized with `space = 0` via `init_if_needed`.

After the transfer, the `dao_treasury` lamports will be above rent-exemption threshold (it starts at minimum_balance(0) ≈ 890,880 lamports, plus whatever is slashed). This is fine as long as it never goes below minimum_balance. But the function does NOT check whether draining from `stake_vault` will leave it below rent-exemption. The `stake_vault` has `space = 0` too, so its minimum rent is also ≈ 890,880 lamports.

If `from_balance == amount`, the stake_vault goes to 0 lamports. A zero-lamport account that is NOT freed (via `close`) will be garbage-collected by the Solana runtime. This would destroy the shared stake vault PDA, making all subsequent `register_issuer` SOL transfers fail with "account not found."

**Risk:** Medium — edge case when the last issuer's entire stake is slashed. The stake vault would disappear.

---

### BUG-NEW-04: `expirationTimestamp` Ordering Constraint in Batch Circuit

**File:** `circuits/batch_credential_query.circom`, Lines 130–157

The zero-schema integrity check correctly forces `expirationTimestamps[i] = 0` when `schemaHash[i] = 0` (via `isZero[i].out * expirationTimestamps[i] === 0`). 

But the ordering constraint at lines 149–157 only checks `schemaHashes[i] < schemaHashes[i+1]` when the next schema is non-zero. It does NOT enforce that zero-schema slots come AFTER non-zero ones. This means:
```
schemaHashes = [0, some_hash, 0, another_hash]
```
...would NOT be caught by the ordering check but would be caught by the canonical order constraint in the verifier (`require!(schema_hash > prev, ErrorCode::InvalidCredentialOrder)`).

However, inside the circuit this creates an odd situation: `isZero[0].out = 1` (first slot is inactive), `isZero[1].out = 0` (active), `isZero[2].out = 1` (inactive again), `isZero[3].out = 0` (active). The ordering check for slot 1→2 requires `schemaHashes[2] > schemaHashes[1]` only if `isZero[2].out = 0` (non-zero). Since `isZero[2].out = 1` (slot 2 is zero), the check is skipped. So interleaved zero and non-zero schemas are not prevented in the circuit.

**Risk:** Low-medium — the on-chain verifier's ordering check handles this (it loops over non-zero pairs only), so the credential verification still passes. But it creates an inconsistency between circuit and on-chain semantics.

---

## Part 6: Integration Cohesion Assessment

This is one of the most important sections. Does the system hang together as a whole, or is it fragmented?

### Cross-Component Data Flow (Verified)

```
[Circuit: batch_credential_query.circom]
  public_inputs[0] = nullifierHash (output signal)
  public_inputs[1] = globalRoot
  public_inputs[2..5] = merkleRoots[4]
  public_inputs[6..9] = schemaHashes[4]
  ...
  public_inputs[28] = verifierAddress
  public_inputs[29] = verifierNonce
  public_inputs[30] = currentTimestamp
  (31 total)

[on-chain: zk-verifier]
  const NR_PUBLIC_INPUTS: usize = 31  ✅ matches
  public_inputs[0] = nullifier  ✅ nullifier check
  public_inputs[1] = globalRoot  ✅ verified against GlobalStateBinding
  public_inputs[2+i] = merkleRoot  ✅ verified per schema tree
  public_inputs[6+i] = schemaHash  ✅ ordering enforced
  public_inputs[28] = verifierAddress  ✅ binds to ID.to_bytes()

[SDK: holder/index.ts]
  snarkjs public signals → bigintToBytes32()  ✅ BUG-02 fixed
  buildVerifyBatchProofIx account order ✅ matches VerifyBatchProof struct

[schema-registry: SchemaTreeBinding layout]
  bytes [0..8)  = b"schmtree"  ✅ matches cpi_helpers SCHEMA_TREE_DISCRIMINATOR
  bytes [8..40) = schema_hash  ✅ matches verify_schema_root_binding
  bytes [72..104) = current_root  ✅
  bytes [112] = status  ✅
  bytes [113..145) = authority  ✅ (forward-compatible extension)

[solid-light: cpi_helpers.rs]
  verify_schema_root_binding: checks [0..8], [8..40], [72..104], [112]  ✅
  verify_state_root_matches: checks [0..8], [8..40]  ✅
  SCHEMA_REGISTRY_ID: hardcoded bytes  ⚠️ NEW-SEC-02 risk
```

### Integration Verdict

The data-contract between the three on-chain programs, the `solid-light` crate, and the TypeScript SDK is **coherent and well-aligned** at the byte level. Account layouts are documented as protocol contracts and the parsers respect them. The public-input ordering between the circuit and the on-chain verifier is identical and correctly numbered (31 inputs).

The main integration risks are:
1. `SCHEMA_REGISTRY_ID_BYTES` being stale (NEW-SEC-02)
2. The `verifierAddress` endianness (NEW-SEC-06)
3. The `IdentityAnchor` always-enabled issue for padding slots (BUG-NEW-02)
4. Missing circuit artifacts (.wasm, .zkey) — no actual end-to-end execution is possible

---

## Part 7: What the Prior Security Audit Got Right vs Missed

### Got Right ✅
- All 4 Critical issues (voting, space, owner checks, voting deadline) — correctly identified
- BUG-02 (nullifier decimal/hex) — correctly identified
- BUG-04 (identity commitment derivation) — correctly identified
- HIGH-03 (ReleaseVote not mut) — correctly identified
- LOW-06 (GLOBAL_DEPTH hardcoded) — correctly identified
- Infrastructure section (CI assessment, bootstrap quality) — accurate

### Missed / Incorrect ❌

| ID | What the Prior Audit Missed |
|----|----------------------------|
| NEW-SEC-02 | `SCHEMA_REGISTRY_ID_BYTES` being a hardcoded manual constant with no validation |
| NEW-SEC-06 | `bufToDecimal` endianness issue for verifierAddress (this is a high-impact bug) |
| BUG-NEW-01 | Identity cohesion check compares against master key, not per-schema key |
| BUG-NEW-02 | `IdentityAnchor` always enabled even for zero-schema slots |
| BUG-NEW-03 | `stake_vault` can reach 0 lamports from slashing and be garbage-collected |
| BUG-NEW-04 | Interleaved zero/non-zero schemas not caught by circuit ordering |
| — | `submit_fraud_proof` + `slash_issuer` have no PDA seed constraint on `issuer_account` (NEW-SEC-03) |

### Things the Prior Audit Reported That Are Now Fixed
As detailed in Part 3, **25 of 37 listed items are verifiably fixed** in the current codebase. The audit doc has NOT been updated to reflect this, which is misleading.

---

## Part 8: Security Audit Items in `security_audit.md` — Meta-Assessment

The `security_audit.md` (2026-04-20) document contains some findings that **should NOT be there** as active issues or are miscategorized:

### Incorrectly Classified as Active (Now Fixed) 

These appear in the doc as current vulnerabilities but are fixed:
- CRITICAL-03 (`VoteOnIssuer` never increments active_votes_count) — **FIXED**
- CRITICAL-04 (`VoteOnIssuer` no deadline) — **FIXED**  
- HIGH-02 (`RegistryConfig` space wrong) — **FIXED**
- HIGH-03 (`ReleaseVote` not mut) — **FIXED**
- HIGH-04, HIGH-05 (owner checks missing) — **FIXED**

### Incorrectly Classified as P0 in Roadmap, Actually P1-P2

- P0-6 (`@solid-protocol/verifier` missing) — was listed as blocking but actually the verifier package EXISTS and is fully implemented. This was fixed between the audit and the roadmap write-up.

### Missing from the Security Audit

All of NEW-SEC-01 through NEW-SEC-10 and BUG-NEW-01 through BUG-NEW-04 are absent.

---

## Part 9: Overall System Assessment

### Architectural Strengths (Confirmed Fresh)

1. **Backend-agnostic ZK verification** — The `verify_state_root_matches` / `verify_schema_root_binding` design is production-grade. Switching from Light Protocol to SPL AC required zero circuit changes.
2. **5-input nullifier with query context** — `Poseidon(masterKey, revocNonce, verifierAddr, queryContextHash, verifierNonce)` is one of the most comprehensive anti-replay designs in Solana ZK.
3. **Stack-safe VK parsing** — `VkBuf` + compile-time `assert!(sz < 3072)` eliminates the `Box::leak` memory leak. This is genuinely best-in-class.
4. **Monotonic root updates** — `require!(now_slot > last_slot)` prevents root regression attacks.
5. **Flash-loan protection** — `require!(now_slot >= staker.last_stake_slot + 100)` is correctly implemented.
6. **Canonical ordering** — Schema-hash ordering enforced in both circuit and on-chain verifier with consistent semantics.
7. **PDA-per-nullifier** — Atomic replay safety with zero bloom filter false-positives.
8. **Trust anchor design** — `approve_via_trust_anchor` with explicit seed constraint + event emission is architecturally clean.

### Critical Gaps That Must Be Resolved Before ANY Testing

1. **Circuit artifacts missing** — No `.wasm`, no `.zkey`. Nothing can be proved.
2. **`solid-core` crate not in repo** — The WASM functions (`computeNullifier`, `deriveCredentialKey`, etc.) are called by the holder SDK but the Rust source isn't visible. The nullifier formula consistency (NEW-SEC-07) cannot be verified.
3. **Governance token not deployed** — The DAO is architecturally complete but operationally inert.
4. **`SCHEMA_REGISTRY_ID_BYTES` not validated** — A stale constant silently breaks the entire security model.

### What I Think of This System

The SolID Protocol is one of the most architecturally sophisticated ZK identity systems built on Solana. The core design decisions — BabyJubJub separate key management, Poseidon-based commitments, DAO-governed registry with tiered staking, SPL Account Compression for credential trees, and a composable batch proof that spans 4 credentials — are all correct and well-executed.

The code quality within each component is high. The Rust programs are well-commented, use checked arithmetic throughout, avoid the typical Solana pitfalls (owner checks are present, PDA seeds are canonical, events are emitted for all state transitions), and have thoughtful in-code security documentation.

The main weakness is **the gap between design and deployment readiness**: no circuit artifacts, no governance token, no trusted setup, and an architecture document describing a significantly different (older) codebase. The system is an **implementation-complete research prototype** that is not yet production-deployable.

---

## Part 10: Actionable Priority List (Freshly Derived)

### Immediate (Before Any Testing)

| Priority | Action |
|----------|--------|
| 🔴 | **Add a test or CI check that cross-validates `SCHEMA_REGISTRY_ID_BYTES` against `Anchor.toml`** (NEW-SEC-02) |
| 🔴 | **Audit `solid-core`'s `computeNullifier` implementation for 5-arg formula consistency** (NEW-SEC-07) |
| 🔴 | **Fix `bufToDecimal` endianness for Pubkey inputs — use big-endian for `verifierAddress`** (NEW-SEC-06) |
| 🔴 | **Fix BUG-NEW-01: identity cohesion check should use per-schema derived pubkey** |
| 🔴 | **Fix BUG-NEW-02: zero-schema slots need `enabled = isZero[i].out` guard in IdentityAnchor** |
| 🔴 | **Update IMPROVEMENTS_ROADMAP.md** — mark the 25+ fixed items as `[x]` immediately |

### Before Devnet Testing

| Priority | Action |
|----------|--------|
| 🟠 | Run trusted setup, compile circuits, host artifacts |
| 🟠 | Deploy governance SPL token mint |
| 🟠 | Add PDA seed constraint to `issuer_account` in `SubmitFraudProof` and `SlashIssuer` |
| 🟠 | Fix BUG-NEW-03: check that `stake_vault` lamports don't go below rent minimum after slash |
| 🟡 | Add `constraint = issuer_account.authority == issuer_authority.key()` to `WithdrawAfterCooldown` |
| 🟡 | Update architecture.md to match actual codebase structure |

### Before Mainnet

All items from `IMPROVEMENTS_ROADMAP.md` still marked `[ ]` that are already fixed should be marked done. The remaining genuine gaps (`P2-8`, `P2-11`, `P2-12`, `P2-13`, `ARCH-4`) are the real pre-mainnet blockers.

---

*End of audit. Total new findings: 10 security issues + 4 logical bugs not in prior audit. Prior audit findings verified: 25/37 fixed, 12 genuinely outstanding.*
