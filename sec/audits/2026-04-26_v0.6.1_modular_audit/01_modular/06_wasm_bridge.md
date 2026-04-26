# 06 WASM Bridge — `wasm/` (crate `solid-wasm`)

- Audit date: 2026-04-26
- Branch: main, version v0.6.1
- HEAD when sourced: `59c99c2`
- Auditor: delegated Rust / wasm-bindgen / cross-language byte-identity reviewer (agent), persisted by orchestrator
- Scope (in this file):
  - `wasm/Cargo.toml` (25 LOC)
  - `wasm/src/lib.rs` (299 LOC)
  - `scripts/wasm_bridge_smoke.mjs` (54 LOC)
- Scope (read for orientation, audited elsewhere):
  - `crates/solid-core/src/{poseidon,babyjubjub,nullifier,commitment,identity}.rs`
  - `crates/solid-core/Cargo.toml`
  - `ts-sdk/packages/core/src/index.ts` (consumer, audited in `08_ts_sdk.md`)
  - `tests/vectors/{check_vectors.ts,commitment_and_nullifier.json}`
  - `crates/solid-core/examples/gen_vectors.rs`
  - `.github/workflows/ci.yml` jobs `wasm` and `wasm_bridge_smoke`

The `solid-wasm` crate is the single allowed home for `#[wasm_bindgen]` exports; ADR-0002 / SOLID-SEC-028 require that `solid-core` stay BPF-compatible and expose zero `#[wasm_bindgen]` symbols. `solid-wasm` imports `solid-core` and re-projects its primitives into a JavaScript ABI. The TypeScript SDK is the only consumer; it never reimplements crypto. The crate is small (one file, eleven exports). The audit therefore goes deep on each export rather than wide.

Verified that ADR-0002 / SOLID-SEC-028 invariant holds: a tree-wide grep for `wasm_bindgen` / `wasm-bindgen` under `crates/` returns only the manifest comment at `crates/solid-core/Cargo.toml:11`, no actual `#[wasm_bindgen]` attribute.

---

## 2. Crate / build configuration audit

### 2.1 Manifest invariants

`wasm/Cargo.toml:1-25` declares the crate as both `cdylib` and `rlib` (line 8). The `cdylib` is what `wasm-pack` consumes; `rlib` is dead weight on the wasm pipeline but enables host-side `cargo test` of this crate without rebuilding solid-core. No tests live in this crate (see 2.4), so the `rlib` is currently not exercised; keeping it is fine but the dev-dep `wasm-bindgen-test = "=0.3.50"` (line 25) is unused. See finding `M06-L01`.

The crate is a workspace member at `Cargo.toml:9`. It inherits `workspace.package.rust-version = "1.75"` from `Cargo.toml:27`, but the actually-installed toolchain enforced project-wide is `1.79.0` (per `rust-toolchain.toml`); this matters because the version pin reasoning below is keyed off rustc 1.79.0, not 1.75.

### 2.2 Dependency-by-dependency audit

| Line | Dep | Pin | Rationale | Resolved (Cargo.lock) | Concern |
|------|-----|-----|-----------|------------------------|---------|
| 11 | `solid-core` | path = `../crates/solid-core` | Source of truth for crypto primitives. `solid-wasm` is a pure projection layer. | path dep | None. The `wasm32-unknown-unknown` arm in `crates/solid-core/Cargo.toml:111-117` is what makes this work. |
| 16 | `wasm-bindgen` | `=0.2.100` | Hard pin: 0.2.118+ raises wasm-bindgen-cli MSRV to rustc 1.86, conflicts with rust-toolchain.toml 1.79.0. Comment at lines 12-15. Consistent with `docs/E2E_BLOCKERS.md` B6. | 0.2.100 (Cargo.lock:2992-3002) | See `M06-I01`: pin is correct now but blocks any future bridge feature requiring a >=0.2.101 fix. |
| 17 | `serde-wasm-bindgen` | `=0.6.5` | Used to convert Rust structs to JsValue at four sites: `generate_bjj_keypair`, `sign_message`, `derive_credential_key` (returning `JsValue`); `verify_signature` returns `Result<bool>` only. | 0.6.5 (Cargo.lock:2081-2090) | None. |
| 18 | `serde` | `1.0` (range) | `#[derive(Serialize)]` for the local result structs. | n/a (workspace dep cascade) | None. |
| 19 | `serde_json` | `1.0` (range) | NOT directly used in `wasm/src/lib.rs`. Only reachable via `solid-core`'s `BJJIdentity::export_json` / `import_json` which already pulls it transitively. | n/a | `M06-L02`: dead direct dep. |
| 20 | `js-sys` | `=0.3.77` | Pulled to satisfy wasm-bindgen 0.2.100's expected js-sys major-minor; also defensive against constraint loosening. Currently the source never imports `js_sys::*`. | 0.3.77 (Cargo.lock:1497-1505) | `M06-I02`: not exercised in source. |
| 21 | `console_error_panic_hook` | `=0.1.7` | Used by `init()` (line 14-16) to forward Rust panics into `console.error`. | 0.1.7 (Cargo.lock:941-949) | None. |
| 22 | `getrandom` | `0.2`, features `["js"]` | The `js` feature wires getrandom to `crypto.getRandomValues`. Without it, `solid_core::babyjubjub::generate_keypair` and `BJJIdentity::generate` (which call `OsRng` / `rand::random`) panic with "the operating system has no available randomness" on wasm32. | 0.2.17 (Cargo.lock:1272-1283) | None — lock entry shows `js-sys` and `wasm-bindgen` deps, confirming the `js` feature is selected. See `M06-I03` for the multiple-getrandom-versions cross-graph note. |

### 2.3 Why `getrandom` `js` is load-bearing

Without the `js` feature, `getrandom 0.2.x` on `wasm32-unknown-unknown` errors out at compile time ("the wasm32-unknown-unknown target is not supported by default"). Since `solid-core`'s host-only dep tree pulls `rand` and `aes-gcm`, both of which transitively need `getrandom::getrandom` at runtime, the wasm32 build of `solid-core` fails immediately if this feature flag is missing. This crate's manifest (line 22) is the only place in the workspace declaring `getrandom` with the `js` feature; CI's wasm build job (`.github/workflows/ci.yml:182-209`) exercises this on every PR.

### 2.4 Dev-dependencies and tests

`wasm-bindgen-test = "=0.3.50"` is declared at `wasm/Cargo.toml:25` but `wasm/src/lib.rs` has zero `#[wasm_bindgen_test]` attributes and no `tests/` directory (verified — `wasm/` contains only `Cargo.toml` and `src/`). The dev-dep is therefore unused. Finding `M06-L01` covers this.

### 2.5 wasm-pack invocation contract

CLAUDE.md and `.github/workflows/ci.yml:202-203` both pin the invocation to:

```
wasm-pack build wasm/ --target nodejs \
    --out-dir ../ts-sdk/packages/core/wasm --release
```

`--target nodejs` is load-bearing. The bridge's design comment at `wasm/src/lib.rs:18-33` records that `--target nodejs` does NOT re-export `WebAssembly.Memory` on the JS module's exports. An earlier "shared buffer" zero-copy path crashed because of this; it has been removed and the file's docstring is now an explicit do-not-resurrect note. If a contributor flips to `--target web`, the snapshot of `--target nodejs`'s output that CI verifies (`solid_wasm.js`, `solid_wasm_bg.wasm`, `solid_wasm.d.ts`, `package.json`, see lines 206-209 in ci.yml) would not match.

The output is written to `ts-sdk/packages/core/wasm/`, which is the TS SDK's relative-import target; the consumer at `ts-sdk/packages/core/src/index.ts:77` does `await import('../wasm/solid_wasm.js')` relative to the compiled `dist/index.js`. The `wasm-pack` artifact path is therefore both a CI artifact and a runtime contract.

---

## 3. Per-export deep dive

Eleven `#[wasm_bindgen]` exports total: one start fn, plus ten public functions. There is also one private helper (`to_arr32`). Every public export returns `Result<T, JsError>`.

### 3.1 `init` — `wasm/src/lib.rs:13-16`

```rust
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}
```

`#[wasm_bindgen(start)]` runs once at module load (right after the JS-side `await import(...)`). The body is a single `set_once()`, idempotent. The hook turns wasm-bindgen's default `unreachable` trap into a readable JS stack via `console.error`.

JS consumer site: `ts-sdk/packages/core/src/index.ts:77`'s `await import(...)` triggers the start fn implicitly. `initWasm()` does NOT explicitly call it — it does not need to.

Concern: best-effort. If a panic does fire, the JS side gets a stack trace but the wasm linear-memory state is undefined. None of the exports below rely on `init()` having run first; `set_once` guards re-entry.

### 3.2 `poseidon_hash` (JS name `poseidonHash`) — `wasm/src/lib.rs:36-41`

`.d.ts` signature: `poseidonHash(fields: BigUint64Array): Uint8Array`.

Calls `solid_core::poseidon::hash_fields_to_bytes(fields)` and `map_err`s into `JsError`. The Rust side enforces 1..=12 inputs (`crates/solid-core/src/poseidon.rs:178-185`), so passing zero or >12 fields rejects with a structured error rather than panicking.

JS consumer: `ts-sdk/packages/core/src/index.ts:86-89`. The TS layer wraps the result in a fresh `Uint8Array` — wasm-bindgen for `&[u8]` already returns an owned copy via the reverse marshal path, so the TS-side `new Uint8Array(...)` is a defensive double-copy. Harmless; not required for correctness.

The smoke test at `scripts/wasm_bridge_smoke.mjs:33` exercises this with `[1n, 2n, 3n]` and asserts (a) length 32, (b) at least one non-zero byte. Two-of-five SOLID-SEC-010 checks. The cross-language vector test does NOT exercise `poseidonHash` directly (only via commitment / nullifier), so the BigUint64Array marshal path is only smoke-checked, not byte-pinned.

### 3.3 `poseidon_hash_bytes` (JS name `poseidonHashBytes`) — `wasm/src/lib.rs:44-60`

Signature: `poseidonHashBytes(inputs: Uint8Array): Uint8Array`.

The JS side at `ts-sdk/packages/core/src/index.ts:91-116` flattens its `Uint8Array[]` argument into one contiguous `Uint8Array` of length `n * 32` and passes that to wasm-bindgen, which marshals via `passArray8ToWasm0` (single-copy from JS heap to wasm linear memory). The Rust side reads the whole slice and chunks at line 49 with `chunks_exact(32)`.

Length validation: `inputs.len() % 32 != 0` returns `JsError`. The TS SDK ALSO validates each chunk is 32 bytes before flattening (`index.ts:108-112`), so the on-the-wire flat length is always a multiple of 32 in practice. Defence in depth: a JS caller bypassing the TS SDK still gets a structured error.

What is NOT validated on the TS side: the chunk count cap of 12 (Poseidon arity). The Rust side propagates the `solid_core::poseidon::hash_bytes` error when given >12 chunks, surfaced as JsError. Adequate, but inefficient — see `M06-L03`.

JS consumer: `ts-sdk/packages/core/src/index.ts:91-116`. Internal callers cascade through `computeIssuerLeaf` (line 286).

### 3.4 `generate_bjj_keypair` (JS name `generateBJJKeypair`) — `wasm/src/lib.rs:65-85`

`.d.ts` signature: returns `any` (because of `JsValue`). The TS layer types it via the local `BJJKeypair` interface at `ts-sdk/packages/core/src/index.ts:29-33`.

Body delegates to `solid_core::babyjubjub::generate_keypair()`, which calls `Fr::rand(&mut OsRng)` (`crates/solid-core/src/babyjubjub.rs:247`) on the wasm32 host. `OsRng` reaches `getrandom`, which on wasm32 dispatches to `crypto.getRandomValues` because of the `js` feature.

Returned shape: a local `KeypairResult` struct with `#[serde(rename_all = "camelCase")]` (line 71). JS sees:
```
{ privateKey: Uint8Array(32), publicKeyX: Uint8Array(32), publicKeyY: Uint8Array(32) }
```

The TS SDK then re-keys via `snakeCaseBjj(...)` at `ts-sdk/packages/core/src/index.ts:129-135` to produce a snake_case shape (`private_key`, `public_key_x`, `public_key_y`) for the public `BJJKeypair` interface. The shim is documented at lines 120-128 as deliberate: the SDK's public contract is snake_case.

`M06-L04`: this rename gymnastic is one camelCase-vs-snake_case typo away from a silent field-loss bug at the boundary.

### 3.5 `sign_message` (JS name `signMessage`) — `wasm/src/lib.rs:88-97`

Signature: `signMessage(privateKey: Uint8Array, message: Uint8Array): any`.

Flow:
- `to_arr32(private_key)?` and `to_arr32(message)?` validate length 32.
- `solid_core::babyjubjub::sign(&sk, &msg)` runs the EdDSA-Poseidon signing algorithm (`crates/solid-core/src/babyjubjub.rs:291-325`). Uses `Blake2b512` for the deterministic nonce, then `poseidon::hash_fr` for the challenge.
- Result is `EdDSASignature { r8_x, r8_y, s }` serialized via `serde_wasm_bindgen::to_value(&sig)?`.

The struct at `crates/solid-core/src/babyjubjub.rs:78-86` has NO `#[serde(rename_all)]`, so JS sees raw snake_case keys: `r8_x`, `r8_y`, `s`. The TS SDK passes them through unchanged at `ts-sdk/packages/core/src/index.ts:170-184`. **This is a different serialization convention from the keypair functions — keypairs are camelCase, signatures are snake_case. See `M06-L04`.**

Error paths:
- length-not-32 -> `JsError("Expected 32 bytes, got N")` (no field name).
- private key zero -> propagates `SolidError::Signature("Private key is zero")`.
- Poseidon failure path is unreachable in practice (always 5 Fq inputs, deterministic).

JS consumer: `ts-sdk/packages/core/src/index.ts:170-173`. Used by the issuer SDK (`ts-sdk/packages/issuer/src/index.ts:21,198`) for issuance signatures.

The smoke test does NOT exercise sign/verify — see Section 4.

### 3.6 `verify_signature` (JS name `verifySignature`) — `wasm/src/lib.rs:100-120`

Signature: `verifySignature(pubX: Uint8Array, pubY: Uint8Array, message: Uint8Array, r8x: Uint8Array, r8y: Uint8Array, s: Uint8Array): boolean`.

Six 32-byte slices in. All six pass through `to_arr32(...)`. If any fails, the JsError surfaces at the first failing slot (no early checking of all inputs; UX miss — see `M06-L05`).

Body delegates to `solid_core::babyjubjub::verify`. That function explicitly:
1. Decodes the public key with `pubkey_to_affine` (`crates/solid-core/src/babyjubjub.rs:161-172`), which fails-closed on off-curve, identity, and small-order (cofactor-8) points.
2. Decodes R8 with `EdwardsAffine::new_unchecked`, then explicitly checks `is_on_curve()` and `is_in_correct_subgroup_assuming_on_curve()`.
3. Recomputes `h = Poseidon(R8.x, R8.y, A.x, A.y, msg)`, reduces to BJJ scalar, checks `S * Base8 == R8 + h * A`.

JS consumer: `ts-sdk/packages/core/src/index.ts:175-185`. The smoke test does not exercise this. Cross-language vectors do not exercise it either. Finding `M06-M01`.

### 3.7 `compute_commitment` (JS name `computeCommitment`) — `wasm/src/lib.rs:128-148`

Signature: `computeCommitment(dataFields: BigUint64Array, schemaHash: Uint8Array, holderPubX: Uint8Array, holderPubY: Uint8Array, salt: Uint8Array): Uint8Array`.

Constructs `BJJPublicKey` from the two coord slices via `to_arr32`. Calls `solid_core::commitment::compute_attestation_commitment` (`crates/solid-core/src/commitment.rs:42-61`):
1. dataHash = Poseidon(data_fields[0..N-1])
2. commitment = Poseidon(dataHash, schemaHash, holderX, holderY, salt)

JS consumer: `ts-sdk/packages/core/src/index.ts:189-201`. Issuer SDK caller at `ts-sdk/packages/issuer/src/index.ts:198`.

**Cross-language vector test EXERCISES this** with the pinned vector at `tests/vectors/commitment_and_nullifier.json:18`: `expected_commitment_hex = 01a5b610...678782c`. This is one of two load-bearing byte-identity gates. If the wasm32 Poseidon backend (light-poseidon direct on wasm32, see `crates/solid-core/src/poseidon.rs:150-170`) ever drifts from the host fallback (solana-program's light-poseidon-backed path, lines 138-148), this test fails.

### 3.8 `compute_hardened_nullifier` (JS name `computeHardenedNullifier`) — `wasm/src/lib.rs:158-177`

Signature: `computeHardenedNullifier(masterKey: Uint8Array, revocationNonce: bigint, verifierAddress: Uint8Array, queryContextHash: Uint8Array, verifierNonce: Uint8Array, issuerTreeRoot: Uint8Array): Uint8Array`.

Six inputs (five 32-byte slices + one u64). Maps directly to `solid_core::nullifier::compute_nullifier` (`crates/solid-core/src/nullifier.rs:45-64`), implementing the ADR-0014 / SOLID-SEC-008 6-input preimage with `issuerTreeRoot` as the 6th input — the SOLID-SEC-008 epoch bind that ties every proof to a specific issuer-tree state.

JS consumer: `ts-sdk/packages/core/src/index.ts:220-233`. **Cross-language vector at lines 20-28 of the JSON file** pins `expected_nullifier_hex = 625c00be...c0ae761f`. Second load-bearing byte-identity gate.

**Important concern:** the `revocationNonce: bigint` parameter is u64-typed in Rust. wasm-bindgen marshals JS `bigint` to/from u64 by truncating to 64 bits. A JS caller that passes a value >= 2^64 silently loses high bits. Finding `M06-L06`.

### 3.9 `generate_identity` (JS name `generateIdentity`) — `wasm/src/lib.rs:182-189`

Signature: `generateIdentity(passphrase: string): string`.

Returns the JSON-serialized `BJJIdentity` (encrypted key bundle: AES-256-GCM ciphertext, Argon2id-derived key, 12-byte nonce, 32-byte salt). Calls `solid_core::babyjubjub::BJJIdentity::generate` then `.export_json()`.

Randomness: three `getrandom` calls — `generate_keypair`'s `Fr::rand`, `rand::random()` for salt, `rand::random()` for nonce (`crates/solid-core/src/babyjubjub.rs:419,425`). All go through the wasm32 `js` feature.

Argon2id parameters at `crates/solid-core/src/babyjubjub.rs:489`: m=65536, t=3, p=4. On wasm32 this is single-threaded synchronous; in node it blocks the JS event loop for 200-400ms. Not a security issue but a UX wart.

The function does NOT validate the passphrase at all (empty, single byte, unicode normalization forms all pass). Deliberate — Argon2id derives a key from any byte sequence.

JS consumer: `ts-sdk/packages/core/src/index.ts:297-300`.

### 3.10 `unlock_identity` (JS name `unlockIdentity`) — `wasm/src/lib.rs:192-200`

Signature: `unlockIdentity(identityJson: string, passphrase: string): Uint8Array`.

Calls `BJJIdentity::import_json(identity_json)` then `.unlock(...)`. Errors propagate at four sites:
- import_json failure (malformed JSON, unknown field) -> `SolidError::Serialization(...)`.
- AES-GCM cipher init failure -> `SolidError::Decryption("Cipher init: ...")`.
- Decrypt failure (wrong passphrase, tampered ciphertext) -> `SolidError::Decryption("Decrypt: ...")`.
- Plaintext length != 32 -> `SolidError::Decryption("Invalid key length")`.

Adversarial-input behaviour: passing a JSON object that parses to BJJIdentity but with empty / non-GCM-tagged ciphertext returns a structured `JsError` ("Decrypt: aead::Error") — no panic. Confirmed at `crates/solid-core/src/babyjubjub.rs:453-468`.

JS consumer: `ts-sdk/packages/core/src/index.ts:302-305`.

### 3.11 `derive_key` (JS name `deriveKey`) — `wasm/src/lib.rs:203-210`

Signature: `deriveKey(masterKey: Uint8Array, context: Uint8Array): Uint8Array`.

Two 32-byte slices in, one 32-byte digest out. `solid_core::babyjubjub::derive_key(&mk, &ctx)` -> `Poseidon(masterKey, context)` (see `crates/solid-core/src/babyjubjub.rs:270-272`). The deterministic sub-key derivation used for credential keys and nullifier keys.

JS consumer: `ts-sdk/packages/core/src/index.ts:307-310`.

### 3.12 `derive_credential_key` (JS name `deriveCredentialKey`) — `wasm/src/lib.rs:227-251`

Signature: returns `{ privateKey, publicKeyX, publicKeyY }` (camelCase, same shape as `generateBJJKeypair`).

Body:
1. `derive_key(master, schema)` -> 32-byte priv.
2. `derive_public_key(priv)` -> on-curve pubkey.

Runs the per-schema BJJ key derivation matching `circuits/lib/identity_anchor.circom`'s `IdentityAnchor` template (see docstring lines 213-226). **Critical for BUG-04 fix**: the SDK MUST use this when computing the per-schema identity leaf, not the master key directly.

JS consumer: `ts-sdk/packages/core/src/index.ts:325-331`. Used by the holder SDK and the end-to-end `prove.ts:122`. Result is `JsValue` because `derive_public_key` can fail (e.g., zero scalar — extremely unlikely but possible for adversarial schema_hash).

### 3.13 `compute_identity_state` (JS name `computeIdentityState`) — `wasm/src/lib.rs:255-269`

Signature: `computeIdentityState(pubkeyX: Uint8Array, pubkeyY: Uint8Array, revocationNonce: bigint): Uint8Array`.

Computes `Poseidon(pubX, pubY, revocationNonce_as_le_bytes)` via `solid_core::identity::IdentityState::commitment` (`crates/solid-core/src/identity.rs:22-30`). The u64 nonce is encoded into 32 bytes by zero-padding to high bytes, matching the on-chain encoding.

JS consumer: `ts-sdk/packages/core/src/index.ts:333-336`. Used by the holder SDK and `prove.ts:127`. Same `bigint` truncation concern as 3.8.

### 3.14 `is_bjj_in_prime_order_subgroup` (JS name `isBjjInPrimeOrderSubgroup`) — `wasm/src/lib.rs:278-285`

Signature: `isBjjInPrimeOrderSubgroup(pubkeyX: Uint8Array, pubkeyY: Uint8Array): boolean`.

Calls `solid_core::babyjubjub::is_in_prime_order_subgroup(&pk)` at `crates/solid-core/src/babyjubjub.rs:181-189`, the SOLID-SEC-007 cofactor-cleared subgroup check.

**This is the SEC-048-load-bearing client-side gate.** With the `sec007-skip-onchain` Cargo feature enabled on `issuer-registry` (localnet/devnet builds, per `docs/E2E_BLOCKERS.md` B9), the on-chain handler only runs `is_on_curve` + `!is_identity`; the full subgroup check happens HERE, off-chain. Every off-chain `register_issuer` caller MUST run it (`scripts/bootstrap_issuer.ts:47` already does).

The Rust function returns `bool` after internal short-circuits (off-curve / identity -> `false`). The wasm wrapper returns `Result<bool, JsError>` only because `to_arr32` length checks may fail. There is no path for the actual subgroup check to surface as JsError.

JS consumer: `ts-sdk/packages/core/src/index.ts:162-168`, with explicit SEC-048 documentation.

### 3.15 `to_arr32` helper — `wasm/src/lib.rs:289-299`

Plain Rust fn (NOT exported). Validates a `&[u8]` is exactly 32 bytes long, copies into `[u8; 32]`, returns `JsError("Expected 32 bytes, got N")` on mismatch. Used on every byte-slice argument across the bridge except the flat-buffer in `poseidon_hash_bytes`.

The single point that converts every "32-byte-slice" contract from wasm-bindgen's `&[u8]` ABI into a Rust `[u8; 32]`. Simple, correct, no panic path. Could optionally take a `&'static str` field name to improve error UX (M06-L05).

---

## 4. Smoke test coverage

### 4.1 Test wiring

`scripts/wasm_bridge_smoke.mjs:1-54` is the only end-to-end gate that proves the wasm-pack-built artifact is reachable through the SDK's pinned import path. CI invokes it from the `wasm_bridge_smoke` job at `.github/workflows/ci.yml:217-251` after the `wasm` build job completes. The CI invocation runs from `ts-sdk/packages/core/` so the script's `__dirname`-based path resolution lines up.

Path resolution at `scripts/wasm_bridge_smoke.mjs:23-25` uses `import.meta.url` to derive `__dirname` to `<repo>/scripts/`, then `resolve(__dirname, '../ts-sdk/packages/core/dist/index.js')` -> `<repo>/ts-sdk/packages/core/dist/index.js`. Correct (CWD-independent).

### 4.2 What is exercised

| Export | Smoke-tested? | Cross-lang vectored? | Fully covered? |
|--------|---------------|----------------------|----------------|
| `init` (`start`) | implicit (import succeeds) | n/a | yes |
| `poseidonHash` | yes (`[1n,2n,3n]` -> 32-byte non-zero, non-equal) | indirect (via commitment) | partial |
| `poseidonHashBytes` | yes (two 32-byte chunks) | indirect | partial |
| `generateBJJKeypair` | no | no | no |
| `signMessage` | no | no | no |
| `verifySignature` | no | no | no |
| `computeCommitment` | no | yes (vector pin) | byte-pinned |
| `computeHardenedNullifier` | no | yes (vector pin) | byte-pinned |
| `generateIdentity` | no | no | no |
| `unlockIdentity` | no | no | no |
| `deriveKey` | no | no | no |
| `deriveCredentialKey` | no | no | no |
| `computeIdentityState` | no | no | no |
| `isBjjInPrimeOrderSubgroup` | no | no | no |

The smoke test catches three failure modes:
1. WASM artifact missing or malformed (`solid_wasm.js` import fails).
2. wasm-pack output not in the expected directory (CI `test -f` gates at lines 206-209 already cover this).
3. Poseidon completely broken (digest is all zeros, OR both variants return identical output).

It does NOT catch:
- Wrong-arity Poseidon (wrong arity gives a non-zero digest, just the wrong one).
- Wrong-permutation Poseidon (e.g., x5 vs x3 mixup).
- Poseidon that drifts from circomlib byte-for-byte.
- Any failure in BJJ keygen, sign, verify, identity, derivation, or subgroup check.
- The `bigint` -> u64 truncation behaviour for nullifier/identity-state.

The byte-pinning for commitment and nullifier comes from `tests/vectors/check_vectors.ts`, but THAT script is NOT wired into the `wasm_bridge_smoke` CI job — it runs separately under the `cross_language_vectors` CI job. If that job is disabled or skipped, the smoke test alone would not catch a Poseidon-wasm32-vs-host divergence.

### 4.3 Gaps

`M06-M02`: smoke test does not cover sign/verify or any identity flow. A holder-side regression in `signMessage` or `unlockIdentity` would only surface in `npm run e2e` or production usage.

`M06-L01`: the unused `wasm-bindgen-test = "=0.3.50"` dev-dep would pay for itself if a `tests/wasm.rs` were added with one `#[wasm_bindgen_test]` per export. node-target wasm-bindgen-test runs in the same node interpreter as the smoke test, so it would need no extra CI plumbing.

---

## 5. Cross-language byte-identity

### 5.1 The byte-identity contract

Two Poseidon backends in this codebase:

1. **BPF + non-wasm32 host**: `solana_program::poseidon::hashv` — sol_poseidon syscall on BPF, light-poseidon-backed fallback on host (`crates/solid-core/src/poseidon.rs:138-148`).
2. **wasm32**: `light_poseidon::Poseidon::<Fr>::new_circom().hash_bytes_le` directly (same file, lines 150-170).

Both reduce to `light-poseidon 0.2.0`'s `hash_bytes_le` under `Bn254X5` / `LittleEndian` parameters by construction. The byte-identity guarantee follows from "same algebraic spec, same parameters" plus the canonicalisation step (`fr_to_bytes_le(&bytes_le_to_fr(...))`) applied to every input on both paths (lines 119-120).

### 5.2 Which exports are vectored

`tests/vectors/commitment_and_nullifier.json` covers two end-to-end primitives:

- **Commitment** (line 18). `expected_commitment_hex = 01a5b61022eba3f7dca36a556d27de1e8e2e0ea9894125a582526970a678782c`. Generated by `crates/solid-core/examples/gen_vectors.rs:59` on the host. Re-derived by `tests/vectors/check_vectors.ts:42-47` calling `computeCommitment` from `@solid-protocol/core`, dispatching through `wasm/src/lib.rs:128-148` -> `solid_core::commitment::compute_attestation_commitment` -> wasm32 Poseidon. Byte-identity proof for `compute_commitment` AND, transitively, for the `poseidon::hash_fields` and `poseidon::hash_fr` paths used inside it.

- **Nullifier** (line 27). `expected_nullifier_hex = 625c00be594f0e697cfbaef6907779f87f2976e894b49b444e1a735ac0ae761f`. Generated by `gen_vectors.rs:67` and re-derived by `check_vectors.ts:66`, calling `computeNullifier` -> `wasm/src/lib.rs:158-177` -> `solid_core::nullifier::compute_nullifier` -> wasm32 byte-oriented `hash_bytes` path. The 6-input nullifier preimage post-ADR-0014; `issuer_tree_root_hex` field at line 26.

### 5.3 What is NOT byte-identity-tested

The cross-language vector covers **2 of 11 exports**: commitment and nullifier. The remaining nine (poseidonHash directly, poseidonHashBytes directly, generateBJJKeypair, signMessage, verifySignature, generateIdentity, unlockIdentity, deriveKey, deriveCredentialKey, computeIdentityState, isBjjInPrimeOrderSubgroup) are not byte-pinned across Rust/wasm.

This is the gap CLAUDE.md flags as SOLID-SEC-010. Two of the missing seven that matter most for byte-identity:

- `computeIdentityState` — drives the global-tree leaf, BUG-04 path. Off-by-byte here causes proofs to fail on-chain Merkle-membership.
- `deriveCredentialKey` — drives the per-schema key derivation; if it drifts, the holder's identity-state leaf in the global tree doesn't match what the prover and verifier expect, breaking every proof for that schema.

Vectors for each would catch silent backend drift on the wasm32 side that the commitment + nullifier vectors might miss (since both go through `hash_bytes`, not the `hash_fr`/`hash_fields` Fr-typed paths). Finding `M06-H01`.

### 5.4 Three-target byte agreement

Three targets share the byte-identity contract:

1. BPF (issuer-registry / schema-registry on-chain).
2. Non-wasm32 host (`cargo test`, `gen_vectors`, off-chain prover).
3. wasm32 (audited here).

Targets 1 and 2 share `solana_program::poseidon::hashv`; their byte equality is asserted by the `test_byte_path_matches_field_path` regression test at `crates/solid-core/src/poseidon.rs:343-358`. Target 3 uses light-poseidon directly. Its byte-identity with target 2 follows from "same crate, same parameters". The `tests/vectors/check_vectors.ts` gate is the in-CI proof of this for the two vectored primitives.

This is the structural three-target split documented in `docs/E2E_BLOCKERS.md` B6 and `crates/solid-core/Cargo.toml:60-78` + `crates/solid-core/src/poseidon.rs:8-40`. Sound provided the cross-language vectors job runs green.

---

## 6. Findings

### M06-H01 — HIGH. Cross-language vector coverage 2 of 11 exports
**Location:** `tests/vectors/commitment_and_nullifier.json`, `crates/solid-core/examples/gen_vectors.rs:47-103`, `tests/vectors/check_vectors.ts:32-84`.
**Excerpt:** the vector covers `expected_commitment_hex` and `expected_nullifier_hex` only.
**Impact:** A wasm32 Poseidon drift (upstream light-poseidon version skew, wasm-bindgen ABI quirk corrupting byte ordering, an unintended `--target web` build) would pass the smoke test (which only checks non-zero) and fail downstream silently in proof generation. SOLID-SEC-010 explicitly calls this out.
**Recommendation:** Extend vectors to: `poseidonHash` (direct), `poseidonHashBytes` (direct), `computeIdentityState`, `deriveCredentialKey` (private + pub_x + pub_y triplet), `deriveKey`, plus one signature round-trip (sign + verify + tamper-detect). Wire the extended `check_vectors.ts` into the wasm_bridge_smoke CI job so a failing vector blocks the same PR that built the divergent artifact.

### M06-M01 — MEDIUM. Sign/verify path has zero in-WASM coverage
**Location:** `wasm/src/lib.rs:88-120`, smoke test `scripts/wasm_bridge_smoke.mjs`.
**Impact:** Any wasm32-side breakage in `sign_message` / `verify_signature` would not fail any CI job. The underlying `solid_core::babyjubjub::sign`/`verify` are well-tested on host (`crates/solid-core/src/babyjubjub.rs:526-552`), but the WASM-marshal layer (slice -> [u8;32], JsValue serde) is not.
**Recommendation:** Add a sign+verify+wrong-key+wrong-msg test to the smoke script. The needed bytes can be computed from `generateBJJKeypair` and `signMessage` themselves. ~25 lines.

### M06-M02 — MEDIUM. Identity (generate / unlock) path has zero coverage
**Location:** `wasm/src/lib.rs:182-200`, smoke test.
**Impact:** AES-256-GCM and Argon2id on wasm32 flow through `getrandom`. A missing `js` feature or misconfigured wasm-pack target surfaces as a runtime panic on every holder onboarding; smoke test would not catch it.
**Recommendation:** Add `generateIdentity('passphrase') -> unlockIdentity(json, 'passphrase') -> derive_public_key(unlocked)` roundtrip to the smoke script. ~10 lines.

### M06-L01 — LOW. Unused `wasm-bindgen-test` dev-dep
**Location:** `wasm/Cargo.toml:25`. Either delete or write at least one wasm-bindgen-test pinning a primitive byte-identity assertion.

### M06-L02 — LOW. Direct `serde_json` dep is dead weight
**Location:** `wasm/Cargo.toml:19`. Drop the line.

### M06-L03 — LOW. TS layer does not pre-cap Poseidon arity
**Location:** `ts-sdk/packages/core/src/index.ts:91-116`. Add `if (inputs.length < 1 || inputs.length > 12)` guard before flatten.

### M06-L04 — LOW. Inconsistent serde rename_all between bridge return shapes
**Location:** `wasm/src/lib.rs:71`, `wasm/src/lib.rs:236-243`, contrast `crates/solid-core/src/babyjubjub.rs:78-86`. `KeypairResult` and `DerivedKey` have `rename_all = "camelCase"`; `EdDSASignature` does not. Drop the renames; remove the `snakeCaseBjj` shim.

### M06-L05 — LOW. `to_arr32` error message lacks field name
**Location:** `wasm/src/lib.rs:289-299`. Pass an `&str` field name into `to_arr32` or split error mapping at each call site.

### M06-L06 — LOW. `bigint` -> u64 truncation is silent
**Location:** `wasm/src/lib.rs:159` (computeHardenedNullifier), `wasm/src/lib.rs:258` (computeIdentityState). Document the cap in `.d.ts` JSDoc; optionally add `if (val >= 2n**64n)` check on the TS side.

### M06-L07 — LOW. Smoke test `assert.notDeepEqual` is fragile
**Location:** `scripts/wasm_bridge_smoke.mjs:46-50`. Add a fixed expected-output assertion (pin one of the two digests to a known hex).

### M06-I01 — INFO. wasm-bindgen 0.2.100 pin will need re-evaluation
Tracked alongside the anchor 0.30.1 -> 0.31.x cutover.

### M06-I02 — INFO. js-sys 0.3.77 not actively imported
Defensive presence; keep.

### M06-I03 — INFO. Multiple getrandom versions in lockfile
Audit elsewhere (`03_dependencies/dependency_audit.md`).

---

## 7. Dependency notes — pinned-to-old-version risks

The wasm-bindgen pin is the load-bearing version constraint for the entire bridge. Today (2026-04-26):
- `wasm-bindgen 0.2.100` is the resolved version (Cargo.lock:2992).
- 0.2.118 raised wasm-bindgen-cli's MSRV to rustc 1.86 per `docs/E2E_BLOCKERS.md` B6.
- Our `rust-toolchain.toml` pins us to rustc 1.79.0.
- Bumping the rust pin is gated on bumping `anchor-lang` from 0.30.1 to 0.31.x (B7 in the same file).

Net: this bridge is structurally stuck on wasm-bindgen 0.2.100 until the larger toolchain upgrade lands. The pin is correct and documented, but it's a strict upper bound, not a floor: if a wasm-bindgen 0.2.101..0.2.117 ships a security advisory, we MUST be willing to bump within that range (and re-test that wasm-bindgen-cli still installs cleanly on rustc 1.79). The Cargo.lock already records 0.2.100, so a `cargo update -p wasm-bindgen --precise 0.2.117` is the in-range bump path.

Same reasoning applies to `js-sys = "=0.3.77"`, `wasm-bindgen-test = "=0.3.50"`, `serde-wasm-bindgen = "=0.6.5"` — all four versions move in lockstep with wasm-bindgen and were chosen to be the latest-known-working set under the rust 1.79 ceiling.

`getrandom 0.2.17` (resolved, Cargo.lock:1273-1283) has no open advisory affecting the wasm32 path. `console_error_panic_hook 0.1.7` is the latest published; stable since 2019.

---

## 8. Open questions / todos

1. **`#[wasm_bindgen(start)]` panic-hook idempotency.** `set_once()` is fine for single-instance loads; if usage moves to multi-iframe / multi-worker browser env, behaviour is well-defined but worth noting.
2. **Argon2id parameters not exposed.** `m=65536, t=3, p=4` (`crates/solid-core/src/babyjubjub.rs:489`). For wasm32 in constrained mobile-browser environments these may be too aggressive. No way to tune through the bridge today.
3. **`bigint` truncation contract.** As noted in M06-L06, the .d.ts says `bigint` but Rust receives `u64`. JS bigints can be arbitrarily large.
4. **No timing-side-channel hardening.** `sign_message` and `unlock_identity` (Argon2id) on wasm32 share the JS thread. Their timing leaks through `performance.now()` to any same-origin observer.
5. **No memory-zeroing on private inputs.** `to_arr32` leaves private-key bytes in wasm linear memory after the function returns.
6. **No regression test for the snake_case-vs-camelCase shim.**

---

## 9. Suggested next actions (priority order)

1. Extend cross-language vectors (M06-H01). Closes SOLID-SEC-010 partially. ~150 LOC.
2. Add a sign+verify+identity round-trip to the smoke test (M06-M01 + M06-M02). ~50 LOC.
3. Drop `serde_json` from `wasm/Cargo.toml` and decide on `wasm-bindgen-test` (M06-L02 + M06-L01).
4. Decide on the camelCase/snake_case wire shape (M06-L04). Single-PR refactor.
5. Pin a fixed expected output in the smoke test (M06-L07). One line.
6. Document the `bigint` truncation contract (M06-L06). ~5 lines.

None of (1)-(6) require a circuit change, on-chain redeploy, or trusted-setup re-run. Item (1) is the only HIGH-rated finding and should land before the external audit cycle on the post-Phase-2 codebase.

---

## Summary

1 HIGH, 2 MEDIUM, 7 LOW, 3 INFO findings. ADR-0002 / SOLID-SEC-028 invariant verified: no `#[wasm_bindgen]` symbol exists outside `wasm/src/lib.rs`. Every export reviewed at signature + body + JS-consumer level. Cross-language byte-identity is sound on the two vectored primitives; the SOLID-SEC-010 gap is the only real soundness risk in the bridge today.
