# SolID Final Devnet Checklist

Status: final operator checklist for the first public devnet feedback run.
Scope: `solid-protocol`, `ts-sdk`, `solid-channel`, `solid-wallet`, and
`solid-console`.

This checklist is intentionally strict. A public tester should be able to
install the wallet, open the console, issue a credential, request a proof,
and verify it on devnet without local monorepo knowledge or hidden manual
steps.

## Launch Rule

Do not call the system "devnet ready" until these are true:

- The three SolID programs are deployed and executable on devnet.
- `deployments/devnet.json` is the single source of truth for program IDs,
  artifact URLs, artifact SHA-256 pins, schema hashes, tree addresses,
  indexer URL, and deployed commit.
- Public SDK packages are installable without local `file:` dependencies.
- Circuit artifacts are served from public HTTPS URLs and verified by
  SHA-256 before use.
- Wallet proof generation uses hosted artifacts, live roots, live Merkle
  proofs, and a real holder credential package.
- Console issuer, DAO, holder-facing issuance, and verifier flows use real
  protocol calls and show honest empty/error states when data is missing.
- One clean external devnet E2E is recorded, including a successful proof
  verification transaction and a failed replay attempt.

## Current Canonical Program IDs

These IDs must stay aligned across `Anchor.toml`, Rust `declare_id!`,
`deployments/devnet.json`, SDK constants, wallet config, console config, and
docs.

| Program | Devnet ID |
| --- | --- |
| `schema_registry` | `4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1` |
| `issuer_registry` | `5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx` |
| `zk_verifier` | `DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb` |

If any ID changes, update every consumer before shipping:

- `Anchor.toml`
- `programs/*/src/lib.rs`
- `deployments/devnet.json`
- generated IDLs
- `ts-sdk` program ID exports
- `solid-wallet` runtime config
- `solid-console` runtime config
- integration docs and examples

## Ordered Checklist

### 0. Freeze The Devnet Source Of Truth

- [ ] Make `solid-protocol/deployments/devnet.json` the canonical machine
  manifest for devnet.
- [ ] Add or confirm fields for:
  - cluster RPC and websocket URL
  - all program IDs
  - deployed git commit
  - deployer address
  - upgrade authority
  - artifact base URL
  - every artifact SHA-256 pin
  - schema catalog
  - issuer tree address and current root source
  - schema tree addresses
  - global state tree address
  - indexer API URL
  - console URL
  - wallet release URL or extension package URL
- [ ] Replace scattered production constants with manifest or env-driven
  loading in SDK, wallet, console, examples, and docs.
- [ ] Add a short script/check that fails when manifest program IDs disagree
  with `Anchor.toml`, generated IDLs, or SDK constants.

Acceptance: a fresh app, wallet, and console can all load the same devnet
manifest and agree on programs, artifacts, schemas, roots, and indexer URL.

### 1. Build And Verify Protocol Locally

Run from a clean checkout:

```bash
cd solid-protocol
bash scripts/bootstrap.sh
export PATH="$PWD/.toolchain/bin:$PATH"

anchor build
npm run build:idl
npm run check-vectors

cd ts-sdk
npm ci
npm run build
cd ..

npm install
npm run e2e
```

- [ ] Anchor programs build.
- [ ] IDLs regenerate.
- [ ] Vector checks pass.
- [ ] SDK packages build.
- [ ] Local E2E completes: initialize, backfill issuer tree, bootstrap schema
  tree, bootstrap issuer, issue credential, generate proof, verify proof.
- [ ] Replay/nullifier reuse is rejected.

Acceptance: localnet is green before touching devnet.

### 2. Deploy Programs To Devnet

Configure deployer:

```bash
solana config set --url devnet --keypair ~/.config/solana/id.json
solana balance
```

Deploy:

```bash
cd solid-protocol
anchor build
anchor deploy --provider.cluster devnet
```

Then verify:

```bash
solana program show 4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1 --url devnet
solana program show 5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx --url devnet
solana program show DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb --url devnet
```

- [ ] `schema_registry` is executable.
- [ ] `issuer_registry` is executable.
- [ ] `zk_verifier` is executable.
- [ ] Upgrade authority is the intended devnet authority.
- [ ] `deployments/devnet.json` has non-null `deployed_at`, deployer, and
  upgrade authority fields.

Acceptance: the three canonical program IDs resolve to executable accounts
on devnet.

### 3. Initialize Devnet State

Use the devnet deployer:

```bash
cd solid-protocol
export ANCHOR_PROVIDER_URL=https://api.devnet.solana.com
export ANCHOR_WALLET=~/.config/solana/id.json

npm run init-onchain
npm run backfill-issuer-tree
npm run bootstrap-schema-tree
```

- [ ] Verifier config PDA exists.
- [ ] Main batch verification key is uploaded, finalized, and frozen.
- [ ] Subgroup verification key is uploaded, finalized, and frozen.
- [ ] Issuer registry config exists.
- [ ] DAO treasury exists.
- [ ] Issuer tree binding exists.
- [ ] Global state binding exists.
- [ ] Launch schema tree bindings exist.
- [ ] Manifest records every generated PDA/tree address.

Acceptance: console and SDK can read initialized devnet accounts without
falling back to local placeholders.

### 4. Register Launch Schemas

Start with a small real schema catalog. Suggested first schemas:

- `basic_identity_v1`
- `student_status_v1`
- `employment_v1`
- `dao_membership_v1`
- `kyc_age_region_v1`

For each schema:

- [ ] Add schema JSON in the repo.
- [ ] Define field names, field indices, field types, and allowed predicates.
- [ ] Compute canonical schema hash.
- [ ] Register schema on devnet.
- [ ] Create or bind the per-schema credential tree.
- [ ] Add schema metadata to `deployments/devnet.json`.
- [ ] Add the schema to the public schema API.
- [ ] Confirm console displays the schema from live data.
- [ ] Confirm wallet can use the schema hash for holder key derivation.
- [ ] Confirm verifier SDK can encode requirements against that schema.

Acceptance: an issuer and verifier can independently reference the same
schema by hash and field indices.

### 5. Host Pinned Circuit Artifacts

Host every required artifact at public HTTPS URLs:

| Artifact | Required by |
| --- | --- |
| batch witness wasm | wallet, holder SDK, prover |
| batch zkey | wallet, holder SDK, prover |
| batch verification key JSON | console, verifier SDK, local verifier |
| subgroup witness wasm | console issuer onboarding |
| subgroup zkey | console issuer onboarding |
| subgroup verification key JSON | protocol initialization, console checks |

- [ ] Use immutable, versioned URLs such as
  `https://artifacts.example.com/solid/devnet/v0.1.0/<file>`.
- [ ] Publish a pin manifest with SHA-256 for every file.
- [ ] Publish optional `.sha256` sidecar files.
- [ ] Configure wallet, console, and SDK to reject hash mismatches.
- [ ] Keep `solid-console/public/artifacts` only if console intentionally
  serves same-origin artifacts through Vercel. Otherwise, replace with the
  public artifact base URL.

Acceptance: every artifact can be fetched from a public HTTPS URL and its
hash matches the manifest before use.

### 6. Deploy The Merkle Proof Indexer And Status API

The wallet needs a real indexer. It cannot be production-like if holders must
paste Merkle paths or use local fixtures.

Required endpoints:

```text
GET /manifest
GET /status
GET /schemas
GET /issuers
GET /roots/current
GET /credentials/:commitment/proof
GET /issuers/:issuer/proof
GET /schemas/:schemaHash/tree
```

- [ ] Index credential issuance events/accounts.
- [ ] Index issuer approvals, rejections, revocations, and tree enrollment.
- [ ] Index schema tree bindings and roots.
- [ ] Return Merkle siblings, path indices, root, slot, and source account.
- [ ] Return clear stale-root and not-found errors.
- [ ] Add health/status checks for console.
- [ ] Add rate limits and request logging for public devnet usage.

Acceptance: given a real credential commitment, the wallet can fetch a
non-stub Merkle proof matching the current devnet root.

### 7. Publish Public SDK Packages

Packages to publish:

- `@solid-protocol/core`
- `@solid-protocol/channel`
- `@solid-protocol/light`
- `@solid-protocol/issuer`
- `@solid-protocol/holder`
- `@solid-protocol/verifier`
- `@solid-protocol/sdk`

Before publishing:

```bash
cd solid-protocol/ts-sdk
npm ci
npm run build
npm pack --workspaces --dry-run
```

Then publish:

```bash
npm publish --workspaces --access public
```

- [ ] Every package has correct `exports`.
- [ ] Every package has correct `types`.
- [ ] No published package depends on local `file:` paths.
- [ ] Public package versions are pinned in console, wallet, examples, and
  docs.
- [ ] Verifier quickstart works from an empty app.
- [ ] Issuer quickstart works from an empty app.

Acceptance: an external developer can install the SDK from npm and integrate
without cloning the monorepo.

### 8. Configure And Package The Wallet

Build:

```bash
cd solid-wallet
npm ci
npm test
npm run build
```

Wallet requirements:

- [ ] Loads devnet manifest or equivalent production config.
- [ ] Uses hosted artifact URLs and SHA-256 pins.
- [ ] Uses live indexer API for Merkle proofs.
- [ ] Handles `window.solid` provider calls.
- [ ] Handles proof request, approval, denial, and response envelopes.
- [ ] Stores holder credentials securely for devnet testing.
- [ ] Imports real issuer-issued credential packages.
- [ ] Shows schema, issuer, requested predicates, verifier domain, and
  disclosure summary before approval.
- [ ] Generates real Groth16 proofs through the holder SDK.
- [ ] Returns typed errors for missing credential, stale root, unsupported
  schema, artifact mismatch, and verifier mismatch.
- [ ] Provides a packaged extension ZIP or private/unlisted browser-store
  release for testers.

Acceptance: a fresh tester can install the wallet, import a real credential,
approve a verifier request, and return a proof to a dApp.

### 9. Deploy Console To Vercel

Recommended production env:

```text
VITE_SOLID_CLUSTER=devnet
VITE_SOLID_RPC_URL=https://api.devnet.solana.com
VITE_SOLID_MANIFEST_URL=https://your-domain.example/manifest
VITE_SOLID_ARTIFACT_BASE_URL=https://your-artifacts.example/solid/devnet/v0.1.0
VITE_SOLID_INDEXER_URL=https://your-api.example
VITE_SOLID_EXPLORER_CLUSTER=devnet
```

Build:

```bash
cd solid-console
npm ci
npm test
npm run build
```

Vercel settings:

```text
Root directory: solid-console
Build command: npm run build
Output directory: dist
```

Console requirements:

- [ ] No fake valid proof path.
- [ ] No default demo data pretending to be live data.
- [ ] Reads live registry, schema, verifier, issuer, DAO, and indexer state.
- [ ] Shows honest "not deployed", "not configured", or "no data yet" states.
- [ ] Can register issuer using real wallet signature and subgroup proof.
- [ ] Can issue a credential to a wallet-derived holder key.
- [ ] Can display pending issuers and DAO votes.
- [ ] Can stake governance tokens or clearly show why staking is unavailable.
- [ ] Can approve/reject/finalize issuer applications.
- [ ] Can build verifier requirements through the SDK surface.
- [ ] Can submit on-chain verification and show devnet transaction links.
- [ ] Can show artifact/indexer/schema/registry health.

Acceptance: console is a real devnet control plane, not a demo UI over local
fixtures.

### 10. Deploy Reference Integrations

Required public examples:

- [ ] Issuer integration example.
- [ ] Verifier dApp example.
- [ ] Private launchpad gate or similar product demo.

Verifier example must show:

- install SDK from npm
- define requirement
- connect to wallet provider
- request proof
- submit or verify proof
- handle typed errors

Issuer example must show:

- get holder schema public key from wallet
- issue credential
- deliver credential package
- explain schema hash and issuer authority

Acceptance: a third-party developer can copy the examples and integrate
without reading protocol internals.

### 11. Run One Clean External Devnet E2E

Run this as if you were an outside tester:

- [ ] Install wallet from packaged release.
- [ ] Open deployed console.
- [ ] Connect Solana wallet on devnet.
- [ ] Register schema or select launch schema.
- [ ] Register issuer.
- [ ] Stake/vote/approve/finalize issuer through DAO flow.
- [ ] Request holder public key through wallet.
- [ ] Issue credential to that holder key.
- [ ] Import credential into wallet.
- [ ] Open verifier demo.
- [ ] Request proof.
- [ ] Approve proof in wallet.
- [ ] Submit on-chain verification.
- [ ] Confirm verification transaction succeeds.
- [ ] Submit replay/nullifier reuse attempt.
- [ ] Confirm replay fails.
- [ ] Record all transaction signatures.
- [ ] Update `docs/DEVNET_STATUS.md`.

Acceptance: one full real-user path succeeds on devnet with recorded evidence.

### 12. Publish Public Docs And Feedback Surface

Docs to publish:

- [ ] Devnet quickstart.
- [ ] Wallet install guide.
- [ ] Issuer guide.
- [ ] Verifier integration guide.
- [ ] Schema authoring guide.
- [ ] Artifact integrity guide.
- [ ] Error codes and troubleshooting.
- [ ] Known devnet limitations.
- [ ] Feedback issue template or form.

Each guide should include:

- package install commands
- URLs
- cluster
- expected wallet prompts
- expected transaction count
- common failures
- support/feedback channel

Acceptance: testers can start without asking for private instructions.

## Things To Replace Before Public Devnet

- [ ] Replace local `file:` SDK dependencies in public-facing apps/examples
  with published npm versions.
- [ ] Replace any hardcoded artifact paths with manifest-driven pinned URLs.
- [ ] Replace any hardcoded indexer URL with manifest/env config.
- [ ] Replace any stale program IDs if the devnet deploy uses new keypairs.
- [ ] Replace local-only credential fixtures with real issuer-issued packages.
- [ ] Replace console demo fallbacks with honest empty/error states.
- [ ] Replace manual proof request JSON construction with SDK helpers where
  external integrators touch it.
- [ ] Replace docs that require cloning the monorepo with npm-based
  integration docs.

## Things Codex Can Do

I can do these directly in the repository:

- Tighten `deployments/devnet.json` schema and add validation checks.
- Wire SDK, wallet, console, and examples to consume the same manifest.
- Remove remaining hardcoded production/devnet constants.
- Add or update console/wallet env examples.
- Add Vercel-ready console configuration.
- Add SDK package metadata, exports, npm pack checks, and release notes.
- Add devnet quickstarts and integration docs.
- Add schema authoring docs and launch schema JSON files.
- Add scripts that verify artifact hashes and program ID consistency.
- Add local smoke tests for manifest loading, SDK imports, wallet provider
  envelopes, and console config.
- Add indexer API contract types and a minimal indexer implementation if you
  want it in this repo.
- Add reference issuer and verifier apps.
- Run local builds/tests and fix repo-side failures.

## Things You Need To Do Manually

You need to do or approve these because they involve accounts, secrets,
third-party ownership, or external infrastructure:

- Choose and secure the devnet deployer keypair or Squads authority.
- Fund the devnet deployer.
- Approve any real devnet program deployment transaction.
- Decide whether current canonical program IDs must be preserved.
- Own npm organization access for `@solid-protocol/*`.
- Approve npm package publication.
- Create or approve artifact hosting infrastructure.
- Create or approve indexer/API hosting infrastructure.
- Create Vercel project and set production env variables.
- Connect deployment domains.
- Package/sign/publish the browser extension or approve the release ZIP.
- Decide the public feedback channel.
- Invite testers and distribute wallet/console/verifier URLs.
- Decide which devnet limitations are acceptable to disclose publicly.

## Minimum Public Devnet Bundle

Public testers should receive exactly this:

- Console URL.
- Wallet extension ZIP or browser-store link.
- Verifier demo URL.
- Devnet status URL.
- SDK npm package names and versions.
- Artifact manifest URL.
- Indexer/status API URL.
- Feedback link.
- Known limitations list.

## Final Go/No-Go Gate

Ship devnet only if every line below is true:

- [ ] Protocol deployed.
- [ ] On-chain state initialized.
- [ ] Artifacts hosted and pinned.
- [ ] Indexer live.
- [ ] SDK published.
- [ ] Wallet packaged.
- [ ] Console deployed.
- [ ] Reference verifier deployed.
- [ ] One clean devnet E2E recorded.
- [ ] Replay protection tested.
- [ ] Docs published.
- [ ] Feedback channel live.

If any of these are false, keep the launch private/internal and label it as
an integration rehearsal, not a public devnet release.
