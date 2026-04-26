# Reference Verifier App

> Design doc for the Day 8-10 reference verifier app described in
> sec/audits/2026-04-25_v0.6.1_deep_comprehensive_audit.md, Section 4.
> Last refreshed: 2026-04-25 for v0.6.1.

This document explains what the reference verifier app is, what it
deliberately is not, and what a production-grade integration would
need on top of it. It exists so that "fake 21+ pool" is not a load-
bearing phrase whose meaning lives only in audit slack.

## 1. Purpose

The reference app is a single artifact that demonstrates the SolID
verification path end to end against a real Solana cluster:

  Holder browser  ->  WASM prover  ->  serialized proof bytes
        |                                       |
        v                                       v
  Verifier dApp UI  --(Anchor ix)-->  Reference Anchor program
                                              |
                                              | CPI
                                              v
                                  zk-verifier::verify_batch_proof

The reference app is the only deliverable in the Phase 4 ship plan
that exercises the full pipeline (browser -> WASM -> on-chain CPI ->
SolID verifier -> success / nullifier write) on devnet. It closes the
loop between "the SDK passes its unit tests" and "a third party can
hold a phone and prove a credential".

## 2. Scope -- what the reference app IS

The reference app contains exactly four pieces:

1. A **Next.js 14 app** (`examples/reference-verifier-app/`):
   - Wallet connect (`@solana/wallet-adapter`).
   - "Request credential" panel that calls a stubbed issuer (the dev
     issuer bootstrapped via `scripts/bootstrap_issuer.ts`).
   - "Generate proof" panel that runs the WASM prover in-browser.
   - "Submit" button that builds an ix targeting the reference
     program below and signs it with the connected wallet.

2. A **reference Anchor program** (`programs/reference-pool/`):
   - One state account: `PoolMembership { holder: Pubkey,
     joined_at: i64, schema_hash: [u8;32] }`.
   - One instruction, `join_pool(query, proof_data)`, that:
     a. Calls `zk_verifier::cpi::verify_batch_proof` with the
        verifier query and proof.
     b. On success, init's a `PoolMembership` PDA.
     c. Emits an event.
   - That is the entire program. No tokens, no math, no oracle.

3. A **deployment + bootstrap script**
   (`scripts/deploy_reference_app.ts`):
   - Deploys the reference program to devnet.
   - Initializes the verifier query template (schema + age >= 21 +
     approved-issuer-set predicate).
   - Prints a URL the user can hit to try the demo.

4. A **README** (`examples/reference-verifier-app/README.md`)
   pointing at this design doc and explaining how to run the demo
   locally + on devnet.

## 3. Non-scope -- what the reference app is NOT

This is deliberately a thin demonstrator. The following are out of
scope for v1 and will not be built in this artifact:

  - No SPL token vault. The pool does not hold value.
  - No share accounting / withdrawal logic.
  - No oracle, no price feed, no liquidation.
  - No DAO over the pool authority.
  - No jurisdiction-specific issuer sets.
  - No compliance opinion. The demo does not claim to satisfy any
    regulator anywhere.
  - No mobile wallet beyond what wallet-adapter already supports.
  - No production indexer beyond the audit's Day 7 watcher.

If a reader needs any of those, they need a real integrator (see
Section 5), not a fork of this app.

## 4. Why a separate program rather than a verifier-only flow

The Day 6 ship plan (audit Section 4, Day 8-10 item 1) explicitly
wants the demo to call `verify_batch_proof` over CPI -- not as a
top-level instruction. The reason:

  - CPI is the only execution model that integrators (Jupiter,
    Drift, MarginFi) will ever use. Top-level direct invocation
    would test a path no real consumer takes.
  - The "real" failure modes -- compute-budget overflow when verify
    is wrapped in a parent ix, account-list ordering, signer
    propagation, error-bubble semantics -- only show up under CPI.
  - The reference program is the smallest possible CPI consumer; if
    SolID can't satisfy this, it can't satisfy anything richer.

The audit's NEW-02 (CU-budget regression gate, see registry
SOLID-SEC-046) becomes a hard signal here: if the CPI-wrapped
verify_batch_proof exceeds 250k CU plus 10%, the reference app
fails to land its tx and the regression is caught immediately.

## 5. From reference app to a real integration

A "real 21+ pool" -- meaning a deployed product that an actual user
puts actual capital into and that a regulator might inspect -- needs
the reference-app primitive plus all of the following. None of the
items in this section live in the SolID repo; they belong to the
integrating protocol.

### 5.1 Product and accounting

  - SPL token vault with reentrancy-equivalent state machine.
  - Deposit / withdraw / share-redeem flows.
  - Per-asset risk parameters and oracle price feeds.
  - Liquidation engine if the pool issues leverage.
  - Real reconciliation tooling and treasury accounting.

### 5.2 Compliance posture

  - Legal opinion stating which jurisdiction the gate is intended
    to cover and what the gate actually proves.
  - Per-jurisdiction issuer sets:
    - US -- e.g. FinCEN-registered, OFAC-deny-listed.
    - EU -- e.g. eIDAS-compliant issuers, GDPR retention rules.
    - Sanctioned-country denylists, refreshed against OFAC SDN.
  - Documentation of how the gate satisfies the regulator's actual
    test, not just "the proof verifies".
  - Audit trail that survives subpoena: which schema, which issuer
    set, which VK generation, which timestamp, which root.

### 5.3 Authority and governance

  - DAO or multisig over:
    - Which schemas gate which actions.
    - Which issuer set is approved per jurisdiction.
    - Pausing the gate.
    - Emergency rotation of the verifier ID, schema, or issuer set.
  - Procedure for adding / removing issuers without breaking
    in-flight proofs.

### 5.4 Verifier-side hardening

  - Verifier nonce bound to the user's order or transaction so the
    proof cannot be replayed in another session.
  - Session-bound expiration (the audit's `expiration` field) tied
    to a specific market action, not just a 5-minute window.
  - Front-run / sandwich resistance for the gate transaction.

### 5.5 Operational

  - Production indexer for `CredentialVerified`,
    `IssuerLeafAppended`, `IssuerLeafReplaced`, and the integrating
    program's own events.
  - Oncall rotation and incident-response playbook.
  - Key-rotation procedure for the integrating program's authority.
  - Independent third-party audit of the integrating protocol,
    scoped to include the gate logic and the SolID CPI shape.
  - Bug bounty.

### 5.6 UX

  - Mobile wallet support that actually works for credential
    transport (see ADR-pending: secure credential transport).
  - Onboarding flow that walks a non-crypto user through "get a
    credential -> store it -> prove against it".
  - Error UX that distinguishes "your credential is wrong",
    "your credential expired", "your issuer was revoked",
    and "the verifier is paused".

A realistic estimate for items 5.1 through 5.6 is 3-6 engineering
months per integrating protocol, dominated by 5.1 (product) and 5.2
(compliance), with 5.3 / 5.4 / 5.5 / 5.6 in parallel.

## 6. Build order for the reference app itself

Within the Day 8-10 window described in the audit:

  Day 8 morning -- scaffold `programs/reference-pool/`, write
  `join_pool` ix, unit-test the CPI path with anchor-bankrun.

  Day 8 afternoon -- scaffold `examples/reference-verifier-app/`,
  wire up wallet-adapter and the WASM prover.

  Day 9 -- end-to-end on localnet: dev issuer issues a credential
  to the connected wallet, browser proves age >= 21 + approved
  issuer, submits, `PoolMembership` PDA appears.

  Day 10 morning -- deploy to devnet via
  `scripts/deploy_reference_app.ts`, run the same flow against
  devnet, capture the tx signatures into
  `docs/DEPLOYMENT_AND_TESTING.md`.

  Day 10 afternoon -- record a 90-second screen capture for the
  audit close-out artifact.

## 7. Cross-references

  - sec/audits/2026-04-25_v0.6.1_deep_comprehensive_audit.md
    Section 4 Day 8-10 -- master schedule and exit criteria.
  - docs/integration-guide.md -- SDK-level "how to call the
    verifier" reference; the reference app uses these APIs.
  - docs/verifier-guide.md -- verifier-side query DSL.
  - docs/FORWARD_ROADMAP.md -- where the reference app sits in the
    larger Phase 4 / Phase 5 / Phase 6 picture.
  - sec/SECURITY_REGISTRY.md -- SOLID-SEC-046 (CU regression gate)
    is the gate that the reference app's CPI path implicitly tests.

## 8. Open questions deferred to a follow-up doc

  - Secure credential transport from issuer to holder wallet.
    Today `scripts/lib/e2e_state.ts` writes plaintext state to
    disk; this is fine for the reference app but not for a real
    integration. Tracked separately as an ADR-pending item.
  - Mobile prover ergonomics. WASM prover times on a phone are
    10-20s; whether this is acceptable UX or whether delegated
    proving (with privacy disclosure, ARCH-2) is required for a
    given integrator is a per-integrator decision.

---

End of doc. Update this file whenever the reference app's scope or
non-scope changes; do not duplicate the content into the audit doc.
