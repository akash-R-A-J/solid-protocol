# Devnet Readiness Checklist

Last updated: 2026-05-12 (after hosted app/API/artifact deploy).

Single source of truth for what blocks the SolID system (`solid-protocol`
+ `solid-sim` + `solid-wallet`) from a public devnet launch. Built
from the 2026-05-04 deep audit, then **revised the same day after a
second-pass audit found that the high-level `@solid-protocol/verifier`
wrapper is already shipped (not "10-14 days of work" as the first audit
claimed).** That changes the critical path.

Tracks the protocol repo and product repos at sibling paths
(`../solid-sim`, `../solid-wallet`).

Current deployment delta: app/API/artifacts/manifest are now hosted under
`solidislive.com`. This checklist remains useful for launch quality gates,
but live deployment state is tracked in `../DEPLOYMENT_TRACKER.md`.

## Operating rules

- **No workarounds.** Every item ships functional. No "good enough for
  devnet, fix later" entries; no "skip if blocked"; no Path A/B
  fallbacks. If an item is hard, it stays open until it lands.
- **Verify before you trust the audit.** Pass-1 audit reports were stale
  on three pre-flight items and on the entire `verifyRequirement`
  status. Future status claims in this file MUST cite a file:line and
  date of verification.
- **All four roles must be functional:** DAO, Issuer, Wallet, and Verifier
  through `solid-sim`, plus the external `solid-wallet` extension path.

## Scope filter

Out of scope for devnet (handled separately by the owner):

- Multi-party trusted-setup ceremony (SOLID-SEC-012, mainnet blocker).
- Governance multisig on slash / fraud-proof / issuer-tree-operator
  (SOLID-SEC-013, SOLID-SEC-043, mainnet blockers).
- The documented-security backlog in `sec/SECURITY_REGISTRY.md`. The
  owner is fixing those independently before devnet deploy.

## Decisions taken 2026-05-04

- Holder UX: use `solid-sim` Wallet for public devnet simulation and keep
  `solid-wallet` as the external-wallet integration path.
- Issuer UX: finish the issuer-side of `solid-sim`. No separate CLI for v0.

## Code-first update 2026-05-05

The latest audit is
`sec/audits/2026-05-05_full_system_devnet_user_testing_audit.md`.
It read implementation code across the Anchor programs, Rust crates,
TS SDK packages, `solid-sim`, and `solid-wallet`; docs were treated
as claims to verify, not as ground truth.

Fast gates verified 2026-05-05:

| Gate | Result |
| --- | --- |
| `python3 scripts/check_program_ids.py` | pass |
| `npm run validate:devnet` | pass |
| `npm run verify:artifacts` | pass |
| `npm run smoke:devnet-config` | pass structurally; public app/API/artifact URLs are now hosted under `solidislive.com` |
| `cd ts-sdk && npm run build && npm test --workspaces --if-present` | pass |
| `cd ../solid-sim && npm run test && npm run build` | pass; large chunk warning |
| `cd ../solid-wallet && npm run test && npm run build` | pass; browser externalization warnings for holder debug imports |
| `cargo fmt --all -- --check` | pass |
| `cargo test -p solid-core -p solid-light --no-fail-fast` | pass |
| `cargo test -p zk-verifier --lib --no-fail-fast` | pass |
| `cargo clippy -p solid-core -p solid-light -- -D warnings` | pass |
| `npm audit --audit-level=moderate` in SDK / console / wallet | fails; see P1-6 below |

Code reality changed several older checklist rows:

- `solid-sim` now has real issuer registration, DAO vote/finalize,
  verifier proof-buffer submission, manifest loading, and wallet-provider
  integration paths.
- `solid-wallet` now imports encrypted envelopes, stores validated
  credentials, opens proof request popups, pin-checks hosted artifacts,
  calls `@solid-protocol/holder`, and returns serialized proof data.
- Public user testing is still blocked by external deployment state:
  populated manifest, artifact host, indexer/API, governance mint/staker
  setup, and a live four-role smoke.

### Devnet deploy update 2026-05-06

The protocol layer is now partially live on public devnet:

| Area | Status |
| --- | --- |
| Program deployment | `schema_registry`, `issuer_registry`, and `zk_verifier` deployed at canonical IDs |
| Verifier setup | registry/config PDAs initialized; batch and subgroup VKs uploaded/finalized |
| Schema smoke | `basic_identity_v2` registered with depth-20 schema tree |
| Issuer/DAO smoke | issuer registration, stake, vote, finalize, and issuer-tree enrollment completed |
| Credential smoke | one `basic_identity_v2` credential issued on devnet |
| Proof smoke | existing sample local witness, local `snarkjs.groth16.verify`, on-chain `verify_batch_proof_v2`, and replay rejection pass |
| Remaining protocol blocker | protocol terminal smoke is green; public testing is blocked by hosted artifacts, indexer/API, solid-sim deployment, and a four-role solid-sim browser smoke |

`basic_identity_v1` was created earlier with a depth-16 schema tree and
must not be used with the current batch circuit. Public launch schemas
must use depth-20 trees.

### Current role-readiness matrix

| Role | Current code path | Required infrastructure | Status |
| --- | --- | --- | --- |
| DAO | `solid-sim` reads issuer accounts and builds vote/finalize txs; System/Flow shows DAO as the start of trust causality | Governance mint, staked DAO voter, live registry config, pending issuer application | protocol smoke green via scripts; public UX still needs smoke |
| Issuer | `solid-sim` derives BJJ identity, loads subgroup artifacts, builds `register_issuer`, signs encrypted credential envelopes | Live programs, hosted subgroup artifacts, registered schema, approved issuer flow, Wallet material | protocol smoke green via scripts; public artifact URLs still missing |
| Wallet | `solid-sim` Wallet creates/imports simulator identity, derives holder keys, imports encrypted envelopes, validates integrity, and proves with artifacts/indexer | Hosted batch artifacts, live Merkle proof indexer, current tree roots, real issued credential | build/test green; public proof UX blocked by hosted artifacts/indexer and four-role smoke |
| Verifier | `solid-sim` locally verifies Groth16 proof or submits proof-buffer tx sequence | Finalized VK, active schema/issuer/global bindings, funded payer, live nullifier PDA path | existing devnet sample verify green; public four-role smoke pending |

### Current P0 blockers

| ID | Blocker | Done when |
| --- | --- | --- |
| P0-1 | Publish populated `deployments/devnet.json` | Deployed timestamp, deployer, upgrade authorities, schemas, trees, roots, sample issuer, sample credential, and existing-sample verify tx are populated. Artifact/indexer/console/wallet URLs and fresh issue/prove evidence remain pending. |
| P0-2 | Host pinned artifacts | All batch/subgroup `.wasm`, `.zkey`, verification key JSON, and `.sha256` sidecars are served over HTTPS and match manifest pins. |
| P0-3 | Deploy Merkle proof indexer/API | `/v1/health`, `/v1/merkle-proof/:tree/:leaf`, issuer/schema reads, and status JSON are live. |
| P0-4 | Configure DAO governance mint/stake | Real devnet mint exists and script smoke vote/finalize can run; public DAO console smoke remains pending. |
| P0-5 | Harden wallet origin policy | Extension is restricted to tester origins or holder/channel key reads require approval/allowlist. |
| P0-6 | Run live four-role smoke | DAO, issuer, holder, and verifier evidence is recorded with tx signatures and replay rejection. |

### Current P1 blockers

| ID | Blocker |
| --- | --- |
| P1-1 | Bind verifier requirement/query to public inputs before transaction build or verification logging. |
| P1-2 | Enforce or hard-gate batch VK finalization before proof verification. |
| P1-3 | Add indexer freshness/root reconciliation and lag rejection. |
| P1-4 | Resolve holder package browser externalization warnings or add a browser-like proof smoke proving they are unreachable. |
| P1-5 | Add meaningful SDK tests for packages that currently pass with no tests. |
| P1-6 | Triage npm audit advisories (`esbuild`/`vite`/`vitest`, `underscore` via `bfj/jsonpath`, `uuid` via `rpc-websockets`). |

## Critical-path summary (revised after pass-2 audit)

```
Week 1
  0.1  deploy keypair
  1.x  devnet deploy (programs, VKs, manifest, sample data)
  2.A  fix SDK blocker bugs (channel package + types)        [~1-2d]

Week 2
  3.x  npm publish (sdk + 6 packages)
  4.1  artifact CDN + 4.4 status endpoint

Week 2-3
  5.x  wallet (real extension)                      ┐
  6.x  console (issuer + verifier + holder pages)    ├─ parallel
  4.2  schema/issuer registry REST API               │
  4.3  Merkle proof / indexer API                    ┘

Week 3-4
  7.x  reference demo
  8.x  documentation
  2.B  SDK polish (non-blocking correctness items)
```

**Estimate: 8-14 days** end-to-end from today (revised down from the
original 23-30 because `verifyRequirement` is shipped). Actual duration
depends on parallelism and on how clean B1/B2 fixes turn out (those
might unmask other channel-package issues).

---

## 0. Pre-flight (this week)

| #   | Item                                                                                  | Why                                                                                                                                                                                       | Status |
| --- | ------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| 0.1 | Provision a devnet deployer keypair and fund it (~15 SOL).                            | Current deployer is `Gdz9JLWUekrfnpT3fPu1SsWfas3b3zMhfC4frvV1QRNm`; balance checked 2026-05-06 at `10.03288796 SOL`. More may be needed for public-RPC retries or full launch-schema setup. | partial |
| 0.2 | ~~`cargo clippy` failure on `crates/solid-core/src/sas.rs`~~                          | Verified 2026-05-04: `crates/solid-core/src/sas.rs:48` already has `#[allow(dead_code)]`. `cargo clippy -p solid-core -p solid-light -- -D warnings` exits 0. Audit finding was stale.    | done   |
| 0.3 | ~~`cargo fmt` failure on `crates/solid-light/src/cpi_helpers.rs`~~                    | Verified 2026-05-04: `cargo fmt --all -- --check` exits 0. Workspace is clean. Audit finding was stale.                                                                                   | done   |
| 0.4 | ~~Doc drift in README / CURRENT_STATE / CLAUDE.md~~                                   | Verified 2026-05-04: `README.md:47`, `docs/CURRENT_STATE.md:4`, `CLAUDE.md:65` already reference v0.6.1 post-Phase-E and the subgroup VK pin `938ab390...e9`. Audit finding was stale.    | done   |

## 1. Protocol — on-chain layer

This layer is already production-quality. These items execute the
deploy; they do not block it.

| #   | Item                                                                                                                                                | Why                                                                                                                                                                                                          | Status |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------ |
| 1.1 | Run `scripts/initialize.ts` against `--provider.cluster devnet`.                                                                                    | Registry/verifier config, VKs, issuer tree, global binding, smoke schema tree, and sample state exist on devnet.                                                                                              | done    |
| 1.2 | Run main VK chunked upload + finalize + freeze, and subgroup VK chunked upload + finalize + freeze.                                                  | The verifier programs are useless without VKs loaded. Freeze locks them under the 48-hour ADR-0015 timelock so rotation requires a real ceremony, not a `--force`.                                            | done    |
| 1.3 | Run `scripts/regen_devnet_manifest.py` after deploy.                                                                                                | Populates `deployer.address`, `deployed_at`, and per-program signatures into `deployments/devnet.json`. Current hosted manifest serves live API, artifact, and app URLs.                                      | partial |
| 1.4 | Run `backfill-issuer-tree` and `bootstrap-schema-tree` against devnet.                                                                              | Verifier owner-checks `issuer_tree_binding` and `schema_tree_N` accounts (ADR-0014). Both bindings exist for the current smoke state.                                                                         | done    |
| 1.5 | Sample-data seed: bootstrap one demo issuer (with on-chain BJJ subgroup proof) and one demo schema (matching `verifier`'s `DEFAULT_SCHEMA_CATALOG` or one supplied via `schemaCatalog`). | `basic_identity_v2`, one sample issuer, and one sample credential exist on devnet. Fresh sample issuance awaits `issuer_registry` upgrade + issuer/schema permission. | partial |
| 1.6 | Capture and pin a reference green `verify_batch_proof_v2` signature on devnet.                                                                      | Existing sample proof tx is `56crrCtH7QDAQbgrqiHuytuJVskXiRm27LvBRGBCBLGXM4k7q9nXW2oHhnd3U9rmxTBQzHMEcHuuEk84Ezy1VNut`; fresh issue/prove evidence remains pending. | partial |

## 2. SDK — bugs and gaps surfaced by the pass-2 audit

The pass-2 audit (2026-05-04) read every file in `ts-sdk/packages/` and
ran `npm run build`. Findings split into two tiers.

### 2.A SDK blockers — fix before any consumer can rely on the SDK

| #     | Item                                                                                                                                                                                                                          | Why                                                                                                                                                                                                                                                                                                                                                              | Status |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| 2.A.1 | `ts-sdk/packages/channel/` is missing `tsconfig.json`. `npm run build` exits 0 silently with no `dist/`. Add a `tsconfig.json` matching the other six packages.                                                                | The package "builds" but emits nothing. Publishing it would ship a package with no compiled JS; consumers would get `ERR_MODULE_NOT_FOUND`. Verified by `ls ts-sdk/packages/channel/` showing only `package.json` + `src/`.                                                                                                                                       | open   |
| 2.A.2 | `ts-sdk/packages/channel/package.json` declares `@noble/ed25519` and `@noble/hashes`. Source imports `tweetnacl` and `tweetnacl-util`. Pick one stack (probably tweetnacl since the code is already written against it) and align the deps.                                                                                                                  | Confirmed: `find /Users/rajakash/Desktop/testing/solid-protocol -name 'tweetnacl*'` returns nothing — neither stack resolves on a fresh `npm install`. The package is currently un-buildable in isolation; only B1 (no tsconfig) is masking that.                                                                                                                | open   |
| 2.A.3 | Channel docstrings (`channel/src/index.ts:16, 20`, `derive-key.ts`, `types.ts`, `decrypt.ts`, `encrypt.ts`) reference the import name `@solid-protocol/credential-channel`. Published name is `@solid-protocol/channel`.        | Anyone copy-pasting from the example will get a 404. Either rename the package or fix the docstrings.                                                                                                                                                                                                                                                            | open   |
| 2.A.4 | `holder/src/index.ts:107-282` `generateProof` (single-credential path) builds a circuit input with `merkleRoot` + `schemaHash` (singular). Current circuit `batch_credential_query.circom` expects plural arrays. Either delete this function or rewrite it to call the batch circuit with one active credential.                                              | If anyone calls it, it'll fail at snarkjs witness generation. If no one calls it, dead code that publishes confusion. Audit the call sites first.                                                                                                                                                                                                                | open   |
| 2.A.5 | Promote `@solana/web3.js` from `dependencies` to `peerDependencies` (with a wide range like `^1.95.0`) in all 7 packages.                                                                                                       | Every package currently lists it as a regular dep. With six packages bundling it, an integrator can end up with multiple copies; `instanceof PublicKey` then fails across package boundaries. Standard Solana SDK convention.                                                                                                                                  | open   |
| 2.A.6 | Add `LICENSE` and `engines.node` fields to all 7 `package.json` files. Add `"exports"` blocks for proper ESM resolution.                                                                                                       | Per top-level README the project is dual Apache-2.0 / MIT — only `channel/package.json` declares MIT. Several packages use Node-only APIs (`fs`, `crypto`); without `engines`, browser bundlers silently fail. Without `exports`, deep imports are unspecified.                                                                                                  | open   |

### 2.B SDK correctness / polish

These are real defects but do not block devnet launch. Land them before
external integrator pilots.

| #      | Item                                                                                                                                                                                                                                                                                                              | Why                                                                                                                                                                                                                                                                                                                                                | Status |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| 2.B.1  | `verifier/package.json` should add `@solid-protocol/light` as a dep, and `verifier/src/index.ts:1817-1837` (`deriveTreesFromPublicInputs`) should import the seed strings (`'schema-tree-binding'`, `'global-binding'`, `'issuer-tree-binding'`) from `@solid-protocol/light` instead of duplicating them.       | If anyone changes a seed in `light` (next version bump), `verifier`'s PDA derivation silently drifts and proofs fail with `AccountNotFound`. Single source of truth.                                                                                                                                                                              | open   |
| 2.B.2  | Two different default artifact host URLs: `verifier/src/index.ts:1128` `DEFAULT_ARTIFACT_HOST = 'https://artifacts.solid.example/v0.6.1/'` and `sdk/src/config.ts:32` `ARTIFACT_BASE_URL = 'https://cdn.solid-protocol.com/artifacts/v1'`. Collapse to a single source.                                          | When you set up the CDN (item 4.1), you'll need to update both. Single source of truth — and these are different placeholder hostnames, neither of which resolves.                                                                                                                                                                                | open   |
| 2.B.3  | `sdk/src/index.ts:187` `{ wasmPath: wasmBytes as unknown as string, zkeyPath: zkeyBytes as unknown as string }` — remove the double-cast. `holder/src/index.ts:344-347` already accepts `string \| Uint8Array`.                                                                                                  | Cast hides intent and violates type safety. Runtime works because snarkjs accepts both.                                                                                                                                                                                                                                                            | open   |
| 2.B.4  | `verifier/src/index.ts:1780-1805` `validatePublicSignals` checks slots 6-9 (schemas), 27 (numPredicates), 28 (compoundLogic), 15-22 (fields/operators), 23-26 (values). Add checks for slot 29 (`verifierAddress` == `requirement.verifierAddress.toBytes()`) and slot 30 (`verifierNonce` == requirement nonce). | Defense-in-depth. A malicious holder could submit a proof with the right predicates but a different nonce; on-chain verify rejects (slot 29 check at `programs/zk-verifier/src/lib.rs:861`), but the SDK pre-flight wouldn't catch it. Cheap fix.                                                                                                  | open   |
| 2.B.5  | `verifier/src/index.ts:1839-1855` `buildQueryFromRequirement` returns `queryContextHash: new Uint8Array(32)` (stub). Compute it properly from the predicate set (mirror `holder/src/index.ts::computeQueryContextHash`).                                                                                          | Currently latent because `verifyOnChain` doesn't read `request.query`. If anyone changes that, the stubbed hash silently lies.                                                                                                                                                                                                                     | open   |
| 2.B.6  | `light/src/index.ts:362-363` comment claims `LocalReplicaAdapter` defaults to keccak-256. The constructor (`:365-368`) requires `hashPair`. Either default it (to `poseidonHashPair` since SolID trees are Poseidon) or fix the comment.                                                                          | Misleading docstring — consumers reading the comment will assume they can omit the arg. Five-min fix.                                                                                                                                                                                                                                              | open   |
| 2.B.7  | `core/src/index.ts:436-439` `QueryBuilder.schemas()` silently truncates inputs beyond index 4. Should throw on >4.                                                                                                                                                                                                | Silent data loss. If a caller supplies 5 schemas, the 5th vanishes.                                                                                                                                                                                                                                                                                | open   |
| 2.B.8  | `core/src/index.ts:495-510` `QueryBuilder.build()` ordering check skips zero→non-zero transitions, so a layout like `[A, 0, B, 0]` passes locally but the on-chain handler at `programs/zk-verifier/src/lib.rs:852-854` only requires strict ascending across active slots. SDK check should match on-chain shape. | SDK is laxer than on-chain. Cosmetic but a code smell.                                                                                                                                                                                                                                                                                             | open   |
| 2.B.9  | `issuer/src/index.ts:282-303` `batchIssueCredentials` runs sequentially. Either parallelise (with concurrency cap) or document the perf trade-off in JSDoc.                                                                                                                                                       | For a 100-credential mint this is 100 round-trips serially. Not a bug; perf footgun without a doc note.                                                                                                                                                                                                                                            | open   |
| 2.B.10 | `verifier/src/index.ts:34` imports from `'crypto'` (Node-only). Add a `README.md` note in `verifier/` documenting browser-build polyfill needs (or move to `@noble/hashes` for isomorphism).                                                                                                                       | Modern bundlers polyfill, but a vanilla Vite browser build will fail without `crypto-browserify` config. Browser is `solid-sim` and `solid-wallet`'s target.                                                                                                                                                                                  | open   |

### 2.C — verified shipped (pass-2 corrections to pass-1 audit)

These were marked open in the original checklist but the pass-2 audit
verified they are implemented. Citations:

| Item                                                          | Where                                                | Verified |
| ------------------------------------------------------------- | ---------------------------------------------------- | -------- |
| `SolidVerifier` class                                         | `verifier/src/index.ts:1186-1450`                    | done     |
| `defineRequirement(spec)`                                     | `verifier/src/index.ts:1217`                         | done     |
| `verifyProof(args)`                                           | `verifier/src/index.ts:1286`                         | done     |
| `verifyRequirement(args)`                                     | `verifier/src/index.ts:1336`                         | done     |
| Typed `VerificationError` (16 variants)                       | `verifier/src/index.ts:973-989`                      | done     |
| `SolidVerificationError` class                                | `verifier/src/index.ts:1009`                         | done     |
| `walletAdapterTransport(wallet)`                              | `verifier/src/index.ts:1452-1477`                    | done     |
| `httpTransport(endpoint)`                                     | `verifier/src/index.ts:1479-1497`                    | done     |
| `explainVerificationError(err)` (full table)                  | `verifier/src/index.ts:1499-1574`                    | done     |
| `SolidVerifier.health()` (program + config + ping)            | `verifier/src/index.ts:1356-1406`                    | done     |
| `SolidVerifier.loadArtifact(kind)` (SHA-256-pinned fetch)     | `verifier/src/index.ts:1408-1424`                    | done     |
| `normalizeVerificationFailure(err)`                           | `verifier/src/index.ts:1857-1885`                    | done     |
| Artifact pin chain (env > sidecar > config)                   | `sdk/src/artifact_integrity.ts:84-105`               | done     |
| Default canonical artifact pins for both circuits             | `verifier/src/index.ts:1131-1138`                    | done     |
| Discriminator computation & cross-check vs on-chain           | `verifier/src/index.ts:87-90` (computed); `issuer/src/index.ts:60` (hardcoded `[0xff,0xc1,...]` matches `python3 sha256('global:issue_credential')[:16]`) | done     |
| Account ordering for `verify_batch_proof_v2` matches on-chain | SDK `verifier/src/index.ts:389-402` ↔ on-chain `programs/zk-verifier/src/lib.rs:1123-1187` | done     |
| Account ordering for `issue_credential` matches on-chain      | SDK `issuer/src/index.ts:178-189` ↔ on-chain `programs/issuer-registry/src/lib.rs:2761-2823` | done     |
| Wire-input slot table (`WIRE_INPUT_SLOTS`)                    | `verifier/src/index.ts:65-67` (21 entries, ascending) | done     |
| G2 byte-order swap (SOLID-SEC-067)                            | `holder/src/index.ts:842-847`                        | done     |
| 6-input nullifier (ADR-0006 / SOLID-SEC-008)                  | `core/src/index.ts:295-308`                          | done     |
| Workspace `npm run build` exits 0                             | run 2026-05-04 from `ts-sdk/`                        | done     |

## 3. SDK — npm publish

| #   | Item                                                                                                                                            | Why                                                                                                                                                                                                                          | Status |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| 3.1 | All of 2.A landed.                                                                                                                              | Blocker prerequisite. Publishing without 2.A.1/2.A.2 ships a broken `@solid-protocol/channel`. Without 2.A.4 ships dead/broken `generateProof`. Without 2.A.5/2.A.6 ships type-safety landmines.                              | open   |
| 3.2 | Run `npm publish` for all 7 packages: `@solid-protocol/{core,holder,issuer,verifier,light,channel,sdk}`.                                          | Right now nothing is on npm. Console + demo + wallet cannot `npm install` what they need. Devnet integrators cannot install at all.                                                                                          | open   |
| 3.3 | Verify install: `npm init -y && npm install @solid-protocol/sdk` in a fresh project succeeds, all transitive deps resolve, types load.            | A "publish then verify" gate. Catches publish-time issues that pass-2 audit would not have surfaced (e.g. files missing from the npm tarball because of `.npmignore` drift).                                                  | open   |

## 4. SDK — test coverage

| #   | Item                                                                                                                                              | Why                                                                                                                                                                                                                                                                                          | Status |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------ |
| 4.1 | At minimum 1 happy-path `*.test.ts` per package in `ts-sdk/packages/{core,holder,issuer,verifier,light,channel,sdk}`.                              | All seven packages declare `"test": "jest"` in `package.json`. Zero packages have any test files. CI's `npm test --workspaces --if-present` passes vacuously. The next breaking refactor will ship without anyone noticing.                                                                | open   |
| 4.2 | Cross-language vectors: bring `tests/vectors/` from 2/10 primitives to 10/10.                                                                      | SOLID-SEC-010 (HIGH, open). The Rust ↔ TS byte-identity guarantee is the contract that lets you swap layers. With only `commitment_and_nullifier.json` covered, schema-hash, BJJ pubkey derivation, EdDSA sign/verify, identity-anchor leaf, issuer-tree leaf, and BE/LE encoding can drift. | open   |
| 4.3 | Implement integration test scenarios 02..11 (currently 1/11) per `tests/integration/README.md`.                                                    | Issuer lifecycle, slashing, schema/bindings, issue, verify happy-path, replay rejection, forged-tree rejection, expired-credential rejection. All happen in production. None are covered by the existing bankrun harness beyond `01_registry_init.test.ts`.                                | open   |

## 5. Hosted devnet infrastructure

| #   | Item                                                                                                                              | Why                                                                                                                                                                                                                                                                                            | Status |
| --- | --------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------ |
| 5.1 | Artifact CDN serving the four pinned files (`batch.wasm`, `batch.zkey`, `batch.vkey.json`, `bjj_subgroup.*`) plus `.sha256` sidecars. After deploy, update both `verifier/src/index.ts:1128` (`DEFAULT_ARTIFACT_HOST`) and `sdk/src/config.ts:32` (`ARTIFACT_BASE_URL`) — see 2.B.2 to collapse to one. | Holders need to download the WASM + zkey to generate proofs in the browser. Today these only exist locally in `circuits/build/`. Without a CDN there is no "the wallet talks to a public artifact endpoint" story.                                                                              | open   |
| 5.2 | Schema/issuer registry REST API (read-only mirror of on-chain `getProgramAccounts`).                                              | `solid-sim` currently uses `getProgramAccounts` with a fragile `dataSize: 200` filter (`solid-sim/src/lib/registry.ts:156`). That is brittle on devnet (rate limits) and impossible at scale. A small read-only API makes "list issuers" and "list schemas" deterministic for any client.   | open   |
| 5.3 | Merkle proof / indexer API for the issuer tree and per-schema trees.                                                              | To generate a proof, the holder needs the merkle path for their credential leaf. Without an indexer, every holder has to scan SPL Account Compression CPI logs themselves. That makes the wallet impossible to ship.                                                                          | open   |
| 5.4 | `/status` health endpoint (or a static `.well-known/solid-protocol.json`) returning `{ programIds, vkPins, lastDeployTx, clusterHeight }`. `SolidVerifier.health()` already pings `${artifactHostUrl}.well-known/solid-protocol.json` (`verifier/src/index.ts:1386`); host that. | `docs/DEVNET_STATUS.md` is human-readable; integrators need a machine-readable equivalent so the SDK can verify "the deployment I am pointing at matches the artifacts I am using" at startup.                                                                                                | open   |

## 6. Wallet (`solid-wallet`, real extension)

Decision: ship the real extension. No console fallback.

| #    | Item                                                                                                                                                                                                                                                                                       | Why                                                                                                                                                                                                                                                                                                                                                                                                  | Status |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| 6.1  | Replace password-PBKDF2 vault with `window.solana.signMessage()` -> HKDF-SHA256 with domain-separated `use:` strings, per `docs/HOLDER_STORAGE_AND_WALLET.md:104-195`. Use `@solid-protocol/channel::deriveChannelKeyFromWallet` for the channel-key portion (already implemented).         | Spec says the wallet is the root of credential identity. Current build (`solid-wallet/src/background/vault.ts:34`) ignores the wallet and PBKDFs a separate password. A user reconnecting their Solana wallet on a fresh device cannot recover credentials. That contradicts the spec and breaks cross-device portability. The channel package already implements the deterministic derivation.   | open   |
| 6.2  | Import `@solid-protocol/holder::generateBatchProof` and replace the 3-second `setTimeout` mock at `solid-wallet/src/popup/App.tsx:100-107`.                                                                                                                                                | Right now the proof generation is a UI animation. The wallet does not actually generate a Groth16 proof. Single most important wallet feature.                                                                                                                                                                                                                                                       | open   |
| 6.3  | Add `snarkjs`, the circuit `.wasm`/`.zkey` artifacts (loaded from CDN per 5.1, pin-checked via `@solid-protocol/sdk::loadAndVerifyArtifact`), and the WASM bridge from `@solid-protocol/core`.                                                                                              | `groth16.fullProve()` needs all three. Today none are in `solid-wallet/package.json`.                                                                                                                                                                                                                                                                                                                | open   |
| 6.4  | Implement claim-link import (`importFromClaimLink(url)`). Use `@solid-protocol/channel::decryptCredential` to decrypt the envelope (already implemented).                                                                                                                                  | Today only paste-JSON is supported. Spec at `docs/HOLDER_STORAGE_AND_WALLET.md:524` requires URL-based claim links. The channel package already provides ECIES decryption — wire it.                                                                                                                                                                                                                  | open   |
| 6.5  | Expose `window.solid.requestProof(envelope)` matching the contract `walletAdapterTransport` already expects at `verifier/src/index.ts:1452-1477`.                                                                                                                                          | The verifier wrapper is already shipped and looks for `wallet.requestProof`, `wallet.solid?.requestProof`, or `window.solid.requestProof`. Wiring the wallet to expose that surface closes the loop.                                                                                                                                                                                                | open   |
| 6.6  | Advertise the `solid:credentials@1` Wallet Standard feature with the ten methods in `docs/HOLDER_STORAGE_AND_WALLET.md:316-363`.                                                                                                                                                            | Without it, dApps have no portable way to discover SolID-capable wallets. The `walletAdapterTransport` shim works without it, but Wallet Standard is the long-term contract.                                                                                                                                                                                                                         | open   |
| 6.7  | Add `chrome.action.openPopup()` in the service worker so dApps can trigger proof requests.                                                                                                                                                                                                  | When a dApp calls `window.solid.requestProof()`, the popup never opens automatically (`solid-wallet/src/background/service-worker.ts`). The user has to click the toolbar icon manually.                                                                                                                                                                                                              | open   |
| 6.8  | Generate manifest icons (16/48/128 px) referenced by `manifest.json` but missing from `public/`.                                                                                                                                                                                          | Manifest references `icons/icon-{16,48,128}.png` (manifest.json:10-12, 26-30); the files are absent. Extension fails to load in Chrome with a missing-asset error.                                                                                                                                                                                                                                  | open   |
| 6.9  | Restrict the content script from `<all_urls>` to specific dApp origins; add CSP and `host_permissions` for the artifact CDN, indexer, and dApp domains.                                                                                                                                    | Running on every page is CSP-violating in strict dApp environments and is a privacy footgun. Spec recommends SRI + CSP. Currently neither is set.                                                                                                                                                                                                                                                    | open   |
| 6.10 | Implement nullifier computation in the proof flow (the `nullifier` field exists in types but is never set). Use `@solid-protocol/core::computeNullifier` (the 6-input ADR-0006 signature, already shipped).                                                                                  | Replay protection lives in the on-chain nullifier PDA init constraint. If the wallet does not compute the nullifier correctly (6-input Poseidon, ADR-0006 revision), the on-chain handler rejects the proof. No nullifier == no working verification.                                                                                                                                              | open   |
| 6.11 | Backup/restore via Argon2id + ChaCha20-Poly1305 per `docs/HOLDER_STORAGE_AND_WALLET.md:252-276`.                                                                                                                                                                                            | Cross-device recovery and disaster recovery. Without this, losing the browser profile loses all credentials.                                                                                                                                                                                                                                                                                         | open   |
| 6.12 | Credential storage format: `encryptedBlob: Uint8Array` (not base64 string), `encryptedSummary`, `schemaHashHint`, `expiresAtHint` per `docs/HOLDER_STORAGE_AND_WALLET.md:209-245`.                                                                                                          | The current shape is wrong relative to the spec. Indexers and the `canSatisfy` predicate evaluator depend on the unencrypted hint fields.                                                                                                                                                                                                                                                            | open   |

## 7. Solid Sim (`solid-sim`, finished issuer + verifier + holder pages)

Decision: finish all roles in the console. No CLI for v0.

| #    | Item                                                                                                                                                                                                                                                                                          | Why                                                                                                                                                                                                                                                                                                                                                                  | Status |
| ---- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| 7.1  | Add `@solid-protocol/sdk`, `@solid-protocol/verifier`, `@solid-protocol/issuer`, `@solid-protocol/core`, `@solid-protocol/light` to `solid-sim/package.json`. Remove the parallel implementations in `src/lib/registry.ts` (manual byte offsets, `dataSize: 200` filter).                | `solid-sim` currently re-derives PDAs and parses `VerifierConfig` by hand. When ADR-0015 layout changes (SPACE bumps, new fields), the app silently breaks. The SDK is the single source of truth; the app must consume it.                                                                                                                                  | open   |
| 7.2  | Wire `useWallet()` from `@solana/wallet-adapter-react` and replace every demo signature with a real signed instruction.                                                                                                                                                                       | `WalletMultiButton` is wired (`src/layout/TopBar.tsx:69`) but `useWallet()` is never called. Every "submit" button currently fakes success. Without this, the console is a screenshot machine, not an interface.                                                                                                                                                     | open   |
| 7.3  | Replace `crypto.getRandomValues()` BJJ key generation in `src/roles/issuer/pages/RegisterIssuer.tsx:21-43` with `@solid-protocol/core::generateKeypair()`.                                                                                                                                  | The current keys are not valid BJJ subgroup elements. They will fail the on-chain `bjj_subgroup_proof` check at `register_issuer` time. The console would never successfully register a real issuer.                                                                                                                                                              | open   |
| 7.4  | Replace the demo-signature return at `src/roles/issuer/pages/RegisterIssuer.tsx:45-68` with real Anchor `register_issuer` ix construction signed via `useWallet()`. Use `@solid-protocol/issuer::generateSubgroupProof` (already shipped) for the 256-byte subgroup proof argument.        | Today the page fakes success. Real registration needs the on-chain BJJ subgroup proof generation (Phase E), DAO vote orchestration, and stake deposit — all of which `bootstrap_issuer.ts` does and the console must do as well.                                                                                                                                  | open   |
| 7.5  | Wire `src/roles/issuer/pages/IssueCredential.tsx:43-74` to compute the real Poseidon credential commitment and call `@solid-protocol/issuer::issueCredential` (already shipped — wraps `issue_credential` CPI).                                                                              | Currently emits a JSON envelope only. No commitment, no CPI, no on-chain effect.                                                                                                                                                                                                                                                                                    | open   |
| 7.6  | Replace placeholder verification at `src/roles/verifier/pages/VerifyProof.tsx:28-44` with `SolidVerifier.verifyRequirement(...)` (already shipped at `verifier/src/index.ts:1336`). Use `httpTransport` for backend-style verification or `walletAdapterTransport` for in-browser.            | Today verification is a 2-second sleep + success. The verifier dashboard is the most-demoed page; it has to actually verify.                                                                                                                                                                                                                                        | open   |
| 7.7  | Add a hand-off so the console can deliver an issued credential to the holder (claim link or QR). Use `@solid-protocol/channel::encryptCredential` to ECIES-wrap it (already shipped — pending 2.A.1/2.A.2 fix).                                                                              | If the issuer console emits credentials that the wallet cannot import, the loop is broken. This is the issuer ↔ wallet contract.                                                                                                                                                                                                                                    | open   |
| 7.8  | Add a holder role: list credentials in the connected wallet, preview disclosure for a requirement, generate a proof through the wallet (`window.solid.requestProof`).                                                                                                                        | Without this, there's no in-console way to test the verifier without the extension installed. Useful for demo days and for non-wallet-extension flows.                                                                                                                                                                                                              | open   |
| 7.9  | Add an env-driven RPC + program ID config (`.env.example` + `import.meta.env.VITE_*`) instead of hardcoding `clusterApiUrl('devnet')` at `src/App.tsx:54`.                                                                                                                                  | When you redeploy programs, ship a hotfix, or test against staging, you do not want a code change. You also need a localnet/devnet/mainnet selector for development.                                                                                                                                                                                                | open   |
| 7.10 | Discriminator-based `getProgramAccounts` filters instead of `dataSize: 200` (`src/lib/registry.ts:156`).                                                                                                                                                                                    | Approximate size filters miss accounts on field additions and over-fetch on similar account types. Anchor IDLs give you exact discriminators for free.                                                                                                                                                                                                              | open   |
| 7.11 | Use IDL-generated client (Anchor's `Program<T>`) for account parsing instead of manual byte offsets in `src/lib/registry.ts:104-133`. Or use `SolidVerifier`'s account parsers (e.g. `checkIssuerStatus` at `verifier/src/index.ts:841`).                                                   | The manual layout was correct yesterday. The next account-shape change will break it. The IDL is regenerated on every build; the manual parser is not.                                                                                                                                                                                                              | open   |

## 8. Reference integration — the demo

| #   | Item                                                                                                                                                                                                | Why                                                                                                                                                                                                                                                                            | Status |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------ |
| 8.1 | `examples/private-launchpad-gate` Next.js app deployed to a public URL, using `SolidVerifier.verifyRequirement` to gate a "join pool" action.                                                       | The listing-quality demo. Without it the Superteam pitch is "private eligibility verification for Solana apps" with no app to point at. With it: "click here, connect wallet, see it work."                                                                                   | open   |
| 8.2 | The demo must round-trip: holder loads it -> wallet popup -> proof generated -> on-chain verified -> demo shows "you are in".                                                                       | Anything less is a slide deck. Replay protection (nullifier PDA reject on second submit) should be visible in the demo too — it is the proof the system actually works.                                                                                                       | open   |

## 9. Documentation

| #   | Item                                                                                                                            | Why                                                                                                                                                                                | Status |
| --- | ------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| 9.1 | `docs/DEVNET_QUICKSTART.md` — "go from zero to verified proof in 15 minutes" using `SolidVerifier`.                              | Without a quickstart, every integrator emails you. The current README is for protocol contributors, not consumers.                                                                | open   |
| 9.2 | `docs/VERIFIER_INTEGRATION.md` — `SolidVerifier` API ref + auth/error/timeout matrix. Reference `explainVerificationError` table. | Needed for #8.                                                                                                                                                                     | open   |
| 9.3 | `docs/ISSUER_GUIDE.md` — operational doc: how to register, how to issue, how the DAO admission process works.                  | Issuers will not onboard without this. Today the only doc is `bootstrap_issuer.ts` source.                                                                                          | open   |
| 9.4 | `docs/HOLDER_FLOW.md` — what the holder UX looks like, claim-link format, recovery story.                                      | Needed for #6.                                                                                                                                                                     | open   |
| 9.5 | `docs/ERROR_CODES.md` — formalize the 16-variant `VerificationError` table from `verifier/src/index.ts:973-989` + `:1499-1574` into a docs page. | Integrators want one-page reference, not "read the source." The table is already in code; just mirror it as docs.                                                                  | open   |
| 9.6 | Refresh `docs/DEVNET_STATUS.md` after first deploy: populate "Sample issuer / schema" placeholders, set `Last live deploy`. Also fix the stale claim that "the high-level verifier SDK does not exist yet" (lines 172-176).                                | This is the public source of truth. After deploying, the placeholders and the stale claim about the missing wrapper both move.                                                    | open   |
| 9.7 | Refresh `plan/VERIFIER_SDK_SHAPE.md` from "Status: Design sketch (not shipped)" to "Status: Shipped (verifier/src/index.ts:1186-1450)".                                                                                                                          | Design doc should reflect reality. Anyone reading the design sketch today gets the wrong impression.                                                                              | open   |
| 9.8 | Refresh `plan/DEVNET_ROLLOUT_PUNCHLIST.md` Tier A3 from "open" to "closed" with the file:line citation.                                                                                                                                                          | The punchlist drives the team's view of remaining work.                                                                                                                            | open   |
| 9.9 | Refresh `docs/CURRENT_STATE.md:110` — `@solid-protocol/verifier` row from `[~]` "high-level wrapper not yet authored" to `[X]` "shipped".                                                                                                                       | Per-component snapshot must match reality.                                                                                                                                         | open   |
| 9.10 | Refresh `docs/SDK_INTEGRATOR_MATRIX.md:29` row for `@solid-protocol/verifier` from "high-level wrapper not yet" to "shipped, see `SolidVerifier`".                                                                                                              | Integrator matrix is the first place a Solana app dev looks.                                                                                                                       | open   |
