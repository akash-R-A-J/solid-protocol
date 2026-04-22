# SolID Architectural Decisions — Rationale & Trade-offs

This document serves as the historical record for why certain high-level architectural paths were chosen for the SolID Protocol. It establishes the "Protocol Philosophy" for future contributors and maintainers.

---

## 1. Privacy-First Key Hierarchy (Master → Derived Keys)

**Decision**: Replaced a single BabyJubJub identity key with a derived-key hierarchy.
**Rationale**: 
- **Privacy Core**: Using a master key directly for all credentials allowed a malicious verifier to link a user's medical records to their financial records via their public key.
- **Unlinkability**: By deriving `credentialPrivKey = Poseidon(masterIdentityKey, schemaHash)`, each credential has a unique, deterministic public key that is uncorrelated to other schemas.
- **Security Cost**: Adds one additional Poseidon hash to the circuit per credential.
- **Trade-off**: Accepted ~5% increase in circuit constraints to achieve "Gold Standard" unlinkable privacy.

---

## 2. Issuer Reputation Tiers (Incentivized Trust)

**Decision**: Moved from a flat "Stake-only" model to a Tiered model (Community, Enterprise, Regulated, Government).
**Rationale**:
- **Institutional Onboarding**: Large institutions (Governments/Banks) require higher certainty and different risk models than community badge-issuers.
- **Capital Efficiency**: Trusted government entities are exempt from high staking requirements, reducing barriers for high-value data providers.
- **Social Graph**: Allows dApps to selectively trust tiers (e.g., "Only allow users with a government-issued ID").

---

## 3. Snapshot-Based Governance (Anti-Flash Loan)

**Decision**: Implemented slot-based snapshots and vote locking for issuer approvals.
**Rationale**:
- **Attack Vector**: Flash loans allow an attacker to borrow millions in tokens, approve a malicious issuer, and return the tokens in a single block.
- **Protocol Safety**: Snapshots ensure only long-term holders can influence the trust registry. Vote locking ensures skin in the game.

---


---

## 5. Multi-Credential Batch Engine (N=4) — Hardened (Phase 3.3)

**Decision**: Implemented a fixed-size batch verification circuit ($N=4$) with **per-credential identity anchors** and strictly ascending **schema ordering**.
**Rationale**:
- **Privacy Core**: Initial implementations used a single anchor for all credentials, which inadvertently linked them. Phase 3.3 refactors this to use unique, schema-derived keys for each credential in the batch while maintaining a shared state root.
- **Malleability Protection**: Strictly ascending canonical ordering ensures a specific set of credentials can only be proven in a single, deterministic way, eliminating permutation-based malleability.
- **Protocol Baseline**: Maintains $N=4$ for power (multi-issuer queries) while achieving "Gold Standard" ZK privacy.

---

## 6. Global Root & Temporal Consistency

**Decision**: All credentials in a batch MUST verify against the same Global State Root snapshot.
**Rationale**:
- **Security Vector**: Using different snapshots for different credentials could allow a "partial revocation bypass" where a user uses a stale root for a revoked credential while using a fresh root for others.
- **Consistency**: Enforcing a shared root ensures that the holder's entire identity state is fresh and synchronized at the moment of proof generation.

| Category | Impact | Reason |
|---|---|---|
| **Privacy** | 🚀 Major Improvement | Transition to Stateless, Unlinkable Privacy. |
| **Security** | 🚀 Major Improvement | Elimination of Identity Theft and Governance Takeovers. |
| **UX Latency**| 📉 Notable Regression | ~10s increase in browser proof time for N=4 batches. |
| **Capital** | 📉 Notable Regression | 14-day lock on issuer stake for safety. |
| **Composability**| 🚀 Major Improvement | Cross-issuer queries (Age AND Credit) now possible. |

**Final Verdict**: SolID is now a **composable identity query engine**, not just a simple attestation system.
