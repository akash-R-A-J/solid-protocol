# Forward Roadmap (Phase 4 -> Phase 6)

> Forward-looking implementation plan from v0.6.1 (post-Phase-2,
> post-Phase-3-Impl-1..4) through devnet v1, pre-mainnet hardening,
> and ecosystem launch. Last refreshed: 2026-04-25 late-session for
> v0.6.1 + the E2E-unblock SOLID-SEC-048 finding (BJJ subgroup BPF
> CU exhaustion; mainnet deploy-blocker; promoted to top of Phase 4
> P0 below).

This document is the canonical forward plan. Two related docs are
deliberately not duplicated here:

  - sec/SECURITY_REGISTRY.md -- one-line status per finding. This
    file references SEC-NNN IDs but does not restate them.
  - docs/IMPROVEMENTS_ROADMAP.md -- historical Phase 1 / Phase 2
    backlog with [x] / [~] / [ ] reconciliation. Preserved as is.
  - sec/audits/2026-04-25_v0.6.1_deep_comprehensive_audit.md --
    the canonical post-fix audit, including the Day 1..10 ship
    schedule for Phase 4. This file references that schedule but
    does not restate the per-day breakdown.

If you are looking for "what's the next concrete thing to do",
read this file. If you are looking for "what is the current state
of finding SEC-NNN", read the registry. If you are looking for "is
the protocol secure right now", read the v0.6.1 audit.

## How to read this document

Each phase is broken down by priority:

  P0  Blocking. Must land before the phase exits.
  P1  Should-have. Phase exits without these only if explicitly
      deferred with a registry entry or an ADR.
  P2  Can-slip. Track in registry; revisit at next phase boundary.
  P3  Nice-to-have. Open issues in the backlog only.

Where an item already has a security-registry ID, the ID is the
source of truth and the line here is a pointer.

---

## Phase 4 -- Devnet v1 (testable end-to-end)

Goal: devnet deployment whose full pipeline (issuer registration ->
credential issuance -> proof generation -> on-chain verification)
is exercised by integration tests, observed by an indexer, and
demonstrated by a reference app.

Master schedule: sec/audits/2026-04-25_v0.6.1_deep_comprehensive_audit.md
Section 4, Days 1..10.

### Phase 4 -- P0 (blocking)

  - SOLID-SEC-048  `register_issuer` BJJ prime-order subgroup check
                   exceeds the 1.4M CU per-tx ceiling on BPF.
                   Localnet/devnet currently runs an interim
                   bypass (Cargo feature `sec007-skip-onchain` on
                   issuer-registry; off-chain TS predicate
                   `isInPrimeOrderSubgroup` as load-bearing gate;
                   `Sec007Bypass` event for telemetry).  **MAINNET
                   DEPLOY-BLOCKER** -- no Phase 5 sign-off without
                   a real fix.  Three real-fix candidates (in
                   increasing soundness order): SDK-level cofactor-
                   clear (cheapest; trusts off-chain caller);
                   move subgroup gate into the issuance circuit
                   (~30K extra constraints; batches with SEC-006
                   Part 2 trusted-setup cycle); `sol_babyjubjub_*`
                   syscall upstream proposal.  Includes shipping
                   the regression gate
                   `tests/integration/register_issuer_compute_units.test.ts`
                   that asserts the no-feature build still hits
                   the CU ceiling until the real fix lands.
                   Tracking: B9 in `docs/E2E_BLOCKERS.md`, P0-7 in
                   `docs/IMPROVEMENTS_ROADMAP.md`.
  - SOLID-SEC-017  Replace `ark_std::test_rng()` in tools/solid-prover.
                   Half-day fix; add a "two proofs over the same inputs
                   must produce different nullifiers" regression test.
  - SOLID-SEC-045  Atomic `update_issuer_tree_root` inside
                   `revoke_issuer_atomic` and
                   `request_withdrawal_atomic`. Half-day fix; closes
                   the half-revoked window.
  - SOLID-SEC-046  CI CU-budget regression gate on `verify_batch_proof`
                   plus `docs/CU_BUDGET.md` baseline. Half-day fix.
  - SOLID-SEC-010  Cross-language vectors expanded from 3/10 to 10/10
                   primitives. Required CI gate. Day 1.
  - Integration suite 02..10 -- the eight tests named in the audit's
                   Day 2-3 schedule. Required to call the pipeline
                   "behaviourally tested".
  - Devnet deploy of all three programs and `initialize.ts` run with
                   `SOLID_VK_SHA256` set. Day 5.
  - Reference verifier app deployed and able to prove + verify a
                   credential end-to-end on devnet. See
                   docs/REFERENCE_VERIFIER_APP.md.
  - Observability watcher for the four event families
                   (`CredentialVerified`, `CredentialIssued`,
                   `IssuerLeafAppended`, `IssuerLeafReplaced`,
                   plus VK rotation events). Day 7.

### Phase 4 -- P1 (should-have)

  - Module split of `programs/issuer-registry/src/lib.rs` into
                   governance / tree_lifecycle / issuance / slashing /
                   state. Day 4. Improves auditor velocity.
  - Revocation SDK helpers in `ts-sdk/packages/issuer/`:
                   `revokeIssuer(oldRoot, merkleProof)` and
                   `requestWithdrawalAtomic(oldRoot, merkleProof)`.
  - Squads 3-of-5 multisig over `registry_config.authority` and
                   `IssuerTreeBinding.operator`. Closes SOLID-SEC-043
                   and reduces SOLID-SEC-013. Day 6.
  - ADR-0016 "Devnet operational runbook" -- VK upload, finalize,
                   backfill, rotation paths.
  - 10k-proof soak test on devnet, success rate >= 99.9%, p99 latency
                   recorded. Day 8-10.
  - 100-issuer stress test (register, approve, enroll, bulk-revoke
                   20, verify post-revocation proofs fail). Day 8-10.

### Phase 4 -- P2 (can-slip past Phase 4 if necessary)

  - Production indexer adapter (Helius DAS or Shyft). ARCH-3 in
                   the historical roadmap; needed before any external
                   integrator can run a node-free verifier flow.
  - HeliusDasAdapter implementation of `MerkleProofAdapter`.
  - Anchor IDL-based TS client. ARCH-5.
  - W3C VC translation layer scaffold. ARCH-6.

### Phase 4 -- exit criteria

Mirrors the v0.6.1 audit Section 4 exit criteria:

  - All CI jobs green, including the 10 integration tests.
  - `deployments/devnet.json.deployed_at != null` and
    `upgrade_authority` recorded.
  - Reference app proves + verifies a credential end-to-end on devnet.
  - Observability dashboard live for the four event families.
  - 10k-proof soak completes with < 0.1% failure rate.
  - All Phase 4 P0 items closed.

Items explicitly NOT in Phase 4 exit:

  - Multi-party trusted setup (Phase 5).
  - SOLID-SEC-006 Part 2 (`vk_generation` in public-input contract).
                Deferred to next trusted-setup cycle.
  - DAO over the VK authority (Phase 5).
  - Revocation indexer event contract for holders (Phase 5).

---

## Phase 5 -- Pre-Mainnet Hardening

Goal: every gate that an external auditor and an external integrator
will check is closed before mainnet deploy.

### Phase 5 -- P0 (mainnet blockers)

  - SOLID-SEC-012  Multi-party trusted-setup ceremony. Replaces
                   `circuits/scripts/setup.js` single-party setup.
                   Coordinator + minimum 3 independent contributors
                   + transcript verification + on-chain
                   commitment. Hard mainnet blocker.
  - SOLID-SEC-006 Part 2  `vk_generation` bound into the circuit's
                   public inputs. Requires a circuit change; batches
                   with the new trusted-setup cycle so it lands once,
                   not twice.
  - SOLID-SEC-013  Replace single-pubkey `VerifierConfig.authority`
                   and `RegistryConfig.authority` with DAO-controlled
                   threshold PDAs. Phase 4 Squads multisig is the
                   half step; Phase 5 finishes it with on-chain DAO
                   votes.
  - External third-party audit of the post-Phase-2 codebase.
                   Scope: all three programs, the circuit, the WASM
                   bridge, the SDK. Findings tracked into
                   sec/SECURITY_REGISTRY.md as SOLID-SEC-NNN.
  - Mainnet deploy plan + dry-run on devnet with full multisig
                   ceremony.
  - Bug bounty live on Immunefi (or equivalent) before mainnet.

### Phase 5 -- P1 (should-have)

  - Revocation v1 operator workflow finishing -- holder SDK helper,
                   issuer SDK helper, indexer event contract for
                   holders. Circuit and on-chain support already in
                   place since Phase 2.
  - SOLID-SEC-014..-019, -021, -034 -- the open MEDIUM-severity
                   registry items not addressed in Phase 4.
  - Cross-program upgrade-authority hygiene: rotate every program's
                   upgrade authority to the same multisig.
  - `cargo audit` in CI as a required gate.
  - Per-environment manifest signing (deployments/devnet.json,
                   deployments/mainnet.json) so a forged manifest
                   fails CI.

### Phase 5 -- P2 (can-slip past mainnet if necessary)

  - SOLID-SEC-022..-024, -035 -- the open LOW-severity registry items.
  - SOLID-SEC-025..-026, -037..-038 -- the open INFO-severity items.
  - Doc cleanup items P3-2..-8 in the historical roadmap.
  - ARCH-1 -- universal-setup PLONK evaluation. Only if the circuit
                   changes are projected to be frequent. Default: skip.

### Phase 5 -- exit criteria

  - Multi-party trusted setup complete; ceremony transcript
    published; circuit artifacts content-addressed and pinned.
  - External audit closed; all HIGH and MEDIUM findings either
    fixed or accepted with rationale.
  - DAO governance live over both authorities.
  - Bug bounty live with at least 30 days of public exposure
    before mainnet deploy.
  - Mainnet deploy on a frozen tag with full ceremony.

---

## Phase 6 -- Ecosystem and Network Effects

Goal: SolID is the default identity primitive on Solana for at
least one regulated use case (e.g. age-gated trading, geo-fenced
RWA access, KYC for institutional pools).

### Phase 6 -- P1 (foundational, no order)

  - First real integration partner shipping a production gate that
                   calls `verify_batch_proof` over CPI. See
                   docs/REFERENCE_VERIFIER_APP.md Section 5 for the
                   full list of what this requires from the
                   integrator.
  - Issuer onboarding pipeline -- documented procedure, automated
                   bootstrap, monitoring, KYC review of issuer
                   identity if required by jurisdiction.
  - Schema registry curation -- a small set of schemas (basic
                   identity, KYC tier 1/2/3, accredited investor,
                   age-only, country-only) signed off by the DAO.
  - Mobile wallet integration. ADR-pending on credential transport
                   and wallet-side storage. Beyond what
                   wallet-adapter provides today.
  - Public status page with proof-verification metrics, issuer
                   counts, schema usage, VK generation, and
                   incident history.

### Phase 6 -- P2 (depends on traction)

  - ARCH-2 delegated proving with TEE attestation, opt-in only.
                   Only if mobile prover times become a verified
                   user complaint.
  - ARCH-6 W3C VC translation layer. Only if an enterprise issuer
                   needs it.
  - Cross-chain bridges of identity commitments. Speculative;
                   needs design before implementation.

### Phase 6 -- P3 (nice-to-have)

  - SDK in additional languages (Python, Go) for issuer-side use.
  - GraphQL frontend over the indexer.
  - Reference app v2 with multi-schema and multi-issuer-set demo.

### Phase 6 -- exit criteria (open-ended)

  - At least one production integrator shipping for >= 90 days.
  - Issuer onboarding automated to the point that adding an issuer
    does not require a code change in this repo.
  - At least three independent issuers live on mainnet.
  - At least three independent verifier integrations live on
    mainnet.

---

## Importance ladder (for executive readers)

If you can only read one section, this is it.

  Most important, this quarter:
    - Phase 4 P0: SOLID-SEC-017, -045, -046; cross-language vectors;
      integration tests 02..10; devnet deploy; reference app;
      observability.

  Most important, next quarter:
    - Phase 5 P0: multi-party trusted setup; SEC-006 Part 2 in the
      same cycle; DAO over authorities; external audit.

  Most important for the year:
    - Phase 6 P1: first real integration partner; issuer onboarding
      pipeline; mobile credential transport.

  Anything else is genuinely "can slip" until the above is locked.

---

## Cross-references

  - sec/SECURITY_REGISTRY.md  -- per-finding status truth.
  - docs/IMPROVEMENTS_ROADMAP.md  -- historical Phase 1/2 backlog.
  - sec/audits/2026-04-25_v0.6.1_deep_comprehensive_audit.md  --
    canonical audit; Section 4 has the Day 1..10 schedule for
    Phase 4.
  - docs/REFERENCE_VERIFIER_APP.md  -- design for the Phase 4
    Day 8-10 reference verifier app, including what "real"
    integration looks like in Phase 6.
  - docs/REVOCATION_DESIGN.md  -- circuit + on-chain support for
    revocation; SDK helper work referenced in Phase 4 P1 lives
    against this design.

End of forward roadmap. Updates land here, not in the historical
IMPROVEMENTS_ROADMAP.md.
