# SolID Protocol — Master Consolidated Audit (v0.3)

> **Date:** 2026-04-22
> **Auditors:** Antigravity (deep system audit, all source files) + Senior-Engineer Audit (post-fix pass)
> **Scope:** All source files — on-chain programs, ZK circuits, TypeScript SDK, Rust crates, CI/CD,
> deployment manifests, all docs.
> **Prior art checked:** `security_audit.md` (2026-04-20), `IMPROVEMENTS_ROADMAP.md` (2026-04-21),
> `POST_REMEDIATION_AUDIT.md`, `solid_protocol_system_audit_2026_04_21.md`
>
> **NOTE FOR OTHER MACHINES:** This file is the canonical audit record as of this commit.
> Artifact files are NOT shared across machines. This file is the single source of truth.
> Do not treat any earlier `security_audit.md` or `SOLID_*` audit docs as current —
> they predate the 2026-04 remediation series and are now archival.

---

## 0. Verdict in One Paragraph

The code is better than the docs claim and the docs are more confident than the code warrants.
The post-remediation work landed: the verifier owner-checks, schema-ordering, atomic replay PDA,
chunked VK, slashing transfer, approve-via-trust-anchor, and monotonic root updates are all real in
`programs/` and match `docs/POST_REMEDIATION_AUDIT.md`. But three critical mainnet-blocking defects
survived remediation — (1) the batch circuit is under-constrained on `queryCredentialIndices` /
`queryFieldIndices` and is a complete soundness break; (2) `register_schema` has its Poseidon hash
integrity check commented out with a "simplified for this task" note; and (3) `issue_credential`
never binds the `merkle_tree` pubkey or the `schema_hash` to a registered schema. Combined, these
three let any approved issuer fabricate a universe of rogue schemas and mint proofs that satisfy
arbitrary predicates over fictitious credentials. Everything else — the VK freeze-gate, the
nullifier-per-PDA scale ceiling, the single-party trusted setup, 1/11 integration tests, the WASM
bridge that CI builds from the wrong directory, the `bufToDecimal` endianness issue — is
serious-but-fixable engineering debt. This is a credible v0.3 testnet. It is not mainnet-safe, and
the canonical backlog in `docs/IMPROVEMENTS_ROADMAP.md` lies about what is closed.

---

## 1. Executive Scorecard

| Dimension | Score | Reasoning |
|-----------|-------|-----------|
| **Testing Readiness** | 🟡 Partial | 1/11 bankrun tests; no E2E; no compiled circuit artifacts |
| **Docs vs Implementation** | 🟠 Drifted | Architecture doc describes v0.1 file tree; ROADMAP has 25+ closed items still marked `[ ]` |
| **Security Posture** | 🟠 Hardened but 3 CRITICALs remain | Major P0/P1 remediations landed; new critical soundness break found |
| **Integration Cohesion** | 🟢 Intact | Public-input contract, PDA layouts, account ownership checks all aligned |
| **Logical Correctness** | 🟠 Several bugs | Identity cohesion check wrong direction; IdentityAnchor always-enabled; bufToDecimal endianness |
| **Overall Infra Rating** | **5.3 / 10** | Strong v0.3 post-remediation — not yet real infra |

---

## 2. Critical Findings — Ranked (Block Any Deployment)

### C-1 — Circuit Soundness Break: Unconstrained `queryCredentialIndices` / `queryFieldIndices`

**Files:** `circuits/batch_credential_query.circom:56-59,216-217`,
`circuits/lib/predicate_evaluator.circom:85-112`

`queryCredentialIndices[i]` and `queryFieldIndices[i]` are public inputs with no range constraint.
`BatchFieldSelector` silently returns `0` for any out-of-range index. A malicious prover sets
`queryValue=0, op=EQ` against the default-zero output and "proves" an arbitrary predicate holds
without holding the referenced credential.

**Impact:** Complete soundness break on all batch proofs. A fake credential satisfying any
predicate can be generated without the actual data.
**Required fix:** Add `LessThan(8)` range checks on both signals in both
`batch_credential_query.circom` and `compound_query.circom`. This forces a new trusted-setup
run — there is no workaround.
**Regression gate:** Property-based test with 1000 iterations of random out-of-range indices must
fail witness generation.

---

### C-2 — Hash Integrity Check Commented Out in `register_schema`

**File:** `programs/schema-registry/src/lib.rs:114-130`

The Poseidon hash integrity check (`computed_hash == schema_hash`) is commented out with the note
"simplified for this task." Anyone can register a schema with an arbitrary `schema_hash` that
does not correspond to the declared metadata. `SEC-06` is falsely reported as closed in prior audit
docs.

**Impact:** Root enabler for C-3. Any entity can create a rogue schema hash that the verifier
will accept as canonical.
**Required fix:** Re-enable `require!(computed_hash == schema_hash, ErrorCode::InvalidSchemaHash)`.
One line. No ceremony needed.
**Regression gate:** Unit test: `register_schema` with mismatched hash returns
`ErrorCode::InvalidSchemaHash`.

---

### C-3 — `issue_credential` Does Not Bind to a Registered Schema or Tree

**File:** `programs/issuer-registry/src/lib.rs:619-716` and context at `:909-937`

`issue_credential` does not:
- Require the `schema_hash` to correspond to a registered `SchemaAccount`
- Bind `merkle_tree.key()` to `SchemaTreeBinding.tree_pubkey`
- Prevent an approved issuer from appending to *any* SPL-AC tree whose authority is
  `PDA(b"tree-authority", any-32-bytes)`

An approved issuer can spawn a rogue schema/tree universe that the on-chain verifier accepts as
canonical. Combined with C-1 and C-2 this is trivially exploitable.

**Impact:** Approved issuers can fabricate arbitrary credential trees. The trust model of the
entire registry is bypassable without the DAO ever knowing.
**Required fix:** Add `schema_account` + `schema_tree_binding` as required accounts on
`issue_credential`. Seed-constrain to `[b"schema", name, version]` under schema-registry.
Add `require!(schema_account.schema_hash == schema_hash)` and
`require!(merkle_tree.key() == binding.tree_pubkey)`.
**Regression gate:** Integration tests `06_issue_credential_rejects_unregistered_schema` and
`07_issue_credential_rejects_wrong_tree`.

---

## 3. High Findings (Close Before External Audit)

| # | Finding | Evidence |
|---|---------|----------|
| **H-1** | In-circuit issuer pubkey binding absent. Revoked issuer's prior credentials still verify; circuit takes `issuerPubKey` as private input with no on-chain cross-check. `check_issuer_status` exists but nothing CPIs into it. | `circuits/batch_credential_query.circom:81-82`, `programs/zk-verifier/src/lib.rs:148-297` |
| **H-2** | `currentTimestamp` (public input 30) is not bound against `Clock::get()`. Attacker submits `currentTimestamp=0` and passes any expiry gate. Proposed fix: bind against clock with a 10-minute (not 5-minute) skew window stored as a `VerifierConfig` constant. | `programs/zk-verifier/src/lib.rs:157-198`, `circuits/batch_credential_query.circom:65,192-198` |
| **H-3** | VK overwrite at chunk 0 has no freeze-gate; authority can prematurely set `is_final_chunk=true` with a small `nr_ic`, bricking all in-flight proofs. Fix: add `vk_generation: u16` to `VerifierConfig`, include in public inputs, implement grace window as `accept_vk_generations: [u16; 2]`. | `programs/zk-verifier/src/lib.rs:82-130` |
| **H-4** | BabyJubJub public keys not subgroup-checked on registration. Small-order / cofactor pubkey allows trivial signature forgery. `fq_to_fr` reduction correctness untested by vectors. | `crates/solid-core/src/babyjubjub.rs:111-114,125-133` |
| **H-5** | Nullifier does not include an epoch / `globalRoot`. Regression of the global binding (reorg or bug) allows a nullifier to apply across tree generations. Consider a monotonic `epoch_counter` rather than the raw root (including root would break nullifiers on legitimate root updates). | `circuits/batch_credential_query.circom:286-292`, `crates/solid-core/src/nullifier.rs:23-40` |
| **H-6** | WASM bridge is structurally broken across three files: `crates/solid-core/Cargo.toml` declares a `wasm` feature with zero `#[wasm_bindgen]` exports in `crates/solid-core/src/`; the real bridge lives in `wasm/src/lib.rs`; CI runs `wasm-pack build crates/solid-core`, producing an empty `pkg`; `ts-sdk/packages/core/package.json` pins `file:../../../wasm/pkg` which neither CI nor any script creates. Fix: keep `wasm/` standalone (option b) — `solid-core` must stay BPF-compatible. Fix CI + the pinned path. Delete the other candidate. | Three locations above |
| **H-7** | `gen_vectors` covers 2/10 primitives (commitment + nullifier). No vectors for Poseidon raw, BJJ sign/verify, `derive_credential_key`, `compute_identity_state`, query-context hash. The TS SDK reimplements query encoding in JavaScript — exactly the drift-prone surface with no vector guard. | `crates/solid-core/examples/gen_vectors.rs:55,62` |
| **H-8** | E2E scripts have three concrete bugs a stranger hits on first run: (a) `circuits/scripts/setup.js:57` writes `circuit_final.zkey` but `scripts/prove.ts:123` reads `batch_credential_query_final.zkey`; (b) `scripts/prove.ts:105` references `keccak256HashPair` which is never imported; (c) `scripts/issue.ts` lacks the `register_issuer → stake → vote → approve` sequence, so on a clean registry `issueCredential` CPI-fails. | Listed above |
| **H-9** | Trusted setup is single-party Phase-2 with entropy `'solid-entropy-' + Date.now()`. Whoever ran that script owns the toxic waste. Required: multi-party ceremony before mainnet. | `circuits/scripts/setup.js:4,47` |

---

## 4. Medium Findings

- **M-1:** Single-key authority on `verifier_config`, `slash_issuer`, `submit_fraud_proof` — no multisig, no timelock.
- **M-2:** `stake_vault` is a single shared PDA; a complete drain (all stakes slashed simultaneously) garbage-collects it and breaks all future `register_issuer` calls (see BUG-NEW-03 below for the rent edge case).
- **M-3:** `approve_via_trust_anchor` has no minimum-tier gate on targets — a trust anchor can approve a Community-tier issuer as Government-tier.
- **M-4:** `transfer_authority` is single-step (no accept/confirm pattern). A typo permanently transfers governance to an uncontrolled key.
- **M-5:** `test_rng` in `tools/solid-prover/src/lib.rs:90` uses a deterministic seed that breaks proof unlinkability in test mode if accidentally used in production.
- **M-6:** No `SchemaBindingFrozen` event emitted by `set_binding_status`; indexers cannot detect emergency freezes.
- **M-7:** `verifier_config` is written on every `verify_batch_proof` call (write lock) — caps throughput at ~100 verifies/sec. Fix: remove the mutable borrow or move the counter to a separate hot account.
- **M-8:** `scripts/issue.ts:118-125` persists issuer and holder secrets (Solana keypair + BJJ private keys) as plaintext to `scripts/e2e_state.json`, which is not in `.gitignore`. Add immediately.

---

## 5. Security Findings from Deep Code Read (Not in Prior Audits)

### NEW-SEC-01 — `WithdrawAfterCooldown` Missing Explicit Authority Constraint

**File:** `programs/issuer-registry/src/lib.rs:995-1005`

No `constraint = issuer_account.authority == issuer_authority.key()` in the struct. `WithdrawStake`
has this at line 788. PDA seeds protect against wrong-PDA, but the explicit constraint parity is
missing and creates a gap if the stored `authority` field ever diverges from the signer.
**Risk:** Low-medium. Add `@ ErrorCode::Unauthorized` constraint for parity.

---

### NEW-SEC-02 — `SCHEMA_REGISTRY_ID_BYTES` Not Validated Against `Anchor.toml`

**File:** `crates/solid-light/src/cpi_helpers.rs:54-59`

```rust
const SCHEMA_REGISTRY_ID_BYTES: [u8; 32] = [
    184, 31, 191, 183, 14, 126, 178, 219, ...
];
```

This is a manually hand-decoded base58 pubkey. If the schema-registry is ever redeployed with a
new ID, this constant silently stays wrong. The P0-2 fix (owner-check on `global_tree` and
`schema_tree_N`) is *only as strong as this constant being correct*. If it is wrong, the critical
bypass is back with no error. `check_program_ids.py` does NOT validate this constant.

**Risk:** High — silent regression vector for the most critical security check in the system.
**Fix:** Add a `#[test] fn schema_registry_id_bytes_matches_anchor_toml()` in `solid-light` that
decodes the base58 ID from an env variable or a `build.rs` constant at compile time.

---

### NEW-SEC-03 — `SubmitFraudProof` and `SlashIssuer` Have No PDA Seed Constraint on `issuer_account`

**File:** `programs/issuer-registry/src/lib.rs:799-823, 861`

Both context structs have `#[account(mut)] pub issuer_account: Account<'info, IssuerAccount>`
with no seed constraint. Any `IssuerAccount` PDA can be passed — not only the canonical
`[b"issuer", issuer.authority]` PDA. Contrast with `approve_via_trust_anchor` and
`withdraw_after_cooldown` which both add the seed constraint. Apply the same pattern here.
**Risk:** Medium — authority gate provides partial protection, but defense-in-depth is missing.

---

### NEW-SEC-04 — `G1 Negation` `borrow` Is `i16`, Not `u16`

**File:** `programs/zk-verifier/src/lib.rs` (G1 negation helper)

The BigEndian subtraction uses `i16` for `borrow`. The check `borrow != 0` after all iterations
is logically correct (any nonzero borrow = underflow = reject), but signed arithmetic means
`borrow` could be positive (overflow in the other direction). In practice this is safe because
the loop is strictly a byte-level subtraction and `borrow` is bounded to `{-1, 0}`. But it should
have a comment explaining why `i16` and why `!= 0` is the correct gate.
**Risk:** Informational only — the logic is correct.

---

### NEW-SEC-05 — `set_binding_status` Can Unfreeze Without Time-Lock

**File:** `programs/schema-registry/src/lib.rs:322-349`

`set_binding_status` accepts `status = STATUS_ACTIVE` (unfreeze) with no time-lock and no
additional authorization beyond the current authority. A compromised authority key can
freeze → wait for panic →  unfreeze, cycling the schema in and out of availability at will.
**Risk:** Low — governance design choice. Document it explicitly.

---

### NEW-SEC-06 — `bufToDecimal` Is Little-Endian; Solana Pubkeys Are Big-Endian

**File:** `ts-sdk/packages/holder/src/index.ts:391-397`

```typescript
function bufToDecimal(buf: Uint8Array): string {
  let result = 0n;
  for (let i = buf.length - 1; i >= 0; i--) {
    result = result * 256n + BigInt(buf[i]);
  }
  return result.toString();
}
```

This is correct for little-endian field elements (e.g., BabyJubJub scalars on Solana). But
`VERIFIER_ID_BYTES = new PublicKey(PROGRAM_IDS.zkVerifier).toBytes()` is big-endian (the canonical
Solana pubkey byte order). Calling `bufToDecimal(VERIFIER_ID_BYTES)` interprets the pubkey LE,
producing the wrong integer for the `verifierAddress` circuit input.

The on-chain check at `zk-verifier` compares `public_inputs[28]` against `ID.to_bytes()` (also
BE). The two wrong-endian numbers will never match.

**Risk:** High — potentially breaks all proof verifications end-to-end. Fix: use
`bigintFromBytesLE` for field elements and a dedicated `bigintFromBytesBE` for Solana pubkeys.

---

### NEW-SEC-07 — Legacy 3-Arg `NullifierComputer` vs Circuit 5-Arg Formula

**File:** `circuits/lib/nullifier_expiry.circom` vs `circuits/batch_credential_query.circom`

The batch circuit nullifier:
```
Poseidon(masterKey, revocationNonce, verifierAddress, queryContextHash, verifierNonce)
```
The dead `NullifierComputer` template:
```
Poseidon(holderPrivKey, schemaHash, verifierNonce)  // 3 inputs
```

While `NullifierComputer` is dead code (never instantiated), the `computeNullifier` WASM
function called by the holder SDK must implement the 5-input formula. If `solid-core` still
implements the old 3-input formula, every nullifier computed client-side will fail the on-chain
check `nullifier == public_inputs[0]`.

**Risk:** Potentially critical — must be verified when `solid-core` source is available. This is
the most likely remaining end-to-end failure point after fixing C-1/C-2/C-3.

---

### NEW-SEC-08 — `SchemaTreeBinding` Reader-Writer Size Asymmetry

**File:** `programs/schema-registry/src/lib.rs:37,249` vs `crates/solid-light/src/cpi_helpers.rs:169`

Writer allocates 145 bytes (includes 32-byte `authority` at [113..145]). Reader only requires
113 bytes (up through status byte at [112]). This is intentional forward-compatibility but
creates a documentation gap — the reader does not parse or validate the authority field.

**Risk:** None — informational. Worth a comment in `verify_schema_root_binding`.

---

### NEW-SEC-09 — `CredentialAtom` Redefines `IsZero`, Shadowing circomlib

**File:** `circuits/lib/credential_atom.circom:83-90`

Defines a local `IsZero` after including `poseidon.circom` (which pulls `comparators.circom`
which already defines `IsZero`). The implementations are identical today, but any circomlib
update will be silently overridden by the local shadow. Rename to `LocalIsZero` or
`FieldIsZero` and update callsites.
**Risk:** Low but real — compiler-version-dependent resolution order.

---

### NEW-SEC-10 — `GreaterThan(8)` for `orSum` Is Safe But Undocumented

**File:** `circuits/batch_credential_query.circom:247-251`

`orSum` sums up to 4 binary predicateResult values (max = 4). `GreaterThan(8)` works for values
≤255. The sum is provably ≤ 4, so this is safe. But there is no comment documenting why 8 bits
is sufficient, leaving auditors to re-derive the bound. Add a comment.
**Risk:** None — informational.

---

## 6. Logical Bugs

### BUG-NEW-01 — Identity Cohesion Check Uses Master Key, Not Per-Schema Derived Key

**File:** `ts-sdk/packages/holder/src/index.ts:250-255`

```typescript
// SEC-17: Identity Cohesion
for (const cred of sortedCredentials) {
    if (Buffer.from(cred.holderPubKeyX).compare(masterPublicKey.x) !== 0) {
        throw new Error("Identity Cohesion Failure: ...");
    }
}
```

The circuit's `CredentialAtom` uses `holderBJJPubKeyAx` = `anchors[i].credentialPubKeyAx` — the
**per-schema derived** public key, not the master key. The catch-22:
- Credentials stored with `masterPublicKey.x` → cohesion check passes → circuit commitment fails
- Credentials stored with `derivedKey.x` → cohesion check fails → proof never starts

**Fix:** Compare `cred.holderPubKeyX` against `deriveCredentialKey(masterPrivKey, cred.schemaHash).public_key_x`.
**Risk:** High — batch proving fails for all correctly-issued credentials.

---

### BUG-NEW-02 — `IdentityAnchor` Always `enabled = 1`; Zero-Schema Slots Over-Constrained

**File:** `circuits/lib/identity_anchor.circom:46-53`

```circom
globalInclusion.enabled <== 1;  // Always enabled — even for zero-schema padding slots
```

`CredentialAtom` guards empty slots with `enabled = 1 - isZero(schemaHash)`. `IdentityAnchor`
has no such guard. A padding slot (schemaHash = 0) still enforces global Merkle inclusion for
`Poseidon(BabyPbk(Poseidon(masterKey, 0)).Ax, ..., revocNonce)`. The global tree would need
zero-schema identity leaves pre-loaded, which is architecturally wrong.

**Fix:** Pass `enabled` signal from `CredentialAtom` to `IdentityAnchor` and gate
`globalInclusion.enabled <== enabled`.
**Risk:** Medium — batch circuit cannot prove with fewer credentials than `NUM_CREDS` without
pre-loading nonsensical global tree entries.

---

### BUG-NEW-03 — `transfer_slashed_lamports` Can Drain `stake_vault` to Zero, Deleting It

**File:** `programs/issuer-registry/src/lib.rs:1192-1206`

If `from_balance == slash_amount`, `stake_vault` drops to 0 lamports. A 0-lamport account not
explicitly `close`d is garbage-collected by the Solana runtime. This destroys the shared
`stake_vault` PDA, causing all future `register_issuer` SOL transfers to fail with
"account not found."

**Fix:** Before the lamport transfer, add:
```rust
let min_bal = Rent::get()?.minimum_balance(0);
require!(from_balance.saturating_sub(slash_amount) >= min_bal || slash_amount == from_balance,
    ErrorCode::StakeVaultWouldBeDestroyed);
```
If total slash is justified, explicitly `close` the vault and reinitialize.
**Risk:** Medium — edge case triggered when the last issuer's entire stake is slashed.

---

### BUG-NEW-04 — Interleaved Zero/Non-Zero Schemas Not Caught by Circuit Ordering

**File:** `circuits/batch_credential_query.circom:130-157`

The ordering check only compares `schemaHashes[i] < schemaHashes[i+1]` when `isZero[i+1].out = 0`.
Interleaved pattern `[0, hash_A, 0, hash_B]` is NOT caught by the circuit — the on-chain
verifier's ordering check (non-zero pairs only) handles it, but the circuit-level semantics are
inconsistent. Low risk in practice (the on-chain gate is correct) but should be documented.
**Risk:** Low-medium — inconsistency, not an exploitable gap.

---

## 7. What Was Fixed Since the April 20 Audit (Verified in Code)

All **6 P0 items** and all **7 P1 items** from the prior audit are verifiably fixed. Key fixes:

| ID | Fix |
|----|-----|
| P0-1 | `RegistryConfig` space 80 → 112 bytes |
| P0-2 | Owner checks on `global_tree` and `schema_tree_N` against `SCHEMA_REGISTRY_ID` |
| P0-3 | `vote_on_issuer` increments `active_votes_count` with overflow check |
| P0-4 | `vote_on_issuer` enforces voting deadline |
| P0-5 | Nullifier decimal-not-hex parsing fixed (`bigintToBytes32(BigInt(...))`) |
| P0-6 | `@solid-protocol/verifier` package fully implemented |
| P1-1 | `ReleaseVote` accounts marked `mut` |
| P1-2 | `IncrementUsage` gated behind authority check |
| P1-3 | `slash_issuer` + `submit_fraud_proof` transfer lamports via `transfer_slashed_lamports()` |
| P1-4 | `approve_via_trust_anchor` emits `IssuerApproved` event |
| P1-5 | `target_issuer` seed-constrained to `[b"issuer", target_authority]` |
| P1-6 | VK size cap enforced + chunk sequence ordering |
| P1-7 | Identity commitment uses per-schema derived key (`deriveCredentialKey()`) |
| P2-1..7,9,14 | numPredicates bound, compoundLogic binary, ExpirationChecker, resolveSchema Borsh walk, listIssuers walk, SchemaAccount space, GLOBAL_DEPTH param, chunk sequence, SolID.prove() real |

> **ROADMAP NOTE:** `docs/IMPROVEMENTS_ROADMAP.md` shows ALL 37 items as `[ ]` unchecked.
> This is wrong. ~25 of 37 are verifiably fixed. The roadmap MUST be updated before any
> external communication — it currently lies about the system's state.

---

## 8. What the Senior-Engineer Audit Added (Cross-Analysis)

The senior-engineer audit found C-1, C-2, C-3 which the deep system audit missed. This was the
most significant gap — the circuit soundness break (C-1) is the most dangerous finding in the
entire history of this codebase. Key additional findings from that audit:

- H-8a/b/c: Three concrete E2E script bugs (zkey filename mismatch, missing import, missing
  issuer-approval sequence) that block a stranger from running the system on first try.
- H-9: Single-party trusted setup with `Date.now()` entropy — the operator owns all toxic waste.
- The `e2e_state.json` secrets file not in `.gitignore` — a credentials leak vector.
- `verifier_config` write lock on every verify caps throughput at ~100/s.
- `active_issuers` counter drifts on Cooldown → Revoked path.
- 15 docs violate the plain-ASCII rule (worst: `system_architecture.md` with 250 hits).

---

## 9. Things the Prior Security Audit Got Wrong (Should Not Be Treated as Active)

The `security_audit.md` (2026-04-20) has these incorrectly classified as active vulnerabilities:
- CRITICAL-03, CRITICAL-04 (vote_on_issuer) — **FIXED**
- HIGH-02 (RegistryConfig space) — **FIXED**
- HIGH-03 (ReleaseVote not mut) — **FIXED**
- HIGH-04, HIGH-05 (owner checks) — **FIXED**
- BUG-02 (nullifier hex/decimal) — **FIXED**
- BUG-04 (identity commitment derivation) — **FIXED**
- MEDIUM-02 (SolID.prove() stub) — **FIXED**
- MEDIUM-03, MEDIUM-04 (resolveSchema, listIssuers offsets) — **FIXED**
- P0-6 (verifier package missing) — **FIXED**, full implementation exists

These docs should be archived under `docs/archive/` with a `HISTORICAL — DO NOT USE` banner.

---

## 10. Integration Cohesion Assessment

The system is **not fragmented**. The byte-level data contract is coherent:

```
Circuit public_inputs (31 total):
  [0]    nullifierHash
  [1]    globalRoot
  [2..5] merkleRoots[4]
  [6..9] schemaHashes[4]
  ...
  [28]   verifierAddress
  [29]   verifierNonce
  [30]   currentTimestamp

On-chain zk-verifier: NR_PUBLIC_INPUTS = 31  ✅ matches
schema-registry SchemaTreeBinding layout:
  [0..8)   b"schmtree"   ✅ matches cpi_helpers SCHEMA_TREE_DISCRIMINATOR
  [8..40)  schema_hash   ✅ matches verify_schema_root_binding
  [72..104) current_root ✅
  [112]    status        ✅
  [113..145) authority   ✅ (forward-compatible)
solid-light cpi_helpers: verifies [0..8],[8..40],[72..104],[112]  ✅
SDK buildVerifyBatchProofIx account order: matches VerifyBatchProof struct  ✅
```

Real integration fractures:
1. WASM bridge: three locations disagree on where `pkg/` lives (H-6)
2. `SCHEMA_REGISTRY_ID_BYTES` is hardcoded, not validated (NEW-SEC-02)
3. `bufToDecimal` endianness for pubkeys (NEW-SEC-06)
4. `IdentityAnchor` always-enabled for padding slots (BUG-NEW-02)
5. No circuit artifacts (.wasm, .zkey) — no end-to-end execution possible today

---

## 11. Infra Properties Rating

| Dimension | Score | Driver |
|-----------|-------|--------|
| Latency | 6.5/10 | ~280-320K CU on-chain fits in one tx; 15-30s client-side prove is the product floor; 8 serial Merkle fetches gate proof start |
| Scalability | 5.5/10 | Depth-20 circuit → ~250K holders protocol-wide; nullifier PDA-per-proof → ~$140K/month rent at 1M proofs/month; `verifier_config` write lock → ~100 verifies/sec ceiling |
| Decentralization | 5/10 | Single-party trusted setup; single-key VK authority; single-key slashing. DAO voting for issuer approval is the only genuinely decentralized primitive. C-2 means SEC-06 is NOT closed. |
| Fault Tolerance | 6/10 | Atomic nullifier replay, monotonic root gates, ResilientConnection RPC failover all solid. No VK versioning (upload overwrites, in-flight proofs brick). No holder recovery beyond encrypted export/import. |
| Distribution | 4.5/10 | Issuer tiers + `approve_via_trust_anchor` federation primitive; revocation v1 end-to-end still pending; no cross-chain; no W3C. |
| Next-gen | 4/10 | Static Groth16 circa 2023. No folding/Nova, no aggregation, no attested TLS, no mobile, no biometric PoP. |
| **Overall** | **5.3/10** | Strong v0.3 post-remediation — not yet real infra |

---

## 12. Phased Plan — No Workarounds, No Regressions

### Phase 1 — Unbrick (4-6 weeks)

Goal: close the three CRITICALs and all E2E-breaking bugs before anything else.

| Item | Root-Cause Fix (No Shortcut) | Regression Gate |
|------|------------------------------|-----------------|
| C-1 | Add `LessThan(8)` range checks on `queryCredentialIndices[i]` and `queryFieldIndices[i]` in both circuits. Force a new trusted setup. | Property-based test: 1000 iterations of out-of-range indices fail witness generation |
| C-2 | Re-enable `require!(computed_hash == schema_hash, ErrorCode::InvalidSchemaHash)` | Unit test: mismatched hash returns `InvalidSchemaHash` |
| C-3 | Add `schema_account` + `schema_tree_binding` as required accounts on `issue_credential`; seed-constrain; add two `require!` checks | Integration tests `06_issue_credential_rejects_unregistered_schema` + `07_issue_credential_rejects_wrong_tree` |
| H-2 | Bind `public_inputs[30]` to `Clock::get()?.unix_timestamp` with 10-minute skew window stored in `VerifierConfig` (not hardcoded) | Integration test `10_verify_expired_credential_rejected` |
| H-8a/b/c | Fix setup.js zkey filename; import `keccak256HashPair` or switch to `poseidonHashPair` consistently; add full `register_issuer → stake → vote → approve` sequence to `scripts/issue.ts` | CI job `e2e_localnet` on `solana-test-validator` |
| H-6 | Keep `wasm/` standalone; fix CI to build from `wasm/` not `crates/solid-core/`; fix `ts-sdk/packages/core/package.json` path to `file:../../../wasm/pkg` pointing to what CI actually produces | `cross_language_vectors` CI step imports from SDK's actual path |
| NEW-SEC-06 | Use separate `bigintFromBytesBE` for Solana pubkeys in `bufToDecimal` / circuit input assembly | Add to cross-language vector suite |
| BUG-NEW-01 | Fix identity cohesion check: compare against per-schema derived pubkey | Holder SDK test: cohesion check passes for credential issued to derived key |
| BUG-NEW-02 | Pass `enabled` from `CredentialAtom` into `IdentityAnchor`; gate `globalInclusion.enabled` | Circuit test: padding slot does not require global proof |
| Secrets | Add `scripts/e2e_state.json` to `.gitignore` | CI check: `git check-ignore scripts/e2e_state.json` exits 0 |
| Doc truth | Flip all ~25 closed checkboxes in `IMPROVEMENTS_ROADMAP.md`; move stale audit docs to `docs/archive/` with `HISTORICAL — DO NOT USE` banners | `scripts/check_docs.py` fails on any `[ ]` whose item is marked closed in `POST_REMEDIATION_AUDIT` |

**No-workaround rule:** Every fix ships a root-cause diff + a regression test. C-1 requires a
new `.zkey` — there is no workaround. C-2 is re-enabling one commented line. C-3 requires adding
accounts and constraints. No "we will validate off-chain" notes. No `#[cfg(feature = "unchecked")]`
escape hatches.

### Phase 2 — Close the Soundness Loop (8-10 weeks after Phase 1)

1. **In-circuit issuer binding (H-1).** Use compressed-issuer-tree option over CPI — cleaner, no CU growth, composes with existing tree infrastructure. New circuit + new trusted setup. Time with Phase 3 MPC ceremony — only one ceremony for the combined change.
2. **VK versioning (H-3).** Add `vk_generation: u16` to `VerifierConfig`. Include in public inputs. Implement `accept_vk_generations: [u16; 2]` grace window.
3. **BJJ subgroup checks + vector expansion (H-4, H-7).** Add `pubkey_to_affine` subgroup rejection for orders 1/2/4/8 via `mul_by_cofactor + !is_zero`. Extend `gen_vectors.rs` to all 10 primitives with TS consumers.
4. **Nullifier epoch (H-5).** Include monotonic `epoch_counter` (not raw root) in nullifier preimage. Bundle into the Phase 2 circuit revision.
5. **Remaining 9 integration tests.** Every row in `tests/integration/README.md` becomes a real bankrun test.
6. **Schema-registry events.** Emit `SchemaRegistered`, `TreeBindingCreated`, `TreeRootUpdated`, `SchemaBindingFrozen`, `AuthorityRotated`. Every mutable state transition in every program becomes indexer-visible.
7. **Revocation v1 end-to-end.** Holder SDK detects stale `revocationNonce`, rebuilds, re-proves. Issuer SDK exposes `bumpRevocationNonce(credentialId)`.

### Phase 3 — Production Infrastructure (12-16 weeks after Phase 2)

1. Multi-party trusted setup (min 10 contributors, published attestation chain, zkey on IPFS+Arweave).
2. Governance + multisig: `VerifierConfig.authority` → 3-of-5 Squads with 48h timelock; `RegistryConfig.authority` → Squads; `slash_issuer` → on-chain proposal with 24h challenge window.
3. Third-party external audit (OtterSec, Halborn, or Trail of Bits). Publish the report.
4. Scale escape: compressed nullifier Merkle tree OR epoch-bucketed Bloom filters with retry semantics. Rent math on PDA-per-nullifier does not work past ~10^5/month.
5. Tree depth: ship a depth-24 circuit revision (16x capacity). Avoid silent cliff at ~250K holders.
6. Artifact versioning: .zkey, .wasm, .so, IDL all on IPFS+Arweave. Content-addressed. Checksums pinned in `deployments/mainnet.json`.
7. Monitoring + incident response: Prometheus exporter, on-call runbook for verifier rejections, issuer over-issuance, VK upload race.
8. Secrets hygiene: `scripts/e2e_state.json` + `~/.solid-protocol/` in `.gitignore`. Rewrite E2E scripts to use ephemeral `/tmp` or encrypted local state. Document HSM/KMS path for enterprise issuers.

### Phase 4 — Next-Gen (6-12 months, Strategic)

Rank-ordered by leverage:

1. Recursive aggregation (Nova or Honk-style folding on BN254) — only honest path to 1000 verifies/sec.
2. Native mobile prover via `uniffi-rs` bindings on `tools/solid-prover`. Sub-10s mobile prove is non-negotiable for consumer identity.
3. Attested TLS (zkPass/TLSNotary-style credential minting from any HTTPS response). Biggest single-feature unlock of the next 24 months; SolID has no story here today.
4. Cross-chain anchoring: publish global-state root to Ethereum/Sui via Wormhole or LayerZero.
5. Threshold-issuer signing (FROST on BabyJubJub). Removes single-issuer-key compromise vector.
6. Biometric PoP integration (Humanity Protocol / VeryAI). Sybil-resistance is the missing pillar.
7. Per-credential revocation via SMT non-membership (REVOCATION_DESIGN v1.1).
8. DA-snapshot publication (Celestia/EigenDA) for censorship-resistant state reconstruction.
9. Formal verification of `verify_batch_proof` public-input index scheme with Kani/Certora.
10. Confidential-compute issuer enclaves (Intel TDX / AWS Nitro) for 24/7 enterprise issuance.

---

## 13. Invariants Verified as HOLDING (18 total — credit where due)

- Program-ID agreement: `Anchor.toml` ↔ `declare_id!` ↔ `deployments/devnet.json` ✅
- Verifier scope binding SEC-13: `public_inputs[28] == ID.to_bytes()` ✅
- Atomic nullifier replay via `init` on `["null", bytes]` ✅
- Owner checks on `global_tree` + `schema_tree_N` against `SCHEMA_REGISTRY_ID` ✅
- Canonical schema ascending ordering (circuit + on-chain) ✅
- `VerifierConfig::SPACE = 45` matches struct fields ✅
- VK parser is memory-safe (stack-owned `VkBuf`, no `Box::leak`) ✅
- `VkBuf` stack-bound compile-time asserted < 3072 bytes ✅
- Circuit public-input count 31 matches Rust constant and circuit signal count ✅
- Monotonic `update_tree_root` / `update_global_root` with `RootSlotNotMonotonic` ✅
- Vote flash-loan defense at 100-slot window ✅
- Identity leaf `Poseidon(Ax, Ay, revocationNonce)` Merkle-verified against `globalRoot` ✅
- EdDSA-Poseidon Rust round-trip sign/verify ✅
- Poseidon determinism ✅
- Schema-registered / binding-monotonicity gates ✅
- Lamport transfer on slash to `dao_treasury` ✅
- `approve_via_trust_anchor` emits `IssuerApproved` + seed-constrained ✅
- `ReleaseVote` accounts marked `mut` ✅

---

## 14. Done / Left / Open Summary

### Done (Verified in Code)
- All P0/P1 fixes from `POST_REMEDIATION_AUDIT.md` §1
- `@solid-protocol/verifier` package with `buildVerifyBatchProofIx`, `verifyOnChain`, PDA helpers, `checkIssuerStatus`
- Program IDs canonical across all three sources
- 58 Rust unit tests passing for primitives + VK parser
- Cross-language vector gate in CI (narrow but real)
- `SolID.prove()` and `SolID.verifyOnChain()` delegate to real implementations
- `resolveSchema` and `listIssuers` walk Borsh length prefixes correctly

### Left (Shipped Incomplete)
- Revocation v1: circuit ✅, on-chain ✅, holder SDK helper ✗, issuer SDK helper ✗, `RevocationEvent` emission ✗
- Integration tests: 1/11 implemented
- `config.ts:35-36` still has `SoLid1111...` / `SoLidToken1111...` placeholders
- No `SchemaBindingFrozen`, `SchemaRegistered`, `TreeBindingCreated`, `TreeRootUpdated`, `AuthorityRotated` events in schema-registry
- No indexer / `HeliusDasAdapter` implementation

### Open (Not Started)
- In-circuit issuer pubkey binding (design choice still open)
- Multi-party trusted setup ceremony
- External audit
- Native mobile prover
- W3C VC translation layer
- Proof aggregation / recursive SNARKs
- Cross-chain anchoring

---

## 15. Can a Stranger Run This Today?

**Localnet:** No. Three concrete H-8 bugs gate a fresh run. Once fixed:
`nix develop` → bootstrap → `cargo test` → `anchor build` → `node scripts/setup.js` [fix zkey name] → `wasm-pack build wasm/` → `npm ci && npm run build` → `solana-test-validator` → `anchor deploy` → `ts-node scripts/initialize.ts` → [add issuer approval flow] → `ts-node scripts/issue.ts` → [fix keccak256HashPair] → `ts-node scripts/prove.ts`.

**Devnet:** Template only. `deployments/devnet.json` has `deployed_at: null`, `deployer.address: null`, all `upgrade_authority: null`. No live deploy has been recorded with the canonical IDs.

**Mainnet:** Gated on C-1 + C-2 + C-3 fixes + trusted setup + multisig. Minimum 10+ items in Phase 1 + 2.

---

## 16. Discipline (Engineering Rules for Whoever Works on This Next)

1. **No workarounds.** C-1 is fixed by a new trusted setup, not a doc note. C-2 is one uncommented line. C-3 is adding accounts and constraints. Every shortcut here is soundness-visible.
2. **No regressions.** Every Phase-1 fix adds a regression test. CI must gate every invariant in `CLAUDE.md` — today only two of five are gated. Phases only close when the test stays green on `main` for a full sprint.
3. **No doc lies.** Either the doc matches the code or it moves to `docs/archive/` with a banner. `IMPROVEMENTS_ROADMAP.md` must reflect reality. Two canonical docs cannot disagree.
4. **One source of truth per artifact.** WASM bridge, program IDs, VK, circuit build outputs — each has exactly one canonical location referenced by everything else.
5. **External audit is not a rubber stamp.** If the external audit surfaces anything equivalent to C-1/C-2/C-3, the mainnet date slips. Schedule mainnet against audit-close + one sprint, not against audit completion.

---

*End of audit.*
*Total findings: 3 Criticals, 9 Highs, 8 Mediums, 10 Security issues (deep read), 4 Logical bugs.*
*Verified fixed: ~25 of 37 prior roadmap items.*
*Verified holding: 18 invariants.*
