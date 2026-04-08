# SolID Protocol — Infrastructure Manifesto
**The Private On-Chain Identity Layer for Solana**

## 1. Executive Summary
SolID is a modular identity infrastructure designed to bridge the gap between **Real-World Identity (KYC/Records)** and **On-Chain Privacy**. It serves as the "Computation & Proof" layer for the Solana ecosystem, specifically designed to integrate with the **Solana Attestation Service (SAS)** and **Light Protocol**.

### Is this really Infrastructure?
**Yes.** Unlike a standalone KYC app, SolID provides:
- **Generic Schema Registry**: Any business can define a schema (e.g., "Medical Record v1", "Accredited Investor v2").
- **DAO-Governed Issuer Registry**: A decentralized "Trust Anchor" where the community decides who is a valid issuer.
- **Pluggable ZK Verifier**: Any dApp (DeFi, Social, Governance) can verify a private proof with a single CPI call.

## 2. The Stakeholder Ecosystem
The system relies on the interaction between four distinct parties:

| Party | Role | Motivation |
|---|---|---|
| **Issuers** | Governments, DAO-KYC providers, Hospitals | Earn fees or provide compliance services; must stake SOL as "Skin in the Game". |
| **Holders** | End-users (Wallets) | Maintain privacy while accessing gated services; 100% self-sovereign control. |
| **Verifiers** | DeFi Protocols, Social Apps, Airdrop Launchpads | Ensure compliance (KYC) or Sybil-resistance (Proof of Personhood) without handling PII. |
| **The DAO** | Token Holders / Governors | Manage the "Trust Registry" and slash malicious issuers to maintain protocol integrity. |

## 3. Integration Matrix: Where does it fit?

### A. The Solana Ecosystem Fit
- **Data Layer (SAS)**: SolID should be the standard ZK-interface for SAS. While SAS stores the data, SolID makes it useful for private DeFi.
- **Compression (Light Protocol)**: SolID utilizes Light Protocol's Merkle trees to "compress" millions of identity commitments into a single on-chain root, reducing storage costs by >99%.

### B. Vertical Use Cases
| Vertical | Use Case Example | Target Integration Partners |
|---|---|---|
| **Institutional DeFi** | Undercollateralized loans requiring "Accredited" status without revealing net worth. | **Maple Finance, Ondo Finance, TrueFi** |
| **Sybil-Resistance** | Preventing bot-farming in airdrops or quadratic voting. | **Jupiter (Launchpad), Realms (Governance), Gitcoin (Solana)** |
| **Health & Insurance** | Proving vaccination or insurance coverage for on-chain health services. | **Helium (Worker coverage), New health dApps** |
| **Real World Assets (RWA)** | Proving residency or ownership for tokenized real estate. | **Parcl, Homebase** |

## 4. Target Partners & Integration Strategy
To become a standard, SolID must integrate with the "Gatekeepers" of Solana:
1. **Helius / Triton**: To index `CredentialVerified` events and provide "Identity-Rich" APIs for developers.
2. **Backpack / Phantom**: To allow users to generate ZK-proofs (via WASM) directly inside the wallet UI.
3. **Jupiter**: To enable "Verified-Only Swaps", protecting against toxic flow or high-leverage bot attacks.

## 5. Why is this needed? (The Problem/Solution)
**The Problem**: Currently, "Identity" on Solana is either completely public (NFTs/SNS) or centralized (relying on a specific API). Proving you are "Over 18" currently requires showing your birthday.
**The Solution**: SolID allows you to prove "Condition X is True" without revealing "Data Y". It is the only protocol in the ecosystem combining **Registry Modularization**, **Native Solana ZK-Syscalls**, and **Multi-Credential Composable Querying (N=4)**.

| Feature | Infrastructure Baseline |
|---|---|
| **Privacy** | 2-Layer Merkle SMT (Light Protocol) + Anchor Nonce. |
| **Composability** | Single ZK proof for predicates across up to 4 independent issuers. |
| **Security** | Circuit-level Identity Binding + Mandatory Scope Nonces. |
| **Rent** | 200x reduction via ZK-Compressed Stateless Storage. |
