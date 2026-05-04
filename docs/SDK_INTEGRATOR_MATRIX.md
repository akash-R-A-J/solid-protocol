# SDK Integrator Matrix

Status: Design. Drafted 2026-05-02.

This document is the source of truth for which SolID SDK package each
integrator type uses. It defines the per-actor method surface, the
new `@solid-protocol/dao` package that does not exist today, and the
extensions to `@solid-protocol/light` that operators need.

## Companion docs

- `plan/VERIFIER_SDK_SHAPE.md` -- detailed sketch for
  `@solid-protocol/verifier`.
- `docs/CREDENTIAL_DELIVERY_DESIGN.md` -- the package format that
  flows between issuer and holder.
- `docs/HOLDER_STORAGE_AND_WALLET.md` -- the holder side and the
  Wallet Standard interface.
- `plan/DEVNET_ROLLOUT_PUNCHLIST.md` -- task-tracking; the new dao
  package is a Tier B candidate (decision pending; see "Tiering" below).

## The five actors

SolID has five integrator types. Each has a different question they
need answered, a different SDK package, and a different acceptance
threshold for what "easy integration" means.

| Actor | Question | Package | Status today |
| --- | --- | --- | --- |
| App / Verifier | "Can this wallet do this action?" | `@solid-protocol/verifier` | Low-level shipped; high-level `SolidVerifier` shipped (`ts-sdk/packages/verifier/src/index.ts:1186`). Pending npm publish + devnet defaults. |
| Issuer | "Can I issue and revoke this credential?" | `@solid-protocol/issuer` | Mid-level shipped (incl. `generateSubgroupProof`); high-level wrapper not yet. |
| Holder / Wallet | "What am I revealing? What stays private?" | `@solid-protocol/holder` | Has `generateBatchProof` + `generateProof`; user-facing methods (`importCredential`, `canSatisfy`, etc.) not yet. |
| DAO Member / Governor | "Should I admit / slash this issuer?" | `@solid-protocol/dao` | **Does not exist.** New package. |
| Indexer / Operator | "Are the trust roots and artifacts healthy?" | `@solid-protocol/light` (extended) | SPL AC adapter shipped; subscription/health surface not yet. |

The high-level facade `@solid-protocol/sdk` re-exports the most
common methods from the actor packages so a developer who is not
sure which actor they are can `import { SolidVerifier, SolidIssuer,
SolidHolder } from "@solid-protocol/sdk"` and pick at the call site.

## Per-actor method matrix

### `@solid-protocol/verifier` (Apps / Verifiers)

Already specified in detail in `plan/VERIFIER_SDK_SHAPE.md`. Summary:

| Method | What it does | Status |
| --- | --- | --- |
| `defineRequirement(spec)` | Resolve a `RequirementSpec` to a typed `Requirement` with public-input encoding. | Shipped (`verifier/src/index.ts:1217`). |
| `requestProof({ wallet, requirement, transport })` | Send a proof request to the holder; return a handle. | Shipped (`verifier/src/index.ts:1255`). |
| `verifyProof({ proof, requirement, payer })` | Submit on-chain; return typed `VerificationResult`. | Shipped (`verifier/src/index.ts:1286`). |
| `verifyRequirement(args)` | One-shot: defineRequirement + requestProof + verifyProof. | Shipped (`verifier/src/index.ts:1336`). |
| `health()` | Probe artifact pins, program IDs, indexer; for status pages. | Shipped (`verifier/src/index.ts:1356`). |
| `loadArtifact(kind)` | Fetch + SHA-256-verify a circuit artifact. | Shipped (`verifier/src/index.ts:1408`). |
| `walletAdapterTransport(wallet)`, `httpTransport(endpoint)` | Pluggable transports for `requestProof`. | Shipped (`verifier/src/index.ts:1452`, `:1479`). |
| `explainVerificationError(err, detail?)` | 16-variant typed-error -> human-readable + suggested action. | Shipped (`verifier/src/index.ts:1499`). |
| `verifyOnChainV2(args)` | Low-level chunked-upload orchestration. | Shipped (SEC-054). |
| `buildVerifyBatchProofIx`, `derivePdas`, `checkIssuerStatus` | Lower-level building blocks; re-exported. | Shipped. |

### `@solid-protocol/issuer` (Issuers)

| Method | What it does | Status |
| --- | --- | --- |
| `register({ authority, metadata, stakeAmount })` | Register issuer, generate subgroup proof, submit; idempotent. | TODO (high-level wrapper) |
| `issueCredential({ holder, schema, fields, expiresAt, delivery })` | Build commitment, sign, CPI on-chain, package for delivery. | TODO (high-level wrapper) |
| `revokeCredential(credentialId)` | Bump revocation nonce; emit event. | TODO |
| `rotateMetadata(updates)` | Update issuer metadata. | TODO |
| `withdrawAfterRevoke()` | Post-cooldown stake withdrawal (SEC-061). | TODO (helper does not exist; on-chain ix shipped). |
| `generateSubgroupProof(bjjPrivKey)` | Phase E.4 SDK helper. | Shipped. |
| `issueCredential` (low-level) | Shipped builder for the on-chain ix. | Shipped. |
| `vote_on_issuer`, `slash_issuer` | NOT in this package. See `@solid-protocol/dao`. | n/a |

### `@solid-protocol/holder` (Holders / Wallets)

| Method | What it does | Status |
| --- | --- | --- |
| `importCredential(pkg)` | Decrypt + validate + store. | TODO |
| `importFromClaimLink(url)` | Fetch encrypted package, then `importCredential`. | TODO |
| `listCredentials(filter?)` | Return summaries. | TODO |
| `canSatisfy(requirement)` | Cheap check; no proof. | TODO |
| `previewDisclosure(requirement)` | Plain-language reveal/hide preview. | TODO |
| `generateProof(requirement)` | Full Groth16 fullProve + serialization. | Partial (`generateProof` low-level shipped; needs `Requirement` integration). |
| `exportEncryptedBackup(passphrase)` | Argon2id-protected backup. | TODO |
| `restoreFromBackup(blob, passphrase)` | Inverse. | TODO |
| `deleteCredential(id)` | Local removal. | TODO |
| `MerkleProofAdapter` | Builds the Merkle inclusion paths. | Shipped. |

The Wallet Standard `solid:credentials@1` feature interface is
defined separately in `docs/HOLDER_STORAGE_AND_WALLET.md` and is
implemented by vendoring this package; the methods above are the
canonical implementations.

### `@solid-protocol/dao` (DAO Members / Governance) — NEW PACKAGE

This package does not exist today. Today, governance operations are
only callable by hand-rolling Anchor instructions or running
`scripts/bootstrap_issuer.ts`. For the DAO to be a real product, it
needs its own SDK.

**Tiering decision.** Today this is filed as a *candidate* Tier B
item -- needed for partner pilots that want to demonstrate "DAO
governance over issuer admission" but not strictly needed for the
Tier A launchpad demo. Specifically the punchlist already covers a
sample DAO governance quorum on devnet (Tier B5 cross-reference).
Promote to Tier B if the first partner pilot asks for governance UX.

#### Methods

| Method | What it does | Underlying ix |
| --- | --- | --- |
| `listPendingIssuers({ limit, cursor? })` | Paginated list of registered-but-not-approved issuers. | RPC; reads `IssuerAccount` accounts via `getProgramAccounts`. |
| `getIssuer(authority)` | Detail view: status, stake, BJJ pubkey, metadata, vote tally, registration time. | RPC. |
| `voteOnIssuer({ issuer, voter, voteToken, weight, position })` | Cast a vote within the active voting window. | `vote_on_issuer`. |
| `releaseVote({ voter, issuer })` | Release the vote-time stake lock so the voter can unstake. Required after voting period closes. | `release_vote`. |
| `finalizeVoting(issuer)` | Anyone can call; flips the issuer to Approved or Rejected based on final tally. | `finalize_voting`. |
| `slashIssuer({ issuer, evidence, multisig? })` | DAO-authorized slashing. Lamports move atomically to treasury PDA. | `slash_issuer`. Pre-mainnet: must be Squads-gated (SEC-013). |
| `submitFraudProof({ issuer, proof })` | Slash + bump revocation nonce + replace_leaf in one ix. | `submit_fraud_proof`. Same pre-mainnet caveat. |
| `treasuryBalance()` | Returns DAO treasury PDA balance + recent slash events. | RPC. |
| `proposeAuthorityTransfer(newAuthority)` | Propose-accept pattern (SEC-016). | `propose_authority_transfer`. |
| `acceptAuthorityTransfer()` | Inverse. | `accept_authority_transfer`. |
| `subscribeToVoteEvents(callback)` | Live-stream `IssuerVoteCast` events. | Geyser-equivalent over the indexer API. |
| `subscribeToSlashEvents(callback)` | Live-stream `IssuerSlashed`. | Same. |

#### Integration shape

```ts
import { SolidDao } from "@solid-protocol/dao";

const dao = new SolidDao({ cluster: "devnet" });

const pending = await dao.listPendingIssuers({ limit: 20 });
for (const issuer of pending) {
  console.log(`${issuer.metadata.name}: ${issuer.stake / 1e9} SOL, ${issuer.voteCount} votes`);
}

await dao.voteOnIssuer({
  issuer: pendingIssuer.authority,
  voter: voterKeypair,
  voteToken: governanceMint,
  weight: 1000n,
  position: "approve",
});
```

#### Why not just expose this from `@solid-protocol/issuer`?

Because issuers and DAO governors are different roles with different
access control needs. An issuer dashboard does not need to expose
slashing UI; a DAO governance app does not need to issue
credentials. Bundling produces auth-context confusion ("did I just
authorize myself to slash, or to issue?"). Separate packages keep
the boundary clean.

### `@solid-protocol/light` (Indexers / Operators) — EXTENDED

Today the package exposes `LocalReplicaAdapter`, PDA derivers, and
SPL AC ID exports. For devnet, indexers and status-page operators
need more.

| Method | What it does | Status |
| --- | --- | --- |
| `LocalReplicaAdapter` | Reads validator log to maintain a local Merkle replica. | Shipped. |
| `RemoteIndexerAdapter` | Pulls Merkle proofs from `indexer.solid.example`. | TODO (Tier B1 deliverable). |
| `subscribeToProofVerifiedEvents(callback)` | Live-stream `ProofVerified`. | TODO |
| `subscribeToCredentialIssuedEvents(callback)` | Same for `CredentialIssued`. | TODO |
| `subscribeToRevocationEvents(callback)` | Same for `IssuerRevoked` / `CredentialRevoked`. | TODO |
| `validateRoots()` | Health probe: are SchemaTreeBinding / GlobalStateBinding / IssuerTreeBinding all consistent with their underlying SPL AC trees? | TODO |
| `healthProbe()` | High-level "everything OK?" -- returns a structured report consumed by the status page. | TODO |
| `derivePdas` | Already shipped. | Shipped. |
| `parseEvent(logs)` | Already shipped. | Shipped. |

The subscription methods are intentionally `subscribe`-style
callbacks (not async iterators) because the underlying transport is
SSE/websocket, and most consumers want fire-and-forget. An async
iterator wrapper can be added in v1.1 without breaking the
callback API.

### `@solid-protocol/sdk` (High-level facade)

Re-exports the actor packages. Adds a top-level `SolID` class that
auto-detects the actor based on the methods called:

```ts
import { SolID } from "@solid-protocol/sdk";

const solid = new SolID({ cluster: "devnet" });

// Verifier:
await solid.verifier.verifyRequirement({ ... });

// Issuer:
await solid.issuer.issueCredential({ ... });

// Holder:
await solid.holder.generateProof(req);

// DAO governor:
await solid.dao.voteOnIssuer({ ... });

// Indexer / operator:
const events = solid.light.subscribeToProofVerifiedEvents(handle);
```

This is purely a convenience layer; it adds no logic on top of the
actor packages.

## Cross-package dependencies

```
                      @solid-protocol/sdk (facade)
                              |
        +--------+------------+------+----------+
        |        |            |      |          |
   verifier  issuer       holder    dao       light
        |        |            |      |          |
        +--------+--+---------+------+----------+
                    |
                  core (WASM bridge)
```

Every actor package depends on `core` (Poseidon, BJJ, nullifier
primitives via WASM). `verifier`, `issuer`, `holder`, and `dao` may
also depend on `light` for SPL AC adapters / Merkle proofs / event
parsing.

No actor package depends on another actor package. This is
deliberate: an issuer dashboard ships without the holder code in
its bundle.

## Versioning

All packages are versioned together under the SolID protocol
version. Today: `0.6.1` across the board.

Breaking changes bump the minor version (`0.7.0`) until the protocol
hits `1.0`, after which breaking changes bump the major. The
`@solid-protocol/wallet-standard-spec` feature name version
(`solid:credentials@1`) is independent and only bumps when the
feature interface itself breaks.

## Bundle size targets

For browser-side actors (verifier, holder, dao):

| Package | Target gzipped | Today |
| --- | --- | --- |
| `@solid-protocol/core` | <80 KB (WASM) | ~70 KB |
| `@solid-protocol/verifier` | <40 KB | TBD |
| `@solid-protocol/issuer` | <40 KB | TBD |
| `@solid-protocol/holder` | <100 KB (excludes snarkjs) | TBD |
| `@solid-protocol/dao` | <30 KB | n/a (new) |
| `@solid-protocol/light` | <30 KB | TBD |
| `@solid-protocol/sdk` (facade only) | <20 KB | TBD |

Snarkjs is the long pole for `holder` -- ~600 KB gzipped. Mitigation:
load snarkjs dynamically only when `generateProof` is first called,
not at import time. Document this in the bundling guide.

## Open questions

1. **Should `@solid-protocol/dao` ship as Tier A or Tier B?**
   Default: B (not blocking the launchpad demo). Promote to A if a
   partner pilot asks for governance UX in the demo set.
2. **Should `@solid-protocol/light` split into `light-replica` (the
   local SPL AC adapter) and `light-indexer` (the remote indexer
   client)?** Default: keep them together until bundle size hurts.
3. **Should the Wallet Standard feature spec be a separate package
   `@solid-protocol/wallet-standard-spec`?** Default: yes, because
   wallets vendor it independently of the holder runtime. Lazy
   ship: when the first wallet vendor opts in.
4. **Type bridging across packages.** Today `Requirement`,
   `CredentialPackage`, `Predicate`, `VerificationError` are
   referenced by every actor package. The clean answer is to
   centralize them in `@solid-protocol/types` (a tiny no-runtime
   package) so circular dependencies do not creep in. Default: do
   this in v1.0; before then, duplicate-and-keep-in-sync.
5. **Does each actor package need its own README + npm registry
   listing?** Default: yes, because npm is where most developers
   discover SDKs. The package READMEs cross-link to the main repo
   docs.

## Implementation order

This matches `plan/DEVNET_ROLLOUT_PUNCHLIST.md`:

1. ~~**Tier A3** -- `@solid-protocol/verifier` high-level wrapper.~~
   **Implemented** (`verifier/src/index.ts:1186-1450`); remaining work
   is npm publish + devnet defaults, tracked under
   `DEVNET_ROLLOUT_PUNCHLIST.md` A3 substeps.
2. **Tier A4** -- `@solid-protocol/holder` user-facing methods
   (`importCredential`, `importFromClaimLink`, `canSatisfy`,
   `previewDisclosure`, `exportEncryptedBackup`, `restoreFromBackup`).
3. **Tier A5** -- `@solid-protocol/issuer` high-level wrapper +
   issuer CLI on top.
4. **Tier B1 dependency** -- `@solid-protocol/light`
   `RemoteIndexerAdapter` + subscription surface.
5. **Tier B candidate** -- `@solid-protocol/dao` first methods
   (listPendingIssuers, voteOnIssuer, finalizeVoting). Slashing
   methods wait for Squads multisig (Tier D2).

## Acceptance criteria for "the SDK is done"

The SDK is complete enough to ship public devnet when:

- A new app developer can `npm install @solid-protocol/verifier` and
  reach `result.verified === true` in under 15 minutes.
- A new issuer can `npm install -g @solid-protocol/issuer-cli` and
  register + issue + deliver a credential in under 30 minutes.
- A new holder can claim a credential and generate a proof in under
  3 minutes.
- A DAO governor can list pending issuers and cast a vote in under
  10 minutes.
- An indexer operator can stand up `RemoteIndexerAdapter` + event
  subscriptions and get a working dashboard in under an hour.

Each of those numbers is the time-to-first-success; it is the
primary success metric for the integration moat described in
`plan/PRODUCT_SURFACE_DEVNET_LAUNCH_PLAN.md`.

---

*End. Five actors, five packages, one facade, one source of truth
per role.*
