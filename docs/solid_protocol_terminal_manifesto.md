# The SolID Protocol — Terminal Manifesto (RC-1.0)
## The Privacy-Preserving Identity Layer for the Compressed Internet

> "Identity is the final bottleneck of decentralized finance. To scale to a billion users without sacrificing sovereignty, we don't need just better accounts—we need stateless, non-malleable, and state-compressed proving systems."

---

## 🏛️ 1. The Narrative: What is SolID?

**SolID** (Sovereign On-chain Identity) is a **stateless ZK-verification middleware** built for the Solana ecosystem. It is the cryptographic "Invisible Proof Oracle" that allows any entity (Protocol, DAO, Game) to verify private facts about a user—such as identity, assets, reputation, or membership—without revealing the underlying data.

### The Problem We Solve: "The Verifiable Fact Gap"
Traditional applications on-chain face a critical choice:
1. **Total Doxxing**: Forced to see the user's raw wallet history or identity to verify eligibility.
2. **Total Blindness**: Unable to verify if a user is a whale, a human, or a sanctioned entity.

### The SolID Solution
SolID solves this by acting as a **Universal Proof Layer**.
- **Issuers** (e.g., Banks, Games, DAOs) sign attestations of facts.
- **Holders** (Users) generate **ZK Proofs** of those facts (Groth16).
- **Verifiers** (All Solana DApps) settle these proofs on-chain via **State Compression** for ~0.00005 SOL.

---

## 🏗️ 2. Core & Killer Features

### 🚀 The Killer Feature: "Atomic Batch Engine" ($N=4$)
SolID handles up to 4 independent credentials in a single Groth16 proof. You can prove **Membership in a DAO** AND **Staking > 500 SOL** AND **Age > 18** simultaneously.
- **Efficiency**: One proof, one verification syscall, one gas fee.
- **Privacy**: The verifier only sees the final result, not the individual attribute values.

### 📉 Generic Proof Middleware (Chainlink for Private Facts)
Unlike identity-only protocols, SolID is **Schema-Agnostic**. You can define any verifiable condition:
- Prove **NFT Rarity** without revealing the token ID.
- Prove **Discord Role** without revealing your Discord ID.
- Prove **Trade Volume** without revealing your trade history.

### 🔄 Stateless Identity Rotation
Our most sophisticated security feature. A user can "Rotate" their identity by bumping a `revocationNonce` on-chain. 
- All previously generated proofs for all DApps are instantly invalidated.
- The user is **not** forced to re-verify with the issuer; they just generate new proofs using the new nonce.
- This is the "Kill Switch" for identity theft.

---

## 🧠 3. The Tough Decisions: Why we built it this way

### Decision 1: Groth16 over PLONK
We chose **Groth16** for our ZK system despite the requirement for a "Trusted Setup." 
- **The Rationale**: Solana provides native syscalls (`alt_bn128`) for the underlying curve operations. This makes Groth16 verification on-chain incredibly cheap and fast (~200k compute units). PLONK or STARKs would currently be too gas-intensive for frequent identity checks on-chain.

### Decision 2: Light Protocol over Pure Merkle Trees
Initially, we considered building our own Merkle trees. 
- **The Rationale**: Maintaining a secure, balanced tree on-chain is a massive engineering feat. By leveraging **Light Protocol**, we inherit their audited state-compression infrastructure, allowing us to focus 100% on the **Identity Logic** rather than the **Storage Logic**.

### Decision 3: Per-Schema Key Derivation
Instead of using the user's Main Wallet Address for everything, we use the `masterIdentityKey` to derive separate BabyJubJub keys for every `schemaHash`.
- **The Rationale**: This prevents "Unlinkability Regression." If a user proves they are over 18 to App A, and they are a Platinum Member to App B, those two apps cannot cooperate to link those two accounts to the same person.

---

## 🛠️ 4. The System Architecture

The SolID architecture is a **Triple-Anchor System**:

### Tier 1: The Cryptographic Core (Rust & Circom)
- **`crates/solid-core`**: The source of truth for Poseidon hashing and BabyJubJub math.
- **`circuits/`**: The Zero-Knowledge R1CS constraints that enforce the rules of identity.

### Tier 2: The On-Chain Settlement (Solana)
- **`programs/zk-verifier`**: The high-performance gatekeeper. It verifies Proofs, checks Global State Roots, and registers Nullifiers.
- **`programs/issuer-registry`**: The DAO-controlled trust layer. It manages who is allowed to issue credentials.

### Tier 3: The Resilient SDK (TypeScript)
- **`ts-sdk`**: The developer's primary interface. It features **Priority Failover** (handling multiple RPC nodes) and the **WASM Memory Bridge** (zero-copy proof generation).

---

## 🏢 5. Competitive Landscape: Why SolID Stands Out

| Feature | SolID Protocol | Worldcoin | Privado ID (Polygon) |
| :--- | :--- | :--- | :--- |
| **Trust Model** | Fully Decentralized (DAO) | Centralized (Orb) | Multi-Chain |
| **Storage cost** | 200x Compressed | High (EVM gas) | Medium |
| **Solana Native**| ✅ Yes | ❌ No | ❌ No |
| **Aggregation** | ✅ Batch-of-4 | ❌ Single-Proof | ❌ Single-Proof |
| **Performance** | WASM Native Bridge | Standard SDK | Mobile-heavy |

SolID is the only protocol designed specifically for the **High-Throughput, Low-Latency** reality of Solana. We prioritize **Statelessness** and **Atomic Performance**.

---

## 🚀 6. Real-World Use Cases

1. **Compliant DeFi (DeFi 2.0)**: Use SolID to prove you are not from a sanctioned region without revealing your passport or connecting your public wallet to a KYC provider.
2. **Sybil-Resistant Airdrops**: Ensure every recipient of an airdrop is a unique human without collecting their email or phone number.
3. **Private Undercollateralized Loans**: Prove your credit score (held as a credential) to a lender without revealing your total net worth or other financial positions.
4. **ZK-Voting**: Prove you are a member of a community (DAO) to vote on-chain without revealing which member you are, preventing targeted bribery or coercion.

---

## 👥 7. User Base: Who uses SolID?

- **The Privacy-Conscious User**: People who want to participate in the digital economy but refuse to be tracked by corporations.
- **The Resource-Constrained Developer**: DApps that want to verify identity at scale but cannot afford the rent costs of millions of PDAs.
- **The Institution**: Regulated entities that need a secure way to issue credentials that their customers can use elsewhere in the ecosystem.

---

## 🚧 8. The "Last 1%": What's still to be done?

- **[ ] 100x Improvement (Recursive SNARKs)**: Moving from individual proofs to "Proofs of Proofs" to achieve constant-time verification for unlimited batch sizes.
- **[ ] Recursive Revocation (Phase 6)**: Implementing origin-anchor tracking in the Issuer Registry to prevent trust-degradation and "infectious approvals."
- **[ ] Turnkey Issuer Nodes (SaaS)**: Providing Dockerized BJJ-signing proxies for institutional onboarding.

---

## 📜 9. Why can you Trust this system?

SolID is based on **Provable Logic, not Promises**:
1. **Source Code is the Truth**: The ZK circuits define exactly what can and cannot be proven.
2. **Non-Malleability**: Our constraints (SEC-20) ensure that no data can be smuggled or reused.
3. **Governance-Locked**: The Issuer Registry is controlled by a DAO, not a single admin key.

---

# 🛑 10. The 100x Future: Where do we go from here?

If SolID is the base, the next level is **Recursive Verification**. 
Imagine a world where you don't just prove 4 credentials, but you prove a **Proof of Proofs**. One single byte can represent a user's entire life-history of verified attributes, all while maintaining perfect privacy. 

SolID is the foundation for the **Compressed Internet of Identity**.

---
*End of Protocol Manifesto — 100% RC-1.0*
