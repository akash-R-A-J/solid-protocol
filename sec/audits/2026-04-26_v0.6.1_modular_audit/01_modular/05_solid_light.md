# Module audit 05 — `solid-light`

- **Date:** 2026-04-26
- **Version under audit:** v0.6.1 (post Phase-3 impl 4; ADR-0014 / ADR-0015 in force)
- **Auditor:** delegated Solana / Anchor / SPL Account Compression specialist (agent), persisted by orchestrator
- **Scope:** `crates/solid-light` — shared BPF-compatible helpers consumed by all three Anchor programs.
- **Files audited (every line):**
  - `crates/solid-light/Cargo.toml` (15 lines)
  - `crates/solid-light/src/lib.rs` (13 lines)
  - `crates/solid-light/src/cpi_helpers.rs` (765 lines)
  - `crates/solid-light/src/credential_tree.rs` (112 lines)
- **Reference context (read for orientation, not in scope):**
  `programs/zk-verifier/src/lib.rs`, `programs/issuer-registry/src/lib.rs`, `programs/schema-registry/src/lib.rs`, `adr/0003`, `adr/0010`, `adr/0014`.

> NOTE: the delegated agent's `Write` and `Bash` calls were denied at run time, so the audit was returned in its final assistant message and persisted to this file by the orchestrator without modification (HTML entities decoded back to `&`, `<`, `>`).

## 0. Executive summary

`solid-light` is the **layout contract** for cross-program state reads. Three on-chain programs (`zk-verifier`, `issuer-registry`, `schema-registry`) trust this crate to (a) name the foreign-program owner pubkeys correctly and (b) parse foreign-program-owned PDAs without panicking. Both contracts hold in v0.6.1.

The crate is small, well-bounded, and disciplined: 5 byte-level parsers covering 3 binding layouts, 4 builders for off-chain leaf records, 2 constants for trust-anchor program IDs, and 21 unit tests pinning every mismatch axis. The 4-test `id_bytes_tests` module is the localised SOLID-SEC-032 drift gate, complementing `scripts/check_program_ids.py`.

**No CRITICAL, HIGH, or MEDIUM findings.** Two LOW findings and three INFO notes.

The single quality concern is that the `build_insert_*` / `build_revoke_*` builders (`cpi_helpers.rs:159-231`) and the `Compressed*` Borsh types in `credential_tree.rs:14-112` are dead code from the workspace's perspective (no in-tree caller). They presumably target external Rust SDK consumers, but the crate does not say so. See **M05-LO-01**.

## 1. Crate / build configuration audit (`Cargo.toml`)

```
[package]
name = "solid-light"            # retained per ADR-0003 / SOLID-SEC-009
[lib]
crate-type = ["lib"]            # plain Rust lib (no cdylib, no BPF entry)
[dependencies]
anchor-lang = { workspace = true }   # 0.30.1
borsh = "0.10"                       # resolves to 0.10.4
hex = "0.4"                          # UNUSED
```

Findings:

1. `crate-type = ["lib"]` correct. No risk of producing a deployable BPF binary from this crate.
2. `borsh = "0.10"` resolves to `0.10.4` (`Cargo.lock:720-728`). `anchor-lang 0.30.1` itself depends on `borsh 0.10.4` (`Cargo.lock:244`), so the `BorshSerialize` trait derived in `credential_tree.rs:14` is the **same trait identity** as the one anchor-lang's macros emit. No version-skew silent-deserialize hazard. The lock also contains `borsh 0.9.3` and `borsh 1.5.7` (transitive elsewhere) — neither reaches `solid-light`.
3. `hex = "0.4"` is declared but **not used** anywhere in `crates/solid-light/`. **M05-LO-02**.
4. No `cfg(target_os = "solana")` gating — every public function is BPF-compatible. The `light-poseidon` BPF stack overflow problem (workspace `Cargo.toml:51-59`) does not apply because `solid-light` does no Poseidon hashing.
5. No `[dev-dependencies]` — tests are in-file and use only the crate's own deps. Minimal toolchain drift surface.

Verdict: **clean** modulo M05-LO-02.

## 2. `lib.rs` (13 lines) — module re-exports

```rust
pub mod cpi_helpers;
pub mod credential_tree;
```

Both modules re-exported at top level; no `pub use` shortcuts, so consumers must spell the full path (`solid_light::cpi_helpers::SCHEMA_REGISTRY_ID`, etc.). Each consumer is therefore grep-able. Verdict: **clean**.

## 3. `credential_tree.rs` (112 lines) — Borsh leaf records

Four types: `CompressedCredential` (lines 14-54), `CompressedNullifier` (60-72), `CompressedIdentity` (78-91), `CompressedIssuer` (97-112). All `BorshSerialize + BorshDeserialize + Debug + Clone`.

### Per-type observations

- **`CompressedCredential`** — 113-byte total, `DISCRIMINATOR = b"solidcrd"` (literal ASCII tag, NOT Anchor sha256-derived). Correct: type is consumed off-chain, where literal tags are cross-language stable.
- **`CompressedNullifier`** — 48-byte; `DISCRIMINATOR = b"solidnul"`. **Distinct from** the on-chain `NullifierRecord` Anchor account (`programs/zk-verifier/src/lib.rs:964-972`), which has Anchor's hashed disc. No collision risk.
- **`CompressedIdentity`** — 48-byte; `DISCRIMINATOR = b"solidid_"`.
- **`CompressedIssuer`** — 114-byte; `DISCRIMINATOR = b"solidiss"`. **Notably different** from the on-chain issuer Merkle leaf preimage, which is `Poseidon5(authority, bjj_x, bjj_y, status_epoch, revocation_nonce)` per ADR-0014. `CompressedIssuer` is the registry's mirror record (no `status_epoch`, has `tier` + enum `status`). Drift risk if a future leaf-preimage revision adds fields. **M05-INFO-02**.

`size()` constants match field arithmetic exactly (Borsh encodes fixed-size arrays/i64/bool with no overhead). No deserialize-side helpers in this file — correct: parsing happens off-chain in TS/JS, and on-chain Anchor programs never read these leaf shapes.

**No on-chain consumer.** Workspace grep returns only `programs/issuer-registry/src/lib.rs:584` (a comment). The four types are pure off-chain layout contracts.

Verdict: **clean**, modulo M05-INFO-02.

## 4. `cpi_helpers.rs` (765 lines) — function-by-function

### 4.1 Trust-anchor program-ID constants (lines 35-78)

```rust
pub const SCHEMA_REGISTRY_PROGRAM_ID: &str = "4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1";
pub const SCHEMA_REGISTRY_ID: Pubkey = Pubkey::new_from_array(SCHEMA_REGISTRY_ID_BYTES);
const SCHEMA_REGISTRY_ID_BYTES: [u8; 32] = [52, 211, 14, 237, ...];

pub const ISSUER_REGISTRY_PROGRAM_ID: &str = "5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx";
pub const ISSUER_REGISTRY_ID: Pubkey = Pubkey::new_from_array(ISSUER_REGISTRY_ID_BYTES);
const ISSUER_REGISTRY_ID_BYTES: [u8; 32] = [69, 105, 202, 235, ...];
```

The two-step (`_BYTES` const + `Pubkey::new_from_array`) is required because `anchor_lang::prelude` does not re-export the `pubkey!` proc-macro; `new_from_array` is `const`-eligible since solana-program 1.18 so the typed constants are usable in `seeds::program = ID` and `require_keys_eq!`.

The byte arrays are the **load-bearing** values; the string literals are documentation. Drift between them silently re-opens the forged-trust-root attack with zero test signal — this is exactly SOLID-SEC-032's rationale.

**Drift tests at lines 101-150** (`id_bytes_tests` module) are the **single local CI gate** catching transcription drift; `scripts/check_program_ids.py` is the repo-wide gate that cross-validates against `Anchor.toml` and the `declare_id!` literals. Both are necessary and both are present.

Cross-checked the canonical IDs in `CLAUDE.md`'s hard invariants and `programs/schema-registry/src/lib.rs:4` (`declare_id!("4ZCrxVB...")`) — match.

### 4.2 Off-chain leaf builders (lines 159-231)

Five `pub fn` builders (`build_insert_credential_data`, `build_insert_nullifier_data`, `build_insert_identity_data`, `build_insert_issuer_data`, `build_revoke_credential_data`). Each:

```rust
let mut data = Vec::with_capacity(T::size());
data.extend_from_slice(&T::DISCRIMINATOR);
borsh::to_writer(&mut data, &t).map_err(|_| error!(LightError::SerializationFailed))?;
Ok(data)
```

Capacity hints match `T::size()`. Misalignment would just trigger a `Vec` realloc, not a correctness bug. `borsh::to_writer` errors collapse to a generic `LightError::SerializationFailed`, fine for an off-chain caller.

**Dead code in the workspace.** Grep shows no caller in `programs/`, `ts-sdk/`, or `wasm/`. The docstring at lines 155-158 says "Consumed by off-chain tooling" but does not identify who. **M05-LO-01**.

Not currently a security risk: builders are pure functions returning `Vec<u8>`; cannot be invoked from BPF without an upstream caller. With `lto = "thin"` (workspace `Cargo.toml:93`) DCE should remove them from the BPF binary, but this is best-effort. **M05-INFO-03**.

### 4.3 `verify_schema_root_binding` (lines 254-276)

```rust
pub fn verify_schema_root_binding(data: &[u8], expected_root: &[u8;32], expected_schema: &[u8;32]) -> bool {
    if data.len() < 113 { return false; }
    if data[..8] != SCHEMA_TREE_DISCRIMINATOR { return false; }
    if &data[8..40] != expected_schema.as_slice() { return false; }
    if &data[72..104] != expected_root.as_slice() { return false; }
    data[112] == 0
}
```

Per-line check against ADR-0010 + the writer at `programs/schema-registry/src/lib.rs:233-241`:

- **Length gate `< 113`.** Writer allocates 145 bytes (`SCHEMA_TREE_BINDING_SIZE`); parser reads first 113. Trailing 32 bytes (`authority` at `[113..145]`) are forward-compatible.
- **Discriminator** matches writer's `b"schmtree"` at `schema-registry/src/lib.rs:235`.
- **Schema match** at `[8..40]` matches writer at `schema-registry/src/lib.rs:236`.
- **Root match** at `[72..104]` matches writer's init at `schema-registry/src/lib.rs:238` and `update_tree_root` at `schema-registry/src/lib.rs:306`.
- **Status gate** `[112] == 0` matches writer's `STATUS_ACTIVE = 0` at `schema-registry/src/lib.rs:240`. `set_binding_status` only allows toggling to `STATUS_FROZEN = 1` (`schema-registry/src/lib.rs:319-322`).

Returns `bool` — caller (`programs/zk-verifier/src/lib.rs:530`) wraps via `require!`. No `unwrap()` anywhere.

Edge cases: all-zero buffer → disc reject; `< 113` → length reject; wrong root → root reject; `[112] = 1` → frozen reject. All fail-closed.

**Verdict: correct**. Combined with the owner-check at `zk-verifier/src/lib.rs:523-527`, the forged-trust-root attack is closed.

### 4.4 `verify_state_root_matches` (lines 287-297)

```rust
pub fn verify_state_root_matches(data: &[u8], expected_root: &[u8;32]) -> bool {
    if data.len() < 40 { return false; }
    if data[..8] != GLOBAL_ROOT_DISCRIMINATOR { return false; }
    &data[8..40] == expected_root.as_slice()
}
```

Writer at `schema-registry/src/lib.rs:347-378` allocates 80 bytes (`GLOBAL_STATE_BINDING_SIZE`); parser reads first 40. Layout: `[0..8] disc`, `[8..40] current_root`, `[40..48] last_updated_slot`, `[48..80] authority`.

**No status field on `GlobalStateBinding`** — verified by reading the writer. The parser correctly does not check one. **No schema-hash check** — there's only one global state per deployment.

Caller: `zk-verifier/src/lib.rs:454` after the owner check at lines 446-450.

**Verdict: correct**. Narrower contract than schema-tree, matching the narrower writer.

### 4.5 SchemaTreeBinding field accessors (lines 320-349)

`schema_tree_binding_schema_hash` (req. 40 bytes), `schema_tree_binding_tree_pubkey` (req. 72), `schema_tree_binding_status` (req. 113). Each:

1. Length-checks the **minimum buffer for the slot it reads**.
2. Re-checks the discriminator (defence-in-depth for callers using one accessor without the bundle gate).
3. Returns `Option<T>` — no `unwrap()` in the crate's tests; the bundle gate uses `.ok_or(...)`.

Slice indices verified against the writer:

| Field | Writer line | Parser slot |
|-------|-------------|-------------|
| schema_hash | `schema-registry/src/lib.rs:236` `[8..40]` | `[8..40]` |
| tree_pubkey | `schema-registry/src/lib.rs:237` `[40..72]` | `[40..72]` |
| status | `schema-registry/src/lib.rs:240` `[112]` | `[112]` |

`Pubkey::new_from_array` accepts any 32 bytes — no on-curve validation. Correct, because comparison is byte-equality with the runtime account, not elliptic-curve check.

**Verdict: correct**. All slice indices within verified buffer length; no panic possible.

### 4.6 `verify_schema_tree_binding_for_issue` (lines 365-383)

The **issue-time** gate used by `issuer-registry::issue_credential` (`issuer-registry/src/lib.rs:1554-1564`). Three sequenced checks: schema match → tree match → status active. Short-buffer rejected at the first accessor needing the missing bytes; mismatches surface specific error codes (`InvalidSchemaBinding`, `TreeBindingMismatch`, `SchemaTreeBindingFrozen`) downstream.

Helper does NOT check the runtime account owner — that lives at the call site (`issuer-registry/src/lib.rs:1547-1551`, `require_keys_eq!(*ctx.accounts.schema_tree_binding.owner, SCHEMA_REGISTRY_ID, ...)`). Splitting the responsibility lets the helper stay `Pubkey`-and-bytes-only and host-testable. Docstring at lines 360-364 is explicit about this contract.

**Verdict: correct.** This is the canonical SOLID-SEC-003 gate.

### 4.7 IssuerTreeBinding parsers (lines 419-453)

Three accessors (`issuer_tree_binding_tree_pubkey` req. 40, `issuer_tree_binding_current_root` req. 72, `issuer_tree_binding_status` req. 81). Layout cross-checked against writer (`issuer-registry/src/lib.rs:91-96` doc + `:938-944` writer):

- `[0..8]` disc `b"issrtree"` — parser slot match
- `[8..40]` tree_pubkey — parser slot match
- `[40..72]` current_root — parser slot match
- `[72..80]` last_updated_slot — unread by parser (writer-only)
- `[80]` status — parser `< 81` length gate, byte at `[80]`
- `[81..113]` authority — unread (forward-compatible)

**Layout difference vs SchemaTreeBinding** (load-bearing):

| Slot | SchemaTreeBinding | IssuerTreeBinding |
|------|-------------------|-------------------|
| `[8..40]` | schema_hash | tree_pubkey |
| `[40..72]` | tree_pubkey | current_root |
| `[72..104]` | current_root | (truncated; no schema_hash) |
| `[80]` | n/a | status |
| `[112]` | status | n/a |

Both layouts are 113 bytes by design (`cpi_helpers.rs:413-416`: "Sized identically to the read-window of `SchemaTreeBinding` so the parser fast-path is shared"). **Discriminators differ** (`b"schmtree"` vs `b"issrtree"`), so the two parsers reject each other's accounts. Test at lines 712-718 forges the disc and confirms accessors return `None`. Fail-closed.

### 4.8 `verify_issuer_tree_binding_for_proof` (lines 466-480)

The **proof-time** gate used by `zk-verifier::verify_batch_proof` (`zk-verifier/src/lib.rs:474-482`). Two checks: root match, status active. Compared with the issue-time gate, this one does NOT check a `tree_pubkey` slot — there is no per-proof tree the verifier knows about, only the root the circuit proved against.

**Belt-and-suspenders at the verifier site** (5 independent gates):

1. Anchor seed-program constraint at the account context (`zk-verifier/src/lib.rs:884-889`): `seeds = [b"issuer-tree-binding"], seeds::program = ISSUER_REGISTRY_ID` — forces the account address to be the canonical PDA.
2. In-handler owner check (`zk-verifier/src/lib.rs:467-471`).
3. In-handler discriminator check (this helper).
4. In-handler root match.
5. In-handler status check.

Each check is independent. Bypassing all five requires forging a PDA derivation (impossible without issuer-registry signing) AND flipping the runtime owner (impossible) AND writing through `update_issuer_tree_root` AND setting `current_root` to the proof's value AND keeping status active. Defence-in-depth correct.

**Verdict: correct.** Implements the gate ADR-0014 specifies.

### 4.9 `LightError` (lines 484-504)

Nine variants. Mapped at:
- `issuer-registry/src/lib.rs:1559-1563` — `InvalidSchemaBinding`, `TreeBindingMismatch`, `SchemaTreeBindingFrozen`.
- `zk-verifier/src/lib.rs:478-482` — `IssuerTreeRootMismatch`, `IssuerTreeBindingFrozen`, generic `InvalidIssuerTreeBinding`.

`SerializationFailed`, `StateRootMismatch`, `CredentialRevoked` are **declared but unused in BPF code paths** — they exist only for the off-chain builders. Recorded under M05-LO-01.

### 4.10 Tests (lines 508-765)

Twenty-one `#[test]` functions (4 drift gates + 5 schema-root + 2 global-root + 6 issue-gate + 4 issuer-tree accessor + 4 proof-gate; light overlap because both gates re-exercise some accessors).

Coverage matrix:

| Parser | Happy | Length | Disc | Schema/Tree | Root | Status |
|--------|-------|--------|------|-------------|------|--------|
| `verify_schema_root_binding` | yes | yes | yes | yes | yes | yes |
| `verify_state_root_matches` | yes | **(no)** | (covered indirectly) | (-) | yes | (-) |
| `verify_schema_tree_binding_for_issue` | yes | yes | yes | yes | (-) | yes |
| `verify_issuer_tree_binding_for_proof` | yes | (covered via accessor short-data tests) | yes | (-) | yes | yes |

Asymmetry: `verify_state_root_matches` has no explicit short-buffer test, while `verify_schema_root_binding` does (`root_binding_rejects_short_data` line 578). The guard is obviously correct by inspection. **M05-INFO-01**.

Tests are host-only (`cargo test -p solid-light`) — fast, no validator required.

## 5. Owner-check + tree-binding parser correctness — dedicated section

The crate's job is to make the ADR-0010 + ADR-0014 owner-check pattern impossible to misuse. Three call sites in the workspace consume it:

### 5.1 `programs/zk-verifier/src/lib.rs:446-456` — `global_tree`

```rust
require_keys_eq!(*ctx.accounts.global_tree.owner, SCHEMA_REGISTRY_ID, ErrorCode::InvalidGlobalRoot);
let global_root = public_inputs[1];
let tree_account_data = ctx.accounts.global_tree.try_borrow_data()?;
require!(cpi_helpers::verify_state_root_matches(&tree_account_data, &global_root),
    ErrorCode::InvalidGlobalRoot);
```

ADR-0010: "Before trusting any byte of a tree PDA, zk-verifier requires `require_keys_eq!(*ctx.accounts.global_tree.owner, SCHEMA_REGISTRY_ID, ...)`." **Invariant met.** Owner check precedes byte-parse; byte-parse adds discriminator + root match. `global_tree` is declared as a plain `UncheckedAccount` (no seed constraint at the verifier context) — the owner check is the sole address-binding gate. This is correctly load-bearing per ADR-0010.

### 5.2 `programs/zk-verifier/src/lib.rs:516-532` — `schema_tree_N` (slots 0..3)

```rust
let tree_info = match i { 0 => &ctx.accounts.schema_tree_0, ..., _ => &ctx.accounts.schema_tree_3 };
require_keys_eq!(*tree_info.owner, SCHEMA_REGISTRY_ID, ErrorCode::InvalidSchemaRootBinding);
let data = tree_info.try_borrow_data()?;
require!(cpi_helpers::verify_schema_root_binding(&data, &merkle_root, &schema_hash),
    ErrorCode::InvalidSchemaRootBinding);
```

The slot loop runs only for non-zero `(merkle_root, schema_hash)` pairs (zero pairs short-circuit at line 500-502 — correctly skipping unread accounts).

`schema_tree_N` is also `UncheckedAccount` with no seed constraint. The schema-match inside `verify_schema_root_binding` is what actually binds slot-i to schema-i (without it, an attacker could swap two valid schema_tree PDAs). Verified.

ADR-0010: "Applied to every `schema_tree_0..3` ... in `verify_batch_proof`." **Invariant met.**

### 5.3 `programs/zk-verifier/src/lib.rs:467-483` — `issuer_tree_binding` (ADR-0014)

```rust
require_keys_eq!(*ctx.accounts.issuer_tree_binding.owner, ISSUER_REGISTRY_ID,
    ErrorCode::InvalidIssuerTreeBinding);
let issuer_tree_root_input = public_inputs[ISSUER_TREE_ROOT_INPUT_INDEX];
let issuer_binding_data = ctx.accounts.issuer_tree_binding.try_borrow_data()?;
cpi_helpers::verify_issuer_tree_binding_for_proof(&issuer_binding_data, &issuer_tree_root_input)
    .map_err(|e| match e { ... })?;
```

Plus the Anchor seed-program constraint at lines 884-889 (`seeds = [b"issuer-tree-binding"], seeds::program = ISSUER_REGISTRY_ID`). The seed-program constraint is **stricter than the owner-check alone** — it also asserts the account address is the canonical PDA derived under issuer-registry's program ID.

ADR-0014: "Owner-check it against `ISSUER_REGISTRY_ID` and parse out `current_root`, asserting it equals `publicInputs[10]`." **Invariant met.**

`ISSUER_TREE_ROOT_INPUT_INDEX = 10` matches the public-input layout in ADR-0014 (and CLAUDE.md hard invariants).

### 5.4 `programs/issuer-registry/src/lib.rs:1547-1564` — `schema_tree_binding` (issue_credential)

```rust
require_keys_eq!(*ctx.accounts.schema_tree_binding.owner, SCHEMA_REGISTRY_ID,
    ErrorCode::InvalidSchemaTreeBindingOwner);
let binding_data = ctx.accounts.schema_tree_binding.try_borrow_data()?;
verify_schema_tree_binding_for_issue(&binding_data, &schema_hash, &ctx.accounts.merkle_tree.key())
    .map_err(|e| match e { ... })?;
```

Plus the seed-program constraint at lines 1988-1992. **Invariant met.** SOLID-SEC-003 gate.

### 5.5 Bytes-level cross-check summary (writer ↔ parser)

| Layout | Writer | Parser | Match |
|--------|--------|--------|-------|
| `SchemaTreeBinding[0..8]` | `b"schmtree"` (`schema-registry/src/lib.rs:235`) | `SCHEMA_TREE_DISCRIMINATOR` (`cpi_helpers.rs:245`) | yes |
| `SchemaTreeBinding[8..40]` schema_hash | line 236 | `[8..40]` schema match | yes |
| `SchemaTreeBinding[40..72]` tree_pubkey | line 237 | `[40..72]` tree match | yes |
| `SchemaTreeBinding[72..104]` current_root | lines 238, 306 | `[72..104]` root match | yes |
| `SchemaTreeBinding[104..112]` last_updated_slot | line 239 | (unread) | n/a |
| `SchemaTreeBinding[112]` status | line 240 | `[112] == 0` | yes |
| `SchemaTreeBinding[113..145]` authority | line 241 | (unread; forward-compat) | n/a |
| `GlobalStateBinding[0..8]` | `b"globroot"` (`schema-registry/src/lib.rs:372`) | `GLOBAL_ROOT_DISCRIMINATOR` (`cpi_helpers.rs:287`) | yes |
| `GlobalStateBinding[8..40]` current_root | lines 373, 410 | `[8..40]` root match | yes |
| `GlobalStateBinding[40..48]` slot | line 374 | (unread) | n/a |
| `GlobalStateBinding[48..80]` authority | line 375 | (unread; forward-compat) | n/a |
| `IssuerTreeBinding[0..8]` | `b"issrtree"` (`issuer-registry/src/lib.rs:939`) | `ISSUER_TREE_DISCRIMINATOR` (`cpi_helpers.rs:419`) | yes |
| `IssuerTreeBinding[8..40]` tree_pubkey | line 940 | `[8..40]` accessor | yes |
| `IssuerTreeBinding[40..72]` current_root | lines 941, 1001 | `[40..72]` root match | yes |
| `IssuerTreeBinding[72..80]` slot | line 942 | (unread) | n/a |
| `IssuerTreeBinding[80]` status | line 943 | `[80] == 0` | yes |
| `IssuerTreeBinding[81..113]` authority | line 944 | (unread; forward-compat) | n/a |

**No drift between writer and parser.** Trailing fields are intentionally forward-compatible — parser reads only the prefix the layout contract freezes; new fields can be appended after the parser window without breaking older builds.

**Verdict for the section: the owner-check + parser pair is correct as specified in ADR-0010 and ADR-0014.** No drift, no hole.

## 6. Findings

| ID | Severity | Class | Status |
|----|----------|-------|--------|
| M05-LO-01 | LOW | Dead code (unused builders + types + 3 error variants) | Open |
| M05-LO-02 | LOW | Unused `hex` dependency | Open |
| M05-INFO-01 | INFO | Missing short-buffer test on `verify_state_root_matches` | Open |
| M05-INFO-02 | INFO | `CompressedIssuer` doc-drift risk vs ADR-0014 leaf preimage | Open |
| M05-INFO-03 | INFO | Latent BPF binary-size bloat from unused builders | Open |

### M05-LO-01 — Unused public builders + unused error variants

- **File / lines:** `cpi_helpers.rs:159-231`, `486-491` (`SerializationFailed`, `StateRootMismatch`, `CredentialRevoked`); `credential_tree.rs:14-112`.
- **Description:** Five `build_*` functions and four `Compressed*` types are `pub` but have no caller in the workspace. Three `LightError` variants exist only for these builders.
- **Impact:** None directly (builders are pure functions returning `Vec<u8>`; not invokable from BPF without a caller). Risk is doc drift: maintainers may misread the crate's surface as BPF-relevant.
- **Recommendation:** Pick one — (1) delete; (2) move to a host-only sibling crate; or (3) document the external SDK contract explicitly in the module docstring.

### M05-LO-02 — Unused `hex` dependency

- **File / lines:** `Cargo.toml:14`.
- **Description:** `hex = "0.4"` declared, zero hits inside `crates/solid-light/`.
- **Recommendation:** Remove the line. Bundle into the same cleanup pass as M05-LO-01.

### M05-INFO-01 — Negative-length test gap on `verify_state_root_matches`

- **File / lines:** `cpi_helpers.rs:583-596`.
- **Description:** No test exercises the `if data.len() < 40 { return false; }` guard at line 290. `verify_schema_root_binding` has a sibling test (`root_binding_rejects_short_data` line 578); coverage is asymmetric.
- **Recommendation:** Add `assert!(!verify_state_root_matches(&[0u8; 32], &[0u8; 32]));`. Two-line diff.

### M05-INFO-02 — Indexer-contract drift risk on `CompressedIssuer`

- **File / lines:** `credential_tree.rs:97-112`.
- **Description:** `CompressedIssuer` is the off-chain mirror record; the on-chain Merkle leaf preimage (ADR-0014) is `Poseidon5(authority, bjj_x, bjj_y, status_epoch, revocation_nonce)` — a **different** tuple. If a future revision adds a field to the on-chain leaf preimage, an external indexer reading `CompressedIssuer` will silently miss it.
- **Recommendation:** Document the off-chain-only contract in the struct's docstring; reference ADR-0014.

### M05-INFO-03 — Unused builders may inflate BPF binary in worst case

- **File / lines:** `cpi_helpers.rs:159-231`.
- **Description:** With `lto = "thin"` + `codegen-units = 1` (`Cargo.toml:93-94`), unused `pub fn` items are expected to be DCEd. Best-effort; depends on linker reachability analysis. Latent footprint risk if a future BPF caller transitively pulls them in.
- **Recommendation:** Add a CI assertion on `target/deploy/*.so` size with a known baseline. Roll into SOLID-SEC-046.

## 7. Compute notes

| Function | CU shape (rough) |
|----------|------------------|
| `verify_state_root_matches` | length + 8-byte disc + 32-byte root compare. ~30 CU. |
| `verify_schema_root_binding` | 4 byte compares + 1 byte test. ~80 CU. |
| `verify_schema_tree_binding_for_issue` | 3 accessor calls + 2 byte compares + 1 byte test. ~140 CU. |
| `verify_issuer_tree_binding_for_proof` | 2 accessor calls + 1 byte compare + 1 byte test. ~100 CU. |

Per `verify_batch_proof` invocation, total parser overhead is ~630 CU worst case (1 global + up to 4 schema + 1 issuer + 6 owner-check `require_keys_eq!`). Dwarfed by ~120K CU Groth16 verify itself. No risk of busting the per-tx CU budget because of the parser. The off-chain `build_*` builders are not invoked on-chain — CU cost not material.

## 8. Dependency notes

- **`anchor-lang = { workspace = true }`** → 0.30.1 (`Cargo.lock:226-249`). Pulls `borsh 0.10.4`, `solana-program 1.18.x`, `bincode`, `bytemuck`, `getrandom 0.2.17`, `thiserror 1.0.69`. Crate uses `anchor_lang::prelude` (`Pubkey`, `Result`, `error!`, `error_code`). All BPF-compatible. No `init_if_needed` / `idl-build` features here.
- **`borsh = "0.10"`** → 0.10.4 (`Cargo.lock:720-728`). Used at `cpi_helpers.rs:167-181` and via the `BorshSerialize/Deserialize` derives in `credential_tree.rs:14, 60, 78, 97`. Pinning to "0.10" is **correct** — must agree with anchor-lang 0.30.1's transitive `borsh 0.10.4` so that derived traits are nominally identical. A pin like `"*"` or `"1"` would emit a different trait at compile time (Rust's nominal trait identity catches it before bytes are written; no silent corruption hazard, but a build-break hazard). Lock also has `borsh 0.9.3` and `borsh 1.5.7` (transitive elsewhere); neither reaches `solid-light`.
- **`hex = "0.4"`** → 0.4.3 (`Cargo.lock:1370`). Declared, unused. **M05-LO-02**.
- **`light-sdk` is NOT a dep.** The v0.1 Light Protocol code path was removed in the v0.2 migration to SPL Account Compression (ADR-0003 / SOLID-SEC-009). The crate name `solid-light` is retained for compatibility. Per the docstring at lines 4-7: "The previous version of this file referenced `light_sdk::cpi::compressed_account_create`, which does not exist in `light-sdk = "0.4.0"` (it was a placeholder). That created a latent panic at deploy-time and a false sense of security." Removed correctly.
- **`light-poseidon` is NOT a dep.** Workspace `Cargo.toml:51-59` notes `light-poseidon` is intentionally host-only (its round-constants overflow the BPF 4 KB stack frame under `lto = "fat"`). Tree-leaf hashing on BPF goes through `solana_program::poseidon::hashv` (the `sol_poseidon` syscall); `solid-light` does no Poseidon hashing itself. Correct absence.

**Cross-language vector hard-gate:** the parser contract is exercised only on the Rust side. The TypeScript SDK does not currently parse `SchemaTreeBinding` / `IssuerTreeBinding` byte layouts directly (it calls `getAccountInfo` and lets the on-chain handler validate). If a TS-side parser is ever added, it must round-trip against `tests/vectors/` per SOLID-SEC-010. Not currently a gap.

## 9. Open questions / TODOs

1. **Are there external Rust SDK consumers of `build_insert_*` and `Compressed*`?** None in-tree. If yes, reference them in the docstrings. If no, delete.
2. **Should `LightError` be split into "parser errors" and "serializer errors"?** Three unused variants suggest the design anticipated more code paths.
3. **Should `CompressedIssuer` track `status_epoch` in lockstep with the on-chain leaf preimage?** Not for correctness (the leaf preimage is computed by `issuer-registry`, not by external indexers). But a docstring pointing to ADR-0014's leaf composition would prevent external SDK authors from assuming `CompressedIssuer` is the leaf-preimage record.
4. **Is the parser window for `SchemaTreeBinding` (113 bytes) ever going to need to read past `[112]`?** Frozen at `cpi_helpers.rs:243-244` and `schema-registry/src/lib.rs:32-34`, but no CI gate enforces compatibility with the writer's `SCHEMA_TREE_BINDING_SIZE = 145`. Recommended: add a doc test asserting the parser window is a prefix of the writer's allocation.

## 10. Suggested next actions

In rough priority order:

1. **Tighten the public API.** Either delete the unused builders + types + error variants (M05-LO-01) or move them to a host-only sibling crate. Same PR can drop the `hex` dep (M05-LO-02). Two-line diff each. No risk.
2. **Add the missing short-buffer test for `verify_state_root_matches` (M05-INFO-01).** One-line add. Brings test parity across the two parsers.
3. **Add a docstring linking `CompressedIssuer` to ADR-0014 (M05-INFO-02).** One paragraph; explicitly disclaim that this is the off-chain mirror record, not the on-chain leaf preimage.
4. **Fold the binary-size regression check into the SOLID-SEC-046 CU/CI gate (M05-INFO-03).**
5. **Optional polishing:** Document who calls `build_insert_*` in the module docstring at the top of `cpi_helpers.rs`. Or just delete the section if no external consumer exists.

None are blockers for v0.6.1. The crate as it stands is **fit for the v0.6.1 external-audit window** — every load-bearing helper agrees with its writer, the drift gate is in place, and the call sites are exhaustively owner-checked.
