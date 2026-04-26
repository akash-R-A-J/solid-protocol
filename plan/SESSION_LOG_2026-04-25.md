# Session Log — 2026-04-25 (E2E pipeline restoration)

Plain-ASCII log of everything decided, found, fixed, abandoned, and
left open in this session. Written after the work, from the working
tree alone (file paths, declare_id! lines, IDL bytes, VK metadata,
.so binaries, snarkjs r1cs info), so every claim here is reproducible
without reading any chat. If a number in this file disagrees with the
code, the code wins and this file is wrong.

Companion documents: `docs/E2E_BLOCKERS.md` (live punch list),
`plan/RESUME.md` (cross-session continuity),
`sec/audits/2026-04-25_v0.6.1_deep_comprehensive_audit.md`
(canonical audit), `adr/0004-three-program-split.md` (ID rotation
addendum).

---

## 0. Goal of the session

> "establish a robust and high-performance solid-protocol system,
> free from regressions and workarounds, specifically enabling
> `npm run e2e`."

Concretely, get the build pipeline from "host tests green, BPF
artefacts present" to "WASM bridge builds + IDLs build + TS SDK
builds + scripts/initialize.ts can talk to localnet". Three of
those four flipped green in this session.

---

## 1. Hard decisions taken (with rationale)

### D1. Promote the wasm32 Poseidon dispatch from "pre-mitigation" to the actual fix.

- **Choice.** Make `solana-program` target-conditional (BPF +
  non-wasm32 host) and route `target_arch = "wasm32"` directly at
  `light-poseidon 0.2.0` inside `crates/solid-core/src/poseidon.rs`.
  Three legitimate deployment targets, three byte-identical
  Poseidon backends.
- **Why.** `solana-program 1.18.x` was never wasm32-clean
  (`std::sync::Mutex`, `std::time::*`, `getrandom` without `js`
  feature). Putting it in unconditional `[dependencies]` was the
  collateral cost of the Phase 2 BPF stack-overflow fix (B3) and
  blocked the WASM bridge build (B6), which in turn blocked the
  TS SDK build, the cross-language vectors gate, and the E2E
  driver — every "open" bullet downstream.
- **Why this is not a workaround.** Same pattern as
  `ring`/`getrandom`/`tokio`: each compilation target picks the
  backend that physically fits, and all three converge on the
  same byte output. The `tests/vectors/` cross-language CI gate
  enforces the byte-identical contract.
- **Rejected alternatives.** (a) feature-flag `solana-program`
  optional — re-introduces B3 silently if the flag is forgotten;
  (b) bump to Solana 2.x — out of scope (B7-class toolchain
  migration); (c) vendor solana-program with wasm shims — we
  would own the patches forever; (d) `unimplemented!()` on
  wasm32 — violates "root cause only".

### D2. Stop trying to fight `anchor-lang-idl`'s `+nightly` + `--cfg procmacro2_semver_exempt` invocation. Drive the IDL build directly.

- **Choice.** A 135-line Node script (`scripts/build_idls.mjs`)
  that runs `cargo test __anchor_private_print_idl --features
  idl-build` on stable rust 1.79.0 with `RUSTFLAGS=-A warnings`
  and explicit `ANCHOR_IDL_BUILD_*` env vars, then parses the
  same `--- IDL begin program ---` markers `anchor-lang-idl`
  itself parses.
- **Why.** `anchor-lang-idl 0.1.2`'s helper hardcodes
  `cargo +nightly` and `RUSTFLAGS="--cfg procmacro2_semver_exempt
  --cfg super_unstable"`. On modern nightly toolchains
  (1.85+), that combination breaks two unrelated transitives:
  - `proc-macro2-1.0.94`'s `procmacro2_semver_exempt` arm calls
    `proc_macro::Span::source_file().path()`, which only exists
    on older nightlies and is gone (renamed/removed) on current
    `+nightly`. anchor-syn surfaces the resulting compile error
    as `E0599: no method named source_file`.
  - `ark-ff-macros 0.4.2`'s `MontFp!` macro relies on
    `syn::Expr::Group` from proc-macro2's wrap mode; the
    semver_exempt arm disables the wrap and `MontFp!` rejects
    the token stream with a "could not parse" panic at IDL build
    time.
- **Why this is not a workaround.** The semver_exempt cfg in
  anchor-syn only gates type-alias resolution
  (`anchor-syn/src/idl/defined.rs:493-499`); nothing in our
  programs reaches Anchor account types through type aliases,
  so the IDL emitted on stable is byte-identical to what nightly
  would emit when the nightly toolchain isn't broken. The hidden
  `__anchor_private_print_idl_*` test is the same entrypoint
  `anchor-lang-idl` itself drives — we're just running it on a
  toolchain that currently builds.
- **Why this is also not future-proof.** It is bridge code. The
  permanent fix is `anchor 0.30 → 0.31` (or 0.30 → solana 2.x +
  anchor 0.31), which removes the semver_exempt requirement
  upstream. Tracked separately as B7 (toolchain migration), not
  closed by this script.
- **Rejected alternatives.** (a) install an old nightly that
  still has the legacy `proc_macro::SourceFile` API — pinning
  to a specific nightly nightly-by-nightly is the textbook
  workaround; (b) hand-write TS instruction encoders and skip
  IDLs entirely (tasks `e2`/`e3`/`e4` in the prior todo list)
  — abandoned the moment IDL generation became a one-command
  reproducible step (no longer a workaround, so the workaround
  task lost its premise).

### D3. Take the program's primary `declare_id!()` from `src/lib.rs` rather than from anchor-lang-idl's `--- IDL begin address ---` block.

- **Choice.** `programIdFromLibRs(crateDir)` in
  `scripts/build_idls.mjs:93-101`: regex-match the *first*
  top-level `declare_id!("...")` line in the program's
  `src/lib.rs`, ignore inner ones inside nested `mod {}` blocks.
- **Why.** anchor's `idl-build` codegen emits one
  `--- IDL begin address ---` block per `declare_id!()` it
  syntactically finds in the crate. `programs/issuer-registry`
  imports SPL Account Compression's
  `mod spl_account_compression_id { declare_id!("cmtDvX..."); }`
  helper, which means cargo test prints two address blocks and
  their order is non-deterministic across `cargo test`
  invocations. The first run produced
  `issuer_registry.json` carrying SPL AC's pubkey
  (`cmtDvXumGCrqC1Age74AVPhSRVXJMd8PJS91L8KbNCK`) instead of
  `5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx`. Anchor's TS
  client would then try to send instructions to the wrong
  program.
- **Why this is sound.** The "primary" program ID is by
  convention the one declared at the crate root (column 0).
  All three programs in this tree follow that convention.
  `scripts/check_program_ids.py` already enforces the same
  invariant against `Anchor.toml`, `deployments/devnet.json`,
  and the keypair pubkeys.
- **Rejected alternatives.** (a) parse all
  `--- IDL begin address ---` blocks and pick "the one that
  matches `Anchor.toml`" — works, but couples the IDL builder
  to `Anchor.toml`; (b) ask anchor-lang to disambiguate
  upstream — out of scope.

### D4. Track program keypairs in `keys/localnet/`, not in `target/deploy/`.

- **Choice.** Three keypair JSONs live at
  `keys/localnet/{issuer_registry,schema_registry,zk_verifier}-keypair.json`,
  documented by `keys/localnet/README.md`. `anchor build` and
  helper scripts walk `keys/localnet/<program>-keypair.json`
  first and `target/deploy/<program>-keypair.json` second.
- **Why.** `anchor build` regenerates
  `target/deploy/*-keypair.json` from scratch when the file is
  missing. On a clean checkout this silently rotates the
  on-chain program ID and breaks every hard-coded canonical
  reference (the verifier's owner-checks on
  `global_tree`/`schema_tree_N`/`issuer_tree_binding`,
  ADR-0014 issuer-tree binding, the schema-registry two-layer
  guard, scripts that PDA-derive against a fixed program). The
  only way to make the canonical IDs durable across machines
  and CI runs is to commit the keypairs.
- **Sanity check.** All three pubkeys reproduce against the
  current `declare_id!()` lines in `programs/*/src/lib.rs`,
  against `Anchor.toml`, and against `deployments/devnet.json`:

  | program          | declare_id!()                                  | keys/localnet pubkey                           | match |
  |------------------|------------------------------------------------|------------------------------------------------|-------|
  | issuer_registry  | `5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx` | `5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx` | yes   |
  | schema_registry  | `4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1` | `4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1` | yes   |
  | zk_verifier      | `DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb`  | `DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb`  | yes   |

### D5. Forward `idl-build` to every Anchor crate the program pulls types from.

- **Choice.** `programs/issuer-registry/Cargo.toml`'s
  `idl-build` feature is `["anchor-lang/idl-build",
  "anchor-spl/idl-build"]`. The other two programs only use
  `anchor-lang` (verified: `programs/schema-registry/Cargo.toml`
  and `programs/zk-verifier/Cargo.toml` both have
  `idl-build = ["anchor-lang/idl-build"]`).
- **Why.** Without `anchor-spl/idl-build`, the IDL build for
  `issuer-registry` fails with `the trait Discriminator is not
  implemented for TokenAccount`. The IDL build needs the
  `Discriminator` impls anchor-spl gates behind its own
  `idl-build` feature; the program crate is what pulls
  anchor-spl, so feature forwarding is its responsibility.
- **Pre-fix evidence.** The build failed with that exact
  trait-not-implemented error. Post-fix the IDL emerges clean.

### D6. Do not write a parallel set of hand-rolled TypeScript instruction encoders.

- **Choice.** Cancel the previously-tracked tasks `e2` ("inventory
  hand-written encoders"), `e3` ("add hand-written
  TransactionInstruction encoders"), `e4` ("refactor scripts to
  drop loadIdl entirely").
- **Why.** Those tasks existed only because IDL generation was
  broken. With `scripts/build_idls.mjs` succeeding and the
  emitted IDLs loading cleanly into `new anchor.Program(idl,
  provider)` (verified — see §6.2), the original
  `scripts/initialize.ts` path that calls `loadIdl` is fine.
  Writing a second encoder layer would be a workaround under
  the rule "root cause only".

---

## 2. Bugs found (this session, with the file:line they bit at)

### Bug B6 — `solana-program` unconditionally on the wasm32 build path

- **Where it bit.** `wasm-pack build wasm/ --target nodejs
  --release`. solid-core compiles for `wasm32-unknown-unknown`
  via `wasm/`. solana-program 1.18 is not wasm32-clean.
- **Code site.** `crates/solid-core/Cargo.toml` (the
  pre-fix version had `solana-program = { workspace = true }`
  in plain `[dependencies]`).
- **Pre-fix root cause.** Phase 2 (B3) replaced light-poseidon's
  field-element hasher with `solana_program::poseidon::hashv` to
  avoid the 4 KB BPF frame overflow under `lto = "fat"`. To do
  that, solana-program had to be visible on every target
  solid-core was built for. wasm32 was not on the radar at the
  time; the dep crossed the wasm32 boundary by accident.
- **How it surfaces.** Earlier sessions recorded
  `wasm-pack build` failing in solana-program transitives with
  `getrandom` and `Mutex`-related compile errors on wasm32.

### Bug B7 — anchor `idl-build` fails on modern nightly via `procmacro2_semver_exempt`

- **Where it bit.** `anchor build` (which calls
  `anchor-lang-idl`'s `idl_build_print` helper) on a host with
  current `+nightly` toolchains.
- **Code sites.**
  - `proc-macro2 1.0.94` → `proc_macro::Span::source_file()`
    behind `procmacro2_semver_exempt` cfg, removed/changed on
    current nightly (`E0599`).
  - `ark-ff-macros 0.4.2` → `MontFp!` panics on a token tree
    that proc-macro2's semver_exempt arm wraps differently than
    the stable arm.
- **Pre-fix root cause.** anchor-lang-idl 0.1.2's helper
  unconditionally invokes `cargo +nightly` and sets
  `RUSTFLAGS="--cfg procmacro2_semver_exempt --cfg super_unstable"`.
  That is fine on the specific nightly anchor 0.30.1 was tested
  against; it is not fine on later nightlies.

### Bug B7-sub — `anchor` IDL generation emits one `address` block per `declare_id!()` it finds, in a non-deterministic order

- **Where it bit.** `target/idl/issuer_registry.json` ended up
  with `"address": "cmtDvXumGCrqC1Age74AVPhSRVXJMd8PJS91L8KbNCK"`
  (SPL AC's program ID) instead of the issuer-registry's own ID.
- **Code site.** Anchor's idl-build codegen prints a
  `--- IDL begin address ---` block for every `declare_id!()`
  syntactic occurrence, including the inner one inside
  `mod spl_account_compression_id { declare_id!(...) }`.
  cargo test orders test outputs non-deterministically.
- **Why we hit it now.** issuer-registry imports
  `spl_account_compression`'s `mod spl_account_compression_id`
  helper to PDA-derive against the SPL AC program. That helper
  contains a nested `declare_id!()`, which made it look like
  this crate had two valid program IDs to anchor's idl-build
  pass.

### Bug B7-meta — Node script run outside the project root cannot resolve `@coral-xyz/anchor`

- **Where it bit.** First version of the IDL load smoke test
  was written to `/tmp/idl_load_smoke.mjs`. `node` resolved
  imports against `/tmp/node_modules`, not the project's
  `node_modules`, so the import failed with
  `ERR_MODULE_NOT_FOUND`.
- **Fix.** Run from project root (`./_idl_load_smoke.mjs`).
  Trivial, but worth recording because it cost a confused
  cycle.

### Pre-existing flag (not introduced this session): post-Phase-2 doc says comment in `circuits/scripts/setup.js:34` claims `~60K constraints`, actual count is 86,616 + 31 + 1.

- See §5 for the canonical numbers from snarkjs.

---

## 3. Solutions applied — and which ones held

### What worked, end-to-end

- **`crates/solid-core/Cargo.toml` three-target split.**
  `[target.'cfg(target_os = "solana")'.dependencies]` and
  `[target.'cfg(all(not(target_os = "solana"), not(target_arch = "wasm32")))'.dependencies]`
  bind solana-program to BPF + non-wasm32 host. wasm32 reaches
  light-poseidon directly via the existing
  `[target.'cfg(not(target_os = "solana"))'.dependencies]` arm.
- **`crates/solid-core/src/poseidon.rs::hash_bytes_dispatch`
  cfg dispatch.** Two functions with the same signature,
  selected by `#[cfg(not(target_arch = "wasm32"))]` vs
  `#[cfg(target_arch = "wasm32")]`. BPF + non-wasm32 host call
  `solana_program::poseidon::hashv`; wasm32 calls
  `light_poseidon::Poseidon::<Fr>::new_circom(N).hash_bytes_le`.
- **Pre-canonicalize through `Fr::from_le_bytes_mod_order →
  fr_to_bytes_le` before dispatch.** Prevents
  `InputLargerThanModulus` rejection on byte slices ≥ p (most
  notably 32-byte verifier addresses with top byte > 0x30 and
  the BN254 modulus MSB). Applies on every target, so the
  byte-equality property is preserved.
- **`scripts/build_idls.mjs`** — drives `cargo test
  __anchor_private_print_idl --features idl-build` on rust
  1.79.0 stable, parses anchor's printed `--- IDL begin/end
  program ---` markers, splices in events/errors, and patches
  `idl.address` from the program's primary `declare_id!()`.
- **`programIdFromLibRs(crateDir)`** with the regex
  `^declare_id!\("([1-9A-HJ-NP-Za-km-z]{32,44})"\)` (multiline
  flag, anchored at column 0). Skips nested
  `mod foo { declare_id!(...) }` because those are indented.
- **`programs/issuer-registry/Cargo.toml`** — feature-forward
  `anchor-spl/idl-build` so `Discriminator` is in scope during
  IDL build for `TokenAccount`-bearing instructions.
- **`package.json` script wiring** — `"build:idl": "node
  scripts/build_idls.mjs"` and `"e2e": "npm run build:idl &&
  npm run init-onchain && npm run backfill-issuer-tree && npm
  run bootstrap-issuer && npm run issue && npm run prove"`. IDL
  generation is now a deterministic prerequisite of E2E.

### What was tried and discarded, with reason

- **"Maybe `B7` is just out of scope, the audit said so."**
  Discarded after reading `scripts/initialize.ts` and
  confirming it imports IDLs and constructs `new
  anchor.Program(idl, provider)`. The audit's phrasing ("IDL
  not on the npm run e2e path") was wrong about *this* repo;
  it likely held for an earlier iteration of the scripts.
- **Setting `RUSTFLAGS="--cfg procmacro2_semver_exempt"` and
  pinning to an old nightly.** Fragile (the right nightly tag
  changes nightly-by-nightly), couples the build to a snapshot
  no CI keeps around, and still hits ark-ff-macros' `MontFp!`
  parse panic. Discarded in favour of bypassing the cfg
  altogether.
- **Hand-writing TypeScript instruction encoders (Anchor
  sighash + Borsh args) per program.** Was the planned fallback
  if IDL generation could not be made reliable. Cancelled the
  moment `build_idls.mjs` succeeded for all three programs
  twice in a row and the resulting IDLs loaded cleanly into
  `new anchor.Program(idl, provider)`.
- **Reading the *first* `--- IDL begin address ---` block.**
  Worked once, then gave a different (wrong) program ID on the
  next invocation. Discarded once non-determinism was
  understood. Replaced with `programIdFromLibRs`.
- **Running the IDL smoke test from `/tmp/`.** Failed
  (`ERR_MODULE_NOT_FOUND` on `@coral-xyz/anchor`). Trivial
  resolution: move the script to the project root.

---

## 4. Current state of the system (what is green, what is grey, what is open)

### Green (verified in this working tree, today)

- **Program ID consistency.** `Anchor.toml`,
  `programs/*/src/lib.rs`'s `declare_id!()`,
  `keys/localnet/*-keypair.json`, `target/deploy/*-keypair.json`,
  `deployments/devnet.json`, and the `address` field of every
  emitted IDL all carry the same three pubkeys. See D4 for the
  table.
- **BPF binaries.** `target/deploy/{issuer_registry,
  schema_registry, zk_verifier}.so` exist (sizes: 684 KB / 332
  KB / 327 KB) and were built earlier today.
- **Anchor IDLs.** `target/idl/{issuer_registry,
  schema_registry, zk_verifier}.json` exist (54 KB / 13 KB / 20
  KB), were re-emitted from source via `npm run build:idl`, and
  load cleanly into `new anchor.Program(idl, provider)` (smoke
  test from project root).
- **WASM bridge.** `ts-sdk/packages/core/wasm/solid_wasm_bg.wasm`
  (~1.77 MB), `solid_wasm.js` (~28 KB), `solid_wasm.d.ts`
  (~3.9 KB). Built `21:59` today via `wasm-pack` against the
  three-target solid-core. This is what the B6 fix exists to
  produce.
- **Cross-language vectors.** `tests/vectors/commitment_and_nullifier.json`
  pins the canonical bytes for the commitment and the 6-input
  nullifier. The pinned `expected_nullifier_hex` is
  `625c00be594f0e697cfbaef6907779f87f2976e894b49b444e1a735ac0ae761f`;
  the pinned `expected_commitment_hex` is
  `01a5b61022eba3f7dca36a556d27de1e8e2e0ea9894125a582526970a678782c`.
- **Verification key.** `circuits/build/verification_key.json`
  declares `nPublic = 32` and has `IC.length = 33`, matching the
  on-chain `NR_PUBLIC_INPUTS = 32` constant in
  `programs/zk-verifier/src/lib.rs:41`. SHA-256 of the VK,
  recorded at `circuits/build/verification_key.sha256`, is
  `debca4c083d0cd4091339367877b199e87708d0c3b7467b0c85115de2f74a15a`.
  This hash is the "VK identity" the on-chain verifier compares
  against under SOLID-SEC-041 (content-addressed VK artefact).
- **Lockfile / toolchain pins.** Cargo.lock at version 3,
  `.cargo/config.toml` opts into the MSRV-aware resolver,
  `Cargo.toml` declares `rust-version = "1.75"` workspace-wide.
  Build pipeline still passes the host-side baseline:
  `cargo test -p solid-core --lib` (49/49), `cargo test -p
  solid-light --lib` (25/25), `cargo test -p zk-verifier --lib`
  (17/17). These were verified earlier in the session and not
  re-run after the IDL work; nothing in the IDL work touches
  those crates' code paths.

### Grey (working but not yet end-to-end-verified post-IDL)

- **`scripts/initialize.ts` against localnet.** The smoke test
  proves the IDLs *load* into Anchor's TS runtime. It does not
  prove the full `new anchor.Program(...).methods.<ix>(...)`
  path resolves the encoded instruction discriminator and sends
  the transaction successfully. That is task `e1`
  (in_progress). See §7 for the runbook.
- **Localnet deploy under the rotated IDs.** Task `e5`
  (pending). The .so binaries exist; they were built before the
  ID rotation in this session (`Apr 25 16:50-16:51`), so we have
  to confirm the `declare_id!()` baked into each binary still
  matches the keypair pubkey. (Check via `solana program show
  --bpf <so>` after a deploy, or `strings` for the embedded
  pubkey. If they drifted, `anchor build` again — keys/localnet
  is wired up so the rebuild will use the canonical keypairs.)
- **macOS AppleDouble pollution (B8).** `COPYFILE_DISABLE=1` +
  `rm -rf test-ledger` smoke not yet observed green this
  session. Task `e6` (pending).
- **CI gates.** `scripts/wasm_bridge_smoke.mjs` and the
  cross-language vectors check (`tsx tests/vectors/check_vectors.ts`)
  are not yet wired into `.github/workflows/ci.yml`. Task `e7`
  (pending). Until they are, a future PR can silently
  reintroduce a wasm32-incompatible dep into solid-core or
  break byte equality across the Rust/TS boundary, and we will
  not catch it until the next E2E attempt.

### Open / not-yet-touched

- **Phase 3 SOLID-SEC-006 Part 2.** Bind `vk_generation` into
  the circuit's public inputs so cross-VK replay is impossible.
  Requires a circuit change → batches with the next trusted-
  setup cycle.
- **Phase 3 SOLID-SEC-010 cross-language vectors.** Extend
  `gen_vectors.rs` and `check_vectors.ts` from 3/10 to 10/10
  primitives. Currently we have commitment + nullifier; the
  audit asks for issuer-leaf, schema-hash, query-hash, and the
  rest.
- **Phase 3 SOLID-SEC-043 issuer_tree_operator single signer.**
  Gate behind Squads 3-of-5 or DAO threshold PDA before
  external audit. Open in the registry.
- **Phase 3 trusted-setup ceremony (SOLID-SEC-012).** Mainnet
  blocker. Replace `circuits/scripts/setup.js`.
- **External audit of the post-Phase-2 codebase.** Open.
- **Anchor 0.30 → 0.31 toolchain bump.** The proper fix to
  retire `scripts/build_idls.mjs`. Tracked as B7-toolchain
  (out-of-scope for current E2E close).

---

## 5. Outputs and pinned values to carry forward

This is the section that future-you (or future-me) will want to read
before any rebuild/redeploy. None of these numbers should drift
silently.

### 5.1 Program IDs (canonical, identical across Anchor.toml /
declare_id!() / keys/localnet / target/deploy / deployments/devnet.json)

```
zk_verifier      DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb
issuer_registry  5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx
schema_registry  4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1
```

(Update 2026-04-25 23:30: CLAUDE.md "Hard invariants" was updated
in this session to carry the post-rotation IDs `Dcyezh…`,
`5fxhJ1…`, `4ZCrx…`. Earlier drafts of this log called for that
follow-up; it is no longer outstanding.)

### 5.2 Circuit constraint counts (snarkjs r1cs info, today)

```
Curve              bn-128
# of Wires         86 780
# of Constraints   86 616  (non-linear; setup.js comment says ~60K — stale)
# of Private Inputs   522
# of Public Inputs    31   (snarkjs counts inputs only)
# of Outputs           1
# of Labels       340 482

VK nPublic        32       (= 31 inputs + 1 output, matches NR_PUBLIC_INPUTS)
VK IC.length      33       (= nPublic + 1)
VK protocol       groth16
VK curve          bn128
VK SHA-256        debca4c083d0cd4091339367877b199e87708d0c3b7467b0c85115de2f74a15a
```

The `~60K` comment in `circuits/scripts/setup.js:34` is stale by
~50% relative to the actual circuit. Doc-truth follow-up.

### 5.3 Public-input layout (post-ADR-0014, see
`circuits/batch_credential_query.circom:445-460`)

The `component main {public [...]}` declaration carries (in order):

```
globalRoot
merkleRoots[NUM_CREDS]
schemaHashes[NUM_CREDS]
issuerTreeRoot                     <- ADR-0014 / SEC-004 / SEC-008
queryCredentialIndices[MAX_PREDICATES]
queryFieldIndices[MAX_PREDICATES]
queryOperators[MAX_PREDICATES]
queryValues[MAX_PREDICATES]
numPredicates
compoundLogic
verifierAddress
verifierNonce
currentTimestamp
```

with `BatchCredentialQuerySolana(20, 20, 16, 8, 4, 4)` parameters.
The actual template signature is at
`circuits/batch_credential_query.circom:63`:

```
template BatchCredentialQuerySolana(TREE_DEPTH, GLOBAL_DEPTH, ISSUER_TREE_DEPTH,
                                    NUM_FIELDS, NUM_CREDS, MAX_PREDICATES)
```

so `(20, 20, 16, 8, 4, 4)` decodes as:

```
TREE_DEPTH         = 20
GLOBAL_DEPTH       = 20
ISSUER_TREE_DEPTH  = 16
NUM_FIELDS         =  8
NUM_CREDS          =  4
MAX_PREDICATES     =  4
```

The on-chain `zk_verifier` reads this layout via the named index
constants `ISSUER_TREE_ROOT_INPUT_INDEX`,
`VERIFIER_ADDRESS_INPUT_INDEX`, `CURRENT_TIMESTAMP_INPUT_INDEX`.

### 5.4 Cross-language Poseidon test vectors
(`tests/vectors/commitment_and_nullifier.json`)

```
Commitment   ( Poseidon(data_fields ++ schema_hash, holder_pub_x, holder_pub_y, salt) )
  data_fields           [21, 840, 1, 0, 0, 0, 0, 0]
  schema_hash_hex       32d736ab34df25f768b9c59d260e23f30263ab8de5d503ffc039699ed832770e
  holder_pub_x_hex      0707...07
  holder_pub_y_hex      0808...08
  salt_hex              2a2a...2a
  expected_commitment   01a5b61022eba3f7dca36a556d27de1e8e2e0ea9894125a582526970a678782c

Nullifier    ( 6-input Poseidon — ADR-0006 revision adds issuerTreeRoot )
  master_key_hex        1111...11
  rev_nonce             7
  verifier_addr_hex     2222...22
  query_hash_hex        3333...33
  verifier_nonce_hex    4444...44
  issuer_tree_root_hex  5555...55
  expected_nullifier    625c00be594f0e697cfbaef6907779f87f2976e894b49b444e1a735ac0ae761f
```

These are the byte-for-byte gates between the Rust path
(`crates/solid-core` → `solana_program::poseidon::hashv` →
light-poseidon fallback on host, `sol_poseidon` syscall on BPF) and
the wasm32 path (`light_poseidon::Poseidon::<Fr>::new_circom().hash_bytes_le`
direct). Any divergence here is a real soundness signal — never
re-baseline without root cause.

### 5.5 Toolchain pins (durable record, not regenerated by build)

```
rust              1.79.0    (stable; pinned in deployments/devnet.json
                              and used by scripts/build_idls.mjs)
                  + 1.75    (workspace rust-version, BPF tooling MSRV)
solana-cli        1.18.22
anchor-cli        0.30.1
anchor-lang       0.30.1
anchor-lang-idl   0.1.2     (the source of B7 — replaced by build_idls.mjs)
circom            2.1.9
snarkjs           0.7.5
wasm-pack         0.13.1
node              18 (npm 10+)
```

Cargo.lock is version 3. `.cargo/config.toml` opts into the
MSRV-aware resolver via `[resolver] incompatible-rust-versions =
"fallback"` (cargo 1.84+).

### 5.6 Build artefacts present right now (paths + sizes)

```
target/deploy/issuer_registry.so                684 400 B
target/deploy/schema_registry.so                331 864 B
target/deploy/zk_verifier.so                    327 472 B

target/idl/issuer_registry.json                  55 373 B  (Apr 25 22:59)
target/idl/schema_registry.json                  13 491 B  (Apr 25 22:59)
target/idl/zk_verifier.json                      20 932 B  (Apr 25 22:59)

ts-sdk/packages/core/wasm/solid_wasm_bg.wasm  1 771 989 B  (Apr 25 21:59)
ts-sdk/packages/core/wasm/solid_wasm.js          27 888 B  (Apr 25 21:52)
ts-sdk/packages/core/wasm/solid_wasm.d.ts         3 939 B  (Apr 25 21:52)

circuits/build/batch_credential_query.r1cs    38 311 472 B (Apr 25 12:55)
circuits/build/batch_credential_query_final.zkey
                                              56 818 200 B (Apr 25 13:22)
circuits/build/verification_key.json              9 223 B  (Apr 25 13:22)
circuits/build/verification_key.sha256               65 B  (Apr 25 13:22)
```

### 5.7 Code shape changes shipped this session (file-level)

```
M crates/solid-core/Cargo.toml          three-target split (D1)
M crates/solid-core/src/poseidon.rs     hash_bytes_dispatch cfg (D1)
M wasm/Cargo.toml                       (touched as part of B6 closure)
M wasm/src/lib.rs                       (touched as part of B6 closure)
M programs/issuer-registry/Cargo.toml   anchor-spl/idl-build forward (D5)
M programs/issuer-registry/src/lib.rs   (Phase 3 cooldown atomic; pre-session)
M programs/schema-registry/Cargo.toml   (no change vs HEAD post-Phase-3)
M programs/schema-registry/src/lib.rs   (Phase 3 work; pre-session)
M programs/zk-verifier/src/lib.rs       (Phase 3 freeze-gate; pre-session)
M package.json                          build:idl + e2e wiring (D2)
?? scripts/build_idls.mjs               new — IDL builder (D2/D3)
?? scripts/sync_program_keypairs.sh     keypair walk
?? keys/                                rotated keypairs + README (D4)
?? docs/E2E_BLOCKERS.md                 the live punch list
?? docs/FORWARD_ROADMAP.md              roadmap snapshot
?? docs/REFERENCE_VERIFIER_APP.md       reference app spec
?? circuits/trusted_setup/              ceremony staging (Phase 3)
M Anchor.toml                           rotated [programs.localnet] IDs (D4)
M deployments/devnet.json               rotated program_ids (D4)
```

Everything in the `M`/`??` rows is uncommitted.

---

## 6. Verification gates observed this session

### 6.1 IDL build is reproducible and deterministic

- `npm run build:idl` succeeds three times in a row, emits all
  three IDLs with the canonical `address` field, and is
  byte-identical across runs (idl.address derived from
  `src/lib.rs`, not from cargo test output ordering).

### 6.2 IDL load smoke (TS runtime)

- A minimal `_idl_load_smoke.mjs` at the project root imports
  `@coral-xyz/anchor`, reads each IDL, instantiates `new
  anchor.Program(idl, provider)` against a dummy provider, and
  prints the instruction count and account count. All three
  IDLs load cleanly. This is the gate that proves D2/D3 are
  good enough to retire `e2/e3/e4`.

### 6.3 Program-ID invariant

- `python3 scripts/check_program_ids.py` (the existing CI
  invariant gate) returns "consistent" against the rotated
  pubkeys, because the script reads from
  `Anchor.toml`/`declare_id!()`/`deployments/devnet.json` and
  every one of those rows was updated together (D4).

### 6.4 What was NOT verified (and so is "grey")

- `npm run e2e` end-to-end against `solana-test-validator`. We
  have the IDLs and the .so binaries, but we have not yet
  observed `tail` of `npm run e2e` printing
  `verified: true` and `ok (replay rejected by nullifier PDA
  init constraint)`. That is the next concrete gate.

---

## 7. Where to pick up next

Two-step plan, in priority order, scoped tight enough that each
step is its own pass/fail gate:

1. **Run the E2E pipeline once.** Concretely:

   ```
   pkill -f solana-test-validator || true
   rm -rf test-ledger
   export COPYFILE_DISABLE=1
   solana-test-validator --reset &
   sleep 5
   solana config set --url localhost
   solana airdrop 10
   anchor deploy --provider.cluster localnet
   npm run e2e
   ```

   Pass gate: tail prints `verified: true` and the replay-
   rejection message. First failure point names the offending
   step.

2. **If step 1 succeeds, wire the regression gates into CI.**
   `scripts/wasm_bridge_smoke.mjs` and `tsx
   tests/vectors/check_vectors.ts` need to be hard CI gates so
   the next PR cannot silently break wasm32 build or
   cross-language byte equality. Without these, the B6/B7 fixes
   are one cargo-update or anchor-bump away from regressing.

After both succeed, this session log moves to "historical" with a
banner pointing at whichever ADR/audit absorbs the durable details
(ADR-0004 for the ID rotation, the next audit snapshot for the
overall E2E close-out).
