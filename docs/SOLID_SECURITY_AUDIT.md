# SolID Protocol — Security Audit Report

NOTICE: this document is historical. It was written before the 2026-04 remediation landed. Many of its findings have been fixed in v0.3. Use docs/POST_REMEDIATION_AUDIT.md for the current state. Read this file only as a historical record of what was broken.

**Comprehensive Technical Review & Vulnerability Assessment**

> **HISTORICAL SNAPSHOT.** Findings here were remediated through
> v0.1 → v0.2. Current security posture is tracked in
> [`SOLID_INFRA_IMPROVEMENTS_STATUS.md`](./SOLID_INFRA_IMPROVEMENTS_STATUS.md)
> (§1 "Security & Cryptographic Integrity") and the v0.2 additions in
> [`REVOCATION_DESIGN.md`](./REVOCATION_DESIGN.md).

## 1. Audit Overview
- **Scope**: Rust Programs (Anchor), Circom Circuits, WASM Crypto Core, TS SDK.
- **Methodology**: Static Analysis, Logic Flow Review, Cryptographic Soundness Check.
- **Date**: 2026-04-04

## 2. Executive Summary
The SolID Protocol architecture is sophisticated but currently contains **Critical Vulnerabilities** in both the Zero-Knowledge circuits and the On-Chain Governance programs. While the "happy path" works, an attacker can bypass the Sybil-resistance logic and steal other users' identities.

---

## 3. Critical Vulnerabilities [Level: SEV-0]

### A. Identity Collision & Theft (Missing Key-Pair Constraint)
- **Location**: `circuits/compound_query.circom`
- **Impact**: **IDENTITY THEFT**. A malicious actor ("Attacker") can pick up any `merkleRoot` commitment from someone else ("Victim") on-chain and prove it's theirs.
- **Technical Detail**: The circuit takes `holderBJJPrivKey` (private) and `holderBJJPubKeyAx/Ay` (private). These are used to calculate the `nullifier` and the `commitment` leaf respectively. However, **there is no constraint** requiring that `PubKey` is derived from `PrivKey`.
- **Attack Vector**: 
    1. Attacker finds a victim's commitment on-chain.
    2. Attacker provides the victim's `PubKey` and the victim's `credentialData` to satisfy the Merkle and Signature checks.
    3. Attacker provide's **their own** `PrivKey` to generate a unique nullifier.
    4. The proof verifies because the circuit never checks `Ax/Ay == BJJ.derive(PrivKey)`.
- **Recommendation**: Add a BabyJubJub derivation component (`EscalarMulAny`) to the circuit to bind the private key to the commitment's public key.

### B. Governance Takeover (Self-Declared Weights)
- **Location**: `programs/issuer-registry/src/lib.rs` -> `vote_on_issuer`
- **Impact**: **PROTOCOL COLLAPSE**. Any user can approve or reject any issuer regardless of their actual "DAO token" holdings.
- **Technical Detail**: The parameter `vote_weight: u64` is passed directly as an instruction argument and added to `issuer.votes_for` without any validation.
- **Attack Vector**: An attacker calls `vote_on_issuer` with `weight = u64::MAX`, immediately approving their own malicious issuer.
- **Recommendation**: The program must perform a CPI to a SPL-Token account or a DAO governance program to verify the user's actual staked or voting balance.

---

## 4. High & Medium Risks [Level: SEV-1, SEV-2]

### C. Bloom Filter False Positives (Denial of Service)
- **Location**: `programs/zk-verifier/src/lib.rs`
- **Impact**: **USER EXCLUSION**. As the system scales, valid users will be blocked from proving their identity.
- **Technical Detail**: Bloom filters are probabilistic. With the current configuration (~100K capacity), a 1% false positive rate means 1 in 100 users will be incorrectly told "Nullifier Already Used".
- **Recommendation**: Use a **Stateless Merkle Tree** (via Light Protocol) or an **On-Chain Sparse Merkle Tree** for nullifiers. This provides 100% precision.

### D. Locked Funds (Missing Stake-Withdraw Logic)
- **Location**: `programs/issuer-registry/src/lib.rs`
- **Impact**: **FROZEN SOL**. Issuers who are "Rejected" or "Revoked" cannot get their SOL stake back.
- **Technical Detail**: There is no instruction in the program that transfers SOL out of the `stake_vault` back to the issuer's wallet.
- **Recommendation**: Implement a `withdraw_stake` instruction that is enabled if `status == Rejected` or `Revoked` (after a slashing period).

---

## 5. Implementation Limitations [Level: INFRA-GAP]

### E. Hardcoded Public Input Size
- **Location**: `programs/zk-verifier/src/lib.rs` -> `NR_PUBLIC_INPUTS = 21`
- **Issue**: The verifier is not generic. It cannot handle different identity circuits (e.g., a "Simple Age Check" vs a "Complex Credit Score") without redeploying.
- **Recommendation**: Move `NR_PUBLIC_INPUTS` to a field in the `VerifierConfig` and support dynamic VK sizes.

## 6. Logic Flow Analysis
1. **Schema Registry**: Secure. Uses PDA seeds for namesquatting protection.
2. **Issuer Registry**: **BROKEN**. Governance logic is purely symbolic currently.
3. **ZK Verifier**: Real Groth16 verification works, but the nullifier logic (Bloom) is only suitable for short-lived alpha tests, not production identity.
