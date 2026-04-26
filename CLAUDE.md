# CLAUDE.md

Auto-loaded context for Claude Code sessions working in this repo.

## What this repo is

solid-protocol is private onchain identity infrastructure for Solana. It uses
Groth16 plus alt_bn128 syscalls for on-chain proof verification, BabyJubJub
EdDSA with Poseidon for issuance signatures, and SPL Account Compression for
credential trees. Three Anchor programs (zk-verifier, issuer-registry,
schema-registry) plus a TypeScript SDK plus Circom circuits.

Current release is v0.6.1 (April 2026, post-Phase-3-impl-4). The
canonical state-of-the-protocol document is
sec/audits/2026-04-25_v0.6.1_deep_comprehensive_audit.md. The
previous v0.6 audit is superseded but kept in place for history.
The older docs/POST_REMEDIATION_AUDIT.md is the Phase 1 post-fix
record and is flagged historical at the top of that file.

## Hard invariants

The following are load-bearing. Breaking any of them will silently compromise
soundness or availability:

- Anchor.toml, declare_id literals in programs/*/src/lib.rs, and
  deployments/*.json must all agree. CI enforces via
  scripts/check_program_ids.py.
- Canonical program IDs:
  - zk_verifier: DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb
  - issuer_registry: 5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx
  - schema_registry: 4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1
- Rust and TypeScript primitives must agree byte-for-byte. The
  cross_language_vectors CI job hard-gates this via tests/vectors/.
- The on-chain verifier must owner-check global_tree, schema_tree_N,
  AND issuer_tree_binding. global_tree + schema_tree_N are checked
  against schema_registry's program ID; issuer_tree_binding is
  checked against issuer_registry's program ID (ADR-0014). Removing
  any of these owner-checks re-opens the forged-trust-root attack.
- VerifierConfig layout post ADR-0015 (Phase 3 impl 2): adds
  vk_finalized (1), vk_generation (2), rotate_request_ts (8) on top
  of the Phase 2 baseline (next_vk_chunk 2, timestamp_skew_seconds
  4). SPACE = 60. Any change to VerifierConfig requires bumping the
  constant VerifierConfig::SPACE.
- circuits/batch_credential_query.circom has NR_PUBLIC_INPUTS = 32
  post ADR-0014 (was 31 pre-2026-04-24) with a fixed index scheme:
  issuerTreeRoot is at [10]; the later slots (queryCredentialIndices
  onward) shifted +1. zk-verifier's verify_batch_proof depends on
  this exact ordering; the constants ISSUER_TREE_ROOT_INPUT_INDEX,
  VERIFIER_ADDRESS_INPUT_INDEX, and CURRENT_TIMESTAMP_INPUT_INDEX
  in programs/zk-verifier/src/lib.rs are the handler's single
  source of truth for the shifted slots. See ADR-0012 for the
  full layout.
- Nullifier preimage is 6-input Poseidon (ADR-0006 revision): adds
  issuerTreeRoot so revoking any issuer invalidates every
  pre-revocation proof's nullifier universe (SOLID-SEC-008).

## Build sequence

Run from the repo root:

```
# Circuits (produces .wasm and .zkey for the current circuit)
cd circuits && npm install && node scripts/setup.js && cd ..

# WASM bridge for the TS SDK.
# The canonical bridge lives in the top-level `wasm/` crate, not in
# `crates/solid-core` (which stays BPF-compatible and has no `#[wasm_bindgen]`
# exports). SOLID-SEC-028 / ADR-0002.
wasm-pack build wasm/ --target nodejs \
    --out-dir ts-sdk/packages/core/wasm --release

# Anchor programs
# `target/deploy/` is gitignored, so on a fresh checkout the program
# keypairs must be hydrated from the tracked `keys/localnet/` copies
# before `anchor build` (otherwise Anchor silently regenerates fresh
# keypairs and the canonical program IDs drift). The script is
# idempotent.
bash scripts/sync_program_keypairs.sh
anchor build

# TS SDK workspace
cd ts-sdk && npm ci && npm run build && cd ..
```

End-to-end against localnet:

```
solana-test-validator --reset &
bash scripts/sync_program_keypairs.sh   # hydrate target/deploy from keys/localnet
anchor deploy --provider.cluster localnet
ts-node scripts/initialize.ts
ts-node scripts/issue.ts
ts-node scripts/prove.ts
```

Pre-commit gates to run locally:

```
cargo fmt --all -- --check
cargo clippy -p solid-core -p solid-light -- -D warnings
cargo test -p solid-core -p solid-light
cargo test -p zk-verifier --lib
python3 scripts/check_program_ids.py
```

## Toolchain pins

Outside Nix you must match these exactly. flake.nix and scripts/bootstrap.sh
do this for you inside Nix.

- rust 1.79.0 with wasm32-unknown-unknown target
- solana-cli 1.18.22
- anchor-cli 0.30.1
- circom 2.1.9
- snarkjs 0.7.5
- wasm-pack 0.13.1
- node 18 with npm 10+

The rust pin is enforced two ways now:

- Top-level rust-toolchain.toml. Cargo and rust-analyzer read this and
  resolve to 1.79.0 inside the project root. Do not delete it; the
  hidden `rustup override` it superseded was invisible to IDEs and
  caused recurring proc-macro ABI mismatches (`expected: rustc 1.95.0,
  got: rustc 1.79.0`) when contributors had a newer default toolchain.
- .vscode/settings.json. Tells rust-analyzer to set
  `RUSTUP_TOOLCHAIN=1.79.0` for both its cargo and proc-macro server,
  and to write its own check artefacts to `target/rust-analyzer/` so
  it does not race anchor build for `target/debug/deps/serde_derive.dylib`.
  The file is kept under version control (gitignore carves out
  `!.vscode/settings.json`) because the wiring is project-wide, not
  personal.

Recent cargo versions (1.95+) work for host tests but anchor build still
wants the pinned rust-toolchain.

## Doc conventions

- Do not use emoji or unicode box-drawing characters in any doc under docs/
  or in the root README. Plain ASCII markdown only.
- Use file:line references when pointing to code.
- sec/audits/2026-04-25_v0.6.1_deep_comprehensive_audit.md is the canonical
  post-fix audit. The 2026-04-24 v0.6 audit is superseded; older
  docs/POST_REMEDIATION_AUDIT.md is flagged historical (Phase 1 era); prior
  audit docs under docs/SOLID_*.md are also historical.
- Update docs/IMPROVEMENTS_ROADMAP.md when closing or adding a P0 / P1 / P2
  item, so CI and audits share the same backlog.

## Open work (Phase 3 scope)

Phase 1 (remediation set) and Phase 2 (ADR-0014 compressed issuer tree
plus 6-input nullifier) are closed. Phase 3 is open and drives the
external-audit-ready close-out.

- SOLID-SEC-006 Part 2 VK generation in public-input contract. Part
  1 landed 2026-04-25 (ADR-0015; on-chain freeze-gate + 48h rotation
  timelock). Part 2 binds vk_generation into the circuit's public
  inputs so cross-VK replay is impossible; it requires a circuit
  change and therefore batches with the next trusted-setup cycle.
- SOLID-SEC-045 (MEDIUM). Atomic handlers (revoke_issuer_atomic,
  request_withdrawal_atomic) do not update
  IssuerTreeBinding.current_root in the same ix. Pre-transition
  proofs remain replayable until update_issuer_tree_root is called
  separately. Fix: write the new root directly into the binding
  PDA inside the atomic ix. Surfaced in the v0.6.1 audit NEW-01.
- SOLID-SEC-046 (MEDIUM). No CU-budget regression gate on
  verify_batch_proof. Future circuit changes can push the ix over
  Solana's per-tx CU ceiling without failing CI. Fix: add a CI job
  that measures CU against a baseline tracked in docs/CU_BUDGET.md.
  Surfaced in the v0.6.1 audit NEW-02.
- SOLID-SEC-010 cross-language vectors (HIGH). Extend gen_vectors.rs and
  check_vectors.ts from 3/10 to 10/10 primitives.
- SOLID-SEC-041 content-addressed VK artifact. Commit
  circuits/build/verification_key.sha256; fail initialize.ts on mismatch.
- SOLID-SEC-043 issuer_tree_operator single signer (MEDIUM). Gate
  behind Squads 3-of-5 or DAO threshold PDA before external audit.
- SOLID-SEC-044 Cooldown does not replace issuer leaf (LOW). Add
  request_withdrawal_atomic mirroring revoke_issuer_atomic.
- Integration test suite 02..11. Ten scenarios specified in
  tests/integration/README.md, none implemented.
- Revocation v1 operator workflow. Circuit and on-chain support are in
  place. Still needs holder SDK helper, issuer SDK helper, and indexer
  event contract. See docs/REVOCATION_DESIGN.md.
- SOLID-SEC-012 multi-party trusted-setup ceremony to replace
  circuits/scripts/setup.js. Mainnet blocker.
- External audit of the post-Phase-2 codebase.

## Debugging discipline (learnings)

These are not stylistic preferences; they are rules earned by paying
for the same mistake more than once. Full origin stories and
concrete examples live in plan/RESUME.md "Learnings (running log)".
Read that section before starting a new debugging arc.

- L1. Take a step back before tinkering. If a fix bounces ("change
  X, then Y breaks, then Z breaks"), stop editing. The cascade is
  the diagnostic: the change is operating below the level of the
  actual defect. Find the one definition whose wrongness produces
  the cascade and fix that.
- L2. Programs and circuits are the source of truth; everything
  else is a consumer. Hierarchy: programs/circuits > crates
  (solid-core, solid-light) > WASM bridge + TS SDK > scripts. When
  a script fights a contract, the script is wrong OR the contract
  is wrong. Never "the script needs to be smarter about working
  around the contract."
- L3. If the contract is wrong, fix the contract. No workarounds,
  no regressions. Encode invariants in the type system (Anchor
  Account<T>, Account<Mint>, Signer, typed PDAs); free Pubkey or
  [u8; 32] params are a smell. Atomicity beats convention -- birth
  co-dependent accounts in the same accounts struct (#[account(
  init, ...)]), not in two ix joined by a doc comment. Stale
  on-chain state from a pre-fix build is NOT a reason to relax the
  new contract; clean-validator-restart is.
- L4. Add unit + integration tests + targeted msg! logs as part of
  the fix, not after. The same commit that fixes the defect lands
  the regression gate that stops it re-landing. A fix without a
  regression gate has not shipped. Note the gate in
  docs/E2E_BLOCKERS.md (or the relevant blocker doc) under
  "Regression gate".
- L5. Runtime address entropy is signal. "Access violation at
  0xXXXX" with the address varying across runs = uninitialised /
  corrupted pointer = fix upstream of the dereference (init,
  writeback, lifetime). Stable address = fixed layout bug = read
  the linker map.
- L6. Doc lies are tomorrow's bugs. "Caller MUST do X" without
  enforcement is one careless integration away from being
  enforcement-by-nothing. Promote convention to type or guard.

## Working-style notes

- Large asks are welcome; prefer dense, thorough output over abridged
  summaries.
- When an earlier document or assumption is wrong, correct it directly and
  explicitly. Do not paper over contradictions.
- Autonomous multi-step work is fine; report progress at natural
  checkpoints (per milestone, per program, per doc cluster).
