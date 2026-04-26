# SolID Protocol v0.6.1 — Modular Audit, 03 Dependencies

## 1. Header

- Repository: solid-protocol
- Branch / commit basis: main, version v0.6.1 (post Phase-3-impl-4)
- Audit date: 2026-04-26
- Auditor: delegated dependency / supply-chain specialist (agent), persisted by orchestrator
- Scope: every `Cargo.toml`, `package.json`, lockfile, and toolchain-pin file in the repository, plus the two `rust-toolchain.toml` pins and `.cargo/config.toml`.
- Companion docs: `CLAUDE.md` (canonical toolchain pins); `docs/E2E_BLOCKERS.md` (per-pin reasoning, B2 / B6 / B7); `Cargo.toml:21-27` (workspace MSRV); `sec/audits/2026-04-25_v0.6.1_deep_comprehensive_audit.md`.
- Method: static read of every manifest + every lockfile line. Cross-referenced declared deps against actual Rust `use ...` / `import` graphs to detect dead deps. Cargo.lock pulled into per-crate multiplicity tables to detect version drift. No runtime dep tools invoked (no `cargo audit`, no `npm audit` — sandbox limitation; advisory data below is documented knowledge-of-record current to early 2026 and cited as such).
- Severity scale:
  - **CRITICAL** — known-exploitable advisory in the active (built) dep graph.
  - **HIGH** — exploitable advisory in a code path the protocol exercises, OR a structural coherence break that endangers soundness invariants.
  - **MEDIUM** — known advisory in a non-exercised code path, or a drift that would block clean reproducibility.
  - **LOW** — hygiene: dead deps, redundant pins, doc-vs-manifest drift.
  - **INFO** — observation, not actionable.

---

## 2. Toolchain pins audit

### 2.1 Inventory

| Component       | Pin                | Where pinned                                                         |
|-----------------|--------------------|-----------------------------------------------------------------------|
| Rust (workspace) | 1.79.0 (channel)   | `rust-toolchain.toml:21-24`                                           |
| Rust (workspace MSRV) | 1.75 (resolver hint) | `Cargo.toml:27`                                                  |
| Rust (prover)   | 1.79.0 (channel)   | `tools/solid-prover/rust-toolchain.toml:5-7`                          |
| Rust (IDE / rust-analyzer) | 1.79.0 (env)| `.vscode/settings.json:25-30`                                       |
| Cargo resolver  | `incompatible-rust-versions = "fallback"` (cargo >= 1.84) | `.cargo/config.toml:16-17` |
| Solana CLI      | 1.18.22            | `Anchor.toml [toolchain] solana_version`, CLAUDE.md                   |
| anchor-cli      | 0.30.1             | `Anchor.toml [toolchain] anchor_version`, CLAUDE.md                   |
| anchor-lang / anchor-spl | 0.30.1     | `Cargo.toml:43-44`                                                    |
| solana-program  | ~1.18              | `Cargo.toml:50` (resolves to 1.18.26)                                 |
| Circom          | 2.1.9              | CLAUDE.md (system binary, no in-tree pin)                              |
| snarkjs (root scripts) | ^0.7.4      | `package.json:21`                                                      |
| snarkjs (circuits) | ^0.7.0          | `circuits/package.json:18`                                             |
| snarkjs (toolchain shim) | ^0.7.5    | `.toolchain/npm/package.json:3`                                        |
| snarkjs (holder SDK) | ^0.7.0        | `ts-sdk/packages/holder/package.json:16`                               |
| wasm-pack       | 0.13.1             | CLAUDE.md (system binary)                                              |
| wasm-bindgen    | =0.2.100 (exact)   | `wasm/Cargo.toml:16`                                                   |
| Node            | 18+ (npm 10+)      | CLAUDE.md (no `engines` field anywhere)                                |
| TypeScript      | ^5.3.0 (most pkgs), ^5.9.3 (`.toolchain/npm`) | every `package.json` `devDependencies.typescript` |
| ts-node         | ^10.9.2            | `.toolchain/npm/package.json:4`                                        |
| tsx             | ^4.21.0            | `package.json:30`                                                      |
| turbo           | ^2.0.0             | `ts-sdk/package.json:14`                                               |

### 2.2 Coherence findings

- All three Rust pins (root `rust-toolchain.toml`, `tools/solid-prover/rust-toolchain.toml`, `.vscode/settings.json`) say `1.79.0`. Coherent.
- MSRV (`rust-version = "1.75"` at `Cargo.toml:27`) < channel (`1.79.0`) — intentional. Workspace declares lower MSRV so cargo's MSRV-aware resolver (cargo >= 1.84, opted in via `.cargo/config.toml:16-17`) can pick transitives whose MSRV is `<= 1.75`, preventing the edition2024 cascade documented in `docs/E2E_BLOCKERS.md` B2. Coherent and load-bearing.
- anchor 0.30.1 + solana-program 1.18.x + spl-account-compression 0.2.1 — version triplet is the documented Anchor 0.30.1 baseline. Coherent for compilation; ark-* multiplicity is **DEP-M01** below.
- snarkjs version drift across four manifests (`^0.7.0` / `^0.7.4` / `^0.7.5`). All resolve to `0.7.6`. Functionally coherent today but pin set is inconsistent with CLAUDE.md `0.7.5` exact. Logged as **DEP-L01**.
- Anchor IDL build with proc-macro2 >= 1.0.95 — known Anchor 0.30.1 upstream limitation (`docs/E2E_BLOCKERS.md` B7). `Cargo.lock:1840` shows `proc-macro2 1.0.94`, on the safe side. Future `cargo update` will lift it; resolver-fallback config does NOT protect (proc-macro2 declares `rust-version = 1.63`). Logged as **DEP-M02**.
- `wasm-bindgen = "=0.2.100"` at `wasm/Cargo.toml:16` with comment block at `wasm/Cargo.toml:12-15` documenting upgrade gate (newer 0.2.118+ raises wasm-bindgen-cli MSRV to rustc 1.86, conflicts with 1.79.0). Coherent.
- No `engines` in any `package.json`. CLAUDE.md says Node 18+, nothing enforces it. Logged as **DEP-L02**.

---

## 3. Rust workspace deps (root)

Root workspace at `Cargo.toml:1-10` has six members: `crates/solid-core`, `crates/solid-light`, `programs/zk-verifier`, `programs/issuer-registry`, `programs/schema-registry`, `wasm`. The off-chain `tools/solid-prover/` is intentionally a separate workspace (Section 4).

### 3.1 `Cargo.toml` (root, workspace deps)

`Cargo.toml:30-50`:

| Dep              | Pin       | Purpose / reason                                                                  |
|------------------|-----------|------------------------------------------------------------------------------------|
| `serde`          | 1.0       | universal serialization (with `derive`)                                            |
| `serde_json`     | 1.0       | host-side credential JSON; not on BPF                                              |
| `num-bigint`     | 0.4 (`serde`, `rand`) | scalar / field arithmetic helpers                                       |
| `num-traits`     | 0.2       | numerical trait companion                                                          |
| `ark-bn254`      | 0.4       | BN254 host arithmetic; pinned 0.4 because solana-program 1.18 uses 0.4             |
| `ark-ff`         | 0.4       | finite-field traits (matches ark-bn254)                                            |
| `ark-ec`         | 0.4       | elliptic-curve traits                                                              |
| `ark-ed-on-bn254`| 0.4       | BabyJubJub                                                                          |
| `ark-std`        | 0.4       | arkworks std shim                                                                  |
| `ark-groth16`    | 0.4       | declared as workspace dep but only consumed via `tools/solid-prover/`. **DEP-L03** |
| `rand`           | 0.8       | RNG; host-only paths in solid-core                                                 |
| `hex`            | 0.4       | hex encode/decode                                                                  |
| `thiserror`      | 1.0       | error derives                                                                       |
| `anchor-lang`    | 0.30.1    | Anchor program framework                                                            |
| `anchor-spl`     | 0.30.1    | SPL token CPI helpers                                                              |
| `solana-program` | ~1.18     | Solana SDK; resolves to 1.18.26                                                    |

### 3.2 `crates/solid-core/Cargo.toml`

| Dep                   | Pin      | Line | Concern |
|-----------------------|----------|------|---------|
| `ark-bn254`           | workspace | 38 | none |
| `ark-ff`              | workspace | 39 | none |
| `ark-ec`              | workspace | 40 | none |
| `ark-ed-on-bn254`     | workspace | 41 | none |
| `ark-std`             | workspace | 42 | none |
| `serde`               | workspace | 43 | none |
| `num-bigint`          | workspace | 44 | used in `multi_cred.rs` |
| `num-traits`          | workspace | 45 | INFO: no `use num_traits` in src/; consumed transitively. **DEP-L04** |
| `hex`                 | workspace | 46 | used by tests / examples |
| `thiserror`           | workspace | 47 | used by `error.rs` |
| `solana-program` (BPF) | workspace | 56 | ADR-0002 BPF Poseidon syscall |
| `solana-program` (host) | workspace | 78 | host fallback to byte-identical light-poseidon path |
| `light-poseidon`      | "0.2"    | 112 | host-only Poseidon backend; pinned 0.2.0 because round-constants table size is the load-bearing input to the BPF stack-overflow analysis (B3). **DEP-H01** |
| `serde_json`          | workspace | 113 | host JSON only |
| `rand`                | workspace | 114 | host RNG |
| `aes-gcm`             | "0.10"   | 115 | host AEAD |
| `argon2`              | "0.5"    | 116 | host KDF |
| `blake2`              | "0.10"   | 117 | host hash |
| `ark-std` (dev)       | workspace | 120 | tests |
| `anyhow` (dev)        | "1.0"    | 121 | examples (`gen_vectors.rs`) |

The triple-arm cfg dispatch (`cfg(target_os = "solana")`, `cfg(all(not(target_os = "solana"), not(target_arch = "wasm32")))`, `cfg(not(target_os = "solana"))`) is documented in detail in the file's comment block. Coherent with `docs/E2E_BLOCKERS.md` B6.

### 3.3 `crates/solid-light/Cargo.toml`

| Dep            | Pin              | Line | Concern |
|----------------|------------------|------|---------|
| `anchor-lang`  | workspace 0.30.1 | 12 | used |
| `borsh`        | "0.10"           | 13 | used (re-export of Anchor's borsh) |
| `hex`          | "0.4"            | 14 | **DEP-L05: zero `hex::` references in `crates/solid-light/src/`. Dead.** |

### 3.4 `programs/zk-verifier/Cargo.toml`

| Dep              | Pin              | Line | Concern |
|------------------|------------------|------|---------|
| `anchor-lang`    | workspace 0.30.1 (`init-if-needed`) | 20 | used |
| `groth16-solana` | "0.2"            | 21 | resolves to 0.2.0; sole driver of ark-0.5 in the on-chain workspace. **DEP-H02** |
| `solid-light`    | path             | 22 | used |

`groth16-solana 0.2.0` (`Cargo.lock:1308-1320`) pulls a parallel ark-bn254/ark-ec/ark-ff/ark-serialize **0.5.0** tree alongside solana-program's 0.4.x tree.

### 3.5 `programs/issuer-registry/Cargo.toml`

| Dep                  | Pin                                    | Line | Concern |
|----------------------|----------------------------------------|------|---------|
| `anchor-lang`        | workspace 0.30.1 (`init-if-needed`)    | 40 | used |
| `anchor-spl`         | workspace 0.30.1                       | 41 | used (`use anchor_spl::token::{...}` at `programs/issuer-registry/src/lib.rs:8`) |
| `solid-light`        | path                                   | 44 | used |
| `schema-registry`    | path (`no-entrypoint`)                 | 48 | used (`SchemaAccount` re-use) |
| `solid-core`         | path                                   | 52 | used (`compute_issuer_leaf`) |

Cargo feature `sec007-skip-onchain` at `programs/issuer-registry/Cargo.toml:37` is the SEC-048 escape hatch; documented "NOT FOR MAINNET". Build-flag concern: any release-build pipeline that conditionally enables features needs an explicit deny-list for this flag on mainnet targets.

### 3.6 `programs/schema-registry/Cargo.toml`

| Dep            | Pin              | Line | Concern |
|----------------|------------------|------|---------|
| `anchor-lang`  | workspace 0.30.1 | 19 | used |
| `solid-core`   | path             | 20 | used |

Comment block (`programs/schema-registry/Cargo.toml:21-28`) records the removal of a previously-direct `light-poseidon = "0.2"` dep that was unused and the primary BPF stack-overflow vector. Clean.

### 3.7 `wasm/Cargo.toml`

| Dep                       | Pin       | Line | Concern |
|---------------------------|-----------|------|---------|
| `solid-core`              | path      | 11 | used |
| `wasm-bindgen`            | =0.2.100  | 16 | EXACT pin; gating reasoning at lines 12-15 |
| `serde-wasm-bindgen`      | =0.6.5    | 17 | EXACT pin (matches wasm-bindgen 0.2.100 ABI) |
| `serde`                   | 1.0 (`derive`) | 18 | used |
| `serde_json`              | 1.0       | 19 | INFO: no `use serde_json` in `wasm/src/lib.rs`. **DEP-L06** |
| `js-sys`                  | =0.3.77   | 20 | EXACT pin; matches wasm-bindgen 0.2.100 |
| `console_error_panic_hook`| =0.1.7    | 21 | used (start hook) |
| `getrandom`               | 0.2 (`js`) | 22 | wasm32 RNG |
| `wasm-bindgen-test` (dev) | =0.3.50   | 25 | tests |

Every `=` pin matches the wasm-bindgen 0.2.100 line. Lockfile (`Cargo.lock:2993-3084`) confirms.

---

## 4. Off-chain prover deps (`tools/solid-prover/`)

### 4.1 Why a separate workspace

`tools/solid-prover/Cargo.toml:1-9` is the canonical justification: `ark-circom 0.5.0-alpha` hard-pins `num-bigint = 0.4.3`, which conflicts with the `num-bigint` version pulled by `groth16-solana` in the main workspace. Specifically:
- Main workspace `num-bigint` resolves to `0.4.6` (`Cargo.lock:1650-1659`).
- `ark-circom 0.5.0-alpha` uses `num-bigint = "=0.4.3"`. Putting both in one workspace forces a downgrade across the on-chain dep graph, which empirically re-opens the BPF stack frame issue from `docs/E2E_BLOCKERS.md` B3 / B4.
- Two workspaces is the clean fix; CI exercises both.

### 4.2 `tools/solid-prover/Cargo.toml`

| Dep                  | Pin           | Line | Concern |
|----------------------|---------------|------|---------|
| `solid-core`         | path          | 26 | used |
| `ark-bn254`          | "0.4"         | 27 | matches solid-core |
| `ark-ff`             | "0.4"         | 28 | matches |
| `ark-groth16`        | "0.4"         | 29 | the prover's main work |
| `ark-relations`      | "0.4"         | 30 | R1CS plumbing |
| `ark-serialize`      | "0.4"         | 31 | proving key (de)serialization |
| `ark-std`            | "0.4"         | 32 | matches |
| `ark-circom`         | "0.5.0-alpha" | 33 | alpha; version-conflict source. **DEP-M03** |
| `ark-snark`          | "0.4"         | 34 | matches |
| `sha2`               | "0.10"        | 35 | proving-key digest |
| `thiserror`          | "1.0"         | 36 | error |
| `serde`              | "1.0"         | 37 | used |
| `serde_json`         | "1.0"         | 38 | proving artifacts |
| `hex`                | "0.4"         | 39 | used |
| `rand`               | "0.8"         | 40 | proof randomness |
| `num-bigint`         | "0.4" (`serde`, `rand`) | 41 | will be forced to 0.4.3 by ark-circom |
| `anyhow` (dev)       | "1.0"         | 44 | tests |

### 4.3 Lockfile state

**There is no `tools/solid-prover/Cargo.lock` in the repo.** `find` confirms only the root `Cargo.lock` and a stray scratch lockfile. The prover's lock is generated on first `cargo build` and is not currently checked in.

**DEP-M04 (MEDIUM):** without a checked-in `tools/solid-prover/Cargo.lock`, the prover's reproducibility depends on transitive defaults at first-build time. For a release artifact (the host-side prover used by issuers and holders), this is a supply-chain hole: `cargo update`-class drift in `ark-circom`'s alpha series, in `tiny-keccak`/`zerocopy`-style transitives, or in any sha2 patch can be introduced silently. Recommended fix is to commit the prover's `Cargo.lock` (Cargo idiom for binary crates) and run `cargo audit` against it in CI.

### 4.4 Profile

`tools/solid-prover/Cargo.toml:46-49` sets `lto = "fat"` — intentional for the off-chain prover (no BPF stack constraint). The on-chain workspace explicitly does NOT set `lto = "fat"` (root `Cargo.toml:62-93` documents the BPF stack-overflow rationale for `lto = "thin"`).

---

## 5. TS / npm deps

### 5.1 Root `package.json`

| Dep                          | Pin     | Line | Concern |
|------------------------------|---------|------|---------|
| `@solana/web3.js`            | ^1.95.0 | 17 | resolves to 1.98.4 |
| `@solana/spl-token`          | ^0.4.6  | 18 | OK |
| `@coral-xyz/anchor`          | ^0.30.1 | 19 | matches anchor-lang 0.30.1 |
| `snarkjs`                    | ^0.7.4  | 20 | resolves to 0.7.6; CLAUDE.md says 0.7.5 (DEP-L01) |
| `ffjavascript`               | ^0.2.63 | 21 | OK |
| `@solid-protocol/*` (5 pkgs) | file:./ts-sdk/packages/* | 22-26 | workspace symlinks |
| `tsx` (dev)                  | ^4.21.0 | 30 | OK |
| `typescript` (dev)           | ^5.3.0  | 31 | OK |
| `@types/node` (dev)          | ^20.11.0 | 32 | INFO: ts-sdk lockfile resolves `@types/node 25.6.0` from web3.js peer chain — drift between root and ts-sdk allowed but worth noting |

**No `package-lock.json` at the repo root.** Logged as **DEP-L07**: consider committing one so `npm ci` at the repo root is reproducible.

### 5.2 `ts-sdk/package.json`

Workspace root declares only `turbo ^2.0.0` and `typescript ^5.3.0`. Lockfile is `ts-sdk/package-lock.json` (lockfileVersion 3, 2019 lines).

#### 5.2.1 `ts-sdk/packages/core/package.json`

| Dep | Pin | Concern |
|-----|-----|---------|
| `typescript` (dev) | ^5.3.0 | OK |

The actual WASM bridge artifact ships under `ts-sdk/packages/core/wasm/` with its own `package.json` (no dependencies declared). Coherent — the `.wasm` blob is self-contained.

#### 5.2.2 `ts-sdk/packages/holder/package.json`

| Dep | Pin (line) | Concern |
|-----|------------|---------|
| `@solid-protocol/core` | 0.1.0 (13) | workspace |
| `@solid-protocol/light` | 0.2.0 (14) | workspace |
| `@solana/web3.js` | ^1.95.0 (15) | OK |
| `snarkjs` | ^0.7.0 (16) | resolves 0.7.6; pin range wider than CLAUDE.md (DEP-L01) |
| `typescript` (dev) | ^5.3.0 | OK |

#### 5.2.3 `ts-sdk/packages/issuer/package.json`

| Dep | Pin (line) | Concern |
|-----|------------|---------|
| `@solid-protocol/core` | 0.1.0 (13) | workspace |
| `@solid-protocol/light` | 0.2.0 (14) | workspace |
| `@solana/web3.js` | ^1.95.0 (15) | OK |
| `bn.js` | ^5.2.1 (16) | resolves 5.2.3 |
| `@types/bn.js` (dev) | ^5.1.5 | OK |
| `typescript` (dev) | ^5.3.0 | OK |

#### 5.2.4 `ts-sdk/packages/light/package.json`

| Dep | Pin (line) | Concern |
|-----|------------|---------|
| `@solana/web3.js` | ^1.95.0 (13) | OK |
| `@solana/spl-account-compression` | ^0.2.1 (14) | resolves 0.2.1 |
| `bn.js` | ^5.2.1 (15) | OK |
| `@types/bn.js` (dev) | ^5.1.5 | OK |
| `typescript` (dev) | ^5.3.0 | OK |

#### 5.2.5 `ts-sdk/packages/sdk/package.json`

| Dep | Pin (line) | Concern |
|-----|------------|---------|
| `@solid-protocol/core` | 0.1.0 (12) | workspace |
| `@solid-protocol/holder` | 0.2.0 (13) | workspace |
| `@solid-protocol/issuer` | 0.2.0 (14) | workspace |
| `@solid-protocol/light` | 0.2.0 (15) | workspace |
| `@solid-protocol/verifier` | 0.1.0 (16) | workspace |
| `@coral-xyz/anchor` | ^0.30.1 (17) | OK |
| `@solana/web3.js` | ^1.95.0 (18) | OK |
| `buffer` | ^6.0.3 (19) | OK |
| `typescript` (dev) | ^5.3.0 | OK |

**Internal version drift:** `core` ships at 0.1.0 while everything else is at 0.2.0. **DEP-L08**.

#### 5.2.6 `ts-sdk/packages/verifier/package.json`

| Dep | Pin (line) | Concern |
|-----|------------|---------|
| `@solid-protocol/core` | 0.1.0 (12) | workspace |
| `@coral-xyz/anchor` | ^0.30.1 (13) | OK |
| `@solana/web3.js` | ^1.95.0 (14) | OK |
| `typescript` (dev) | ^5.3.0 | OK |

### 5.3 Toolchain shim (`.toolchain/npm/package.json`)

| Dep | Pin | Concern |
|-----|-----|---------|
| `snarkjs` | ^0.7.5 | matches CLAUDE.md target; differs from root `^0.7.4` and packages `^0.7.0` (DEP-L01) |
| `ts-node` | ^10.9.2 | only consumer of ts-node in repo (root uses tsx). Possibly dead. **DEP-L09** |
| `typescript` | ^5.9.3 | drifts from `^5.3.0` everywhere else; INFO |

### 5.4 ts-sdk lockfile observations

Resolutions of interest:
- `@solana/web3.js` -> 1.98.4 (pulled in as `peer` of multiple sub-packages).
- `@coral-xyz/anchor` -> 0.30.1.
- `snarkjs` -> 0.7.6.
- `bn.js` -> 5.2.3 (one version, clean).
- `borsh` -> 0.7.0 (pulled by `@solana/web3.js` 1.98.4) coexists with `@coral-xyz/borsh 0.30.1`. Both present, neither broken; standard JS borsh ecosystem fragmentation.
- `node-fetch` -> 2.7.0. v2 line in deprecation maintenance mode upstream; modern web3.js 2.x uses fetch native. Until SDK bumps to web3.js 2.x, this is the supported v1 path.
- `ws` -> 7.5.10. Resolution at exactly 7.5.10 IS the patched version against GHSA-3h5v-q93c-6h6q. **DEP-INFO01**.
- `crypto-hash` 1.3.0 — pulled by `@coral-xyz/anchor`. Small wrapper around node `crypto`. INFO.
- `jayson` -> 4.3.0 with `commander 2.20.3` and `uuid 8.3.2`.
- Optional native binaries: `bufferutil`, `utf-8-validate` (peer-optional under `ws`). Not built by default.
- `turbo` 2.9.6 with optional platform binaries; standard distribution shape.

---

## 6. Circuit npm deps (`circuits/package.json`)

| Dep | Pin | Line | Resolved | Concern |
|-----|-----|------|----------|---------|
| `circomlib` | ^2.0.5 | 11 | 2.0.5 (`circuits/package-lock.json:414-419`) | OK |
| `mocha` (dev) | ^10.2.0 | 15 | 10.8.2 | OK |
| `chai` (dev) | ^4.3.7 | 16 | 4 series | OK (kept on chai 4 vs ESM-only chai 5; **DEP-INFO07**) |
| `snarkjs` (dev) | ^0.7.0 | 17 | 0.7.6 | drifts from CLAUDE.md 0.7.5 (DEP-L01) |
| `ffjavascript` (dev) | ^0.2.59 | 18 | 0.2.63 | OK; snarkjs 0.7.6 uses 0.3.1 internally so two ffjavascript versions resolve in the lockfile (INFO, expected) |
| `circom_tester` (dev) | ^0.0.20 | 19 | 0.0.20 | pre-1.0 hobbyist crate, low maintenance cadence. **DEP-INFO02** |

Notes:
- circom (the compiler, `circom 2.1.9`) is a system binary, not an npm dep. Consider adding a precondition check in `circuits/scripts/setup.js`.
- `circomlib 2.0.5` is the only production dep (`dependencies`); everything else is in `devDependencies`. Coherent.
- `wasmcurves 0.2.2` and `wasmbuilder 0.0.16` are reached transitively via `ffjavascript`. iden3-maintained. INFO.

---

## 7. Cross-workspace coherence (Cargo.lock multiplicity)

The root `Cargo.lock` (3211 lines, 259 unique crate names; some crates resolve to multiple versions) was scanned for same-crate-different-version situations. Notable:

| Crate           | Versions present                       | Pulled by                                                                                 | Concern |
|-----------------|----------------------------------------|--------------------------------------------------------------------------------------------|---------|
| `aead`          | 0.4.3, 0.5.2                            | 0.4 via `aes-gcm-siv`->solana-zk-token-sdk, 0.5 via `aes-gcm` 0.10.3                      | INFO |
| `aes`           | 0.7.5, 0.8.4                            | 0.7 via aes-gcm-siv, 0.8 via aes-gcm                                                      | INFO |
| `ahash`         | 0.7.8, 0.8.12                           | 0.7 via hashbrown 0.11.2, 0.8 via hashbrown 0.13/0.15 (ark-* 0.5)                          | INFO |
| `ark-bn254`     | **0.4.0, 0.5.0**                        | 0.4 via solana-program / solana-bn254 / solid-core; 0.5 via groth16-solana                | **DEP-M01** |
| `ark-ec`        | **0.4.2, 0.5.0**                        | mirrored                                                                                   | DEP-M01 |
| `ark-ff`        | **0.4.2, 0.5.0**                        | mirrored                                                                                   | DEP-M01 |
| `ark-poly`      | **0.4.2, 0.5.0**                        | mirrored                                                                                   | DEP-M01 |
| `ark-serialize` | **0.4.2, 0.5.0**                        | mirrored                                                                                   | DEP-M01 |
| `ark-std`       | **0.4.0, 0.5.0**                        | mirrored                                                                                   | DEP-M01 |
| `base64`        | 0.12.3, 0.21.7                          | 0.12 via libsecp256k1, 0.21 via solana-program / solana-zk-token-sdk                       | INFO |
| `block-buffer`  | 0.9.0, 0.10.4                           | 0.9 via sha2 0.9, 0.10 via sha2 0.10                                                       | INFO |
| `borsh`         | **0.9.3, 0.10.4, 1.5.7**                | solana-program reaches all three (`Cargo.lock:2329-2331`); anchor uses 0.10; spl-token-2022 uses 1.5.7 | **DEP-M05** |
| `bs58`          | 0.4.0, 0.5.1                            | 0.4 via solana-program, 0.5 via anchor-* attribute crates                                  | INFO |
| `cipher`        | 0.3.0, 0.4.4                            | mirrors aes split                                                                           | INFO |
| `ctr`           | 0.8.0, 0.9.2                            | mirrors aes split                                                                           | INFO |
| `digest`        | 0.9.0, 0.10.7                           | sha2 0.9 vs 0.10                                                                            | INFO |
| `getrandom`     | 0.1.16, 0.2.17, 0.3.4                   | 0.1 via tiny-bip39 / rand 0.7, 0.2 via solana-program / wasm crate, 0.3 via jobserver       | INFO |
| `hashbrown`     | 0.11.2, 0.13.2, 0.15.2                  | 0.11 via ahash 0.7, 0.13 via ark-ec 0.4 / borsh 0.10, 0.15 via ark-ec 0.5 / indexmap        | INFO |
| `hmac`          | 0.8.1, 0.12.1                           | 0.8 via tiny-bip39, 0.12 via solana-sdk / ed25519-dalek-bip32                               | INFO |
| `itertools`     | 0.10.5, 0.13.0                          | 0.10 via solana-program, 0.13 via ark-ec 0.5                                                | INFO |
| `pbkdf2`        | 0.4.0, 0.11.0                           | 0.4 via tiny-bip39, 0.11 via solana-sdk                                                     | INFO |
| `polyval`       | 0.5.3, 0.6.2                            | mirrors aead split                                                                          | INFO |
| `proc-macro-crate` | 0.1.5, 3.3.0                         | 0.1 via borsh 0.9 / 0.10, 3.3 via num_enum / borsh 1.5.7                                    | INFO |
| `rand`          | 0.7.3, 0.8.6                            | 0.7 via tiny-bip39 / libsecp256k1 / ed25519-dalek 1, 0.8 via solana-program / solid-core    | INFO |
| `rand_chacha`, `rand_core` | (mirrored)                  | mirrors rand split                                                                          | INFO |
| `sha2`          | 0.9.9, 0.10.9                           | 0.9 via libsecp256k1 / ed25519-dalek 1, 0.10 via solana-program / spl-*                     | INFO |
| `sha3`          | 0.9.1, 0.10.9                           | 0.9 via solana-zk-token-sdk, 0.10 via solana-program                                        | INFO |
| `syn`           | 1.0.109, 2.0.117                        | 1.x via anchor-syn / borsh-derive / ark-* 0.4, 2.x via wasm-bindgen / ark-* 0.5 / serde     | INFO; standard ecosystem split |
| `thiserror`     | 1.0.69, 2.0.18                          | 1.0 broadly, 2.0 only via solana-bn254 (`Cargo.lock:2258`)                                  | INFO |
| `toml`          | 0.5.11, 0.8.23                          | 0.5 via proc-macro-crate 0.1.5, 0.8 via cargo_toml (anchor-syn)                             | INFO |
| `universal-hash`| 0.4.1, 0.5.1                            | mirrors polyval split                                                                       | INFO |
| `wasi`          | 0.9.0, 0.11.1                           | 0.9 via getrandom 0.1, 0.11 via getrandom 0.2                                               | INFO |

The two material concerns:

- **DEP-M01** — six ark-* crates at both 0.4.x and 0.5.x. The 0.5 tree comes only from `groth16-solana 0.2.0`; 0.4 feeds everything else. Both trees linked into zk-verifier's `.so`. Functionally correct (both compute the same group operations); doubles BN254 arithmetic code surface in the BPF binary. Merging requires either groth16-solana adopting ark 0.4 OR everyone adopting ark 0.5 — the latter requires solana-program 1.18.x to ship ark 0.5, which it does not. Structural pin. **MEDIUM**.
- **DEP-M05** — three borsh major versions (0.9.3, 0.10.4, 1.5.7) coexist. Anchor 0.30.1 sticks to 0.10.4. solana-sdk 1.18.26 has migrated to 1.5.7 for some struct paths. Today protocol borsh use goes through Anchor (`anchor_lang::prelude::*` -> 0.10.4); future careless `use borsh::...` could silently bind to a different version. **MEDIUM**.

---

## 8. Yanked / CVE / advisory check

This section is a knowledge-of-record review (early 2026). Where I cite a specific advisory ID, I am citing the canonical RustSec / GHSA database identifier as published prior to the audit date. No fresh network lookup was performed (sandbox limitation; WebSearch was denied). The recommendation in Section 12 is to run `cargo audit --db rustsec/advisory-db` and `npm audit` in CI to corroborate.

### 8.1 Rust dep graph

| Dep                  | Version present       | Advisory / status                                                                                                          | Severity |
|----------------------|------------------------|----------------------------------------------------------------------------------------------------------------------------|----------|
| `ed25519-dalek`      | 1.0.1 (`Cargo.lock:1142`) | RUSTSEC-2022-0093 (public-key recovery from short EdDSA signatures) — affects 1.x. solana-program/solana-sdk 1.18.x pulls 1.0.1. Fixed in 2.x. | **HIGH** in principle, MEDIUM in practice — solana-sdk's usage is internal/audited; SolID's own code does not call ed25519-dalek directly (BabyJubJub via ark-ed-on-bn254 is a different curve). Pin locked to solana-program 1.18.x. **DEP-H03** |
| `curve25519-dalek`   | 3.2.1 (`Cargo.lock:1047`) | RUSTSEC-2024-0344 (timing leak in `Scalar::from_canonical_bytes`) — affected `< 4.1.3`. Pulled by solana-program / solana-zk-token-sdk. | **MEDIUM**. SolID does not use curve25519 directly. **DEP-H04** |
| `tiny-bip39`         | 0.8.2                 | crate is unmaintained as of late 2024 (knowledge-of-record). Host-only via solana-sdk. | **LOW** (host-only). **DEP-INFO03** |
| `atty`               | 0.2.14                | RUSTSEC-2021-0145 — unmaintained advisory; minor unsafe correctness bug on Windows pipes. Pulled by `env_logger 0.9.3 -> solana-logger`. | **LOW** (host-only). **DEP-INFO04** |
| `derivative`         | 2.2.0                 | RUSTSEC-2024-0388 — unmaintained advisory only. Pulled by ark-* 0.4 macros. | **INFO**. **DEP-INFO05** |
| `paste`              | 1.0.15                | RUSTSEC-2024-0436 — unmaintained advisory. Used by ark-ff macros. | **INFO** |
| `borsh 0.9.3`        | (`Cargo.lock:710`)    | borsh 0.9.x line had a parsing-bypass class issue resolved in 0.10. solana-program reaches 0.9.3 only via legacy paths. | **LOW**. **DEP-INFO06** |
| `wasm-bindgen 0.2.100` | (`Cargo.lock:2994`) | RUSTSEC-2024-0375 affected `< 0.2.99` (CSP / timing nit in JS binding generator). 0.2.100 is the patched version. | **OK** |
| `getrandom 0.1.16`   | (`Cargo.lock:1261`)   | RUSTSEC-2024-0384 — unmaintained advisory. Pulled by tiny-bip39. | **LOW** (host-only) |
| `light-poseidon 0.2.0` | (`Cargo.lock:1577`) | Light Protocol's external review covered this version; no public RustSec advisory. The BPF stack-overflow is a project-side integration concern, NOT a soundness defect. | **DEP-H01** (LOW for soundness, **HIGH for ops** — pinned because moving to 0.3 would change the round-constants table layout, requiring fresh proof of byte-identity with `solana_program::poseidon::hashv`) |
| `groth16-solana 0.2.0` | (`Cargo.lock:1308`) | Maintained by Light Protocol, audited as part of zk-compression deploy; no public advisory. | **DEP-H02** (MEDIUM): the only consumer pulling ark-0.5 into the on-chain workspace (DEP-M01). Maintainer cadence is low (no 0.3 release at audit time) |

No yanked crate versions detected in the lockfile.

### 8.2 npm dep graph (ts-sdk lockfile)

| Dep | Version | Advisory / status |
|-----|---------|-------------------|
| `node-fetch` | 2.7.0 | maintenance mode; v3+ ESM-only. CVE class historical (GHSA-r683-j2x4-v87g resolved in 2.6.7). 2.7.0 is patched. INFO |
| `ws` | 7.5.10 | GHSA-3h5v-q93c-6h6q resolved in 7.5.10 + 8.17.1. We are on patched 7.x. **OK** |
| `bn.js` | 5.2.3 | OK |
| `borsh-js` | 0.7.0 | maintenance line; no active advisory. INFO |
| `bs58` | 4.0.1, 5.0.0 | both fine |
| `crypto-hash` | 1.3.0 | small, no advisory |
| `cross-fetch` | OK |  |
| `tslib` | 2.8.1 | OK |
| `uuid` | 8.3.2 | deprecated by maintainer in favor of v9; not exploitable |
| `superstruct` | 0.15.5 + 2.0.2 | parallel versions; OK |
| `jayson` | 4.3.0 | OK |
| `agentkeepalive` | 4.6.0 | OK |
| `text-encoding-utf-8` | 1.0.2 | reached via borsh-js 0.7.0; small wrapper |

### 8.3 npm dep graph (circuits lockfile)

| Dep | Version | Status |
|-----|---------|--------|
| `circomlib` | 2.0.5 | OK |
| `snarkjs` | 0.7.6 | OK; patched against historical R1CS-parsing issues |
| `mocha` | 10.8.2 | OK |
| `chai` | 4.x specific | OK; chai 5 ESM-only not adopted (DEP-INFO07) |
| `circom_runtime` | 0.1.28 | OK |
| `circom_tester` | 0.0.20 | pre-1.0 (DEP-INFO02); no advisory but low maintenance |

### 8.4 Summary

No CRITICAL findings. Two HIGH-class items are upstream advisories (`ed25519-dalek 1.0.1`, `curve25519-dalek 3.2.1`) pinned by solana-program 1.18.x; resolution requires anchor 0.30 -> 0.31 + solana 1.18 -> 2.x cutover (`docs/E2E_BLOCKERS.md` B7, out of scope for v0.6.1 close-out). Remaining items are LOW / INFO / hygiene.

---

## 9. Upgrade-blocked pins

| Pin | Where | Blocks-the-upgrade-of | Why |
|-----|-------|-----------------------|-----|
| `wasm-bindgen = "=0.2.100"` | `wasm/Cargo.toml:16` | wasm-bindgen >= 0.2.118 | newer wasm-bindgen-cli MSRV is rustc 1.86; pinned 1.79.0 to satisfy anchor 0.30.1 + solana 1.18.22. Doc at `wasm/Cargo.toml:12-15`. |
| `serde-wasm-bindgen = "=0.6.5"` | `wasm/Cargo.toml:17` | serde-wasm-bindgen >= 0.6.6 | ABI couples to wasm-bindgen 0.2.100 |
| `js-sys = "=0.3.77"` | `wasm/Cargo.toml:20` | js-sys >= 0.3.78 | same wasm-bindgen ABI couple |
| `console_error_panic_hook = "=0.1.7"` | `wasm/Cargo.toml:21` | newer 0.1.x | same |
| `wasm-bindgen-test = "=0.3.50"` | `wasm/Cargo.toml:25` | newer 0.3.x | same |
| `light-poseidon = "0.2"` | `crates/solid-core/Cargo.toml:112` | light-poseidon 0.3.x | bumping to 0.3 changes round-constants table layout. Byte-identity with `sol_poseidon` syscall is documented to hold for 0.2.0 exactly (`crates/solid-core/Cargo.toml:64-68`). Bump requires fresh byte-identity proof + cross-language vector regen + audit signoff. **DEP-H01** |
| `solana-program = "~1.18"` | `Cargo.toml:50` | solana-program 2.x | pulls anchor 0.30 -> 0.31 + Solana CLI bump and the docs/E2E_BLOCKERS B7 cascade. Includes ed25519-dalek 1 -> 2 transition |
| `anchor-lang = "0.30.1"` | `Cargo.toml:43` | anchor 0.31.x | proc-macro2 1.0.95+ removed `Span::source_file()` API used by anchor 0.30 idl-build; anchor 0.31 fixes (`docs/E2E_BLOCKERS.md` B7). Until then `anchor build --no-idl` is the workaround |
| `proc-macro2 1.0.94` | `Cargo.lock:1840` (resolved, not directly pinned) | proc-macro2 1.0.95+ | same anchor 0.30.1 idl-build breakage. Currently NOT upgrade-blocked at the manifest level (no explicit pin); `cargo update -p proc-macro2` would silently break `anchor build` (with idl). Should be pinned explicitly. **DEP-M02** |
| `ark-bn254 = 0.4` (workspace) | `Cargo.toml:35-39` | ark-* 0.5 | groth16-solana 0.2 already uses 0.5; everything else uses 0.4 because solana-program 1.18 uses 0.4. Unifying requires solana-program 2.x |
| `ark-circom = "0.5.0-alpha"` | `tools/solid-prover/Cargo.toml:33` | num-bigint != 0.4.3 | hard-pins num-bigint 0.4.3, structural reason for the separate workspace |
| `borsh = "0.10"` (solid-light) | `crates/solid-light/Cargo.toml:13` | borsh 1.x | solid-light's borsh re-exports must match anchor-lang 0.30.1's borsh 0.10.x to avoid trait-bound mismatches. Couples to anchor 0.30.1 |
| `chai = "^4.3.7"` | `circuits/package.json:16` | chai 5.x | chai 5 is ESM-only; circuits/test scripts use mocha CommonJS path. **DEP-INFO07** |

---

## 10. Unused / dead deps

| Dep | Where | Status |
|-----|-------|--------|
| `hex = "0.4"` | `crates/solid-light/Cargo.toml:14` | **DEAD** (no `hex::` reference in `crates/solid-light/src/`). **DEP-L05** |
| `ark-groth16 = "0.4"` (workspace) | `Cargo.toml:45` | **DEAD as a workspace dep** (no member of the main workspace consumes it; only `tools/solid-prover/` does, and it pins directly). **DEP-L03** |
| `serde_json = "1.0"` (wasm) | `wasm/Cargo.toml:19` | likely **dead direct dep** (no `serde_json::` in `wasm/src/lib.rs`). **DEP-L06** |
| `num-traits = "0.2"` (solid-core) | `crates/solid-core/Cargo.toml:45` | likely **dead direct dep** (no `num_traits::` in `crates/solid-core/src/`; consumed transitively). **DEP-L04** |
| `ts-node = "^10.9.2"` | `.toolchain/npm/package.json:4` | possibly dead — root scripts use `tsx`, no `ts-node` invocation in any committed script. **DEP-L09** |

Intentional-keep deps verified live:
- `circuits/package.json` `dependencies.circomlib` is correctly the ONLY production dep.
- `aes-gcm`, `argon2`, `blake2`, `rand`, `serde_json` in `solid-core` host-only block ARE used by `babyjubjub.rs` / `credential.rs`.
- `anchor-spl` in `programs/issuer-registry/Cargo.toml:41` IS used (`use anchor_spl::token::{...}` at `programs/issuer-registry/src/lib.rs:8`).
- `borsh` in `solid-light` IS used (`use borsh::{BorshDeserialize, BorshSerialize}` at `crates/solid-light/src/credential_tree.rs:8`).
- `anyhow` dev-dep in solid-core IS used in `examples/gen_vectors.rs:47`.

---

## 11. Findings (severity-sorted, IDs DEP-<sev><nn>)

### Critical
None.

### High

**DEP-H01** — `light-poseidon = "0.2"` is upgrade-blocked and the byte-identity invariant is load-bearing. Pin to exact `=0.2.0`.

**DEP-H02** — `groth16-solana = "0.2"` pins ark-* 0.5 alongside ark-* 0.4 (DEP-M01).

**DEP-H03** — `ed25519-dalek 1.0.1` in lockfile (RUSTSEC-2022-0093). Add `cargo deny` rule disallowing direct imports.

**DEP-H04** — `curve25519-dalek 3.2.1` in lockfile (RUSTSEC-2024-0344). Resolution requires solana-program 2.x.

### Medium

**DEP-M01** — Six ark-* crates duplicated 0.4 / 0.5 in the BPF binary.

**DEP-M02** — `proc-macro2 1.0.94` is implicitly pinned. Add `proc-macro2 = "=1.0.94"` to workspace.

**DEP-M03** — `ark-circom = "0.5.0-alpha"` is alpha and the prover's central dep.

**DEP-M04** — `tools/solid-prover/Cargo.lock` is not committed.

**DEP-M05** — Three borsh major versions coexist (0.9.3, 0.10.4, 1.5.7). Add cargo-deny rule disallowing direct borsh imports outside solid-light.

### Low

**DEP-L01** — snarkjs version drift across four manifests. Unify to `^0.7.5`.

**DEP-L02** — No `engines` field in any `package.json`. Set Node `>=18.0.0`.

**DEP-L03** — `ark-groth16` is a dead workspace dep. Delete.

**DEP-L04** — `num-traits` direct dep in solid-core may be dead. Verify with `cargo udeps`.

**DEP-L05** — `hex` direct dep in solid-light is dead. Delete.

**DEP-L06** — `serde_json` direct dep in wasm crate may be dead. Verify and remove.

**DEP-L07** — No root `package-lock.json`. Commit one.

**DEP-L08** — TS package version split (core 0.1.0 vs holder/issuer/light/sdk 0.2.0).

**DEP-L09** — `ts-node` in `.toolchain/npm/package.json` may be dead.

### Info

**DEP-INFO01** — `ws 7.5.10` is the patched 7.x line.
**DEP-INFO02** — `circom_tester 0.0.20` is pre-1.0.
**DEP-INFO03** — `tiny-bip39 0.8.2` is unmaintained but host-only.
**DEP-INFO04** — `atty 0.2.14` carries an unmaintained advisory; host-only test logging.
**DEP-INFO05** — `derivative 2.2.0` unmaintained-advisory only.
**DEP-INFO06** — `borsh 0.9.3` historical line; reached only by solana-program legacy paths.
**DEP-INFO07** — `chai 4.x` (vs ESM-only chai 5) is intentional under mocha CommonJS.

---

## 12. Suggested actions (ordered)

### Now (low cost, high signal)

1. **Delete dead direct deps.** Drop `hex` from `crates/solid-light/Cargo.toml:14`; drop `ark-groth16` from workspace `Cargo.toml:45`. Verify and remove `num-traits` from solid-core, `serde_json` from wasm. Check `ts-node` in `.toolchain/npm`.
2. **Tighten upgrade-blocked pins to exact.** Change `light-poseidon = "0.2"` to `=0.2.0`. Add `proc-macro2 = "=1.0.94"` to workspace.
3. **Add `tools/solid-prover/Cargo.lock` to the repo.**
4. **Unify snarkjs to a single pin.** Set `snarkjs = "^0.7.5"` (or `=0.7.6`) in all four manifests.
5. **Add `engines` to root `package.json` and `ts-sdk/package.json`.**
6. **Commit a root `package-lock.json`.**

### Soon (CI / hygiene)

7. **Wire `cargo audit` into CI** for both workspaces.
8. **Wire `npm audit --omit=dev` into CI** for `ts-sdk/` and `circuits/`.
9. **Add a `cargo tree -d` regression check.**
10. **Add a `cargo deny` rule** disallowing direct `borsh` imports outside solid-light and direct `ed25519-dalek = "1"` imports anywhere outside solana-sdk.
11. **Mainnet-build deny-list for `sec007-skip-onchain` Cargo feature** (already documented; CI gate is the missing piece).

### Later (coordinated upgrades)

12. **Track `groth16-solana 0.3+`.**
13. **Plan the anchor 0.30.1 -> 0.31 + solana 1.18.22 -> 2.x cutover.**
14. **Replace `ark-circom 0.5.0-alpha` in the prover** with stable Groth16 builder.
15. **Plan `light-poseidon 0.2 -> 0.3`** with fresh byte-identity proof.

---

## Notes

- WebSearch was denied during the agent run, so all CVE / advisory citations are knowledge-of-record (RustSec / GHSA IDs published prior to early 2026). Section 12 step 7-8 recommends running `cargo audit` and `npm audit` in CI to corroborate against the live advisory database.
- Two distinct workspace lockfiles needed attention: the root `Cargo.lock` is committed (3211 lines, scanned in full); the prover `tools/solid-prover/Cargo.lock` is **not committed**, which is itself a finding (DEP-M04).
- Single most actionable item: the five "Now" items (Section 12.1-6) are all manifest-only edits that close DEP-L03/L04/L05/L06/L07/L09 and DEP-H01/M02/M04 with no semantic change to the build. Recommend landing them as one PR before the external audit.
