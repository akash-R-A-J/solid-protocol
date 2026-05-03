# SolID Product Surface and Devnet Launch Plan

Status: Draft
Date: 2026-05-02
Audience: product, engineering, docs, ecosystem partners

## Companion documents

This plan is the strategic narrative. The tactical follow-ups live in:

- `plan/DEVNET_ROLLOUT_PUNCHLIST.md` -- per-task tier list (A/B/C/D)
  with effort estimates, dependencies, and acceptance criteria.
  **Tier A and B are the agreed bar before public devnet launch.**
- `plan/VERIFIER_SDK_SHAPE.md` -- design sketch for
  `@solid-protocol/verifier` high-level wrapper (Tier A3).
- `docs/CREDENTIAL_DELIVERY_DESIGN.md` -- issuer-to-holder package
  format and three delivery channels (claim link, downloadable JSON,
  Wallet Standard handoff).
- `docs/HOLDER_STORAGE_AND_WALLET.md` -- holder-side storage
  architecture, deterministic key derivation from wallet signature,
  and the proposed Wallet Standard `solid:credentials@1` feature.
- `docs/SDK_INTEGRATOR_MATRIX.md` -- five-actor SDK package surface
  including the new `@solid-protocol/dao` package.
- `docs/DEVNET_STATUS.md` -- live program IDs, VK pins, artifact
  hashes, and known limitations.

## Executive Thesis

SolID should not be presented first as "ZK identity infrastructure." That is
true technically, but it is not the user-facing reason someone adopts it.

The strongest product positioning is:

> Private eligibility and compliance verification for Solana apps, without
> apps storing user PII.

The protocol's cryptography is the moat. The product surface must sell the
outcome:

- Apps can gate access without collecting documents.
- Users can prove facts without revealing raw identity data.
- Issuers can issue revocable credentials into a Solana-native trust network.
- Verifiers can integrate with one SDK/API call instead of running their own
  credential, proof, revocation, and issuer-trust infrastructure.

The best first wedge is not "identity for everything." It is:

> Private KYC / eligibility gates for Solana DeFi, launchpads, DAOs, games,
> and payments apps.

This is the use case where the pain is obvious: apps need compliance or access
control, but do not want to hold PII or build identity infrastructure.

## Product Areas to Focus On

### 1. Verifier Product

This should be the first product surface.

Why:

- Verifiers feel the clearest pain: "Can this wallet do this action?"
- Verifiers are most likely to pay.
- A verifier integration creates demand for issuers and holder UX.
- If verifier integration is hard, the system will not be adopted no matter
  how strong the cryptography is.

Target user:

- Solana app developer.
- DeFi / launchpad / DAO / game team.
- Compliance-conscious protocol that does not want to store PII.

Core job:

> Let a Solana app privately check whether a wallet satisfies a requirement.

Target API:

```ts
const result = await solid.verify({
  requirement: "kyc:basic",
  wallet,
  action: "join_launchpad_pool",
});

if (result.verified) {
  allowUser();
}
```

What this hides from the integrator:

- Circuits.
- Proof buffers.
- Merkle paths.
- Issuer tree roots.
- Schema tree roots.
- Artifact loading.
- Nullifier construction.
- Revocation checks.
- Account ordering.
- Public input byte ordering.

Implementation surface:

- `@solid-protocol/verifier`
  - `createRequirement(...)`
  - `requestProof(...)`
  - `verifyProof(...)`
  - `verifyOnChain(...)`
  - `explainVerificationError(...)`
- Optional hosted REST API:
  - `POST /v1/requirements`
  - `POST /v1/proof-requests`
  - `POST /v1/verify`
  - `GET /v1/issuer/:pubkey`
  - `GET /v1/schemas`

Required verifier errors:

- `MISSING_CREDENTIAL`
- `UNSUPPORTED_SCHEMA`
- `EXPIRED_CREDENTIAL`
- `REVOKED_ISSUER`
- `REVOKED_CREDENTIAL`
- `PROOF_REPLAYED`
- `PROOF_GENERATION_FAILED`
- `RPC_UNAVAILABLE`
- `ARTIFACT_PIN_MISMATCH`
- `USER_REJECTED`

Success metric:

- A developer can add a private eligibility gate to a demo app in under
  15 minutes.

### 2. Issuer Product

This should come second, after the verifier flow is understandable.

Why:

- The trust network only matters if credible issuers can onboard.
- Issuers should not need to run raw scripts.
- Issuers are the source of the credentials that make the verifier product
  valuable.

Target user:

- KYC provider.
- DAO admin.
- University / certification body.
- Protocol team issuing membership or compliance credentials.
- Enterprise or regulated issuer.

Core job:

> Issue a revocable credential to a user without exposing the credential data
> publicly.

Issuer dashboard features:

- Register issuer.
- Upload and manage metadata.
- Show registry status: pending, approved, revoked, cooldown.
- Generate issuer BabyJubJub keypair locally or connect an HSM/KMS-backed key.
- Issue credential to holder.
- Revoke credential or issuer.
- View issued credential count.
- View issuer tree enrollment status.
- Export audit log.

Issuer SDK features:

- `registerIssuer(...)`
- `generateIssuerSubgroupProof(...)`
- `issueCredential(...)`
- `revokeCredential(...)`
- `rotateIssuerMetadata(...)`
- `exportCredentialPackage(...)`

Success metric:

- A test issuer can register and issue a credential without touching Anchor
  account ordering, raw scripts, or circuit artifacts.

### 3. Holder Product

This should come third, but it must exist before a public devnet demo feels
real.

Why:

- Holders need to understand what they received and what they reveal.
- If holder UX is confusing, privacy becomes invisible or scary.
- The holder flow is what makes the product feel magical instead of purely
  infrastructural.

Target user:

- Wallet user.
- DAO member.
- DeFi user.
- Launchpad participant.
- Credential recipient.

Core job:

> Receive a credential, store it privately, and prove an attribute when asked.

Minimum viable holder app:

- Claim credential from issuer link.
- Encrypt and store credential locally.
- Show issuer name, schema, expiration, and revocation status.
- Generate proof for a verifier request.
- Show disclosure text before proving:
  - "You are proving you passed KYC."
  - "You are not revealing your name, ID number, address, or date of birth."
  - "This proof is scoped to this app and action."

Holder SDK features:

- `importCredentialPackage(...)`
- `listCredentials(...)`
- `canSatisfyRequirement(...)`
- `generateProofForRequirement(...)`
- `exportEncryptedBackup(...)`

Success metric:

- A non-technical user can claim a credential and generate a proof without
  reading protocol documentation.

### 4. Hosted Infrastructure Product

This should be optional but available early.

Why:

- Many teams will not run indexers, artifact hosting, proof-path builders, or
  custom RPC infrastructure during evaluation.
- Hosted defaults reduce integration friction dramatically.
- Advanced teams can still self-host later.

Hosted components:

- Artifact CDN with SHA-256 pins.
- Schema registry API.
- Issuer registry API.
- Merkle proof/indexer API.
- Devnet faucet/helper API.
- Status page for devnet programs, roots, and artifacts.
- Example issuer credentials for demo flows.

Self-hosting should remain possible, but not required for first integration.

Success metric:

- First-time integrator can use hosted devnet defaults with no infra setup.

### 5. Docs, Examples, and Videos

This is a product surface, not an afterthought.

Why:

- The system is complex. Docs must compress complexity.
- Developers judge infrastructure by time-to-first-success.
- Videos help non-crypto stakeholders understand why private verification
  matters.

Docs to ship before public devnet:

- `README`: product-first overview.
- `docs/GETTING_STARTED.md`: 15-minute verifier integration.
- `docs/VERIFIER_INTEGRATION.md`: frontend + backend examples.
- `docs/ISSUER_GUIDE.md`: issue first credential.
- `docs/HOLDER_FLOW.md`: claim and prove credential.
- `docs/DEVNET_QUICKSTART.md`: public devnet addresses, faucets, sample
  issuer, sample schemas, artifact pins.
- `docs/ERROR_CODES.md`: human-readable errors and fixes.
- `docs/ARCHITECTURE_PRODUCT.md`: product architecture, not cryptographic
  deep dive.

Videos to ship:

- 90-second product explainer:
  - Problem: apps need eligibility checks but should not store PII.
  - Solution: private proof of eligibility on Solana.
  - Demo: user joins gated app without revealing personal data.
- 5-minute developer quickstart:
  - Install SDK.
  - Create requirement.
  - Request proof.
  - Verify result.
- 8-10 minute issuer demo:
  - Register issuer.
  - Issue credential.
  - Holder claims credential.
  - Verifier checks proof.
- 2-minute trust/revocation demo:
  - Issuer revoked.
  - Old proof stops verifying.
  - App does not need to handle PII.

Success metric:

- A developer can watch the quickstart and integrate the verifier SDK without
  asking the core team how trees, nullifiers, or VKs work.

## Execution Order

### Phase 0: Make Public Devnet Safe Enough

Goal:

Public devnet should not be embarrassing, misleading, or hijackable.

Required before product launch:

- Fix subgroup verifier config authority so the first caller cannot hijack the
  subgroup VK authority.
- Sync `sec/SECURITY_REGISTRY.md`, `docs/CU_BUDGET.md`, and
  `tests/cu_baselines.json` with the real SEC-048 state.
- Ensure subgroup wasm/zkey artifacts are SHA-256-pinned, not only the subgroup
  VK.
- Run full localnet e2e from clean validator.
- Publish exact devnet program IDs and artifact hashes.
- Create a devnet status document:
  - program IDs,
  - deployed slots,
  - current VK hashes,
  - sample schema hashes,
  - sample issuer pubkeys,
  - known limitations.

Exit criteria:

- Clean e2e reaches `verified: true`.
- A fresh devnet user cannot initialize or own any protocol singleton that
  should belong to the protocol authority.
- Docs do not mention removed bypasses as active behavior.

### Phase 1: Verifier Demo First

Goal:

Build the smallest compelling app that proves the value.

Flagship demo:

> Private launchpad eligibility gate.

Flow:

1. User connects Solana wallet.
2. App says: "This pool requires basic KYC and non-restricted jurisdiction."
3. User clicks "Prove eligibility."
4. Holder app generates proof.
5. Verifier checks proof on Solana.
6. App allows entry without seeing PII.

Why this is the best demo:

- It is Solana-native.
- It maps to a real pain point.
- It demonstrates privacy, compliance, issuer trust, revocation, and replay
  protection in one flow.
- It is better than a generic "prove age" demo because it has a direct buyer:
  DeFi apps and launchpads.

Careful product claim:

- Do not claim "impossible anywhere else."
- Claim: "SolID makes this Solana-native and composable without each app
  storing PII or building its own credential stack."

Implementation:

- Next.js demo app.
- One sample requirement: `kyc.basic && jurisdiction.allowed`.
- One test issuer.
- One sample holder credential.
- One verifier SDK call.
- One clear success/failure UI.

Exit criteria:

- Demo can be run by a new developer from docs.
- Demo records no raw PII.
- Demo shows the exact user disclosure before proof generation.

### Phase 2: Verifier SDK and Hosted Devnet

Goal:

Turn the demo into a reusable integration path.

Build:

- `@solid-protocol/verifier` high-level API.
- Hosted devnet artifact config.
- Hosted schema/issuer metadata endpoint.
- Example backend verification route.
- Example frontend proof request button.

Friction reduction:

- Default config should work on devnet.
- No manual artifact paths for first-time users.
- No manual account derivation for first-time users.
- No raw Anchor account lists in quickstart.
- Clear typed errors.

Developer quickstart shape:

```bash
npm install @solid-protocol/verifier
```

```ts
import { SolidVerifier } from "@solid-protocol/verifier";

const solid = new SolidVerifier({ cluster: "devnet" });

const result = await solid.verifyRequirement({
  wallet,
  requirement: {
    schema: "kyc_basic_v1",
    predicates: [
      { field: "kyc_level", op: ">=", value: 1 },
      { field: "restricted_jurisdiction", op: "==", value: 0 },
    ],
    action: "join_launchpad_pool",
  },
});
```

Exit criteria:

- Verifier quickstart is under 15 minutes.
- A demo app can copy-paste the integration.
- Error states are understandable without reading source code.

### Phase 3: Issuer Credential Delivery

Goal:

Make credential issuance usable without scripts.

Build:

- Credential package format.
- Encrypted delivery envelope.
- Claim link format.
- Issuer dashboard or minimal issuer CLI.
- Holder import flow.

Credential package should include:

- schema hash,
- schema metadata,
- issuer authority,
- issuer BJJ pubkey,
- issuer signature,
- commitment data needed for proof generation,
- expiration,
- revocation metadata,
- tree location / proof adapter hint,
- package version.

Delivery options:

- Devnet MVP: encrypted claim link or downloadable encrypted JSON.
- Later: wallet-native handoff, mobile deep link, or secure inbox.

Friction reduction:

- Issuer should not ask the holder to copy raw JSON.
- Holder should not need to understand schema hashes.
- Verifier should not need to know how credential was delivered.

Exit criteria:

- Issuer can issue to holder.
- Holder can claim.
- Holder can prove to verifier.
- Entire flow works in a browser.

### Phase 4: Holder UX

Goal:

Make privacy visible and understandable.

Build:

- Holder credential page.
- Credential list.
- Proof request review screen.
- Disclosure statement.
- Revocation/expiration status.
- Backup/export.

Important UX copy:

- "This proof reveals only that you satisfy the requirement."
- "Your raw credential data is not sent to the app."
- "This proof is scoped to this app and action."
- "Reusing the same proof will fail; generate a fresh proof when requested."

Friction reduction:

- Use plain words: "eligible", "expired", "revoked", "missing credential."
- Hide proof internals.
- Explain wallet signatures separately from identity proofs.

Exit criteria:

- A user can explain what they proved after the flow.
- A user can distinguish "wallet connection" from "credential proof."

### Phase 5: Partner Pilot

Goal:

Get one real verifier-like partner and one issuer-like partner.

Best pilot target:

- A Solana launchpad, gated token sale, DAO membership app, or DeFi app that
  needs eligibility checks.

Pilot promise:

- "You can gate one sensitive action without collecting user PII."

What to measure:

- Time to integrate.
- Proof generation success rate.
- Verification latency.
- User drop-off.
- Error frequency.
- Number of successful eligibility checks.
- Partner willingness to pay for hosted API / support.

Exit criteria:

- One real app can integrate without core-team handholding.
- One issuer can issue a credential through the issuer flow.

## Friction Reduction Plan

### Developer Friction

Problem:

The protocol has many moving pieces.

Plan:

- Ship one default devnet config.
- Wrap raw accounts in SDK helpers.
- Provide hosted artifact URLs and pins.
- Provide hosted proof-path/indexer API.
- Expose typed errors.
- Provide copy-paste examples.

Principle:

The developer should think in requirements, not circuits.

### Issuer Friction

Problem:

Issuers do not want to run setup scripts or understand subgroup proofs.

Plan:

- Build issuer dashboard or CLI.
- Generate subgroup proof automatically during registration.
- Show clear status:
  - registered,
  - pending vote,
  - approved,
  - enrolled in issuer tree,
  - revoked.
- Provide credential templates.
- Provide audit log.

Principle:

The issuer should think in credential templates and recipients, not protocol
accounts.

### Holder Friction

Problem:

Users do not understand ZK proof semantics.

Plan:

- Use disclosure-first UX.
- Show plain-language proof purpose.
- Let users inspect issuer and expiration.
- Store credentials encrypted.
- Make backup/export obvious.

Principle:

The holder should know what is revealed and what is not revealed.

### Operator Friction

Problem:

Running the full stack requires artifacts, RPC, trees, roots, and monitoring.

Plan:

- Hosted devnet environment.
- Devnet status page.
- One command to refresh artifacts.
- One command to verify program IDs and VK hashes.
- One runbook for devnet reset.

Principle:

Operators should have checklists and health checks, not tribal knowledge.

## Devnet Launch Checklist

### Protocol Readiness

- Public devnet deployment at canonical program IDs.
- Program ID consistency gate passing.
- Subgroup verifier config authority fixed.
- SEC-048 docs synced to actual implementation.
- Subgroup wasm/zkey/VK hashes pinned.
- Batch circuit wasm/zkey/VK hashes pinned.
- Full clean localnet e2e green.
- Devnet smoke test green.
- CU baselines refreshed for new `register_issuer` cost.
- Known limitations documented.

### Product Readiness

- Verifier SDK quickstart.
- Issuer CLI or dashboard MVP.
- Holder claim/prove MVP.
- One flagship demo app.
- Hosted artifact config.
- Hosted sample issuer.
- Hosted sample schema.
- Sample credentials for demo wallets.

### Docs Readiness

- Product-first README.
- Devnet quickstart.
- Verifier integration guide.
- Issuer guide.
- Holder flow guide.
- Error code guide.
- Architecture overview.
- Security assumptions page.
- Known limitations page.

### Video Readiness

- 90-second product explainer.
- 5-minute verifier integration.
- Issuer credential issuance walkthrough.
- Holder claim/prove walkthrough.
- Revocation demo.

### Partner Readiness

- One-page pitch for Solana apps.
- One-page pitch for issuers.
- Demo script.
- Integration support checklist.
- Feedback form.

## Best Flagship Use Case

### Private Launchpad Eligibility Gate

Problem:

Launchpads and DeFi protocols often need to restrict participation based on
KYC, geography, accreditation, membership, or sybil resistance. The usual
options are poor:

- collect and store PII directly,
- outsource to a centralized KYC iframe,
- maintain an allowlist,
- use a non-private soulbound token,
- rely on manual review,
- avoid the feature entirely.

SolID demo:

- Issuer verifies user off-chain and issues a private credential.
- User stores credential privately.
- Launchpad asks for proof of eligibility.
- User proves eligibility without revealing raw PII.
- Launchpad verifies proof on Solana.
- Nullifier prevents replay.
- Revocation invalidates future proofs.

Why this is compelling on Solana:

- Solana apps are latency- and UX-sensitive.
- Solana apps prefer native programs and direct composability.
- On-chain verification and account-compression roots let multiple apps share
  one credential trust surface.
- Apps avoid building their own identity backend.

What to show in demo:

1. User connects wallet to launchpad.
2. Launchpad says "Basic KYC required."
3. User clicks "Prove eligibility."
4. Holder screen explains what is revealed.
5. Proof is generated and submitted.
6. App unlocks participation.
7. Revocation demo: issuer revoked, proof fails.

Demo success copy:

> Eligible. Your proof verified on Solana. No personal data was shared with
> this app.

Demo failure copy:

> Not eligible. Your credential is missing, expired, revoked, or does not
> satisfy this requirement.

## Competitive Strategy

Do not compete as "another identity protocol."

Compete as:

- fastest Solana-native private eligibility integration,
- best developer experience,
- best issuer-to-holder flow,
- clearest privacy UX,
- easiest hosted devnet,
- strongest revocation-aware credential story.

Competitors may have good crypto. Many will lose on:

- integration time,
- unclear user disclosure,
- poor issuer tooling,
- missing revocation,
- non-Solana-native flows,
- no usable devnet sandbox.

SolID can win if the first experience feels simple:

> Install SDK, request proof, verify eligibility.

## Competitive Landscape

Research date: 2026-05-02.

Scope and evidence standard:

- This comparison is based on public websites, public docs, public API docs,
  and public blog posts available during research.
- It does not assume private enterprise features, non-public roadmaps, hidden
  partnerships, pricing discounts, or unpublished security audits.
- "Unknown" means no reliable public source was found in this research pass.
- A competitor being strong in one column does not mean it is weak overall.
  It means its public product is optimized for a different job.

### Market Categories

SolID should not treat every identity product as the same kind of competitor.
There are four distinct markets around it.

1. Community and token gating

Examples: Matrica, Collab.Land, Guild.xyz, Lit Protocol.

These products are usually the best choice when the job is:

- verify wallet ownership,
- inspect token or NFT holdings,
- grant Discord / Telegram / web access,
- manage community roles,
- run quests or community campaigns.

They are not primarily designed to let a Solana program verify a private
credential proof without seeing the user's underlying data.

2. ZK identity and verifiable credentials

Examples: zkMe, Privado ID, Holonym / Human ID, World ID, Sismo-era products.

These are the closest conceptual competitors. They support privacy-preserving
identity claims, proof of personhood, KYC-like flows, or verifiable credentials.
Some are stronger than SolID today on maturity, user base, issuer tooling, and
production integrations. SolID's possible advantage is narrower: Solana-native
program verification, compressed credential roots, revocation-aware issuer
trees, and SDKs aimed directly at Solana dApps.

3. Compliance / passport primitives

Examples: Quadrata, Civic Pass / Gateway-style identity passes, Synaps-like
KYC providers, Fractal ID / Persona / Sumsub-style Web2 KYC providers.

These can be better choices when the buyer wants a known compliance vendor,
human document review, enterprise onboarding, or regulatory workflows more than
on-chain privacy/composability.

4. Non-crypto substitutes

Examples: allowlists, centralized KYC iframes, hosted risk APIs, manual review,
database flags, Web2 OAuth / email verification.

These are often easier to ship. They win when the buyer does not care about
on-chain composability, user-held credentials, privacy-preserving proofs, or
decentralized revocation.

### Matrica Comparison

Matrica is a real competitor for the broad phrase "verify users," but it is
not the same product category as SolID if SolID stays focused on private
credential proofs for Solana apps.

Publicly documented Matrica strengths:

- Matrica describes itself as an all-in-one community management solution with
  token verification and questing across Solana, Bitcoin, Ethereum, Polygon,
  Eclipse, Base, Monad, and ApeChain.
- Matrica supports Discord holder verification, sales bots, multi-collection
  support, event management, Telegram gating, amount- and attribute-based
  roles, SPL / ERC-20 / BRC-20 support, collection snapshots, cNFT support,
  and custodial NFT tracking depending on plan.
- Matrica Connect provides OAuth 2.0 apps with scopes such as `profile`,
  `wallets`, `nfts`, `socials.discord`, `socials.twitter`,
  `socials.telegram`, and `email`.
- Matrica's OAuth user API exposes profile, wallets, email, Twitter, Discord,
  Telegram, roles, NFTs, tokens, and domains.
- Matrica has clear pricing in public docs: Premium at $99/month or $899/year,
  Pro at $199/month or $1,750/year, and Enterprise by sales call. Public docs
  show Matrica Connect and API access under Enterprise.

What this means:

- If a project wants Discord / Telegram gating, NFT holder roles, social
  account linkage, token snapshots, cNFT community checks, or quests, Matrica
  is likely a better near-term choice than SolID.
- If a project wants a simple OAuth login where the app can request wallets,
  NFTs, tokens, roles, socials, or email, Matrica is likely a better near-term
  choice than SolID.
- If a project wants a Solana program to verify "this wallet satisfies a
  private KYC / accreditation / residency / issuer-signed credential predicate"
  without receiving the user's raw profile, socials, email, wallet inventory,
  or credential data, Matrica's public docs do not show that as its main
  product surface.

The honest positioning is:

> Matrica verifies wallet-linked community facts and returns useful user data
> to apps. SolID should verify private credential predicates and reveal only
> the answer needed by the app.

That difference only matters for buyers who care about privacy, compliance
liability, on-chain enforcement, or credential reuse. For ordinary NFT Discord
roles, SolID should not try to beat Matrica.

### Competitor Matrix

| Product | Public category | Main user | Best public strength | SolID advantage if executed | When to choose competitor instead |
| --- | --- | --- | --- | --- | --- |
| Matrica | Community management, token verification, OAuth user data | NFT communities, games, apps, DAOs | Fast community gating, Discord / Telegram roles, token and NFT snapshots, OAuth scopes, multi-chain coverage | Private Solana credential proof verification with minimal disclosure and on-chain enforcement | Choose Matrica for Discord roles, holder verification, social linkage, quests, or token-gated community ops |
| Collab.Land | Token-gated communities | Communities using Discord / Telegram | Mature token-gating rules for balance and NFT attribute checks; automatic role removal when balances fail | SolID can prove non-token facts privately and feed Solana program logic | Choose Collab.Land for social-role gating based on token/NFT holdings |
| Guild.xyz | Community growth, quests, token gating | Large crypto communities | Broad integrations, quests, role ranking, gated forms, community growth tooling | SolID can target stricter privacy/compliance predicates instead of broad community engagement | Choose Guild for campaigns, quests, rewards, contributor scoring, and many app integrations |
| Lit Protocol | Access control and encryption | Developers gating content or resources | Solana RPC conditions, threshold encryption, token-gated content, programmable Lit Actions | SolID can provide issuer-signed credential proofs and revocation-aware Solana verification | Choose Lit for encrypted content, JWT access, or token/NFT/SOL balance conditions |
| zkMe | zkKYC, compliance, identity credentials | dApps needing compliance/KYC | Public Solana mainnet support, zkKYC, SBT identity credential minting, on-chain transactional KYC, claimed 80+ projects and 1.5M credentials | SolID can be more Solana-protocol-native if it makes verifier SDK/on-chain verification simpler for Solana programs | Choose zkMe when you need a mature KYC vendor, existing Solana zkKYC product, or enterprise compliance support now |
| Privado ID | SSI, W3C VCs, ZK proofs | Issuers, verifiers, wallet builders | Mature triangle-of-trust model, W3C VC/DID alignment, off-chain and on-chain verification, issuer/wallet/verifier tooling, revocation modes | SolID can avoid broad SSI complexity and focus on Solana app integration, Solana account roots, and a narrow eligibility API | Choose Privado ID when W3C VC/DID interoperability, multi-chain SSI, or mature issuer nodes matter more than Solana-native UX |
| Holonym / Human ID | Proof of uniqueness, KYC proof of personhood | Airdrops, grants, anti-sybil apps | Government ID, e-passport, phone, biometrics uniqueness, US residency, off-chain proof flow, API checks | SolID can support arbitrary issuer-defined credentials and Solana-native verifier program flows | Choose Holonym for human uniqueness, airdrop protection, or residency checks with existing Human ID flows |
| World ID | Proof of personhood | Apps requiring one-human-one-action | Large proof-of-humanity network, nullifier-based sybil resistance, mature EVM on-chain verifier docs | SolID can support non-humanity credential predicates, issuer-specific credentials, and Solana-native verification | Choose World ID for proof-of-humanity where World ID user coverage and EVM support are acceptable |
| Gitcoin / Human Passport | Sybil scoring, stamps, model API | Grants, rewards, governance, access programs | Low-friction API score for EVM addresses, stamp ecosystem, model-based sybil detection | SolID can provide cryptographic credential proofs instead of risk scores, and target Solana users natively | Choose Passport for EVM sybil scoring, grants, and low-friction API-based humanity checks |
| Quadrata | Compliance passport | DeFi / regulated apps | KYC, AML, KYB, wallet screening, country, age, accreditation; smart-contract-readable passport attributes on supported EVM chains | SolID can provide private Solana-native credential verification if it avoids exposing broad passport attributes | Choose Quadrata when EVM compliance passport workflows or AML/KYB vendor maturity matter most |
| Civic Pass / Gateway-style passes | Access / proof passes | Token issuers, apps, DAOs | Civic Pass historically offered tokenized identity passes, proof of personhood, and Solana integration examples; current Civic public docs are now more AI-agent/Auth focused | SolID can own the current Solana private credential proof niche if Civic's public identity docs remain less central | Choose Civic where Civic Pass / Gateway integrations already fit, or where Civic Auth/embedded wallet UX is the main need |
| Sismo | Historic ZK badges / Sismo Connect | Privacy app builders | Strong ZK selective disclosure history and developer-centric ZK app ideas | SolID can learn from Sismo's shift: developer UX and distribution matter as much as cryptography | Not a primary current competitor for Solana devnet unless Sismo Connect resurfaces as the relevant product |
| Traditional KYC providers | Web2 identity verification | Regulated businesses | Mature compliance operations, document verification, support, legal familiarity | SolID reduces verifier-side PII custody and enables reusable private on-chain proofs | Choose Web2 KYC when legal/regulatory comfort, fiat onboarding, or human review is more important than crypto-native privacy |

### Parameter-by-Parameter Comparison

| Parameter | Matrica | SolID target | Why it matters |
| --- | --- | --- | --- |
| Primary job | Community/user verification, token gating, OAuth user data | Private eligibility proof for Solana actions | Same word "verify," different buying reason |
| Best buyer | NFT/community/team ops, Discord/Telegram admins, apps needing wallet/social/token data | Solana app developers, DeFi/launchpads/DAOs needing private eligibility/compliance gates | The buyer determines what "simple" means |
| User data returned | Public docs show profile, wallets, NFTs, tokens, socials, email, roles, domains via OAuth scopes | Return a boolean/proof result and minimal public inputs | SolID should win only where data minimization is valuable |
| Privacy model | OAuth consent and platform-managed user/account data | Zero-knowledge proof of issuer-signed credential predicates | Strong privacy is SolID's main reason to exist |
| On-chain enforcement | Public docs focus on API/OAuth/community gating, not Solana program proof verification | Solana program verifies proof and nullifier/revocation state | Needed for composable DeFi/launchpad enforcement |
| Credential source | Wallet holdings, NFTs, tokens, roles, socials, profiles | Issuer-signed private credentials with revocation | SolID should support facts that are not visible on-chain |
| Revocation | Community roles can update based on holdings; OAuth/user data freshness depends on Matrica platform | Issuer/tree/credential revocation should be first-class | Compliance and issuer trust require explicit revocation semantics |
| Friction | Very strong for community gating; OAuth integration documented | Currently high; must be reduced to SDK/API wrappers | Matrica wins until SolID hides proof/account complexity |
| Chain scope | Solana plus Bitcoin, Ethereum, Polygon, Eclipse, Base, Monad, ApeChain | Solana-first | SolID should not compete on chain breadth at devnet |
| Compliance use cases | Not positioned primarily as private KYC/compliance proof infra in public docs | KYC/eligibility/accreditation/residency predicates are the wedge | This is the product gap SolID can occupy |
| Enterprise readiness | Public pricing and Enterprise support exist | Not ready until audits, docs, hosted infra, and issuer tooling mature | SolID should not overclaim maturity |
| Open protocol surface | Public docs show APIs/OAuth; internal verification model not protocol-native from app perspective | Protocol should be auditable, program-verifiable, and SDK wrapped | This is relevant to Solana-native teams |

### Why Someone Would Use SolID Instead of Matrica

Only if at least one of these is true:

1. The verifier must not receive user PII, social identity, email, wallet
   inventory, raw credential fields, or broad profile data.
2. The verifier needs a Solana program to enforce the result, not just a
   backend, Discord bot, Telegram gate, OAuth callback, or allowlist.
3. The fact being verified is not simply "owns token/NFT/domain" or "has a
   social-linked profile." It is an issuer-signed claim such as KYC status,
   residency, age range, accreditation, membership, credential level, or
   compliance eligibility.
4. The app wants one credential to be reusable across multiple Solana apps
   without each app integrating directly with a KYC vendor or storing user data.
5. Revocation must be cryptographically tied to proof validity, not handled as
   an off-chain role-sync convention.
6. The team wants a protocol surface it can audit and compose with on Solana.

If none of those are true, Matrica is probably the better product for that
customer today.

### Why Someone Would Use Matrica Instead of SolID

Matrica is likely better when the task is:

- "Give holders the right Discord role."
- "Gate Telegram or community channels."
- "Check if a wallet owns an NFT, cNFT, token, domain, or staked asset."
- "Run community snapshots."
- "Use OAuth to fetch wallet, NFT, token, social, email, or role data."
- "Launch quickly with an existing hosted community product."

SolID should not fight this battle directly. It should integrate with products
like Matrica later, for example by letting Matrica-like dashboards consume a
SolID verification result as one more gating rule.

### Closest Direct Threats

The biggest threats are not Matrica. The biggest direct threats are:

1. zkMe, because it publicly claims Solana mainnet support for privacy-preserving
   identity verification, zkKYC, identity SBT minting, and on-chain
   transactional KYC.
2. Privado ID, because it has a mature issuer-holder-verifier model, W3C VC/DID
   alignment, SDKs, wallets, issuer nodes, and revocation modes.
3. Holonym / Human ID, because it has focused proof-of-uniqueness flows, API
   checks, and anti-sybil positioning.
4. World ID and Gitcoin / Human Passport, because they can solve "is this a
   unique human / likely human?" with far more distribution than a new protocol.

SolID's answer should not be "we are better at identity." That is too broad and
not proven. The answer should be:

> We are the Solana-native private eligibility verifier for apps that need
> program-enforced, revocation-aware, issuer-signed credential checks.

### Open Competitive Risks

- If zkMe's Solana integration is already easy for developers, SolID must win
  on narrower Solana-native composability, not on "we also do zkKYC."
- If Privado ID expands first-class Solana support, SolID loses any "Solana
  native" story unless its integration is dramatically simpler.
- If Matrica adds private credential proofs as an Enterprise feature, SolID's
  wedge narrows to open protocol/auditability/on-chain enforcement.
- If a traditional KYC vendor ships a Solana SDK with allowlist attestations,
  many teams may choose that because it is easier to explain to lawyers.
- If SolID does not ship a clean holder UX, the privacy benefit will not be
  visible to users.
- If SolID does not ship a hosted verifier/proof-path service, developers may
  prefer centralized APIs despite weaker privacy.

### Competitive Product Recommendation

The devnet product should be framed as a verifier-first infrastructure product,
not a community management platform.

Use this landing-page contrast:

> Matrica answers: "Does this user own the right assets or have the right
> linked account?"
>
> SolID answers: "Can this wallet privately prove an issuer-signed eligibility
> claim to a Solana program?"

The first public demo should not be Discord gating. Matrica, Collab.Land, and
Guild are already strong there. The first demo should be a private launchpad or
DeFi eligibility gate:

- user has KYC/accreditation/residency credential,
- launchpad never sees the underlying data,
- proof verifies on Solana,
- nullifier prevents replay,
- issuer revocation breaks future proofs,
- app integrates through one SDK call.

That demo creates a reason to care that is not already solved well by Matrica.

## Superteam Idea Fit and Moats

The Superteam Build idea list includes "Private Onchain Identity" under
Infrastructure, framed as:

> Build a protocol to reveal specific info required for app use, or mask
> sensitive information completely using ZK tech.

The listed problem areas include healthcare, hospitality, supply chain, and
other industries where verifiable identification plus on-chain privacy matters.
The listed resources include Light Protocol and Privado ID.

This is strong ecosystem validation, but it is not product-market validation by
itself. The idea is broad. To win, SolID must convert it into a concrete wedge:

> Private Solana eligibility verification for apps that need issuer-signed,
> revocation-aware credential proofs without storing user PII.

### What SolID Already Has

These are real protocol-level features in this codebase or current plan. Some
still need product polish before they become user-facing advantages.

| Feature | SolID status | Why it matters |
| --- | --- | --- |
| Solana-native programs | Three Anchor programs for verification, issuer registry, and schema registry | Verification can become part of Solana app logic instead of only an off-chain API check |
| Groth16 proof verification on Solana | On-chain verifier uses alt_bn128 syscalls | Enables private proofs to be checked by programs |
| Private credential predicates | Circuit architecture supports proving facts without revealing raw credential data | This is the core privacy promise |
| Issuer registry | On-chain issuer governance and issuer status | Verifiers can reason about who issued a credential |
| Schema registry | On-chain schema roots and schema governance | Verifiers can pin what kind of credential is being proven |
| Compressed credential/tree roots | SPL Account Compression is used as part of the credential trust surface | Makes Solana-scale credential state more plausible |
| Revocation-aware trust roots | Issuer tree root and nullifier design account for revocation | Revocation is central for compliance and issuer trust |
| Replay resistance | Nullifier flow prevents proof reuse in the same action domain | Required for launchpads, mints, voting, and gated actions |
| Artifact pinning direction | Proof artifacts are moving toward SHA-256-pinned integrity checks | Reduces off-chain prover supply-chain risk |
| Devnet-first launch plan | Plan focuses on a concrete private launchpad eligibility demo | Avoids trying to be "identity for everything" too early |

### Features Competitors Have That SolID Does Not Yet Have

These gaps are important. They are not reasons to stop; they are reasons to
focus.

| Capability | Who has it publicly | SolID gap |
| --- | --- | --- |
| Mature community gating UI | Matrica, Collab.Land, Guild.xyz | SolID has no comparable community dashboard and should not build this first |
| Existing user distribution | Matrica, World ID, Gitcoin / Human Passport, zkMe, Holonym | SolID has no holder network yet |
| Enterprise KYC operations | zkMe, Quadrata, Web2 KYC providers, likely Synaps/Fractal-style vendors | SolID is infrastructure, not a full compliance vendor |
| Polished OAuth flow | Matrica | SolID has no consumer-grade "connect and consent" UX yet |
| Broad chain support | Matrica, Privado ID, zkMe, Quadrata, World ID / Passport on EVM ecosystems | SolID is intentionally Solana-first |
| W3C VC / DID ecosystem | Privado ID | SolID does not currently win on identity standards interoperability |
| Mobile identity wallet | Privado ID, World ID, zkMe/Holonym-style products | SolID needs holder UX before public users understand it |
| Simple hosted API | Matrica, Passport, Holonym, Web2 KYC vendors | SolID currently exposes too much protocol complexity |
| Known brand trust | Matrica, Civic, World ID, Gitcoin, zkMe, Privado ID | SolID must earn trust through audits, docs, demos, and partners |

### Features SolID Can Have That Competitors Usually Do Not Combine

The moat is not one feature. It is the combination.

| Moat candidate | Why it can matter | Who overlaps |
| --- | --- | --- |
| Solana-native private verifier program | A Solana app can enforce eligibility without trusting only a backend API | zkMe may overlap if its Solana transactional KYC is easy and open enough |
| Revocation-aware issuer tree plus credential roots | Proof validity can depend on current issuer/credential state, not static allowlists | Privado ID has revocation concepts; SolID's edge must be Solana-native implementation |
| Minimal disclosure by default | Verifier gets "eligible / not eligible" and required public inputs, not wallet inventory or profile data | ZK identity competitors overlap; Matrica/community tools generally do not |
| Narrow developer API for Solana apps | `verify({ requirement, wallet, action })` can hide circuits, roots, nullifiers, accounts, artifacts | Most identity systems lose developers at integration complexity |
| Solana account-compression fit | Credential/revocation state can be designed for Solana scale and cost patterns | Light Protocol and compression-aware teams may overlap |
| Open auditable protocol surface | Programs/circuits can be audited and reasoned about by Solana teams | Some competitors are more hosted/API-first |
| Issuer-governed trust network | Apps can choose trusted issuers/schemas instead of trusting one opaque platform decision | Privado ID overlaps conceptually; SolID must make it simpler for Solana |

### Strongest Moats If Executed

1. Integration moat

The biggest practical moat is not cryptography alone. It is becoming the
default way a Solana app adds private eligibility checks.

Required proof:

- `examples/private-launchpad-gate` works end-to-end.
- Verifier SDK hides account ordering, proof buffers, Merkle paths, roots,
  artifact loading, and nullifiers.
- A developer can integrate in under 15 minutes.

2. Trust-network moat

If credible issuers register and issue reusable credentials, every new verifier
gets more value from joining the same network.

Required proof:

- At least 2-3 credible devnet issuers.
- Clear schema registry.
- Clear issuer status and revocation semantics.
- Public issuer metadata and health page.

3. Security/audit moat

Private identity infrastructure only works if users and apps trust the protocol.
The repo already treats security as central, but the public version needs the
same clarity.

Required proof:

- SEC-048 and all devnet-blocking issues closed without bypasses.
- Trusted setup story documented honestly.
- Artifact pinning complete.
- Security assumptions and known limitations are public.

4. Solana-native composability moat

If verification can be composed inside Solana app flows, SolID has a stronger
reason to exist than a hosted KYC API.

Required proof:

- On-chain verifier call is stable.
- CU budget is measured and documented.
- Example dApp demonstrates enforcement, replay rejection, and revocation
  failure.

5. Privacy UX moat

Users do not care that a proof is zero knowledge if they cannot understand what
is being revealed.

Required proof:

- Holder UI says exactly what is proven.
- Holder UI says what is not shared.
- Failure states are understandable.
- No raw secrets leak in logs or debug modes.

### What Competitors Have That We Should Not Copy First

Do not copy these before the core wedge works:

- Matrica-style community ops dashboard.
- Guild-style quest engine.
- Collab.Land-style broad social-role bot.
- World ID-style global proof-of-humanity network.
- Full Privado ID-style SSI platform.
- Full enterprise KYC vendor workflow.

Those are large product surfaces. Copying them would dilute the devnet launch.
The correct first product is narrower:

> A Solana app privately verifies eligibility in one flow, with proof,
> nullifier, revocation, issuer trust, and schema trust handled for it.

### Grant/Hackathon Framing

If this is positioned for the Superteam idea, the application should not say
"we are building private onchain identity" as the whole pitch. That repeats the
idea title but does not prove execution.

Use this framing:

> SolID implements the Private Onchain Identity idea as a Solana-native private
> eligibility verifier. Instead of exposing a user's identity, wallet inventory,
> or KYC fields, the user proves only the requirement needed by a Solana app.
> The verifier can enforce the result on-chain, while issuer and schema
> registries provide revocation-aware trust roots.

Best proof-of-work for a grant:

- Live devnet private launchpad eligibility gate.
- One command to initialize devnet.
- One command to issue a demo credential.
- One SDK call to request/verify proof.
- Public docs explaining exactly what is private, what is public, and what is
  still not production-ready.

### Sources Consulted

- Matrica docs: `https://docs.matrica.io/`
- Matrica API getting started: `https://docs.matrica.io/api-reference`
- Matrica Connect app setup: `https://docs.matrica.io/matrica-connect/create-your-application`
- Matrica OAuth2 user API: `https://docs.matrica.io/matrica-connect/oauth2-api-reference-v2/oauth2userv2`
- Matrica pricing guide: `https://docs.matrica.io/guides/pricing-guide`
- Matrica wallet address gating: `https://docs.matrica.io/guides/community-guide/verification/wallet-address-gating`
- Collab.Land token gating docs: `https://docs.collab.land/help-docs/key-features/token-gate-communities`
- Guild website: `https://guild.xyz/`
- zkMe zkKYC docs: `https://docs.zk.me/hub/what/zkkyc`
- zkMe Solana launch post: `https://blog.zk.me/zkme-expands-privacy-preserving-identity-verification-to-solana-blockchain/`
- Privado ID introduction: `https://docs.privado.id/docs/introduction`
- Privado ID issuer configuration and revocation: `https://docs.privado.id/docs/issuer/issuer-configuration`
- Human Passport / Gitcoin Models API: `https://docs.passport.xyz/building-with-passport/models`
- World ID on-chain verification: `https://docs.world.org/world-id/idkit/onchain-verification`
- Holonym / Human ID KYC proof-of-personhood flow: `https://docs.holonym.id/architecture/flow-of-data-kyc-proof-of-personhood`
- Holonym off-chain proofs: `https://docs.holonym.id/for-developers/off-chain-proofs`
- Holonym API reference: `https://zeronym-docs.holonym.id/for-developers/api-reference`

## Monetization Paths

Do not lead with monetization before usage. But design surfaces that can become
paid later.

Possible models:

- Hosted verifier API subscription.
- Per-verification fee for production apps.
- Issuer dashboard SaaS.
- Enterprise support for regulated issuers.
- Hosted indexer / proof-path API.
- Compliance reporting and audit logs.
- Premium schemas and verified issuer onboarding.

Near-term metric:

- Not revenue first.
- First measure:
  - verifier integrations,
  - issued credentials,
  - successful proofs,
  - time to integrate,
  - proof success rate,
  - repeat verifier usage.

## Immediate Next 10 Tasks

1. Fix subgroup verifier authority initialization.
2. Sync security registry and CU docs to actual SEC-048 closure state.
3. Refresh CU baselines after full e2e.
4. Add subgroup wasm/zkey sidecar hash generation.
5. Write `docs/DEVNET_QUICKSTART.md`.
6. Build `examples/private-launchpad-gate`.
7. Create high-level verifier SDK wrapper.
8. Create issuer credential package format.
9. Create holder claim/prove MVP.
10. Record the 5-minute verifier integration video.

## Non-Goals for Devnet Launch

- Full mainnet trust ceremony.
- Perfect issuer marketplace.
- Mobile wallet plugin.
- Enterprise compliance dashboard.
- Broad identity standard negotiation.
- General-purpose identity for every use case.

Devnet should prove one thing extremely well:

> A Solana app can privately verify user eligibility without handling PII.

## Final Product Principle

Every surface should answer one question for its user:

- Verifier: "Can this wallet do this action?"
- Issuer: "Can I issue and revoke this credential?"
- Holder: "What am I proving, and what stays private?"
- Operator: "Is the trust surface healthy?"

If a feature does not improve one of those answers, defer it.
