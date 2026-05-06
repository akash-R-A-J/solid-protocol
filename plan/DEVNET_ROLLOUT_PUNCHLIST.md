# SolID Devnet Rollout Punchlist

Status: Active. Tactical task list.
Last updated: 2026-05-06.

## Commitment

**Tier A and Tier B items in this document are mandatory before public
devnet launch.** Tier C is mandatory before the protocol is described as
"production-quality" in any partner-facing material. Tier D is mainnet
scope and explicitly **not** a devnet blocker.

Companion docs:

- `plan/PRODUCT_SURFACE_DEVNET_LAUNCH_PLAN.md` -- strategic positioning,
  competitive analysis, demo design.
- `plan/INTEGRATION_AND_TEST_STATUS_2026-05-02.md` -- empirical
  test/integration audit at HEAD `3671943`.
- `plan/VERIFIER_SDK_SHAPE.md` -- design sketch for the high-level
  verifier wrapper.
- `docs/CREDENTIAL_DELIVERY_DESIGN.md` -- issuer-to-holder package
  format and delivery channels.
- `docs/HOLDER_STORAGE_AND_WALLET.md` -- holder storage architecture
  and Wallet Standard integration spec.
- `docs/SDK_INTEGRATOR_MATRIX.md` -- per-actor SDK package surface.
- `../solid-wallet/REAL_WALLET_DEPLOYMENT_CHECKLIST.md` -- wallet
  runtime config, issuer/verifier usage, and real holder deployment gate.

## Conventions

Each row carries:

- **Effort** -- single-developer estimate, in calendar days.
- **Dependency** -- what must finish before this can start. `none` means
  parallelizable today.
- **Acceptance** -- a single sentence that, if true, the task is done.
  No fuzzy "shipped" claims.

Status legend (mirrors `docs/CURRENT_STATE.md`):

- `[ ]` open -- not started.
- `[~]` partial -- some scaffolding, blocked or incomplete.
- `[X]` closed -- shipped + acceptance test green.

---

## Current ordered devnet blockers -- 2026-05-06

This section is the current "what is left, in order" list. It supersedes
older product-side wording that assumed the verifier SDK and holder wallet
did not exist yet, and it reflects the partial public devnet protocol
deploy completed on 2026-05-06.

### 0. Freeze the devnet source of truth

- **What:** Keep `deployments/devnet.json`, `docs/DEVNET_STATUS.md`,
  wallet constants, console constants, and SDK defaults aligned on the
  same program IDs, schema hashes, tree addresses, artifact URLs, and
  artifact pins.
- **Why:** Every other step depends on all clients proving against the
  same verifier program, schema registry, issuer registry, circuit
  artifacts, and tree roots. If these disagree, failures look like proof
  bugs but are actually configuration drift.
- **Acceptance:** One config manifest can be consumed by protocol
  scripts, `@solid-protocol/verifier`, `solid-wallet`, and
  `solid-console` without manual copy/paste. Current state:
  `deployments/devnet.json` has live program/schema/tree fields, explicit
  existing-sample verify evidence, and null artifact/indexer/console/wallet
  URLs. Separate localnet/devnet env examples now exist for protocol
  scripts, solid-sim, and the extension wallet.

### 1. Upgrade issuer registry and complete fresh devnet proof smoke

- **What:** `schema-registry`, `issuer-registry`, and `zk-verifier` are
  deployed; registry/verifier/VK state is initialized; `basic_identity_v2`
  issuer/credential/local-proof smoke is green; existing-sample on-chain
  verify is green. Finish by upgrading `issuer_registry` from the current
  source, granting the issuer/schema permission, rerunning
  `npm run issue && npm run prove`, and recording a fresh devnet
  `verify_batch_proof_v2` transaction plus replay rejection.
- **Why:** Program deployment alone is not enough for public testers. The
  public claim needs fresh issuance and verification through the current
  deployed program/source contract.
- **Acceptance:** `deployments/devnet.json` and `docs/DEVNET_STATUS.md`
  contain a fresh issue tx, verify tx, and failed replay evidence. Existing
  sample verify tx already recorded:
  `56crrCtH7QDAQbgrqiHuytuJVskXiRm27LvBRGBCBLGXM4k7q9nXW2oHhnd3U9rmxTBQzHMEcHuuEk84Ezy1VNut`.

### 2. Register the launch schema set

- **What:** Register launch schemas on devnet with credential tree depth
  20 and record each schema PDA, schema hash, field list, and tree.
- **Why:** Issuers and verifiers need a shared schema contract. Wallet
  proof previews and verifier requirement encoding are only meaningful if
  field names, indices, and hashes are canonical.
- **Acceptance:** the launch schemas are queryable from the schema
  registry / indexer API and match the repo schema JSON byte-for-byte.
  Current smoke schema is `basic_identity_v2`; do not reuse the early
  depth-16 `basic_identity_v1` binding for batch-circuit proofs.

### 3. Initialize and expose the compressed-state trees

- **What:** Create and record:
  - Per-schema credential tree for each launch schema.
  - Global identity-state tree.
  - Singleton issuer Merkle tree.
  - Current issuer-tree root.
- **Why:** Holder proof generation needs inclusion paths for credentials,
  identity state, and issuer status. Without these addresses and roots,
  the wallet cannot produce real proofs.
- **Acceptance:** Tree addresses and current roots are present in the
  devnet manifest/status API and can be read by wallet/console config.

### 4. Host pinned circuit artifacts

- **What:** Host:
  - `batch_credential_query.wasm`
  - `batch_credential_query.zkey`
  - `verification_key.json`
  - subgroup wasm/zkey/VK files
  with SHA-256 pins.
- **Why:** The SDK and wallet now enforce artifact integrity. This is what
  prevents a malicious or stale host from feeding holders the wrong
  circuit bytes.
- **Acceptance:** Public HTTPS URLs serve the artifact bytes and the
  hashes match `docs/DEVNET_STATUS.md` / sidecar `.sha256` files.

### 5. Deploy the Merkle proof indexer

- **What:** Run an indexer that watches credential issuance, identity
  state, issuer approvals/revocations, tree root updates, and exposes
  Merkle proof endpoints consumed by the wallet.
- **Why:** The wallet uses real `@solid-protocol/holder` proof
  generation. It cannot invent Merkle paths and should not fall back to
  stubs.
- **Acceptance:** Given a credential commitment, identity leaf, or issuer
  leaf, the indexer returns a non-stub proof with root/siblings/path
  indices matching the current devnet tree state.

### 6. Publish or link the SDK packages for integrators

- **What:** Publish `@solid-protocol/verifier@0.2.0` and
  `@solid-protocol/sdk@0.3.0`, or document a temporary local-package
  install path for devnet pilots.
- **Why:** The high-level wrapper code exists, but external developers
  need an installable integration surface. Without this, partners still
  have to monorepo-link.
- **Acceptance:** A fresh app can install the SDK and call
  `defineRequirement`, `requestProof`, and `verifyRequirement` without
  importing local files from this repo.

### 7. Finish `solid-console` as the real control plane

- **What:** Replace demo/simulation paths with real registry, issuer,
  issuance, verifier, schema, and status flows.
- **Why:** Console is the operator/admin/product surface. Without it,
  devnet can work technically but still feels like scripts plus docs.
- **Acceptance:** From the console, an operator can register/view schemas,
  register/approve issuers, issue a credential to a wallet-derived holder
  key, inspect roots/indexer health, build verifier requirements, and run
  a proof request without editing scripts.

### 8. Configure and test `solid-wallet` against devnet

- **What:** Load the built extension, configure indexer URL, artifact base
  URL, global state tree, issuer Merkle tree, and issuer tree root; import
  a real holder credential package.
- **Why:** The wallet code path is now real, but it needs live
  infrastructure. This is the difference between "builds" and "proves."
- **Acceptance:** The wallet can receive a verifier proof envelope, show a
  disclosure preview, generate a Groth16 proof, and return serialized
  proof bytes to the requesting app.

### 9. Wire issuer flow to wallet-derived holder keys

- **What:** Issuer UI/CLI must call
  `window.solid.getHolderPublicKey(schemaHash)` and issue credentials to
  that BabyJubJub public key.
- **Why:** Issuing to guessed or random holder keys breaks identity
  cohesion in the holder SDK. Real credentials must target the holder key
  derived by the wallet for the schema.
- **Acceptance:** A credential issued by console/issuer CLI imports into
  the wallet and passes holder SDK identity-cohesion checks during proof
  generation.

### 10. Wire verifier flow through `@solid-protocol/verifier`

- **What:** Verifier apps and console verifier pages should build
  `Requirement` objects and send `ProofRequestEnvelope` requests to the
  wallet instead of hand-rolling query JSON.
- **Why:** The SDK is the stable product surface. It owns field encoding,
  action scoping, artifact pins, public input shape, and typed errors.
- **Acceptance:** Console/demo verifier code no longer constructs raw
  proof requests manually; it uses `SolidVerifier.defineRequirement` and
  `requestProof` / `verifyRequirement`.

### 11. Run one real devnet E2E from scratch

- **What:** Execute the full external-user path:
  deploy -> register schema -> approve issuer -> ask wallet for holder
  key -> issue credential -> import into wallet -> request proof ->
  approve -> verify on-chain -> replay rejects.
- **Why:** Unit/build success is not enough for a ZK protocol. The public
  devnet claim needs one full, recorded transaction path through all
  deployed systems.
- **Acceptance:** A clean run produces a successful devnet verification
  signature, a failed replay attempt, and updated docs/status entries.

### 12. Publish quickstarts and demo surface

- **What:** Update public docs, examples, and demo deployment:
  verifier quickstart, issuer guide, wallet setup, console guide, error
  codes, and private launchpad gate.
- **Why:** Devnet only matters if outside developers can reproduce it
  without asking the core team for hidden steps.
- **Acceptance:** A developer can go from empty app to verified proof
  using the docs and deployed systems.

---

## Tier A -- Cannot ship a public devnet demo without these

### A1. First sanctioned devnet deploy

- **Status:** [~]
- **Effort:** 1 day (assuming no surprises).
- **Dependency:** none.
- **Acceptance:** `deployments/devnet.json` has non-null `deployed_at`,
  non-null `deployer.address`, non-null `upgrade_authority`; the three
  program IDs resolve to executable account data on
  `https://api.devnet.solana.com`; `scripts/check_program_ids.py` green
  against the deployed manifest. Remaining acceptance gap: known-good
  on-chain verify tx and replay-rejection evidence.
- **Substeps:**
  1. Provision a deployer keypair (or Squads address) with sufficient
     SOL on devnet.
  2. `bash scripts/sync_program_keypairs.sh` (idempotent).
  3. `cargo build-sbf` for all three programs (avoids the IDL
     proc-macro2 trap; documented in `CLAUDE.md`).
  4. `solana program deploy target/deploy/<prog>.so --program-id
     target/deploy/<prog>-keypair.json --url devnet` for each program.
  5. `scripts/initialize.ts` against devnet (registry config + main VK
     chunked upload + finalize + subgroup VK chunked upload + finalize).
  6. Smoke test: `register_issuer` against devnet with a throwaway BJJ
     keypair and 256-byte subgroup proof; expect success.
  7. `python3 scripts/regen_devnet_manifest.py > deployments/devnet.json`.
  8. Commit + tag `devnet-v0.6.1-deploy-1`.

### A2. Hosted artifact CDN

- **Status:** [ ]
- **Effort:** 2 days.
- **Dependency:** none (can run in parallel with A1).
- **Acceptance:** every artifact in `circuits/build/` (six files: 2
  wasm, 2 zkey, 2 VK) is reachable at a stable URL of the form
  `https://artifacts.solid.example/v0.6.1/<filename>`; each URL serves
  the file with `Content-SHA-256` header matching the pinned hash; URL
  is content-addressed AND version-prefixed (so future VK rotations do
  not break old SDK versions).
- **Implementation notes:**
  - Cloudflare R2 + Workers, or AWS S3 + CloudFront, or any other
    static host with HTTPS + cache headers.
  - The pin chain in `ts-sdk/packages/sdk/src/artifact_integrity.ts`
    already enforces SHA-256 verification client-side; the CDN does
    not need to be trusted for integrity, only for availability.
  - Add a `.well-known/solid-protocol.json` at the host root that
    declares the canonical pin manifest -- lets external tooling
    discover the artifacts without reading our docs.

### A3. `@solid-protocol/verifier` high-level wrapper

- **Status:** [~] (code path implemented locally:
  `SolidVerifier`, `defineRequirement`, `requestProof`,
  `verifyProof`, `verifyRequirement`, typed errors, artifact pin
  loading, and README. Still needs package publishing and devnet E2E
  acceptance before flipping to `[X]`).
- **Effort remaining:** 1-2 days for publish/release polish once devnet
  defaults exist.
- **Dependency:** A2 (so default config can resolve artifact URLs).
- **Acceptance:** `plan/VERIFIER_SDK_SHAPE.md` "Acceptance criteria" all
  hold. Specifically: a new developer can `npm install` and reach
  `result.verified === true` in under 15 minutes against devnet
  starting from the published quickstart.
- **Remaining substeps:**
  1. Point default devnet config at real artifact/indexer URLs.
  2. Publish `@solid-protocol/verifier@0.2.0`.
  3. Publish/update `@solid-protocol/sdk@0.3.0`.
  4. Run the 15-minute quickstart from a fresh app against devnet.

### A4. Holder claim/prove web MVP

- **Status:** [~] (`solid-wallet` extension now has real credential
  import, encrypted-at-rest storage, pending proof approvals, holder SDK
  proof generation, artifact pin checks, and injected provider methods.
  Still needs real devnet config, real issuer-issued credential package,
  and live E2E acceptance).
- **Effort remaining:** 2-4 days after A1/A2/B1 are live.
- **Dependency:** A3 partial (the Holder-side methods land in step 5
  of A3); can start scaffolding the UI before then.
- **Acceptance:** A non-technical user can: (1) connect a wallet, (2)
  paste a claim link or upload a credential package, (3) decrypt and
  see the credential's schema/issuer/expiration, (4) accept a proof
  request from a verifier app, (5) see a plain-language disclosure
  preview, (6) generate and submit the proof. All without reading
  protocol documentation.
- **Remaining substeps:**
  1. Configure wallet against live `indexerUrl`, `artifactBaseUrl`,
     `globalStateTree`, `issuerMerkleTree`, and `issuerTreeRoot`.
  2. Import a real issuer-generated holder credential package.
  3. Prove against a real verifier request envelope.
  4. Add backup/restore and claim-link decrypt if required for the
     public demo path.

### A7. `solid-console` real devnet control plane

- **Status:** [ ]
- **Effort:** 5-8 days.
- **Dependency:** A1 + A3 + A4 partial + B1 for live status/proof data.
- **Acceptance:** The console can operate the public devnet without demo
  simulations: schema browsing, issuer registration/status, credential
  issuance to wallet-derived holder keys, verifier requirement builder,
  proof request flow, and infrastructure health panels all hit real
  deployed systems.
- **Implementation order:**
  1. Replace demo issuer registration with real issuer-registry calls.
  2. Replace demo credential issuance with real `@solid-protocol/issuer`
     issuance targeting `window.solid.getHolderPublicKey(schemaHash)`.
  3. Replace hardcoded schema fields with schema catalog/registry data.
  4. Wire verifier pages to `@solid-protocol/verifier`.
  5. Add devnet config/status page for program IDs, artifact pins,
     tree addresses, indexer lag, issuer tree root, and schema roots.
  6. Add error surfaces based on `VerificationError` and registry
     errors instead of generic simulation messages.

### A5. Issuer CLI

- **Status:** [ ]
- **Effort:** 3 days.
- **Dependency:** A1 (so it can target a real devnet).
- **Acceptance:** A test issuer can run `npx @solid-protocol/issuer-cli
  register --metadata ./issuer.json --stake 1000` and reach approved
  status without reading any script in `scripts/`. The CLI also
  supports `issue`, `revoke`, `rotate-metadata`, and
  `withdraw-after-revoke`. All flags are documented in `--help`.
- **Implementation notes:**
  - Wraps `scripts/bootstrap_issuer.ts` and the issuer-side methods
    of `@solid-protocol/issuer`.
  - Generates BJJ keypair locally OR loads from an HSM/KMS
    integration (the latter is v1.1 -- ship local-keypair only for
    now and document the path to KMS).
  - Auto-runs `generateSubgroupProof` (Phase E.4 SDK helper) before
    submission; surfaces the on-chain `verify_groth16_proof::<2>`
    rejection as a typed error if the subgroup proof fails.
  - Includes `issuer-cli health` to check registry status,
    enrollment in issuer tree, stake balance, recent events.

### A6. `examples/private-launchpad-gate` Next.js demo

- **Status:** [ ]
- **Effort:** 3-5 days.
- **Dependency:** A3 (verifier wrapper) + A4 (holder app to receive
  proof requests) + A5 (issuer CLI to mint demo credentials).
- **Acceptance:** Deployed at a public URL (e.g.
  `launchpad-demo.solid.example`); a visitor with a fresh wallet can
  (1) request a demo credential from the bundled faucet issuer, (2)
  click "Prove eligibility" on the launchpad page, (3) see the
  disclosure preview, (4) approve, (5) get into the gated section.
  A separate "revocation demo" path shows the same flow failing after
  the issuer is revoked. End-to-end happy path under 90 seconds.
- **Implementation notes:**
  - One sample requirement: `kyc_basic_v1` with predicates
    `kyc_level >= 1` AND `restricted_jurisdiction == 0`.
  - One bundled test issuer wallet (ephemeral, regenerated on
    each redeploy).
  - One bundled test schema, registered on first deploy.
  - Uses `@solid-protocol/verifier::verifyRequirement` with
    `walletAdapterTransport`.
  - "Revocation demo" button calls
    `issuer-cli revoke <credential-id>` server-side, then prompts
    the user to retry the proof (which now fails).

---

## Tier B -- Need before partner pilots feel real

### B1. Hosted Merkle proof / indexer API

- **Status:** [ ]
- **Effort:** 5-7 days.
- **Dependency:** A1 (real devnet to index).
- **Acceptance:** A standalone service, deployed at
  `https://indexer.solid.example`, exposes:
  - `GET /v1/merkle-proof/:tree/:leaf` -> Merkle inclusion path for an
    indexed credential commitment.
  - `GET /v1/issuers/:issuer/proof` -> Merkle inclusion path against the
    current issuer tree when an issuer leaf has been indexed.
  - `GET /v1/credential-requests` plus `POST`/`PATCH` request lifecycle
    endpoints for holder-to-issuer credential requests.
  - `GET /v1/issuers` -> paginated list with status/stake/metadata.
  - `GET /v1/schemas` -> paginated list with hash/fields/version.
  - `GET /v1/health` -> commitment lag, last indexed slot, error
    rates.
- **Implementation notes:**
  - Today the holder runs `LocalReplicaAdapter` against the
    validator log (via `@solid-protocol/light`). For devnet, that
    does not scale -- public devnet RPCs do not stream the events
    we need.
  - Build on top of Helius/Triton or a self-hosted indexer with
    Geyser plugin support. Process `MintToCollectionV1` /
    `Append` SPL AC instructions and reconstruct Merkle paths
    on demand.
  - Cache aggressively: Merkle paths only change on `update_*_root`
    transitions, so a 10-second cache is safe.
  - Expose Server-Sent Events at `/v1/events` for indexer-grade
    subscribers (operator dashboards).

### B2. Schema registry browser UI

- **Status:** [ ]
- **Effort:** 3 days.
- **Dependency:** B1.
- **Acceptance:** A web page at `https://schemas.solid.example` lists
  every schema registered on devnet, showing name, version, hash,
  fields, registration timestamp, registering authority, and current
  tree pubkey. Searchable + filterable. Each schema has a permalink
  with a copy-to-clipboard button for the schema hash.
- **Why:** Apps need to know what they can verify before they can
  define a `RequirementSpec`. Without this, every integrator asks
  "which schemas exist?" via Slack/email.

### B3. Issuer health / status page

- **Status:** [ ]
- **Effort:** 3 days.
- **Dependency:** B1.
- **Acceptance:** A web page at `https://issuers.solid.example` lists
  every registered issuer, showing authority pubkey, BJJ pubkey,
  status (Pending/Approved/Cooldown/Revoked), stake, registration
  date, vote count if Pending, recent events. Each issuer has a
  permalink and a "trust this issuer" button that copies the pubkey
  for use in a `RequirementSpec.trustedIssuers` array.
- **Why:** Apps decide which issuers they trust at the
  RequirementSpec level. Without a discoverability surface, the
  trust-network moat from
  `plan/PRODUCT_SURFACE_DEVNET_LAUNCH_PLAN.md` does not form.

### B4. Devnet credential faucet

- **Status:** [ ]
- **Effort:** 2 days.
- **Dependency:** A1 + A5.
- **Acceptance:** A web page at `https://faucet.solid.example` allows
  a visitor to: (1) connect a wallet, (2) pick a sandbox schema
  (e.g. `kyc_basic_v1`), (3) request a credential, (4) receive an
  encrypted claim link. The faucet issuer is registered and
  approved on devnet; rate-limited to one credential per wallet per
  schema per 24 hours.
- **Why:** Without this, every integrator has to deploy their own
  issuer, register it, vote it through, and issue a credential to
  themselves before they can test the verifier flow. That breaks
  the "15-minute integration" promise.

### B5. Devnet integrator quickstart docs

- **Status:** [ ]
- **Effort:** 4-5 days total (one day per doc).
- **Dependency:** A3 + A4 + A5 (so the docs describe shipped
  surfaces, not aspirational ones).
- **Acceptance:** The following docs exist, each tested against a
  fresh checkout:
  - `docs/DEVNET_QUICKSTART.md` -- 15-minute path from `npm
    install` to verified proof.
  - `docs/VERIFIER_INTEGRATION.md` -- frontend + backend examples,
    error handling, replay scoping.
  - `docs/ISSUER_GUIDE.md` -- issuer CLI walkthrough, BJJ key
    management, credential templates.
  - `docs/HOLDER_FLOW.md` -- holder web app walkthrough, claim
    link mechanics, disclosure UX, backup/restore.
  - `docs/ERROR_CODES.md` -- canonical map from
    `VerificationError` enum to human-readable text and
    suggested actions.

### B6. 90-second product video + 5-minute developer quickstart video

- **Status:** [ ]
- **Effort:** 3 days.
- **Dependency:** A6 (need the demo to film against).
- **Acceptance:** Two videos uploaded to YouTube + embedded in
  README.md and the project landing page.
  - Product video: problem -> solution -> 30-second demo of the
    launchpad gate including the revocation case. No code shown.
  - Developer video: install -> defineRequirement -> verifyRequirement
    -> done. Maximum five minutes.

---

## Tier C -- Robustness gates before "production-quality" claim

### C1. SOLID-SEC-010 cross-language vectors 2/10 -> 10/10

- **Status:** [~] (2 of 10 primitives covered today).
- **Effort:** 1 day.
- **Dependency:** none.
- **Acceptance:** `tests/vectors/commitment_and_nullifier.json`
  expanded to ten primitive families: commitment, nullifier,
  schema_hash, BJJ pubkey derivation, EdDSA sign, EdDSA verify,
  identity-anchor leaf, issuer-tree leaf, BE/LE field encoding,
  predicate-evaluator. CI gate `cross_language_vectors` green for
  one full sprint.

### C2. Integration suite 02..11

- **Status:** [~] (1 of 11 implemented:
  `tests/integration/01_registry_init.test.ts`).
- **Effort:** 5-10 days, incremental.
- **Dependency:** none.
- **Priority order** (highest leverage first):
  - 03 -- slash transfers lamports
  - 04 -- root monotonicity
  - 06 -- verify happy path
  - 07 -- replay rejection
  - 08 -- forged global tree rejected (no machine gate today)
  - 09 -- forged schema tree rejected (no machine gate today)
  - 02, 05, 10, 11 -- per `tests/integration/README.md`.
- **Acceptance:** All 11 scenarios green in CI; `npm run
  test:integration` runner script committed to `package.json`.
- **Harness:** Use `litesvm-anchor` over `solana-bankrun` for lower
  friction (per the 2026-05-02 audit recommendation).

### C3. SEC-083 live constraint test (`12_subgroup_vk_init_authority_race.test.ts`)

- **Status:** [ ]
- **Effort:** 2 hours.
- **Dependency:** none.
- **Acceptance:** A bankrun test that calls `init_subgroup_verifier`
  with a non-registry-authority signer and asserts
  `ErrorCode::Unauthorized`; same test with the registry authority
  signer succeeds.

### C4. `withdraw_after_revoke` SDK helper + integration test 13

- **Status:** [ ]
- **Effort:** 2 hours.
- **Dependency:** none.
- **Acceptance:** `@solid-protocol/issuer::withdrawAfterRevoke()`
  callable from the issuer CLI; bankrun test
  `13_withdraw_after_revoke.test.ts` exercises the 24-hour cooldown
  happy path and the early-withdrawal rejection.

### C5. ts-sdk jest baseline

- **Status:** [ ] (jest declared in 6 package.json; zero
  `*.test.ts` files; jest not installed).
- **Effort:** 5 days, one package per day.
- **Dependency:** none.
- **Acceptance:** Each of `core`, `light`, `holder`, `issuer`,
  `verifier`, `sdk` has at least one passing `*.test.ts` covering
  the package's primary surface. CI runs `npm test` per package.

---

## Tier D -- Mainnet scope (NOT a devnet blocker)

### D1. SOLID-SEC-012 multi-party trusted setup ceremony

- **Effort:** 1-2 weeks (logistics-heavy).
- **Acceptance:** Ten or more contributors complete the ceremony for
  both the batch and subgroup circuits; attestation chain published
  on IPFS + Arweave; new VK pins committed; `verify_ceremony.js`
  green from a fresh checkout.

### D2. SOLID-SEC-043 Squads multisig on `IssuerTreeBinding.operator`

- **Effort:** 2-4 days.
- **Acceptance:** The `operator` field on `IssuerTreeBinding` is a
  Squads 3-of-5 PDA; integration test asserts a single-key signer
  cannot append/replace leaves.

### D3. SOLID-SEC-013 + SEC-034 Squads multisig on slash + fraud-proof authorities

- **Effort:** 2-4 days.
- **Acceptance:** `slash_issuer` and `submit_fraud_proof` require a
  Squads 3-of-5 signer; PDA seeds enforce this at handler entry.

---

## Cross-cutting items (not a tier; ambient hygiene)

These are not blocking devnet but will accumulate cost if deferred.

- **Privacy threat model document.** Single page at
  `docs/PRIVACY_THREAT_MODEL.md`. What the verifier learns; what
  the on-chain transaction reveals; what the indexer can correlate;
  what the issuer learns about verification events. ~half a day.
- **Public devnet activity dashboard.** Daily proof count,
  registered issuers, schemas, recent events. Turns the protocol
  from a repo into a network. ~3 days, depends on B1.
- **`docs/EXTERNAL_AUDIT_PLAN.md`.** Even pre-audit, having the
  placeholder shows the project knows audit is mandatory. Lists
  the firms shortlisted, scope, and expected timeline. ~half a day.
- **`.well-known/solid-protocol.json`.** Static JSON describing
  protocol version, program IDs, schemas, pin chain. Lets external
  indexers and aggregators discover SolID without scraping docs.
  ~half a day, ships with A2.

---

## Status snapshot

| Tier | Total items | Closed | Open |
| --- | --- | --- | --- |
| A    | 7           | 0      | 7    |
| B    | 6           | 0      | 6    |
| C    | 5           | 0      | 5    |
| D    | 3           | 0      | 3    |
| Cross-cutting | 4  | 0      | 4    |

When an item closes, flip its status header `[ ]` -> `[X]`, link the
landing PR, and update this table. The table is the single
load-bearing summary.

---

## Update protocol

This is a living document. The rules:

1. New items go in the right tier. If the tier is wrong, the rollout
   risk profile is wrong -- escalate before adding.
2. When an item closes, flip its status, link the closing PR, and
   update the snapshot table. Do not delete closed items; their
   acceptance criteria are useful for future audits.
3. If an item turns out to be in the wrong tier, demote/promote with
   a one-line note explaining what changed. Do not silently move.
4. Effort estimates are single-developer estimates. If pairing or
   parallelizing, note the team-size assumption explicitly.
5. Dependencies are hard-blocking. If something says "depends on A3"
   and A3 is open, do not start. If you need to break the dependency,
   that itself is a tier-A decision and goes through the standing
   exception register in `plan/IMPLEMENTATION_PLAN.md`.

---

*End of punchlist. Source-of-truth for what stands between the current
post-Phase-E state and a public devnet launch.*
