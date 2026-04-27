# Go-to-Market Strategy -- solid-protocol

> Plain-ASCII strategy doc. Last refreshed: 2026-04-27.
> Start date for the sequenced plan in Section 6 is 2026-05-15.
> Companion to `docs/FORWARD_ROADMAP.md` (technical phase plan) and
> `docs/REFERENCE_VERIFIER_APP.md` (what real integration requires).
>
> This doc answers the questions the technical roadmap deliberately
> does not:
>   1. Why would anyone use this over the alternatives?
>   2. What is missing, beyond shipping E2E, to convert this from a
>      repo into an adopted product?
>   3. What is the realistic 6-month sequence to find that out?
>
> Source of truth principle still applies: this is strategy, not
> code. If a claim here disagrees with reality on the ground (a
> founder conversation, an integrator commitment, a regulator
> opinion), reality wins and this doc is updated.

---

<!-- 
  these skills can be used to improve the visibility of the product
  Skills I will use: anything frontend, marketing, design, product-review, deck/grant, brand.
 -->

## 0. The honest framing

solid-protocol is structurally complete and architecturally sound.
Shipping E2E, mainnet-deploying, closing the four open mainnet
blockers (SEC-012 ceremony, SEC-007/048 BJJ subgroup fix, SEC-045
atomic-handler binding root, SEC-043 multisig) is necessary but
not sufficient. The gap between "infrastructure works" and
"infrastructure is used" is a product/market problem, not a
technical one.

The Solana Foundation has explicitly listed "Private Onchain
Identity" as an Infrastructure idea on Superteam Build (see
Section 1). That listing changes some assumptions and not others.
This doc reconciles both.

---

## 1. The Solana Foundation Superteam Build listing -- what it
   actually means

Listed at `https://build.superteam.fun/ideas/private-onchain-identity`
(Infrastructure track), under the Solana Foundation org, with the
following framing:

  Problem:   "There are dozens of use-cases for verifiable
              identification, paired with onchain privacy,
              especially in industries like healthcare, hospitality,
              supply chain, etc."
  Solution:  "Build a protocol to reveal specific info required for
              app use, or mask sensitive information completely
              using ZK tech."
  Resources: "Light Protocol: ZK on Solana" + "Example: Privado ID"

This is a meaningful signal. Reading it carefully:

### What the signal tells me

  - The Solana Foundation thinks this category is worth filling.
    They have an existing identity bet (SAS, May 2025 -- attestation-
    based, deliberately non-ZK). They are NOT saying SAS solves
    this; they are saying ZK selective disclosure is a separate gap.
  - The Foundation explicitly endorses the Privado-style framing.
    "Example: Privado ID" is exactly the architectural family
    solid-protocol implements: BabyJubJub / Poseidon / Groth16 /
    selective disclosure. solid-protocol is not contrarian; it is
    aligned with the Foundation's stated direction.
  - The Foundation explicitly endorses Light Protocol-style
    compressed state. solid-protocol uses SPL Account Compression
    (a Foundation-maintained sibling) which is a directly equivalent
    primitive; the architecture survives this framing intact.
  - The Foundation's grant programs are equity-free. That's small
    money but real validation, and Foundation amplification at
    launch is a non-trivial distribution lever.

### What the signal does NOT tell me

  - It does not tell me a paying customer exists. Foundation idea
    boards are wishlists. They identify gaps in ecosystem
    coverage. They are populated based on what the Foundation
    thinks is missing, not based on integration commitments from
    Drift / Jupiter / Kamino / Ondo.
  - It does not tell me adoption will follow shipping. EVM has the
    same category endorsed by every major foundation since 2022.
    Privado has 4M issued credentials and under USD 5M revenue.
    Sismo had grants and lost narrative momentum. Foundation
    endorsement is necessary, not sufficient.
  - It does not tell me ZK beats SAS for any specific use case. The
    Foundation is asking for both to exist. They are not saying
    integrators will pick ZK over attestations.
  - It does not constitute a commitment to fund, promote, or
    integrate solid-protocol specifically. Other builders may also
    target this idea.

### Net adjustment to my prior analysis

  - "First mover" claim: stronger. solid-protocol is plausibly the
    most architecturally complete attempt at the Privado-on-Solana
    framing the Foundation explicitly named.
  - "Demand signal" claim: stronger at the ecosystem level
    (Foundation), still weak at the dApp level (no DeFi protocol
    has publicly committed). Both are true at the same time.
  - "Path to product" claim: unchanged. Customer-first is still
    the discipline. Foundation interest accelerates fundraising
    and amplification, not customer acquisition.

The right way to use this signal: treat it as a credible
recruiting argument when approaching grant programs, hackathons
(Colosseum Frontier, ongoing), and angel investors. Treat it as
zero evidence when approaching dApp integrators. Their decision
turns on their own product needs, not on Foundation framing.

---

## 2. The competitive map (verified, 2026-04-26 market scan)

  Direct ZK-credential systems on Solana:
    - SAS (Solana Foundation, May 2025). Attestation registry.
      Deliberately NOT ZK. Free, blessed, live. Used by Civic,
      Solana.ID, Trusta, Wecan, Polyflow.
    - Civic Pass. 2M users, USD 500M+ TVL secured. Hosted KYC pass.
      Pseudo-ZK only.
    - Solana.ID. Reputation aggregator + SAS. Not Privado-style.
    - VeryAI. USD 10M Polychain seed (Mar 2026). ZK + biometric
      palm-scan PoP on Solana. Adjacent, not competitive on
      credentials.
    - zkid.digital. Surfaced in search; warrants direct
      investigation.
    - Privado / iden3. EVM-only. No official Solana port.

  EVM-side reality (relevant because it bounds what's possible):
    - Privado ID -- 4M credentials, < USD 5M revenue.
    - World ID -- 18M users, but won by abandoning the credential
      framing for AI-era proof-of-personhood (Tinder, Zoom,
      Shopify integrations Apr 2026).
    - Holonym / Human.tech -- 2.1M users, mostly airdrop sybil
      resistance.
    - Galxe Passport -- 1M+ holders, compliance-flavored KYC reuse,
      not ZK selective disclosure.
    - Sismo -- still operating, lost narrative momentum after 2023.

  EVM RWA winner (the strongest commercial wedge):
    - ERC-3643 -- non-ZK portable identity contracts. Winning
      institutional integrations because regulators and lawyers
      understand verifiable credentials with audit trails better
      than Groth16 proofs. This is the bar solid-protocol must
      beat for the strongest wedge.

Pattern: years of work, low revenue, real users only show up when
sybil resistance for airdrops or Worldcoin-scale PoP becomes the
wedge. Generic identity loses to verticalised plays.

---

## 3. Why anyone would (or would not) use solid-protocol

### 3.1 Where solid-protocol genuinely beats SAS

Concretely, name the use case where leaking the field that SAS
exposes loses unacceptable money or users:

  - Accredited investor gating where the regulator distinguishes
    "proved accredited" from "proved age 18+". SAS attestations
    leak which fact is asserted; ZK selective disclosure can hide
    everything except the boolean.
  - Healthcare credentials where HIPAA classifies the existence of
    the credential as PHI. SAS PDA presence is observable on-chain;
    a ZK proof reveals only the predicate.
  - Cross-jurisdiction tier gating where the holder presents
    different credential subsets to different verifiers without
    correlation. SAS PDA addresses are linkable to wallets;
    SolID's per-schema BJJ subkeys (ADR-0005) are not.

These are real wins, but they're niche wins. They matter for one
category at a time. They do not collectively justify "every dApp
plugs in."

### 3.2 Where solid-protocol probably loses to alternatives

  - Vs. SAS for general KYC-once-access-everywhere. SAS is
    sufficient, free, Foundation-blessed, integrated in 2 weeks.
    SolID integration is 3-6 engineering months per
    `docs/REFERENCE_VERIFIER_APP.md:170-173`. The 10x bar for
    integrators is real.
  - Vs. ERC-3643 for institutional RWA. Lawyers prefer audit-
    trailable verifiable credentials over Groth16 proofs they
    cannot read. solid-protocol has to win on jurisdictional fit
    + regulator legibility, not just cryptographic strength.
  - Vs. centralized KYC vendors (Sumsub, Persona) for fiat ramps.
    Already integrated by Jupiter Card, etc. Switching cost beats
    cryptographic upgrade.
  - Vs. Worldcoin-style PoP for AI defense. Different category;
    World ID has the wedge.

### 3.3 The honest answer

In a default world: most teams use SAS or centralized KYC. Some
teams that need the specific privacy properties in 3.1 -- and are
willing to absorb 3-6 months of integration cost, and have
regulator coverage -- choose solid-protocol. The market is real,
the market is small, and the bar for any single integrator is
high.

The Foundation listing raises the ceiling on grant funding,
distribution, and category legitimacy. It does not raise the
ceiling on customer demand for any specific dApp.

---

## 4. What's missing, beyond shipping E2E

`docs/REFERENCE_VERIFIER_APP.md:108-173` is unusually honest about
this. Section 5 of that doc lists the 3-6 engineering months per
integrating protocol that lives outside this repo. The five
buckets, ranked by how far they sit from the codebase:

### 4.1 One named integration partner with a written commitment

The single highest-leverage missing piece. A logo on a landing
page does not count. A protocol team that has said in writing:
"if SolID delivers X by date Y, we will integrate it for use case
Z" is the only thing that converts protocol-with-grant-funding
into protocol-with-customers.

### 4.2 An issuer producing real credentials at real volume

Two viable paths:
  1. KYC-as-a-service vendor (Sumsub, Persona, Civic) issues
     SolID-format credentials. They already do KYC on Solana;
     adding a SolID issuance flow is incremental for them.
  2. Regulated entity issues for its own users (broker-dealer
     issues "accredited" to its already-vetted base).

Without (1) or (2), the issuer side of the stack is theatre. The
DAO governance code in `programs/issuer-registry/src/lib.rs` is
irrelevant until the DAO is governing real issuers.

### 4.3 Compliance / regulatory positioning

Per `docs/REFERENCE_VERIFIER_APP.md:120-130` -- per-jurisdiction
issuer sets, FinCEN / OFAC / eIDAS opinions, audit trails that
survive subpoena. None of this exists in the repo. None of it is
engineering. All of it is mandatory for the strongest wedge.

A compliance lawyer (or compliance-as-a-service partnership) is
required before the second integrator conversation, not after.

### 4.4 SDK + ergonomics that beat SAS by 10x

If integration takes 3-6 months and SAS takes 2 weeks, SolID
needs to be 10x better for those 3 months to be worth it.
Concretely:

  - Drop-in CPI helpers indistinguishable from regular Anchor.
  - A `solid-cli` for non-crypto issuer staff.
  - A hosted indexer (Helius DAS or Shyft adapter) -- the ARCH-3
    P2 in `docs/FORWARD_ROADMAP.md:118-121`. This is actually P0
    for adoption; node-free verifier flow is non-negotiable for
    integrators.
  - Mobile prover under 5 seconds. WASM prover times on a phone
    are 10-20s today (per `docs/REFERENCE_VERIFIER_APP.md:215-218`).
    For consumer use cases this is fatal. ARCH-2 delegated
    proving with TEE attestation is the option, but it has its
    own privacy disclosure trade-off.

### 4.5 Distribution + narrative

  - Pick a wedge (Section 5).
  - Write the commercial whitepaper for that wedge -- not the
    technical one (already exists in
    `docs/private_onchain_identity_deep_dive.md`).
  - Launch with one integrator.
  - Get one regulator quote.
  - Get one VC partner who funds regulated DeFi on Solana to
    publicly say SolID is the rail.
  - Foundation amplification once the integration is live.

None of this is in the codebase. None of it can be deferred to
"after we ship."

---

## 5. Wedges, ranked by realism

  Wedge                                                  Realism

  RWA / accredited-investor gating on Solana             High
    Why: Ondo on Solana, MiCA pressure, no Solana
    equivalent of ERC-3643 yet. Strongest commercial
    wedge. Market exists.

  Sybil resistance for Solana airdrops                   Medium
    Why: LayerZero, Galxe, Human Passport prove demand,
    but incumbents are entrenched and chain-agnostic.
    Possibly winnable with one anchor-protocol commit.

  Healthcare-on-Solana / supply-chain-on-Solana          Low
    Why: matches Foundation problem statement and the
    deep-dive doc, but these industries pick consortia
    not chains, and Solana has no presence yet. Years
    of evangelism with no guaranteed payoff.

  Age-gating prediction markets / gambling-adjacent      Med-Low
    Why: real legal need; integrators are scrappy and
    will IP-block before they integrate ZK.

  Generic "identity layer for every dApp"                ~Zero
    Why: Privado tried it. Sismo tried it. Both stalled.
    Market punishes generality in this category.

Bet asymmetrically on the High row. Treat the Medium row as a
secondary if a major Solana airdrop team commits. The Foundation's
problem statement points at healthcare/hospitality/supply-chain;
that framing is the wishlist, not the wedge. Pick what closes
first, not what reads best.

---

## 6. The 6-month sequence (start: 2026-05-15)

In order. Each step validates whether the next is worth doing.

### Phase A -- 2026-05-15 to 2026-05-28 (2 weeks): Discovery

  - 10 founder conversations. Solana RWA / compliance focus first.
  - Concrete targets:
      Ondo on Solana (tokenized stocks, accredited gating)
      Maple Finance (institutional pools)
      Drift institutional product line
      Kamino restricted-jurisdiction features
      Phoenix and any compliance-aware fork attempts
      Two teams in Colosseum Frontier 2026 building RWA / KYC
      One Solana-native KYC-as-a-service vendor (potential issuer)
  - The conversation is NOT "want to integrate?". It is:
      "What would have to be true for you to gate this specific
       feature with private credentials instead of IP-blocking
       or off-chain KYC?"
  - Goal: at least one written intent-to-integrate from a named
    team, conditional on a specific deliverable by a specific
    date.
  - Decision gate (2026-05-28):
      Yes -> Phase B.
      No (zero teams said yes after 10 conversations) -> wedge
      is wrong or the category is dead on Solana right now.
      Pivot or stop. See Section 7.

### Phase B -- 2026-05-29 to 2026-06-11 (2 weeks): Lock the pair

  - Pick one integrator (from Phase A) plus one issuer.
  - Issuer options: KYC-as-a-service vendor signing up to issue
    SolID credentials for that integrator's specific need, OR
    a regulated entity issuing for its own user base.
  - If both committed: Phase C.
  - If only one committed: extend Phase A by 2 weeks to find the
    other; do not start Phase C with half a market.

### Phase C -- 2026-06-12 to 2026-08-06 (8 weeks): Ship the
   integration, not the protocol

  - What ships is THEIR gate, not YOUR protocol.
  - Repo gains the bare minimum to make their integration real:
      Mobile prover under 5s (delegated proving acceptable if
        privacy disclosure documented)
      Helius DAS / Shyft indexer adapter
      `solid-cli` for issuer staff
      Compliance opinion for the integrator's jurisdiction
      Mainnet blockers closed (SEC-012, SEC-007/048, SEC-043,
        SEC-045) -- their lawyer will ask
  - Everything else from `docs/FORWARD_ROADMAP.md` is P3 during
    this phase. The repo serves the integration; the integration
    does not wait on the repo.

### Phase D -- 2026-08-07 to 2026-08-20 (2 weeks): Launch

  - Joint announcement with the integrator.
  - One regulator quote.
  - One VC partner funding regulated DeFi on Solana publicly
    naming SolID as the rail.
  - Foundation post if achievable (the Superteam Build idea
    listing is the recruiting hook).

### Phase E -- 2026-08-21 to 2026-11-14 (3 months): Replicate

  - Second integrator. If unable to land within 90 days of the
    first launching, the model is not replicable. That is the
    decision point in Section 7.

### Total

2026-05-15 -> 2026-11-14. Six months. End state: either two live
integrators on mainnet or a clear honest read on which Section-7
branch to take.

---

## 7. The decision tree if the path doesn't open

Not failure modes. Honest branches. Each is rational given a
specific pattern of evidence.

### 7.1 Branch A: Two integrators land on schedule

  Continue as a protocol play. Raise a seed round on the back of
  two live integrations + Foundation alignment + audit close-out.
  Phase 6 of the technical roadmap (ecosystem + network effects)
  becomes plausible.

### 7.2 Branch B: One integrator lands, second won't commit

  Narrow to a vertical product. Stop pitching "ZK identity
  protocol"; start pitching "the accredited-investor gating
  product for Solana RWA" (or whichever wedge converted). Same
  codebase, completely different go-to-market. Sell turnkey
  integrations to 5-10 RWA protocols. This is a services-flavored
  product business, not a protocol business. Smaller TAM, much
  faster cash flow, much higher chance of mattering.

### 7.3 Branch C: Zero integrators after Phase A

  Two sub-options:

  C.1  Partner with SAS rather than compete. SolID's ZK layer
       sits on top of SAS attestations as the optional privacy
       upgrade. Stop being a competitor; become a feature. Smaller
       market, faster adoption, alignment with Foundation's
       existing identity bet.
  C.2  Treat as a research artifact. Publish, get hired by a
       major protocol or fund as the ZK-identity expert, take
       the architecture to a place with distribution. This is
       the most common honest outcome for category-defining
       infrastructure with no demand-side pull. It is not failure.

The Foundation listing is consistent with all three branches. It
raises the floor (Branch C still gets you a grant and credibility);
it does not raise the ceiling (Branch A still requires real
customers).

---

## 8. What this doc does NOT decide

  - Which integrator to call first. Pick after Phase A, based on
    which conversation is warmest.
  - Whether to take a Foundation grant. Take it if offered;
    equity-free is upside.
  - Whether to prioritize SEC-012 ceremony before customer
    discovery. Don't. The mainnet blockers are needed by the
    integrator's lawyer at close, not by the integrator's PM at
    Phase A discovery. Sequence them into Phase C.
  - The legal entity, fundraising structure, or hiring plan.
    Those are downstream of Phase A outcomes.
  - Pricing. Infrastructure pricing models on Solana are not
    settled enough for a decision now. Default to free-during-
    integration, revisit at Phase D.

---

## 9. The single sentence

The protocol is sound; the product is missing; the Foundation has
endorsed the category but not the customer; and the only thing
that converts solid-protocol from a repo into a product is one
named integrator in writing within 30 days of 2026-05-15. Every
other decision -- protocol vs vertical vs research, grants vs
seed, SAS-partner vs compete -- answers itself once that
question is answered.

---

End of go-to-market doc. Update this file when Phase A closes,
Phase B closes, or any of Section 7's branches becomes the active
path. Do not duplicate the technical phase plan; that lives in
`docs/FORWARD_ROADMAP.md`.
