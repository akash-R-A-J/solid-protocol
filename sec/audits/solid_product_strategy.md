# SolID Protocol -- Product Strategy, Credential Security & Demo Architecture

> **Date:** 2026-05-03 | **Scope:** Credential delivery, storage, product apps, demo, docs, speed optimization

---

## 1. Credential Delivery: Issuer -> Holder (The Right Way)

### 1.1 The Problem

After `issue_credential` puts a 32-byte commitment on-chain, the issuer must deliver the **cleartext credential bundle** to the holder:

```
Bundle = {
  attestation_data: [age, country, level, ...],  // 8 field elements
  signature: { R8x, R8y, S },                    // BJJ EdDSA sig
  salt: bigint,                                   // commitment randomness
  issuer_bjj_pubkey: { x, y },                   // signing key
  schema_hash: bytes32,                           // which schema
  expiration_timestamp: u64,                      // when it expires
  merkle_proof: { siblings[], pathIndices[] },    // SPL AC tree proof
  leaf_index: u32,                                // position in tree
  tree_address: PublicKey,                        // which SPL AC tree
}
```

This bundle is ~2-4 KB. It contains NO secrets (no private keys), but it IS privacy-sensitive: if leaked, anyone can learn the holder's attributes (age, country, etc). The holder's BJJ private key is NEVER in this bundle.

### 1.2 Four Approaches Evaluated

#### Option A: Direct HTTPS Endpoint (Simple, Good)

```
Issuer Server                              Holder Client
     |                                          |
     |  1. Holder authenticates (OAuth/wallet)  |
     |<-----------------------------------------|
     |                                          |
     |  2. TLS-encrypted response with          |
     |     credential JSON                      |
     |----------------------------------------->|
     |                                          |
     |  3. Holder stores locally (encrypted)    |
```

**Pros:** Simple, proven, works everywhere, TLS provides transport encryption.
**Cons:** Issuer must run a server, requires authentication, no E2E encryption (TLS terminates at CDN/LB).
**Security:** TLS protects in transit; credentials are encrypted at rest by the holder SDK.
**Verdict:** Good for web issuers (universities, banks) with existing web portals. **Score: 7/10.**

#### Option B: Encrypted QR Code (In-Person)

```
Issuer Kiosk/App                           Holder Phone
     |                                          |
     |  1. Generate credential + encrypt to     |
     |     holder's Curve25519 pubkey            |
     |  2. Encode as QR (or series of QRs       |
     |     if > 2KB via animated QR)             |
     |----------------------------------------->|
     |     (camera scan)                        |
     |                                          |
     |  3. Holder decrypts with derived key     |
```

**Pros:** Air-gapped, no server needed, works offline, great for in-person KYC.
**Cons:** Limited to ~2KB per QR (need animated multi-QR for full bundle), requires camera, in-person only.
**Security:** E2E encrypted; no network involvement.
**Verdict:** Best for in-person issuance (DMV, hospital check-in, conference badge). **Score: 6/10** (limited by QR capacity).

#### Option C: Wallet-to-Wallet Encrypted Message (Decentralized)

```
Issuer Wallet                              Holder Wallet
     |                                          |
     |  1. Derive shared secret:                |
     |     ECDH(issuer_x25519, holder_x25519)   |
     |  2. Encrypt credential with AES-256-GCM  |
     |  3. Post encrypted blob to:              |
     |     - IPFS (pinned, content-addressed)    |
     |     - OR Solana memo ix (if < 566 bytes)  |
     |     - OR Dialect/Shadow Drive             |
     |----------------------------------------->|
     |  4. Send delivery notification:           |
     |     - On-chain event (CredentialDelivered) |
     |     - OR push notification via Dialect     |
     |----------------------------------------->|
     |                                          |
     |  5. Holder fetches + decrypts             |
```

**Pros:** Fully decentralized, E2E encrypted, no server needed, works async.
**Cons:** Complex key management (need X25519 in addition to BJJ), IPFS pinning costs, notification reliability.
**Security:** E2E encrypted with forward secrecy if using ephemeral ECDH.
**Verdict:** Most decentralized but over-engineered for v1. **Score: 5/10** for v1 complexity.

#### Option D: Hybrid ECIES + HTTPS (RECOMMENDED)

```
Issuer Server                              Holder Client
     |                                          |
     |  1. Holder connects wallet               |
     |<-----------------------------------------|
     |                                          |
     |  2. Holder sends ephemeral X25519 pubkey  |
     |     (derived from wallet: signMessage     |
     |      ("solid:credential-channel:v1")      |
     |      -> SHA-256 -> X25519 seed)           |
     |<-----------------------------------------|
     |                                          |
     |  3. Issuer encrypts credential:           |
     |     - ECIES envelope (ephemeral key +     |
     |       AES-256-GCM ciphertext + HMAC)      |
     |  4. Returns encrypted blob over HTTPS     |
     |----------------------------------------->|
     |                                          |
     |  5. Holder decrypts with derived key      |
     |  6. Holder stores encrypted at rest       |
```

**Pros:**
- **E2E encrypted** even if TLS is compromised (defense in depth)
- **No extra key management** -- X25519 key derived deterministically from wallet seed
- **Works over any transport** -- HTTPS, WebSocket, even email (blob is self-contained)
- **Scalable** -- issuer's HTTPS infra handles millions of deliveries
- **Recoverable** -- wallet seed recovers the decryption key
- **Standard crypto** -- ECIES is well-audited; `tweetnacl` or `libsodium` for impl

**Cons:** Issuer needs a server (but they already have one for KYC).

**Security chain:**
```
wallet_seed -> signMessage("solid:credential-channel:v1") -> SHA-256
  -> X25519_keypair -> ECDH with issuer ephemeral -> AES-256-GCM key
  -> decrypt credential -> re-encrypt for storage with vault key
```

**Verdict: This is the right answer for v1.** Score: 9/10.

### 1.3 My Recommendation

**Ship Option D (Hybrid ECIES + HTTPS) for v1.** It's the best balance of:
- Security (E2E encrypted, forward secrecy possible)
- Simplicity (deterministic key derivation, standard ECIES)
- Scalability (HTTPS infra, no on-chain blob storage)
- Recovery (wallet seed = master key for everything)

Add Option B (QR) as a secondary channel for in-person issuance in v2.

---

## 2. Credential Storage Architecture

### 2.1 Desktop (Browser / Electron)

```
Storage Location: IndexedDB (browser) or AppData/solid-protocol/ (Electron)

Encryption:
  wallet.signMessage("solid:vault:v1")
    -> SHA-256 -> vault_key (256-bit AES key)

Schema:
  credentials_db = {
    [schema_hash + issuer_pubkey]: {
      encrypted_blob: AES-256-GCM(vault_key, credential_json),
      iv: 12 bytes (unique per credential),
      metadata: {  // plaintext, for UI display
        schema_name: "basic_identity_v1",
        issuer_name: "Acme Hospital",
        expiration: 1735689600,
        issued_at: 1714502400,
        status: "active"  // updated on proof failure
      }
    }
  }
```

**Why IndexedDB?** It's the only browser storage API with enough capacity (>50MB), structured data support, and transactional semantics. LocalStorage is 5MB limited and synchronous.

**Why not SQLite?** For a pure browser extension (like Phantom), IndexedDB is the only option. For an Electron app, SQLite is fine but adds a native dependency.

### 2.2 Mobile

```
iOS:
  Storage: Keychain Services (kSecClassGenericPassword)
  Encryption: Hardware-backed AES-256 (Secure Enclave)
  Access: kSecAttrAccessibleWhenUnlockedThisDeviceOnly
  Biometric: Optional Face ID/Touch ID gate

Android:
  Storage: Android Keystore + EncryptedSharedPreferences
  Encryption: AES-256-GCM with Keystore-managed key
  Access: setUserAuthenticationRequired(true) for biometric
  Backup: Excluded from Android backup by default

Both platforms:
  BJJ master key: Stored in the secure enclave, NEVER exported
  Credentials: Encrypted blobs in app-private storage
  Recovery: wallet seed -> deterministic BJJ key + re-request credentials from issuers
```

### 2.3 Key Principle: Credentials Are Re-Requestable

Unlike a private key (which is irreplaceable), credentials can ALWAYS be re-issued by the original issuer. The holder proves identity to the issuer, the issuer re-signs the same credential, and the holder stores the new copy. This means:

- **Backup is nice-to-have, not critical** -- losing your phone loses credentials but not identity
- **Migration is straightforward** -- new device -> connect wallet -> re-request credentials
- **The BJJ master key IS the irreplaceable asset** -- and it's derived from the wallet seed

---

## 3. Product Build Strategy: 2 Apps, Not 4

### 3.1 Why 2, Not 4

You asked: should we build separate apps for DAO, Issuer, Holder, and Verifier?

**No. Build exactly 2 things:**

| App | Who Uses It | What It Does |
|-----|------------|-------------|
| **SolID Wallet** | Holders (end users) | Store credentials, generate proofs, manage identity |
| **SolID Console** | DAO members, Issuers, Verifiers | Role-switcher dashboard for all admin/operational flows |

**Why not 4?**
- DAO members, issuers, and verifiers are all **operators** -- they use a dashboard, not a wallet
- A single Console with a role-switcher (like AWS Console's service selector) is faster to build and easier to demo
- The holder experience is fundamentally different (mobile-first, wallet UX) and deserves its own app

### 3.2 SolID Wallet (Holder App)

**Target:** Browser extension (Chrome/Firefox) + mobile (React Native)

```
Screens:
  1. Onboarding
     - Connect existing Solana wallet (Phantom, Backpack)
     - Auto-derive BJJ identity from wallet
     - "Your identity is ready" confirmation

  2. Credentials
     - List of stored credentials (cards UI)
     - Each card shows: issuer, schema, expiration, status
     - Tap to see details (field names, not values for privacy)
     - "Add Credential" -> opens issuer portal or scans QR

  3. Proof Request
     - Incoming request notification
     - "Acme DeFi wants to verify: age >= 21"
     - [Approve] [Deny] buttons
     - Progress indicator during Groth16 proving (~2-3 sec)
     - Success/failure confirmation

  4. Settings
     - Backup reminder
     - Connected wallet info
     - Debug mode toggle
```

### 3.3 SolID Console (Unified Dashboard)

**Target:** Web app (Next.js 14) with responsive design

```
Layout:
  +--sidebar--+--main-content--+
  |           |                |
  | [DAO]     |  (role-       |
  | [Issuer]  |   specific    |
  | [Verifier]|   content)    |
  |           |                |
  +-----------+----------------+

Role: DAO
  - Registry overview (issuer count, schema count, tree stats)
  - Pending issuer applications (vote/approve/reject)
  - Active issuers (revoke, view stake, view activity)
  - VK rotation controls (request, monitor, commit)
  - Governance token management

Role: Issuer
  - Registration form (name, metadata, BJJ key generation)
  - Credential issuance form (select schema, fill fields, sign)
  - Issued credentials log (with commitment hashes)
  - Tree statistics (capacity used, depth)
  - Staking dashboard

Role: Verifier
  - Query builder UI (drag-and-drop predicate construction)
  - Verification log (proof submissions, nullifiers, timestamps)
  - Integration code generator (copy-paste SDK snippet)
  - Analytics (verification rate, unique holders, schema breakdown)
```

### 3.4 For the Demo

**One web page, one URL, three roles.** You open the Console, click "DAO" to bootstrap, click "Issuer" to issue a credential, then open the Wallet in another tab to prove it. The entire E2E flow is visible in 60 seconds.

---

## 4. Demo Landing Page Vision

### 4.1 Design Direction

**Inspiration:** Solana.com + Jito.network + Helius.dev -- dark theme, glassmorphism, smooth scroll animations, gradient accents.

```
Page Structure:

HERO SECTION (full viewport)
  - Animated particle field (identity nodes connecting/proving)
  - "Private Identity for Solana" (large, Inter font, gradient text)
  - "Prove who you are. Reveal nothing else." (subtitle)
  - [Launch App] [Read Docs] buttons
  - Live stats: "47 findings closed | 322K CU verify | <2s proof time"

HOW IT WORKS (scroll section)
  - 4-step animated flow:
    1. Issuer verifies you (KYC icon -> checkmark)
    2. Credential stored in your wallet (lock icon)
    3. dApp requests proof (query icon)
    4. ZK proof verified on-chain (Solana icon -> green check)
  - Each step animates on scroll-into-view

LIVE DEMO (interactive section)
  - Embedded role-switcher (DAO | Issuer | Holder | Verifier tabs)
  - "Try it yourself" -- connect wallet, issue test credential, prove it
  - Real on-chain transactions (devnet) with explorer links

ARCHITECTURE (technical section)
  - Animated diagram of 3 programs + circuit + SDK
  - Hover states reveal details
  - "Open Source" badge linking to GitHub

USE CASES (grid)
  - Age-gated DeFi
  - Accredited investor
  - Healthcare credentials
  - Supply chain provenance
  - Each card with icon + 2-line description

INTEGRATORS SECTION
  - "Built for Solana's ecosystem"
  - Logos of potential partners (Jupiter, Drift, Phantom -- "Coming Soon" badge)
  - Code snippet showing 5-line integration

FOOTER
  - Links: Docs | GitHub | Discord | Twitter
  - "Built with Groth16, BabyJubJub, and SPL Account Compression"
```

### 4.2 Name Consideration

"SolID" is good -- it's a play on "Solid" + "Solana ID". It's memorable, clean, and the protocol already uses it everywhere. Alternatives to consider:

| Name | Pros | Cons |
|------|------|------|
| **SolID** (keep) | Established, clean, pun works | Generic-sounding |
| **Veil** | Privacy-forward, short, memorable | Too abstract |
| **Attest** | Descriptive, action-oriented | Generic, taken in web2 |
| **Shroud** | Privacy metaphor, unique | Slightly negative connotation |

**My recommendation: Keep SolID.** It's already in all the code, docs, and artifacts. Changing now would be a documentation/code sweep for no meaningful gain. Just design a great logo.

---

## 5. Docs Site Strategy

### 5.1 When to Ship

**Ship the docs site BEFORE devnet launch, not after.** Here's why:

- Developers evaluate tools by reading docs FIRST, then trying the product
- A polished docs site signals credibility and maturity
- You need the docs site URL for your Solana Foundation grant application
- The docs content already exists in your `/docs/` directory -- it just needs formatting

### 5.2 Structure

```
docs.solid-protocol.io (or solid-docs.vercel.app)

Getting Started
  - What is SolID?
  - Quick Start (5-minute tutorial)
  - Architecture Overview

Guides
  - Verifier Integration (from integration-guide.md)
  - Issuer Onboarding (from issuer-guide.md)
  - Holder SDK (from key-management.md)
  - Schema Reference (from schemas.md)

Technical Reference
  - Circuit Design (from circuits.md)
  - Program APIs (auto-generated from Anchor IDL)
  - SDK API Reference (auto-generated from TypeScript)
  - Security Model (from SYSTEM_VISION.md)

Resources
  - Security Registry (from sec/SECURITY_REGISTRY.md)
  - Roadmap (from FORWARD_ROADMAP.md)
  - GitHub | Discord | Twitter
```

**Tool: Docusaurus or Nextra** (both Next.js-based, support MDX, have dark themes, deploy to Vercel in minutes).

---

## 6. Speed, Scalability & Cost -- Winning the Race

### 6.1 Current Performance

| Metric | Current | Target |
|--------|---------|--------|
| Proof generation (laptop) | ~2-3 sec | < 1 sec |
| Proof generation (mobile) | ~10-20 sec | < 5 sec |
| On-chain verification | 322K CU (~$0.001) | Same or lower |
| Credential issuance | 48K CU (~$0.0002) | Same |
| Issuer registration | 180K CU (~$0.0008) | Same |

### 6.2 Speed Optimization Path

```
Phase 1 (now): WASM prover
  - snarkjs WASM in browser/Node.js
  - ~2-3 sec on laptop, ~10-20 sec on phone
  - Good enough for v1

Phase 2 (post-devnet): Parallel witness computation
  - Web Workers for witness generation (parallelize field ops)
  - Target: 30-40% speedup -> ~1.5 sec on laptop

Phase 3 (post-mainnet): Native prover via WebGPU
  - GPU-accelerated multi-scalar multiplication
  - Target: < 500ms on laptop, < 3 sec on phone
  - Requires WebGPU support (Chrome 113+, Safari 18+)

Phase 4 (future): Delegated proving with TEE
  - Opt-in: holder sends encrypted witness to TEE prover
  - TEE generates proof, returns only proof bytes
  - Target: < 200ms regardless of device
  - Privacy: TEE attestation proves no data exfiltration
```

### 6.3 Cost Comparison

| Protocol | Verification Cost | Proof Time | Privacy |
|----------|------------------|------------|---------|
| **SolID** | ~$0.001 (322K CU) | 2-3 sec | Full ZK |
| Civic | ~$0.01 (token gate tx) | Instant (no proof) | None (reveal) |
| Privado ID (EVM) | ~$0.50-2.00 (gas) | 3-5 sec | Full ZK |
| Worldcoin | Free (L2) | N/A (biometric) | Partial |

**SolID is already 500x cheaper than Privado ID on EVM.** Solana's low fees are a structural advantage.

### 6.4 Scalability Targets

```
Current capacity:
  - Per-schema tree: 2^20 = 1M credentials (depth 20)
  - Issuer tree: 2^16 = 64K issuers (depth 16)
  - Global tree: 2^20 = 1M identity leaves (depth 20)
  - Nullifier PDAs: unlimited (one PDA per proof, rent-exempt)

Bottleneck analysis:
  - Global tree (1M) is the tightest limit for holders
  - At 1M users: upgrade to depth 26 (64M) or shard by region
  - Nullifier PDAs: ~0.001 SOL rent per proof -- at 1M proofs/day,
    ~1000 SOL/day in rent. Solution: periodic garbage collection
    of expired nullifier PDAs (after their schema's expiration window)

Cost at scale (1M verifications/day):
  - Verification CU: 322K * 1M = 322B CU/day = ~$1000/day in priority fees
  - Nullifier rent: ~$150/day (reclaimable)
  - Total: ~$1150/day = ~$420K/year
  - Revenue at $0.01/verify: $10K/day = $3.65M/year -> 8.7x margin
```

---

## 7. Immediate Next Steps (Priority Order)

1. **Deploy to devnet** -- all three programs + VK upload + bootstrap
2. **Build SolID Console** -- unified dashboard with role-switcher
3. **Build docs site** -- Docusaurus/Nextra, deploy to Vercel
4. **Design landing page** -- dark theme, animations, live demo embed
5. **Build SolID Wallet** -- browser extension first, then mobile
6. **Logo + branding** -- commission a designer or use AI generation
7. **Apply for Solana Foundation grant** -- with docs site URL + devnet demo link
8. **Implement Option D credential delivery** -- ECIES + HTTPS

---

*End of product strategy artifact. This document should be revisited after devnet launch to update timelines and priorities based on real-world feedback.*
