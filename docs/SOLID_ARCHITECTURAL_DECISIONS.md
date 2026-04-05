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

## 4. The "Security vs. Latency" Compromise

**Evaluation**:
The system is designed with a **Security-First** philosophy. In identity protocols, speed is worthless if data can be stolen or trust can be spoofed.

| Category | Impact | Reason |
|---|---|---|
| **Privacy** | 🚀 Major Improvement | Transition to Stateless, Unlinkable Privacy. |
| **Security** | 🚀 Major Improvement | Elimination of Identity Theft and Governance Takeovers. |
| **UX Latency**| 📉 Notable Regression | ~30–50% increase in browser (WASM) proof generation time. |
| **Capital** | 📉 Notable Regression | 14-day lock on issuer stake for safety. |

**Final Verdict**: SolID trades a few seconds of user "wait time" for a system that can secure billions of dollars in RWA value. We accept higher operational "heaviness" in exchange for "Protocol Credibility."
