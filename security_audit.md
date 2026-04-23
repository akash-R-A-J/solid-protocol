# SolID Protocol — Comprehensive Ground-Level Security Audit

> **HISTORICAL — DO NOT USE AS CURRENT STATE.** Frozen 2026-04-20
> snapshot.  Many findings here have been closed by Phase 1 + Phase 2
> work.  The living tracker is `sec/SECURITY_REGISTRY.md`; formal
> audit snapshots live under `sec/audits/`.  Kept at the repo root
> for historical reference + git-blame continuity.

> **Audit Date:** 2026-04-20  
> **Auditor:** Antigravity Deep Audit Engine  
> **Scope:** Every source file in the repository — on-chain programs, ZK circuits, TypeScript SDK, Rust crates, CI/CD, deployment manifests, and scripts.  
> **Methodology:** Manual line-by-line code review + architectural analysis + cross-component integrity verification.

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Security Vulnerabilities](#2-security-vulnerabilities)
3. [Logical Flaws & Bugs](#3-logical-flaws--bugs)
4. [Circuit Security Analysis](#4-circuit-security-analysis)
5. [SDK Security Analysis](#5-sdk-security-analysis)
6. [Infrastructure & CI Security](#6-infrastructure--ci-security)
7. [Comprehensive System Assessment (Pros & Cons)](#7-comprehensive-system-assessment)
8. [Complete Deployment Guide](#8-complete-deployment-guide)

---

## 1. Executive Summary

The SolID Protocol is a **Zero-Knowledge Identity Infrastructure** on Solana that enables privacy-preserving credential verification using Groth16 proofs over BN254, with a DAO-governed issuer registry and SPL Account Compression-based credential storage. The system has undergone multiple rounds of hardening (the 2026-04 remediation series) and shows significant architectural maturity.

### Verdict

| Severity | Count | Status |
|----------|-------|--------|
| 🔴 **Critical** | 4 | Active — must fix before mainnet |
| 🟠 **High** | 5 | Active — should fix before mainnet |
| 🟡 **Medium** | 5 | Acceptable for devnet, fix for mainnet |
| 🔵 **Low / Info** | 6 | Informational |
| ✅ **Passed Checks** | 22 | Core cryptographic paths verified |

---

## 2. Security Vulnerabilities

### 🔴 CRITICAL-01: `devnet.json` Program IDs Diverge from `declare_id!` / `Anchor.toml`

**Location:** [devnet.json](file:///c:/Users/KIIT/Desktop/solid-protocol/deployments/devnet.json) vs [Anchor.toml](file:///c:/Users/KIIT/Desktop/solid-protocol/Anchor.toml)

**Evidence:**

| Program | `Anchor.toml` / `declare_id!` | `devnet.json` |
|---------|-------------------------------|---------------|
| `schema_registry` | `DPk6XUH6CArLWt4KMqJmpNBnPwQ3gG9P3dBd3MDVE3bT` | `2oma2yU2vfSYNR8sPDq5JvRbrtetyk5wuuGzwGYupAsH` |
| `zk_verifier` | `BZkVFdMhAEeGMvEAhXNjt3r3bEA2sCPqEFcsEbSbFGj2` | `FhtEvsUwxRRT8nf3uaxK6iAefFqvqCYimc9vqnH5XEgr` |
| `issuer_registry` | `CRGYfonXwDk6gKEm9fC1U33VVBkqnQVD3sPdLKzqHWoR` | `6ewriDVJLTaeBG7AsjuobAfyhDWfCz681RDWMFq5Laeo` |

**Impact:** The `devnet.json` manifest records the *old* deployed program IDs from a prior deployment. The code (`declare_id!`, `Anchor.toml`, `config.ts`) uses *different* IDs. This means:
- Any tooling reading `devnet.json` will point to stale/wrong programs.
- The `check_program_ids.py` CI script catches `Anchor.toml ↔ declare_id!` drift but does NOT currently reconcile `devnet.json`.
- The `devnet.json` also references `nullifier-bloom` PDA seeds — the code has since switched to PDA-per-nullifier. This is a ghost reference.

**Risk:** **Deployment failure / proof submission to wrong programs.**

---

### 🔴 CRITICAL-02: `IncrementUsage` Has No Access Control

**Location:** [schema-registry/src/lib.rs:371-375](file:///c:/Users/KIIT/Desktop/solid-protocol/programs/schema-registry/src/lib.rs#L371-L375)

```rust
#[derive(Accounts)]
pub struct IncrementUsage<'info> {
    #[account(mut)]
    pub schema_account: Account<'info, SchemaAccount>,
}
```

**Impact:** Anyone can call `increment_usage` and inflate a schema's `usage_count` to `u64::MAX`. While `usage_count` is currently cosmetic, it could be used by downstream analytics, governance weight, or fee calculations. An attacker can:
1. Spam `usage_count` to max out the counter.
2. Cause the `Overflow` error to DOS legitimate CPI callers.

**Risk:** **Denial-of-service on schemas + data integrity corruption.**

---

### 🔴 CRITICAL-03: `VoteOnIssuer` Does Not Increment `active_votes_count`

**Location:** [issuer-registry/src/lib.rs:144-178](file:///c:/Users/KIIT/Desktop/solid-protocol/programs/issuer-registry/src/lib.rs#L144-L178)

The `vote_on_issuer` instruction creates a `VoteRecord` and adds weight to `votes_for`/`votes_against`, but it **never increments** `staker_account.active_votes_count`. The `unstake_tokens` instruction checks `active_votes_count == 0` before allowing unstake:

```rust
require!(staker_account.active_votes_count == 0, ErrorCode::ActiveVotesExist);
```

Since `active_votes_count` is never incremented during voting, a staker can:
1. Stake tokens → gain 100-slot matured weight.
2. Vote on an issuer.
3. **Immediately unstake** all tokens (because `active_votes_count` is still 0).
4. The vote weight stays committed, but the economic skin-in-the-game is gone.

This defeats the entire DAO governance model — voters have zero cost to vote maliciously.

**Risk:** **Complete governance bypass — costless voting.**

---

### 🔴 CRITICAL-04: `VoteOnIssuer` Does Not Verify Voting Period

**Location:** [issuer-registry/src/lib.rs:144-178](file:///c:/Users/KIIT/Desktop/solid-protocol/programs/issuer-registry/src/lib.rs#L144-L178)

The `vote_on_issuer` instruction does not check whether `Clock::get()?.unix_timestamp < issuer.voting_ends_at`. While `finalize_voting` correctly requires the period to have ended, votes can be cast **after** the voting period has expired but before anyone calls `finalize_voting`. This means:
- An attacker can monitor the vote tally, wait until the period ends, then stuff votes before finalization.

**Risk:** **Late-vote stuffing bypasses the governance time-lock.**

---

### 🟠 HIGH-01: `slash_issuer` Does Not Transfer Slashed Lamports

**Location:** [issuer-registry/src/lib.rs:355-388](file:///c:/Users/KIIT/Desktop/solid-protocol/programs/issuer-registry/src/lib.rs#L355-L388)

The instruction decrements `issuer.staked_amount` but does not actually transfer the lamports from `stake_vault` to a treasury/burn address. The lamports remain in the `stake_vault` PDA but are "orphaned" — they can never be withdrawn because the issuer's `staked_amount` has been reduced.

```rust
issuer.staked_amount -= slash_amount;  // ← accounting only, no actual transfer
```

Similarly, `submit_fraud_proof` has the same issue — it does `saturating_sub` on the accounting but moves no lamports.

**Risk:** **Permanent lock of slashed funds in the vault.**

---

### 🟠 HIGH-02: `RegistryConfig` Space Calculation Is Wrong

**Location:** [issuer-registry/src/lib.rs:611-621](file:///c:/Users/KIIT/Desktop/solid-protocol/programs/issuer-registry/src/lib.rs#L611-L621)

```rust
space = 8 + 32 + 8 + 8 + 8 + 8 + 8,  // = 80 bytes
```

But `RegistryConfig` has these fields:
- `authority: Pubkey` (32)
- `governance_token_mint: Pubkey` (32)  ← **MISSING from space calc!**
- `min_stake_lamports: u64` (8)
- `voting_period_seconds: i64` (8)
- `approval_threshold_bps: u64` (8)
- `total_issuers: u64` (8)
- `active_issuers: u64` (8)

Required space: `8 + 32 + 32 + 8 + 8 + 8 + 8 + 8` = **112 bytes**

The `governance_token_mint` Pubkey (32 bytes) is not accounted for. This causes the PDA to be allocated with 32 bytes too few, meaning `governance_token_mint` will overlap with subsequent fields, corrupting all downstream reads.

**Risk:** **Account data corruption — registry becomes uninitialized/bricked.**

---

### 🟠 HIGH-03: `ReleaseVote` Account is Not Mutable

**Location:** [issuer-registry/src/lib.rs:851-868](file:///c:/Users/KIIT/Desktop/solid-protocol/programs/issuer-registry/src/lib.rs#L851-L868)

```rust
pub struct ReleaseVote<'info> {
    #[account(seeds = [b"staker", voter.key().as_ref()], bump)]
    pub staker_account: Account<'info, StakerAccount>,  // ← NOT mut
    // ...
    pub vote_record: Account<'info, VoteRecord>,         // ← NOT mut
```

But the handler modifies both:
```rust
staker_account.active_votes_count -= 1;
vote_record.released = true;
```

The Anchor framework will silently skip the serialization of unchanged accounts back to the ledger. Since neither account is marked `mut`, the changes are **discarded** — `release_vote` becomes a no-op.

**Risk:** **Votes can never be released, tokens permanently locked.**

---

### 🟠 HIGH-04: `global_tree` Account Ownership Not Verified in ZK Verifier

**Location:** [zk-verifier/src/lib.rs:459-460](file:///c:/Users/KIIT/Desktop/solid-protocol/programs/zk-verifier/src/lib.rs#L459-L460)

```rust
/// CHECK: Global-state Merkle tree account (Light Protocol or SolID-native).
pub global_tree: UncheckedAccount<'info>,
```

Neither the account struct nor the handler checks that `global_tree.owner == schema_registry_program_id`. An attacker can pass a **fake account** with crafted data that contains the right `globroot` discriminator and root bytes:
1. Create a system-program-owned account with `globroot` prefix + forged root.
2. Submit a proof against the forged root.

The `verify_state_root_matches` function checks the *data bytes* but not the *owner*. This bypasses the entire global root trust anchor.

**Risk:** **Forged global state root → accept proofs for non-existent identities.**

---

### 🟠 HIGH-05: `schema_tree_N` Account Ownership Not Verified

**Location:** [zk-verifier/src/lib.rs:462-469](file:///c:/Users/KIIT/Desktop/solid-protocol/programs/zk-verifier/src/lib.rs#L462-L469)

Same pattern as HIGH-04. The four `schema_tree_N` accounts are `UncheckedAccount` with no owner constraint. The `verify_schema_root_binding` function checks discriminator + data but not that the account is owned by the `schema_registry` program.

An attacker can forge a `schmtree`-prefixed account with any root+schema pair:
```text
craft account = schmtree || target_schema || tree_pk || fake_root || slot || status_active
```

**Risk:** **Forged schema root binding → accept proofs with fabricated credential trees.**

---

### 🟡 MEDIUM-01: `approve_via_trust_anchor` Does Not Emit Event

**Location:** [issuer-registry/src/lib.rs:444-462](file:///c:/Users/KIIT/Desktop/solid-protocol/programs/issuer-registry/src/lib.rs#L444-L462)

Unlike `finalize_voting` which emits `IssuerApproved`, the trust-anchor bypass does not emit the event. Downstream indexers that rely on `IssuerApproved` to populate the compressed issuer tree will miss trust-anchor-approved issuers.

**Risk:** **Off-chain indexer desync for trust-anchor approvals.**

---

### 🟡 MEDIUM-02: SDK `prove()` Returns Hardcoded Placeholder

**Location:** [ts-sdk/packages/sdk/src/index.ts:66-78](file:///c:/Users/KIIT/Desktop/solid-protocol/ts-sdk/packages/sdk/src/index.ts#L66-L78)

```typescript
static async prove(...): Promise<any> {
    return {
      proof: 'ZK_PROOF_DATA',
      publicSignals: Array(31).fill('0'),
    };
}
```

The top-level `SolID.prove()` method returns fabricated data. Any dApp using the `@solid-protocol/sdk` package's `prove()` will get garbage.

**Risk:** **Non-functional proof generation via the primary SDK entry point.**

---

### 🟡 MEDIUM-03: SDK `resolveSchema` Uses Incorrect `memcmp` Offset

**Location:** [ts-sdk/packages/sdk/src/index.ts:131-135](file:///c:/Users/KIIT/Desktop/solid-protocol/ts-sdk/packages/sdk/src/index.ts#L131-L135)

```typescript
filters: [
    { memcmp: { offset: 8+32+256+1+256+32, bytes: ... } }
]
```

The hardcoded offset `585` does not match the actual Borsh layout of `SchemaAccount`. The `name` field uses 4-byte length prefix + up to 64 bytes, not 256. The correct offset depends on the actual serialized sizes, but this constant is definitely wrong.

**Risk:** **Schema discovery always returns empty results.**

---

### 🟡 MEDIUM-04: SDK `listIssuers` Filter Is Incorrect

**Location:** [ts-sdk/packages/sdk/src/index.ts:156-160](file:///c:/Users/KIIT/Desktop/solid-protocol/ts-sdk/packages/sdk/src/index.ts#L156-L160)

```typescript
{ memcmp: { offset: 8+32+64+128+32+32+1, bytes: '2' } }
```

This attempts to filter by `IssuerStatus::Approved` (the second enum variant, index 1). But:
1. The base58-encoded byte `'2'` is the raw byte `2`, not `1`. `Approved` is at enum index `1`.
2. The offset calculation doesn't account for Borsh string length prefixes.

**Risk:** **Issuer discovery returns no results or wrong results.**

---

### 🟡 MEDIUM-05: Batch Circuit `activeCheck` Variable Shadows Template Scope

**Location:** [batch_credential_query.circom:177](file:///c:/Users/KIIT/Desktop/solid-protocol/circuits/batch_credential_query.circom#L177)

```circom
component activeCheck = LessThan(8);
```

Inside the `for (var i = 0; i < MAX_PREDICATES; i++)` loop, `activeCheck` is declared as a new component on each iteration. In circom 2.1.0, component declarations inside for-loops instantiate separate R1CS constraints, but the variable `activeCheck` is reassigned each iteration — meaning only the **last** iteration's `activeCheck` is reachable after the loop. While the `isActive[i]` signals capture the correct values before reassignment, this is fragile and has led to bugs in other circom codebases.

**Risk:** **Latent circom scoping issue — currently benign but fragile.**

---

### 🔵 LOW / INFORMATIONAL

| ID | Description |
|----|-------------|
| LOW-01 | `NullifierComputer` template in `nullifier_expiry.circom` is never used (dead code — the batch circuit uses inline Poseidon(5) instead). |
| LOW-02 | `signature_verifier.circom` is listed in `lib/` but not `include`d by any circuit (dead file). |
| LOW-03 | `AUTHORITY_PUBKEY` in `config.ts` is a placeholder: `SoLid111...` — not a valid Pubkey derivation. |
| LOW-04 | `SOLID_TOKEN_MINT` in `config.ts` is a placeholder: `SoLidToken111...` — governance token not deployed. |
| LOW-05 | `devnet.json` references `anchor_cli: "1.0.0"` and `solana_cli: "3.1.12"` which don't match the pinned versions in `bootstrap.sh` (Anchor 0.30.1, Solana 1.18.22). |
| LOW-06 | `batch_credential_query.circom` has `globalSiblings[NUM_CREDS][20]` with hardcoded `20` instead of using `GLOBAL_DEPTH` parameter. |

---

## 3. Logical Flaws & Bugs

### BUG-01: `credential_atom.circom` Redefines `IsZero` — Collision with circomlib

The circuit file defines its own `IsZero` template at line 83, which shadows the `IsZero` from `circomlib/circuits/comparators.circom` (already included). While the local implementation is correct, this creates a name collision. If circomlib is updated, the local version will silently take precedence, potentially causing constraint divergence.

---

### BUG-02: `holder/src/index.ts` — Nullifier Parsing Assumes Hex Encoding

**Location:** [holder/src/index.ts:310](file:///c:/Users/KIIT/Desktop/solid-protocol/ts-sdk/packages/holder/src/index.ts#L310)

```typescript
const nullifier = Uint8Array.from(Buffer.from(publicSignals[0], 'hex'));
```

snarkjs `publicSignals[0]` is a **decimal string** (e.g., `"1234567890..."`), not hex. This will produce a zero-length or garbled nullifier. The correct conversion would be:
```typescript
const nullifier = bigintToBytes32(BigInt(publicSignals[0]));
```

This bug means **every batch proof will register the wrong nullifier on-chain**, either failing verification or (worse) not preventing replay of the actual proof.

---

### BUG-03: `holder/src/index.ts` — `computeQueryContextHash` Diverges from Circuit

**Location:** [holder/src/index.ts:392-407](file:///c:/Users/KIIT/Desktop/solid-protocol/ts-sdk/packages/holder/src/index.ts#L392-L407)

The `computeQueryContextHash` in the holder SDK uses:
```typescript
inputs.push(bytesToBigInt(query.schemaHash));
// then fieldIndex, operator, value for each predicate
// then numPredicates, compoundLogic, expirationTimestamp
```

But the batch circuit (Step 4) uses:
```circom
qHasherIndices = Poseidon(credentialIndices || fieldIndices)   // 8 inputs
qHasherOps     = Poseidon(operators || values)                  // 8 inputs
qHasherFinal   = Poseidon(qHasherIndices, qHasherOps, numPredicates, compoundLogic)  // 4 inputs
```

These are completely different hash structures. The holder SDK's `computeQueryContextHash` is for the **compound_query** circuit, not the **batch_credential_query** circuit. The batch proof path in `generateBatchProof` does not call `computeQueryContextHash` at all — it relies on snarkjs to compute it inside the circuit. However, the `generateProof` (single credential) path does use it, and the hash structure doesn't match either — the circuit's `compound_query.circom` hashes `Poseidon(fieldIndices)` then `Poseidon(operators||values)`, but the SDK puts `schemaHash` as the first input and mixes all together.

---

### BUG-04: `holder/src/index.ts` — `generateBatchProof` Computes `identityCommitment` with Wrong Inputs

**Location:** [holder/src/index.ts:241-245](file:///c:/Users/KIIT/Desktop/solid-protocol/ts-sdk/packages/holder/src/index.ts#L241-L245)

```typescript
const identityCommitment = computeIdentityCommitment(
    masterPublicKey.x,
    masterPublicKey.y,
    revocationNonce
);
```

But the circuit's `IdentityAnchor` computes `identityState = Poseidon(Ax, Ay, revocationNonce)` where `Ax, Ay` are the **per-schema derived** keys (output of `BabyPbk(Poseidon(masterKey, schemaHash))`), NOT the master public key directly.

The SDK passes `masterPublicKey.x/y` (the top-level BJJ key), while the circuit derives schema-specific keys internally. The global tree would need to store `Poseidon(derivedAx, derivedAy, revocNonce)` for EACH schema, but the SDK queries for a single `identityCommitment` using the master key.

This is a fundamental **identity commitment mismatch** between the SDK and the circuit.

---

### BUG-05: `verifyOnChain` Returns Hardcoded Transaction Signature

**Location:** [ts-sdk/packages/sdk/src/index.ts:117](file:///c:/Users/KIIT/Desktop/solid-protocol/ts-sdk/packages/sdk/src/index.ts#L117)

```typescript
return 'SOLANA_TX_SIGNATURE_RC_100';
```

This is a stub that will never submit an actual transaction.

---

### BUG-06: `flake.nix` Pin vs `devnet.json` Toolchain Mismatch

The `devnet.json` records:
```json
"anchor_cli": "1.0.0",
"solana_cli": "3.1.12",
"rust": "1.94.1",
"node": "v24.10.0"
```

But the actual pinned versions are:
- Anchor 0.30.1 (not 1.0.0)
- Solana 1.18.22 (not 3.1.12)
- Rust 1.79.0 (not 1.94.1)
- Node 18 (not v24.10.0)

This means `devnet.json` was hand-edited with wrong metadata or deployed from a different environment.

---

### BUG-07: `compound_query.circom` Expiration Check Missing from Batch Circuit

The single-credential `compound_query.circom` has an `ExpirationChecker` (Step 4), but the `batch_credential_query.circom` does **not** check `expirationTimestamps[i]` — the private input is wired into the circuit but never constrained. Expired credentials pass the batch verifier.

---

### BUG-08: `LocalReplicaAdapter` Memory Bomb

**Location:** [ts-sdk/packages/light/src/index.ts:303](file:///c:/Users/KIIT/Desktop/solid-protocol/ts-sdk/packages/light/src/index.ts#L303)

```typescript
while (nodes.length < 1 << this.depth) nodes.push(zero);
```

With `depth = 20`, this allocates `2^20 = 1,048,576` Uint8Array(32) entries ≈ **33 MB** on every proof request. For a tree depth of 26 (if ever configured), it would be **2 GB**. The comment says "Do NOT use for production" but there's no guard.

---

## 4. Circuit Security Analysis

### ✅ Passed Checks

| Check | Status |
|-------|--------|
| Nullifier is domain-separated by `verifierAddress` (SEC-13) | ✅ |
| Nullifier includes `queryContextHash` — prevents query malleability | ✅ |
| Canonical ordering enforcement (`schemaHash[i] < schemaHash[i+1]`) | ✅ |
| Zero-schema integrity (padding slots force all fields = 0) | ✅ |
| Merkle inclusion uses conditional check (`enabled * diff === 0`) | ✅ |
| EdDSA signature verification gated by `enabled` flag | ✅ |
| `pathIndices` constrained to binary (`p * (p-1) === 0`) | ✅ |
| Poseidon commitment matches canonical formula | ✅ |
| `BabyPbk` derives per-schema keys from master key | ✅ |
| `PredicateEvaluator` uses one-hot operator decoding (no soundness gap) | ✅ |

### ⚠️ Observations

1. **`currentTimestamp` is not constrained** in the batch circuit — it's a public input but nothing in the circuit checks credentials against it. The `ExpirationChecker` from `nullifier_expiry.circom` is only used in `compound_query.circom`.

2. **Circom `LessThan(252)` bit-width** is used for ordering. At 252 bits, this works for BN254 field elements but has a subtle edge: values near `p/2` can wrap. Since `schemaHash` values should be Poseidon outputs (≤ 254 bits), this is safe in practice.

3. **No circuit-level issuer public key verification** against the on-chain registry. The circuit trusts the issuer signature, and the on-chain verifier checks schema roots, but there's no binding between `issuerPubKeyAx/Ay` in the proof and any on-chain issuer account.

---

## 5. SDK Security Analysis

### ✅ Good Practices

| Practice | Details |
|----------|---------|
| Random salt generation | `crypto.getRandomValues(salt)` — cryptographically secure |
| Input validation | `schemaHash.length !== 32` checks throughout |
| Resilient RPC | Priority failover with sticky active endpoint |
| Dry-run mode | `dryRun?: boolean` in issuance options |
| Identity cohesion check | SEC-17 enforced before batch proof |

### ⚠️ Issues

1. **`ISSUE_CREDENTIAL_DISCRIMINATOR`** is manually pinned — if the Anchor IDL changes the instruction namespace, this will silently fail. Should be derived from `sha256("global:issue_credential")[..8]` at build time or validated in tests.

2. **`formatProofForSolana`** does NOT negate the A point. The on-chain verifier calls `negate_g1_point(&proof_a)`, expecting the *un-negated* A point. The SDK comments say "negate Y coordinate for groth16-solana" but doesn't actually do it — it just copies `pi_a` directly. Fortunately, the on-chain code handles negation, so this is consistent, but the comment is misleading.

3. **Node.js `crypto` dependency** — `crypto.getRandomValues(salt)` uses the Web Crypto API, which requires either a browser context or Node.js ≥ 17. Server-side usage on older Node versions will crash.

---

## 6. Infrastructure & CI Security

### ✅ CI Pipeline Assessment

The CI pipeline is **exceptionally well-designed**:

| Job | Purpose | Verdict |
|-----|---------|---------|
| `toolchain` | Pinned bootstrap + cache | ✅ Excellent |
| `fmt-clippy` | Format + lint enforcement | ✅ |
| `rust` | Library tests (solid-core, solid-light) | ✅ |
| `prover` | Separate workspace test | ✅ |
| `anchor` | BPF build + IDL sanity | ✅ |
| `wasm` | WASM bridge build | ✅ |
| `circuits` | Circom compile gate | ✅ |
| `sdk` | TypeScript build + tests | ✅ |
| `cross_language_vectors` | Rust↔TS bit-level agreement | ✅ **Best-in-class** |
| `program_id_consistency` | Anchor.toml ↔ declare_id! | ✅ |

### ⚠️ Gaps

1. **No integration test job** — there's no `anchor test` in CI. The BPF build passes, but no program-test harness validates instruction-level behavior.

2. **No circuit witness-generation test** — circuits are compiled but never run with test vectors in CI.

3. **`check_program_ids.py` does not check `devnet.json`** — as evidenced by CRITICAL-01.

4. **Bootstrap script has no checksum verification** — the Solana CLI binary is downloaded via HTTPS but its hash is not verified against a known-good value.

---

## 7. Comprehensive System Assessment

### 💎 Strengths (Pros)

| # | Strength | Details |
|---|----------|---------|
| 1 | **Architecturally Sophisticated** | The three-program design (Issuer Registry + Schema Registry + ZK Verifier) with clean separation of concerns is production-grade. Each program has a single, well-defined responsibility. |
| 2 | **Cryptographically Sound Core** | The Groth16/BN254 + Poseidon + BabyJubJub EdDSA stack is the gold standard for on-chain ZK verification (used by Polygon ID, Worldcoin, Semaphore). |
| 3 | **Hardened Nullifier Design** | The 5-input nullifier `Poseidon(masterKey, revocNonce, verifierAddr, queryContextHash, verifierNonce)` is one of the most robust anti-replay designs in production identity systems. |
| 4 | **Backend-Agnostic Storage** | The CPI helpers (`verify_state_root_matches`, `verify_schema_root_binding`) work with ANY account shape that follows the documented layout. Not locked to Light Protocol. |
| 5 | **Cross-Language Vector Testing** | The CI enforces bit-for-bit agreement between Rust and TypeScript on commitments & nullifiers. This is rare even in mature ZK projects. |
| 6 | **SPL Account Compression Integration** | Using SPL AC for credential trees is production-proven (cNFT standard) and avoids the dependency mess of Light Protocol. |
| 7 | **DAO Governance Model** | Tiered staking, flash-loan protection (100-slot maturity), and trust-anchor bypass create a governance system that balances decentralization with operational reality. |
| 8 | **Composable Batch Proofs** | Proving across 4 credentials simultaneously with canonical ordering is unique in the Solana ecosystem. |
| 9 | **Reproducible Dev Environment** | The `flake.nix` + `bootstrap.sh` combination gives byte-identical toolchains across all environments. |
| 10 | **Stack-Safe VK Parsing** | The `VkBuf` struct eliminates the previous `Box::leak` memory leak and fits within BPF's 4KB stack budget with compile-time assertions. |

---

### ⚠️ Weaknesses (Cons)

| # | Weakness | Details |
|---|----------|---------|
| 1 | **Holder SDK Has Stub-Level Proving** | The top-level `SolID.prove()` and `SolID.verifyOnChain()` return hardcoded placeholders. Only the lower-level `@solid-protocol/holder` package has real proving, but it requires the caller to bring their own circuit artifacts (.wasm + .zkey). |
| 2 | **No Circuit Artifacts Shipping** | The `.wasm` and `.zkey` files are referenced via CDN (`https://cdn.solid-protocol.com/artifacts/v1`) but are not generated in CI and no CDN is actually deployed. A dApp integrator has no way to get working circuit artifacts without compiling circom themselves. |
| 3 | **No On-Chain Integration Tests** | There are zero `program-test` or `bankrun` harnesses. All testing is unit-level (Rust lib tests, VkBuf parser tests). The program instructions have never been tested against a real Solana runtime in CI. |
| 4 | **No Credential Revocation** | `docs/REVOCATION_DESIGN.md` documents a design but it is not implemented. Issued credentials are permanent — there is no on-chain revocation mechanism. |
| 5 | **Governance Token Not Deployed** | The `SOLID_TOKEN_MINT` is a placeholder. The DAO voting/staking system cannot function without a real SPL token. |
| 6 | **Missing Issuer Public Key Verification on Proof Path** | The ZK proof proves the holder has a credential signed by *some* issuer, and the on-chain verifier checks the Merkle root against the schema-tree binding, but it does NOT verify that the issuer who signed the credential is an *approved* issuer in the registry. |
| 7 | **Documentation-Code Drift** | The `devnet.json` manifest is completely stale. Multiple docs reference Light Protocol concepts that have been sunset in the v0.2 codebase. |
| 8 | **Single Upgrade Authority** | All three programs use a single deployer keypair as upgrade authority. In production, this should be a multisig. |
| 9 | **No Fee/Rent Mechanism** | No mechanism for the protocol to sustain itself — all operations are free beyond Solana transaction fees. |
| 10 | **Fixed Circuit Parameters** | `NUM_CREDS=4`, `NUM_FIELDS=8`, `MAX_PREDICATES=4` are compile-time constants. Changing them requires a full ceremony re-run. |

---

## 8. Complete Deployment Guide

### 8.0 Prerequisites Overview

You need the following tools installed:

| Tool | Required Version | Purpose |
|------|-----------------|---------|
| **Rust** | 1.79.0 | Compile on-chain programs + solid-core/solid-light |
| **Solana CLI** | 1.18.22 | Blockchain interaction, keypair management, deployment |
| **Anchor CLI** | 0.30.1 | Build Anchor programs, generate IDLs |
| **Node.js** | 18.x LTS | TypeScript SDK compilation + testing |
| **circom** | 2.1.9 | Compile ZK circuits (.circom → .r1cs + .wasm) |
| **snarkjs** | 0.7.5 | Trusted setup + proof generation/verification |
| **wasm-pack** | 0.13.1 | Build solid-core → WebAssembly for browser/Node |
| **Git** | 2.x+ | Version control |
| **Python** | 3.11+ | Run `check_program_ids.py` |

> [!IMPORTANT]
> **You are on Windows.** The SolID toolchain (Solana CLI, Anchor, circom) only runs natively on Linux/macOS. **You MUST use WSL2 (Windows Subsystem for Linux)** for development and deployment.

---

### 8.1 Setting Up WSL2 (Windows Users)

```powershell
# Step 1: Install WSL2 (from PowerShell as Administrator)
wsl --install -d Ubuntu-22.04

# Step 2: Restart your computer, then open Ubuntu terminal
# Step 3: Update system
sudo apt update && sudo apt upgrade -y

# Step 4: Install essential build tools
sudo apt install -y build-essential pkg-config libssl-dev curl git python3 python3-pip jq libudev-dev
```

> [!NOTE]
> All remaining commands should be run inside WSL2 Ubuntu terminal, not PowerShell.

---

### 8.2 Installing All Dependencies

#### A. Rust 1.79.0
```bash
# Install rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

# Pin to exact version
rustup install 1.79.0
rustup default 1.79.0

# Add WASM target for solid-core → browser compilation
rustup target add wasm32-unknown-unknown

# Verify
rustc --version   # Should show: rustc 1.79.0
cargo --version   # Should show: cargo 1.79.0
```

#### B. Solana CLI 1.18.22
```bash
# Download and install
sh -c "$(curl -sSfL https://release.anza.xyz/v1.18.22/install)"

# Add to PATH
echo 'export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc

# Verify
solana --version   # Should show: solana-cli 1.18.22

# Configure for devnet
solana config set --url https://api.devnet.solana.com

# Generate a keypair (if you don't have one)
solana-keygen new --outfile ~/.config/solana/id.json

# Fund your wallet with devnet SOL (you'll need ~5 SOL for deployment)
solana airdrop 5
solana airdrop 5    # Run multiple times if needed (2 SOL per airdrop limit)
```

#### C. Anchor CLI 0.30.1
```bash
# Install via cargo
cargo install --git https://github.com/coral-xyz/anchor --tag v0.30.1 anchor-cli --locked

# Verify
anchor --version   # Should show: anchor-cli 0.30.1
```

#### D. Node.js 18.x
```bash
# Install nvm
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash
source ~/.bashrc

# Install Node 18
nvm install 18
nvm use 18

# Verify
node --version    # Should show: v18.x.x
npm --version     # Should show: 10.x.x
```

#### E. circom 2.1.9
```bash
# Clone and build from source
git clone --depth 1 --branch v2.1.9 https://github.com/iden3/circom /tmp/circom
cd /tmp/circom
cargo install --path circom --locked
cd -

# Verify
circom --version   # Should show: circom compiler 2.1.9
```

#### F. snarkjs 0.7.5
```bash
# Install globally
npm install -g snarkjs@0.7.5

# Verify
snarkjs --version  # Should show: 0.7.5
```

#### G. wasm-pack 0.13.1
```bash
cargo install --locked --version 0.13.1 wasm-pack

# Verify
wasm-pack --version   # Should show: wasm-pack 0.13.1
```

---

### 8.3 Building the Project

```bash
# Navigate to project (adjust path for WSL2)
cd /mnt/c/Users/KIIT/Desktop/solid-protocol

# ─── 1. Build Rust libraries ─────────────────────────────────
cargo build -p solid-core -p solid-light
cargo test -p solid-core -p solid-light --no-fail-fast

# ─── 2. Build the WASM bridge ────────────────────────────────
cd crates/solid-core
wasm-pack build --target nodejs --out-dir ../../ts-sdk/packages/core/wasm --release
cd ../..

# ─── 3. Build on-chain programs (BPF) ────────────────────────
anchor build

# Verify the build artifacts exist
ls -la target/deploy/*.so
# Should show:
#   issuer_registry.so
#   schema_registry.so
#   zk_verifier.so

# ─── 4. Build TypeScript SDK ─────────────────────────────────
cd ts-sdk
npm install
npm run build
cd ..

# ─── 5. Compile circuits ─────────────────────────────────────
cd circuits
npm install    # installs circomlib
mkdir -p ../target/circuits

circom batch_credential_query.circom \
    --r1cs --wasm --sym \
    -o ../target/circuits \
    -l node_modules -l lib

circom compound_query.circom \
    --r1cs --wasm --sym \
    -o ../target/circuits \
    -l node_modules -l lib

cd ..

# ─── 6. Run unit tests ───────────────────────────────────────
cargo test -p zk-verifier --lib --no-fail-fast
```

---

### 8.4 Deploying to Devnet

```bash
# Ensure you're on devnet
solana config set --url https://api.devnet.solana.com

# Check your balance (need ~5 SOL)
solana balance

# ─── Deploy all three programs ───────────────────────────────

# Deploy Schema Registry
anchor deploy --program-name schema_registry --provider.cluster devnet

# Deploy Issuer Registry
anchor deploy --program-name issuer_registry --provider.cluster devnet

# Deploy ZK Verifier
anchor deploy --program-name zk_verifier --provider.cluster devnet

# ─── Verify deployments ─────────────────────────────────────
solana program show DPk6XUH6CArLWt4KMqJmpNBnPwQ3gG9P3dBd3MDVE3bT
solana program show CRGYfonXwDk6gKEm9fC1U33VVBkqnQVD3sPdLKzqHWoR
solana program show BZkVFdMhAEeGMvEAhXNjt3r3bEA2sCPqEFcsEbSbFGj2

# ─── Upload IDLs ─────────────────────────────────────────────
anchor idl init BZkVFdMhAEeGMvEAhXNjt3r3bEA2sCPqEFcsEbSbFGj2 \
    --filepath target/idl/zk_verifier.json \
    --provider.cluster devnet

anchor idl init CRGYfonXwDk6gKEm9fC1U33VVBkqnQVD3sPdLKzqHWoR \
    --filepath target/idl/issuer_registry.json \
    --provider.cluster devnet

anchor idl init DPk6XUH6CArLWt4KMqJmpNBnPwQ3gG9P3dBd3MDVE3bT \
    --filepath target/idl/schema_registry.json \
    --provider.cluster devnet
```

---

### 8.5 Post-Deployment Initialization

After deployment, you must initialize the on-chain state (PDAs):

```bash
# ─── Initialize Issuer Registry ──────────────────────────────
# Using the root package.json script (requires the initialize.ts E2E script):
npm run init-onchain

# Or manually via Anchor:
# 1. Initialize RegistryConfig PDA
# 2. Initialize stake vault PDA
# 3. Initialize governance vault PDA

# ─── Initialize Schema Registry Globals ──────────────────────
# 1. Initialize GlobalStateBinding PDA
# 2. Register your first schema
# 3. Create the SPL AC credential tree for each schema
# 4. Initialize SchemaTreeBinding PDAs

# ─── Initialize ZK Verifier ─────────────────────────────────
# 1. Initialize VerifierConfig PDA
# 2. Store the verification key (from trusted setup)
```

---

### 8.6 Trusted Setup (Circuit Key Generation)

> [!CAUTION]
> The trusted setup ceremony creates the proving key (.zkey) that the holder uses to generate proofs. If the toxic waste from the ceremony is not properly destroyed, the entire system's soundness is compromised. For production, use a multi-party ceremony (e.g., Hermez, Snark.js Ceremony).

```bash
# ─── Phase 1: Powers of Tau (universal, reusable) ────────────
snarkjs powersoftau new bn128 20 pot20_0000.ptau -v
snarkjs powersoftau contribute pot20_0000.ptau pot20_0001.ptau \
    --name="First contribution" -v
snarkjs powersoftau prepare phase2 pot20_0001.ptau pot20_final.ptau -v

# ─── Phase 2: Circuit-specific setup ─────────────────────────
# For batch_credential_query:
snarkjs groth16 setup \
    target/circuits/batch_credential_query.r1cs \
    pot20_final.ptau \
    batch_credential_query_0000.zkey

snarkjs zkey contribute \
    batch_credential_query_0000.zkey \
    batch_credential_query_final.zkey \
    --name="SolID batch setup" -v

# Export verification key (for on-chain storage)
snarkjs zkey export verificationkey \
    batch_credential_query_final.zkey \
    batch_credential_query_vk.json

# ─── Store VK on-chain ───────────────────────────────────────
# Convert VK JSON to the binary format expected by zk-verifier
# (This requires the regen_devnet_manifest.py script or custom tooling)
```

---

### 8.7 Where to Deploy (Production)

| Environment | RPC Provider | Cost | Notes |
|-------------|-------------|------|-------|
| **Devnet** | `api.devnet.solana.com` | Free | Testing only; resets periodically |
| **Mainnet-Beta** | [Helius](https://helius.dev/) | $49+/mo | Recommended — has DAS for SPL AC trees |
| | [QuickNode](https://www.quicknode.com/) | $49+/mo | Good fallback |
| | [Triton](https://triton.one/) | $99+/mo | Enterprise-grade |

> [!IMPORTANT]
> **For mainnet deployment, you MUST:**
> 1. Transfer upgrade authority to a **multisig** (e.g., Squads Protocol).
> 2. Use a dedicated RPC provider (NOT the public endpoint).
> 3. Run a proper multi-party trusted setup ceremony.
> 4. Deploy the circuit artifacts to IPFS/Arweave (not a centralized CDN).
> 5. Set `AUTHORITY_PUBKEY` and `SOLID_TOKEN_MINT` to real production keys.

### 8.8 Mainnet Deployment Checklist

```
[ ] Fix all CRITICAL and HIGH vulnerabilities listed in this audit
[ ] Run full integration test suite against localnet
[ ] Complete multi-party trusted setup ceremony
[ ] Transfer program upgrade authority to multisig
[ ] Deploy circuit artifacts to decentralized storage (IPFS/Arweave)
[ ] Configure production RPC endpoints (Helius/QuickNode/Triton)
[ ] Deploy governance token (SPL Token)
[ ] Set up monitoring + alerting for program logs
[ ] Publish IDLs to Anchor IDL registry
[ ] Update devnet.json with correct post-deployment metadata
[ ] Third-party security audit (recommended: OtterSec, Halborn, or Trail of Bits)
```

---

> **End of Audit Report**
>
> This audit was performed as a code review without dynamic testing (no program-test execution). The findings are based on static analysis of all source files in the repository as of commit `64a9f78` on the `main` branch.
