# SolID Protocol -- Improvement Roadmap

> **Original document date:** 2026-04-21
> **Last status reconciliation:** 2026-04-25 (Phase 3 kickoff doc sweep)
> **Original source:** Independent system audit (2026-04-21, Antigravity)
>
> **Status as of 2026-04-25.** Phase 1 and Phase 2 are closed. The
> sec/SECURITY_REGISTRY.md living tracker is the canonical source of
> truth for every open and closed security item. This document is
> preserved as the historical backlog and is kept reconciled with the
> registry. When the two disagree, the registry wins.
>
> - New findings since 2026-04-21: recorded in the registry as
>   SOLID-SEC-NNN, not appended here.
> - Closed items: marked `[x]` below with a pointer to the relevant
>   SOLID-SEC-NNN (or `-- no registry ID` if the item predates the
>   registry).
> - Cross-check: `scripts/check_docs.py` (Phase 1 work) enforces that
>   `[x]` marks here agree with the registry's status board.

## How to read this document

Items are grouped into four priority tiers. Emoji legend preserved for
history; plain ASCII is used everywhere else per CLAUDE.md.

| Tier | Meaning |
|------|---------|
| **P0** Blocking     | System cannot be tested or deployed until fixed |
| **P1** Pre-devnet   | Must fix before any public devnet testing |
| **P2** Pre-mainnet  | Must fix before a mainnet launch |
| **P3** Polish       | Quality-of-life improvements; do after mainnet |

Each entry has:
- A short **title**
- The **exact file/line** where the problem lives
- A **description** of the fix required
- The **risk** if left unfixed

---

## P0 — Blocking (Fix These First, In Order)

These six issues mean the system **cannot produce or verify a single real proof today**.
Fix them before writing any new features.

---

### P0-1 — `RegistryConfig` Account Space Is 80 Bytes, Needs 112

**File:** `programs/issuer-registry/src/lib.rs`, line 613

```rust
// WRONG — missing governance_token_mint (32 bytes)
space = 8 + 32 + 8 + 8 + 8 + 8 + 8,   // = 80

// CORRECT
space = 8 + 32 + 32 + 8 + 8 + 8 + 8 + 8, // = 112
```

**What to fix:** Add the missing `+ 32` for `governance_token_mint: Pubkey` in the `InitializeRegistry` account constraint.

**Risk if not fixed:** `initialize_registry` has never succeeded on any real cluster, and never will. The DAO is completely undeployable. Every downstream instruction that reads `RegistryConfig` will read corrupted data.

---

### P0-2 — `global_tree` and `schema_tree_N` Accounts Have No Owner Check

**File:** `programs/zk-verifier/src/lib.rs`, lines 459–469

```rust
/// CHECK: Global-state Merkle tree account (Light Protocol or SolID-native).
pub global_tree: UncheckedAccount<'info>,

/// CHECK: Per-schema tree metadata PDA (slot 0).
pub schema_tree_0: UncheckedAccount<'info>,
// ... schema_tree_1, _2, _3
```

**What to fix:** Before calling `cpi_helpers::verify_state_root_matches` / `verify_schema_root_binding`, assert that the account is owned by the `schema-registry` program. Add an owner check in the handler body:

```rust
require_keys_eq!(
    *ctx.accounts.global_tree.owner,
    schema_registry::ID,
    ErrorCode::InvalidGlobalRoot
);
```

Repeat for each `schema_tree_N`.

**Risk if not fixed:** An attacker can craft a system-program-owned account with the correct `globroot` / `schmtree` discriminator prefix and a fake root. The entire ZK verification pipeline is bypassable. This is the most dangerous production vulnerability in the system.

---

### P0-3 — `vote_on_issuer` Never Increments `active_votes_count`

**File:** `programs/issuer-registry/src/lib.rs`, lines 144–178

The `vote_on_issuer` instruction creates a `VoteRecord` and tallies votes, but never writes to `staker_account.active_votes_count`. The `unstake_tokens` guard checks `active_votes_count == 0`, meaning voters can stake → vote → immediately unstake with zero economic stake locked.

**What to fix:** Add at end of handler, before `Ok(())`:

```rust
let staker = &mut ctx.accounts.staker_account;
staker.active_votes_count = staker.active_votes_count
    .checked_add(1)
    .ok_or(ErrorCode::Overflow)?;
```

Also mark `staker_account` as `#[account(mut, ...)]` in the `VoteOnIssuer` context struct.

**Risk if not fixed:** Any staker can vote with zero economic consequence. DAO governance is completely free to manipulate.

---

### P0-4 — `vote_on_issuer` Does Not Enforce the Voting Deadline

**File:** `programs/issuer-registry/src/lib.rs`, lines 144–178

There is no check that the current time is before `issuer.voting_ends_at`. Votes can be cast after the period closes, before anyone calls `finalize_voting`.

**What to fix:** Add at the top of the handler:

```rust
require!(
    Clock::get()?.unix_timestamp < issuer.voting_ends_at,
    ErrorCode::VotingPeriodEnded
);
```

**Risk if not fixed:** An attacker can monitor the tally, wait for the voting period to expire, then stuff enough late votes to swing the result before `finalize_voting` is called.

---

### P0-5 — BUG-02: Holder SDK Parses Nullifier as Hex but snarkjs Returns Decimal

**File:** `ts-sdk/packages/holder/src/index.ts`, line 310

```typescript
// WRONG — publicSignals[0] is a decimal string like "123456789...", not hex
const nullifier = Uint8Array.from(Buffer.from(publicSignals[0], 'hex'));

// CORRECT
const nullifier = bigintToBytes32(BigInt(publicSignals[0]));
```

**Risk if not fixed:** Every batch proof submission registers a garbage nullifier on-chain. Either the nullifier PDA init fails (wrong seeds), or a wrong nullifier is registered, allowing the real proof to be replayed indefinitely.

---

### P0-6 — `@solid-protocol/verifier` Package Is Missing

**Location:** `ts-sdk/packages/` directory

`docs/SOLID_FINAL_AUDIT_2026_04.md` and `docs/e2e_run_guide.md` reference a `@solid-protocol/verifier` package with `buildVerifyBatchProofIx`, `verifyOnChain`, `checkIssuerStatus`, and PDA derivation helpers. This package directory has no `src/` folder — it was described in audit documents but never committed.

**What to fix:** Create `ts-sdk/packages/verifier/src/index.ts` with:
- `buildVerifyBatchProofIx(params)` — hand-rolled Anchor instruction builder (discriminator = `sha256("global:verify_batch_proof")[..8]`, Borsh-serialized args)
- `verifyOnChain(connection, payer, request, trees)` — sends a real confirmed transaction
- `deriveNullifierPda(nullifier)` — PDA at seeds `[b"null", nullifier]`
- `deriveVerifierConfigPda()` — PDA at seeds `[b"verifier-config"]`
- `deriveVkStoragePda(verifierConfigKey)` — PDA at seeds `[b"vk-storage", config]`
- `checkIssuerStatus(connection, issuerAuthority)` — reads and parses `IssuerAccount` PDA

**Risk if not fixed:** Every E2E guide in the docs references this package. It cannot be imported. All demo and integration code fails at the first import statement.

---

## P1 — Pre-Devnet (Fix Before Any Public Test)

---

### P1-1 — `ReleaseVote` Accounts Not Marked `mut`

**File:** `programs/issuer-registry/src/lib.rs`, lines 851–868

Both `staker_account` and `vote_record` are missing `#[account(mut)]`. Anchor silently discards writes to non-mut accounts — `release_vote` is a no-op that never actually releases anything.

**What to fix:** Add `mut` to both accounts in the `ReleaseVote` context struct:

```rust
#[account(mut, seeds = [b"staker", voter.key().as_ref()], bump)]
pub staker_account: Account<'info, StakerAccount>,

#[account(mut, seeds = [...], bump, constraint = ...)]
pub vote_record: Account<'info, VoteRecord>,
```

---

### P1-2 — `IncrementUsage` Has No Access Control

**File:** `programs/schema-registry/src/lib.rs`, lines 371–375

Anyone can call `increment_usage` and spam `usage_count` to `u64::MAX`, causing `Overflow` errors and DoS-ing CPI callers.

**What to fix:** Gate behind either:
- A CPI-caller check (only `issuer-registry` can call it), or
- An authority signer requirement

At minimum, add a `#[account(constraint = caller.key() == issuer_registry_program_id)]` or convert to a CPI-only instruction.

---

### P1-3 — `slash_issuer` Does Not Transfer Slashed Lamports

**File:** `programs/issuer-registry/src/lib.rs`, lines 355–388

The handler decrements `issuer.staked_amount` but does not move lamports from `stake_vault` to a treasury. Slashed lamports are permanently locked in the vault. `submit_fraud_proof` has the same issue.

**What to fix:** After decrementing `staked_amount`, add a lamport transfer from `stake_vault` to a DAO treasury account (add it to the context struct):

```rust
**ctx.accounts.stake_vault.to_account_info().try_borrow_mut_lamports()? -= slash_amount;
**ctx.accounts.dao_treasury.to_account_info().try_borrow_mut_lamports()? += slash_amount;
```

---

### P1-4 — `approve_via_trust_anchor` Does Not Emit `IssuerApproved` Event

**File:** `programs/issuer-registry/src/lib.rs`, lines 444–462

`finalize_voting` emits `IssuerApproved` when an issuer is approved via DAO vote. But `approve_via_trust_anchor` (which can also approve issuers) emits nothing. Off-chain indexers relying on `IssuerApproved` to populate the compressed issuer tree will not learn about trust-anchor approvals.

**What to fix:** Add an `emit!(IssuerApproved { ... })` call inside `approve_via_trust_anchor`, matching the structure emitted by `finalize_voting`.

---

### P1-5 — `approve_via_trust_anchor` Has No PDA Constraint on `target_issuer`

**File:** `programs/issuer-registry/src/lib.rs`, line 712

```rust
#[account(mut)]
pub target_issuer: Account<'info, IssuerAccount>,
```

`target_issuer` can be any `IssuerAccount`, not just one derived from the canonical `[b"issuer", authority]` seed. A malicious trust anchor could approve a crafted account.

**What to fix:** Add a seed constraint to verify `target_issuer` is a legitimate PDA by requiring the target authority pubkey to be passed as an instruction argument and used in the seed derivation.

---

### P1-6 — `VkStorage` Has No Cap on Total VK Size

**File:** `programs/zk-verifier/src/lib.rs`, line 428

The account is allocated `8 + 4 + 10240` bytes, but `store_verification_key` does an unbounded `extend_from_slice`. Sending more chunks than the account can hold causes a serialization error.

**What to fix:** Add a length cap in the handler:

```rust
require!(
    vk_storage.data.len() + chunk_data.len() <= 10240,
    ErrorCode::VkStorageFull
);
```

And add `VkStorageFull` to the error enum.

---

### P1-7 — BUG-04: `generateBatchProof` Uses Master Public Key for Identity Commitment

**File:** `ts-sdk/packages/holder/src/index.ts`, lines 241–245

```typescript
// WRONG — uses master key directly
const identityCommitment = computeIdentityCommitment(
    masterPublicKey.x,
    masterPublicKey.y,
    revocationNonce
);
```

The circuit's `IdentityAnchor` computes `identityState = Poseidon(derivedAx, derivedAy, revocationNonce)` where `Ax, Ay` are the **per-schema derived** keys (output of `BabyPbk(Poseidon(masterKey, schemaHash))`), NOT the master public key. The SDK and circuit are computing different leaves, so the global Merkle inclusion proof will always fail.

**What to fix:** Either derive per-schema keys client-side before computing the identity commitment, or align the circuit to use the master public key directly (and update the `IdentityAnchor` template accordingly).

---

### P1-8 — Update `deployments/devnet.json`

**File:** `deployments/devnet.json`

The file records an April 4 deployment with completely wrong program IDs and a `nullifier_bloom` PDA that no longer exists. Toolchain versions are also fabricated.

**What to fix:**
- Update all three program IDs to match `Anchor.toml`
- Remove the `nullifier_bloom` PDA entry (replaced by PDA-per-nullifier)
- Update `toolchain` section to match actual pinned versions (`anchor_cli: "0.30.1"`, `solana_cli: "1.18.22"`, `rust: "1.79.0"`, `node: "18.x"`)
- Update `deployed_at` timestamp after next deployment

---

### P1-9 — Wire `check_program_ids.py` to Also Validate `devnet.json`

**File:** `scripts/check_program_ids.py`

The CI script currently only checks `Anchor.toml ↔ declare_id!` consistency. It does not check that `deployments/devnet.json` matches.

**What to fix:** Extend the script to also parse `devnet.json` and assert that each program ID matches `Anchor.toml`. Fail CI on drift.

---

## P2 — Pre-Mainnet (Required Before Production)

---

### P2-1 — Add Missing Circuit Constraint: `numPredicates <= MAX_PREDICATES`

**File:** `circuits/batch_credential_query.circom`

`numPredicates` is a public input with no upper-bound constraint. Values > 4 are silently treated as 4 (all predicates active). This is a circuit underspecification.

**What to fix:** Add after the signal declarations:

```circom
component numPredsCheck = LessEqThan(8);
numPredsCheck.in[0] <== numPredicates;
numPredsCheck.in[1] <== MAX_PREDICATES;
numPredsCheck.out === 1;
```

---

### P2-2 — Add Missing Circuit Constraint: `compoundLogic` Must Be Binary

**File:** `circuits/batch_credential_query.circom`

`compoundLogic` is a public input with no constraint. Values ≥ 2 silently behave as AND. This is an underspecification that a malicious prover could exploit to cause unexpected behavior.

**What to fix:**

```circom
// Constrain compoundLogic to {0, 1}
compoundLogic * (compoundLogic - 1) === 0;
```

---

### P2-3 — Fix `expirationTimestamps` Not Constrained in Batch Circuit

**File:** `circuits/batch_credential_query.circom`

`expirationTimestamps[i]` is a private input that is wired in but **never constrained**. The `ExpirationChecker` template exists in `lib/nullifier_expiry.circom` but is never instantiated in the batch circuit. Expired credentials silently pass the batch verifier.

**What to fix:** Instantiate `ExpirationChecker` for each active credential slot and connect it to `currentTimestamp`:

```circom
component expiryCheck[NUM_CREDS];
for (var i = 0; i < NUM_CREDS; i++) {
    expiryCheck[i] = ExpirationChecker();
    expiryCheck[i].currentTimestamp <== currentTimestamp;
    expiryCheck[i].expirationTimestamp <== expirationTimestamps[i];
    // Only enforce for active (non-zero-schema) slots
    (1 - isZero[i].out) * (1 - expiryCheck[i].valid) === 0;
}
```

---

### P2-4 — Fix `resolveSchema` memcmp Offset in SDK

**File:** `ts-sdk/packages/sdk/src/index.ts`, line 133

```typescript
// WRONG — uses hardcoded 585 (assumes 256-byte strings, wrong)
{ memcmp: { offset: 8+32+256+1+256+32, bytes: ... } }
```

`SchemaAccount` has Borsh-encoded `String` fields using `4 + actual_length` bytes, not 256. The hardcoded offset is incorrect.

**What to fix:** Calculate the correct offset based on the actual Borsh layout: `offset = discriminator(8) + authority(32) + name_len_prefix(4) + max_name(64) + version(1) + category_len_prefix(4) + max_category(64) + field_names_vec(...)`. Or use `getProgramAccounts` without the wrong filter and filter client-side after deserializing.

---

### P2-5 — Fix `listIssuers` Filter Byte and Offset

**File:** `ts-sdk/packages/sdk/src/index.ts`, line 158

```typescript
// WRONG — offset doesn't account for Borsh string length prefixes;
//         bytes: '2' is wrong (Approved = variant index 1, not 2)
{ memcmp: { offset: 8+32+64+128+32+32+1, bytes: '2' } }
```

**What to fix:** Calculate the correct Borsh offset accounting for `String` length prefixes in `name` (4+64) and `metadata_uri` (4+128), and use `bytes: Buffer.from([1]).toString('base58')` for `IssuerStatus::Approved` (enum discriminant 1).

---

### P2-6 — `SchemaAccount` Space May Undercount `field_names`

**File:** `programs/schema-registry/src/lib.rs`, line 354

```rust
space = 8 + 32 + 64 + 1 + 64 + 256 + 32 + 1 + 8 + 8
```

`field_names: Vec<String>` in Borsh is `4 (length prefix) + sum(4 + str_len for each string)`. With 8 fields of up to 32 chars each, the maximum size is `4 + 8*(4+32) = 292` bytes, not the hardcoded `256`.

**What to fix:** Increase the space allocation or cap field name length in the instruction and account for it exactly:

```rust
space = 8 + 32 + (4+64) + 1 + (4+64) + (4 + 8*(4+32)) + 32 + 1 + 8 + 8
```

---

### P2-7 — Fix `globalSiblings` Hardcoded Depth in Batch Circuit

**File:** `circuits/batch_credential_query.circom`, lines 48–49 and 72

```circom
signal input globalSiblings[NUM_CREDS][20];  // 20 hardcoded
signal input globalPathIndices[NUM_CREDS][20];  // 20 hardcoded

anchors[i] = IdentityAnchor(20);  // 20 hardcoded, not TREE_DEPTH
```

The batch circuit template takes `TREE_DEPTH` as a parameter (for the schema trees), but the global tree depth is hardcoded to `20`. These two depths should be independent parameters.

**What to fix:** Add a `GLOBAL_DEPTH` template parameter and thread it through:

```circom
template BatchCredentialQuerySolana(TREE_DEPTH, GLOBAL_DEPTH, NUM_FIELDS, NUM_CREDS, MAX_PREDICATES) {
    signal input globalSiblings[NUM_CREDS][GLOBAL_DEPTH];
    // ...
    anchors[i] = IdentityAnchor(GLOBAL_DEPTH);
}
component main = BatchCredentialQuerySolana(20, 20, 8, 4, 4);
```

Note: re-running the trusted setup is required after any circuit change.

---

### P2-8 — Deploy Governance Token and Replace All Placeholders

**File:** `ts-sdk/packages/sdk/src/config.ts`, lines 35–36

```typescript
AUTHORITY_PUBKEY: 'SoLid1111111111111111111111111111111111111',
SOLID_TOKEN_MINT: 'SoLidToken11111111111111111111111111111111',
```

Both are placeholder strings. The DAO staking/voting system cannot function without a real SPL token.

**What to fix:**
1. Deploy an SPL token mint for `SOLID_TOKEN_MINT`
2. Set `AUTHORITY_PUBKEY` to the actual deployer/DAO multisig keypair
3. Update `config.ts`, `devnet.json`, and any script that references these placeholders

---

### P2-9 — Add Chunk Sequence Validation to `store_verification_key`

**File:** `programs/zk-verifier/src/lib.rs`, lines 86–99

Chunks can arrive out of order with no error. The handler appends chunk data regardless of `chunk_index` sequence.

**What to fix:** Track expected next chunk index in `VkStorage` or `VerifierConfig`:

```rust
// In VerifierConfig, add: pub next_vk_chunk: u16
require!(chunk_index == config.next_vk_chunk, ErrorCode::ChunkOutOfOrder);
config.next_vk_chunk += 1;
```

---

### P2-10 — Wire Cross-Language Vector Test into CI as Required Gate

**File:** `.github/workflows/` (whichever CI config file exists)

`tests/vectors/check_vectors.ts` exists and is correct. It is NOT wired into CI. Any divergence between Rust and TypeScript cryptographic outputs (the most catastrophic possible bug) goes undetected.

**What to fix:** Add a CI job:
```yaml
cross_language_vectors:
  steps:
    - run: cargo run --example gen_vectors
    - run: cd ts-sdk && npm ci && npx ts-node ../tests/vectors/check_vectors.ts
```
Mark it as a required status check. It must pass before any merge.

---

### P2-11 — Implement Revocation (v1 Design Exists, Just Unimplemented)

**Reference:** `docs/REVOCATION_DESIGN.md` (documents the design accurately)

The revocation design (identity-state rotation via `revocationNonce`) is fully documented and architecturally sound. It is completely absent from the implementation. This is required for any KYC or compliance use case.

**What to fix:**
1. The on-chain scaffolding is already there (`GlobalStateBinding`, `update_global_root`)
2. Implement the holder-side workflow: detect revocation, increment `revocationNonce`, re-derive identity leaf, request re-insertion into global tree
3. Document the issuer-side revocation procedure
4. Later: implement v1.1 (SMT per-credential revocation) per `REVOCATION_DESIGN.md` section 3

---

### P2-12 — Add Issuer Public Key Binding in ZK Proof Path

**File:** `circuits/batch_credential_query.circom` and `programs/zk-verifier/src/lib.rs`

Currently, the circuit proves "credential was signed by SOME BabyJubJub key" and "that credential is in SOME tree whose root is registered." But it does NOT prove "the BJJ key that signed the credential belongs to an **approved** issuer in the registry."

An attacker who calls `issue_credential` (which only checks `issuer.status == Approved`) can put any commitment into the tree, including one signed by an ephemeral key.

**What to fix (two options):**
- **Option A (on-chain):** In `verify_batch_proof`, add a CPI to `issuer-registry::check_issuer_status` for the issuer pubkey extracted from `public_inputs`. Requires passing issuer account addresses.
- **Option B (circuit):** Add `issuerPubKeyHashes[NUM_CREDS]` as public inputs and enforce them against a Merkle membership proof in the issuer tree. More gas-efficient, more private.

---

### P2-13 — Run Trusted Setup Ceremony and Publish Artifacts

Currently there are no `.wasm` or `.zkey` circuit artifacts anywhere. The SDK references `cdn.solid-protocol.com/artifacts/v1` which does not exist.

**What to fix:**
1. Compile the circuits with `circom batch_credential_query.circom --r1cs --wasm`
2. Download the Hermez powers-of-tau (Phase 1): `powersOfTau28_hez_final_21.ptau`
3. Run Phase 2 ceremony: `snarkjs groth16 setup batch.r1cs pot.ptau circuit_0000.zkey`
4. Apply a random contribution: `snarkjs zkey contribute`
5. Export verification key: `snarkjs zkey export verificationkey`
6. Host `.wasm` and `.zkey` at a stable URL and update `SOLID_CONFIG.ARTIFACT_BASE_URL`

> **IMPORTANT:** For mainnet, use a multi-party ceremony (e.g., Hermez ceremony tooling) and publish all contributor attestations. The toxic waste from a single-contributor setup compromises soundness.

---

### P2-14 — Implement Real `SolID.prove()` in Top-Level SDK

**File:** `ts-sdk/packages/sdk/src/index.ts`, lines 66–78

`SolID.prove()` returns a hardcoded stub. It should delegate to `@solid-protocol/holder::generateBatchProof` using circuit artifacts fetched from `SOLID_CONFIG.ARTIFACT_BASE_URL`.

**What to fix:** Replace the stub with:
```typescript
static async prove(query: MultiCredentialQuery, identity: BJJKeypair): Promise<any> {
    const wasmPath = `${SOLID_CONFIG.ARTIFACT_BASE_URL}${SOLID_CONFIG.CIRCUIT_METADATA.BATCH_QUERY.WASM_PATH}`;
    const zkeyPath = `${SOLID_CONFIG.ARTIFACT_BASE_URL}${SOLID_CONFIG.CIRCUIT_METADATA.BATCH_QUERY.ZKEY_PATH}`;
    // ... fetch credentials, merkle proofs, then call generateBatchProof
}
```

---

## P3 — Polish (After Mainnet Launch)

---

### P3-1 — Remove `NullifierComputer` Dead Template from Circuits

**File:** `circuits/lib/nullifier_expiry.circom`, lines 8–19

`NullifierComputer` is a 3-arg nullifier template (old design) that is never instantiated by any circuit. The batch circuit uses an inline 6-arg Poseidon post-Phase-2 (ADR-0006 revised by ADR-0014; prior to Phase 2 it was 5-arg). This dead template is confusing for auditors.

**What to fix:** Remove the `NullifierComputer` template, or add a comment clearly stating it is deprecated and kept for historical reference only.

---

### P3-2 — Remove `signature_verifier.circom` Dead File

**File:** `circuits/lib/signature_verifier.circom`

This file is never `include`d by any circuit. The signature verification is done inline in `credential_atom.circom` via `EdDSAPoseidonVerifier`. The orphan file creates confusion.

**What to fix:** Remove the file, or move it to a `circuits/lib/deprecated/` folder.

---

### P3-3 — Fix `IsZero` Name Collision in `credential_atom.circom`

**File:** `circuits/lib/credential_atom.circom`, lines 83–90

The file defines its own `IsZero` template, which shadows the one from `circomlib/circuits/comparators.circom`. The local version is correct but creates a latent collision risk.

**What to fix:** Rename the local template to `LocalIsZero` or `FieldIsZero` and update all callsites in the same file. Import the circomlib version explicitly where needed.

---

### P3-4 — Remove `nullifier_bloom` Ghost Reference from `devnet.json`

**File:** `deployments/devnet.json`, lines 41–44

```json
"nullifier_bloom": {
    "seeds": ["nullifier-bloom", "<verifier_config_pubkey>"],
    "program": "FhtEvsUwxRRT8nf3uaxK6iAefFqvqCYimc9vqnH5XEgr"
}
```

The bloom filter was replaced by PDA-per-nullifier in v0.2. This PDA seed pattern no longer exists on-chain and is misleading to operators.

**What to fix:** Replace with the correct PDA entries:
```json
"nullifier_record_example": {
    "seeds": ["null", "<nullifier_bytes_hex>"],
    "program": "BZkVFdMhAEeGMvEAhXNjt3r3bEA2sCPqEFcsEbSbFGj2",
    "note": "One PDA per proof; created atomically during verify_batch_proof"
}
```

---

### P3-5 — Fix Dead Code Warning: `SasAttestationBuilder.attester`

**File:** `crates/solid-core/src/sas.rs`, line 45

The `attester` field exists in `SasAttestationBuilder` but is never read. The build output warns about this. Decide: is the attester identity supposed to be part of the commitment? If yes, wire it in. If no, remove the field.

---

### P3-6 — Add `request_withdrawal` and `withdraw_after_cooldown` to `issuer-guide.md`

**File:** `docs/issuer-guide.md`

These two instructions are implemented but not documented for issuers. An issuer who wants to gracefully exit the registry has no guide.

---

### P3-7 — Add `set_binding_status` (Emergency Freeze) to `schemas.md`

**File:** `docs/schemas.md`

The `set_binding_status` instruction on schema-registry allows the authority to freeze a `SchemaTreeBinding`, causing all proofs against that schema to fail immediately. This is a useful emergency stop that is not mentioned anywhere in user-facing documentation.

---

### P3-8 — Add VK Stack-Size Assertion to Developer Docs

**File:** `docs/architecture.md`

The `const _: () = { assert!(sz < 3072, "VkBuf exceeds safe BPF stack slice") }` compile-time assertion in `zk-verifier` is an elegant safety net that prevents BPF stack overflows from sneaking into production builds. This is worth documenting as a pattern for other Solana ZK projects.

---

## Architectural Improvements (Longer-Term)

These are not bugs — they are design-level improvements for scale and longevity.

---

### ARCH-1 — Consider Universal-Setup PLONK for Circuit Upgrades

**Timeline:** After v1.0 mainnet stabilization

Groth16 is the right choice for the MVP: smallest proof size, lowest on-chain verification cost, and the same stack used by Polygon ID and Worldcoin. However, it requires a new trusted setup ceremony for every circuit change (e.g., adding a field to `NUM_FIELDS`, increasing `NUM_CREDS`, adding the revocation check).

A universal-setup PLONK variant (Marlin, Fflonk) would allow circuit upgrades without a new ceremony, at the cost of slightly larger proofs. **Do not migrate preemptively** — only if the circuit needs to change frequently.

---

### ARCH-2 — Add Delegated Proving with Privacy Disclosure

**Timeline:** After proof generation UX becomes a bottleneck

Client-side WASM proving is slow (10–60 seconds on low-end devices). A delegated proving server (user sends private inputs, server generates proof) is the fastest UX fix but must be presented to users as a privacy tradeoff — the server learns the holder's private credential data.

Any delegated proving implementation MUST:
1. Clearly communicate to the user that the server learns their private attributes
2. Use TLS + end-to-end attestation (e.g., server runs in a TEE)
3. Be opt-in, not default

The default must always remain client-side proving for privacy guarantees.

---

### ARCH-3 — Production Indexer Integration (Helius DAS or Shyft)

**Timeline:** Before inviting any external integrators

The `MerkleProofAdapter` interface is correctly designed. The `LocalReplicaAdapter` exists for testing. What's missing is a production adapter for a real indexer.

**What to build:**
```typescript
class HeliusDasAdapter implements MerkleProofAdapter {
    constructor(private heliusApiKey: string) {}
    async fetch(treeAddress, leafCommitment): Promise<MerkleProof> {
        // Call Helius DAS getAssetProof API
    }
}
```

Publish this as part of `@solid-protocol/light` or as a separate `@solid-protocol/helius` package.

---

### ARCH-4 — Rotate Upgrade Authority to a Multisig Before Mainnet

**Timeline:** Before mainnet deployment

All three programs currently use a single deployer keypair as upgrade authority. A compromised key = permanent loss of the protocol. Before mainnet:

1. Create a 3-of-5 multisig (e.g., using Squads Protocol)
2. Transfer all three program upgrade authorities to the multisig
3. Transfer `RegistryConfig.authority` to the multisig
4. Transfer `VerifierConfig.authority` to the multisig
5. Document the multisig signers and their key management procedures

---

### ARCH-5 — Anchor IDL-Based TypeScript Client

**Timeline:** After trusted setup and artifact deployment

The current SDK uses hand-crafted instruction builders. Once `anchor build` is wired into CI and IDLs are generated, replace the manual builders with IDL-derived clients. Keep the manual builders as a fallback for environments where the IDL isn't available.

---

### ARCH-6 — W3C Verifiable Credentials Translation Layer

**Timeline:** For enterprise integration

Add a bidirectional JSON-LD adapter:
- **Inbound:** Parse a W3C VC → extract fields → populate `StoredCredential`
- **Outbound:** Take proof result → format as a W3C VP with a Solana-specific proof type

This enables direct integration with existing enterprise credential infrastructure (universities, KYC providers) without requiring them to understand Circom or BabyJubJub.

---

## Summary Checklist

Reconciled against code + `sec/SECURITY_REGISTRY.md` on 2026-04-23.
Format: `[x]` = landed in code; `[ ]` = open (cross-reference given).

```
P0 -- Fix Before Any Testing (6 items)
[x] P0-1  RegistryConfig space 80 -> 112 bytes                            (in code)
[x] P0-2  Add owner checks to global_tree and schema_tree_N                (ADR-0010; guarded by SOLID-SEC-032 build-time check)
[x] P0-3  vote_on_issuer must increment active_votes_count                 (in code)
[x] P0-4  vote_on_issuer must check voting deadline                        (in code)
[x] P0-5  Fix nullifier hex vs decimal parsing in holder SDK               (in code)
[x] P0-6  Create @solid-protocol/verifier package                          (in code: ts-sdk/packages/verifier/src/index.ts)

P1 -- Fix Before Devnet Testing (9 items)
[x] P1-1  Mark staker_account and vote_record as mut in ReleaseVote        (in code)
[x] P1-2  Add access control to IncrementUsage                             (in code)
[x] P1-3  Transfer slashed lamports to treasury in slash_issuer            (in code; rent-floor guard tracked as SOLID-SEC-030)
[x] P1-4  Emit IssuerApproved in approve_via_trust_anchor                  (in code)
[x] P1-5  Add PDA constraint to target_issuer in ApproveViaTrustAnchor     (in code)
[x] P1-6  Add VK size cap in store_verification_key                        (in code)
[~] P1-7  Fix identity commitment: use per-schema keys, not master key     (circuit + commitment path fixed; cohesion check still compares master key: SOLID-SEC-033)
[ ] P1-8  Update devnet.json with correct program IDs and toolchain        (no live deploy recorded; SOLID-SEC-027 tracks doc truth)
[ ] P1-9  Extend check_program_ids.py to validate devnet.json              (bundled with SOLID-SEC-032)

P2 -- Fix Before Mainnet (14 items)
[x] P2-1  Add numPredicates <= MAX_PREDICATES constraint to circuit        (in code)
[x] P2-2  Add compoundLogic binary constraint to circuit                   (in code)
[x] P2-3  Instantiate ExpirationChecker for each credential                (in circuit; Clock binding tracked as SOLID-SEC-005)
[x] P2-4  Fix resolveSchema memcmp offset                                  (in code)
[x] P2-5  Fix listIssuers filter byte and offset                           (in code)
[x] P2-6  Fix SchemaAccount space for field_names Vec                      (in code)
[x] P2-7  Add GLOBAL_DEPTH parameter to batch circuit                      (in code)
[ ] P2-8  Deploy governance token and replace placeholder pubkeys          (placeholders still present in config.ts:35-36)
[x] P2-9  Add chunk sequence validation to store_verification_key          (in code; VK freeze-gate tracked as SOLID-SEC-006)
[~] P2-10 Wire cross-language vector test into CI as required gate         (narrow: 3/10 primitives post-Phase-2 -- SOLID-SEC-010; expansion in Phase 3)
[~] P2-11 Implement revocation (v1 design is ready)                        (circuit + on-chain done; holder + issuer SDK + events not done)
[x] P2-12 Add issuer public key binding in ZK proof path                   (SOLID-SEC-004 FIXED 2026-04-24, Phase 2; ADR-0014)
[ ] P2-13 Run trusted setup ceremony and publish circuit artifacts         (single-party; multi-party is SOLID-SEC-012)
[x] P2-14 Implement real SolID.prove() in top-level SDK                    (in code)

P3 -- Polish (8 items)
[x] P3-1  Reconcile nullifier.rs docstring                                 (SOLID-SEC-036 FIXED 2026-04-24, Phase 2 commit 73871dc)
[ ] P3-2  Remove dead signature_verifier.circom file                       (still present; defer)
[ ] P3-3  Fix IsZero name collision in credential_atom.circom              (SOLID-SEC-022)
[ ] P3-4  Fix nullifier_bloom ghost reference in devnet.json               (bundled with P1-8)
[ ] P3-5  Resolve dead SasAttestationBuilder.attester field                (defer)
[ ] P3-6  Document withdrawal flow in issuer-guide.md                      (defer)
[ ] P3-7  Document set_binding_status in schemas.md                        (defer; SOLID-SEC-035 covers the governance side)
[ ] P3-8  Document VkBuf stack assertion pattern in architecture.md        (defer)

Architectural (6 items -- longer term)
[ ] ARCH-1 Evaluate universal-setup PLONK for circuit upgrade flexibility  (Phase 4)
[ ] ARCH-2 Implement delegated proving with proper privacy disclosures     (Phase 4)
[ ] ARCH-3 Build HeliusDasAdapter for production Merkle proof retrieval    (Phase 3)
[ ] ARCH-4 Rotate all upgrade authorities to a multisig before mainnet     (SOLID-SEC-013; Phase 3)
[ ] ARCH-5 Generate Anchor IDL and wire IDL-based TS client                (Phase 2/3)
[ ] ARCH-6 Build W3C Verifiable Credentials translation layer              (Phase 4)
```

Legend: `[x]` landed; `[~]` partially landed (scope split between
original roadmap and a more specific SOLID-SEC-NNN); `[ ]` open.

Registry count (as of 2026-04-25, post Phase 3 impl 3): 44
findings; 23 closed (Phase 1 + Phase 2 + SEC-007 + SEC-006 Part 1
+ SEC-041 landed).  Open HIGH: SOLID-SEC-010, -012.  Open MEDIUM:
SOLID-SEC-013..-019, -021, -034, -043.  Open LOW: SOLID-SEC-022,
-023, -024, -035, -044.  Open INFO: SOLID-SEC-025, -026, -037, -038.

Note: SOLID-SEC-006 Part 2 (circuit-bound `vk_generation`) is
deferred behind the next trusted-setup cycle and is tracked in
ADR-0015 "Proposed / pending" rather than re-opening the registry
entry.

New post-roadmap items added in 2026-04-25 sweep (not in the P0..P3
structure; tracked by registry ID only):

- SOLID-SEC-043 (MEDIUM, Open).  `IssuerTreeBinding.operator` is a
  single signer; gate behind Squads 3-of-5 before external audit.
- SOLID-SEC-044 (LOW, Open).  Cooldown status does not replace the
  issuer's tree leaf; add `request_withdrawal_atomic`.

See the v0.6 deep audit's Section 7.1 for the current Phase 3 close-
out priority ordering.

---

*End of roadmap. Current state lives in `sec/SECURITY_REGISTRY.md`.*
