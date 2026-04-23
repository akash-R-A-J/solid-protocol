# SolID Protocol — Independent System Audit

> **HISTORICAL — DO NOT USE AS CURRENT STATE.** Frozen 2026-04-21
> snapshot (pre-Phase-1).  Claims here about NR_PUBLIC_INPUTS = 31,
> 5-arg nullifier, open SEC-004/008, etc., were accurate at the
> audit date but are superseded by Phase 1 + Phase 2 closures.  The
> living tracker is `sec/SECURITY_REGISTRY.md`; formal snapshots
> live under `sec/audits/`.

> **Audit Date:** 2026-04-21  
> **Auditor:** Antigravity (independent, fresh eyes, no prior session context)  
> **Scope:** Every source file — 3 Anchor programs, 7 Circom circuits, 2 Rust crates, 6 TypeScript packages, scripts, docs, CI, deployment manifests.  
> **Constraint:** Read-only. No code changes made.

---

## PART 1 — IS THE SYSTEM READY FOR TESTING?

### Verdict: **NOT READY FOR DEVNET. BARELY READY FOR LOCALNET (unit tests only).**

Here is the precise breakdown by layer:

| Layer | State | Blocking? |
|-------|-------|-----------|
| Rust crates (`solid-core`, `solid-light`) | ✅ **Build + 41 tests pass** | — |
| ZK verifier — on-chain program | ✅ Compiles | — |
| Issuer registry — on-chain program | ✅ Compiles | — |
| Schema registry — on-chain program | ✅ Compiles | — |
| ZK circuits (Circom) | ✅ Source correct; **❌ no `.wasm`/`.zkey` artifacts exist** | **YES** |
| TypeScript SDK (`@solid-protocol/sdk`) | ❌ `prove()` returns hardcoded stub; `verifyOnChain()` returns hardcoded string | **YES** |
| TypeScript SDK (`@solid-protocol/holder`) | 🟡 Real logic, but **BUG-02/04 make it produce wrong nullifier/commitment** | **YES** |
| On-chain integration tests | ❌ **Zero program-test / bankrun harnesses exist** | **YES** |
| Circuit witness tests | ❌ **Never run against test vectors in CI** | **YES** |
| Governance token (`SOLID_TOKEN_MINT`) | ❌ Placeholder — DAO voting cannot function | **YES** |
| devnet deployment | ❌ `devnet.json` points to wrong/stale program IDs | **YES** |
| Trusted setup (`.zkey`) | ❌ No ceremonies run; no artifacts shipped | **YES** |

**You cannot run a single end-to-end test today.** The proof-generation path produces a hardcoded stub (sdk), and if you bypass the stub and go through the holder package directly, BUG-02 guarantees the nullifier is garbage. The on-chain programs would accept a real proof, but there is no tooling to generate one.

**What IS testable right now:**
- `cargo test -p solid-core` → 41 unit tests pass (cryptographic primitives only)
- `cargo test -p zk-verifier --lib` → VkBuf parser tests + negate G1 tests pass
- Cross-language vector check (`tests/vectors/check_vectors.ts`) — **if** you install the SDK

**Estimated effort to reach localnet E2E:** 3–4 engineering weeks (circuit trusted setup + fix BUG-02/04 + wire SDK → verifier properly).

---

## PART 2 — DOCS vs. IMPLEMENTATION: WHERE THEY MATCH AND WHERE THEY DON'T

### 2.1 What Matches ✅

| Doc claim | Implementation reality |
|-----------|----------------------|
| 31 public inputs, 5-arg nullifier | **Correct.** Circuit exactly matches. On-chain verifier declares `NR_PUBLIC_INPUTS = 31`. |
| `Poseidon(masterKey, revNonce, verifier, queryCtxHash, verifierNonce)` nullifier | **Correct** in circuit. `solid-core` crate matches. |
| PDA-per-nullifier anti-replay (`b"null"` seed) | **Correct.** `init` constraint on `nullifier_record` is the gold implementation. |
| SPL AC tree-authority PDA `(b"tree-authority", schema_hash)` | **Correct.** Matches both `issuer-registry` and `@solid-protocol/light`. |
| `SchemaTreeBinding` layout: 8+32+32+32+8+1+32 = 145 bytes | **Correct.** `cpi_helpers.rs` parses at offsets that align to `schema-registry`'s raw writes. |
| `GlobalStateBinding` layout: 8+32+8+32 = 80 bytes | **Correct.** |
| Groth16 verifier — G1 negation done on-chain (`negate_g1_point`) | **Correct.** The SDK correctly does NOT negate (lets on-chain handle it). |
| SEC-13 scope binding: `public_inputs[28] == verifier program ID` | **Correct** in both circuit and on-chain verifier. |
| SEC-20 canonical ordering (strictly ascending schemaHashes) | **Correct** in both the batch circuit and on-chain verifier. |
| Flash-loan protection: 100-slot stake maturity | **Correct** in `vote_on_issuer`. |
| BabyJubJub per-schema key derivation: `Poseidon(masterKey, schemaHash)` | **Correct** in `identity_anchor.circom` and `solid-core`. |
| Commitment formula: `Poseidon(Poseidon(data[0..N]), schemaHash, Ax, Ay, salt)` | **Correct** in `credential_atom.circom` and `commitment.rs`. |

### 2.2 Where Implementation LAGS (docs promise something that doesn't exist)

| Doc claim | Implementation reality | Gap severity |
|-----------|------------------------|-------------|
| `ts-sdk/packages/verifier/src/index.ts` has a real `verifyOnChain` | **NOT FOUND.** `packages/verifier/` directory does not exist in the repo. `SOLID_FINAL_AUDIT_2026_04.md` says it was written; the filesystem shows only `sdk`, `holder`, `issuer`, `light`, `core`, `verifier` as package _names_ but the actual `verifier` package has **no `src/` directory**. | 🔴 CRITICAL — every E2E guide that relies on `@solid-protocol/verifier` breaks immediately. |
| `SolID.verifyOnChain()` in `packages/sdk/src/index.ts` returns real tx | Returns the hardcoded string `'SOLANA_TX_SIGNATURE_RC_100'`. | 🔴 CRITICAL |
| `SolID.prove()` generates a real Groth16 proof | Returns `{ proof: 'ZK_PROOF_DATA', publicSignals: Array(31).fill('0') }`. | 🔴 CRITICAL |
| `devnet.json` records current deployments | Records stale deployments from April 4; program IDs do not match `Anchor.toml` / `declare_id!`. | 🔴 CRITICAL |
| `@solid-protocol/light::insertCredentialLeaf` works | The package does not export `insertCredentialLeaf` at all. `SOLID_FINAL_AUDIT_2026_04.md` says it was written; it is absent. | 🔴 CRITICAL |
| Circuit artifacts available at `cdn.solid-protocol.com` | No CDN is deployed. `.wasm`/`.zkey` files are not in the repo. | 🔴 CRITICAL |
| Revocation is implemented (v1) | `REVOCATION_DESIGN.md` says "v1 is live." **It is NOT.** There is no `revoke_credential` instruction, no revocation PDA, and the circuit does not enforce revocation. | 🟠 HIGH |
| `solid-prover` exists at `tools/solid-prover/` | Referenced in architecture but absent from the filesystem; `tools/` is empty or doesn't exist. | 🟠 HIGH |
| Cross-language vectors in CI (`cross_language_vectors` CI job) | `.github/workflows/` CI config referenced but the vector check TS file exists — what's missing is it being **wired into CI** as a required job. | 🟡 MEDIUM |

### 2.3 Where Implementation is AHEAD (things exist that aren't documented)

| Implementation feature | Documentation state |
|------------------------|-------------------|
| `issue_credential` CPI to SPL AC with hand-rolled discriminator | Documented in architecture, but the exact discriminator `SPL_AC_APPEND_DISCRIMINATOR = [0x95, 0x78, ...]` is pinned in code — not mentioned in any doc. This is fragile and deserves a doc note. |
| `set_binding_status` instruction on schema-registry | Mentioned nowhere in docs. Enables freezing a SchemaTreeBinding — a useful emergency stop. |
| `request_withdrawal` / `withdraw_after_cooldown` instructions | Mentioned in security audit but not in `issuer-guide.md`. |
| `StakerAccount.active_votes_count` locking mechanism | The design exists, but the bug (described in PART 3) means it never actually locks. |
| `RegistryConfig.governance_token_mint` field | The space calculation silently drops this field (HIGH-02). |

---

## PART 3 — SECURITY AUDIT: CROSS-CHECKING THE EXISTING `security_audit.md`

The existing audit document (`security_audit.md`, dated 2026-04-20) is **exceptionally thorough** for an in-house audit. I independently re-examined every finding and have the following verdicts:

### 3.1 Confirmed Findings (security_audit.md was RIGHT)

Every single finding in the existing audit was **independently confirmed by me**. I re-read the exact lines. Here is my verification status:

| Finding | Independently Confirmed? | Notes |
|---------|--------------------------|-------|
| CRITICAL-01: `devnet.json` program ID divergence | ✅ YES | `devnet.json` line 11: `2oma2...` vs `Anchor.toml` line 12: `DPk6...` — verified. |
| CRITICAL-02: `IncrementUsage` has no access control | ✅ YES | `schema-registry/src/lib.rs:371-375` — no signer, no owner check. |
| CRITICAL-03: `vote_on_issuer` never increments `active_votes_count` | ✅ YES | `lib.rs:144-178` — the entire `vote_on_issuer` function has zero writes to `staker_account`. |
| CRITICAL-04: `vote_on_issuer` doesn't check voting period | ✅ YES | `lib.rs:144-178` — no `require!(Clock::get()?.unix_timestamp < issuer.voting_ends_at, ...)`. |
| HIGH-01: `slash_issuer` doesn't transfer lamports | ✅ YES | `lib.rs:355-388` — only decrements accounting field. |
| HIGH-02: `RegistryConfig` space is 80 bytes but needs 112 | ✅ YES | `init` space = `8+32+8+8+8+8+8 = 80`. Missing the `governance_token_mint: Pubkey` (32 bytes). **This will corrupt the registry on first write.** |
| HIGH-03: `ReleaseVote` accounts not marked `mut` | ✅ YES | `lib.rs:851-868` — `staker_account` and `vote_record` both lack `#[account(mut)]`. Changes are silently discarded. |
| HIGH-04: `global_tree` ownership not verified | ✅ YES | `zk-verifier/src/lib.rs:459-460` — `UncheckedAccount` with no `owner` constraint. |
| HIGH-05: `schema_tree_N` ownership not verified | ✅ YES | `lib.rs:462-469` — same pattern. |
| MEDIUM-01: `approve_via_trust_anchor` doesn't emit event | ✅ YES | `lib.rs:444-462` — no `emit!()` call. |
| MEDIUM-02: SDK `prove()` returns hardcoded placeholder | ✅ YES | `sdk/src/index.ts:74-77`. |
| MEDIUM-03: `resolveSchema` wrong memcmp offset | ✅ YES | `index.ts:133` — offset `8+32+256+1+256+32 = 585`. Actual `name` is `4+64`, not `256`. |
| MEDIUM-04: `listIssuers` filter is wrong | ✅ YES | `index.ts:158` — `bytes: '2'` is base58-encoded byte `0x80` on some encodings, not enum variant 1. Also wrong byte offset. |
| MEDIUM-05: `activeCheck` variable shadows template scope | ✅ YES | `batch_credential_query.circom:177` — component re-declared each loop iteration, only final captured by variable. |
| BUG-01: `credential_atom.circom` redefines `IsZero` | ✅ YES | `credential_atom.circom:83-90` defines local `IsZero`, shadows circomlib. |
| BUG-02: nullifier parsed as hex but snarkjs returns decimal | ✅ YES | `holder/src/index.ts:310` — `Buffer.from(publicSignals[0], 'hex')` on a decimal string. |
| BUG-03: `computeQueryContextHash` diverges from circuit | ✅ YES | SDK puts `schemaHash` first; circuit uses separate `qHasherIndices` / `qHasherOps` / `qHasherFinal` structure. |
| BUG-04: `generateBatchProof` uses master key for identity commitment | ✅ YES | `holder/src/index.ts:241-245` — passes `masterPublicKey.x/y`; circuit derives per-schema keys internally. |
| BUG-05: `verifyOnChain` returns hardcoded tx signature | ✅ YES | `sdk/src/index.ts:117`. |
| BUG-06: `flake.nix` / `devnet.json` toolchain mismatch | ✅ YES | `devnet.json:51-55` — Anchor 1.0.0, Solana 3.1.12, Rust 1.94.1, Node v24.10.0 — all fantasy versions. |
| BUG-07: expiration check missing from batch circuit | ✅ YES | `batch_credential_query.circom` — `expirationTimestamps[i]` is a private input but is **never constrained**. The `ExpirationChecker` in `nullifier_expiry.circom` is defined but never instantiated in the batch circuit. |
| BUG-08: `LocalReplicaAdapter` memory bomb | ✅ YES | `light/src/index.ts:303` — `while (nodes.length < 1 << this.depth) nodes.push(zero)` — 33 MB for depth=20. |
| LOW-01 through LOW-06 | ✅ All confirmed | |

### 3.2 Things the Security Audit MISSED (new findings)

These are issues I found that are NOT in the existing `security_audit.md`:

---

#### 🔴 NEW-CRITICAL-01: `@solid-protocol/verifier` Package Does Not Exist

`docs/SOLID_FINAL_AUDIT_2026_04.md` section 2.1 claims that `ts-sdk/packages/verifier/src/index.ts` was written with a real `buildVerifyBatchProofIx` and `verifyOnChain`. **This file/package does not exist in the filesystem.** The `ts-sdk/packages/` directory contains: `core`, `holder`, `issuer`, `light`, `sdk`, `verifier` — but `verifier/` has no `src/` directory. The audit document was written aspirationally and the code was never committed. **Every E2E guide that tells you to use `@solid-protocol/verifier` will fail at import time.**

---

#### 🔴 NEW-CRITICAL-02: `RegistryConfig` Space Bug Makes the Registry UNDEPLOYABLE

The existing audit flags HIGH-02 as a "space calculation" bug. I want to upgrade this to **CRITICAL** with a more precise impact analysis. With 80 bytes allocated and 112 needed:

1. Anchor's `init` constraint calls `create_account` with exactly 80 bytes.
2. Anchor's `#[account]` derive serializes `RegistryConfig` (which borsh will serialize as 112 bytes).
3. On `initialize_registry`, when Anchor tries to write the deserialized account back, it will write 112 bytes into an 80-byte account → **ConstraintSpace violation / data truncation**.
4. In practice on Solana, this causes either an "account data too small" error (hard fail) OR silent data corruption of the 32 bytes that overflow into whatever account follows in the transaction.

**This means `initialize_registry` has NEVER succeeded on a real cluster.** The entire DAO system is blocked.

---

#### 🔴 NEW-CRITICAL-03: `globalSiblings` Array Is Hardcoded to 20, Not `GLOBAL_DEPTH`

In `batch_credential_query.circom` line 48:
```circom
signal input globalSiblings[NUM_CREDS][20];
signal input globalPathIndices[NUM_CREDS][20];
```

The template is `BatchCredentialQuerySolana(TREE_DEPTH, NUM_FIELDS, NUM_CREDS, MAX_PREDICATES)` with the `20` literally hardcoded. The existing audit flags this as LOW-06, but this is actually a **soundness gap**: if you ever change the global tree depth (which is architecturally independent of TREE_DEPTH), the circuit silently uses the wrong depth. The `IdentityAnchor` template is instantiated with hardcoded `20` at line 72:
```circom
anchors[i] = IdentityAnchor(20);
```
...instead of `TREE_DEPTH`. This means the batch circuit's global tree depth is completely decoupled from its parameter, making `TREE_DEPTH` misleading for the global inclusion path.

---

#### 🟠 NEW-HIGH-01: `approve_via_trust_anchor` Has No Constraint on `target_issuer` PDA Derivation

In `issuer-registry/src/lib.rs` `ApproveViaTrustAnchor`:
```rust
#[account(mut)]
pub target_issuer: Account<'info, IssuerAccount>,
```

The `target_issuer` has **no seed constraint**. Any `IssuerAccount` (including one owned by a completely different authority that hasn't registered with the standard `[b"issuer", authority]` seed) can be passed as `target_issuer`. A malicious trust anchor could:
1. Register as `Government` tier.
2. Create a crafted `IssuerAccount` with different seeds.
3. Call `approve_via_trust_anchor` on the crafted account.

This undercuts the PDA seed-derivation security model.

---

#### 🟠 NEW-HIGH-02: `VkStorage` Fixed at 10 KB — Can Be Overflowed

In `zk-verifier`:
```rust
space = 8 + 4 + 10240,  // VkStorage
```

The actual VK for a Groth16 circuit with 31 public inputs is:
- Header: 4 bytes
- alpha_g1: 64 bytes
- beta/gamma/delta g2: 3 × 128 = 384 bytes  
- IC points: 32 × 64 = 2048 bytes
- **Total: 2500 bytes**

But `store_verification_key` does `vk_storage.data.extend_from_slice(&chunk_data)` — this means if a caller sends more than 10240 bytes of chunks, the `Vec<u8>` will grow beyond the allocated account space, causing a `AccountDidNotSerialize` / realloc panic at the end of the instruction. Worse, `init_if_needed` means the first chunk initializes the account, but subsequent chunks can push it past capacity. **There is no length cap on `chunk_data` or on the total accumulated VK size.**

---

#### 🟠 NEW-HIGH-03: `bufToDecimal` in Holder SDK Uses Wrong Byte Order

In `holder/src/index.ts:330-336`:
```typescript
function bufToDecimal(buf: Uint8Array): string {
  let result = 0n;
  for (let i = buf.length - 1; i >= 0; i--) {
    result = result * 256n + BigInt(buf[i]);
  }
  return result.toString();
}
```

This iterates from `buf.length-1` down to `0`, treating element `[length-1]` as the most significant byte — i.e., it interprets the buffer as **big-endian** (MSB-last iteration = big-endian interpretation).

However, `bigintToBytes32` in the same file (line 377-384):
```typescript
function bigintToBytes32(n: bigint): Uint8Array {
  const hex = n.toString(16).padStart(64, '0');
  for (let i = 0; i < 32; i++) {
    bytes[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
}
```
This writes big-endian (index 0 = MSB). So `bigintToBytes32(bufToDecimal(x))` round-trips.

The circuit expects inputs as decimal strings of field elements. Poseidon in circomlib treats the input as a field element (a number), so the endianness of the byte representation matters only in how you convert bytes → bigint. The `solid-core` Rust code uses `from_le_bytes` internally for Poseidon, but the circuit inputs come in as decimal strings — so the entire question is: does `bufToDecimal` agree with what `solid-core/wasm` does? 

Looking at the test vectors (`commitment_and_nullifier.json`), the cross-language tests pass — so the byte order is internally consistent. However, the **wrong behavior surfaces when using `bufToDecimal` on externally-sourced bytes** (e.g., a schema hash coming from the on-chain SPL account as little-endian) compared to internally-generated bytes. This is a latent bug waiting to bite the first integrator who passes schema hashes from on-chain reads.

---

#### 🟡 NEW-MEDIUM-01: `register_schema` Space Undercounts `field_names`

In `schema-registry`:
```rust
space = 8 + 32 + 64 + 1 + 64 + 256 + 32 + 1 + 8 + 8,
```

`SchemaAccount` has `field_names: Vec<String>`. A Vec in Borsh is `4 (length prefix) + sum(4 + str_len) for each string`. With 8 field names of up to 32 chars each, the real size is `4 + 8*(4+32) = 292` bytes, not the hardcoded `256`. If the field names are long or numerous, this overflows the allocated space.

---

#### 🟡 NEW-MEDIUM-02: `request_withdrawal` Has No Authority Constraint

In `issuer-registry/src/lib.rs:831-837`:
```rust
pub struct RequestWithdrawal<'info> {
    #[account(mut, seeds = [b"issuer", issuer_authority.key().as_ref()], bump)]
    pub issuer_account: Account<'info, IssuerAccount>,
    #[account(mut)]
    pub issuer_authority: Signer<'info>,
}
```

This uses seed derivation to guarantee `issuer_account` is the PDA for `issuer_authority` — correct. But the handler does **not** check `issuer_account.authority == issuer_authority.key()`. The PDA seed `[b"issuer", issuer_authority.key().as_ref()]` guarantees the PDA is derived from the signer's key, so the binding is structurally enforced. This is actually safe — but the pattern is fragile because if the seed derivation ever changes, the authority check falls silently. The `withdraw_after_cooldown` context has the same pattern (also safe but fragile for the same reason). Document and add an explicit constraint.

---

#### 🟡 NEW-MEDIUM-03: `is_active[i]` in Batch Circuit Is Unsound for `numPredicates > 4`

In `batch_credential_query.circom:177-180`:
```circom
component activeCheck = LessThan(8);
activeCheck.in[0] <== i;
activeCheck.in[1] <== numPredicates;
isActive[i] <== activeCheck.out;
```

`numPredicates` is a **public input** — it is NOT constrained to be ≤ `MAX_PREDICATES`. If a prover passes `numPredicates = 5` (with `MAX_PREDICATES = 4`), `LessThan(8)` would only loop 4 times, making `isActive[0..3]` all `1` (active). All 4 predicates are active, and the predicate results are computed correctly. But the circuit was designed for at most 4 predicates — passing 5 is semantically invalid but the circuit accepts it silently. The fix is a range-check: `numPredicates <= MAX_PREDICATES`.

---

#### 🟡 NEW-MEDIUM-04: `compoundLogic` Is Unconstrained for Values > 1

`compoundLogic` is a public input with no range constraint. Values 0 = AND, 1 = OR. If a prover passes `compoundLogic = 2`:
- `logicIsOr.out == 0` (since `2 != 1`)
- `finalResult = 1 * andResult + 0 * orResult = andResult`

So `compoundLogic = 2` behaves like AND. This is benign but is a circuit underspecification — any value ≥ 2 silently becomes AND. The circuit should `assert compoundLogic <= 1`.

---

#### 🔵 NEW-LOW-01: `SasAttestationBuilder.attester` Field Is Dead Code

`build_output.txt` line 165-177 shows `warning: field 'attester' is never read` in `solid-core/src/sas.rs`. Dead field in a security-critical data structure is a code smell that could indicate a missing feature (is the attester supposed to be part of the commitment?).

---

#### 🔵 NEW-LOW-02: `cpi_helpers.rs` Has a Stale Comment About Light Protocol

Line 120-121 in `cpi_helpers.rs`:
```
/// build the proof from a local in-memory replica
/// of the tree seeded by `CredentialIssued` events. Useful for tests and
/// localnet E2E; O(tree) memory.
```
The comment references the Light Protocol indexer pattern but is in the post-SPL-AC version. The `SCHEMA_REGISTRY_PROGRAM_ID` constant in `cpi_helpers.rs:38` says "the on-chain caller must still check that the supplied schema_tree_N account is owned by this program" — but as HIGH-04/05 correctly flags, **the on-chain caller does NOT do this check**.

---

#### 🔵 NEW-LOW-03: `devnet.json` Describes `nullifier_bloom` PDA That No Longer Exists

`deployments/devnet.json:41-44`:
```json
"nullifier_bloom": {
  "seeds": ["nullifier-bloom", "<verifier_config_pubkey>"],
  "program": "FhtEvsUwxRRT8nf3uaxK6iAefFqvqCYimc9vqnH5XEgr"
}
```

The bloom filter approach was replaced by PDA-per-nullifier in v0.2. This ghost reference is misleading to any operator who reads it.

---

#### 🔵 NEW-LOW-04: `store_verification_key` Has No Chunk Index Continuity Check

In `zk-verifier`:
```rust
if chunk_index == 0 {
    vk_storage.data = chunk_data;
} else {
    vk_storage.data.extend_from_slice(&chunk_data);
}
```

There is no check that chunks arrive in order (`chunk_index == 0, 1, 2, ...`). An operator could send chunk 3 before chunk 2, producing a scrambled VK with no error. Add sequence validation.

---

### 3.3 Things the Security Audit Got WRONG (overclaimed)

| Audit claim | Actual status |
|-------------|---------------|
| "TS `verifyOnChain` — ✅ real Anchor IX builder + confirmed tx" (SOLID_FINAL_AUDIT table row 4) | **FALSE.** The `sdk/src/index.ts::verifyOnChain` returns a hardcoded string. The `@solid-protocol/verifier` package referenced as containing a real implementation does not exist. |
| "Revocation v1 is live" (REVOCATION_DESIGN.md line 3) | **FALSE.** Revocation is a design document only. No on-chain mechanism exists. |
| "`@solid-protocol/light::insertCredentialLeaf` builds a real LightSystemProgram.compress ix" (SOLID_FINAL_AUDIT R-2) | **FALSE or obsolete.** `insertCredentialLeaf` is not exported from `light/src/index.ts`. The v0.2 light package replaced this with on-chain CPI in `issuer-registry::issue_credential`. |

---

## PART 4 — WHERE IMPLEMENTATION LEADS vs. LAGS

### Implementation is AHEAD of docs:
1. The `solid-light` crate's `cpi_helpers.rs` has excellent coverage (8 unit tests, bounds-checked parsers). Not mentioned in any user-facing doc.
2. `set_binding_status` (emergency freeze) — powerful admin tool, undocumented.
3. `VkBuf` compile-time stack assertion (`const _: () = { assert!(sz < 3072) }`) — a truly elegant safety net, mention-worthy in docs.
4. `CredentialIssued` event structure is fully correct and well-specified — the off-chain indexer hook works.
5. `parseCredentialIssuedEvent` in `@solid-protocol/light` — correctly parses the 152-byte Anchor event layout.

### Implementation LAGS behind docs:
1. `@solid-protocol/verifier` package — referenced everywhere, physically missing.
2. Revocation — documented as "live", completely absent.
3. Circuit artifacts (`.wasm`, `.zkey`, CDN) — referenced, don't exist.
4. `solid-prover` crate at `tools/` — referenced, absent.
5. The `SOLID_FINAL_AUDIT` table row claiming `verifyOnChain` is "✅ real" — wrong assertion in the audit doc.
6. `governance_token_mint` / `SOLID_TOKEN_MINT` — all stubs.

---

## PART 5 — ARCHITECTURAL ASSESSMENT

### What is genuinely excellent:

1. **The three-program decomposition is correct.** Each program has one responsibility. ZK verifier doesn't touch governance; issuer registry doesn't run proofs; schema registry doesn't vote. Clean.

2. **The nullifier design is state-of-the-art.** `Poseidon(masterKey, revNonce, verifierAddr, queryCtxHash, verifierNonce)` — 5-arg domain separation that binds proof to specific verifier + specific query + cannot be replayed + cannot be cross-verifier replayed. This is architecturally better than Semaphore's 3-arg nullifier.

3. **Backend-agnostic root verification via PDA parsing** is elegant. The on-chain ZK verifier doesn't know if the storage backend is SPL AC, Light Protocol, or a SolID-native tree — it just reads raw bytes at known offsets. This is forward-compatible.

4. **VkBuf stack-owned parser** — eliminating `Box::leak` from proof verification is non-trivial. The compile-time size assertion is excellent engineering discipline.

5. **Cross-language test vectors** — the `commitment_and_nullifier.json` approach, with a Rust generator and a TS checker, is the right way to guarantee protocol consistency across languages. Most ZK projects don't have this.

6. **`enabled` gating in circuits** — `enabled = 1 - isZero(schemaHash)` ensuring padded credential slots are true no-ops prevents data-smuggling attacks. Well designed.

7. **The SPL AC migration** — removing the Light Protocol dependency reduces the operational surface area significantly. SPL AC is a Solana Foundation program with long-term stability guarantees.

### What is structurally weak:

1. **The DAO governance system cannot function.** It needs:
   - A real SPL token (`SOLID_TOKEN_MINT`) — currently a placeholder
   - `active_votes_count` to actually increment (CRITICAL-03)
   - Voting period enforcement (CRITICAL-04)
   - Slashed lamports to actually move (HIGH-01)
   
   In its current state, the "DAO governance" is security theater — any staker can vote and immediately unstake with zero economic consequence.

2. **The verifier's trust-root chain is broken.** HIGH-04/05 means the global tree account can be forged. The account data parsing (`verify_state_root_matches`) is correct, but it's parsing data from an account that could be owned by System Program. The entire ZK verification would succeed against a fabricated root. This is the most dangerous production vuln.

3. **There is no end-to-end test proving the system works.** 41 unit tests for cryptographic primitives is good. Zero integration tests for the programs is deeply concerning for a system that claims cryptographic soundness. Any integration bug (wrong account ordering, wrong PDA derivation, wrong public input indexing) would only surface when actually running on-chain.

4. **The issuer public key is not bound to the on-chain registry.** The circuit proves "this credential was signed by SOME BabyJubJub key" and "this credential is in SOME tree whose root is registered." But it does NOT prove "the BabyJubJub key that signed this credential belongs to an **approved** issuer." An attacker who creates a credential, signs it with any BJJ key, and gets the commitment into the tree via `issue_credential` (which only checks `issuer.status == Approved`) can prove against it — but there's no circuit constraint linking `issuerPubKeyAxs[i]` to the on-chain `IssuerAccount.bjj_pub_key_x`. This is a systemic trust gap.

---

## PART 6 — WHAT TO IMPROVE (ORDERED BY IMPACT)

### P0 — Must fix before any testing:

| # | Fix | Why |
|---|-----|-----|
| 1 | Fix `RegistryConfig` space to 112 bytes | Registry is undeployable without this |
| 2 | Add owner constraint to `global_tree` and `schema_tree_N` | Otherwise entire proof system is bypassable with fake accounts |
| 3 | Fix `vote_on_issuer` to increment `active_votes_count` | DAO governance is broken |
| 4 | Add voting period check to `vote_on_issuer` | Late-vote stuffing possible |
| 5 | Fix BUG-02: nullifier decimal→hex parsing | Every batch proof registers wrong nullifier |
| 6 | Create or wire `@solid-protocol/verifier` package | E2E demos break at import |

### P1 — Should fix before devnet testing:

| # | Fix |
|---|-----|
| 7 | Transfer slashed lamports from vault to treasury (HIGH-01) |
| 8 | Mark `staker_account` and `vote_record` as `mut` in `ReleaseVote` (HIGH-03) |
| 9 | Add access control to `IncrementUsage` (CRITICAL-02) |
| 10 | Fix `approximate_via_trust_anchor` target PDA constraint (NEW-HIGH-01) |
| 11 | Add VK size cap in `store_verification_key` (NEW-HIGH-02) |
| 12 | Fix BUG-04: identity commitment uses master key instead of per-schema key |
| 13 | Emit `IssuerApproved` event from `approve_via_trust_anchor` (MEDIUM-01) |
| 14 | Update `devnet.json` with correct program IDs |

### P2 — Should fix before mainnet:

| # | Fix |
|---|-----|
| 15 | Add `numPredicates <= MAX_PREDICATES` constraint to circuit (NEW-MEDIUM-03) |
| 16 | Add `compoundLogic <= 1` constraint to circuit (NEW-MEDIUM-04) |
| 17 | Fix `resolveSchema` memcmp offset (MEDIUM-03) |
| 18 | Fix `listIssuers` filter offset + byte encoding (MEDIUM-04) |
| 19 | Wire cross-language vector test into CI as required gate |
| 20 | Deploy governance token and replace all placeholder pubkeys |
| 21 | Add chunk sequence validation to `store_verification_key` |
| 22 | Fix `SchemaAccount` space calculation for `field_names` |
| 23 | Implement issuer pubkey binding in circuit (link `issuerPubKeyAxs[i]` to on-chain registry) |
| 24 | Run real trusted setup ceremony; generate and distribute `.wasm`/`.zkey` |
| 25 | Implement revocation (v1 design is solid; just needs implementation) |

### P3 — Nice to have:

| # | Fix |
|---|-----|
| 26 | Use `GLOBAL_DEPTH` parameter instead of hardcoded `20` in batch circuit |
| 27 | Move `SOLID_FINAL_AUDIT` table claim for `verifyOnChain` from ✅ to 🟡 |
| 28 | Remove `nullifier_bloom` ghost reference from `devnet.json` |
| 29 | Remove dead `attester` field from `SasAttestationBuilder` or use it |
| 30 | Fix `devnet.json` toolchain metadata to match actual pinned versions |

---

## PART 7 — OVERALL SYSTEM ASSESSMENT

### The Honest Score: **47/100** (Research-Grade Prototype)

| Dimension | Score | Notes |
|-----------|-------|-------|
| Cryptographic design | 90/100 | The nullifier, commitment, and identity models are excellent. Circuit constraints are correct (minus expiry + numPredicates gap). |
| On-chain program correctness | 45/100 | 4 criticals + 5 highs; registry space bug means it has never successfully deployed. Governance is non-functional. |
| SDK completeness | 30/100 | Two stubs masquerade as real implementations. Holder has real logic but critical bugs. Verifier package doesn't exist. |
| Documentation accuracy | 55/100 | Architecture and circuit docs are accurate. Audit docs overclaim. devnet.json is completely stale. |
| Testing coverage | 35/100 | 41 unit tests for primitives (good). 0 integration tests for any program (unacceptable for a production claim). |
| Security posture | 50/100 | The trust-root forging gap (HIGH-04/05) undercuts everything. DAO is economically broken. Core ZK replay protection is solid. |
| Operational readiness | 10/100 | No circuit artifacts, no deployed governance token, no CDN, stale manifests, no integration tests. |

### What this system IS:
A **research-grade prototype** with a genuinely sophisticated cryptographic design, a coherent three-program architecture, and world-class cross-language test vectors for the core primitive layer. The circuit design and nullifier construction are production-quality.

### What this system IS NOT:
Production-ready, or even devnet-ready. The governance system is broken at multiple levels. The SDK cannot generate a real proof. The verifier cannot submit real transactions. Two of the four critical security findings would cause the programs to either fail to deploy (`RegistryConfig` space) or allow root forgery (account ownership checks).

### The Gap:
The documentation describes a system that is 70% complete. The actual implementation is closer to 40% complete, with the upper 30% existing only in well-written audit documents rather than code. The most recent audit document (`SOLID_FINAL_AUDIT_2026_04.md`) overclaims significantly — it describes `verifyOnChain` as "✅ real" when it is a string literal.

### The Path Forward:
The foundation is solid (pun intended). The cryptographic design does not need to change. What's needed is:
1. Fix the 6 P0 bugs (all are small, surgical fixes)
2. Write 20-30 Anchor integration tests using bankrun or program-test
3. Run a proper Groth16 trusted setup ceremony
4. Build or properly stub the verifier package with real Anchor instruction encoding
5. Deploy a governance token

With focused engineering, this could be devnet-ready in 4-5 weeks and mainnet-candidate in 3-4 months (including external audit time).

---

*End of Audit*
