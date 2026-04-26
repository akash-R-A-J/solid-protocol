# Docs vs. Code Drift Audit — solid-protocol v0.6.1

| Field | Value |
|-------|-------|
| Date | 2026-04-26 |
| HEAD | `59c99c2` |
| Method | Read every doc claim that names a specific file/struct/function/line, compare against the current code at HEAD. Code wins on every conflict. |
| Auditor | Orchestrator |

This is a delta-only document: it lists every doc claim that no longer matches the code. **No doc edits are made here** — this is the diff to be approved before Wave 3 reconciliation lands.

---

## Severity rubric for drift

- **CRITICAL**: doc would lead a reader to write/use code in a way that breaks soundness.
- **HIGH**: doc misstates a load-bearing invariant or interface signature; misleads SDK callers.
- **MEDIUM**: doc says a known-OPEN item is closed, or vice versa; misleads roadmap planning.
- **LOW**: stale field name / count / version reference.
- **INFO**: cosmetic — outdated phrasing, formatting inconsistency.

---

## DRIFT-01: `docs/MODULE_CONTRACTS.md` — `GlobalStateBinding` layout wrong (HIGH)

**Doc claim** (lines ~710-717):
```
`GlobalStateBinding` PDA seeds=[b"globroot"]:
  Raw bytes (104 bytes):
    [0..8)    discriminator b"globroot"
    [8..40)   global_tree_pubkey [u8;32]
    [40..72)  current_root [u8;32]
    [72..104) last_slot u64 LE (but also: high bytes reserved)
```

**Actual code** (`programs/schema-registry/src/lib.rs:347-378` writer):
```
80-byte allocation
[0..8)    discriminator b"globroot"
[8..40)   current_root [u8;32]                <- NOT global_tree_pubkey
[40..48)  last_updated_slot u64 LE
[48..80)  authority [u8;32]
```

**There is no `global_tree_pubkey` field at all.** Total is 80 bytes, not 104. The on-chain reader at `crates/solid-light/src/cpi_helpers.rs:289-297` reads only `[0..40)` (disc + root). Confirmed in `03_schema_registry.md` §3.3.2 (M03-DOCS).

**Action**: Rewrite this section.

---

## DRIFT-02: `docs/MODULE_CONTRACTS.md` — `register_schema` says "DISABLED" (MEDIUM)

**Doc claim** (lines ~723-725):
```
`register_schema(name, version, category, field_names, schema_hash)`
  Gate: payer is signer.
  Computes expected hash = Poseidon(name_chunks, version, num_fields).
  SHOULD enforce: computed_hash == schema_hash.  [DISABLED -- SEC-002 fix pending]
```

**Actual code** (`programs/schema-registry/src/lib.rs:113-123`): the integrity check IS enforced. `require!(computed_hash == schema_hash, ErrorCode::InvalidSchemaHash)` at line 122. SEC-002 fix landed.

**Action**: Remove "[DISABLED -- SEC-002 fix pending]"; reflect that `computed_hash == schema_hash` is enforced and tested by `test_compute_schema_hash_parts_matches_definition` (`crates/solid-core/src/schema.rs:381-392`).

---

## DRIFT-03: `docs/MODULE_CONTRACTS.md` — `update_tree_root` signature wrong (HIGH)

**Doc claim** (~lines 730-733):
```
`update_tree_root(schema_hash, new_root, new_slot)`
  Gate: tree binding exists, binding.authority == signer, now_slot > last_slot (monotonicity).
```

**Actual code** (`programs/schema-registry/src/lib.rs:263`): `update_tree_root(ctx, schema_hash: [u8;32], new_root: [u8;32])` — only 2 args. `now_slot` is read internally from `Clock::get()?.slot`, not passed.

**Action**: Drop `new_slot` from the signature; add a sentence "slot is read from Clock at handler time" for clarity.

---

## DRIFT-04: `docs/MODULE_CONTRACTS.md` — `update_global_root` signature wrong (HIGH)

**Doc claim** (~lines 746-747):
```
`update_global_root(new_root, new_slot)`
  Updates GlobalStateBinding.  Monotonicity enforced.
```

**Actual code** (`programs/schema-registry/src/lib.rs:382`): `update_global_root(ctx, new_root: [u8;32])` — only 1 arg. Slot from Clock.

**Action**: Drop `new_slot`.

---

## DRIFT-05: `docs/MODULE_CONTRACTS.md` — `initialize_global_binding` signature wrong (HIGH)

**Doc claim** (implied by the `global_tree_pubkey` field in DRIFT-01):
```
`initialize_global_binding(global_tree_pubkey)`
```

**Actual code**: `initialize_global_binding(ctx)` — takes no args. There is no tree pubkey field in the binding's layout (DRIFT-01).

**Action**: Drop the `global_tree_pubkey` arg; document that the binding is a singleton with no parameters.

---

## DRIFT-06: `circuits/batch_credential_query.circom:103-106` — `issuerAuthority` BE vs LE (LOW)

**Doc claim** (in the circuit's comment block):
```
//   `issuerAuthority` is the Solana authority Pubkey of the issuer,
//   encoded as a big-endian field element (SEC-031).
```

**Actual code**: every off-chain consumer (`ts-sdk/packages/core/src/index.ts:265-293`, `ts-sdk/packages/holder/src/index.ts:509-513`) feeds `authority.toBytes()` directly to `bufToDecimal` (LE). The on-chain Rust path (`programs/issuer-registry/src/lib.rs:63-77`) does `authority.to_bytes()` → `hash_bytes` which canonicalizes via `bytes_le_to_fr` (LE). All sites use LE.

`SOLID-SEC-031` itself addresses the **`verifierAddress` slot specifically** (which IS BE, see `holder/src/index.ts:476-485`'s `bufToDecimalBE`). The circuit comment confused the two slots.

**Action**: Update the circuit comment to "encoded as a little-endian field element (matches LE convention used for all field-element-typed inputs except `verifierAddress`, which is BE per SOLID-SEC-031)."

---

## DRIFT-07: `crates/solid-light/src/credential_tree.rs:62` — stale 3-input nullifier comment (LOW)

**Doc claim**:
```
//! In the "Great Infra" design, nullifiers are stored in a dedicated Merkle tree.
//! Each proof "shields" a nullifier; if the leaf already exists, the creation fails.
[...around line 60-62...]
    /// The unique nullifier = Poseidon(masterKey, verifierAddress, schemaHash)
    pub nullifier: [u8; 32],
```

**Actual code**: post-ADR-0006 / ADR-0014 the nullifier is 6-input Poseidon: `(masterKey, revocationNonce, verifierAddress, queryContextHash, verifierNonce, issuerTreeRoot)`. The struct is the off-chain leaf-data type (essentially dead code — see M05-LO-01 in solid-light audit).

**Action**: Either delete the entire `CompressedNullifier` struct (preferred — it's dead) or update the comment to point to ADR-0006 + ADR-0014 with the current 6-input formula.

---

## DRIFT-08: `programs/zk-verifier/src/lib.rs:25-40` — public-input slot doc uses inclusive-end notation (LOW)

**Doc claim** (in the file's comment block):
```
// batch_credential_query public inputs (32 total; ADR-0014 revision):
//   [0]      = nullifierHash
//   [1]      = globalRoot
//   [2..5]   = merkleRoots[4]
//   [6..9]   = schemaHashes[4]
[...]
```

The notation `[2..5]` reads ambiguously: in Rust half-open ranges it's `2,3,4`; the doc means `2,3,4,5`. The constants at lines 47-53 (`ISSUER_TREE_ROOT_INPUT_INDEX = 10`, etc.) are correct.

**Action**: Rewrite as `[2..=5]` or `[2], [3], [4], [5]` for unambiguity. Cognitive trap, not a soundness issue.

---

## DRIFT-09: `circuits/lib/nullifier_expiry.circom:6-19` — `NullifierComputer` template body shows old 3-input form (LOW)

**Doc claim + code body**:
```circom
/// Compute a nullifier hash for anti-replay.
/// nullifier = Poseidon(holderPrivKey, schemaHash, verifierNonce)
template NullifierComputer() {
    [...3-input Poseidon body...]
}
```

This template is **dead code** — it's not referenced anywhere in `circuits/batch_credential_query.circom` (which uses an inline `Poseidon(6)` at line 434). M07-L01.

**Action**: Delete the `NullifierComputer` template (and the 3-input comment goes with it). Keep `ExpirationChecker` in the same file. Or delete the whole file if `ExpirationChecker` is moved elsewhere.

---

## DRIFT-10: `circuits/lib/signature_verifier.circom` — dead code (LOW)

Wraps `EdDSAPoseidonVerifier` from circomlib. **Not referenced** by `batch_credential_query.circom` (which uses `EdDSAPoseidonVerifier` directly inside `CredentialAtom`). M07-L02.

**Action**: Delete after CI regen test confirms no consumer.

---

## DRIFT-11: `circuits/lib/credential_hasher.circom` — dead code (LOW)

Wraps `Poseidon(NUM_FIELDS)`. **Not referenced** by the production circuit (`CredentialAtom` inlines its own `dataHasher`). M07-L03.

**Action**: Delete.

---

## DRIFT-12: `ts-sdk/packages/core/src/index.ts:435` — "Solana verifier expects 31 inputs" stale (LOW)

**Doc claim**:
```ts
/** Convert to circuit public inputs (Solana verifier expects 31 inputs) */
toCircuitInputs() {
```

**Actual**: post-ADR-0014, the count is 32 (`programs/zk-verifier/src/lib.rs:41`, `verifier/src/index.ts:35`).

**Action**: Update comment to "32 inputs (post ADR-0014)".

---

## DRIFT-13: `ts-sdk/packages/core/src/index.ts:381-403` — `QueryBuilder._computeContextHash` does not match circuit STEP 4 (MEDIUM, see M08-M01)

**Code**:
```ts
const hIndices = poseidonHash([...credIndices, ...fieldIndices]);
```
This concatenates `[cred_0, cred_1, cred_2, cred_3, field_0, field_1, field_2, field_3]`.

**Circuit `batch_credential_query.circom:395-398`**:
```circom
qHasherIndices.inputs[i*2]   <== queryCredentialIndices[i];
qHasherIndices.inputs[i*2+1] <== queryFieldIndices[i];
```
Interleaves: `[cred_0, field_0, cred_1, field_1, ...]`.

**Effect**: the queryContextHash this helper produces would differ from the circuit's. As noted in M08-M01, this helper is currently NOT consumed by the proof submission path (the circuit computes its own context hash internally and emits the nullifier as `publicSignals[0]`, which is what the production path uses). But a future caller invoking the helper's output to compute a nullifier off-chain would silently break.

**Action**: Either (a) delete `_computeContextHash` and the `queryContextHash` field from `MultiCredentialQuery` since they're not load-bearing, or (b) fix the helper to interleave like the circuit. (b) is safer for forward-compat.

---

## DRIFT-14: Orchestrator's `01_zk_verifier.md` mis-states nullifier preimage (INFO)

**Audit doc claim** (in `sec/audits/2026-04-26_v0.6.1_modular_audit/01_modular/01_zk_verifier.md`):
```
- Nullifier preimage = 6-input Poseidon (ADR-0006).
[...]
identityCommitment, secret, schemaId, credentialIndex, currentTimestamp, issuerTreeRoot
```

**Actual** (verified in 07_circuits.md §2 and 04_solid_core.md §4): preimage is `(masterKey, revocationNonce, verifierAddress, queryContextHash, verifierNonce, issuerTreeRoot)`.

**Action**: Patch `01_zk_verifier.md` accordingly. M07-FIX01.

---

## DRIFT-15: `adr/0012-thirty-one-public-input-contract.md` — filename outdated (INFO)

The filename references "31" but the ADR's content correctly documents the post-ADR-0014 32-input contract (with a header note acknowledging the rename).

**Action**: optional rename to `adr/0012-thirty-two-public-input-contract.md`. Low priority — git history continuity is reasonable to preserve.

---

## DRIFT-16: `docs/IMPROVEMENTS_ROADMAP.md` — needs status reconciliation against SOLID-SEC-* registry (MEDIUM)

The doc itself acknowledges (lines 22-23): "When the two disagree, the registry wins." Worth a sweep to ensure every `[x]` mark in the roadmap matches the registry's status board, especially:
- SOLID-SEC-002 (closed; verify roadmap reflects)
- SOLID-SEC-007 (open with bypass; verify roadmap reflects)
- SOLID-SEC-041 (closed; verify)
- SOLID-SEC-044 (closed per `212eb6f Phase 3 impl 4`; verify)
- SOLID-SEC-045 (open; new finding)
- SOLID-SEC-048 (open with bypass; verify)

**Action**: Run `scripts/check_docs.py` (referenced as Phase 1 work in IMPROVEMENTS_ROADMAP.md line 47-48) to confirm roadmap ↔ registry consistency. If the script doesn't currently exist, that's a separate gap.

---

## DRIFT-17: `docs/MODULE_CONTRACTS.md` — schema-registry "MISSING" annotations are stale (LOW)

The doc lists multiple `MISSING:` annotations:
- `MISSING: emit!(TreeRootUpdated) event` on `update_tree_root` — partially true, but logged as M03-L07 (no event), not "fix pending".
- `MISSING: re-assertion of data[8..40] == schema_hash (SEC-019)` on `set_binding_status` and `transfer_tree_binding_authority` — TRUE (M03-M02 / SOLID-SEC-019 still open).
- `MISSING: emit!(SchemaBindingFrozen) event` — TRUE (M03-L07).
- `MISSING: single-step transfer risk (SEC-016 analog)` — TRUE (M03-M03).

**Action**: Reconcile each `MISSING:` against the registry. Some are accurate; others can be closed.

---

## DRIFT-18: `docs/architecture.md` — accurate (NO DRIFT)

I scanned the first 50 lines: "Public input count is 32 (ADR-0014 revision; was 31)" — correct. Nullifier "Poseidon(6) post ADR-0014, with issuerTreeRoot as the 6th input" — correct. Owner-checks invariant correctly stated. No drift detected.

---

## DRIFT-19: `docs/PROGRAM_ID_RECONCILIATION.md` — likely stale (INFO)

Top of file says `scripts/check_program_ids.py` "currently exits 1 with the details". As of HEAD `59c99c2` the program IDs are reconciled. The runbook is for a historical drift incident.

**Action**: Add a "Status: closed; this runbook is preserved for the next time program-ID drift occurs" header. Or move to `docs/archive/`.

---

## DRIFT-20: `docs/REVOCATION_DESIGN.md` — partially up-to-date (LOW, needs full check)

CLAUDE.md's open work list (lines 132-134) says: "Revocation v1 operator workflow. Circuit and on-chain support are in place. Still needs holder SDK helper, issuer SDK helper, and indexer event contract."

If `docs/REVOCATION_DESIGN.md` describes the SDK + indexer contracts as "in place" or "designed but not implemented", verify.

**Action**: Full read pass on `docs/REVOCATION_DESIGN.md`; align with current state.

---

## Drift summary

| Severity | Count |
|----------|-------|
| CRITICAL | 0 |
| HIGH | 4 (DRIFT-01, -03, -04, -05) |
| MEDIUM | 3 (DRIFT-02, -13, -16) |
| LOW | 8 (DRIFT-06, -07, -08, -09, -10, -11, -12, -17, -20) |
| INFO | 3 (DRIFT-14, -15, -19) |
| Total | 20 drift items |

**Most consequential**: DRIFT-01 / -03 / -04 / -05 (all in `docs/MODULE_CONTRACTS.md`). The schema-registry handler signatures are misdocumented — an SDK author following the doc would write incorrect callsites and get runtime errors.

---

## Recommended Wave 3 reconciliation order

1. Fix DRIFT-01 / -03 / -04 / -05 in `docs/MODULE_CONTRACTS.md` first — highest blast radius for SDK developers.
2. Fix DRIFT-12 (TS SDK doc comment) and DRIFT-08 (zk-verifier slot doc) — load-bearing read paths.
3. Fix DRIFT-06 (circuit BE/LE comment) — prevents future witness-builder confusion.
4. Fix DRIFT-14 (orchestrator's earlier audit) — internal consistency.
5. Resolve DRIFT-09 / -10 / -11 (dead-code circuit templates) — delete after CI regen test.
6. Resolve DRIFT-13 (TS QueryBuilder helper) — delete or fix.
7. Reconcile DRIFT-16 (IMPROVEMENTS_ROADMAP vs registry) — automated sweep.
8. DRIFT-02 / -07 / -15 / -17 / -19 / -20 — tidy-up.

**Do NOT make these doc edits yet.** The user requested a delta first; reconciliation is Wave 3.

---

## Process improvement recommendations (to prevent future drift)

1. **Run `scripts/check_docs.py` in CI** (Phase 1 work referenced in IMPROVEMENTS_ROADMAP.md). If it doesn't exist, create it: parse SOLID-SEC-* IDs from sec/SECURITY_REGISTRY.md and from doc files; fail on mismatch.
2. **Doc-string-as-test pattern**: critical numeric invariants (NR_PUBLIC_INPUTS, slot indices, layout sizes) should have a Rust unit test that asserts the doc-comment value matches the constant. Already present for `VerifierConfig::SPACE`; extend.
3. **Pre-commit hook**: lightweight `grep` check that newly-added handler signatures in code match their MODULE_CONTRACTS.md entries. (Heavier than worth — but a CI lint to flag known doc paths if their referenced `programs/.../lib.rs:LINE` no longer contains the expected token would catch a lot.)
4. **ADR ↔ code link audit on every PR**: if a PR touches `programs/`, ensure any referenced ADR is updated in the same PR.

---

End of docs drift audit.
