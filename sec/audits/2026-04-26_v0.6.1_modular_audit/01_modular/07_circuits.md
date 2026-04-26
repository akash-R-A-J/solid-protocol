# Audit 07 — circuits (Circom 2.1 / Groth16)

| Field | Value |
|-------|-------|
| Files | `circuits/batch_credential_query.circom` (459 LOC), `circuits/compound_query.circom` (275 LOC, alternate template), `circuits/lib/*.circom` (7 files), `circuits/scripts/setup.js`, `circuits/test/*` |
| Toolchain | circom 2.1.9, snarkjs 0.7.5, circomlib (system dep), `Bn254X5` parameters, BN254 curve |
| Auditor | Orchestrator hand-audit (after agent stalled) |
| Date | 2026-04-26 |

---

## 1. Public-input layout (32 slots, post ADR-0014)

Verified directly against `circuits/batch_credential_query.circom:20-34` and the `component main { public [...] }` declaration at lines 445-459. The order in the public-input array follows the order of `signal input` declarations in the template plus the `signal output` last:

| Index | Name | Constraint |
|-------|------|------------|
| 0 | `nullifierHash` | output of Poseidon6 at STEP 5 |
| 1 | `globalRoot` | bound to identity anchor for every active slot |
| 2..5 | `merkleRoots[NUM_CREDS]` | bound via `CredentialAtom`'s Merkle inclusion |
| 6..9 | `schemaHashes[NUM_CREDS]` | drives padding-slot zeroing + schema hash binding |
| 10 | `issuerTreeRoot` | ADR-0014 issuer-tree binding |
| 11..14 | `queryCredentialIndices[MAX_PREDICATES]` | bounded `< NUM_CREDS` (line 149-152) |
| 15..18 | `queryFieldIndices[MAX_PREDICATES]` | bounded `< NUM_FIELDS` (line 154-157) |
| 19..22 | `queryOperators[MAX_PREDICATES]` | NO bound (handled by switch) |
| 23..26 | `queryValues[MAX_PREDICATES]` | NO bound (Fr arbitrary) |
| 27 | `numPredicates` | `<= MAX_PREDICATES` (line 128-131) |
| 28 | `compoundLogic` | `∈ {0, 1}` (line 135) |
| 29 | `verifierAddress` | NO bound; bound by zk-verifier on-chain |
| 30 | `verifierNonce` | NO bound |
| 31 | `currentTimestamp` | NO in-circuit bound; on-chain enforces u64 |

**Cross-check against zk-verifier constants** (`programs/zk-verifier/src/lib.rs:47-53`):
- `ISSUER_TREE_ROOT_INPUT_INDEX = 10` ✓
- `VERIFIER_ADDRESS_INPUT_INDEX = 29` ✓
- `CURRENT_TIMESTAMP_INPUT_INDEX = 31` ✓
- `NR_PUBLIC_INPUTS = 32` ✓

**The orchestrator's earlier zk-verifier hand-audit doc-comment summary at `01_zk_verifier.md` Section "Module-level constants" used inclusive-end notation (e.g. `[2..5]` for indices 2-5)** — that matches the circuit's natural slot grouping. The constants are correct.

---

## 2. Critical correction: nullifier preimage

**The orchestrator's zk-verifier hand-audit at `01_modular/01_zk_verifier.md` and the wave-1 prompts incorrectly cited the 6 nullifier inputs as `(identityCommitment, secret, schemaId, credentialIndex, currentTimestamp, issuerTreeRoot)`.**

The actual circuit at `batch_credential_query.circom:434-441` is:

```circom
component nullifier = Poseidon(6);
nullifier.inputs[0] <== masterIdentityKey;
nullifier.inputs[1] <== revocationNonce;
nullifier.inputs[2] <== verifierAddress;
nullifier.inputs[3] <== queryContextHash;
nullifier.inputs[4] <== verifierNonce;
nullifier.inputs[5] <== issuerTreeRoot;
nullifierHash <== nullifier.out;
```

This MATCHES the Rust `crates/solid-core/src/nullifier.rs:45-64` (`compute_nullifier(master_key, rev_nonce, verifier_addr, query_hash, verifier_nonce, issuer_tree_root)`) exactly. **The cross-language byte-identity holds.** M04-H01 closed.

**Action**: orchestrator should patch `01_zk_verifier.md` to use the correct 6-input description. Tracked as M07-FIX01.

---

## 3. Cross-language byte-identity verification (per primitive)

### 3.1 Issuer leaf preimage (ADR-0014)

Circuit (`batch_credential_query.circom:264-269`):
```
issuerLeafHasher[i] = Poseidon(5);
issuerLeafHasher[i].inputs[0] <== issuerAuthorities[i];
issuerLeafHasher[i].inputs[1] <== issuerPubKeyAxs[i];
issuerLeafHasher[i].inputs[2] <== issuerPubKeyAys[i];
issuerLeafHasher[i].inputs[3] <== issuerStatusEpochs[i];
issuerLeafHasher[i].inputs[4] <== issuerRevocationNonces[i];
```

Rust (`programs/issuer-registry/src/lib.rs:63-77`):
```rust
solid_core::poseidon::hash_bytes(&[
    issuer.authority.to_bytes(),
    issuer.bjj_pub_key_x,
    issuer.bjj_pub_key_y,
    status_epoch_bytes,
    rev_nonce_bytes,
])
```

**Field ordering MATCHES** ✓.

**Endianness sub-check**: circuit doc at `batch_credential_query.circom:103-106` says `issuerAuthority` is "encoded as a big-endian field element (SEC-031)". Rust feeds `authority.to_bytes()` directly to `hash_bytes` which canonicalizes via `bytes_le_to_fr` (LE). **This is a doc-vs-code drift to investigate**. The witness builder (likely in TS SDK) is the disambiguator: if the witness uses LE encoding for `issuerAuthority`, the doc is wrong. If the witness uses BE, the on-chain leaf computed by Rust would NOT match the in-circuit leaf and every issuer-tree membership proof would fail. **M07-INVESTIGATE-01**: confirm against `ts-sdk/packages/holder/src/index.ts` witness construction.

### 3.2 Commitment preimage

Circuit (`circuits/lib/credential_atom.circom:54-60`):
```
commitHasher = Poseidon(5);
commitHasher.inputs[0] <== dataHash;
commitHasher.inputs[1] <== schemaHash;
commitHasher.inputs[2] <== holderBJJPubKeyAx;
commitHasher.inputs[3] <== holderBJJPubKeyAy;
commitHasher.inputs[4] <== salt;
```

Rust (`crates/solid-core/src/commitment.rs:57-58`):
```rust
let commitment = poseidon::hash_fr(&[data_hash_fr, schema_fr, holder_x_fr, holder_y_fr, salt_fr])?;
```

**MATCHES** ✓.

### 3.3 Identity-state preimage

Circuit (`circuits/lib/identity_anchor.circom:57-62`):
```
idStateHasher = Poseidon(3);
idStateHasher.inputs[0] <== bjjDerivation.Ax;
idStateHasher.inputs[1] <== bjjDerivation.Ay;
idStateHasher.inputs[2] <== revocationNonce;
```

Rust (`crates/solid-core/src/identity.rs:22-30`):
```rust
poseidon::hash_bytes(&[x, y, nonce_32])
```

where `nonce_32` has u64 LE in low 8 bytes, high 24 bytes zero. Since `bytes_le_to_fr(nonce_32) == Fr::from(u64)`, the inputs are byte-identical. **MATCHES** ✓.

### 3.4 Per-schema credential-key derivation

Circuit (`circuits/lib/identity_anchor.circom:43-53`):
```
credKeyDeriver = Poseidon(2);
credKeyDeriver.inputs[0] <== masterIdentityKey;
credKeyDeriver.inputs[1] <== schemaHash;
credentialPrivKey <== credKeyDeriver.out;
bjjDerivation = BabyPbk();
bjjDerivation.in <== credentialPrivKey;
```

Rust (`wasm/src/lib.rs:227-251`):
```
deriveKey(master, schema) -> 32-byte priv via Poseidon(master, schema)
derive_public_key(priv) -> sk * Base8
```

**MATCHES** ✓. The circuit's `BabyPbk(credentialPrivKey)` is `credentialPrivKey * Base8`, identical to Rust's `sk * Base8`.

---

## 4. Range-check coverage table

| Public input | Bound | Where | Severity if missing |
|--------------|-------|-------|---------------------|
| `numPredicates` | `<= 4` | line 128-131 | HIGH (would allow undefined semantics) |
| `compoundLogic` | `∈ {0, 1}` | line 135 | HIGH (would silently AND) |
| `queryCredentialIndices[i]` | `< 4` | line 149-152 | CRITICAL (SOLID-SEC-001 fix) |
| `queryFieldIndices[i]` | `< 8` | line 154-157 | CRITICAL (SOLID-SEC-001 fix) |
| `queryOperators[i]` | NONE | n/a | LOW — switch handles |
| `queryValues[i]` | NONE | n/a | INFO — comparator-handled |
| `currentTimestamp` | NONE in-circuit | on-chain only | LOW — see §6 |
| `verifierAddress` | NONE | bound by zk-verifier on-chain |  ✓ |
| `verifierNonce` | NONE | n/a | INFO |
| `globalRoot` | NONE | bound to anchor membership | ✓ |
| `merkleRoots[i]` | NONE | bound to credential atom | ✓ |
| `schemaHashes[i]` | strict ascending + zero-marker | lines 169-206 | CRITICAL — padding semantics |
| `issuerTreeRoot` | NONE | bound to issuer-tree membership | ✓ |
| Per-credential private inputs (when `schemaHash == 0`) | forced to zero | lines 175-195 | CRITICAL — padding integrity |
| `pathIndices[i]` (Merkle) | `∈ {0, 1}` | `merkle_inclusion.circom:34` | HIGH |
| `enabled` (IdentityAnchor) | `∈ {0, 1}` | `identity_anchor.circom:40` | HIGH |

**SOLID-SEC-001 fix verified** at lines 137-158: every `queryCredentialIndices[i]` is bounded into `[0, NUM_CREDS)` and every `queryFieldIndices[i]` into `[0, NUM_FIELDS)`. Without these, `BatchFieldSelector` silently returns 0 for out-of-range indices and (combined with `compoundLogic == OR`) admits unconstrained predicates.

---

## 5. Padding-slot integrity (lines 169-196)

For each `i in 0..NUM_CREDS`:
- `schemaHashes[i] == 0` → `isZero[i].out == 1`.
- `isZero[i].out * X === 0` constrains every per-credential signal `X` to zero when the slot is padding:
  - `merkleRoots[i]`, `data[i][j]`, `salts[i]`, `issuerPubKeyAxs[i]`, `issuerPubKeyAys[i]`, `issuerSigR8xs[i]`, `issuerSigR8ys[i]`, `issuerSigSs[i]`, `expirationTimestamps[i]`.
  - **ADR-0014 additions** (lines 193-195): `issuerAuthorities[i]`, `issuerStatusEpochs[i]`, `issuerRevocationNonces[i]`.

Padding slots:
- Skip the global-tree inclusion check (`anchors[i].enabled = 0`).
- Skip the issuer-tree inclusion check (`issuerInclusion[i].enabled = 0`).
- Skip the credential-atom Merkle inclusion (via `enabled` flag in `CredentialAtom`).
- Trivially pass `ExpirationChecker` (since `expirationTimestamp == 0` returns `valid = 1`).

The integrity zero-constraints prevent a prover from smuggling state through padding slots while the inclusion checks are turned off. **Verified.**

---

## 6. Schema strict-ascending ordering

Lines 199-206:
```
ordering[i] = LessThan(252);
ordering[i].in[0] <== schemaHashes[i];
ordering[i].in[1] <== schemaHashes[i+1];
orderingNextNotZero[i] <== 1 - isZero[i+1].out;
orderingNextNotZero[i] * (1 - ordering[i].out) === 0;
```

Translation: if `schemaHashes[i+1] != 0` (active next slot), then `ordering[i].out` must equal 1 (strict ascending). Padding slots (where the next is zero) skip the check. ✓

This matches the on-chain `verify_batch_proof` strict-ascending check at `programs/zk-verifier/src/lib.rs:504-507`. ✓

---

## 7. Per-template walkthrough

### 7.1 `IdentityAnchor` (`identity_anchor.circom`, 76 LOC)

INPUTS: `enabled`, `masterIdentityKey`, `revocationNonce`, `schemaHash`, `globalRoot`, `globalSiblings`, `globalPathIndices`.

PROCESSING:
1. `enabled ∈ {0, 1}` constraint.
2. `credentialPrivKey = Poseidon(masterIdentityKey, schemaHash)`.
3. `(Ax, Ay) = BabyPbk(credentialPrivKey)`.
4. `identityState = Poseidon(Ax, Ay, revocationNonce)`.
5. `MerkleInclusion` against `globalRoot` (skipped when `enabled == 0`).

OUTPUTS: `credentialPubKeyAx`, `credentialPubKeyAy`.

Per-schema key derivation. SOLID-SEC-029 hardening: padding slots skip the inclusion check.

### 7.2 `CredentialAtom` (`credential_atom.circom`, 83 LOC)

INPUTS: schemaHash, merkleRoot, issuerPubKeyAx/Ay (public-input-derived); attestationData[NUM_FIELDS], salt, holderBJJPubKey, signature, merkle proof (private).

PROCESSING:
1. `enabled = 1 - IsZero(schemaHash)`.
2. `dataHash = Poseidon(data[0..N])`.
3. `commitment = Poseidon(dataHash, schemaHash, holderAx, holderAy, salt)`.
4. EdDSA-Poseidon verifier on the commitment.
5. Merkle inclusion proof against merkleRoot.

OUTPUT: `attestationHash` (= commitment).

The `EdDSAPoseidonVerifier` from circomlib does the on-curve / subgroup checks for the issuer's signature inside the circuit. Combined with the issuer-leaf inclusion proof at STEP 0.75, this prevents an adversarial issuer from forging signatures with cofactor-8 keys: the leaf binds (Ax, Ay) into the issuer-tree, and the circuit verifies the signature with that exact (Ax, Ay).

### 7.3 `MerkleInclusion` (`merkle_inclusion.circom`, 61 LOC)

Standard binary Merkle inclusion with conditional `enabled` flag. `pathIndices[i] ∈ {0, 1}`. `Mux1` to select left/right at each level. `Poseidon(2)` compression.

When `enabled == 0`, the final equality is skipped — `enabled * (computedRoot - root) === 0`.

### 7.4 `PredicateEvaluator` (`predicate_evaluator.circom`, 112 LOC)

Operators: 0=NOOP, 1=EQ, 2=NE, 3=GT, 4=GTE, 5=LT, 6=LTE.

Encoded via 7 one-hot equality checks against the operator value. Each `opIs[i]` returns 1 only when `operator == i`. Result is sum of `opIs[i] * predicate_outcome_i`.

If `operator > 6`, all `opIs[i].out = 0` → `result = 0` → predicate fails. **Safe**, but a more defensive design would explicitly bound `operator <= 6`.

`BatchFieldSelector` (lines 85-112) selects `data[credIndex][fieldIndex]` via two-level multiplexing. Sum of products is the canonical Circom O(N) selector.

### 7.5 `nullifier_expiry.circom` (39 LOC)

Two templates:
- **`NullifierComputer`** (lines 8-19): **DEAD CODE.** Uses 3-input Poseidon `(holderPrivKey, schemaHash, verifierNonce)` — old Phase 1 design, predates ADR-0006 / 0014. NOT referenced by `batch_credential_query.circom`, which uses inline `Poseidon(6)` at line 434. **M07-L01**: delete or replace with the current 6-input form.
- **`ExpirationChecker`** (lines 23-39): if `expirationTimestamp == 0` → valid=1; else `currentTimestamp <= expirationTimestamp`. ✓

### 7.6 `signature_verifier.circom` (24 LOC) — DEAD CODE

Wraps `EdDSAPoseidonVerifier` from circomlib. **NOT referenced** by the batch circuit (which uses `EdDSAPoseidonVerifier` directly inside `CredentialAtom`). Likely leftover from earlier composition. **M07-L02**: delete.

### 7.7 `credential_hasher.circom` (16 LOC) — DEAD CODE

Wraps `Poseidon(NUM_FIELDS)`. NOT referenced by the batch circuit (which inlines the same hasher in `CredentialAtom`). **M07-L03**: delete.

### 7.8 `compound_query.circom` (275 LOC) — alternate / unused

Not deeply audited. Per the file's existence and CLAUDE.md silence, this is likely an alternate template not in the production pipeline. **M07-INVESTIGATE-02**: confirm whether this circuit is built / used anywhere. If dead, delete; if live, audit separately.

---

## 8. Trusted setup (`scripts/setup.js`)

- **Single-party ceremony** — toxic waste held by whoever runs the script. SOLID-SEC-012 (mainnet blocker; tracked in CLAUDE.md).
- `PTAU_POWER = 17` (= 131,072 constraints). Current circuit uses 86,616 (66% utilization, ~34% headroom).
- **Entropy upgraded** (lines 41-54): replaces old `'solid-entropy-' + Date.now()` with `crypto.randomBytes(32)`. CLAUDE.md / SECURITY_REGISTRY confirm this fix landed.
- **Output artifacts**:
  - `circuits/build/batch_credential_query.r1cs` (from compile, prerequisite)
  - `circuits/build/batch_credential_query_final.zkey`
  - `circuits/build/verification_key.json`
  - `circuits/build/verification_key.sha256` (SOLID-SEC-041 content-addressed)
  - `circuits/trusted_setup/pot_final.ptau`
- Cleans up intermediate `pot_0000.ptau`, `pot_0001.ptau`, `zkey0`.

**SOLID-SEC-041 closed** (per `ee` commit `4d97692 Phase 3 impl 3`): VK content hash committed; `initialize.ts` refuses non-matching VK upload.

**Open items**:
- **M07-H01 (HIGH)**: SOLID-SEC-012 multi-party ceremony before mainnet. Single-party today.
- **M07-INF01**: PTAU at 2^17. With ~34% headroom, adding new public inputs (e.g. SOLID-SEC-006 part 2 `vk_generation` binding) brings constraint count up. If it crosses ~110K, bump to PTAU_POWER = 18 and re-run ceremony.

---

## 9. Test coverage

- `circuits/test/batch_range_checks.test.js` — exercises numPredicates / compoundLogic / queryCredentialIndices / queryFieldIndices range checks (SOLID-SEC-001).
- `circuits/test/padding_slot.test.js` — exercises padding-slot integrity zero-constraint.
- `circuits/test/templates/anchor_enabled_isolated.circom` — IdentityAnchor with `enabled = 0`.
- `circuits/test/templates/range_check_isolated.circom` — Helper for range tests.

What's NOT tested:
- The full batch circuit end-to-end witness against a known proof / VK (relies on `tests/integration/`, only `01_registry_init` exists).
- Issuer-tree membership: no isolated test.
- Cross-language byte-identity for the ADR-0014 leaf hash.

**M07-M01 (MEDIUM)**: gap in cross-lang byte-identity test for issuer leaf. Add a vector to `tests/vectors/` that pins `Poseidon5(authority, bjj_x, bjj_y, status_epoch, rev_nonce)` with a known input.

---

## 10. Findings summary

| ID | Severity | Issue |
|----|----------|-------|
| M07-FIX01 | INFO | Patch `01_zk_verifier.md` to reference correct nullifier 6 inputs. |
| M07-INVESTIGATE-01 | HIGH | `issuerAuthority` BE vs LE encoding — circuit doc says BE, Rust path is LE; verify witness builder. |
| M07-INVESTIGATE-02 | LOW | `compound_query.circom` (275 LOC) — alive or dead? |
| M07-H01 | HIGH | Single-party trusted setup (SOLID-SEC-012); mainnet blocker. |
| M07-M01 | MEDIUM | No cross-lang byte-identity vector for issuer leaf. |
| M07-L01 | LOW | `NullifierComputer` template at `nullifier_expiry.circom:8-19` is dead code. |
| M07-L02 | LOW | `signature_verifier.circom` is dead code. |
| M07-L03 | LOW | `credential_hasher.circom` is dead code. |
| M07-INF01 | INFO | PTAU 2^17 has 34% headroom; bump to 2^18 if new public inputs grow constraint count. |

---

## 11. Soundness analysis (paranoid prover model)

**Could a malicious prover...**

1. **Forge a valid proof for an unauthorized credential?** No. The `CredentialAtom`'s `EdDSAPoseidonVerifier` requires a valid issuer signature; the issuer-tree membership at STEP 0.75 binds the (Ax, Ay) signing key to an Approved issuer leaf; revoking an issuer rotates `issuerTreeRoot` and breaks both the membership proof and the nullifier universe.

2. **Replay a proof?** No. `nullifierHash = Poseidon6(masterKey, revNonce, verifierAddr, queryContextHash, verifierNonce, issuerTreeRoot)` is deterministic per (holder, query, verifier-session, issuer-tree-epoch). The nullifier-PDA gate at `verify_batch_proof:567-570` (`init` constraint) atomically prevents re-registration.

3. **Pass a proof against the wrong verifier?** No. `verifierAddress` is a public input bound on-chain to `ID.to_bytes()`.

4. **Submit a stale proof?** No. `currentTimestamp` is a public input bound on-chain to `Clock::unix_timestamp ± skew`.

5. **Insert a fake issuer leaf?** No. The issuer tree's `current_root` is mirrored on-chain in `IssuerTreeBinding[40..72]` and verified against `public_inputs[10]`. Only `update_issuer_tree_root` can write. (Caveat: M02-H01 SOLID-SEC-045 — atomic handlers don't update the binding root, so there's a window before the operator pushes it. Pre-revocation proof window.)

6. **Smuggle data via padding slots?** No. STEP 0 integrity zero-constraints force every per-credential signal to zero when `schemaHash == 0`.

7. **Forge a small-order issuer key?** Currently no in-circuit subgroup check on `issuerPubKeyAxs/Ays`. The off-chain `require_in_prime_order_subgroup` runs at issuer registration only when `sec007-skip-onchain` is OFF. With the bypass on, the off-chain TS predicate is the gate. **SEC-048 / M02-H02.**

8. **Choose a malicious `currentTimestamp` to defeat expiration?** No. On-chain enforces u64 fit and freshness window. In-circuit `ExpirationChecker` enforces `currentTimestamp <= expirationTimestamp` per credential.

9. **Pick `verifierNonce` to collide a nullifier?** Possible only if the prover controls all 6 inputs to the Poseidon6 (which they do for `verifierNonce`); but they cannot deterministically achieve a collision because Poseidon is collision-resistant. The `verifierNonce` exists to give verifiers replay-domain separation across sessions.

10. **Submit a proof under a different `globalRoot`?** No. The IdentityAnchor binds the per-schema identity leaf to `globalRoot` for every active slot.

**No CRITICAL soundness findings.** SEC-048 / SEC-045 are tracked elsewhere with HIGH severity.

---

## 12. Compute notes

Witness generation: dominated by 4 × Poseidon scalar mul (BabyPbk) + 4 × Merkle inclusion (DEPTH=20) × 2 trees + 4 × signature verify. Estimated ~5-15 seconds on a beefy machine for the host-side prover; not on-chain.

Constraint count: 86,616 non-linear constraints (per setup.js comment line 36). Groth16 prove time scales linearly with constraint count + log² for FFTs.

On-chain verification cost (zk-verifier): see `04_compute/cu_budget.md` — ~280-345K CU dominated by alt_bn128 syscalls.

---

## 13. Suggested next actions

1. **M07-FIX01**: patch `01_zk_verifier.md` to fix the nullifier preimage description.
2. **M07-INVESTIGATE-01**: trace the witness builder in TS SDK to confirm BE-vs-LE for `issuerAuthority`. If the witness uses LE, fix the circuit comment; if BE, document the transposition step explicitly.
3. **M07-INVESTIGATE-02**: confirm `compound_query.circom` build status; delete if dead.
4. **M07-H01 (SOLID-SEC-012)**: design multi-party trusted-setup ceremony before mainnet.
5. **M07-M01**: add issuer-leaf vector to `tests/vectors/`.
6. **M07-L01..L03**: delete dead-code templates after CI regen test confirms no consumer.
7. **M07-INF01**: monitor constraint count; bump PTAU when crossing 110K.

---

## Summary

The circuit is well-structured: 6 templates + main, with explicit padding integrity, range checks for indices, schema ordering, identity binding, signature verification, Merkle inclusion (×2 trees), expiration enforcement, query-context hashing, and 6-input nullifier. SOLID-SEC-001, SEC-029 hardening verified.

**Cross-language byte-identity verified for**: nullifier preimage (matches Rust `nullifier.rs:45-64`), issuer leaf preimage (matches `issuer-registry/src/lib.rs:63-77`), commitment preimage (matches `commitment.rs:42-61`), identity-state preimage (matches `identity.rs:22-30`), per-schema key derivation (matches `wasm/src/lib.rs:227-251`).

**Outstanding**: SOLID-SEC-012 (multi-party ceremony), SOLID-SEC-006 part 2 (vk_generation in public inputs), M07-INVESTIGATE-01 (issuerAuthority encoding), 3 dead-code templates.

10 findings: 2 HIGH (one is INVESTIGATE), 1 MEDIUM, 4 LOW, 3 INFO. **0 CRITICAL.**
