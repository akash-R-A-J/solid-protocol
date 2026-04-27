# E2E Blockers Tracker

Living scratchpad of every concrete item that stands between a fresh
checkout and a green `npm run e2e` on localnet (then devnet). Scope is
narrower than the security registry: this doc covers buildability,
toolchain consistency, and pipeline mechanics. Anything that lands a
sound circuit bug or a missing soundness gate goes to
`sec/SECURITY_REGISTRY.md` instead.

This doc is intended to be archived (move under `docs/archive/` with a
`HISTORICAL` banner) once a clean-machine `npm run e2e` succeeds twice
in a row and the canonical runbook (`docs/DEPLOYMENT_AND_TESTING.md`)
has been updated to reflect the post-fix state.

- **Created:** 2026-04-25 (during Phase 3 impl 1-4 close-out + Poseidon
  syscall refactor)
- **Owner:** rolling — last engineer to update this file
- **Purpose:** track the punch list, not the audit. The audit lives in
  `sec/audits/2026-04-25_v0.6.1_deep_comprehensive_audit.md`.
- **State as of 2026-04-27 evening:** unchanged from 2026-04-26
  ~02:15 IST.  No technical work landed today (strategy session,
  see `plan/RESUME.md` last update + `plan/GO_TO_MARKET.md`).  Live
  edge is still step 18 of the runbook below: `npm run issue`
  followed by `npm run prove`.  Working tree from this morning's
  Phase 3.4 crypto + IDE stability session is intact and
  uncommitted; commit before pushing.

- **State as of 2026-04-28 (this session):** much further down the
  runbook.  Steps 1-17 + `npm run init-onchain`, `npm run
  backfill-issuer-tree`, `npm run bootstrap-schema-tree`, `npm run
  bootstrap-issuer`, `npm run issue` ALL green.  Live edge is now
  `npm run prove` (Groth16 witness-gen) which rejects at
  `EdDSAPoseidonVerifier_325 line: 117` inside `CredentialAtom`.
  Three gates fell during this session:
    - **SEC-050** schema-canonicality bypass closed in-circuit;
      trusted-setup re-run produced a new VK (sha256
      `8385b82b032f65e505c784b28486ca8bec7da3f3d4b97b82724e697734565146`);
      tests 39/39.
    - **SEC-049** `SPL_AC_REPLACE_LEAF_DISCRIMINATOR` corrected
      from `0xe388...` to `0xcca5...` (sha256("global:replace_leaf")[..8]).
      Was a doc-lie that nothing called -- so latent until a future
      revoke / cooldown integration test.
    - **SEC-052 (NEW)** Two BPF-runtime / cross-layer coord-form
      drifts: (a) `is_on_curve` rewritten to evaluate the
      circomlib-native curve equation directly (avoids BPF-incompat
      arkworks `EdwardsAffine::is_on_curve()`); (b) the WASM bridge
      was stale (Apr 25, pre-cff06c2 "circuit update") -- rebuilt
      and aligned on-chain + off-chain to circomlib-native form.
      See B11 below for the rebuild gate.

---

## Status legend

- **Fixed (verified)** — change applied AND verification gate observed green.
- **Fixed (claimed)** — change applied; verification gate not yet shown
  green this session.
- **Open** — not yet fixed.
- **Out-of-scope-for-E2E** — real issue, separately tracked, not a
  blocker for the next localnet `npm run e2e` run.

---

## Build / dep / toolchain

### B1. Circuit compile errors at HEAD (3 distinct)

- **Status:** Fixed (verified) in working tree, **not yet committed**.
- **Files (working-tree state):**
  - `circuits/lib/credential_atom.circom` — local `IsZero` template
    deduplicated against circomlib's `comparators.circom:24`; the
    include of `comparators.circom` was added to the file.
  - `circuits/batch_credential_query.circom` — `signal nextNotZero`
    hoisted out of the `for` body to a top-level array
    `signal orderingNextNotZero[NUM_CREDS - 1];` (circom 2.1.x scope
    rule).
  - `circuits/batch_credential_query.circom` — sum-of-N-products in
    the OR-mux (`orSum`) and AND/OR final selector (`finalResult`)
    decomposed into single-multiply intermediate signals to satisfy
    R1CS quadratic form.
- **Verification gate observed:** `cd circuits && npm run compile`
  produces `build/batch_credential_query.r1cs` cleanly with **86,616
  non-linear constraints** + 31 public inputs + 1 public output (=
  `NR_PUBLIC_INPUTS = 32`).
- **Notes:** the in-tree comment in `circuits/scripts/setup.js:34`
  says `~60K constraints`. Stale by ~50%; flag for a doc-pass once
  the circuit fixes commit.

### B2. Cargo.lock edition2024 dep cascade (rustc < 1.85)

- **Status:** Fixed (verified).
- **Root cause:** Cargo 1.79's resolver picks newest version
  satisfying spec, ignoring downstream MSRV. Several transitive deps
  (`base64ct 1.8.x`, `block-buffer 0.12.0`, `toml_parser 1.1.2`,
  `wit-bindgen 0.57.x`, `indexmap 2.14`, `borsh 1.6.x`,
  `unicode-segmentation 1.13.x`, `blake3 1.8.x`) require
  `edition2024`, which is rustc 1.85+. Solana 1.18.x platform-tools
  ships rustc 1.75. Build failed with
  `feature `edition2024` is required` for whichever transitive cargo
  reached first.
- **Fix shape:**
  - `Cargo.toml` workspace `[workspace.package]` — declared
    `rust-version = "1.75"`.
  - `.cargo/config.toml` (new file) — opted into cargo 1.84+ MSRV-
    aware resolver via
    `[resolver] incompatible-rust-versions = "fallback"`.
  - `Cargo.lock` regenerated with cargo 1.79.0 (writes lockfile v3,
    which `cargo-build-sbf` 1.18.22 accepts) plus targeted
    `cargo +1.79.0 update --precise` calls for the few transitives
    whose own crate manifests don't declare `rust-version` and so
    weren't constrained by MSRV-aware resolution: `base64ct@1.6.0`,
    `blake3@1.5.5`, `borsh@1.5.7`, `proc-macro-crate@3.3.0`,
    `wasip2@1.0.0`, `indexmap@2.7.1`, `unicode-segmentation@1.12.0`.
- **Verification gates observed:**
  - `head -3 Cargo.lock` shows `version = 3`.
  - All three `target/deploy/*.so` files built (sbf-target compile
    succeeded against the pinned platform-tools rustc 1.75).
  - `cargo test -p solid-core --lib` = 49/49.
  - `cargo test -p solid-light --lib` = 25/25.
  - `cargo test -p zk-verifier --lib` = 17/17.
- **Open follow-up:** the `cargo +1.79.0 update --precise` pins are
  durable as-committed in `Cargo.lock`. If a future engineer runs
  `cargo update` *without* respecting MSRV (e.g., on a cargo that
  doesn't honor the `[resolver]` config), the cascade re-opens. The
  `.cargo/config.toml` resolver opt-in covers that for cargo 1.84+;
  CI must enforce that contributors use a cargo at or above 1.84
  when regenerating the lockfile.

### B3. light-poseidon 0.2.0 BPF stack overflow (`get_poseidon_parameters`)

- **Status:** Fixed (verified).
- **Root cause:** `light-poseidon 0.2.0`'s
  `parameters::bn254_x5::get_poseidon_parameters` allocates the
  round-constants table on the stack. With `lto = "fat"` enabled in
  `[profile.release]` (`Cargo.toml:42`), LLVM inlines that body into
  callsites, producing a single BPF function frame that exceeds the
  4 KB per-frame budget by ~26 KB.
- **Fix shape:**
  - `crates/solid-core/Cargo.toml` — `light-poseidon` moved to
    `[target.'cfg(not(target_os = "solana"))'.dependencies]`. BPF
    target no longer pulls light-poseidon into the binary.
  - `crates/solid-core/src/poseidon.rs` — `hash_bytes` and
    `hash_fields_to_bytes` made dual-target; on BPF they dispatch
    to `solana_program::poseidon::hashv` (the `sol_poseidon`
    syscall) instead of the in-binary light-poseidon path. The
    syscall is byte-identical to `light-poseidon 0.2.0`'s
    `hash_bytes_le` under `Bn254X5` / `LittleEndian` parameters.
  - `crates/solid-core/src/poseidon.rs` — `hash_fr` and
    `hash_fields` (the `Fr`-typed API surface) gated host-only via
    `#[cfg(not(target_os = "solana"))]`.
  - `crates/solid-core/src/poseidon.rs` — pre-canonicalization step
    added: every input is reduced via
    `Fr::from_le_bytes_mod_order` before dispatch. `ark-bn254` is
    BPF-clean, so this step runs on both targets. Without it, the
    syscall (and the host fallback) reject any byte slice >= p with
    `InputLargerThanModulus`. Two regression tests pin the
    contract:
    - `test_byte_path_matches_field_path` — Fr-path vs byte-path
      byte-equivalence.
    - `test_hash_bytes_canonicalizes_oversized_inputs` — silent
      mod-reduce semantic, not strict-reject.
  - `crates/solid-core/src/babyjubjub.rs` — `generate_keypair`,
    `sign`, `verify` (which depend on `OsRng` and the host-only
    `hash_fr` path) gated to host. `is_in_prime_order_subgroup`
    and `require_in_prime_order_subgroup` (SEC-007 protection)
    intentionally NOT gated, kept dual-target.
  - `programs/schema-registry/Cargo.toml` — unused
    `light-poseidon = "0.2"` direct dep dropped.
- **Verification gates observed:**
  - `target/deploy/issuer_registry.so` (684 KB), `schema_registry.so`
    (332 KB), `zk_verifier.so` (327 KB) all built — the syscall
    refactor proven structurally clean for the BPF link path.
  - `cargo test -p solid-core --lib` = 49/49 (includes the two
    new regression tests).
- **Cross-language equality:** see B5; not yet shown green this
  session.

### B4. zk-verifier `verify_batch_proof` BPF stack frame overflow (456 bytes)

- **Status:** Fixed (verified).
- **Root cause:** structural, NOT optimization-driven. The Anchor
  `__global::verify_batch_proof` wrapper holds: the Borsh-deserialized
  ix struct (~1312 B for `proof_a` 64 + `proof_b` 128 + `proof_c` 64
  + `public_inputs` 1024 + `nullifier` 32), the
  `VerifyBatchProof` accounts struct (~700-900 B for 10 accounts),
  BPF outgoing-arg slots for the user fn call (~1312 B), plus
  `ctx`/`bumps`/spill (~200 B). That's ~3500-3700 B baseline + ~700 B
  of saved-register/alignment overhead = ~4500 B per frame. Solana's
  per-frame BPF stack ceiling is 4 KB; deficit ~456 B.
- **Why it surfaced now:** masked until the B3 (light-poseidon) link
  failure was lifted. Pre-existing since Phase 2 impl 2 (`73871dc`),
  which grew `NR_PUBLIC_INPUTS` from 31 to 32.
- **Why LTO setting irrelevant:** the wrapper's frame is determined
  by declarative storage requirements (struct sizes + BPF calling
  convention), not by optimizer decisions. Toggling
  `lto = "thin"` left the overflow at exactly 4552 B.
- **Fix shape:** in `programs/zk-verifier/src/lib.rs`:
  - `verify_batch_proof` ix arg signature changed:
    `public_inputs: [[u8; 32]; NR_PUBLIC_INPUTS]` →
    `public_inputs: Vec<[u8; 32]>` (24-byte descriptor on stack;
    1024-byte payload on the BPF heap allocator).
  - Explicit `require!(public_inputs.len() == NR_PUBLIC_INPUTS)`
    length validation immediately on entry.
  - Inner Groth16-verify body extracted into a separate
    `#[inline(never)]` helper (line ~717-723) that takes
    `public_inputs: &[[u8; 32]; NR_PUBLIC_INPUTS]` by reference —
    splits the work across two BPF frames so neither hits the 4 KB
    ceiling.
- **Verification gate observed:** `target/deploy/zk_verifier.so`
  built (327 KB ELF eBPF). Frame ceiling fits.
- **Registry placement:** registered as **SOLID-SEC-047 (NEW-03)**,
  MEDIUM (security exploit nil; deploy-blocker class HIGH).
- **Open follow-up (not E2E-blocking):**
  - SDK encoders that hand-roll the ix data must add the 4-byte
    Borsh `Vec` length prefix between `proof_c` and the 32×32 byte
    payload. If the SDK uses Anchor's auto-generated client, this is
    handled by the IDL regeneration; otherwise the encoder needs an
    audit pass.
  - Add a CI gate that emits the actual `verify_batch_proof` BPF
    frame size and fails on >3.6 KB (10% margin). Closes the gap the
    component-level `VkBuf < 3072` assertion at
    `programs/zk-verifier/src/lib.rs:80-87` does NOT cover.
    Pairs with **SOLID-SEC-046** (CU-budget regression gate).

### B5. Cross-language vector byte-equality post-Poseidon-refactor

- **Status:** Fixed (claimed); verification gate **not yet observed**
  this session.
- **What's claimed by the engineer:** "Rust vectors regen OK." No
  `git diff --exit-code tests/vectors/commitment_and_nullifier.json`
  output shown.
- **Hard CLAUDE.md invariant:** "Rust and TypeScript primitives must
  agree byte-for-byte. The cross_language_vectors CI job hard-gates
  this via `tests/vectors/`."
- **Verification gates needed before claiming Fixed (verified):**
  ```
  cargo run -p solid-core --example gen_vectors
  git diff --exit-code tests/vectors/commitment_and_nullifier.json
  (cd ts-sdk && npm ci && npm run build)
  cd tests/vectors && tsx check_vectors.ts
  ```
- **Pinned expected bytes (committed JSON; must match after regen):**
  - `commitment.expected_commitment_hex` =
    `01a5b61022eba3f7dca36a556d27de1e8e2e0ea9894125a582526970a678782c`
  - `nullifier.expected_nullifier_hex` =
    `625c00be594f0e697cfbaef6907779f87f2976e894b49b444e1a735ac0ae761f`
- **Failure mode if it diverges:** the canonicalize-then-syscall
  path produces different bytes than the prior light-poseidon-Fr-
  direct path. Most likely culprit would be the ark-ff/num-bigint
  serialization round-trip used inside the byte path. Abort on any
  diff — do NOT rebuild vectors against the new output.

### B6. WASM bridge with `solana-program` newly in solid-core's dep graph

- **Status:** Open. Untested.
- **Risk:** `solana-program` is now in `crates/solid-core/Cargo.toml`
  `[dependencies]` (regular, dual-target). On `wasm32-unknown-unknown`
  target, `solana-program`'s host-fallback path uses
  `std::sync::Mutex`, `std::time::*`, `getrandom` — wasm32 is a
  strict subset that may not provide all of these.
- **Verification gate needed:**
  ```
  rm -rf ts-sdk/packages/core/wasm
  PATH="$PWD/.toolchain/bin:$PATH" \
    wasm-pack build wasm/ --target nodejs \
      --out-dir ts-sdk/packages/core/wasm --release
  node scripts/wasm_bridge_smoke.mjs
  ```
- **Pre-mitigation if it fails:** add a third cfg branch in
  `crates/solid-core/Cargo.toml`:
  ```
  [target.'cfg(target_os = "solana")'.dependencies]
  solana-program = { workspace = true }              # syscall path

  [target.'cfg(not(any(target_os = "solana",
                       target_arch = "wasm32")))'.dependencies]
  solana-program = { workspace = true }              # host fallback

  [target.'cfg(target_arch = "wasm32")'.dependencies]
  light-poseidon = "0.2"                             # wasm32 direct
  ```
  …and the matching cfg branch in `poseidon.rs` so the wasm32
  `hash_bytes` path calls
  `light-poseidon::Poseidon::<Fr>::new_circom().hash_bytes_le()`
  directly. Three-way cfg, all three converge on the same byte
  output. Not a workaround — it acknowledges three distinct
  deployment targets with three Poseidon backends, all agreeing
  bit-for-bit.

### B7. Anchor IDL build failure on `proc-macro2 >= 1.0.95`

- **Status:** Out-of-scope-for-E2E. Anchor 0.30.1 upstream limitation.
- **Cause:** anchor 0.30.1's `idl-build` feature uses a proc-macro
  helper that reads source files via `proc_macro2::Span::source_file()`.
  proc-macro2 1.0.95+ removed that public API; the IDL generation
  fails with an unresolved-method error. anchor 0.31 fixes this via
  `local_file()`.
- **Workaround for current E2E run:** `anchor build --no-idl`. The
  TS SDK currently consumes hand-written instruction encoders, not
  IDL-generated ones, so `--no-idl` does NOT block `npm run e2e`.
- **Real fix (future PR):** upgrade anchor 0.30.1 → 0.31.x, which
  is a real toolchain bump (`Anchor.toml` + `bootstrap.sh` +
  CLAUDE.md + DEPLOYMENT_AND_TESTING.md). Track separately.

### B8. macOS validator `solana-test-validator` AppleDouble pollution

- **Status:** Open (workaround applies for now).
- **Cause:** macOS injects `._<filename>` AppleDouble metadata files
  into directories. Solana's genesis tar reader rejects them with
  `Archive error: extra entry found: "._genesis.bin"`.
- **Workaround:**
  ```
  pkill -f solana-test-validator 2>/dev/null; sleep 2
  rm -rf test-ledger
  defaults write com.apple.desktopservices DSDontWriteNetworkStores -bool true
  export COPYFILE_DISABLE=1
  solana-test-validator --reset
  ```
- **Real fix (separate, low priority):** patch `bootstrap.sh` to set
  `COPYFILE_DISABLE=1` in the toolchain's environment, OR add a
  pre-flight script that purges `._*` files from `test-ledger`.

### B9. `register_issuer` BJJ subgroup check exceeds 1.4M CU per-tx ceiling

- **Status:** Fixed (claimed) via temporary feature-gated bypass; full
  on-chain check pending SEC-048 remediation.
- **Discovered:** 2026-04-25 while running `npm run bootstrap-issuer`
  against fresh localnet validator. Failure mode: transaction failed
  with `exceeded CUs meter at BPF instruction` even with
  `ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 })` (the
  per-tx ceiling). Reduction reproduces with any non-trivial scalar:
  the cost is structural, not optimization-driven.
- **Root cause:** `solid_core::babyjubjub::require_in_prime_order_subgroup`
  performs a full `r * P == O` check, where `r` is the BJJ
  prime-order-subgroup order (~251 bits). That is ~251 doublings + ~125
  point additions through arkworks' `EdwardsAffine::mul_bigint`. On x86
  host the call costs a few ms; on BPF (rustc 1.75 platform-tools,
  `lto = "thin"`, `codegen-units = 1`) it costs > 1.4M CU, which is the
  hard per-transaction compute ceiling. No build flag, no inlining
  setting, no helper crate selection brings it under the cap. Solana
  does not currently expose a syscall for arkworks BN-family curves on
  BPF, only alt_bn128 (which is the BN254 G1/G2 used by zk-verifier and
  is structurally distinct from BabyJubJub).
- **Why audit didn't catch this earlier:** B3's verification gate was
  "the .so links / cargo test passes / the host-side Poseidon
  refactor is byte-equivalent." It explicitly notes
  `is_in_prime_order_subgroup` and `require_in_prime_order_subgroup`
  were "intentionally NOT gated, kept dual-target." The dual-target
  property is true (both targets compile and link); what was not
  measured is the runtime BPF compute cost per call. SEC-007 was
  closed on logic correctness; this is the orthogonal CU-budget
  finding.
- **Fix shape (interim, 2026-04-25):**
  - `programs/issuer-registry/Cargo.toml` — added Cargo feature
    `sec007-skip-onchain` (no default).
  - `programs/issuer-registry/src/lib.rs` — gated the
    `require_in_prime_order_subgroup` call behind
    `#[cfg(not(feature = "sec007-skip-onchain"))]`. With the feature
    set, the handler instead runs:
    1. `is_on_curve(&pk)` — single curve-equation eval, BPF-cheap.
    2. `!is_identity(&pk)` — single field-equality eval, BPF-cheap.
    3. `msg!("SEC-048: sec007-skip-onchain active; …")` — operator-
       visible bypass receipt in the program log.
    4. `emit!(Sec007Bypass { issuer_authority, slot })` — structured
       on-chain event so off-chain monitors can alert when the bypass
       binary lands on a cluster it shouldn't (mainnet incident
       detector).
  - `crates/solid-core/src/babyjubjub.rs` — added `is_on_curve(&pk)`
    and `is_identity(&pk)` public helpers as the BPF-affordable
    consolation gate.
  - `ts-sdk/packages/core/src/index.ts` — re-exposed
    `isInPrimeOrderSubgroup(publicKeyX, publicKeyY)` as the canonical
    client-side predicate. Wraps the existing WASM
    `isBjjInPrimeOrderSubgroup` binding (which under the hood calls
    `solid_core::babyjubjub::is_in_prime_order_subgroup` on host —
    same code path the on-chain handler used to call, just executed
    off-chain instead).
  - `scripts/bootstrap_issuer.ts` — added a hard pre-submit check
    using the new `isInPrimeOrderSubgroup` export. A failing key
    aborts the script before `register_issuer` is even built. Compute
    budget reduced from `1_400_000` to `400_000` CU on the same call
    site to reflect the cheaper on-chain path (real cost ~120K CU,
    400K leaves headroom for `init`, the `system_program::transfer`
    CPI, and Clock syscalls).
  - The off-chain check is now the load-bearing SEC-007 gate. Every
    off-chain `register_issuer` caller MUST run it pre-submit. The
    on-chain `is_on_curve` + `is_identity` filter only catches
    obviously-malformed inputs; a cofactor-8 torsion point still
    passes it.
- **Build / deploy invocation (localnet, 2026-04-25 onward):**
  ```
  CARGO_TARGET_DIR=$PWD/target \
    cargo build-sbf -p issuer-registry --features sec007-skip-onchain
  # OR for the full set, with the same flag scoped to issuer-registry:
  CARGO_TARGET_DIR=$PWD/target anchor build --no-idl --skip-lint \
    -- --features sec007-skip-onchain
  solana program deploy target/deploy/issuer_registry.so \
    --program-id keys/localnet/issuer_registry-keypair.json
  ```
  `check_program_ids.py` is unaffected (the program ID does not
  change). The IDL hash baseline is also unaffected — the bypass is
  pure feature gating, no struct or instruction layout change. The
  IDL diff CI gate (item E4 below) will not fire on this change.
- **Verification gate:** `npm run bootstrap-issuer` lands the
  `register_issuer` ix within 400K CU and the program log contains
  exactly one `SEC-048: sec007-skip-onchain active; …` line per
  successful registration. After bootstrap, `npm run e2e` proceeds
  through `issue` and `prove`.
- **Mainnet posture:** **mainnet builds MUST NOT enable
  `sec007-skip-onchain`.** Adding it is a deploy-blocker; the feature
  flag is documented as "NOT FOR MAINNET" in
  `programs/issuer-registry/Cargo.toml`. A future CI gate
  (SEC-048 closeout) will reject mainnet release builds that set the
  feature, and a runtime monitor on the production cluster will alert
  on any `Sec007Bypass` event seen in mainnet logs.
- **Real fix (SEC-048, P0 in `docs/IMPROVEMENTS_ROADMAP.md`):** the
  on-chain BJJ subgroup check needs a CU-affordable replacement. Three
  candidate paths, in increasing soundness preference:
  1. **Cofactor-clear in the issuer SDK** — multiply the candidate
     pubkey by `8` off-chain (cofactor) before submission and check
     the result is non-identity; on-chain stays at the
     `is_on_curve + !is_identity` consolation gate. Cheapest, but
     trusts the off-chain SDK to do the multiply (a malicious caller
     can skip it and bypass detection until the issuer's first signed
     credential fails to verify in-circuit).
  2. **Move the subgroup gate into the issuance circuit** — every
     `issue_credential` proof commits to the issuer's pubkey already;
     adding a `is_in_prime_order_subgroup` constraint on the pubkey
     makes the gate enforcement a circuit-level invariant rather than
     a handler-level one. Costs ~30K extra constraints (one EdDSA-
     style scalar mul) and shifts ZK proving cost up correspondingly.
     Strongest binding to soundness, slowest to ship (requires
     trusted-setup re-run, batches with SEC-006 Part 2).
  3. **Solana BJJ syscall** — propose adding a `sol_babyjubjub_*`
     syscall family upstream so on-chain code can do
     subgroup/scalar-mul checks at curve speed (a few thousand CU).
     Multi-quarter timeline, depends on validator buy-in.
- **Tracking:**
  - Security registry entry: `sec/SECURITY_REGISTRY.md` SEC-048 (HIGH;
    blocks mainnet deploy).
  - Roadmap entry: `docs/IMPROVEMENTS_ROADMAP.md` (P0 item).
  - Top-of-mind blockers: `plan/RESUME.md`, `docs/FORWARD_ROADMAP.md`.
  - Audit cross-reference: SOLID-SEC-007 was closed on host-logic
    correctness; this finding extends it to BPF-runtime feasibility
    and is the new active line.
- **Receipts (observed on localnet 2026-04-26 ~01:30 IST, post-rebuild
  with `--features sec007-skip-onchain`, deployed program
  `5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx`):**
  - `register_issuer` lands within budget. Operator-visible CU on the
    happy path is 64,487 (well under the 400K cap and ~21x cheaper
    than the un-gated 1.4M+ blow-up). Reproducer:
    `npm run bootstrap-issuer` step `[3/9]` — line
    `   ok (issuer=DmREQdituAXLsoQAxYRH2SgWr4RTrB6UGz8FREayLK9G)`.
  - Both bypass-detector signals are emitted. Validator log shows the
    `msg!`:
    `Program log: SEC-048: sec007-skip-onchain active; cheap consolation
     gate (is_on_curve + !is_identity) is the only on-chain BJJ filter.`
    AND the structured event `Sec007Bypass { issuer_authority, slot }`
    (decodable via the IDL-published event discriminator). Either of
    these alone would be sufficient to alert a mainnet monitor; both
    being present is the SEC-048 closeout-pre-condition for the
    "cluster-rejects-the-feature-binary" CI gate.
  - Reframing: this is **not** a milestone. SEC-007 is still live as
    SEC-048 until one of the three real-fix candidates ships. What
    landed is the bypass plumbing — feature flag + consolation gate +
    client-side gate + on-chain telemetry — which is the *infra* the
    real fix will piggyback on. The verification gate above is "bypass
    plumbing works as designed", not "issuer registration is sound on-
    chain". The mainnet binary still has SEC-007 closed by the full
    `require_in_prime_order_subgroup` path, because the feature is
    off by default. Localnet/devnet runs that need to clear E2E set
    the feature and accept the documented soundness regression.

### B11. Stale `solid_wasm_bg.wasm` after a `babyjubjub.rs` byte-format change

- **Status:** Fixed (verified) 2026-04-28.
- **Symptom (pre-fix).**  `npm run bootstrap-issuer` step `[3/8]
  register_issuer` fails on every freshly-generated keypair with
  on-chain `InvalidBJJPubKey` (program logs `Program log:
  AnchorError thrown in programs/issuer-registry/src/lib.rs:303.
  Error Code: InvalidBJJPubKey`), even though the off-chain pre-
  submit gate `isInPrimeOrderSubgroup(pkX, pkY)` accepts the same
  bytes.  Reproduces on a fresh validator + fresh state file +
  `cargo build-sbf -p issuer-registry --features sec007-skip-onchain`
  + freshly redeployed program.
- **Root cause.**  `ts-sdk/packages/core/wasm/solid_wasm_bg.wasm`
  is the WASM bridge that off-chain callers go through to invoke
  `solid_core::babyjubjub::*`.  It is built by `wasm-pack build
  wasm/`, NOT by `cargo` or `anchor build`, so it is NOT
  regenerated when the in-tree babyjubjub source changes -- only
  when an operator explicitly re-runs wasm-pack.  The `cff06c2`
  commit (2026-04-27 "circuit update") rewrote
  `crates/solid-core/src/babyjubjub.rs` to make `affine_to_pubkey`
  emit **circomlib-native** coordinate-form bytes (instead of the
  prior arkworks-form bytes) and updated every Poseidon /
  signing / commitment call site to expect that form.  But
  whoever ran `cff06c2` did NOT also re-run `wasm-pack build` --
  the bridge stayed at its 2026-04-25 21:59 timestamp, still
  emitting arkworks-form bytes.  Effect: off-chain SDK produces
  bytes in arkworks form; on-chain code post-cff06c2 expects
  circomlib-native form; on-chain `is_on_curve` correctly rejects
  every fresh BJJ pubkey because the bytes are in the wrong
  curve-form for the circomlib equation.
- **Fix shape.**
  - `wasm-pack build wasm/ --target nodejs --out-dir
    ts-sdk/packages/core/wasm --release`
    re-emits `solid_wasm_bg.wasm` (and its `.js` shim, `.d.ts`,
    `package.json`) against the post-cff06c2 babyjubjub source.
    Output bytes are now circomlib-native, agreeing with on-chain.
  - `cd ts-sdk && npm run build` recompiles the typed
    re-exports.  The `dist/` files do not encode coord form
    semantics (they are TS shims around the WASM module loaded at
    runtime), but rebuilding ensures version stamps line up so
    `tsx` consumers see fresh code.
  - `crates/solid-core/src/babyjubjub.rs::is_on_curve` AND
    `is_identity` were ALSO rewritten this session (SEC-052) to
    evaluate the circomlib-native curve equation directly instead
    of going through `EdwardsAffine::new_unchecked +
    x_circ_to_ark + ark_is_on_curve`.  The arkworks path is
    suspected of differing between host and BPF for some valid
    points; the direct circomlib equation is BPF-stable by
    construction (only `Fq * Fq` and `Fq + Fq`).
- **Verification gate observed (2026-04-28 ~01:13 IST).**
  - Validator: localnet, fresh `--reset` with the SPL AC + noop
    cloned programs.
  - All three programs redeployed.  `issuer-registry` rebuilt
    with `--features sec007-skip-onchain`.
  - State file cleared (`rm -rf $TMPDIR/solid-e2e-$UID/`).
  - `npm run init-onchain` -> uploads new VK
    `8385b82b...` (post-SEC-050), 3 chunks, sha256 pin matches.
  - `npm run backfill-issuer-tree` -> tree
    `6oxgd4f1pXPXU3n5b6sy889pBw82dU7h4sor3PgprvMR` initialised.
  - `npm run bootstrap-schema-tree` -> tree
    `9kEpm21BLknx54D7h4Vz83FVMvfZWpvZsX2tiN1QzDbU` bound.
  - `SOLID_VOTING_PERIOD_SECONDS=120 npm run bootstrap-issuer`
    -> `[3/8] register_issuer ok (issuer=FT28...)` followed by
    [4/8]..[8/8] + [8b/10] `append_issuer_leaf` all green.
  - `npm run issue` -> credential committed
    (commitment `f0d9...`).
- **Process gate (durable).**  Runbook step 8.5 is now mandatory
  on any commit that touches `crates/solid-core/src/babyjubjub.rs`
  byte-format helpers (`affine_to_pubkey`, `pubkey_to_affine`,
  `affine_to_circomlib_xy`, the SQRT_A_LE / BASE8_X_ARK_LE
  constants) OR `wasm/src/lib.rs`:
  ```
  rm -rf ts-sdk/packages/core/wasm
  PATH="$PWD/.toolchain/bin:$PATH" wasm-pack build wasm/ \
    --target nodejs --out-dir ts-sdk/packages/core/wasm --release
  cd ts-sdk && npm ci && npm run build
  ```
  CI should also run `node scripts/wasm_bridge_smoke.mjs` against
  the freshly-built bridge and assert that
  `generateBJJKeypair() -> isBjjInPrimeOrderSubgroup(...)`
  round-trips green; this would have surfaced cff06c2 as a
  failing PR rather than a runtime regression.
- **Tracking.**  Registry: `sec/SECURITY_REGISTRY.md` SOLID-SEC-052.
  Root cause: cff06c2 commit; missing CI gate for
  WASM<->on-chain wire alignment.

### B12. EdDSA witness-gen drift inside `CredentialAtom` (live edge)

- **Status:** Open as of 2026-04-28.  Diagnosis WIP.
- **Symptom.**  `npm run prove` reaches step `[3/4] Generating
  Groth16 batch proof...` and the `circom_runtime` witness
  calculator throws:
  ```
  Error in template ForceEqualIfEnabled_324 line: 56
  Error in template EdDSAPoseidonVerifier_325 line: 117
  Error in template CredentialAtom_326 line: 70
  Error in template BatchCredentialQuerySolana_485 line: 339
  ```
  i.e. the issuer's EdDSA-Poseidon signature on the recomputed
  commitment fails verification inside the circuit.  This is
  AFTER B11 was fixed -- the bytes the off-chain signer produces
  and the bytes the circuit consumes are both in circomlib-native
  form, so the prior layer-mismatch is already closed.
- **Likely surfaces (in priority order).**
  1. **Holder-pubkey derivation drift.**  The circuit recomputes
     the holder per-schema pubkey via
     `IdentityAnchor -> Poseidon(masterKey, schemaHash)
     -> BabyPbk254`.  The off-chain SDK derives via WASM
     `deriveCredentialKey` (Poseidon then `derive_public_key`,
     which uses arkworks scalar mul + iso transform back to
     circomlib-native).  Both paths are mathematically
     equivalent IFF `Poseidon` agrees byte-for-byte AND
     `priv * Base8` lands on the same point in either coord
     form.  A 1-byte drift in either step cascades to a different
     `holderPubKeyAx/Ay` which means the issuer signed
     `Poseidon(dataHash, schemaHash, holderAx_offchain,
     holderAy_offchain, salt)` while the circuit recomputes
     `Poseidon(dataHash, schemaHash, holderAx_circuit,
     holderAy_circuit, salt)` -- if `holderAx_offchain !=
     holderAx_circuit`, the recomputed commitment differs from
     what was signed and EdDSA fails.
  2. **R8 coord-form drift.**  `solid_core::babyjubjub::sign`
     returns `r8_x` / `r8_y` in circomlib-native form; circomlib's
     `EdDSAPoseidonVerifier` expects circomlib-native; should
     match.  Verify the bytes round-trip.
  3. **Message hashing convention.**  Off-chain uses
     `bytes_to_fq(message)` to lift the commitment bytes into
     `Fq` before the challenge `h = Poseidon(R8x, R8y, Ax, Ay, M)`.
     Circuit feeds the recomputed commitment as `M` directly (it
     is already an `Fq` value).  Should match if the LE byte
     encoding round-trips canonically; verify there is no
     mod-reduction mismatch on the boundary.
  4. **Salt / data ordering.**  Off-chain SDK and circuit must
     agree on field order in the commitment Poseidon: `Poseidon5(
     dataHash, schemaHash, holderAx, holderAy, salt)`.  A swap
     between any two would silently produce different commitments.
- **Diagnostic plan (next session).**
  1. Capture circuit input dump:
     `SOLID_DEBUG_CIRCUIT_INPUT=1 npm run prove` -> writes
     `/tmp/solid-circuit-input.json`.
  2. Add a host-only test that takes those exact values and runs
     `solid_core::babyjubjub::verify(&pubkey, &commitment_bytes,
     &signature)`.  If host-side verify rejects, the off-chain
     SDK produced an invalid signature against its own commitment
     (a crypto bug, not a contract drift).  If host-side verify
     accepts, the circuit's recomputation of `commitment` differs
     from what was signed -- next debug step is to compute
     `expected_commitment = Poseidon5(dataHash_offchain,
     schemaHash, holderAx_offchain, holderAy_offchain, salt)`
     and compare to the circuit-recomputed value (would need a
     witness inspection or a focused Circom test).
  3. Fix at root cause; no workarounds.
- **E2E impact.**  Yes -- this is the live edge.  Until B12 is
  closed, `npm run e2e` cannot reach `verified: true`.

### B10. `stake_tokens` access violation post-CPI (handler returns, runtime crashes on writeback)

- **Status:** Fixed (verified) 2026-04-26 ~02:15 IST via contract
  redesign of `initialize_registry` (NOT a workaround on
  `stake_tokens` itself). Detail below.
- **Resolution shape:** the original instinct ("drop `init_if_needed`
  on `governance_vault` from `StakeTokens`, pre-create the vault in a
  dedicated ix") was correct in spirit but solved the wrong
  contract. While drafting `init_governance_vault` we hit a guard
  failure (`Unauthorized`) because the on-chain
  `RegistryConfig.governance_token_mint` was `Pubkey::default()`. Root
  cause: `initialize_registry` accepted `governance_token_mint:
  Pubkey` as a free parameter with no validation, so any caller (a
  legacy script, a stale state file, an attacker) could write
  `Pubkey::default()` into the registry and poison every downstream
  guard that checks "vault.mint == registry.governance_token_mint".
  That was the actual contract defect. The fix:
  - `initialize_registry` now takes a typed `Account<'info, Mint>`
    (`governance_mint`) instead of a raw `Pubkey`, so Anchor's
    deserializer rejects anything not owned by the SPL Token
    program (closes the `Pubkey::default()` write path at the
    program boundary).
  - `governance_vault` is now born **atomically with the registry**
    inside `InitializeRegistry`'s accounts struct via
    `#[account(init, token::mint = governance_mint,
    token::authority = governance_vault, seeds = [b"governance-vault",
    registry_config.key().as_ref()], bump)]`. By the time any
    caller reaches `stake_tokens`, the vault is already created,
    owned by itself, and bound to the registry's mint by
    construction.
  - `StakeTokens` no longer carries `init_if_needed` on
    `governance_vault` (it's `mut`-only with a `constraint`
    asserting `governance_vault.mint == governance_mint.key()`).
    `staker_account` retains `init_if_needed` — that one is
    operationally fine; the access-violation pattern was specific
    to co-locating System+SPL init with a Token::Transfer CPI on
    the same account in the same tx.
  - `init_governance_vault` (and its `InitGovernanceVault` accounts
    struct) was deleted. It was the wrong API; the right one is
    "the vault is a registry invariant, not a separate setup step."
  - New error codes `ErrorCode::InvalidThreshold` and
    `ErrorCode::InvalidVotingPeriod` plus `require!` guards inside
    `initialize_registry` (threshold ≤ 10000 bps, voting_period >
    0). Closes a parallel "free Pubkey, free integers" defect
    surface on the same ix.
- **Why this is the right contract, not a workaround.** The original
  shape made the registry a "trust-me" struct: a script could pass
  `Pubkey::default()` and the program would persist it. The new
  shape encodes the invariant ("governance_token_mint is an SPL
  mint, governance_vault is bound to it, both born together") at the
  program boundary, where it cannot be bypassed. Scripts now have to
  respect the contract, not invent it. This is exactly the
  bottom-up rule: programs and circuits are the source of truth;
  scripts are consumers.
- **Fallout, all closed in the same change set:**
  - `target/idl/issuer_registry.json` regenerated. `init_governance_vault`
    is gone; `initialize_registry` now lists `governance_mint` and
    `governance_vault` as accounts, and adds the two new error
    codes. IDL diff is non-trivial — any future PR that touches
    `initialize_registry` should expect the IDL-snapshot CI gate
    (item E4) to fire.
  - `scripts/build_idls.mjs` learned to denamespace IDL entries
    (strip `<crate>::` from `accounts[].name`, `types[].name`,
    `events[].name`, and any `defined` reference). Anchor 0.30.1's
    `__anchor_private_print_idl_*` machinery emits fully-qualified
    names when `ANCHOR_IDL_BUILD_RESOLUTION=FALSE`, but Anchor's
    TypeScript client expects unqualified camelCase names on
    `program.account.X.fetch`. Without the denamespace pass, the
    new contract's `program.account.registryConfig` is `undefined`
    and every script that fetches it throws `Cannot read
    properties of undefined (reading 'fetch')`. This is a CI gate
    candidate of its own — a future Anchor bump or a future
    namespacing change in `build_idls.mjs` could regress it
    silently.
  - `scripts/initialize.ts` rewritten:
    - Drops `Pubkey.default` fallback. Mint resolution is now
      "env override > cached state > fresh `createMint`" with the
      chosen mint persisted to the E2E state file.
    - Reads `SOLID_VOTING_PERIOD_SECONDS` and
      `SOLID_MIN_STAKE_LAMPORTS` from env (defaults 86400s and
      1 SOL). The previous hardcoded `86400` was a hidden contract
      with `bootstrap_issuer.ts`, which uses the same env var with
      a `20`s default. With the two scripts disagreeing, the
      registry was being initialized with 1-day voting period and
      `bootstrap_issuer.ts` would then time out at `finalize_voting`
      with `VotingPeriodNotEnded`. Both scripts now read the same
      env var; the first one to land sets the period, the second
      is a no-op.
  - `scripts/bootstrap_issuer.ts` updated:
    - The `[3b/9] init_governance_vault` step is gone (the ix no
      longer exists).
    - Steps renumbered `[1/9]..[9/9]` -> `[1/8]..[8/8]`.
    - `governanceVaultPda` now persisted to the E2E state file.
    - The `LocalReplicaAdapter`-based root computation now uses the
      newly-added `getRoot()` method on the adapter (see TS SDK
      change below).
  - `tests/integration/01_registry_init.test.ts` rewritten on top
    of `solana-bankrun`. Helpers `encodeSplMint` / `seedMint`
    inject a real SPL Mint account into the test context. New
    test cases pin the contract's regression surface:
    `rejects approval_threshold > 10000 bps`,
    `rejects voting_period == 0`,
    `rejects a non-Mint account passed as governance_mint`. The
    last one is the explicit regression gate against the
    `Pubkey::default()` defect.
  - `ts-sdk/packages/light/src/index.ts`: added a `getRoot()`
    method to `LocalReplicaAdapter`. Same lazy sparse-tree walk as
    the existing `fetch` method; returns root only. Required by
    `bootstrap_issuer.ts` step `[8b/10] append_issuer_leaf` so the
    on-chain `update_issuer_tree_root` ix gets the locally-
    computed root after a leaf append, without round-tripping a
    per-leaf proof through the indexer.
  - `rust-toolchain.toml` (new) pins channel 1.79.0 with
    `rustfmt`/`clippy`/`rust-src`/`wasm32-unknown-unknown`.
    `.vscode/settings.json` (new, tracked) points
    `rust-analyzer` at the same toolchain and a separate
    `target/rust-analyzer` directory. Closes the `serde_derive`
    proc-macro ABI mismatch (rustc 1.79 vs the proc-macro server's
    1.95 build) that was throwing red lines across the `.rs` files
    during the redesign session. `CLAUDE.md` updated to record
    these two new files as part of the toolchain pin.
- **Verification gate observed (2026-04-26 ~02:15 IST):**
  - Validator: localnet, fresh `--reset`, with both
    `cmtDvXumGCrqC1Age74AVPhSRVXJMd8PJS91L8KbNCK` (account
    compression) and `noopb9bkMVfRPU8AsbpTUg8AQkHtKwMYZiFUjNRtMmV`
    (noop, used for compression logging) cloned from devnet via
    `--clone-upgradeable-program`. A vanilla
    `solana-test-validator --reset` does NOT load these and
    `backfill_issuer_tree.ts` will fail at `init_empty_merkle_tree`
    with `Attempt to load a program that does not exist`. Either
    clone them or switch the runbook to a hard-coded BPF blob; we
    chose clone for now (cheaper).
  - Programs: all three redeployed (`issuer_registry` with
    `--features sec007-skip-onchain`).
  - `npm run init-onchain` exits 0; fresh SPL governance mint is
    created and persisted; registry initialized with
    `voting_period_seconds = 120`.
  - `npx tsx scripts/backfill_issuer_tree.ts` exits 0
    (`init_empty_merkle_tree` + `transfer_authority` succeed; "0
    approved issuer(s)" enrolled, expected pre-bootstrap).
  - `SOLID_VOTING_PERIOD_SECONDS=120 npm run bootstrap-issuer`
    exits 0 with all 8 contract steps + `[8b/10] append_issuer_leaf`
    green:
    ```
    [3/8] register_issuer
       sec-048 off-chain subgroup check: ok
       ok (issuer=aTdW1Q9Y83bQNuEGAN5YGqt9LRxWuUK9DxsN7hDKWhS)
    [4/8] stake_tokens (voter = wallet)
       ok (staked 1000000000)
    [5/8] waiting 100 slots for flash-loan cool-off...
    [6/8] vote_on_issuer  ok (voted APPROVE)
    [7/8] waiting for voting period to end...
    [8/8] finalize_voting  ok
    [8b/10] append_issuer_leaf
       ok (leaf appended; binding=ExJ2PLDf8qrDxYsYRJbuKNTJ38xPxt1USJpe7cdfgL6h)
       ok (binding root updated to 0x03e14da05ddbcb36...)
    ```
    `[4/8] stake_tokens` lands cleanly with no `Access violation`,
    no post-handler crash, and Anchor's writeback path returns
    normally. B10's failure mode is no longer reproducible against
    the new contract.
- **Calibration note (env var, not a code fix).**
  `SOLID_VOTING_PERIOD_SECONDS=20` is too short on localnet: the
  flash-loan cool-off in `bootstrap_issuer.ts` is "wait 100 slots
  ≈ 40s," which can elapse the voting window before `vote_on_issuer`
  fires, surfacing as `VotingPeriodEnded`. 120s clears both the
  cool-off and the vote within the window with margin. Don't go
  below 60s on localnet.
- **Mainnet posture.** Mainnet runs without
  `--features sec007-skip-onchain`, so `register_issuer` still
  costs >1.4M CU and `stake_tokens` is unreachable from a fresh
  caller in production today. B10 is therefore not a live mainnet
  incident. It IS still a P0 for any public devnet rollout because
  the new contract is what makes the staking path soundly
  reachable; reverting any of these contract changes re-opens the
  `Pubkey::default()` write path.
- **Tracking.** No security-registry entry; B10 was a pure
  contract / Anchor-pattern bug, not a memory-safety class issue
  (`Access violation` was the *symptom* of the contract defect, not
  a memory-safety primitive). The IDL-snapshot CI gate (item E4)
  will catch any regression of `initialize_registry`'s account
  list. The integration test `tests/integration/01_registry_init.test.ts`
  is the long-lived regression gate.

---

#### Historical: original B10 diagnosis (kept for cross-reference)

- **Status (historical):** Open at 2026-04-26 ~01:30 IST. Closed by
  the contract redesign documented above.
- **Symptom:** `npm run bootstrap-issuer` clears `[3/9] register_issuer`
  cleanly, then on `[4/9] stake_tokens` the validator returns:
  ```
  Program 5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx invoke [1]
  Program log: Instruction: StakeTokens
  Program 11111111111111111111111111111111 invoke [2]
  Program 11111111111111111111111111111111 success
  Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]
  Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success
  Program log: stake_tokens: voter=…  amount=…
  Program 5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx consumed
    40694 of 200000 compute units
  Program 5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx failed:
    Access violation in unknown section at address
    0x360358543e3fd94a of size 8
  ```
  The crash address varies between runs (0x360…, then a different
  high-entropy 64-bit value on the next reproduce), which is the
  signature of a corrupted / uninitialized pointer being dereferenced,
  not a fixed layout bug. The handler's own `msg!("stake_tokens: …")`
  printed first, so user code returned `Ok(())`. The crash is in the
  Anchor-generated post-handler writeback (account serialize back to
  `data_mut()`).
- **Pre-existing fixes that are NOT this bug (closed):**
  - **Issuer-authority funding.** Earlier reproduce surfaced
    `custom program error: 0x1` (System program "insufficient
    lamports") on the `register_issuer` site. Root cause was that
    `scripts/bootstrap_issuer.ts` was funding the
    `issuerAuthority` keypair with only ~0.05 SOL while the
    `register_issuer` `init` allocation plus the staked-deposit CPI
    needed >1 SOL. Fixed in `scripts/bootstrap_issuer.ts:225-242`
    (now transfers `1.1 * LAMPORTS_PER_SOL`, with idempotent
    balance check at `1.05 SOL`). Verified green in the same
    transaction set that printed the SEC-048 receipts above. Listed
    here so this earlier underfund-failure isn't accidentally
    re-reported as B10.
- **Working hypotheses (under investigation, not yet validated):**
  1. **`init_if_needed` cascade on `governance_vault`.** `StakeTokens`
     marks both `staker_account` and `governance_vault` as
     `init_if_needed`. The vault's `token::authority = governance_vault`
     is a self-referential PDA-as-its-own-authority on first init.
     Anchor 0.30.1's `init_if_needed` writeback path for this shape
     has known stack-frame regressions in shared issue threads; on
     first-call (account doesn't exist yet) it allocates AND signs
     AND writes back in one ix, which is the most fragile path.
  2. **`Sysvar<'info, Rent>` deprecation interaction.** Anchor 0.30.x
     deprecated the `rent` Sysvar account; if the IDL still emits it
     but the runtime path no longer wires it through, a stale pointer
     could end up in the account-meta array.
  3. **Stack frame overflow on writeback.** Mirrors B4 (zk-verifier).
     `StakeTokens` carries 9 accounts; with `init_if_needed` on two
     of them, Anchor's generated writeback closure may exceed the
     4 KB stack frame on the BPF side. B4's fix (heap-spill via
     `Box<…>`) is the precedent.
  4. **`StakerAccount` SPACE arithmetic.** Declared as
     `space = 8 + 32 + 8 + 4 + 8 = 60`. Account itself has fields
     `voter: Pubkey, total_staked: u64, lock_until: i64, …`. Need
     to confirm the discriminator (8) + fields (≥48) + any Anchor-
     internal padding all fit. A 1-byte under-allocation would
     present exactly as a writeback-time access violation.
- **Why audit didn't catch this earlier:** `stake_tokens` was
  introduced in commit `0556191` ("security hardening: voter staking
  + governance vault") and never exercised by an integration test. The
  fact that B9 (SEC-048 bypass) is what unblocked the first ever
  on-chain run of `stake_tokens` is exactly why it surfaced now: this
  ix has been latent-broken since it was committed, and we only just
  reached it.
- **Verification gate:** `npm run bootstrap-issuer` step `[4/9]` lands
  with no validator-side `Access violation`, the staker PDA exists
  with `total_staked > 0`, and the governance vault holds the staked
  tokens. After this lands, steps `[5/9]` through `[9/9]` are
  expected to flow without further changes.
- **Mainnet posture:** mainnet today does not exercise this path
  (governance is gated behind a flag in the deploy plan), so B10 is
  not a live mainnet incident; it is a pure E2E blocker. But it is a
  P0 for any public devnet rollout because the failure class
  (uninitialized memory dereference) is the kind of bug that, if it
  were on a write path that landed and *then* corrupted, would be a
  consensus-visible event.
- **Tracking:**
  - This file (B10) is the running diagnosis log.
  - On root-cause + fix, mirror to `sec/SECURITY_REGISTRY.md` if the
    cause is memory-safety class (not just an Anchor pattern bug).
  - `plan/RESUME.md` open-blocker section.

---

## Cross-language / SDK / pipeline

### C1. TS SDK build with new WASM bundle

- **Status:** Open. Depends on B6.
- **Verification:** `(cd ts-sdk && npm ci && npm run build)` exits 0
  for all 6 packages (core, holder, issuer, verifier, light, sdk).

### C2. `tests/vectors/check_vectors.ts` workspace linkage

- **Status:** Open. Engineer reported "TS side needs the workspace
  linked (separate concern)" — i.e., the check script currently fails
  because it imports from `@solid-protocol/core` which has not been
  built yet.
- **Verification:** after C1, `cd tests/vectors && tsx check_vectors.ts`
  exits 0.
- **Hard CLAUDE.md invariant:** without TS-side check, B5 is half-
  proven. Both halves (Rust diff and TS check) must be observed green.

### C3. Localnet deploy + initialize + E2E

- **Status:** Open. Untested post-refactor.
- **Sequence (per `docs/DEPLOYMENT_AND_TESTING.md`):**
  ```
  pkill -f solana-test-validator 2>/dev/null; sleep 2
  rm -rf test-ledger
  export COPYFILE_DISABLE=1
  solana-test-validator --reset &
  sleep 5
  solana config set --url localhost
  solana airdrop 10
  anchor deploy --provider.cluster localnet
  npm run e2e
  ```
- **Expected exit:** `npm run e2e` tail prints `verified: true` AND
  `ok (replay rejected by nullifier PDA init constraint)`. Exit 0.
- **VK pin contract (SEC-041):** `circuits/build/verification_key.sha256`
  was regenerated this session to
  `debca4c083d0cd4091339367877b199e87708d0c3b7467b0c85115de2f74a15a`.
  `initialize.ts` accepts the in-tree pin file by default; no
  `SOLID_VK_SHA256` env override needed.

---

## Out-of-scope for the next localnet E2E (real, but separately tracked)

### O1. SOLID-SEC-045 (NEW-01) — atomic handlers don't update `IssuerTreeBinding.current_root` in same ix

- **Where tracked:** `sec/SECURITY_REGISTRY.md` SEC-045; `plan/RESUME.md`
  Section 2a; v0.6.1 audit Section 5.4.
- **E2E impact:** none for happy path; revocation-replay test
  (`tests/integration/07b_revoke_atomic_binding_update.test.ts`)
  doesn't exist yet, so the gap can't fail E2E either way.

### O2. SOLID-SEC-046 (NEW-02) — no CU-budget regression gate on `verify_batch_proof`

- **Where tracked:** `sec/SECURITY_REGISTRY.md` SEC-046; v0.6.1 audit
  Section 5.4.
- **E2E impact:** none; CI gate is the deliverable, not a runtime check.

### O3. SOLID-SEC-047 (NEW-03) — companion to B4 above

- **Where tracked:** to be added to `sec/SECURITY_REGISTRY.md`.
- **Code-side resolution:** B4 above. Closed at the program; the
  registry entry should record the diagnosis + frame-budget gate as
  P0 follow-up.

### O7. SOLID-SEC-048 — companion to B9 above

- **Where tracked:** `sec/SECURITY_REGISTRY.md` SEC-048 (HIGH); P0
  in `docs/IMPROVEMENTS_ROADMAP.md`; `plan/RESUME.md`;
  `docs/FORWARD_ROADMAP.md`.
- **Code-side resolution:** interim bypass via
  `sec007-skip-onchain` feature; off-chain SDK predicate is the
  load-bearing gate until a CU-affordable on-chain replacement
  ships. Mainnet deploy MUST NOT enable the feature.
- **E2E impact:** with the feature set on the localnet build,
  `bootstrap_issuer.ts` lands `register_issuer` within 400K CU and
  e2e proceeds. Without the feature set, `bootstrap_issuer.ts`
  cannot land the ix at all (>1.4M CU) and the entire pipeline is
  blocked at step 4 of 6.

### O4. `circuits/test/padding_slot.test.js` pre-existing failure

- **Status:** **Closed (verified) 2026-04-27.**  Was already
  resolved in code by the Phase 3.4 `BabyPbk254` introduction
  (replaces the old `circomlib::BabyPbk()` whose internal
  `Num2Bits(253)` rejected ~26 % of valid 254-bit Poseidon
  outputs).  The doc entry was stale -- the test had been passing
  since Phase 3.4 landed, but nobody verified after-the-fact.
  Reproducer this session: `cd circuits && PATH="$PWD/../.toolchain/bin:$PATH" npm test`
  -> 22/22 passing, including all three padding-slot cases
  (`enabled=0` arbitrary siblings, `enabled=1` garbage globalRoot,
  `enabled=2` bit-constraint).
- **E2E impact:** none.

### O5. `docs/system_architecture.md:1198` aspirational `SolidIssuer.deliverToHolder()`

- **Status:** doc-vs-code drift. The function does not exist in
  `ts-sdk/packages/issuer/src/index.ts`.
- **E2E impact:** none. The E2E pipeline uses a local `state.json`
  test transport (see SEC-020 mitigation), not any production
  delivery channel.
- **Where to track:** delivery-channel design ADR (next ADR slot
  after 0015). Cross-link the doc edit and the implementation in
  the same PR.

### O6. `docs/DEPLOYMENT_AND_TESTING.md` "every command works as written"

- **Status:** drift, partially closed by this session's fixes. The
  runbook will be accurate once B1-B4 land in commits AND the
  toolchain pin section reflects `rust-version = 1.75` +
  `.cargo/config.toml`. Not E2E-blocking, but a doc-truth issue.

---

## Sequenced runbook to clear remaining E2E blockers

(Assumes the working-tree state above; do NOT modify code.)

1. `head -3 Cargo.lock` -> `version = 3` (B2 sanity).
2. `rustup show active-toolchain` -> `1.79.0` (B2 sanity).
3. `ls target/deploy/*.so | wc -l` -> `3` (B3, B4 sanity).
4. `cargo run -p solid-core --example gen_vectors` (B5 step 1).
5. `git diff --exit-code tests/vectors/commitment_and_nullifier.json`
   (B5 step 2 — gate).
6. `(cd ts-sdk && npm ci && npm run build)` (C1).
7. `cd tests/vectors && tsx check_vectors.ts` (C2 — gate).
8. `rm -rf ts-sdk/packages/core/wasm && PATH="$PWD/.toolchain/bin:$PATH" wasm-pack build wasm/ --target nodejs --out-dir ts-sdk/packages/core/wasm --release` (B6 — if it fails, apply the three-way cfg).
9. `node scripts/wasm_bridge_smoke.mjs` (B6 — gate).
10. `pkill -f solana-test-validator; rm -rf test-ledger; export COPYFILE_DISABLE=1; solana-test-validator --reset &` (B8).
11. `sleep 5 && solana config set --url localhost && solana airdrop 10` (C3 prep).
12. **B9 / SEC-048 build step (localnet only):** rebuild
    issuer-registry with the bypass feature before deploying:
    `CARGO_TARGET_DIR=$PWD/target cargo build-sbf -p issuer-registry --features sec007-skip-onchain`.
    Skipping this leaves the ~1.4M-CU on-chain check in place and
    `bootstrap_issuer.ts` will fail at step 4 with
    `exceeded CUs meter`.  Mainnet builds MUST NOT pass this flag.
13. `anchor deploy --provider.cluster localnet` (C3 deploy).  If the
    feature-flagged build at step 12 ran, `solana program deploy
    target/deploy/issuer_registry.so --program-id keys/localnet/issuer_registry-keypair.json`
    is a sufficient subset.
14. **Validator must have account-compression + noop programs.** A
    bare `solana-test-validator --reset` does NOT load
    `cmtDvXumGCrqC1Age74AVPhSRVXJMd8PJS91L8KbNCK` (SPL Account
    Compression) or `noopb9bkMVfRPU8AsbpTUg8AQkHtKwMYZiFUjNRtMmV`
    (noop, used by compression for log-side effects). Without them,
    `scripts/backfill_issuer_tree.ts` aborts at
    `init_empty_merkle_tree` with `Attempt to load a program that
    does not exist`. Start with:
    ```
    solana-test-validator --reset \
      --clone-upgradeable-program cmtDvXumGCrqC1Age74AVPhSRVXJMd8PJS91L8KbNCK \
      --clone-upgradeable-program noopb9bkMVfRPU8AsbpTUg8AQkHtKwMYZiFUjNRtMmV \
      --url https://api.devnet.solana.com
    ```
    OR commit the BPF blobs and use `--bpf-program` (cheaper at
    runtime, larger repo footprint). The clone path is good enough
    for E2E and avoids vendoring third-party binaries.
15. `npm run init-onchain` (C3 prep). Creates a fresh governance
    SPL mint, initializes registry with `voting_period_seconds`
    sourced from `SOLID_VOTING_PERIOD_SECONDS` env (recommend 120s
    on localnet), registers schema, initializes both global and
    schema/tree bindings, initializes `zk_verifier` and stores the
    VK.
16. `npx tsx scripts/backfill_issuer_tree.ts`. Creates the SPL
    account-compression issuer tree, transfers authority to the
    `IssuerTreeBinding` PDA, and enrolls any already-approved
    issuers (zero on a fresh validator).
17. `SOLID_VOTING_PERIOD_SECONDS=120 npm run bootstrap-issuer`.
    Steps `[1/8]..[8/8]` + `[8b/10] append_issuer_leaf` all green
    as of 2026-04-26 (B9 receipts intact: CU=64,487 on
    `register_issuer`, SEC-048 `msg!` line, `Sec007Bypass` event;
    B10 closed: `stake_tokens` lands cleanly).
18. `npm run issue` then `npm run prove` (or the rolled-up `npm run
    e2e` if the validator state is still live).
19. Confirm tail: `verified: true` + `ok (replay rejected by
    nullifier PDA init constraint)` (C3 — gate).
20. Confirm program log on `register_issuer` contains exactly one
    `SEC-048: sec007-skip-onchain active; …` line — receipt that the
    bypass build is the one running, and a forward telemetry signal.

The first failure point names exactly which blocker is still live.
As of 2026-04-26 ~02:15 IST, all steps through 17 are green; the
live edge is now step 18 (`npm run issue` / `npm run prove`).

---

## Update protocol

- When a blocker moves Open -> Fixed (claimed), record the change
  shape and the specific verification gate that needs to fire.
- When the verification gate fires green, move to Fixed (verified)
  and mark the timestamp.
- When the doc has zero Open / Fixed (claimed) entries AND
  `docs/DEPLOYMENT_AND_TESTING.md` reflects the post-fix state,
  archive this file under `docs/archive/` with a `HISTORICAL`
  banner and remove the pointer from this file.

---

*End of tracker.*
