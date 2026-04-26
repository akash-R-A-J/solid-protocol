# Audit 03 - schema-registry

Module: `programs/schema-registry`
Program ID: `4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1`
Version: v0.6.1 (HEAD `59c99c2` on 2026-04-26)
LOC in scope: 599 (`programs/schema-registry/src/lib.rs`).
Auditor: delegated specialist (agent), persisted by orchestrator.

The file has no in-program test module and no `[dev-dependencies]`. Coverage relies on caller-side and `solid-core` tests.

---

## 1. Header

The schema-registry program is the protocol's metadata authority and the on-chain root mirror for every credential tree. Its responsibilities, in v0.6.1:

1. **Schema registration** (`register_schema`) — validates that a caller-supplied `schema_hash` matches the canonical Poseidon derivation over `(name, version, field_count)` and stores the schema's metadata in a typed `SchemaAccount` PDA.
2. **Schema lifecycle** (`deprecate_schema`, `increment_usage`) — authority-gated mutations on `SchemaAccount`.
3. **Per-schema tree binding** (`initialize_tree_binding`, `update_tree_root`, `set_binding_status`, `transfer_tree_binding_authority`) — manages a 145-byte `SchemaTreeBinding` PDA per schema.
4. **Global state binding** (`initialize_global_binding`, `update_global_root`, `transfer_global_binding_authority`) — manages a singleton 80-byte `GlobalStateBinding` PDA mirroring the global identity-state tree's root.

It does NOT:
- CPI into `spl-account-compression`.
- Verify proofs.
- Hold any value.
- Emit any events. (`emit!()` is unused in this file.)

Counterpart consumers verified:
- `programs/issuer-registry/src/lib.rs:15,1969-1993,2322` consumes `SchemaAccount` and `SchemaTreeBinding`.
- `programs/zk-verifier/src/lib.rs:438-456,485-543,864-873` consumes `GlobalStateBinding` and `SchemaTreeBinding`.
- `crates/solid-light/src/cpi_helpers.rs:236-298,320-391` is the byte-level parser contract.
- `crates/solid-core/src/schema.rs:139-156` is the off-chain mirror of the schema-hash derivation.
- `scripts/initialize.ts:170-234` is the canonical bootstrap caller.
- `ts-sdk/packages/sdk/src/index.ts:162-198` is the canonical SDK reader for `SchemaAccount`.

`Anchor.toml:12,17` and `programs/schema-registry/src/lib.rs:4` agree on `4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1`.

---

## 2. Crate / build configuration audit

`programs/schema-registry/Cargo.toml` (29 lines).

- `crate-type = ["cdylib", "lib"]` — required for Anchor multi-crate workspaces.
- `name = "schema_registry"` (lib name, underscore) vs `name = "schema-registry"` (package name, hyphen). Matches `Anchor.toml:12`.
- Features (`no-entrypoint`, `no-idl`, `no-log-ix-name`, `cpi`, `idl-build`) are the standard Anchor-emitted set.
- `[dependencies]`: exactly two entries — `anchor-lang` (workspace `0.30.1`) and `solid-core` (path).
- `[dev-dependencies]` is **absent**. No in-program unit tests. (See M03-M01.)

### 2.2 The removed `light-poseidon` dependency

The 8-line comment block at `programs/schema-registry/Cargo.toml:21-28` documents removal of a direct `light-poseidon = "0.2"` dep. Cross-checked:

- `grep` of `light_poseidon|light-poseidon` in `programs/schema-registry/src/` returns zero hits.
- The schema-hash code path: `programs/schema-registry/src/lib.rs:119-122` calls `solid_core::schema::compute_schema_hash_from_parts` -> `crates/solid-core/src/schema.rs:155` calls `poseidon::hash_fields_to_bytes` -> `crates/solid-core/src/poseidon.rs:178-190` calls `hash_bytes` -> `crates/solid-core/src/poseidon.rs:138-148` (BPF + non-wasm host) dispatches to `solana_program::poseidon::hashv` (the `sol_poseidon` syscall on BPF).

The claim is exact: BPF binaries never load a `light_poseidon` parameter table.

---

## 3. State accounts

### 3.1 `SchemaAccount` (typed Anchor account)

Definition `programs/schema-registry/src/lib.rs:556-567`:

```rust
#[account]
pub struct SchemaAccount {
    pub authority: Pubkey,
    pub name: String,
    pub version: u8,
    pub category: String,
    pub field_names: Vec<String>,
    pub schema_hash: [u8; 32],
    pub deprecated: bool,
    pub created_at: i64,
    pub usage_count: u64,
}
```

#### 3.1.1 PDA seeds

`seeds = [b"schema", name.as_bytes(), &[version]]`. One PDA per `(name, version)` pair.

#### 3.1.2 SPACE accounting

`SCHEMA_ACCOUNT_SPACE = 510 bytes`; with discriminator = **518 bytes total allocation**.

Length caps: `name <= 64`, `category <= 64`, `field_names.len() <= 8`, `each fname.len() <= 32`. Enforced before Borsh serialization.

**Cross-check**: TS SDK at `ts-sdk/packages/sdk/src/index.ts:178-192` walks the same Borsh layout. **No drift.**

#### 3.1.3 Field semantics

- `authority` is set ONCE at registration. **No `transfer_schema_authority` instruction**. A lost authority key is unrecoverable (M03-L02).
- `version: u8` permits 0..=255.
- `field_names` are NOT used in `compute_schema_hash_from_parts` — only the count is.
- `deprecated` is one-way (no `un_deprecate`).
- `usage_count` uses `checked_add`.

### 3.2 `SchemaTreeBinding` (raw 145-byte account)

```
offset  0..  8 : discriminator = b"schmtree"           (literal ASCII)
offset  8.. 40 : schema_hash                           ([u8; 32])
offset 40.. 72 : tree_pubkey                           ([u8; 32])
offset 72..104 : current_root                          ([u8; 32])
offset 104..112: last_updated_slot                     (u64 LE)
offset 112..113: status                                (u8: 0=Active, 1=Frozen)
offset 113..145: authority                             ([u8; 32])
```

#### 3.2.1 Writer / reader asymmetry

Writer allocates **145 bytes**. Reader at `crates/solid-light/src/cpi_helpers.rs:259` requires **at least 113 bytes** and reads only `[0..113)`. The trailing `authority` field at `[113..145)` is **forward-compatible**.

#### 3.2.2 Discriminator

Literal ASCII `b"schmtree"`. NOT an Anchor-derived discriminator. Mirrored exactly in `crates/solid-light/src/cpi_helpers.rs:245`.

If anyone changes the literal, parser-side cross-language tests catch it. **The writer side has NO direct test, so a one-letter typo would silently break every binding (M03-M01).**

#### 3.2.3 PDA seeds

`seeds = [b"schema-tree-binding", schema_hash.as_ref()]`. One binding PDA per `schema_hash`.

### 3.3 `GlobalStateBinding` (raw 80-byte account)

```
offset  0..  8 : discriminator = b"globroot"           (literal ASCII)
offset  8.. 40 : current_root                          ([u8; 32])
offset 40.. 48 : last_updated_slot                     (u64 LE)
offset 48.. 80 : authority                             ([u8; 32])
```

#### 3.3.1 Reader/writer asymmetry

Writer allocates 80 bytes. Reader requires at least 40 bytes and reads `[0..40)`. The `last_updated_slot` and `authority` fields are forward-compatible.

#### 3.3.2 Drift hazard: layout vs `MODULE_CONTRACTS.md`

`docs/MODULE_CONTRACTS.md:710-717` documents `GlobalStateBinding` as 104 bytes with a `global_tree_pubkey [u8;32]` field at `[8..40)`. **This is wrong.** Actual writer at `programs/schema-registry/src/lib.rs:347-377` allocates 80 bytes total. (M03-DOCS.)

### 3.5 Manual creation pattern

Both raw-byte accounts use `system_instruction::create_account` + `invoke_signed`. Owner set to `crate::ID`. Allocation size is the full writer-side size. Re-initialization protected by System program.

### 3.6 Mutator-handler defensive checks

For every raw-byte account write/update handler, the standard pattern is:
1. `binding_info.owner == crate::ID`
2. `data.len() >= SIZE`
3. `data[0..8] == DISCRIMINATOR`
4. (For `SchemaTreeBinding` only) `data[8..40] == schema_hash` re-assertion
5. (For root updates) Monotonic slot enforcement
6. Authority byte-comparison from stored offset

Two handlers (`set_binding_status` and `transfer_tree_binding_authority`) **omit** step 4. SOLID-SEC-019, MEDIUM. See M03-M02.

---

## 4. Schema hash: structure, ordering, dual-target byte-identity

### 4.1 The on-chain integrity check (`programs/schema-registry/src/lib.rs:113-123`)

```rust
let computed_hash =
    solid_core::schema::compute_schema_hash_from_parts(&name, version, field_names.len())
        .map_err(|_| error!(ErrorCode::PoseidonFailed))?;
require!(computed_hash == schema_hash, ErrorCode::InvalidSchemaHash);
```

This is the post-SOLID-SEC-002-fix version. **Verified correct.**

### 4.2 The shared derivation function (`crates/solid-core/src/schema.rs:139-156`)

Preimage rules:
1. Name chunked into 8-byte LE u64s; last chunk right-padded with zeros.
2. Version as u64.
3. Field count (usize) as u64.
4. Truncate to 16.
5. Dispatch.

With on-chain length caps (name <= 64 bytes => <= 8 chunks; total <= 10 inputs), truncation never fires. `hash_fields_to_bytes` itself caps at **12**. The 16 vs 12 mismatch is a doc nit (M03-LOW).

### 4.3 Dual-target byte-identity

The Poseidon dispatcher routes:
- BPF: `solana_program::poseidon::hashv` -> `sol_poseidon` syscall, parameters `Bn254X5`, endian `LittleEndian`.
- Non-wasm host: same `solana_program::poseidon::hashv` (light-poseidon-backed pure-Rust fallback).
- wasm32: directly `light_poseidon::Poseidon<Fr>::new_circom().hash_bytes_le`.

All three converge by parameter agreement.

### 4.4 The off-chain (SDK) computation path

`scripts/initialize.ts:159-169` constructs the same input vector by hand. Byte-identical to the Rust derivation by construction.

**Audit observation.** This logic is **inlined in scripts** rather than wrapped in a TS helper. There is no `wasm`-bridge export named `compute_schema_hash`. Drift hazard. (M03-L03.)

### 4.6 Findings against the schema-hash path

- **M03-LOW** — 16-input truncation in `compute_schema_hash_from_parts` is misleading (Poseidon caps at 12).
- **M03-DOCS** — `MODULE_CONTRACTS.md:723-725` says `register_schema` "DISABLED -- SEC-002 fix pending". Stale.
- **M03-L03** — TS-side derivation is inlined in scripts, not a callable SDK helper.

---

## 5. SPL-AC integration

**There is no SPL-AC CPI in `programs/schema-registry/src/lib.rs`.** Verified by grep. The SPL-AC CPI lives in `programs/issuer-registry/src/lib.rs` and TypeScript scripts.

Schema-registry's role is to:
1. Hold the off-chain-supplied tree pubkey at `SchemaTreeBinding[40..72)`.
2. Mirror the off-chain-computed Merkle root at `SchemaTreeBinding[72..104)` via `update_tree_root`.
3. Provide an authoritative on-chain reference for the verifier.

ADR-0003 documents the depth-20 choice. Schema-registry does NOT enforce a depth.

---

## 6. Per-instruction-handler deep dive

### 6.1 `register_schema` (lines 86-137)

Defensive sequence: field-count cap (1..=8), name length cap (<=64), category length cap (<=64), per-field-name length cap (<=32), schema-hash integrity check, field assignments.

**Authority model:** Permissionless. Anti-spam only via rent (M03-L04).

**Idempotency:** Anchor `init` ensures `(name, version)` cannot be registered twice.

### 6.2 `deprecate_schema` (lines 139-145)

`schema_account.authority == authority.key()` constraint. Sets `deprecated = true`. Idempotent. One-way.

### 6.3 `increment_usage` (lines 147-166)

Post-fix version. Defensive sequence: deprecation gate, authority gate, `checked_add(1).ok_or(Overflow)?`. **No log, no event** (M03-L07).

### 6.4 `initialize_tree_binding` (lines 188-249)

Defensive sequence: authority gate, schema-hash sanity, allocate via System ix, write 145-byte layout, `msg!()`.

**Tree-pubkey provenance:** Handler does NOT validate that `tree_pubkey` exists, is owned by `spl-account-compression`. Once written, immutable (M03-L05).

### 6.5 `update_tree_root` (lines 263-309)

Six-step gate: owner check, length, discriminator, schema-hash re-assertion, status active, authority. Slot strictly monotonic. Write `[72..104)` and `[104..112)`. **No log, no event** (M03-L07).

**Edge cases:**
- Same-slot replay: rejected.
- `new_root == [0; 32]`: accepted; verifier treats zero as skip.
- Indexer race: loser fails with `RootSlotNotMonotonic`.

`try_into().unwrap()` patterns at lines 291, 301, 334, 398, 405, 436, 462. All length-guaranteed by prior gate. Statically safe.

### 6.6 `set_binding_status` (lines 314-341)

Reuses `UpdateTreeRoot` accounts. Status validity, owner check, length+disc, authority, write status byte.

**M03-M02 (SOLID-SEC-019):** OMITS schema-hash re-assertion. Two-line fix.

**M03-LOW: No timelock on unfreeze** (SOLID-SEC-035 open).

### 6.7 `transfer_tree_binding_authority` (lines 420-444)

Reuses `UpdateTreeRoot` context. Owner, length+disc, authority, write new authority bytes. `msg!()`.

**M03-M02** schema-hash gap. **M03-M03**: single-step transfer (SOLID-SEC-016 analog). `Pubkey::default()` accepted (M03-L06).

### 6.8 `initialize_global_binding` (lines 347-378)

Allocate via System ix, write 80-byte layout. `msg!()`.

**M03-M04 critical-but-mitigated:** **There is no payer-must-be-DAO check.** The first caller becomes the global authority. Race-to-init pattern.

### 6.9 `update_global_root` (lines 382-413)

Same shape as `update_tree_root` minus discriminator-specific check. **No log, no event.**

### 6.10 `transfer_global_binding_authority` (lines 447-470)

Same shape as `transfer_tree_binding_authority`.

---

## 7. Per-error catalogue

`programs/schema-registry/src/lib.rs:569-599`. 14 variants.

Notable:
- `UnauthorizedTreeBinding` is used for both `SchemaTreeBinding` and `GlobalStateBinding` authority mismatches.
- `MalformedBinding` collapses three sub-conditions.
- No error for "binding already initialized" — System and Anchor produce different errors.
- No error for `Pubkey::default()` new authority (M03-L06).

---

## 8. Findings

### M03-M01 — No in-program test coverage; discriminator literal is load-bearing across crates

- **Severity:** MEDIUM. **Status:** Open.
- **Location:** `programs/schema-registry/src/lib.rs:36-43` (constants); `programs/schema-registry/Cargo.toml` lacks `[dev-dependencies]`.
- **Description.** Literals `b"schmtree"` and `b"globroot"` are duplicated across `programs/schema-registry/src/lib.rs:36,39` and `crates/solid-light/src/cpi_helpers.rs:245,287`. **No in-program test asserts the writer literal matches the parser literal.**
- **Recommendation.** Add a unit test asserting equality.

### M03-M02 — SOLID-SEC-019: schema-hash re-assertion missing in `set_binding_status` and `transfer_tree_binding_authority`

- **Severity:** MEDIUM. **Status:** Open.
- **Location:** `programs/schema-registry/src/lib.rs:314-341,420-444`.
- **Description.** Both handlers omit `data[8..40] == schema_hash` re-assertion. `update_tree_root` does (lines 283-287).
- **Recommendation.** Add `require!(&data[8..40] == _schema_hash.as_slice(), ErrorCode::SchemaHashMismatch);` to both. Two lines.

### M03-M3 — SOLID-SEC-016 analog: single-step authority transfer

- **Severity:** MEDIUM. **Status:** Open.
- **Location:** `programs/schema-registry/src/lib.rs:420-444,447-470`.
- **Description.** Single-step transfer; no propose/accept. Typo bricks the binding.
- **Recommendation.** Add `pending_authority: Pubkey` (32 bytes) to each layout, propose-then-accept ix pair.

### M03-M04 — `initialize_global_binding` is permissionless

- **Severity:** MEDIUM. **Status:** Open.
- **Location:** `programs/schema-registry/src/lib.rs:347-378`.
- **Description.** Anyone can call and become global authority. Race-to-init.
- **Recommendation.** Constrain initial authority to program upgrade authority. Operationally mitigated by private-RPC bootstrap.

### M03-L01 — `SCHEMA_TREE_BINDING_SIZE`, `GLOBAL_STATE_BINDING_SIZE` not asserted in tests
**Recommendation.** `debug_assert!(data.len() == ...)` after each manual write.

### M03-L02 — No `transfer_schema_authority` instruction
A lost authority key bricks the schema.

### M03-L03 — TS-side schema-hash derivation is inlined in scripts
Add `compute_schema_hash` to wasm bridge.

### M03-L04 — Permissionless `register_schema`; no anti-spam beyond rent

### M03-L05 — `initialize_tree_binding` accepts `tree_pubkey == Pubkey::default()` and is one-shot

### M03-L06 — `transfer_*_authority` handlers accept `new_authority == Pubkey::default()`

### M03-L07 — Several state-mutating handlers emit no log/event
`update_tree_root`, `update_global_root`, `set_binding_status`, `increment_usage`.

### M03-LOW — `compute_schema_hash_from_parts` truncates to 16 but Poseidon caps at 12

### M03-DOCS — Several doc claims drift
- `docs/MODULE_CONTRACTS.md:710-717` — `GlobalStateBinding` documented as 104 bytes with `global_tree_pubkey` field. Actual is 80 bytes; no such field.
- `docs/MODULE_CONTRACTS.md:723-725` — stale "DISABLED" note.
- `docs/MODULE_CONTRACTS.md:741-744` — `update_tree_root` signature drift.
- `docs/MODULE_CONTRACTS.md:746-747` — `update_global_root` signature drift.

---

## 9. Compute notes

| Instruction | Approx CU | Hot operations |
| --- | --- | --- |
| `register_schema` | ~30K-50K | Borsh ser + Poseidon syscall + Anchor `init` |
| `deprecate_schema` | ~3K | One byte write |
| `increment_usage` | ~3K | One u64 write |
| `initialize_tree_binding` | ~10K | System CPI + 145 bytes write |
| `update_tree_root` | ~3K | Length + disc + auth + slot + write |
| `set_binding_status` | ~3K | Same shape, one byte write |
| `transfer_tree_binding_authority` | ~3K | Same shape, 32-byte write |
| `initialize_global_binding` | ~10K | System CPI + 80 bytes write |
| `update_global_root` | ~3K | Same as `update_tree_root` minus disc-specific |
| `transfer_global_binding_authority` | ~3K | Same shape |

No instruction approaches 1.4M CU ceiling.

---

## 10. Dependency notes

- `anchor-lang = 0.30.1` (workspace-pinned; no known CVE).
- Removed: `light-poseidon = "0.2"`.
- Internal: `solid-core` only.
- Transitive: `borsh`, `solana-program`, `bytemuck`. Pinned by workspace lockfile.

---

## 11. Open questions

1. **Confirm CU envelope.** No measured CU numbers for any schema-registry ix.
2. **Confirm in-program test coverage gap.** Adding even one unit test would close M03-M01.
3. **Confirm operational practice for `initialize_global_binding`.** Document in `docs/architecture.md`.
4. **Confirm whether `set_global_binding_status` is intentionally absent.** SchemaTreeBinding has freeze/unfreeze; GlobalStateBinding does not.
5. **Confirm depth/buffer choices for SPL-AC trees.**
6. **Multi-validator slot oracle.** Threat-model worthy.

---

## 12. Suggested next actions

1. **Close SOLID-SEC-019 (M03-M02).** Add schema-hash re-assertion. Two-line fix.
2. **Close M03-L05 / M03-L06.** Reject `Pubkey::default()`.
3. **Open M03-M01 ticket.** Add `[dev-dependencies]` and unit test.
4. **Open M03-L07 ticket.** Add `msg!()` lines.
5. **Open M03-DOCS pass.** Sync `docs/MODULE_CONTRACTS.md`.
6. **Defer M03-M03 / SOLID-SEC-016.**
7. **Defer M03-M04.** Add doc note about private-RPC bootstrap.
8. **Defer cosmetic items.**

---

## Appendix: Cross-references

- ADR-0003, ADR-0005, ADR-0010
- SOLID-SEC-002 (Fixed), SOLID-SEC-003 (Fixed), SOLID-SEC-016 (Open), SOLID-SEC-019 (Open, M03-M02), SOLID-SEC-032 (Fixed), SOLID-SEC-035 (Open), SOLID-SEC-046 (Open)

---

## Summary

The schema-registry is the smallest of the three programs (599 LOC) and the most defensive in shape. Every raw-byte mutator has a 6-step gate. The post-SOLID-SEC-002 schema-hash integrity check is correctly wired. Dual-target byte-identity argument is sound. Historical `light-poseidon` dep removal is a real fix.

**Highest-priority action.** Close M03-M02 (SOLID-SEC-019). Two-line fix.

**Highest-leverage hardening.** Add a single in-program unit test (M03-M01).

**Most subtle risk.** M03-M04 (permissionless `initialize_global_binding`).

Total: 14 ErrorCode variants, 10 handlers walked line-by-line, 11 findings (4 MEDIUM, 7 LOW, 1 INFO+1 DOCS), 0 CRITICAL.
