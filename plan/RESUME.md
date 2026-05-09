# Resume -- where to pick up next session

Living handoff doc. Read this first when starting a new session.
Updated at the end of each session; the last-updated line is
authoritative.

- **Last updated:** 2026-05-07 (fresh devnet issue/prove/replay green).

  **Current truth:** the protocol is partially live on public devnet, but
  the system is **not public-test-ready yet**. Programs, registry config,
  verifier config/VKs, issuer tree, `basic_identity_v2` schema tree,
  sample issuer, and fresh credential issuance exist on devnet. `zk_verifier`
  and `issuer_registry` are upgraded from current source. The smoke
  issuer/schema permission PDA is initialized. `scripts/prove.ts` is
  live-root-aware for repeated devnet runs, and fresh issue -> root sync ->
  Groth16 proof -> on-chain verify -> replay rejection is green:
  `4gp49ttdgCeZeegBiE3LsJN3uBXhsHYCiQqQW58F9YjJYhbvhAd6eLRdKRedvnfP8v8eqZnT67b2X1oYZPsre6yE`.
  Public tester onboarding is now blocked by hosted artifacts, indexer/API,
  solid-sim deployment, and a four-role solid-sim smoke.

  **Admin/deployer state:**
  - Deployer pubkey: `Gdz9JLWUekrfnpT3fPu1SsWfas3b3zMhfC4frvV1QRNm`.
  - Local deployer path: `~/.config/solana/solid-devnet-admin.json`.
  - Balance after ProgramData extension / deploy / buffer cleanup:
    approximately `18.19 SOL`.
  - `NO_DNA=1 anchor build --no-idl` completed and
    `solana program deploy target/deploy/zk_verifier.so ...` completed for
    `DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb`.
  - `issuer_registry` ProgramData was extended by `262144` bytes, then
    deployed successfully. Live slot moved to `460541773`.
  - Failed deploy buffers from public-RPC retries were closed and rent was
    recovered. Do not paste or preserve any ephemeral buffer seed phrase
    printed by failed deploy commands.

  **Live devnet smoke values now recorded in `deployments/devnet.json` and
  `docs/DEVNET_STATUS.md`:**
  - Programs:
    - `schema_registry`: `4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1`
    - `issuer_registry`: `5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx`
    - `zk_verifier`: `DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb`
  - Governance mint: `5PrqMfDCWqMWnX1WdqHUuaa3kx29NeGLJFazB2hXGDzu`
  - Registry config PDA: `7CVeXUKeiWBhkGUuvte89GVCkXJHnR5D2YNCLWvVgSmH`
  - Verifier config PDA: `G4jFquTUyeRqzvsnNwRdDRE3TeiGyzKSGZ9q4PbwJPmW`
  - VK storage PDA: `8qjhn8Ze3Mhj9xdgAajMVxKbnXxHhmY6nTQf5N5yG4Rf`
  - Global binding PDA: `68Twk6dXwbahut6VDv1wSRkQMdFZRaeTbhUMNPiaJM8o`
  - Global root:
    `692c7934333b0be0fcac73a111406be96d5be50b6b0ce7eb6424a08fa601e21f`
  - Issuer tree binding PDA: `ExJ2PLDf8qrDxYsYRJbuKNTJ38xPxt1USJpe7cdfgL6h`
  - Issuer tree: `FajCWko9tc6dhdPtwrS5kfkehPoEV8pn5LdL4kLL7k7f`
  - Issuer tree root:
    `bfc02634df26b227601eabeccb97d6d79f39fb23ec19d7b62ec1565f669f5b27`
  - Smoke schema: `basic_identity_v2`, version `2`, depth `20`.
  - Smoke schema hash:
    `6b5014bf611a025a4693b196a517ece9f2d0672672a2eb38d7a50481474e6823`
  - Smoke schema PDA: `EmdhTJt5VcQ4FiTy9xitTBBNa3XaBvwXsAB34FJWkx4`
  - Smoke schema tree binding: `FiiYYq2gysBiwhVYptS5GkmKvMJETXNXthz9SrTciuDb`
  - Smoke schema tree: `4mhWLGb2KAtF1bY2mdrGb37xhAUpmRsL9bgzLRjE35sc`
  - Smoke schema tree root:
    `2c9ac3430dccf2c66f2128c5ea8b7c4ad83efdce6fa29b57b29b5e044773fb1e`
  - Sample issuer authority: `GwqjvUSmPKeNnPSWzFBPnURGXVkpLAzdujMuPCEPMyNi`
  - Sample issuer account: `FEdBRzX1junyhKf5Co15XKtHeAezT8i49zXSK6cGCEwL`
  - Sample credential commitment:
    `69cf17f65415a057480ab7b9a84bba23ddae3d5764dafc5c3a9db8a97c476425`
  - Sample credential tx:
    `gUFgazuUQnMu89rGMVSvS2ZfmBhUxWd2Ki9aCsECDaGDCe7GE5YB1tEEV2nz1NgCNCtp8KSsTRbWt5SNcXjbS3g`
  - Sample verify tx:
    `4gp49ttdgCeZeegBiE3LsJN3uBXhsHYCiQqQW58F9YjJYhbvhAd6eLRdKRedvnfP8v8eqZnT67b2X1oYZPsre6yE`
  - Issuer/schema permission PDA:
    `4Eo32kQPu9mvVypVRM83FV76ZZa3RSe5LWBwgpZcxx5H`
  - Issuer/schema permission tx:
    `2x9CefTUgwfthgqob9NpLYMhL3prGsd88c1wrbLsrb6w4r2yxykeveFnpK4Sw2Rzm4XZhynJJ4XuCppJbVzFyX81`
  - Smoke schema tree root:
    `5304536b69350abe799a36ed8104efbfa66a5198f8ab6cef6bc3e6c48357b919`
  - Global root:
    `610b2e31e33bc10e3e7e0cf0e6fbeaae0c23b42c24266f1bc3a80685e805ce01`

  **Important caveat:** an early `basic_identity_v1` binding used a
  depth-16 credential tree. Do not use that binding for current batch
  circuit proofs. Use `basic_identity_v2` / depth 20 for smoke testing,
  and register all launch schemas with depth-20 trees.

  **Docs refreshed in this pass:**
  - `deployments/devnet.json`
  - `docs/DEVNET_STATUS.md`
  - `docs/DEVNET_QUICKSTART.md`
  - `docs/DEPLOYMENT_AND_TESTING.md`
  - `docs/integration-guide.md`
  - `docs/VERIFIER_INTEGRATION.md`
  - `docs/ISSUER_GUIDE.md`
  - `docs/WALLET_PROVIDER_FLOW.md`
  - `docs/SCHEMA_AUTHORING.md`
  - `docs/schemas.md`
  - `plan/DEVNET_READINESS_CHECKLIST.md`
  - this `plan/RESUME.md`
  - `config/devnet.env.example`
  - `config/localnet.env.example`
  - sibling `solid-console/.env.devnet.example`,
    `solid-console/.env.localnet.example`, `solid-wallet/.env.devnet.example`,
    and `solid-wallet/.env.localnet.example`

  **Verification run after the docs/manifest update:**
  - `npm run validate:devnet` -> pass.
  - `npm run smoke:devnet-config` -> pass; shows live program/schema/tree
    fields and null artifact/indexer/console/wallet URLs.
  - IDE lints on edited docs/manifest -> no linter errors.
  - Current session reran `python3 scripts/check_program_ids.py`,
    `npm run validate:devnet`, `npm run test:prove-root-plan`, devnet
    `npm run prove`, and live Solana account checks. The devnet prove run
    produced the verify tx above and replay rejection. Fresh `npm run issue`
    also succeeded after the upgraded `issuer_registry` deployment and
    issuer/schema permission initialization.

  **Manual terminal E2E command block is now in
  `docs/DEVNET_QUICKSTART.md`.** The current expected result is green
  through issuer/credential/root-sync/local-proof/on-chain verifier/replay
  rejection.

  **solid-sim state:** `solid-console` is now the `solid-sim` package and
  app surface. It has one System tree with Flow, Schemas, Logs, and DAO,
  Issuer, Wallet, and Verifier subsections. The Flow page visualizes DAO ->
  Issuer -> Wallet -> Verifier causality, with motion originating at the DAO
  trust body. The visual system uses one SolID product accent; status colors
  are semantic only. The Wallet section creates/imports a simulator seed,
  derives real channel and schema-bound holder BJJ public keys, imports
  encrypted credential envelopes, validates holder
  binding/commitment/subgroup/signature integrity, and only generates proofs
  when real artifacts plus a Merkle proof indexer are configured. `npm run
  build` and `npm run test` pass in `solid-console`.

  Public solid-sim/integrator testing still requires hosted circuit artifacts,
  a Merkle proof indexer/API, public manifest URLs, and a four-role browser
  smoke in the deployed solid-sim.

  **Immediate next action:** deploy the artifact host, deploy the indexer/API,
  update `deployments/devnet.json` with public URLs, deploy solid-sim, then
  run the full four-role smoke through solid-sim instead of terminal scripts.

- **Previous update (preserved for history):** 2026-05-02 (SEC-048
  Phase E close).  This
  session shipped Phase E.1..E.4 + the SOLID-SEC-083 hardening
  (auth-race on `init_subgroup_verifier`) + three workflow learnings
  (pipe-tail buffering / clean-slate-before-e2e / snarkjs hangs).
  **SOLID-SEC-048 closed end-to-end on the no-bypass build:**
  `npm run e2e` exit 0; `verified: true` reference tx
  `3NkZqYjYDdSwrEveqYJMZaDPuadMiQoJ79bwjbFzPs9B1CUHWZTqeUyehJW99mTmMRaxgfzm4NnM1iYmYtE1G5k4`
  Finalized on the local validator; replay rejected.
  `sec007-skip-onchain` Cargo feature DELETED; `Sec007Bypass` event
  DELETED.  Quick state:
  - **HEAD:** post Phase E.4 commit `30343b5` + the pending E.6
    SEC-083 / process.exit / docs-sweep commit (NOT YET PUSHED).
    Five Phase E commits land locally before the push: `cbbe088`
    (E.1) -> `87cdd34` (E.2) -> `0a97099` (E.3) -> `30343b5` (E.4) ->
    (pending E.6).  Full receipts at
    `plan/SESSION_LOG_2026-05-02.md`.
  - **Host tests:** 279 total (solid-core 73 / solid-light 68 /
    zk-verifier 30 / issuer-registry 33 / schema-registry 30) +
    45 circuit witness tests, all green.
  - **E2E:** `npm run e2e` exit 0 against the no-bypass build.
    Reference tx confirmed Finalized on local validator.
  - **CU baselines:** `RegisterIssuer` jumped 76,107 -> 180,037 CU
    (on-chain Groth16 N=2 subgroup verify added; 35% of 500K cuIx);
    `VerifyBatchProofV2` 322,356 CU (1.008x baseline).  21/21 ixs
    within 1.10x tolerance.  SOLID-SEC-046 CI gate clean.
  - **SEC-048 status:** **CLOSED 2026-05-02 via Phase E (Option B).**
    Subgroup-circuit Groth16 verify wired into `register_issuer`;
    bypass DELETED.
  - **SEC-083 status:** **CLOSED 2026-05-02** (HIGH; auth-race on
    `init_subgroup_verifier`; constraint `authority ==
    registry_config.authority` added).
  - **What's next:** push the local commits, then the multi-party
    trusted setup ceremony (SOLID-SEC-012) which gates mainnet for
    BOTH the batch circuit and the subgroup circuit.  Detailed
    pickup notes at the bottom of
    `plan/SESSION_LOG_2026-05-02.md::Pickup next session`.

- **Previous update (preserved for history):** 2026-05-01 late
  (post-NF-batch + 3 fix arcs + SEC-048 Phase A/B/D-1).  This
  session shipped SEC-017 (prover OsRng + workspace rebuild),
  SEC-081 (TS pubkey-literal CI gate + 5 regression tests), SEC-080
  (schema-registry pre-CPI binding anchor mirroring SEC-077, +5
  host tests), the doc-consistency sweep, and SEC-048 Option B
  Phases A/B/D-1 (spec correction, isolated subgroup-check circuit
  + 6 mocha tests, parameterised setup script + ceremony reusing
  the existing PTAU, end-to-end snarkjs prove+verify smoke green).
  Quick state at the time:
  - **HEAD:** `27d46ba` ("SEC-048 Phase D-1: parameterize setup.js
    + run subgroup ceremony").  Five commits this session:
    `f985b1e` (SEC-017 + prover rebuild) -> `1b7643a` (SEC-081) ->
    `4a8d8d6` (SEC-080) -> `07449c3` (SEC-048 A+B) -> `27d46ba`
    (SEC-048 D-1).  All on `origin/main`.
  - **Host tests:** 217+ across solid-core / solid-light /
    zk-verifier / issuer-registry / schema-registry; 45/45 circuit
    witness tests (was 39 + 6 new SEC-048 cases); 5/5 SEC-081
    regression tests; 1/1 SEC-017 RNG tripwire.
  - **E2E:** `npm run e2e` returns `verified: true` + replay
    rejection.  Last reference green tx for `verify_batch_proof_v2`:
    `5i3DzrRFXCXQmuEi...` (fresh post-SEC-080).
  - **CU baselines:** refreshed twice this session (post-M-batch
    `+3K` constant overhead on buffer-account ixs; post-SEC-080
    `+356/+320 CU` on UpdateTreeRoot/UpdateGlobalRoot sentinel
    path).  21/21 ixs within 1.10x tolerance.  Largest consumer
    still `AppendIssuerLeaf` at ~428K CU.
  - **SEC-046 CI gate:** active and clean; re-measures per PR.
  - **SEC-081 CI gate:** added; runs `scripts/test_check_program_ids.py`
    after the live tree gate.
  - **SEC-048 status:** Phase A (spec) + B (circuit + tests) +
    D-1 (subgroup ceremony) all green.  Bypass
    (`sec007-skip-onchain`) STILL active; Phase E lands the
    on-chain wiring + drops the bypass + closes SEC-048.
  - **What's next:** the trusted-setup batch -- SEC-048 Phase E
    (on-chain) + SEC-006 Part 2 + SEC-051, then ts-sdk jest
    baseline.  Detailed handoff at the new section
    "Pickup next session: Phase E + SEC-006 P2 + SEC-051 +
    ts-sdk jest baseline" below.

- **Previous update:** 2026-04-29 (B13 (1)+(3) attempt; latent-bug
  fixes; pivot to Option 2 for tomorrow).  See
  `plan/SESSION_LOG_2026-04-29.md` for the full receipts.  Quick
  state:
  - **HEAD:** `c240b90` plus uncommitted WIP across 12
    semantically-modified files (handler + SDK + scripts + 5 docs).
    Per-file ledger in the session log §6.  No commit yet --
    tomorrow's first action should split this into atomic
    commits per L9 of the session log's discipline rules.
  - **E2E status:** `npm run e2e` walks through `init-onchain ->
    backfill-issuer-tree -> bootstrap-schema-tree ->
    bootstrap-issuer -> issue -> prove (Groth16 proof generated)`
    ALL GREEN through proof generation.  On-chain
    `verify_batch_proof` submission via the (1)+(3) path is
    **architecturally 37 bytes over** Solana's 1232-byte
    legacy-tx packet cap once a `setComputeUnitLimit` ix is
    prepended (which is required because the default 200K CU is
    insufficient for `verify_batch_proof`'s ~285K-345K baseline).
    See `docs/E2E_BLOCKERS.md` B13 for the byte-by-byte ledger.
    Live edge has shifted from "implement (1)+(3)" to "implement
    Option 2 (buffer-account / chunked upload)".
  - **What landed in code this session (uncommitted WIP):**
    - 11 of 32 public-input slots reconstructed on-chain in
      `programs/zk-verifier/src/lib.rs` (B13 (1)).
    - SDK encoder (`ts-sdk/packages/verifier/src/index.ts`) sends
      21-input wire (972 bytes).
    - Address Lookup Table support: `ensureLookupTable` helper +
      v0 transaction path in `verifyOnChain` (B13 (3)).
    - 19 new cargo unit tests (13 in solid-light extract helpers,
      6 in zk-verifier slot-partition invariants).  Cargo
      workspace: **189/189 host tests green**, up from 169.
    - Three latent-bug fixes (LB1-LB3) the session uncovered.
      See §1.3 of the session log for the full story; short form:
      - **LB1.** Anchor 0.30.1's `Vec<[u8; 32]>` BorshDeserialize
        fails on BPF.  Fix: change `public_inputs` arg to
        `Vec<u8>`, chunk in handler.  Why exactly the per-element
        deser path breaks on BPF is unverified -- the fix is
        empirical and durable.
      - **LB2.** SEC-005 timestamp slot was being read as `LE u64`
        from bytes [0..8], but the SDK BE-encodes every public
        input.  Fix: read `from_be_bytes(ts_bytes[24..32])`,
        zero-check `[0..24]`.  Pre-existing latent since SEC-005
        landed (no proof had ever reached the on-chain handler).
      - **LB3.** Reconstructed Merkle roots / schema hashes are
        stored on-chain in LE byte-form (holder uses `bufToDecimal`
        LE-decode), but Groth16 expects BE.  Fix: byte-reverse on
        copy for slots 1, 2..5, 6..9, 10.  `verifierAddress`
        (slot 29) is the exception because the holder uses
        `bufToDecimalBE` for it (SOLID-SEC-031).  Pre-existing
        latent in `verify_state_root_matches` /
        `verify_schema_root_binding` /
        `verify_issuer_tree_binding_for_proof` -- those compare
        wire BE bytes to stored LE bytes and would have failed
        the same way had they ever been exercised.
    - SOLID-SEC-054 row added to registry (status: should be
      downgraded to "Open (interim partial)" tomorrow -- see §3
      of the session log).
    - New `docs/REMEDIATION_OPTIONS_ARCHIVE.md` capturing
      rejected paths.
  - **Other findings logged for future work** (NOT introduced
    this session, see session log §4):
    - Pre-existing `cargo clippy -p solid-core -- -D warnings`
      breakage on `main` (5 dead-code warnings).  User said they
      will fix in a separate commit.
    - LB2 / LB3 implies `verify_state_root_matches` family has
      latent BE/LE bugs too.  Audit + remove or fix when
      pre-reconstruction code paths are no longer reachable.
    - SOLID-SEC-031 should grow into a full cross-layer
      byte-encoding table (which slot uses which convention).
  - **Workspace tests at session close:** 189/189 cargo (host).
    Mocha witness-tester not re-run.  ts-sdk builds clean.

## Pickup next session: Phase E + SEC-006 P2 + SEC-051 + ts-sdk jest baseline

**Read first:** this section, then `plan/BACKLOG_2026-05-01.md`
Tier-1 entries 2a + 4 + 5, then `sec/SECURITY_REGISTRY.md` SEC-048
"Real fix candidates" Option B section (corrected spec) + SEC-006
+ SEC-051.  Discipline rules L1, L4, L7, L8, L9 from this file's
Learnings log apply directly.

### Pre-flight invariants (do NOT proceed if any of these are wrong)

1. `git log --oneline -5` shows `27d46ba` at HEAD on main.
2. `circuits/build/bjj_subgroup_verification_key.sha256` reads
   `938ab39020f31156fa7e8fc230fc458adba5f13e08c64d41d9dbffdbc3643ce9`.
   If absent, regenerate via:
   ```
   cd circuits && PATH="$(cd .. && pwd)/.toolchain/bin:$PATH" \
     circom bjj_subgroup_proof.circom --r1cs --wasm --sym --c \
            --output build/ -l node_modules/circomlib/circuits
   node scripts/setup.js --circuit bjj_subgroup_proof
   ```
   The sha256 will differ on each run (fresh OS entropy); **pin
   the hash you generate as the new in-tree pin** before
   committing.
3. `cd circuits && npm test` returns 45/45.  If not, the existing
   subgroup tests broke -- diagnose before any new work.
4. `cargo test -p solid-light --lib` returns 54/54 (49 +
   5 SEC-080 helper tests).  If not, the SEC-080 anchor
   regressed and Phase E pulls something incompatible.
5. `bash scripts/sync_program_keypairs.sh --reset-state` runs
   clean.  Validator can be `solana-test-validator --reset` and
   re-deployed via the CLAUDE.md / DEPLOYMENT_AND_TESTING.md
   runbook.

If any invariant fails, **fix that first** -- do NOT attempt Phase
E on a broken baseline (L1: cascade is the diagnostic).

### Architecture (locked, don't second-guess)

The user approved **Option 1 (registration-time subgroup proof)**
over **Option 2 (in-batch-circuit subgroup constraints)** at the
2026-05-01 check-in.  Decision recorded in
`sec/SECURITY_REGISTRY.md` SEC-048 entry.  The reasons are:

1. Registry state itself becomes the trust set ("every registered
   issuer has a subgroup-validated key").  Auditors can reason
   about SEC-048 from registry state alone.
2. Per-credential proving cost stays unchanged forever (no
   10-15% tax on every proof).
3. Clean Option C (Solana SIMD `sol_babyjubjub_*` syscall)
   migration -- the syscall replaces just this small circuit's
   on-chain verify; main batch circuit doesn't move and doesn't
   need a re-ceremony.

The **spec correction** is also locked: the invariant is
`on_curve(P) AND P != (0, 1) AND [r] * P == (0, 1)` where
`r = 2736030358979909402780800718157159386076813972158567259200215660948447373041`
is the BJJ subgroup prime order.  The earlier `[8] * P == P`
framing in older commits/comments is wrong; do NOT revive it.

### Phase E -- on-chain wiring for SEC-048 (recommended commit shape)

This is **~400 lines** of Rust + ~150 lines of TS + script
updates.  Below is the order that minimises blast radius and keeps
the tree buildable at every commit boundary.

#### E.1 -- Refactor: lift `verify_groth16_proof` + `VkBuf` + `negate_g1_point` to `solid-light::groth16`

**Why a refactor instead of duplication:** SEC-048's verify path
needs the same alt_bn128 syscall machinery zk-verifier already
has, but for a different `NR_PUBLIC_INPUTS` value (2 instead of
33).  Three options were considered:

- **CPI from issuer-registry to a new ix on zk-verifier**:
  rejected.  CPI overhead (~50K CU) + trust-boundary noise + ix
  surface bloat in zk-verifier.
- **Duplicate the verify code in issuer-registry**: rejected.
  150 lines of cryptographic primitives duplicated across
  programs is a maintenance liability and a real-not-toy red
  flag.  L6 (doc lies) generalises here: "two copies of the
  same hot crypto path" is tomorrow's silent divergence.
- **Lift to `solid-light` and parameterise over const `N`**:
  CHOSEN.  Cleanest long-term home for shared crypto.

**File touchpoints:**

- New: `crates/solid-light/src/groth16.rs`
  - Contains `pub fn verify_groth16_proof<const N: usize>(...)`,
    `pub struct VkBuf<const N: usize>`, `pub fn negate_g1_point(...)`.
  - All adapted from `programs/zk-verifier/src/lib.rs:1073-1130`.
    Make `VkBuf` const-generic over `N` (the public-input count).
    `MAX_IC = N + 1` becomes a const expr inside the type.
  - Compile-time assertion the existing zk-verifier code carries
    (file:line `programs/zk-verifier/src/lib.rs:140-145`,
    `VkBuf` size < 1 KB stack budget) MUST move with it.  Use a
    `const _: () = ...` or `static_assertions::const_assert!`.
  - Re-export from `crates/solid-light/src/lib.rs`.
- Modify: `programs/zk-verifier/src/lib.rs`
  - Replace local `verify_groth16_proof` + `VkBuf` +
    `negate_g1_point` with `use solid_light::groth16::*;` and
    callsite parameterisation `verify_groth16_proof::<NR_PUBLIC_INPUTS>(...)`.
  - The local definitions can be deleted entirely.
  - Existing host test
    `programs::zk-verifier::src::lib.rs::tests::groth16_host_verify_round_trip`
    (file:line ~2204) must still pass; the underlying logic is
    unchanged, just relocated.
- No change to: any consumer of `zk-verifier`'s public ix surface.

**Regression test:** `cargo test -p zk-verifier --lib` MUST stay
at 42/42.  The existing
`groth16_host_verify_round_trip` consumes
`tests/fixtures/groth16_e2e_proof.json` (32 publicSignals,
NR_PUBLIC_INPUTS=32 today) and verifies host-side.  After the
refactor, this test is the witness that the move was clean.

**Commit:** `refactor: lift Groth16 verify primitives to solid-light::groth16`.

#### E.2 -- New `SubgroupVerifierConfig` PDA + chunked VK upload ixs in `programs/issuer-registry/`

**Pattern source:** mirror `programs/zk-verifier/src/lib.rs`'s
`VerifierConfig` PDA + `initialize` + `store_verification_key` +
`finalize_verification_key` + `request_vk_rotation` +
`rotate_verification_key` ixs.  The 48h freeze-gate timelock from
ADR-0015 SHOULD apply identically to the subgroup VK -- it's the
same VK-rotation soundness concern.

**Concrete additions to `programs/issuer-registry/src/lib.rs`:**

- New PDA seed: `b"subgroup-verifier-config"`.
- New struct `SubgroupVerifierConfig` (mirror layout of
  `VerifierConfig`):
  - `authority: Pubkey` (32)
  - `paused: bool` (1)
  - `bump: u8` (1)
  - `next_vk_chunk: u16` (2)
  - `vk_finalized: bool` (1)
  - `vk_generation: u16` (2)  -- own generation counter for the
    subgroup VK
  - `rotate_request_ts: i64` (8)
  - SPACE: 32 + 1 + 1 + 2 + 1 + 2 + 8 = 47 bytes (no proof_count;
    no timestamp_skew_seconds -- subgroup verify has no Clock
    binding).
- New ixs:
  - `init_subgroup_verifier`
  - `store_subgroup_vk_chunk`
  - `finalize_subgroup_vk`
  - `request_subgroup_vk_rotation` (48h timelock; reuse the
    `VK_ROTATION_TIMELOCK_SECONDS` const from zk-verifier or
    duplicate as `SUBGROUP_VK_ROTATION_TIMELOCK_SECONDS`)
  - `cancel_subgroup_vk_rotation`
  - `rotate_subgroup_vk`
- Storage layout:
  - VK chunks live in a separate `subgroup_vk_storage` PDA seeded
    `[b"subgroup-vk-storage"]`.  Mirror zk-verifier's
    `VkStorage` pattern.

**Why a chunked-upload PDA instead of `include_bytes!` const:**

- Allows VK rotation without a program upgrade.
- Symmetric with zk-verifier's pattern; no new mental model.
- Operator runbook stays uniform.

**Trade-off:** ~200 lines of new code, but it's mostly mechanical
mirror-of-pattern work.  Each ix has an existing analog at known
file:line.

**Regression tests:** new host tests in
`programs/issuer-registry/src/lib.rs::tests`:

- `subgroup_vk_chunk_upload_idempotent` (re-upload same chunks
  works; cursor advances correctly).
- `subgroup_vk_finalize_locks_writes` (post-finalize,
  `store_subgroup_vk_chunk` fails with the freeze error).
- `subgroup_vk_rotation_timelock_enforced` (request +
  attempted-rotate-before-48h fails; rotate-after-48h succeeds).

**Commit:** `SEC-048 Phase E.2: SubgroupVerifierConfig PDA + chunked VK upload ixs`.

#### E.3 -- Modify `register_issuer` to consume + verify the subgroup proof; drop the bypass

**File:** `programs/issuer-registry/src/lib.rs`.

**Signature change:** add `subgroup_proof: Vec<u8>` argument
(256 bytes: proof_a [64] + proof_b [128] + proof_c [64];
optionally pack as a typed `[u8; 256]` once Anchor 0.30.1's
Vec<[u8; N]> deserialise issue is confirmed -- see LB1 in
SESSION_LOG_2026-04-29.md).  Public inputs are reconstructed
from the existing `bjj_pub_key_x` + `bjj_pub_key_y` ix args
(both already canonical-encoded by the SEC-062 gate).

**Verify path inside register_issuer:**

1. Read `SubgroupVerifierConfig` (assert `vk_finalized == true`,
   `paused == false`).
2. Read `subgroup_vk_storage` PDA contents.
3. Construct `public_inputs: [[u8; 32]; 2]` from
   `(bjj_pub_key_x, bjj_pub_key_y)` -- byte-order MUST match the
   circuit's expected encoding (LE -> BE conversion if the
   circuit uses LE; see SEC-062 + SEC-067 receipts for the byte
   contract).  **Don't assume; check** by running
   `groth16_host_verify_round_trip`-style host test against a
   captured fixture from `npm run prove` on the subgroup circuit
   first.
4. Call `solid_light::groth16::verify_groth16_proof::<2>(...)`.
5. On verify success: existing register_issuer logic.
6. On verify failure: reject with new
   `ErrorCode::InvalidSubgroupProof`.

**Bypass removal (the `sec007-skip-onchain` deletion):**

- Delete the `sec007-skip-onchain` feature from
  `programs/issuer-registry/Cargo.toml`.
- Delete the `#[cfg(feature = "sec007-skip-onchain")]` arm in
  `register_issuer` (around `programs/issuer-registry/src/lib.rs:189`).
- Delete the `Sec007Bypass` event declaration.
- Update `crates/solid-core/src/babyjubjub.rs::is_on_curve` and
  `is_identity` doc-comments (these stay; they're now a CHEAP
  always-on registration gate, not a "bypass consolation" gate).
- Delete `--features sec007-skip-onchain` from CLAUDE.md and
  AGENTS.md "Build sequence" + "End-to-end" runbooks.
- Delete from `docs/DEPLOYMENT_AND_TESTING.md`.

**CU expectations (predict, then measure):**

- Pre-batch baseline (current): `RegisterIssuer = 76,057 CU`.
- Predicted post-batch:
  - Drop bypass arm: `-3K to -5K`.
  - Add subgroup-VK-read + Groth16 verify:
    `+285K to +320K` (one alt_bn128 pairing; constant per proof).
  - Total: `~360-395K CU`.
- cuIx: bump from 200K (current) to 500K for safety margin.
- 1.4M per-tx ceiling: ~70% headroom.

**Regression tests:**

- New host test
  `register_issuer_rejects_invalid_subgroup_proof`: pass a
  random non-proof byte string, assert
  `InvalidSubgroupProof` is returned.
- New host test `register_issuer_accepts_valid_subgroup_proof`:
  generate a real proof for Base8 via snarkjs, feed through,
  assert success.
- Integration test (real validator): a torsion-tainted issuer
  pubkey CANNOT be registered.  Mirror the SEC-077 anchor
  rejection pattern.

**Commit:** `SEC-048 Phase E.3: register_issuer verifies subgroup proof; bypass dropped`.

#### E.4 -- SDK + script updates

- New helper in `ts-sdk/packages/issuer/src/index.ts`:
  ```ts
  export async function generateSubgroupProof(
    bjjPubKey: { x: Uint8Array; y: Uint8Array },
    artifacts: { wasm: string; zkey: string },
  ): Promise<{ proof: Uint8Array; publicSignals: [string, string] }>
  ```
  Uses `snarkjs.groth16.fullProve` against the subgroup
  zkey/wasm.  Format proof for Solana via the SEC-067 G2
  byte-order swap (`formatProofForSolana` from holder package).
- Modify `scripts/initialize.ts`:
  - Upload subgroup VK chunks via the new
    `init_subgroup_verifier` + `store_subgroup_vk_chunk` ixs.
  - Call `finalize_subgroup_vk` post-loop, mirroring the
    SEC-078 freeze-gate enforcement.
  - Pin sha256 read from
    `circuits/build/bjj_subgroup_verification_key.sha256` (mirror
    of SEC-041).
- Modify `scripts/bootstrap_issuer.ts`:
  - Before the `register_issuer` call, generate the subgroup
    proof using the new helper.
  - Pass the proof bytes into `register_issuer`.
  - The existing off-chain `isInPrimeOrderSubgroup` predicate
    stays as a UX/early-failure gate (now defense-in-depth, not
    load-bearing).
- Update `tests/cu_baselines.json`: `RegisterIssuer` expected
  ~360-395K (will be measured + auto-refreshed via
  `python3 scripts/measure_cu.py --write-baselines --yes`).
- Update `docs/CU_BUDGET.md`: replace the predicted ledger
  section's predictions with the actual measurements after the
  e2e green run.

**Commit:** `SEC-048 Phase E.4: issuer SDK + scripts upload + consume subgroup proof`.

#### E.5 -- E2E green + registry transition

- Run `bash scripts/sync_program_keypairs.sh --reset-state`,
  validator reset, anchor build (now WITHOUT
  `--features sec007-skip-onchain`), anchor deploy.
- `SOLID_VOTING_PERIOD_SECONDS=120 npm run e2e` -> reach
  `verified: true` end-to-end.
- Refresh CU baselines via `--write-baselines --yes`.
- Update `sec/SECURITY_REGISTRY.md`:
  - SEC-048 status: Open -> Fixed.  Add closure date 2026-05-XX,
    summary of Option-1 (registration-time subgroup proof),
    reference to the small-circuit ceremony VK pin.
  - Decrement open-HIGH count.
- Update `docs/CU_BUDGET.md` with measured numbers.
- Update `docs/CURRENT_STATE.md` with a 2026-05-XX session delta.
- Update CLAUDE.md / AGENTS.md "Open Phase 3 / Phase 4 scope"
  closure entries.

**Commit:** `SEC-048 close: bypass dropped + e2e green end-to-end`.
**Push.**

### SEC-006 Part 2 + SEC-051 -- the OTHER ceremony (separate from SEC-048's small circuit)

These two findings live in the **main batch circuit**
(`circuits/batch_credential_query.circom`), not in the SEC-048
small circuit.  They share a separate ceremony with its own
re-run.  Recommended order: land SEC-048 fully (Phases E.1
through E.5 above) BEFORE touching the batch circuit -- the
SEC-048 ceremony is independent and can land in isolation.  Once
SEC-048 is closed, batch the SEC-006 P2 + SEC-051 changes
together.

#### Combined batch-circuit changes

- **SEC-006 Part 2: bind `vk_generation` into circuit publics.**
  - Add `signal input vkGeneration;` to
    `batch_credential_query.circom` after `currentTimestamp` (the
    new slot becomes index 32 of public inputs;
    `NR_PUBLIC_INPUTS = 32 -> 33`).
  - The slot is recovered by the circuit but doesn't need to be
    constrained in any way internally; just exposing it as a
    public input is enough -- the on-chain verifier's check
    `public_inputs[32] == config.vk_generation` makes
    cross-VK replay impossible.
- **SEC-051: reject all-padding proofs.**
  - Add the prepared 3-line snippet (registry SEC-051 entry has
    the exact circom):
    ```circom
    component anySchemaSet = IsZero();
    anySchemaSet.in <== schemaHashes[0];
    anySchemaSet.out === 0;
    ```
  - Insert at the first STEP-0 constraint location in
    `batch_credential_query.circom`.

#### On-chain cascade (`programs/zk-verifier/src/lib.rs`)

- `NR_PUBLIC_INPUTS: usize = 32 -> 33`.
- `MAX_IC: usize = NR_PUBLIC_INPUTS + 1` auto-updates to 34.
- `RECONSTRUCTED_INPUT_SLOTS` array: append slot 32.
- Reconstruct slot 32 from
  `verifier_config.vk_generation` (already a `u16` field at byte
  offset 50-51; pad to 32 bytes BE).
- `WIRE_INPUT_SLOTS`: NO change -- vk_generation is reconstructed
  on-chain, NOT on the wire.
- New `ErrorCode::VkGenerationMismatch`: emitted when
  `public_inputs[32] != config.vk_generation` after public-input
  assembly.
- Test assertion at line ~1789 (`assert_eq!(NR_PUBLIC_INPUTS, 32)`)
  flips to 33.
- Stack buffer auto-resizes (`[[u8; 32]; NR_PUBLIC_INPUTS]`).

#### TS SDK cascade

- `ts-sdk/packages/verifier/src/index.ts:47`: `NR_PUBLIC_INPUTS = 32 -> 33`.
- `WIRE_INPUT_SLOTS`: unchanged (slot 32 not on wire).
- `extractWirePublicInputs()` assertion `publicSignals.length === NR_PUBLIC_INPUTS` updates from 32 to 33.
- `holder/generateBatchProof`: extend witness to include
  `vkGeneration` (read from on-chain `verifier_config` before
  proving).

#### Regression tests

- Circuit witness-tester in `circuits/test/`:
  - `vk_generation_propagates_through_publics`: prover witness
    sets vkGeneration=N -> public-input slot 32 == N.
  - `padding_zero_proof_rejected`: schemaHashes = [0,0,0,0]
    fails at the new IsZero gate.
- Host test in zk-verifier:
  - `vk_generation_mismatch_rejected`: build a proof with
    generation=N, flip on-chain `config.vk_generation` to N+1,
    verify fails with `VkGenerationMismatch`.
- E2E: `npm run e2e` reaches `verified: true` post-batch with
  the new VK pin.

#### Ceremony (re-run for batch circuit)

- `cd circuits && PATH="..." npm run compile && \
    PATH="..." node scripts/setup.js`  (default circuit, no flag).
- New zkey + VK + sha256 written to `circuits/build/`.
- The **PTAU file is shared** with the SEC-048 ceremony; no
  PTAU regen.
- New VK pin sha256: regenerate + commit.

#### Mainnet posture (do NOT skip)

For mainnet, the batch ceremony MUST be multi-party (SEC-012);
the SEC-048 ceremony MUST also be multi-party.  Same shared PTAU
+ same contributor pool, just two consecutive Phase-2s in the
same session.

**Commit shape (per L9):**

1. `SEC-006 P2 close: bind vk_generation into batch circuit publics + on-chain check`.
2. `SEC-051 close: reject all-padding proofs in batch circuit`.
3. `batch ceremony re-run + new VK pin + post-batch CU baselines`.

### ts-sdk jest tests baseline

This is BACKLOG Tier 2 #11.  Auditor-blocker; "auditors will
flag jest configured but zero `.test.ts` files".  Estimated
3-5 days of focused work; can land after SEC-048 closure.

#### Scaffolding (one-time)

- `ts-sdk/package.json`: add jest + ts-jest + @types/jest +
  @types/node as devDependencies.  Pin versions compatible with
  the workspaces' ESM setup (`"module": "ESNext"` in tsconfig).
- New `ts-sdk/jest.config.cjs` (or .mjs) at the workspace root
  with multi-project discovery covering all six packages.
- ts-jest preset OR babel-jest with the right TS+ESM config.
  jest's native ESM is fiddly; ts-jest with `useESM: true` is
  the standard recipe.
- New CI step in `.github/workflows/ci.yml::sdk` job to run
  `cd ts-sdk && npm test`.

#### Per-package tests (priority order)

1. **`@solid-protocol/core`**: `computeSchemaHash` determinism
   (same inputs -> same output) + cross-language vector match
   against the Rust reference.  `PROGRAM_PUBKEYS` shape +
   freeze.  Pure functions; easy.
2. **`@solid-protocol/sdk`**: `artifact_integrity.ts` hash-gate
   priority (env > sidecar > config) + tamper detection +
   missing-pin error.  File-based; easy with tmpdir fixtures.
3. **`@solid-protocol/light`**: `extract_active_issuer_tree_root`
   parser; rejects frozen / wrong-discriminator buffers.
   Pure-bytes; easy.
4. **`@solid-protocol/issuer`**: `computeIssuerLeaf` matches the
   on-chain Poseidon(5) consumed by `append_issuer_leaf`.
   Cross-language fixture comparison.  Once SEC-048 Phase E
   lands, also test `generateSubgroupProof` happy path.
5. **`@solid-protocol/holder`**: `generateBatchProof` invariants
   -- commitment determinism, schema-sort canonicality (proof
   is invariant under input permutation), edge cases on the
   Merkle replica.  More involved; needs zkey + wasm fixtures
   in tests.
6. **`@solid-protocol/verifier`**: `verifyOnChainV2` happy path
   + replay path + buffer-overflow path.  Needs RPC mocking;
   most invasive.  Defer to AFTER integration tests 02..11
   stand up the bankrun harness.

#### Non-goals (call out explicitly)

- **No real validator dependency** in jest tests.  RPC-bound
  flows belong in integration tests 02..11 (bankrun harness),
  not jest.
- **No coverage of WASM bridge byte-format** in jest -- that
  contract is gated by `cross_language_vectors` CI job and the
  Rust host tests, NOT by jest.

### Performance opportunities (consider, don't blindly apply)

These are notes for the next session.  Apply only if both
"keeps it correct" and "keeps it robust" gates are clearly met;
otherwise log + defer per L4 (regression gate FIRST).

#### Subgroup circuit (post-Phase E)

- **Hamming-weight win:** the current circuit uses 251 BabyDbl +
  115 BabyAdd (Hamming weight of `r`).  `r` cannot be changed,
  but a windowed double-and-add (e.g., 4-bit windows with
  precomputed multiples of P) would reduce additions at the cost
  of 16 precomputations per window.  Net savings unclear at
  this constraint scale; probably NOT worth it for a one-time
  per-issuer cost.
- **Skip the on-curve check?** No.  `BabyCheck` is ~5
  constraints and removing it weakens the gate (a non-curve
  point with the right projective coords could spoof identity
  on some implementations).  Keep it.

#### Batch circuit (post-SEC-006-P2)

- **Reconstruct slot 32 from `verifier_config.vk_generation`
  rather than wire it.**  Already designed this way in the
  handoff above; saves 32 bytes of ix data + matches the
  pattern for slots 1..10 + 29.

#### On-chain `verify_batch_proof_v2`

- **VK pre-parse + cache.**  Currently `VkBuf::parse` runs every
  call (~few thousand CU).  Caching the parsed `VkBuf` in a
  scratch PDA IF the VK hasn't been rotated would save those CU
  per call -- but adds rent + a stale-cache attack surface.
  Probably NOT worth it for v1; revisit if verify_batch_proof_v2
  CU breaches 500K.
- **Reduce alt_bn128 pairing cost?** Out of our control; that's
  the syscall.  Option C (`sol_babyjubjub_*` SIMD) is the only
  way to push that down.

#### `register_issuer` (post-Phase E)

- **Combine subgroup verify + canonical-encoding gate into one
  pass.**  Currently SEC-062 canonical-encoding gate is a
  pre-check; the subgroup verify reads the same bytes.  No
  meaningful CU saved by merging; keeps separation of concerns.

#### CU baseline policy (general)

The 10% tolerance in SEC-046 is generous; some baselines (e.g.,
`VoteOnIssuer`) have shown +7% transient drift across runs.
Consider tightening to 5% post-mainnet once stability is proven.

### What MUST NOT happen during Phase E

These are the L1-L9 + audit-specific landmines for this batch:

1. **Do NOT re-introduce `[8] * P == P` framing.**  It's
   mathematically wrong; the corrected spec is locked.
2. **Do NOT re-add `sec007-skip-onchain` as a "fallback".**  L3
   (no workarounds): if Phase E hits a hard blocker, debug the
   blocker, do NOT keep the bypass.  CI gates from SEC-081 will
   reject re-introduction anyway.
3. **Do NOT skip the host-side Groth16 verify test before
   on-chain wiring.**  L8 / 4.1 (don't speculate; observe):
   first capture a fresh proof from `node scripts/setup.js
   --circuit bjj_subgroup_proof`-derived artefacts via snarkjs,
   run host-verify, observe pass; THEN write on-chain code.
4. **Do NOT bundle SEC-006 P2 + SEC-051 with SEC-048 in one
   ceremony.**  They're SEPARATE circuits.  One PTAU shared,
   yes; one Phase-2 each, no.
5. **Do NOT regenerate the in-tree
   `circuits/build/verification_key.sha256`** without first
   updating CLAUDE.md's "Hard invariants" section -- the pin
   is load-bearing for `scripts/initialize.ts`'s SOLID-SEC-041
   gate.
6. **Do NOT commit `circuits/build/*` artifacts.**  The
   directory is gitignored.  Pin hashes only.
7. **L9 strict per-finding commits.**  SEC-048 closure is 5
   commits (E.1 through E.5).  SEC-006 P2 + SEC-051 are 3
   commits (one each + ceremony).  ts-sdk jest is 1-N commits
   (per package).  Don't bundle to "save commit hygiene"; the
   git history IS the audit trail.

### Reproducing this session's artefacts (sanity check on Phase D-1)

```
cd /Users/rajakash/Desktop/testing/solid-protocol
PATH="$(pwd)/.toolchain/bin:$PATH"

# 1. Compile the subgroup circuit.
cd circuits
circom bjj_subgroup_proof.circom --r1cs --wasm --sym --c \
       --output build/ -l node_modules/circomlib/circuits

# 2. Verify constraint count.
# Expected: ~2,191 non-linear + 7 linear = ~2,198 R1CS.
# (Will print as part of compile output.)

# 3. Run the trusted setup (reuses pot_final.ptau).
node scripts/setup.js --circuit bjj_subgroup_proof
# Expected output:
#   VK   sha256 : <fresh hash; differs each run>
#   zkey sha256 : <fresh hash; differs each run>
#   VK          : circuits/build/bjj_subgroup_verification_key.json
#   VK sha256   : circuits/build/bjj_subgroup_verification_key.sha256
#   zkey        : circuits/build/bjj_subgroup_proof_final.zkey

# 4. End-to-end snarkjs prove + verify smoke.
cat > /tmp/subgroup_input.json <<'EOF'
{
  "Ax": "5299619240641551281634865583518297030282874472190772894086521144482721001553",
  "Ay": "16950150798460657717958625567821834550301663161624707787222815936182638968203"
}
EOF
cd build/bjj_subgroup_proof_js
node generate_witness.js bjj_subgroup_proof.wasm /tmp/subgroup_input.json /tmp/sg.wtns
cd ..
snarkjs groth16 prove bjj_subgroup_proof_final.zkey /tmp/sg.wtns /tmp/sg_proof.json /tmp/sg_pub.json
snarkjs groth16 verify bjj_subgroup_verification_key.json /tmp/sg_pub.json /tmp/sg_proof.json
# Expected: [INFO]  snarkJS: OK!
```

If steps 1-4 all pass, the Phase A/B/D-1 baseline is intact and
Phase E can proceed.

## Pickup tomorrow (2026-04-30 morning)

**Before any code change**: read `plan/SESSION_LOG_2026-04-29.md`
end-to-end, especially §5 (learnings).  The discipline rules L7,
L8, L9 in that section are new and load-bearing for tomorrow's
arc -- they exist precisely because this session violated them.

### Step 1.  Commit-split the WIP (atomic, per finding)

Per L9 of the session log: the current uncommitted WIP bundles
the B13 reconstruction, three latent-bug fixes (LB1, LB2, LB3),
the ALT helpers, and doc updates into one diff.  Split into:

1. Doc-only corrections (arc 1 work).  Cheapest review.
2. Options archive doc.  Pure new file.
3. LB2 (timestamp BE/LE fix).  Single-file zk-verifier change +
   regression test that asserts a BE-encoded u64 round-trips.
   Promote to its own SEC-XXX entry (sibling of SEC-031).
4. LB3 (Merkle-root BE/LE fix).  Multi-file (handler + tests).
   Sibling SEC-XXX.
5. LB1 + B13 (1) -- the wire shrink + Vec<u8> fix + reconstruction.
   These are coupled.  Promote to **SOLID-SEC-054** with the
   "(1)+(3) ceiling, pivot to Option 2 in next commit" caveat in
   the registry entry.

Each commit lands its regression gate in the same commit (L4).

### Step 2.  Implement B13 Option 2 (buffer-account / chunked upload)

The architectural escape valve documented at
`docs/REMEDIATION_OPTIONS_ARCHIVE.md` §1.1 + `docs/E2E_BLOCKERS.md`
B13 ("(2) Buffer-account upload").  See `plan/SESSION_LOG_2026-04-29.md`
§2 for the full design.  Short form:

1. Add three new ixs to `programs/zk-verifier/src/lib.rs`:
   - `init_proof_buffer(payer)` -- allocates a scratch PDA
     `[b"proof-buffer", payer.key]`, ~1024 bytes.
   - `upload_proof_chunk(buffer, offset: u32, bytes: Vec<u8>)` --
     writes bytes into the buffer; idempotent on
     re-upload-same-bytes.
   - `verify_batch_proof_v2(buffer)` -- reads staged proof from
     buffer, performs B13 reconstruction (reuse the existing
     extract helpers), runs Groth16, atomically inits nullifier
     PDA, closes buffer.
2. SDK orchestration in `verifyOnChain`:
   - `ensureLookupTable` (already implemented; reuse)
   - `init_proof_buffer` (1 tx, fits trivially)
   - `upload_proof_chunk` * N (~2 chunks of ~700 bytes each)
   - `verify_batch_proof_v2(buffer)` (1 tx, small enough for
     cuIx + ALT + the proof-buffer PDA reference)
3. Regression gates:
   - cargo unit test: chunked upload assembles to byte-identical
     payload as the legacy direct-tx wire.
   - cargo unit test: missing chunk in buffer -> `_v2` rejects.
   - integration test (when bankrun harness lands): full 3-tx
     flow -> `verified: true`.
4. Run `npm run e2e`; assert `verified: true` tail.
5. Promote SEC-054 from "Open (interim partial)" to "Fixed" once
   green.  "Verified" after one full sprint of the integration
   regression test in CI per CLAUDE.md non-negotiable #2.

### Step 3.  Arc 4 -- SOLID-SEC-046 (CU regression gate)

Spec is fully laid out in
`sec/audits/2026-04-26_v0.6.1_modular_audit/04_compute/cu_budget.md`
§5.  Implementation, not design.

- Generate `tests/vectors/cu_baseline_proof.json` (deterministic
  test vector; record `(proof_a, proof_b, proof_c, public_inputs,
  nullifier)` from a successful Option-2 e2e run).
- Write `docs/CU_BUDGET.md` with starter baselines for
  `verify_batch_proof_v2`, `register_issuer` (bypass build),
  `append_issuer_leaf`, `revoke_issuer_atomic`,
  `request_withdrawal_atomic`, `issue_credential`.  Schema in
  cu_budget.md §5.
- New CI step: `setComputeUnitLimit({ units: 800_000 })`; submit
  the test-vector proof; parse `consumed X of 800000 compute
  units` from `solana confirm`; assert `X <= baseline * 1.10`.

### Step 4.  Arc 5 -- SOLID-SEC-048 Option E (in-circuit subgroup constraint + ceremony)

Major arc.  See task #5 description.  Shape A (just SEC-048 Option
E) was the agreed scope; SEC-006 Part 2 / SEC-051 / predicate-operand
range checks are co-traveler candidates that should NOT be batched
unless the user explicitly says yes (this is a constraint surface
that gets harder to review the bigger it gets).

Steps:

1. Modify `circuits/batch_credential_query.circom`: add an
   in-circuit subgroup check on the issuer pubkey witness (one
   EdDSA-style scalar mul, ~30K constraints, ~250ms proving cost
   per the cu_budget audit §4).
2. Re-run `node circuits/scripts/setup.js`.  New
   `circuits/build/verification_key.json` + `.zkey`.  Update
   `circuits/build/verification_key.sha256`.
3. Anchor program changes:
   - Remove `sec007-skip-onchain` Cargo feature in
     `programs/issuer-registry/Cargo.toml`.
   - Remove the `#[cfg(not(feature = "sec007-skip-onchain"))]`
     gate around `require_in_prime_order_subgroup` in
     `register_issuer`.  Decision: keep `is_on_curve +
     !is_identity` as cheap defense-in-depth (~3.5K CU); the
     in-circuit constraint is the load-bearing soundness gate.
   - Remove `Sec007Bypass` event.
4. Bootstrap script change: keep the SDK-side
   `isInPrimeOrderSubgroup` predicate as fail-fast UX (no longer
   load-bearing soundness, but a friendly pre-submit error).
5. Witness-tester regression: torsion-tainted issuer pubkey ->
   witness gen aborts on the new subgroup constraint.
6. Cross-language vector (`tests/vectors/subgroup_check.json`):
   closes 1 of 7 SOLID-SEC-010 missing primitives.
7. Registry: SEC-048 -> Closed; SEC-007 re-flipped from
   "host-only" to "in-circuit" closure; SEC-052 partial close
   completes (the `pubkey_to_affine` BPF-iso path becomes dead
   code).
8. CLAUDE.md hard-invariants update for the new VK pin sha256.

Once SEC-048 closes, the `register_issuer` localnet/devnet
runbook drops the `--features sec007-skip-onchain` step; mainnet
posture is unblocked.

### Step 5.  Arc 6 -- full sweep + e2e

`cargo test -p solid-core -p solid-light -p zk-verifier
-p issuer-registry -p schema-registry`; `cargo fmt + clippy
-p solid-core -p solid-light -- -D warnings` (after the
pre-existing dead-code cleanup the user said they would do
separately); `bash scripts/sync_program_keypairs.sh && anchor
build --no-idl`; `npm test` in circuits and ts-sdk; full
`npm run e2e` to `verified: true`.

Update `plan/RESUME.md` with the receipts.

### Step 6.  After arcs 4-5-6

- SOLID-SEC-010 cross-language vectors 3 -> 10 (closes the
  remaining primitives that didn't get vectored as side-effects
  of the above arcs).
- Integration suite 02..11 (per `tests/integration/README.md`).
  Bankrun + jest harness stand-up.
- SOLID-SEC-051 (one-constraint padding fix) -- batches with the
  next sanctioned trusted-setup cycle if SEC-048 ceremony was
  Shape A; already shipped if Shape B/C was chosen.
- M02-H02 (CI gate against shipping `sec007-skip-onchain` to
  mainnet) -- becomes moot once SEC-048 Option E lands; can be
  retired.
- AUDIT-2026-04-28-002 (integration tests 02..11 incomplete) and
  AUDIT-2026-04-28-003 (mutable action tags in CI) from the
  2026-04-28 audit follow-up.

### Step 7.  Pre-existing CI breakage (separate commit)

The user has explicitly asked to handle the pre-existing
`cargo clippy -p solid-core -- -D warnings` failures (5 dead-code
warnings, mostly the `attester` field of `SasAttestationBuilder`)
as their own commit.  Do NOT bundle with B13 / SEC-046 / SEC-048.

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
