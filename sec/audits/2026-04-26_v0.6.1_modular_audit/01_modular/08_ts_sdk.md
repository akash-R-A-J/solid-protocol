# Audit 08 — TS SDK (cross-language byte-identity edge)

| Field | Value |
|-------|-------|
| Files | `ts-sdk/packages/{core,holder,issuer,light,sdk,verifier}/src/*.ts` (8 files, 2614 LOC) |
| Auditor | Orchestrator hand-audit (after agent stalled) |
| Date | 2026-04-26 |

The TS SDK is the off-chain edge of the protocol. While `sec007-skip-onchain` is enabled in `programs/issuer-registry`, the SDK's `isInPrimeOrderSubgroup` is the load-bearing BJJ subgroup gate. Any Rust↔TS byte-identity drift breaks proof verification.

---

## 1. Workspace + cross-package dep map

```
core ──► (WASM bridge: ../wasm/solid_wasm.js)
       └─ exports: poseidonHash, poseidonHashBytes, generateKeypair,
          isInPrimeOrderSubgroup, sign, verify, computeCommitment,
          computeNullifier, computeIdentityState, computeIssuerLeaf,
          generateIdentity, unlockIdentity, deriveKey, deriveCredentialKey,
          QueryBuilder, type BJJKeypair / EdDSASignature / Predicate
holder ──► core, light, snarkjs (off-chain prover)
issuer ──► core, light  (issuance signing)
light ──► (SPL-AC adapter)
sdk ──► core, holder, issuer, light, verifier  (top-level convenience)
verifier ──► core (read PROGRAM_IDS), @solana/web3.js (instruction builder)
```

---

## 2. `core/src/index.ts` (471 LOC)

### 2.1 `isInPrimeOrderSubgroup` (lines 162-168) — load-bearing while SEC-048 bypass active

Wraps WASM `isBjjInPrimeOrderSubgroup`. Returns `boolean`. Lines 142-161 doc-comment correctly documents the SOLID-SEC-007 / SEC-048 trust boundary: every off-chain `register_issuer` caller MUST run this. Verified that this delegates to `solid_core::babyjubjub::is_in_prime_order_subgroup` which performs the canonical `r * P == O` cofactor-cleared check.

### 2.2 `computeNullifier` (lines 220-233) — 6-input ADR-0006 revision

Wraps WASM `computeHardenedNullifier`. Six inputs in the documented order: `(masterKey, revocationNonce, verifierAddress, queryContextHash, verifierNonce, issuerTreeRoot)`. **Matches Rust `nullifier.rs:45-64` and circuit `batch_credential_query.circom:434-441` exactly.** ✓

### 2.3 `computeIssuerLeaf` (lines 265-293) — manually packs u64s

Lines 273-285 manually pack `statusEpoch` and `revocationNonce` (bigint) into 32-byte LE buffers (low 8 bytes carry the value, remaining 24 bytes zero). Then calls `poseidonHashBytes([authority, x, y, statusEpochBytes, nonceBytes])`. **Matches Rust `compute_issuer_leaf_bytes` at `issuer-registry/src/lib.rs:63-77` exactly.** ✓

The doc at lines 261-263 explicitly notes: "raw 32-byte arrays are fed directly into Poseidon, so callers MUST pass `authority.toBytes()` (not a BE-encoded BigInt)" — this is the LE encoding path. Combined with the BE-vs-LE concern from circuits audit M07-INVESTIGATE-01, this confirms: **the SDK uses LE for `issuerAuthority`**. The circuit comment at `batch_credential_query.circom:103-106` says BE. **Resolution**: the witness builder in `holder/src/index.ts:484` uses `bufToDecimalBE(VERIFIER_ID_BYTES)` for `verifierAddress` only; for `issuerAuthorities` it uses `bufToDecimal(c.issuerAuthority.toBytes())` (LE). So the witness pipeline uses LE for issuerAuthority, matching the Rust side. **The circuit comment at lines 103-106 is incorrect doc drift.** M07-INVESTIGATE-01 resolves to M08-DOCS01.

### 2.4 `QueryBuilder._computeContextHash` (lines 381-403) — **STRUCTURALLY DIFFERENT FROM CIRCUIT**

```ts
const hIndices = poseidonHash([...credIndices, ...fieldIndices]);
const hOps = poseidonHash([...ops, ...vals]);
return poseidonHashBytes([hIndices, hOps, len_le, logic_le]);
```

Concatenates `[cred_0..3, field_0..3]` (8 inputs).

But `batch_credential_query.circom:395-398` interleaves:
```
qHasherIndices.inputs[i*2] <== queryCredentialIndices[i];
qHasherIndices.inputs[i*2+1] <== queryFieldIndices[i];
```

The two produce **different** queryContextHash values for the same predicate set.

**HOWEVER**, the QueryBuilder is a UX helper whose `queryContextHash` field is NOT consumed by the proof submission path. The actual production batch path (`holder.generateBatchProof`) reads the nullifier from `publicSignals[0]` (line 554) — the circuit computes the queryContextHash internally from public inputs, and the nullifier emerges from STEP 5. No off-chain queryContextHash compare happens in the batch path.

**M08-M01 (MEDIUM)**: `QueryBuilder._computeContextHash` is silently incorrect. If a future caller uses `MultiCredentialQuery.queryContextHash` (set by `build()`) to compute a nullifier off-chain, it would diverge from the circuit's computation and the on-chain `nullifier == public_inputs[0]` check would fail. Recommend either fix the helper to interleave like the circuit, or remove the field from `MultiCredentialQuery` entirely.

### 2.5 `QueryBuilder.toCircuitInputs` (lines 435-470) — stale comment

Line 435 doc says "Solana verifier expects 31 inputs" — stale. Post-ADR-0014 the count is 32. The `verifier/src/index.ts:35` correctly uses `NR_PUBLIC_INPUTS = 32`. **M08-DOCS02**.

### 2.6 `snakeCaseBjj` (lines 129-135) — camelCase/snake_case shim

WASM bridge serializes BJJ keypairs as camelCase (`privateKey, publicKeyX, publicKeyY`); SDK contract is snake_case (`private_key, public_key_x, public_key_y`). The shim re-keys. Already flagged by wasm-bridge audit M06-L04. Fragile but functional.

### 2.7 Other functions

- `poseidonHash` / `poseidonHashBytes` — direct WASM passthrough.
- `generateKeypair` — WASM passthrough + camelCase shim.
- `sign` / `verify` — direct WASM passthrough.
- `computeCommitment` — direct WASM passthrough.
- `computeIdentityState` / `computeIdentityCommitment` — direct WASM passthrough; matches Rust `identity.rs:22-30`.
- `deriveKey` / `deriveCredentialKey` — direct WASM passthrough; second uses `snakeCaseBjj`.
- `generateIdentity` / `unlockIdentity` — encrypted-key-bundle JSON passthrough.

---

## 3. `holder/src/index.ts` (691 LOC) — proof generation

### 3.1 `generateProof` (single-credential, lines 107-275) — STRUCTURALLY BROKEN

Calls `computeQueryContextHash(query)` (line 136) then uses that as the 4th input to `computeNullifier` (line 144).

`computeQueryContextHash` (lines 666-684):
```ts
inputs = [schemaHash, field[0], op[0], val[0], ..., field[3], op[3], val[3], numPredicates, compoundLogic, expirationTimestamp]
return poseidonHash(inputs);  // Poseidon(16)
```

That's 1 + 4×3 + 3 = **16 inputs**. But Poseidon arity caps at 12 (`solid-core/src/poseidon.rs:111`, `wasm/src/lib.rs:36-41`). At runtime, `poseidonHash` rejects with `PoseidonHash("Poseidon supports 1-12 inputs, got 16")` and throws a JsError.

**M08-CRITICAL01**: `generateProof` (single-credential) is structurally broken at runtime. Either it's dead code (only `generateBatchProof` is used in production) or it has never been exercised.

The doc at line 661 says "Mirrors Step 4 of `circuits/compound_query.circom`" — referring to the alternate circuit, not the production `batch_credential_query.circom`. If `compound_query.circom` is dead (M07-INVESTIGATE-02), this whole function is dead.

### 3.2 `generateBatchProof` (lines 281-557) — production path

The production proof generator. Builds full circuit input including:
- Public inputs: `globalRoot, merkleRoots[4], schemaHashes[4], issuerTreeRoot, queryCredentialIndices[4], queryFieldIndices[4], queryOperators[4], queryValues[4], numPredicates, compoundLogic, verifierAddress (BE), verifierNonce (LE), currentTimestamp`.
- Private inputs: `masterIdentityKey, revocationNonce, globalSiblings[4][20], globalPathIndices[4][20], data[4][8], salts[4], issuerSig*[4], issuerPubKey*[4], merkleSiblings[4][20], merklePathIndices[4][20], expirationTimestamps[4], issuerAuthorities[4], issuerStatusEpochs[4], issuerRevocationNonces[4], issuerSiblings[4][16], issuerPathIndices[4][16]`.

**Critical-path observations**:

(a) **Sorted credentials** (lines 305-321): SEC-20 canonical ordering enforced before circuit input. Predicates remapped via `indexMap` so `queryCredentialIndices[i]` points at the new sorted position. ✓

(b) **Identity cohesion check** (lines 344-355) (SOLID-SEC-033): credentials must store the per-schema derived pubkey, not the master pubkey. Throws if `cred.holderPubKeyX != deriveCredentialKey(masterPrivateKey, cred.schemaHash).public_key_x`. ✓

(c) **Per-credential global-state proofs** (lines 381-405): each credential gets its own global-tree inclusion proof for the per-schema identity leaf. Cross-checks all returned roots are equal. ✓

(d) **Issuer-tree proofs** (lines 407-458): per-credential proofs for issuer leaves. Padding slots get zero-filled placeholder paths. Active slots cross-check the returned root matches `options.issuerTreeRoot`. ✓

(e) **Endianness contract** (lines 476-485): `verifierAddress` uses `bufToDecimalBE` (lines 604-610), everything else uses `bufToDecimal` (LE). This is SOLID-SEC-031: the `verifierAddress` public input must equal `ID.to_bytes()` byte-for-byte on-chain (which `verify_batch_proof` checks at `programs/zk-verifier/src/lib.rs:402`). The BE vs LE distinction is intentional and correct.

(f) **Nullifier sanity check** (line 554): `nullifier = bigintToBytes32(BigInt(publicSignals[0]))`. The circuit's nullifier output is consumed directly; no off-chain Poseidon recompute. So even if `computeQueryContextHash` is broken, generateBatchProof works. ✓

### 3.3 `bufToDecimal` (LE) vs `bufToDecimalBE` (BE)

Both functions are correctly implemented. The convention is:
- LE for field-element-typed inputs (Poseidon outputs, BJJ scalars, merkle roots, schema hashes, salts, issuer signatures, issuer authority).
- BE for Solana program/account pubkeys (verifierAddress).

This convention is documented at lines 575-582 and 591-602.

### 3.4 `formatProofForSolana` (lines 620-649) — Groth16 packing

Packs snarkjs proof into the format groth16-solana expects: `proofA = X || Y` (G1, 64B), `proofB = X.c0 || X.c1 || Y.c0 || Y.c1` (G2, 128B), `proofC = X || Y` (G1, 64B). Uses `bigintToBytes32` which packs BE (line 651-658). ✓

Note: `formatProofForSolana` does NOT negate proofA. The on-chain `negate_g1_point` in `programs/zk-verifier/src/lib.rs:755-784` negates proofA's Y coordinate before passing to Groth16Verifier. This is the load-bearing convention difference between snarkjs and groth16-solana — groth16-solana expects -A.

---

## 4. `verifier/src/index.ts` (287 LOC) — instruction builder

### 4.1 `buildVerifyBatchProofIx` (lines 117-193) — production path

Hand-rolls the Anchor instruction discriminator + Borsh-serialized arguments + accounts list. Verified against `programs/zk-verifier/src/lib.rs`:

**Borsh layout**:
- 8 bytes discriminator (sha256("global:verify_batch_proof")[0..8]). ✓
- 64 bytes proof_a, 128 bytes proof_b, 64 bytes proof_c. ✓
- 4 bytes Vec<[u8;32]> length prefix (LE u32 = 32). ✓
- 32 × 32 bytes public_inputs. ✓
- 32 bytes nullifier. ✓
- Total: 1324 bytes. ✓

**Accounts ordering** (lines 178-190):
1. configPda (mut) — `verifier_config`
2. vkPda — `vk_storage`
3. nullifierPda (mut) — `nullifier_record`
4. globalTree — `global_tree`
5. schemaTree0..3 — `schema_tree_{0,1,2,3}`
6. issuerTreeBinding — `issuer_tree_binding` (ADR-0014, post-schema-trees)
7. payer (signer, mut)
8. system_program

Compared to `VerifyBatchProof` Accounts struct in `programs/zk-verifier/src/lib.rs:842-894`:
1. `verifier_config` (mut, init-if-needed) ✓
2. `vk_storage` ✓
3. `nullifier_record` (init) ✓
4. `global_tree` ✓
5. `schema_tree_0..3` ✓
6. `issuer_tree_binding` ✓
7. `payer` (mut, Signer) ✓
8. `system_program` ✓

**Account ordering MATCHES.** ✓

### 4.2 PDA derivers (lines 86-113)

- `deriveVerifierConfigPda([b"verifier-config"], programId)`. ✓
- `deriveVkStoragePda([b"vk-storage", configPda], programId)`. ✓
- `deriveNullifierPda([NULLIFIER_SEED, nullifier], programId)`. ✓ Matches `programs/zk-verifier/src/lib.rs:858-860`.

### 4.3 `checkIssuerStatus` (lines 241-272) — IssuerAccount deserializer

Hand-rolled Borsh deserialization. Reads:
- 8 disc + 32 authority
- 4-byte name length + name bytes
- 4-byte metadata_uri length + metadata bytes
- 32 Ax + 32 Ay
- 1 tier
- 1 status
- 8 staked_amount

**Cross-check against `IssuerAccount` struct at `programs/issuer-registry/src/lib.rs:2272-2329`**: field order is `authority, name, metadata_uri, bjj_pub_key_x, bjj_pub_key_y, tier, status, staked_amount, ...`. Matches. ✓

`approved: status === 1` — `IssuerStatus::Approved = 1` per the enum at `programs/issuer-registry/src/lib.rs:2246-2253` (`Pending=0, Approved=1, Cooldown=2, Rejected=3, Revoked=4`). ✓

### 4.4 `generateVerifierNonce` (lines 278-287)

Uses `crypto.getRandomValues` (Web Crypto, available Node 18+) with fallback to `node:crypto.randomFillSync`. 32 bytes. ✓

---

## 5. `issuer/src/index.ts` (269 LOC) — credential issuance

Not deeply audited in this hand-pass.

**M08-TODO01**: full audit deferred. Key items to verify:
- BJJ subgroup check call site (must invoke `isInPrimeOrderSubgroup` before `register_issuer`).
- Credential commitment computation (must match Rust `compute_attestation_commitment`).
- EdDSA signing path (must use WASM `signMessage`, not reimplement).
- Issuer-leaf computation for backfill scenarios.

Top-level inspection shows imports of `core`, `light`, `bn.js`, `@solana/web3.js`. Standard pattern.

---

## 6. `light/src/index.ts` (503 LOC) — SPL-AC adapter

Not deeply audited.

**M08-TODO02**: full audit deferred. Key items:
- `MerkleProofAdapter` interface and implementations (LocalReplicaAdapter, Helius DAS adapter).
- `fetchMerkleProof(adapter, tree, leaf)` — what's the failure mode if the indexer is stale?
- Any reimplementation of SPL-AC logic in TS (should be minimal — wraps the upstream `@solana/spl-account-compression` package).
- Helper for `deriveIssuerTreeBinding` (referenced by verifier package as PDA seed source).

---

## 7. `sdk/src/index.ts` (252 LOC), `config.ts` (54), `rpc.ts` (86) — top-level

Not deeply audited.

**M08-TODO03**: full audit deferred. The top-level SDK is a convenience wrapper that re-exports from sub-packages. Likely low-risk surface.

---

## 8. Findings

### Critical

**M08-CRITICAL01**: `holder.generateProof` (single-credential path, lines 107-275) is structurally broken. `computeQueryContextHash` pushes 16 Poseidon inputs but the WASM bridge caps at 12. Throws at runtime. Either dead code or unexecuted in CI.

### High

**M08-H01**: `holder.generateProof` and `compound_query.circom` may both be dead — confirm and either delete or repair. Cross-references M07-INVESTIGATE-02.

### Medium

**M08-M01**: `QueryBuilder._computeContextHash` (core/src/index.ts:381-403) silently produces a queryContextHash inconsistent with the circuit's STEP 4. Currently unused in the proof path but a footgun for future callers.

### Low

**M08-L01 / M06-L04 (cross-ref)**: snake_case/camelCase shim is fragile.

**M08-DOCS01**: circuit comment at `batch_credential_query.circom:103-106` says `issuerAuthority` is BE; the SDK feeds LE. The comment is wrong; the SDK matches the on-chain Rust path. Fix the circuit comment.

**M08-DOCS02**: `QueryBuilder.toCircuitInputs` doc says "Solana verifier expects 31 inputs" — stale (now 32).

### Info

**M08-INF01**: `formatProofForSolana` does not negate proofA. The on-chain handler does negation in `negate_g1_point`. This split is intentional and matches groth16-solana's expected `-A` convention. Document this contract.

**M08-INF02**: `bigintToBytes32` (holder line 651) packs BE (high byte first). `bufToDecimal` is LE; `bufToDecimalBE` is BE. The two pairs are inverses by intent — caller must use matching pair.

---

## 9. Cross-language byte-identity verification (TS ↔ Rust ↔ circuit)

| Primitive | TS source | Rust source | Circuit source | Match? |
|-----------|-----------|-------------|----------------|--------|
| Poseidon hash | core.poseidonHash / poseidonHashBytes (WASM passthrough) | poseidon.rs:110-125 (canonicalize+hashv) | circomlib Poseidon | ✓ (via vector gate) |
| BJJ subgroup check | core.isInPrimeOrderSubgroup (WASM passthrough) | babyjubjub.rs:181-189 | EdDSAPoseidonVerifier internal | ✓ |
| EdDSA sign | core.sign (WASM passthrough) | babyjubjub.rs:291-325 | n/a (sign is off-chain only) | ✓ |
| EdDSA verify | core.verify (WASM passthrough) | babyjubjub.rs:336-374 | EdDSAPoseidonVerifier (in CredentialAtom) | ✓ |
| Commitment | core.computeCommitment (WASM passthrough) | commitment.rs:42-61 | credential_atom.circom:54-60 | ✓ (vectored) |
| Identity state | core.computeIdentityState (WASM passthrough) | identity.rs:22-30 | identity_anchor.circom:57-62 | ✓ (not vectored) |
| Per-schema key derivation | core.deriveCredentialKey (WASM passthrough) | babyjubjub::derive_key + derive_public_key | identity_anchor.circom:43-53 | ✓ (not vectored) |
| Issuer leaf | core.computeIssuerLeaf | issuer-registry.rs:63-77 | batch_credential_query.circom:264-269 | ✓ (not vectored) |
| Nullifier | core.computeNullifier (WASM passthrough) | nullifier.rs:45-64 | batch_credential_query.circom:434-441 | ✓ (vectored) |

**Two primitives currently vectored: Commitment + Nullifier (per `tests/vectors/commitment_and_nullifier.json`).**

**Seven primitives NOT vectored**: poseidonHash direct, poseidonHashBytes direct, isInPrimeOrderSubgroup, sign, verify, computeIdentityState, deriveCredentialKey, computeIssuerLeaf. SOLID-SEC-010 is the gap.

Per the wasm-bridge audit (M06-H01), this is the most impactful close-out item.

---

## 10. Suggested next actions

1. **Resolve M08-CRITICAL01**: confirm `generateProof` is dead, delete it. If alive, fix `computeQueryContextHash` to use ≤12 Poseidon inputs (matching the actual `compound_query.circom` STEP 4 layout if that circuit is alive).
2. **M08-H01**: confirm `compound_query.circom` build status (cross-references M07-INVESTIGATE-02).
3. **M08-M01**: delete `QueryBuilder._computeContextHash` or fix to interleave.
4. **M08-DOCS01**: fix circuit comment at `batch_credential_query.circom:103-106` (BE → LE).
5. **M08-DOCS02**: update `QueryBuilder.toCircuitInputs` doc to "32 inputs" post-ADR-0014.
6. **Extend cross-language vectors** to cover the 7 missing primitives (closes SOLID-SEC-010 + M06-H01).
7. **Defer**: full audits of issuer/light/sdk packages (M08-TODO01..03).

---

## 11. Summary

The TS SDK's production proof path (`generateBatchProof` → `buildVerifyBatchProofIx`) is correct and matches the on-chain Anchor accounts/Borsh layout. The cross-language byte-identity is sound where exercised (commitment + nullifier vectors), but 7 of 9 primitives lack pinned vectors.

**Critical**: `generateProof` (single-credential) is broken at runtime by Poseidon arity cap.

**Doc drifts**: circuit comment on issuerAuthority encoding (says BE, SDK uses LE matching Rust); QueryBuilder doc references 31 inputs (now 32).

**Findings**: 1 CRITICAL, 1 HIGH, 1 MEDIUM, 3 LOW, 2 INFO + 3 deferred. The CRITICAL is operationally dormant if the function isn't called, but should be either repaired or deleted.
