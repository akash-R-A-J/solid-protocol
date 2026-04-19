# SolID — Deep Comprehensive Audit (2026-04-19)

**Auditor:** Senior Engineering Review, pre-remediation snapshot  
**Scope:** Entire workspace — circuits, Rust crates, Anchor programs, TS SDK, WASM, scripts, docs  
**Goal:** Identify every logical flaw, vulnerability, incomplete implementation, and cross-layer mismatch blocking an end-to-end, production-grade bring-up.

> This document captures the state of the repository **before** the 2026-04 remediation pass. The companion document `SOLID_POST_REMEDIATION_AUDIT_2026_04.md` captures the state **after** fixes.

---

## 1. Executive summary

The protocol idea — **privacy-preserving credential queries with compound logic, batched proofs, and compressed on-chain state** — is sound, novel, and matches a real market gap on Solana. However, the *claimed* status (per `SOLID_INFRA_IMPROVEMENTS_STATUS.md` and `SOLID_COMPREHENSIVE_REVIEW.md`) significantly overstates the actual state of the code.

| Dimension | Claimed | Actual (this audit) |
|---|---|---|
| Workspace compiles | ✅ | ❌ (typo in `crates/solid-prover/Cargo.toml`, undefined symbols) |
| Rust ⇄ Circom crypto agreement | ✅ | ❌ commitment formula differs |
| WASM bridge works | ✅ | ❌ missing imports, wrong arity, truncated function |
| On-chain Groth16 verification | ✅ | ⚠️ logic present but file has a missing `}` and removed helper fns |
| Light Protocol CPI | ✅ | ❌ calls non-existent `light_sdk::cpi::compressed_account_create` |
| Schema–root binding | ✅ | ❌ `verify_schema_root_binding` is `return true;` |
| Fraud-proof slashing | ✅ | ❌ anyone can slash any issuer arbitrarily |
| TS SDK e2e | ✅ | ❌ verifier ID is `new Uint8Array([/* ... */])`, identity commitment stub returns zeros |

**Realistic completion: ~32%.** The design is coherent and the documentation is excellent, but the implementation has multiple class-breaking defects.

---

## 2. Idea validation (pros / cons)

### Pros
- Attacks a genuine, under-served problem: programmable selective disclosure for on-chain identity.
- Strong architectural choices: Poseidon everywhere, BabyJubJub native to circomlib, Light Protocol for compressed state.
- Composability: batch proofs over N=4 heterogeneous credentials is uncommon and valuable.
- Clean separation between data-availability (SAS) and computation (SolID ZK layer).
- Well-factored Circom library (`lib/*`) with reusable templates.
- Very good documentation discipline (architectural docs, threat model, change logs).

### Cons / risks
- Groth16 + trusted setup = operational liability (powers-of-tau ceremony, VK integrity, circuit immutability).
- Proof generation latency (snarkjs ≥ 10 s on mobile-class CPUs) hurts UX for real-time verification flows.
- Two independent Merkle-tree domains (global identity tree + per-schema credential trees) complicate the Light Protocol integration surface.
- Hardened nullifier (5-way) adds privacy and replay safety, but raises the entropy budget for indexing/analytics features.
- Issuer registry with token-weighted voting is a governance attack surface — needs quorum, emergency pause, and vote-decay mechanics.

### Competition / differentiation
Compare with: zkLogin (Sui), WorldID, Polygon ID, Sismo. SolID's differentiators are (a) native Solana + Light Protocol, (b) compound-predicate batching, (c) key-derivation-based credential privacy. The differentiation is real but only matters if the implementation actually ships.

---

## 3. Layer-by-layer findings

### 3.1 `crates/solid-prover`

- **`Cargo.toml` has a parse error:** line 1 is `ve [package]` instead of `[package]`. This breaks the entire workspace `cargo` resolution, not just this crate.
- Prover code itself (ark-circom driver) is reasonable once the manifest is fixed.

### 3.2 `crates/solid-core`

- **`lib.rs` does not re-export** `Credential`, `MultiCredentialQuery`, `BJJPublicKey`, `MAX_PREDICATES`. Downstream crates using `use solid_core::{Credential, ...};` fail to compile.
- **`query.rs`:** lines 237–248 close the `CircuitMultiQueryInputs` struct, but lines 250–286 contain `evaluate` and `to_circuit_inputs` floating outside any `impl` block — a parse error. The methods were meant to live inside `impl CompoundQuery`.
- **`query.rs` tests:** `Predicate::new(0, Operator::Gte, 21)` calls the constructor with 3 args, but `Predicate::new` takes 4 (credential index, field index, operator, value).
- **`query.rs` `MultiCredentialQuery`** has no `verifier_address` nor `current_timestamp`, but `multi_cred.rs` and the circuit both require them.
- **`multi_cred.rs`** accesses `query.verifier_addr`, `query.current_timestamp`, and `cred.merkle_siblings`, `cred.issuer_sig_r8`, `cred.issuer_sig_s`, none of which exist on the real `Credential` struct (which uses `issuer_signature: EdDSASignature`, no per-credential merkle proof).
- **`multi_cred.rs`** hardcodes `issuer_sig_r8ys.push(BigInt::from(0))` — the Y-coordinate of the signature R8 is *required* for circom verification; zeroing it silently corrupts every witness.
- **`commitment.rs`** computes `Poseidon(dataHash, schemaHash, holderX, holderY, salt)` — **five inputs, schemaHash included.**

### 3.3 Circom circuits

- **Commitment mismatch:** `circuits/lib/credential_atom.circom` computes `attestationHash = Poseidon(data[0..N-1], salt, holderAx, holderAy)` — no `dataHash` intermediate, no `schemaHash`, no domain separation. This **does not match** Rust (§3.2). Consequences: any credential created by the Rust issuer SDK has a commitment that can never be proven by the holder's circuit. The two layers are talking past each other.
- **`SignatureVerifier` misuse:** `credential_atom.circom` passes `pubKeyX/pubKeyY` to `SignatureVerifier`, but standard `EdDSAPoseidonVerifier` from circomlib expects `Ax/Ay`. Also omits `enabled` parameter.
- **Include paths:** Most `circuits/lib/*.circom` files use relative paths like `include "poseidon.circom";` that only resolve when invoked from `circuits/`; the top-level `compound_query.circom` uses `node_modules/circomlib/circuits/poseidon.circom`, which is correct but inconsistent.
- **Merkle proof backend:** `merkle_inclusion.circom` wraps `SMTVerifier`, which assumes sparse-Merkle semantics. Light Protocol uses an indexed binary Merkle tree. The mismatch means a proof generated off-chain will not verify against Light's root even when the data is right. Either swap to a binary-Merkle-tree template or add a client-side adapter.

### 3.4 WASM bridge (`wasm/src/lib.rs`)

- **No top-level imports:** `wasm_bindgen`, `Serialize`, `JsError`, `JsValue` are used but never `use`d. The file won't compile.
- **`deriveKey` is truncated:** line 184 has the `#[wasm_bindgen(...)]` attribute, but there is no `pub fn deriveKey(...)` signature — the function body begins mid-air with `let mut mk = [0u8; 32];`.
- **`computeNullifier` arity mismatch:** the wrapper takes 3 args (`holder_private_key`, `schema_hash`, `verifier_nonce`) but the Rust implementation takes 5 (`master_key`, `rev_nonce`, `verifier_addr`, `query_hash`, `verifier_nonce`).
- **Missing `computeHardenedNullifier`:** TS SDK calls `wasmModule.computeHardenedNullifier(...)` but nothing of that name is exported.
- **`poseidonHashShared` race:** shared `Mutex<Vec<u8>>` buffer is safe for correctness but a single-threaded JS runtime can still cause interleaved calls if async hashing is invoked concurrently.

### 3.5 Anchor programs

#### `programs/zk-verifier`

- **Program ID mismatch:** `declare_id!("FhtEvsUwxRRT8nf3uaxK6iAefFqvqCYimc9vqnH5XEgr")` vs `Anchor.toml = BZkVFdMhAEeGMvEAhXNjt3r3bEA2sCPqEFcsEbSbFGj2` vs TS SDK `ID_BYTES = new Uint8Array([/* ... */])` (empty).
- **Syntactic break:** `store_verification_key` ends at line 79 without a closing `}` or `Ok(())` — subsequent doc comment for `verify_batch_proof` starts inside the same function scope.
- **Dangling references:** `deserialize_vk(...)` and `negate_g1_point(...)` are called but not defined in this file (comment on line 221 says "unchanged logic for deserialize_vk, bloom functions removed" — the implementations are genuinely missing).
- **Bloom constants still present** despite the previous audit recommending removal in favour of a real replay-safe registry.
- **`verify_schema_root_binding` trusted blindly** — its implementation in `solid-light` returns `true` unconditionally (see §3.6).

#### `programs/issuer-registry`

- **Duplicate imports:** lines 2–3 both import `token` from `anchor_spl`.
- **Undefined `registry` binding** at line 55 (`registry.min_stake_lamports * stake_multiplier`) — should be `ctx.accounts.registry_config`.
- **`register_issuer` space allocation** (`8 + 32 + 64 + 128 + 32 + 32 + 1 + 8 + 8 + 8 + 8 + 8 + 8 + 8`) does not match the `IssuerAccount` struct layout (missing `cooldown_ends_at`, slash_count tail, String length prefixes).
- **Auto-approval hardcode:** `if issuer.votes_for >= 1_000_000` — no relation to configured threshold.
- **`submit_fraud_proof` is wide open:** the comment explicitly says "In V1 we stub the actual ZK verification" — any signer can slash any Approved issuer up to `slash_amount`. Even with the reporter-bounty incentive this is a **full compromise** of the trust layer.
- **Missing error variants:** `NoVotingPower`, `InvalidWithdrawStatus` referenced but not defined.
- **`register_issuer_cpi` called** (line 297) but not defined in `solid-light::cpi_helpers` — only `build_insert_issuer_data` (private).

#### `programs/schema-registry`

- **`solve_poseidon`** on-chain is commented out; `register_schema` does not verify that `schema_hash` matches the canonical Poseidon of the schema fields.
- Otherwise mostly fine — this is the cleanest Anchor program.

### 3.6 Light Protocol CPI layer

- **`verify_state_root_matches`** assumes `tree_account_data[8..40]` is the current root; this is a hard-coded layout assumption that will silently pass or fail whenever Light's account format changes.
- **`verify_schema_root_binding`** does the root integrity check then returns `true` unconditionally with a comment "Placeholder for actual binding check". This nullifies the entire anti-root-smuggling defense.
- **`register_nullifier_cpi` / `register_identity_cpi`** invoke `light_sdk::cpi::compressed_account_create(...)` and `light_sdk::cpi::accounts::CompressedAccountCreate`. **Neither symbol exists in `light-sdk = "0.4.0"`** as published on crates.io — this is a fabricated API path.
- **`register_issuer_cpi`** is called from `issuer-registry` but not defined anywhere.

### 3.7 TypeScript SDK

- **`ID_BYTES` placeholder:** `new Uint8Array([/* ... FhtEvsU ... bytes */])` — empty byte array, will serialize to zero, breaking nullifier-scope-binding assertions.
- **`MAX_CREDENTIALS` / `NUM_FIELDS` not exported** from `@solid-protocol/core` but imported from it in `holder`.
- **`computeNullifier`** calls `wasmModule.computeHardenedNullifier(...)` which does not exist; meanwhile the 5-arg wrapper `computeNullifier` is called from `holder/index.ts` with only 3 args.
- **`computeIdentityCommitment` in holder/index.ts** is a stub that returns `new Uint8Array(32)` (all zeros) — every batch proof built via the public SDK binds to the zero identity.
- **`insertCredentialLeaf`** in `light/index.ts` calls `LightSystemProgram.compress` with a raw buffer; that method is for lamport compression, not arbitrary-data leaf insertion.
- **`verifier/index.ts`** `verifyOnChain` and `checkIssuerStatus` are stubs returning `undefined`/`true`.
- **`fetchMerkleProof`** retrieves accounts using `commitment.slice(0,32)` as *owner*; the owner of a compressed account is the issuer program, not the leaf hash.
- **`getStateRoot`** is a `return new Uint8Array(32)` stub.

### 3.8 Scripts

- `scripts/initialize.ts` still initializes a Bloom filter; contradicts the recommendation to use a PDA-per-nullifier or a real SMT.
- `scripts/prove.ts` calls `generateProof` with a `CompoundQuery` shape missing `globalRoot`, `masterIdentityKey`, `revocationNonce`.
- `scripts/issue.ts` is close to correct but depends on the broken issuer SDK.

---

## 4. Critical flaws, prioritized

| ID | Severity | Issue | Blocks |
|---|---|---|---|
| P0-1 | Crit | Commitment formula mismatch (Rust vs Circom) | End-to-end proof generation |
| P0-2 | Crit | `solid-prover/Cargo.toml` typo | Entire workspace build |
| P0-3 | Crit | `verify_schema_root_binding` returns `true` | Root-smuggling attack |
| P0-4 | Crit | `submit_fraud_proof` not gated | Arbitrary stake theft |
| P0-5 | Crit | `light_sdk::cpi::compressed_account_create` is fabricated | Any on-chain write to Light |
| P0-6 | Crit | WASM `compute_nullifier` arity mismatch | All client-side nullifiers |
| P0-7 | Crit | Program ID inconsistency (Anchor/declare/SDK/scripts) | All on-chain calls |
| P1-1 | High | `zk-verifier` missing `deserialize_vk` / `negate_g1_point` | Proof verification |
| P1-2 | High | `query.rs` parse errors | solid-core build |
| P1-3 | High | `multi_cred.rs` uses non-existent fields | batch proof generation |
| P1-4 | High | holder SDK `computeIdentityCommitment` returns zeros | Identity binding |
| P1-5 | High | `issuer-registry` duplicate imports, undefined `registry` | Anchor build |
| P1-6 | High | WASM missing top-level imports, truncated `deriveKey` | WASM build |
| P2-1 | Med | Bloom filter constants + docs out of sync | Ops complexity |
| P2-2 | Med | `SMTVerifier` vs Light indexed tree mismatch | Root agreement |
| P2-3 | Med | Schema hash not verified on-chain in `schema-registry` | Tamper vector |
| P2-4 | Med | TS SDK inconsistent naming (camel/snake) | DX only |

---

## 5. Missing entirely

- Cross-language test vectors for commitment + nullifier (to prove Rust ⇄ Circom ⇄ TS agree).
- On-chain **replay-safe nullifier registry** that actually works without Light Protocol (PDA-per-nullifier).
- A real **schema-registry ↔ credential-tree binding** (registered tree pubkey per schema).
- **Per-credential revocation** endpoint + circuit hook.
- **Trusted-setup ceremony** scripts (`setup.js` is a toy).
- **Governance emergency pause** and upgrade-authority handover.
- **Integration tests** that run anchor-localnet + circom + snarkjs in one loop.
- **Keeper / indexer service** to watch on-chain events and feed the holder SDK.

---

## 6. Completion scorecard (realistic)

| Layer | Completion |
|---|---|
| Docs / architecture | 80% |
| Circom circuits (structure) | 70% |
| Circom circuits (crypto correctness vs Rust) | 40% |
| Rust `solid-core` | 55% |
| Rust `solid-light` | 30% |
| Rust `solid-prover` | 20% (unbuildable) |
| WASM bridge | 40% |
| `zk-verifier` program | 55% |
| `issuer-registry` program | 45% |
| `schema-registry` program | 70% |
| TS SDK `core` | 50% |
| TS SDK `holder` | 35% |
| TS SDK `issuer` | 30% |
| TS SDK `verifier` | 15% |
| TS SDK `light` | 25% |
| Tests | 10% |
| E2E scripts | 20% |
| **Overall** | **~32%** |

---

## 7. Remediation strategy (executed in companion PR)

1. Make the workspace build (manifest fix, imports, re-exports, dangling fns).
2. Pick **one** canonical commitment and nullifier formula; align Rust, Circom, TS, and WASM to it. Add cross-language test vectors.
3. Replace `light_sdk::cpi::compressed_account_create` with a documented trait + a Solana-native **PDA-per-nullifier** fallback that gives real replay-safety without Light, and a clean seam for when Light is wired.
4. Implement `verify_schema_root_binding` against a **real** registered-tree PDA in `schema-registry`.
5. Gate `submit_fraud_proof` behind either (a) `registry_config.authority` or (b) a verified fraud-proof account — default: authority until ZK fraud-proof verifier ships.
6. Unify program IDs across `Anchor.toml`, `declare_id!`, `deployments/devnet.json`, and the TS SDK `ID_BYTES`.
7. Implement the missing TS SDK pieces: real identity commitment via WASM, real `verifyOnChain` via Anchor IDL, real leaf insertion via the new trait.
8. Update all docs to reflect post-fix reality; no claim may stand without code to back it.
9. Re-audit and publish `SOLID_POST_REMEDIATION_AUDIT_2026_04.md`.

The remediation holds the **core idea** constant: Poseidon-committed credentials in compressed Merkle trees, queried by Groth16 proofs under a hardened nullifier, with a DAO-curated issuer set. What changes is the rigor, not the architecture.
