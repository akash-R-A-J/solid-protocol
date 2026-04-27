# Resume -- where to pick up next session

Living handoff doc. Read this first when starting a new session.
Updated at the end of each session; the last-updated line is
authoritative.

- **Last updated:** 2026-04-28 late session (circuit/ZK audit +
  e2e bring-up + SEC-053 close-out).  Major code session;
  see "2026-04-28 -- circuit/ZK audit + e2e bring-up" below for the
  full receipts.  Quick state:
  - **HEAD:** `ac84deb SEC-053: EdDSA-Poseidon cofactor-8 fix; prove
    now generates a valid Groth16 proof` (committed; will be pushed
    at end-of-session).  Pushed branch: `main`.
  - **E2E status:** `npm run e2e` walks through `init-onchain ->
    backfill-issuer-tree -> bootstrap-schema-tree ->
    bootstrap-issuer -> issue -> prove (Groth16 proof generated
    in 5.09s)` ALL GREEN through proof generation.  Live edge is
    the on-chain submission half of `npm run prove`: **B13**
    (`verify_batch_proof` ix data 1324 bytes exceeds Solana's
    1232-byte legacy-tx wire size).  See `docs/E2E_BLOCKERS.md`
    B13 for the architectural-blocker remediation analysis.
    Recommended path is documented in `plan/IMPLEMENTATION_PLAN.md`
    Appendix D.
  - **Closures this session:**
    - SOLID-SEC-045 (atomic binding update; helper +
      keccak path-recompute + 7 unit tests + write-binding
      regression suite).
    - SOLID-SEC-049 NEW (wrong `replace_leaf` discriminator;
      `0xe388...` -> `0xcca5...`).
    - SOLID-SEC-050 NEW (schema-canonicality bypass via
      interleaved padding; circuit constraint + 17 witness-
      tester regressions; trusted-setup re-run; new VK pin
      `8385b82b032f65e505c784b28486ca8bec7da3f3d4b97b82724e697734565146`).
    - SOLID-SEC-052 NEW partial (BPF / cross-layer coord-form
      drift; `is_on_curve` rewritten + WASM bridge rebuilt;
      process gate at `docs/E2E_BLOCKERS.md` B11).
    - SOLID-SEC-053 NEW (EdDSA-Poseidon cofactor-8 mismatch
      between off-chain `sign` and circomlib's in-circuit
      verifier; `S = r + h * 8 * sk`; regression gate
      `babyjubjub::tests::sec_053_eddsa_cofactor_8_round_trip`).
    - `docs/E2E_BLOCKERS.md` O4 (padding_slot test) verified
      green and closed.
    - `docs/E2E_BLOCKERS.md` B12 (EdDSA witness drift) closed
      via SEC-053.
    - SOLID-SEC-041 confirmed in-code (VK sha256 pin
      gated `initialize.ts`).
  - **Open trackers introduced this session:** SOLID-SEC-051
    (all-padding circuit; LOW; defer to next setup cycle), B11
    (WASM rebuild gate; documented + process gate added), B13
    (legacy-tx wire size; live e2e edge -- promotes to
    SOLID-SEC-054 once remediation choice sanctioned).
  - **Workspace tests:** 169/169 cargo + 39/39 circuit witness-
    tester (mocha) green; +1 host-side ignored forensic snapshot.

## Pickup tomorrow (2026-04-29 morning)

1. Read `plan/IMPLEMENTATION_PLAN.md` Appendix D ("Next session
   pickup").  That section has the full B13 remediation steps.
2. Implement on-chain reconstruction of the 12 redundant public
   inputs in `programs/zk-verifier/src/lib.rs::verify_batch_proof`.
3. Update the SDK encoder
   `ts-sdk/packages/verifier/src/index.ts::buildVerifyBatchProofIx`
   to send only the 20-input subset.
4. Promote to **SOLID-SEC-054** in `sec/SECURITY_REGISTRY.md`.
5. Run e2e end-to-end; expect `verified: true` tail.
6. Then SOLID-SEC-046 CU gate; SOLID-SEC-010 vectors 3->10;
   integration suite 02..11.

(The 2026-04-27 strategy-session notes below are preserved as
historical context; they were unchanged technically from the
Phase 3.4 checkpoint at the time, and the code state has now
moved well past that point.)

- **Strategy-session snapshot (2026-04-27 evening):** unchanged
  technical state; three planning deliverables added.
  1. **`plan/GO_TO_MARKET.md`** (new, 335 lines).  Six-month
     sequenced go-to-market plan starting **2026-05-15**.  Phases
     A--E (discovery -> integrator+issuer pair -> ship the
     integration not the protocol -> launch -> replicate).  Three
     decision branches if the path doesn't open (continue as
     protocol play / narrow to vertical product / partner with SAS
     or publish-and-join).  Section 1 explicitly reconciles the
     Solana Foundation Superteam Build "Private Onchain Identity"
     listing: meaningful ecosystem signal, NOT customer demand
     evidence.  Strongest commercial wedge identified is RWA /
     accredited-investor gating on Solana (Ondo, MiCA, no Solana
     ERC-3643 equivalent yet).  ZK is currently losing the
     equivalent EVM market to ERC-3643 on regulator-legibility
     grounds; SolID has to beat that, not just beat SAS.
  2. **Competitive scan verified** (logged in chat, source list
     pinned in `plan/GO_TO_MARKET.md` §2): SAS (Foundation, May
     2025, deliberately non-ZK; the Foundation chose attestations
     over ZK credentials), Civic Pass (2M users, $500M+ TVL
     secured), VeryAI ($10M Polychain, biometric PoP not
     credentials), Solana.ID (reputation + SAS, not Privado-style),
     zkid.digital (warrants direct investigation).  No official
     Privado/iden3 Solana port despite syscall-readiness.  EVM-side
     reality bound: Privado has 4M credentials and < $5M revenue;
     World ID won by abandoning the credential framing for AI-era
     PoP.  First-mover claim mostly true but mostly irrelevant if
     the category hasn't proven product-market fit on Solana.  Zero
     public statements from major Solana DeFi protocols (Drift,
     Jupiter, Kamino, MarginFi, Phoenix) requesting ZK identity.
  3. **Tomorrow's pickup is now a two-track decision** rather than
     a single linear continue.  Both tracks are listed under "Next
     session: start here." below; pick the one that matches the
     state you wake up in (rested + customer-mode, or rested +
     code-mode).
  Working tree at session close mirrors this morning's state
  exactly -- the strategy-session edits touched `plan/` and `docs/`
  only and added no new modifications to `circuits/`, `crates/`,
  `programs/`, `wasm/`, or `ts-sdk/`.  Uncommitted Phase 3.4
  technical work is intact (see Previous update).
- **Previous update:** 2026-04-27 morning (Phase 3.4 crypto + IDE
  stability).
  `crates/solid-core/src/babyjubjub.rs` no longer uses `ark_ff::MontFp!`
  for pinned `Fq` constants (`sqrt(168700)`, its inverse, and
  arkworks-form Base8); the same wire values are loaded via
  `Fq::from_le_bytes_mod_order` over `const [u8;32]` so rust-analyzer
  stops panicking on `proc-macro panicked: could not parse` while
  `cargo test -p solid-core --lib babyjubjub` stays green.  Circuit
  regression suite (`circuits/test/*.test.js` against
  `gen_circuit_vectors` output) and CI wiring are documented in
  `docs/CURRENT_STATE.md` §5.2--5.3 and `CLAUDE.md` (toolchain note).
  See also `plan/IMPLEMENTATION_PLAN.md` revision line.  Working
  tree at this checkpoint includes (uncommitted): `circuits/lib/
  lt_bn254.circom` (new), `circuits/test/babypbk254.test.js`,
  `circuits/test/identity_anchor_derivation.test.js`,
  `circuits/test/lt_bn254.test.js`, isolated test templates under
  `circuits/test/templates/`, `crates/solid-core/examples/
  gen_circuit_vectors.rs` (new), modifications to
  `circuits/batch_credential_query.circom`, `circuits/lib/
  identity_anchor.circom`, `circuits/lib/predicate_evaluator.circom`,
  `crates/solid-core/src/babyjubjub.rs` (the MontFp! swap above),
  `scripts/initialize.ts`, `scripts/issue.ts`, `scripts/prove.ts`,
  `ts-sdk/packages/core/src/index.ts`,
  `ts-sdk/packages/holder/src/index.ts`,
  `ts-sdk/packages/issuer/src/index.ts`,
  `.github/workflows/ci.yml`, `package.json`, `circuits/
  package.json`, `circuits/package-lock.json`, plus new
  `docs/CURRENT_STATE.md`, `docs/SYSTEM_VISION.md`, `scripts/
  bootstrap_schema_tree.ts`.  None of this is on the issue/prove
  hot path; it's the SEC-010 cross-language-vector expansion + IDE
  stability layer.  Commit before pushing.
- **Previous update:** 2026-04-26 ~02:20 IST (E2E push, B10 closed by
  contract redesign).  Latest event: `npm run bootstrap-issuer`
  walks all 8 contract steps + `[8b/10] append_issuer_leaf` cleanly
  on a fresh localnet validator.  B10 (`stake_tokens` access
  violation) is closed by **redesigning `initialize_registry`**, NOT
  by relaxing the failing handler.  The original instinct was
  right -- "drop `init_if_needed` on `governance_vault`, pre-create
  the vault" -- but draft-1 (a separate `init_governance_vault` ix)
  hit a guard failure (`Unauthorized`) because the on-chain
  `RegistryConfig.governance_token_mint` was `Pubkey::default()`.
  Root cause: `initialize_registry` accepted `governance_token_mint:
  Pubkey` as a free parameter with no validation, so any caller
  could write `Pubkey::default()` into the registry and poison every
  downstream guard checking `vault.mint ==
  registry.governance_token_mint`.  That was the actual contract
  defect.  Fix shape:
  (i) `initialize_registry` now takes a typed `Account<'info, Mint>`
  (Anchor's deserializer rejects anything not owned by the SPL
  Token program -- closes the `Pubkey::default()` write path at
  the program boundary);
  (ii) `governance_vault` is born **atomically with the registry**
  inside `InitializeRegistry`'s accounts struct via
  `#[account(init, token::mint = governance_mint, token::authority
  = governance_vault, seeds = [b"governance-vault",
  registry_config.key().as_ref()], bump)]` -- by the time any caller
  reaches `stake_tokens`, the vault is already created, owned by
  itself, and bound to the registry's mint by construction;
  (iii) `StakeTokens` no longer carries `init_if_needed` on the
  vault (it's `mut`-only with a `constraint` asserting the mint
  match);
  (iv) `init_governance_vault` (and its accounts struct) deleted --
  the right API is "the vault is a registry invariant, not a setup
  step";
  (v) new error codes `InvalidThreshold` and `InvalidVotingPeriod`
  plus `require!` guards (threshold ≤ 10000 bps, voting_period > 0)
  on the same ix, closing a parallel "free integers" defect surface.
  Also in this change set:
  `target/idl/issuer_registry.json` regenerated; `scripts/build_idls.mjs`
  learned to denamespace IDL entries so Anchor's TS client resolves
  `program.account.registryConfig` rather than
  `program.account['issuer_registry::RegistryConfig']`;
  `scripts/initialize.ts` rewritten to drop `Pubkey.default` fallback
  and source `voting_period` from `SOLID_VOTING_PERIOD_SECONDS` (was
  hardcoded `86400`, conflicting with `bootstrap_issuer.ts`'s `20`
  default and producing a `VotingPeriodNotEnded` failure at
  `finalize_voting`); `scripts/bootstrap_issuer.ts` updated (no
  more `[3b/9]`, steps renumbered `[1/8]..[8/8]`, `governanceVaultPda`
  persisted to E2E state); `tests/integration/01_registry_init.test.ts`
  rewritten on top of `solana-bankrun` with helpers that seed real
  SPL Mint accounts and three new test cases pinning the contract's
  regression surface (`rejects approval_threshold > 10000 bps`,
  `rejects voting_period == 0`, `rejects a non-Mint account passed
  as governance_mint`); `LocalReplicaAdapter` (in
  `ts-sdk/packages/light/src/index.ts`) gained a `getRoot()` method
  required by `bootstrap_issuer.ts:[8b/10]`.  Validator-side
  observation: a vanilla `solana-test-validator --reset` does NOT
  load the SPL Account Compression program
  (`cmtDvXumGCrqC1Age74AVPhSRVXJMd8PJS91L8KbNCK`) or the noop
  program (`noopb9bkMVfRPU8AsbpTUg8AQkHtKwMYZiFUjNRtMmV`) that
  account compression uses for log side-effects; both must be
  cloned via `--clone-upgradeable-program` from devnet for
  `backfill_issuer_tree.ts` to land.  Calibration note (env, not
  code): `SOLID_VOTING_PERIOD_SECONDS=20` is too short on localnet
  because the flash-loan cool-off is 100 slots ≈ 40s, which can
  elapse the voting window before `vote_on_issuer` fires; recommend
  120s.  Toolchain-IDE side-resolution from earlier in the same
  session: `rust-toolchain.toml` (pin channel 1.79.0 + wasm32
  target), `.vscode/settings.json` (rust-analyzer uses the same
  toolchain via `RUSTUP_TOOLCHAIN=1.79.0` and a separate
  `target/rust-analyzer` build dir to avoid contention with
  `cargo build-sbf`); `serde_derive` proc-macro ABI-mismatch is
  gone.  Documented in `CLAUDE.md` "Toolchain pins".  B9 receipts
  intact (`register_issuer` at CU=64,487, SEC-048 `msg!` line +
  `Sec007Bypass` event both emitted) -- the bypass plumbing is
  unchanged.  SEC-007 is still live as SEC-048 until one of the
  three real-fix candidates ships.
- **Previous update:** 2026-04-26 ~01:30 IST (E2E push, B9 receipts
  in, B10 surfaced).  With the SEC-048 bypass build deployed,
  `bootstrap-issuer` cleared steps 1-3 cleanly; step 4
  (`stake_tokens`) hit `Access violation in unknown section`
  post-handler.  See "Latest event" above for the resolution.
- **Previous update:** 2026-04-25 late-session (E2E unblock).  B6 + B7
  closed (`wasm-pack build wasm/` against the new `solana-program`
  regular dep is green; external IDL generation via
  `scripts/build_idls.mjs` works around the anchor 0.30.1 vs
  proc-macro2 >= 1.0.95 incompat).  `npm run e2e` walked through
  `initialize` (registry config, schema register, VK store) and
  `backfill-issuer-tree` (SPL AC tree + transfer-authority
  workaround for the `AccountNotSigner` quirk + correct
  `init_empty_merkle_tree` discriminator).  **`bootstrap-issuer`
  surfaced a structural BPF compute-unit blocker on the BJJ prime-
  order subgroup check inside `register_issuer`** -- the host-side
  SEC-007 helper `r * P == O` exceeds the 1.4M CU per-tx ceiling on
  BPF.  No build flag, inline setting, or compute-budget override
  brings it under the cap.  **Resolution shape (interim).**  Off-
  chain TS predicate `isInPrimeOrderSubgroup` becomes the load-
  bearing gate; on-chain check is gated behind a new
  `sec007-skip-onchain` Cargo feature on issuer-registry; bypass
  arm runs the cheap `is_on_curve + !is_identity` consolation gate,
  emits `msg!("SEC-048: sec007-skip-onchain active; …")`, and
  emits a `Sec007Bypass { issuer_authority, slot }` event for
  operator telemetry.  Localnet/devnet only; mainnet builds MUST
  NOT enable the feature.  Registered as **SOLID-SEC-048 (HIGH;
  Open with interim bypass live; mainnet deploy-blocker)** in the
  registry and as **B9** in `docs/E2E_BLOCKERS.md`; **P0-7** in the
  improvements roadmap.  This MUST stay top-of-mind during every
  subsequent piece of work -- closing it requires either
  cofactor-clear in the SDK + circuit-level binding, moving the
  subgroup gate into the issuance circuit (batches with SEC-006
  Part 2 trusted-setup cycle), or a `sol_babyjubjub_*` syscall.
  Earlier in the day: build-pipeline restoration (circuit compile
  fixes, dep-cascade resolution via `rust-version = "1.75"` +
  `.cargo/config.toml` MSRV resolver, Poseidon BPF refactor through
  `sol_poseidon` syscall, `verify_batch_proof` BPF stack-frame fix;
  SEC-047 (Fixed) registered).  Earliest in the day: Phase 3 impl
  1-4 landed (SEC-007, SEC-006 Part 1, SEC-041, SEC-044 all
  closed); v0.6.1 deep audit dropped in, registering SEC-045 +
  SEC-046 as MEDIUM; E2E runbook `docs/DEPLOYMENT_AND_TESTING.md`
  rewritten as the canonical install / build / deploy / run /
  verify / probe doc.
- **Next session: start here.**  Two-track decision.  Pick ONE
  track per session; do not interleave.

  **TRACK A -- code mode (recommended if rested and the head is
  in technical detail):**
  Live edge is the holder-side issuance flow plus committing the
  Phase 3.4 working tree.  Sequence:
  0. **Commit the Phase 3.4 working tree first.**  It's been
     uncommitted for a session and a half.  Split into clean
     logical commits (suggested ordering):
       - `crypto: replace MontFp! with from_le_bytes_mod_order in
          babyjubjub.rs (rust-analyzer proc-macro stability)`
       - `circuits: add lt_bn254 + babypbk254 + identity_anchor
          isolated templates and witness tests`
       - `examples: gen_circuit_vectors.rs for SEC-010 expansion`
       - `circuits: identity_anchor.circom + predicate_evaluator
          adjustments`
       - `scripts: bootstrap_schema_tree.ts + initialize/issue/
          prove updates for the new schema-tree path`
       - `sdk: core + holder + issuer surface adjustments`
       - `ci: <whatever .github/workflows/ci.yml gained>`
       - `docs: CURRENT_STATE.md + SYSTEM_VISION.md +
          plan/GO_TO_MARKET.md`
     Run `cargo test -p solid-core -p solid-light` and `cargo
     test -p zk-verifier --lib` between commits as a sanity gate.
     Do NOT push yet -- E2E live edge below should land first.
  1. `npm run issue` end-to-end (todo `e1b`).  Needs an
     issuer-bound BJJ keypair (the one `bootstrap-issuer`
     registered) and produces a Poseidon-bound credential
     delivered to a holder.  Watch for `deliverToHolder`
     doc-vs-code drift (O5) -- credential-delivery design ADR is
     still open; if `issue.ts` diverges from
     `SolidIssuer.deliverToHolder`'s contract, fix the contract
     first (the script is the consumer, not the source of truth
     -- same discipline that closed B10).
  2. `npm run prove` end-to-end (todo `e1c`).  Two gates: (a)
     Groth16 verify on-chain returns `verified: true`; (b) replay
     attempt rejects with `account already in use` from the
     nullifier PDA's `init` constraint.  If (a) fails: check
     `verification_key.sha256` pin matches what `initialize.ts`
     uploaded, and `NR_PUBLIC_INPUTS = 32` ordering matches
     between the prover and `verify_batch_proof`.  If (b) fails:
     SEC-008 6-input nullifier soundness regression -- stop and
     diagnose; do not relax the constraint.
  3. After both green: wire `scripts/wasm_bridge_smoke.mjs` (todo
     `e2`), `tsx tests/vectors/check_vectors.ts` (todo `e3`), and
     IDL-snapshot diff (todo `e4`) as CI regression gates.
  Validator prerequisites: `--clone-upgradeable-program
  cmtDvXumGCrqC1Age74AVPhSRVXJMd8PJS91L8KbNCK
  --clone-upgradeable-program noopb9bkMVfRPU8AsbpTUg8AQkHtKwMYZiFUjNRtMmV`
  (SPL AC + noop, not loaded by `--reset` alone), and
  `SOLID_VOTING_PERIOD_SECONDS=120`.  See `docs/E2E_BLOCKERS.md`
  runbook for the full ordering.

  **TRACK B -- customer mode (recommended if the question
  "should I keep building this" is still active):**
  Phase A discovery per `plan/GO_TO_MARKET.md` Section 6.
  Discovery formally starts 2026-05-15, but prep can start
  immediately:
  1. Build the target list (10 names).  Concrete starters from
     the GTM doc: Ondo on Solana, Maple Finance, Drift
     institutional, Kamino restricted-jurisdiction features,
     Phoenix compliance forks, two Colosseum Frontier 2026 RWA
     teams, one Solana-native KYC-as-a-service vendor.  Add 2--3
     of your own based on who you can warm-introduce to.
  2. Draft the 1-page commercial pitch (NOT the technical deep
     dive in `docs/private_onchain_identity_deep_dive.md`).
     Audience is a product / compliance person at a regulated
     Solana protocol; not a cryptographer.  The question being
     answered is "what does SolID let your dApp do that SAS
     doesn't, and is it worth 3-6 engineering months to find
     out".
  3. Draft the outreach message.  Keep it under 200 words.  Does
     NOT pitch the protocol.  Asks the framing question from
     GTM Section 6 Phase A: "What would have to be true for you
     to gate this specific feature with private credentials
     instead of IP-blocking or off-chain KYC?"  If they can't
     answer that concretely, the answer is no, and you've
     learned something important.
  4. Schedule the first 5 conversations to land before
     2026-05-15.  Two of them this week if possible.

  Track A and Track B compound: Track A buys you the credibility
  to have Track B conversations ("we're shipping E2E this week"
  is a different opening than "we're still debugging the prover").
  But Track B answers the question Track A cannot answer
  ("should we ship more").  Pick whichever matches the energy of
  the morning; neither is wrong.
- **Current branch:** `main`
- **Current phase:** Phase 2 closed; Phase 3 open
- **Working-tree state at this checkpoint:** modifications staged
  but **not yet committed** in `circuits/`, `crates/solid-core/`,
  `programs/zk-verifier/src/lib.rs`, `programs/schema-registry/
  Cargo.toml`, `Cargo.toml`, `Cargo.lock`, `.cargo/` (new),
  `circuits/build/` + `circuits/trusted_setup/` (untracked
  artifacts), `docs/E2E_BLOCKERS.md` (new).  See `git status` for
  the full list.
- **Discipline in force:** root-cause only, no regressions, no doc
  lies, one source of truth per artifact (see
  `plan/IMPLEMENTATION_PLAN.md` Section 0).

---

## Where we left off

**Phase 2 is closed.**  Both load-bearing soundness items (SEC-004
+ SEC-008) shipped end-to-end: on-chain atomic status transitions,
circuit rev with compressed issuer tree + 6-input nullifier, SDK
migration across every package, backfill script, E2E pipeline
green through `npm run e2e`.  Snapshot:
`sec/audits/2026-04-24_v0.6_phase2_closeout.md`.

**Build pipeline restored (2026-04-25 late-session).**  A clean
checkout against the pinned toolchain (Rust 1.79 + Solana 1.18.x +
platform-tools rustc 1.75) had drifted into a broken state since
the `167138e` commit.  This session traced and closed every layer
of that drift; the BPF link path is now green.  Detail:

1. **Circuit compile errors at HEAD (3 distinct)** — fixed in
   working tree, **uncommitted**.  Duplicate `IsZero` template in
   `circuits/lib/credential_atom.circom:83` collided with
   circomlib's `comparators.circom`; an inline `signal nextNotZero`
   inside a `for` body in `circuits/batch_credential_query.circom:
   ~203` violated circom 2.1.x scope rules; sums of N>=2 quadratic
   products in the OR-mux + AND/OR final selector violated R1CS
   single-multiply form.  After the fix, `npm run compile`
   produces the R1CS cleanly with **86,616 non-linear
   constraints** (the in-tree `circuits/scripts/setup.js:34`
   "~60K" comment is stale by ~50%).
2. **Trusted setup re-run.**  New artefacts in `circuits/build/`
   (untracked):
   - VK sha256:
     `debca4c083d0cd4091339367877b199e87708d0c3b7467b0c85115de2f74a15a`
   - zkey sha256:
     `989498c7288dade1aae0e2f27b04253642f4cd188ae714ae0469ec91ae9ed402`
   - SOLID-SEC-041 pin file `verification_key.sha256` regenerated.
3. **Cargo.lock edition2024 cascade fix.**  Cargo's pre-1.84
   resolver picked transitives whose own MSRVs exceeded the
   platform-tools rustc 1.75 (e.g. `block-buffer 0.12.0`,
   `base64ct 1.8.x`, `toml_parser 1.1.2+spec-1.1.0`,
   `wit-bindgen 0.57.x`, `indexmap 2.14`, `borsh 1.6.x`,
   `unicode-segmentation 1.13.x`, `blake3 1.8.x`).  Fix:
   - `Cargo.toml` workspace declares `rust-version = "1.75"`.
   - `.cargo/config.toml` (new) opts into the cargo 1.84+
     MSRV-aware resolver via
     `[resolver] incompatible-rust-versions = "fallback"`.
   - `Cargo.lock` regenerated lockfile-v3 with targeted
     `cargo +1.79.0 update --precise` for the few transitives
     whose own crate manifests don't declare `rust-version`:
     `base64ct@1.6.0`, `blake3@1.5.5`, `borsh@1.5.7`,
     `proc-macro-crate@3.3.0`, `wasip2@1.0.0`, `indexmap@2.7.1`,
     `unicode-segmentation@1.12.0`.  These pins persist in the
     lockfile; `.cargo/config.toml` keeps them stable on
     subsequent `cargo update` runs.
4. **Poseidon BPF stack overflow (`light-poseidon 0.2.0`)** —
   light-poseidon's `bn254_x5::get_poseidon_parameters`
   stack-allocated round-constants table overflowed BPF's 4 KB
   per-frame ceiling under `lto = "fat"`.  Fix:
   - `crates/solid-core/Cargo.toml` — `light-poseidon` moved to
     `[target.'cfg(not(target_os = "solana"))'.dependencies]`;
     `solana-program` added as a regular workspace dep.
   - `crates/solid-core/src/poseidon.rs` — `hash_bytes` and
     `hash_fields_to_bytes` are now dual-target and dispatch to
     `solana_program::poseidon::hashv` (the `sol_poseidon`
     syscall on BPF, byte-identical light-poseidon fallback on
     host).  Pre-canonicalisation via `Fr::from_le_bytes_mod_order`
     applied uniformly so the syscall's strict mod-p input check
     matches the historical silent-reduce semantic.  Two new
     regression tests pin this contract:
     `test_byte_path_matches_field_path` and
     `test_hash_bytes_canonicalizes_oversized_inputs`.
   - `crates/solid-core/src/babyjubjub.rs` — `generate_keypair`,
     `sign`, `verify` (which depend on host-only `OsRng` and the
     host-only `hash_fr` path) gated to host-only.
     `is_in_prime_order_subgroup` and
     `require_in_prime_order_subgroup` (SOLID-SEC-007 protection)
     intentionally NOT gated, kept dual-target.
   - `programs/schema-registry/Cargo.toml` — dropped unused
     `light-poseidon = "0.2"` direct dep.
5. **`verify_batch_proof` BPF stack-frame overflow (456 B)** —
   surfaced once (4) was lifted; pre-existing since Phase 2 impl 2
   (`73871dc`, when `NR_PUBLIC_INPUTS` grew 31 -> 32).  The Anchor
   `__global::verify_batch_proof` wrapper's frame held the
   1312-byte deserialized `__ix` struct + the BPF outgoing-arg
   slots for the user fn call (~1312 B) + the `VerifyBatchProof`
   accounts struct (~700-900 B) + ctx/bumps + alignment, totalling
   ~4552 B against the 4 KB BPF per-frame ceiling.  LTO setting
   was tested orthogonal — the size is structural, not optimiser-
   driven.  Fix in `programs/zk-verifier/src/lib.rs:360-380,
   717-723`:
   - ix arg signature changed
     `public_inputs: [[u8; 32]; NR_PUBLIC_INPUTS]` ->
     `public_inputs: Vec<[u8; 32]>` (24-byte stack descriptor;
     1024-byte payload on the BPF heap allocator).
   - Explicit `require!(public_inputs.len() == NR_PUBLIC_INPUTS)`
     length validation immediately on entry.
   - Inner Groth16-verify body extracted into a separate
     `#[inline(never)]` helper that takes `&[[u8; 32]; N]` by
     reference, splitting the work across two BPF frames so
     neither hits 4 KB.
   - This is registered as the new finding **SOLID-SEC-047**
     (Fixed; see `sec/SECURITY_REGISTRY.md`).  Matches the
     audit's stack-owned VkBuf strength (no Box, no LTO knob
     change).
6. **Anchor build is now green.**  All three programs link to BPF:
   - `target/deploy/zk_verifier.so`        (327 KB ELF eBPF)
   - `target/deploy/issuer_registry.so`    (684 KB ELF eBPF)
   - `target/deploy/schema_registry.so`    (332 KB ELF eBPF)
   The IDL-build step is unrelated and currently blocked by
   anchor 0.30.1 vs `proc-macro2 >= 1.0.95` upstream incompat;
   `--no-idl` is the agreed defer.  See B7 in `docs/E2E_BLOCKERS.md`.

Registry state: **22 open / 25 fixed / 47 total** (this session
adds SOLID-SEC-047 as Fixed).  Severities: **CRITICAL 0 open**,
HIGH 2 open, MEDIUM 12 open, LOW 4 open, INFO 4 open.

The to-the-end-of-localnet-E2E punch list (build, host-test, and
pipeline gates remaining) lives in **`docs/E2E_BLOCKERS.md`**.
That tracker is the canonical near-term to-do; entries below
focus on the longer-arc Phase 3 / pre-external-audit roadmap.

### Commits landed this Phase 2 session

| Commit    | What                                                         |
|-----------|--------------------------------------------------------------|
| `ad944ff` | Prelude: doc drifts, script gaps, safety guards (SEC-039/-040/-042 fixed). |
| `a9f0b9d` | ADR-0014: compressed issuer tree with BJJ-binding leaf.      |
| `4fba821` | CI harness: circuit_witness_tests + e2e_localnet.            |
| `58afb93` | Impl 1: IssuerTreeBinding PDA + solid-light parser.          |
| `73871dc` | Impl 2: circuit rev (issuer-tree inclusion + 6-input nullifier) + zk-verifier. |
| `df33ffe` | Impl 3a: 6-input nullifier across Rust/WASM/TS + IssuerAccount fields. |
| `167138e` | Impl 3b: atomic status-transition hooks + leaf lifecycle.    |
| (this commit) | Impl 3c: SDK + E2E pipeline + backfill + Phase 2 close-out. |

### Host-side baseline (green at this checkpoint)

```
cargo test -p solid-core  --lib    ->  49/49 (Phase 3 impl 1 SEC-007 +
                                             this session's two new
                                             Poseidon canonicalisation
                                             regression tests)
cargo test -p solid-light --lib    ->  25/25
cargo test -p zk-verifier --lib    ->  17/17 (Phase 3 impl 2 SEC-006 timelock)
python3 scripts/check_program_ids.py  -> consistent
head -3 Cargo.lock                   -> version = 3
ls target/deploy/*.so | wc -l        -> 3
```

### Open registry (by priority)

```
HIGH (3 open)
  SOLID-SEC-010  Cross-language vectors narrow (covers 3 of ~10 primitives)
  SOLID-SEC-012  Trusted setup single-party (mainnet blocker)
  SOLID-SEC-048  register_issuer BJJ subgroup check exceeds 1.4M CU
                  per-tx ceiling on BPF; localnet/devnet runs with
                  sec007-skip-onchain feature + off-chain SDK
                  predicate as load-bearing gate.  Mainnet deploy-
                  blocker.  P0-7 in improvements roadmap; B9 in
                  E2E_BLOCKERS.  MUST stay top-of-mind during all
                  subsequent work.

MEDIUM (12 open)
  -013..-019, -021, -034  (governance + throughput + schema-hash
                            re-assertion + fraud-proof seed check)
  -043                    (IssuerTreeBinding.operator single signer;
                            gate behind Squads 3-of-5 before ext. audit)
  -045                    (atomic handlers don't update
                            IssuerTreeBinding.current_root in-ix;
                            v0.6.1 NEW-01; half-day P0)
  -046                    (no CU-budget regression gate on
                            verify_batch_proof; v0.6.1 NEW-02;
                            half-day P0)

LOW (4 open)   -022, -023, -024, -035
               (SEC-041 and -044 closed 2026-04-25 Phase 3 impl 3/4)

INFO (4 open)  -025, -026, -037, -038

Fixed this late-session (2026-04-25): SOLID-SEC-047 (verify_batch_proof
BPF stack-frame overflow; structural).  Detail in
`sec/SECURITY_REGISTRY.md` SEC-047.

See sec/SECURITY_REGISTRY.md for the full detail + remediation plan
on each.
```

### To-the-end-of-localnet-E2E punch list

Tracked in **`docs/E2E_BLOCKERS.md`** with status legend
(Fixed-verified / Fixed-claimed / Open / Out-of-scope).  As of this
checkpoint:

- **Fixed (verified):**  B1 circuit compile, B2 Cargo.lock cascade,
  B3 light-poseidon BPF stack, B4 verify_batch_proof frame overflow,
  B6 wasm-pack against `solana-program` regular dep, B7 IDL
  generation via external `scripts/build_idls.mjs`.
- **Fixed (claimed; gate not yet observed):**  B5 cross-language
  vectors byte-equality.
- **Fixed (verified, 2026-04-26 ~02:15 IST):**  B10 -- `stake_tokens`
  post-CPI access violation.  Closed by **redesigning
  `initialize_registry`**: typed `Account<Mint>` for governance
  mint (closes the `Pubkey::default()` write path at the program
  boundary), atomic vault creation in the same accounts struct
  (`#[account(init, token::mint = governance_mint, token::authority
  = governance_vault, ...)]`), removal of `init_if_needed` on
  `governance_vault` from `StakeTokens`, deletion of the
  `init_governance_vault` ix, and new `InvalidThreshold` /
  `InvalidVotingPeriod` guards.  Verified end-to-end:
  `bootstrap-issuer` walks `[1/8]..[8/8]` + `[8b/10]
  append_issuer_leaf` cleanly on a fresh validator with
  `SOLID_VOTING_PERIOD_SECONDS=120`.  Why this is the right fix
  (not "drop init_if_needed"): the original `Pubkey::default()`
  was writeable into the registry because the contract had no
  validation -- relaxing the mint-match guard in `init_governance_vault`
  to accommodate stale state would have papered over a contract
  defect in `initialize_registry`.  The contract is the source of
  truth; the fix lives at the program boundary.  Regression gates:
  `tests/integration/01_registry_init.test.ts` (three new test
  cases pinning the contract invariants) + the planned IDL-snapshot
  CI gate (E4).
- **In-flight (E2E walked partway through):**  C3 (`npm run e2e`).
  `init-onchain` + `bootstrap-issuer` are green; `npm run issue`
  and `npm run prove` are now the live edge.
- **Live edge:**  `npm run issue` end-to-end.  `bootstrap-issuer`
  has put a registered, voted-on, finalized, tree-appended issuer
  on-chain; the next gate is the holder-side issuance flow producing
  a Poseidon-bound credential signed by that issuer's BJJ key.
  After that, `npm run prove` (Groth16 verify on-chain + replay-
  reject via nullifier PDA init).
- **Open:**  B9 / SOLID-SEC-048 (real on-chain fix; bypass plumbing
  landed but SEC-007 still live -- mainnet deploy-blocker), C1/C2
  (TS SDK link + check_vectors as CI gates), B8 (macOS validator
  `COPYFILE_DISABLE=1` + `rm -rf test-ledger` workaround), E4 (IDL-
  snapshot CI gate; not yet wired).
- **Out-of-scope-for-E2E:**  O1-O4 (already-tracked registry items
  + the pre-existing padding-slot test edge case), O5
  (`SolidIssuer.deliverToHolder` doc-vs-code drift; resolves with
  the credential-delivery design ADR), O7 (SOLID-SEC-048 closure --
  companion to B9; out-of-scope for the bypass landing but the
  whole-protocol soundness item).

The sequenced runbook in `docs/E2E_BLOCKERS.md` now includes:
(a) the `--features sec007-skip-onchain` rebuild + redeploy step
before `bootstrap-issuer`; (b) the `--clone-upgradeable-program`
flags for SPL Account Compression + noop on `solana-test-validator
--reset`; (c) the `SOLID_VOTING_PERIOD_SECONDS=120` calibration for
localnet (the default 86400 conflicts with the `register_issuer`
flash-loan cool-off only on long-running deployments, but the
20s default in `bootstrap_issuer.ts` is too short on localnet
because the cool-off is 100 slots ≈ 40s).  See the doc for the
current ordering.

---

## Next action (Phase 3 kick-off, sequenced)

### 1. ~~SOLID-SEC-006 -- VK freeze-gate~~ (Part 1 closed 2026-04-25)

Landed in Phase 3 impl 2.  ADR-0015.  `VerifierConfig` grew by 11
bytes (vk_finalized, vk_generation, rotate_request_ts; SPACE
49 -> 60).  Four new ix: `finalize_verification_key`,
`request_vk_rotation`, `cancel_vk_rotation`,
`rotate_verification_key`.  48-hour timelock.
`store_verification_key` now refuses every write against a
finalized VK.  Six host regression tests; zk-verifier 11 -> 17
green.  Part 2 (bind `vk_generation` into the public-input contract
and reject cross-VK replay) is deferred to the next trusted-setup
cycle where it batches with SEC-010 expansion.

### 2. SOLID-SEC-007 -- BJJ subgroup check at registration (Fixed host-side; partially reopened on-chain as SEC-048)

Phase 3 impl 1 landed the host-side fix.
`crates/solid-core/src/babyjubjub.rs` gained
`is_in_prime_order_subgroup` + `require_in_prime_order_subgroup`,
and `pubkey_to_affine` + `verify()` now use
`EdwardsAffine::new_unchecked` with an explicit
`is_in_correct_subgroup_assuming_on_curve()` check.
`programs/issuer-registry::register_issuer` was wired to call the
new helper before touching state.  WASM side exposes
`isBjjInPrimeOrderSubgroup`.  5 new Rust unit tests; solid-core
44 -> 49 host tests green.

**Reopen on-chain as SOLID-SEC-048 (2026-04-25 late-session).**
The host-side helper compiles for BPF but at runtime the `r * P
== O` scalar mul exceeds the 1.4M CU per-tx ceiling, so
`register_issuer` cannot land with the on-chain check enforced.
Interim shape: on-chain check is gated behind the
`sec007-skip-onchain` Cargo feature on issuer-registry (bypass
arm runs `is_on_curve + !is_identity` and emits a `Sec007Bypass`
event); off-chain TS `isInPrimeOrderSubgroup` predicate is the
load-bearing gate; localnet/devnet only.  Mainnet deploy-blocker.
Real-fix candidates (in increasing soundness order):

1. SDK-level cofactor-clear (cheapest; trusts off-chain caller).
2. Move subgroup gate into the issuance circuit (~30K extra
   constraints; batches with SEC-006 Part 2 trusted-setup cycle).
3. `sol_babyjubjub_*` syscall upstream proposal.

Tracking: `sec/SECURITY_REGISTRY.md` SOLID-SEC-048;
`docs/E2E_BLOCKERS.md` B9 + O7; `docs/IMPROVEMENTS_ROADMAP.md`
P0-7.

### 2a. SOLID-SEC-045 -- atomic handlers update `IssuerTreeBinding` in-ix

v0.6.1 NEW-01.  MEDIUM, P0 per v0.6.1 Section 6.1.  Half-day.

**Root cause.**  `revoke_issuer_atomic` and
`request_withdrawal_atomic` CPI `replace_leaf` into the SPL AC
tree (new root lands immediately) but leave
`IssuerTreeBinding.current_root` untouched.  The handlers document
"caller MUST invoke `update_issuer_tree_root` in the same tx",
which is enforcement-by-convention rather than
enforcement-by-code.  Until a follow-up `update_issuer_tree_root`
lands, a cached pre-transition proof still verifies against the
stale binding.  SEC-008's epoch-bound nullifier prevents
regenerating the same proof, but does not stop replay of a
pre-captured one.

**Change set.**  Promote `issuer_tree_binding` from
`UncheckedAccount` to a writable account in `RevokeIssuerAtomic`
and `RequestWithdrawalAtomic` contexts.  After `invoke_signed`
succeeds, derive the new root on-chain from the proof nodes in
`remaining_accounts` (or re-read from SPL AC changelog) and write
it plus `Clock::slot` into the binding's `current_root` +
`last_updated_slot` fields in the same ix.  Owner-check the
binding against `crate::ID`.  Update ADR-0014 amendment prose to
reflect the full atomicity claim.

**Regression gate.**  `tests/integration/07b_revoke_atomic_
binding_update.test.ts` plus mirror for `request_withdrawal_
atomic`.  Invoke atomic handler, then immediately attempt
`verify_batch_proof` with a pre-transition proof; expect
`IssuerTreeRootMismatch`, not success.

### 2b. SOLID-SEC-046 -- CU-budget regression gate on `verify_batch_proof`

v0.6.1 NEW-02.  MEDIUM, P0 per v0.6.1 Section 6.1.  Half-day.

**Root cause.**  No CI job measures `verify_batch_proof`'s CU
utilisation.  The compile-time stack-slice assertion at
`programs/zk-verifier/src/lib.rs:84-87` catches stack growth
only.  SEC-006 Part 2 (`vk_generation` public input) and SEC-010
primitives will bump CU at the next trusted-setup cycle; without
a baseline gate a future circuit change can push over the per-tx
CU ceiling silently.

**Change set.**  Add `.github/workflows/`
`verify_batch_proof_cu_baseline` CI job:
  1. Start localnet + run E2E through `issue.ts`.
  2. Construct `verify_batch_proof` tx with
     `ComputeBudgetProgram::set_compute_unit_limit(1_400_000)`.
  3. Parse "consumed X of Y compute units" from the tx log.
  4. Assert `X <= BASELINE * 1.10` (10% tolerance).
  5. Re-record baseline in `docs/CU_BUDGET.md` on sanctioned bumps.

**Regression gate.**  The CI job itself, plus the first-run
baseline recorded in `docs/CU_BUDGET.md`.  Trigger a conscious
budget-review on every sanctioned constraint addition.

### 3. SOLID-SEC-010 -- Extend cross-language vectors

Primitives still missing from `tests/vectors/`:
- Poseidon raw (fields + bytes)
- BJJ keypair generation
- BJJ sign / verify
- `derive_credential_key`
- `computeIdentityState`
- `QueryBuilder._computeContextHash`
- ADR-0014 issuer leaf (already implicitly covered via the WASM
  round-trip, but vector it explicitly)

Extend `crates/solid-core/examples/gen_vectors.rs` +
`tests/vectors/check_vectors.ts`.  Straightforward; blocks external
audit readiness.

### 4. ~~SOLID-SEC-041 -- Content-addressed VK artifact~~ (closed 2026-04-25)

Landed in Phase 3 impl 3.  `circuits/scripts/setup.js` now writes
`verification_key.sha256` next to the VK artifact and prints the VK
hash in the console summary.  `scripts/initialize.ts` enforces the
pin via `SOLID_VK_SHA256` env var (authoritative for release) or
the file (developer-loop convenience); refuses to upload when
either (a) the pin is missing, or (b) the computed hash does not
match the pin.

### 5. SOLID-SEC-012 -- Multi-party trusted setup

Mainnet blocker.  Must run a real ceremony with 10+ geographically
distributed contributors, each signing an attestation chain.
Artifacts (zkey, ptau, VK) hosted on IPFS + Arweave.  Not
one-sprint scope.

### 6. SOLID-SEC-043 -- gate issuer_tree_operator behind multisig

Replace the single-pubkey `IssuerTreeBinding.operator` with a PDA
signer (Squads 3-of-5 or SolID DAO threshold PDA). All four
tree-mutating ix (`update_issuer_tree_root`, `append_issuer_leaf`,
`replace_issuer_leaf`, `revoke_issuer_atomic`) wrap existing logic
behind that authority. Amend ADR-0014. Bundled with the wider
SEC-013 Squads migration.

### 7. ~~SOLID-SEC-044 -- request_withdrawal_atomic~~ (closed 2026-04-25)

Landed in Phase 3 impl 4.  `request_withdrawal_atomic` added to
issuer-registry; clones the `revoke_issuer_atomic` CPI flow with
`Cooldown` as the target status and the issuer's own keypair as
signer.  Legacy `request_withdrawal` now rejects enrolled issuers
with `IssuerTreeUpdateRequired`.  New `RevokeReason::
CooldownRequested` variant on `IssuerLeafReplaced`.  ADR-0014 now
carries the "Cooldown is verify-negative" amendment.

### 8. Governance cluster + LOW/INFO cleanup

SOLID-SEC-013..019, -034.  Squads 3-of-5 multisig, per-issuer stake
vaults, propose/accept authority transfer, etc.  Work separable
into small commits; good "warm-up" work while -012 is in flight.

### 9. External audit

After the items above close, schedule the external audit.  Per
Section 0 non-negotiable #5, mainnet slips one sprint after audit
close-out, not one sprint after submission.

---

## Verification steps on resume

```
cd /Users/rajakash/Desktop/testing/solid-protocol
git fetch origin
git log --oneline origin/main | head -10
git status                                # working-tree drift since last commit

# Toolchain pin (this session added MSRV-aware resolver opt-in)
rustup show active-toolchain               # expect 1.79.0
cat .cargo/config.toml | head             # expect resolver fallback opt-in
grep '^rust-version' Cargo.toml           # expect "1.75"

# Lockfile
head -3 Cargo.lock                        # must be version = 3

# Host-side baseline
cargo test -p solid-core  --lib           # expect 49/49
cargo test -p solid-light --lib           # expect 25/25
cargo test -p zk-verifier --lib           # expect 17/17
python3 scripts/check_program_ids.py      # expect "consistent"

# BPF artefacts (this session restored; smoke check)
ls target/deploy/*.so | wc -l             # expect 3
file target/deploy/zk_verifier.so         # expect ELF 64-bit ... eBPF
```

If any of the above tests has regressed against these numbers, the
working-tree drift is the first thing to inspect — the build-pipeline
fixes are uncommitted as of this checkpoint.

---

## Open decisions carried into Phase 3

1. **VK freeze-gate semantics.**  Should rotation require a DAO
   vote + 48h timelock (strongest), or registry-authority multisig
   only?  Ties into the SEC-013 Squads migration.  Decide in the
   ADR for SEC-006.

2. **Cooldown status semantics.**  Now tracked as SOLID-SEC-044
   (LOW, Open) in the registry.  Default Phase 3 stance: Cooldown
   disables proof verification; `request_withdrawal_atomic` mirrors
   `revoke_issuer_atomic` with a `replace_leaf` CPI.  Final ADR-0014
   amendment lands with the SEC-044 fix.

3. **`scripts/e2e_localnet` CI gate.**  Landed as scaffolding in
   `4fba821`.  Should it be a hard gate on every push (current),
   or graduated (nightly only)?  Resolution depends on runtime
   cost once we measure it on CI.

---

## Learnings (running log -- read before debugging)

Hard-won. Each entry names the concrete failure that taught it, so
future sessions can pattern-match instead of relearning.

### L1. Take a step back before tinkering

When a fix bounces ("change X, then Y breaks, then Z breaks"), stop
editing.  The cascade is the diagnostic: it means the change is
operating below the level of the actual defect, and every edit is
papering over symptoms one layer above the bug.  Sit down, write
out the current contract, write out what's failing, and find the
one thing whose definition produced the cascade.  Fix that.  The
cascade evaporates.

**Origin: B10 (2026-04-26).**  We bounced through three "fix the
handler" attempts on `stake_tokens` (drop `init_if_needed`, add a
separate `init_governance_vault` ix, then a guard-relaxation idea)
before recognising the real defect was one layer up:
`initialize_registry` accepted a free `governance_token_mint:
Pubkey` parameter with no validation.  Once the contract was
fixed, all three "handler-side" changes became unnecessary -- the
correct shape (atomic vault creation in `InitializeRegistry`'s
accounts struct) was the ONLY shape that didn't generate a
cascade.

### L2. Programs and circuits are the source of truth; everything else is a consumer

The hierarchy is:

1. **Programs + circuits** (anchor `lib.rs`, `*.circom`) define
   contracts: input types, validation, state transitions, output
   guarantees.  These are load-bearing.
2. **Crates** (`solid-core`, `solid-light`) are the canonical
   primitives -- byte-equal to the circuit and the programs, and
   referenced by both.
3. **WASM bridge + TS SDK** are a transcription layer above the
   crates.  They MUST round-trip byte-equal (cross_language_vectors
   CI gate).
4. **Scripts** (`scripts/*.ts`, `scripts/*.mjs`) are operational
   consumers.  Their job is to drive the contract; they do not get
   to redefine it.

When a script fights a contract, the script is wrong, OR the
contract is wrong.  It is never "the script needs to be smarter
about working around the contract."  Pick one of those two,
fix it, and move on.

**Origin: B10.**  When `init_governance_vault` rejected a stale
on-chain `Pubkey::default()` mint with `Unauthorized`, the first
instinct was "make the script reconcile."  The user's call --
"if the contract itself is incorrect then it should be fixed" --
was the correct one: the contract was wrong (it had let
`Pubkey::default()` in to begin with).  The script change would
have been a workaround that hid a contract defect.

### L3. If the contract is wrong, fix the contract -- no workarounds, no regressions

Corollary to L2.  When you decide the contract is wrong:

- Fix it at the program / circuit boundary.  Not in the caller, not
  in the SDK, not in a script.
- Encode the invariant in the type system where possible (Anchor
  `Account<T>`, `Account<Mint>`, `Signer`, `Program`, typed PDAs).
  Free `Pubkey` / `[u8; 32]` parameters are a smell; they let
  garbage in.
- Atomicity beats convention.  If two accounts must be born
  together, birth them in the same accounts struct
  (`#[account(init, ...)]`), not in two separate ix with a "caller
  MUST invoke both" comment.  Convention rots; types don't.
- Add `require!` guards for any ranges / enumerations the type
  system can't express (e.g. basis-points <= 10000, periods > 0).
  Each guard gets a named `ErrorCode` -- the error name is the
  contract documenting itself.
- Stale on-chain state from a pre-fix build is NOT a reason to
  relax the new contract.  The fix is a clean validator restart
  (`--reset` plus the right `--clone-upgradeable-program` flags),
  not a softer guard.

**Origin: B10.**  Fix landed as: typed `Account<'info, Mint>`,
atomic vault creation, `require!` guards on threshold / voting
period, `init_governance_vault` deleted entirely, three new test
cases pinning the new invariants.  Any future call that violates
the contract fails at deserialise-time; no script-level
reconciliation logic exists or is needed.

### L4. Add unit + integration tests + logs as part of the fix, not after

When a defect surfaces, the same commit should land:

- A **unit test** at the contract boundary that fails on the
  pre-fix code and passes on the post-fix code.
- An **integration test** if the defect crossed program boundaries
  or required end-to-end orchestration.
- **Targeted `msg!` lines** in the suspect code paths if the
  defect was opaque to logs (B10 needed `stake_tokens` trace
  `msg!`s to narrow the access-violation site to "post-handler
  writeback").  Trace logs that paid off should stay; speculative
  ones can be removed once the root cause is known.
- A note in `docs/E2E_BLOCKERS.md` (or the relevant blocker doc)
  that names the regression gate.  "Regression gate" is the column
  that tells future readers what stops the same defect from
  re-landing.

The test is part of the fix, not a follow-up.  A fix without a
regression gate has not actually shipped.

**Origin: B10.**  `tests/integration/01_registry_init.test.ts`
was rewritten on top of `solana-bankrun` with three new cases
(`rejects approval_threshold > 10000 bps`, `rejects voting_period
== 0`, `rejects a non-Mint account passed as governance_mint`)
PINNING the new invariants, including the explicit regression
gate for the original `Pubkey::default()` defect.  Without those
cases, the next refactor of `initialize_registry` could
silently regress.

### L5. The error address is a clue, not noise

When the runtime emits `Access violation in unknown section at
address 0xXXXX of size N` and the address varies between runs,
that high-entropy address is signal: it means the dereferenced
pointer is uninitialised / corrupted, not a fixed layout bug.  The
fix is upstream of the dereference (init / writeback / lifetime),
not at the dereference site.

When the address is stable across runs, it's a fixed layout bug:
read the linker map, read the writeback contract, find what owns
that offset.

**Origin: B10.**  The varying address (`0x360358543e3fd94a`,
`0xb091...`, etc.) was the signal that pointed at writeback /
init lifecycle, not at a constant offset in `StakerAccount`.  Time
not spent staring at byte arithmetic was time available for the
contract redesign.

### L6. Doc lies are tomorrow's bugs

Every time a doc says "caller MUST do X" instead of the program
enforcing X, you have already shipped a future bug.  The
enforcement-by-convention is one careless integration away from
becoming enforcement-by-nothing.  Promote convention to type or
guard.  See SOLID-SEC-045 (`update_issuer_tree_root` "caller MUST
invoke ...") for a live example -- promoted to atomic enforcement
in v0.6.1 NEW-01.

---

## Rules of engagement (reprinted)

Full list in `plan/IMPLEMENTATION_PLAN.md` Section 9.  Quick
reference:

1. Root cause only.  No workarounds, no suppression, no
   `--no-verify`, no `#[cfg(feature = "unchecked")]`.
2. No regressions.  Every fix ships a regression gate.
3. No doc lies.  Either the doc matches the code or the doc moves
   to `docs/archive/` with a HISTORICAL banner.
4. One source of truth per artifact.
5. PR review checklist: root cause? regression test? doc truth?
   single source of truth? ADR compliance? registry aligned?
6. Take a step back when a fix bounces (L1).  The cascade is the
   diagnostic.

Phase 1 held to all six.  Phase 2 held to all six.  Phase 3 must
too.

---

## 2026-04-28 -- circuit/ZK audit + e2e bring-up (this session)

**Receipts (all verifiable from the working tree).**

Program-side audit + closures (committed at HEAD `59a85b3`):
- SEC-045 (MEDIUM, NEW-01) closed.  `revoke_issuer_atomic` and
  `request_withdrawal_atomic` now write `IssuerTreeBinding.
  current_root` atomically.  Helper
  `solid_light::cpi_helpers::compute_concurrent_merkle_root_keccak`
  walks the proof path with Solana's `keccak::hashv`; soundness
  rests on the CPI's prior validation of the path against the
  pre-CPI tree root + Keccak256 pre-image resistance.  The
  `update_issuer_tree_root` doc lie is gone.  Helper unit tests
  (7) + binding-write regression (5 negative cases) + 13
  layout-invariant tests in issuer-registry.
- SEC-049 (HIGH, NEW) closed.  `SPL_AC_REPLACE_LEAF_DISCRIMINATOR`
  was `[0xe3,0x88,0x6a,0x74,...]` -- matches no SPL AC ix
  preimage.  Correct `sha256("global:replace_leaf")[..8]` =
  `[0xcc,0xa5,0x4c,0x64,0x49,0x93,0x00,0x80]`.  Latent because
  no integration test covered revoke / cooldown.  Caught by the
  discriminator-derivation regression test I added in the
  test sweep.
- SEC-044 (LOW) verified closed: pre-existing
  `request_withdrawal_atomic` + the SEC-045 fix together close the
  Cooldown-replay window.
- 30 schema-registry layout + helper tests; 17 issuer-registry
  layout + binding-write tests; 12 zk-verifier additions.
  Workspace 167/167 cargo green.

Circuit-side audit + closures (working tree, post-trusted-setup):
- SEC-050 (MEDIUM, NEW) closed.  `batch_credential_query.circom`
  schema-ordering canonicality bypass.  Strict-ascending check was
  skipped when next slot was inactive, so `[A1, 0, A2, A3]` and
  `[0, A1, 0, A2]` both passed and produced different
  `queryContextHash` values for the same credential set --
  defeats the verifier's per-claim nullifier rate-limit.  Fix:
  one extra constraint per pair `isZero[i].out * (1 -
  isZero[i+1].out) === 0`.  Trusted-setup re-run; new VK pin
  `8385b82b032f65e505c784b28486ca8bec7da3f3d4b97b82724e697734565146`
  written to `circuits/build/verification_key.sha256`.
  17-case witness-tester regression at
  `circuits/test/schema_ordering.test.js`.  Existing 22 circuit
  tests + 17 new = 39/39.
- SEC-051 (LOW, NEW) tracked.  All-padding `[0,0,0,0]` proofs are
  admitted by the circuit and the on-chain handler skips zero-
  schema slots at `programs/zk-verifier/src/lib.rs:500-502`.
  Realistic queries fail on zero data, but "at least 1
  credential" is not a circuit-level invariant.  Defer-with-
  justification to next sanctioned trusted-setup cycle (bundles
  with SEC-006 Part 2 + the predicate-operand range-checks).
- SEC-052 (HIGH, NEW) closed (partial).  Two BPF-runtime /
  cross-layer coord-form drifts surfaced during e2e bring-up:
    (a) `is_on_curve` and `is_identity` rewritten in
    `crates/solid-core/src/babyjubjub.rs` to evaluate the
    circomlib-native curve equation directly using only `Fq * Fq`
    and `Fq + Fq` -- avoids the BPF-incompat
    `EdwardsAffine::new_unchecked(x_circ_to_ark(x_circ), y).
    is_on_curve()` path that rejects valid points on BPF.  Host
    test `test_keygen_passes_on_chain_consolation_gate` pins the
    new contract.
    (b) WASM bridge `solid_wasm_bg.wasm` was stale (Apr 25 21:59,
    pre-cff06c2 "circuit update") -- produced arkworks-form bytes
    while on-chain code expected circomlib-native form.  Rebuilt
    via `wasm-pack build wasm/`; both sides aligned.  Process
    gate added at `docs/E2E_BLOCKERS.md` B11.

E2E pipeline run:
- Validator killed and restarted with `--reset
  --clone-upgradeable-program <SPL AC> --clone-upgradeable-program
  <noop>`.
- All three programs redeployed (issuer-registry with
  `--features sec007-skip-onchain`; program-data account extended
  via `solana program extend ... 200000` to fit the larger .so).
- `npm run init-onchain` -> uploads new VK pin (3 chunks, 2564
  bytes); RegistryConfig + GlobalStateBinding + VerifierConfig
  initialised.
- `npm run backfill-issuer-tree` -> issuer SPL AC tree at
  `6oxgd4f1pXPXU3n5b6sy889pBw82dU7h4sor3PgprvMR`,
  `IssuerTreeBinding` at
  `ExJ2PLDf8qrDxYsYRJbuKNTJ38xPxt1USJpe7cdfgL6h`.
- `npm run bootstrap-schema-tree` -> schema SPL AC tree at
  `9kEpm21BLknx54D7h4Vz83FVMvfZWpvZsX2tiN1QzDbU`,
  `SchemaTreeBinding` at
  `bM7C5RPo6mAZPa4agZgEACJYdcY1FJbNFZKW5UXp4YM`.
- `SOLID_VOTING_PERIOD_SECONDS=120 npm run bootstrap-issuer`
  -> [3/8] `register_issuer` ok (issuer
  `FT28CPrQ1RU3aydXJyBxErYMRR33kc7zbC6eRyoBhwd2`); [4/8]
  `stake_tokens` ok; [6/8] `vote_on_issuer` ok; [8/8]
  `finalize_voting`; [8b/10] `append_issuer_leaf` ok (binding
  root `0x47165b08fd77ac91...`).
- `npm run issue` -> credential committed
  (commitment `f0d980f648927882d9fedea82df2e7c885ad55fcc394904e684552fa3ce6e02c`).
- `npm run prove` -> blocked at B12 (EdDSA witness drift inside
  `CredentialAtom`).  Diagnostic plan at
  `docs/E2E_BLOCKERS.md` B12.

**Learnings (running log).**
- L7. **Off-chain WASM byte format must be CI-gated against
  on-chain expectations.**  cff06c2 changed the wire byte format
  (arkworks -> circomlib-native) without rebuilding
  `solid_wasm_bg.wasm`; host tests pass on both sides
  independently because they exercise their own runtime, but the
  cross-layer wire contract was never asserted.  Pattern:
  "dual-target-link is not dual-target-runtime" (sibling of the
  SEC-048 receipt).  Fix: a CI smoke test that runs
  `wasm-pack build wasm/` -> `node scripts/wasm_bridge_smoke.mjs`
  -> assert byte-equal round-trip with on-chain `is_on_curve` for
  a freshly-generated keypair.  Promote any contract-bearing wire
  format change in `babyjubjub.rs` (or any `crates/solid-core/`
  byte-format helper) to also touch CI.  L6 lesson reinforced:
  doc claims of "rust + ts byte-equal" must be enforced
  end-to-end, not just at the Poseidon vector layer.
- L8. **Audit depth-first vs cross-layer.**  This session's circuit
  + program audits found SEC-049 + SEC-050 + reinforced SEC-051
  cleanly, but missed SEC-052 because each layer was reviewed in
  isolation.  Add a "wire format" audit pass to the playbook:
  for every coord-form / byte-format change, walk the producer
  (off-chain SDK) and consumer (circuit + on-chain) and assert
  they share the documented invariant.
