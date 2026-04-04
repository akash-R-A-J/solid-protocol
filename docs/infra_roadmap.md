# SolID Protocol — Infrastructure Roadmap (Finalized)

> From **85% real protocol** to **Solana's Identity Layer**

---

## ✅ Completed: Tier 1 + Tier 2

These items have been implemented and are now in the codebase.

### Tier 1.1 — Groth16 Verification Wired On-Chain ✅

`programs/zk-verifier/src/lib.rs` now uses `groth16_solana::Groth16Verifier` to:
- Accept proof bytes (π_A, π_B, π_C) and public inputs
- Negate G1 point (required by groth16-solana)
- Execute real `Groth16Verifier::verify()` using Solana's native `alt_bn128` syscalls
- Verification key stored in separate `VkStorage` PDA (10KB, chunked upload)

### Tier 1.2 — SAS CPI Types ✅

`crates/solid-core/src/sas.rs` provides:
- `SasSchema` and `SasAttestation` types matching SAS on-chain structures
- `SasAttestationBuilder` for constructing CPI instruction data
- SAS Program ID constant for devnet targeting

### Tier 1.3 — Light Protocol On-Chain CPI ✅

New crate `crates/solid-light/` with:
- `CompressedCredential` type for Merkle tree leaves
- `build_insert_credential_data()` for constructing CPI payloads
- `build_revoke_credential_data()` for credential revocation
- `verify_state_root_matches()` for on-chain root verification

### Tier 2.1 — Bloom Filter Nullifier Registry ✅

Replaced `Vec<[u8; 32]>` with a 32KB Bloom filter:
- **Capacity**: ~100K entries at <1% false positive rate
- **Lookup**: O(1) using double hashing (k=7)
- **Hash function**: Direct byte extraction from nullifier (already uniformly distributed)
- **Future path**: Light Protocol compressed nullifier tree (unlimited)

### Tier 2.2 — Account Size for Verification Key ✅

**Chosen approach: Separate `VkStorage` PDA**

Why this is best long-term:
- Allocates 10KB — enough for ~100 public inputs
- Supports chunked upload (VK can be uploaded in multiple txns)
- Decoupled from `VerifierConfig` lifecycle
- `realloc` was considered but adds complexity on every resize; separate PDA is cleaner

---

## Remaining: Tier 3 — Infra Differentiators

### 3.0 Identity State Anchoring (Architectural Foundation)

> [!IMPORTANT]
> This is the architectural shift that turns SolID from a credential proof system into an **identity state machine**. Must be designed before 3.1 (multi-credential proofs) since it changes the circuit architecture.

**The problem with our current design:**

```
Current: credential leaf → flat Merkle tree
         (no concept of "this holder's identity state")
```

Two vulnerabilities:
1. **Stale root replay** — holder can prove against old Merkle roots (before revocation)
2. **No identity cohesion** — credentials are independent objects, not part of a unified identity

**The correct model (Solana-adapted 2-layer):**

We use the Iden3/Polygon ID identity state concept but adapted for Solana's constraints:

```
Full Iden3 model (3 layers, expensive):
  claim ∈ claimsTree           ← depth 20
  claimsRoot ∈ identityState   ← depth 5
  identityState ∈ globalTree   ← depth 20
  Cost: ~18K extra constraints (nearly doubles circuit)

Our model (2 layers, Solana-optimized):
  claim ∈ perSchemaClaimsTree       ← depth 14 (~16K claims per schema)
  identityState ∈ globalStateTree   ← depth 20 (Light Protocol)
  Cost: ~10K extra constraints (~50% increase)
```

**Key design choices for Solana:**

| Decision | Iden3 | SolID (ours) | Why |
|---|---|---|---|
| **Trees** | Per-user (3 trees each) | Per-schema (shared) | 1M users × 3 trees = 3M trees is too expensive on Solana |
| **Revocation** | Full revocation tree | `revocationNonce` counter | Simpler, cheaper, nonce change invalidates all old proofs |
| **Identity state** | `Poseidon(claimsRoot, revocRoot, metaRoot)` | `Poseidon(claimsRoot, revocationNonce)` | 2 inputs vs 3, fewer constraints |
| **State anchoring** | L2 state | Light Protocol compressed account | Native Solana compression |

**Identity state hash:**

```
identityState = Poseidon(claimsTreeRoot, revocationNonce)
```

When a credential is revoked, `revocationNonce++` → `identityState` changes → all old proofs against the previous state are invalid automatically.

**Circuit addition (2 extra verification steps):**

```circom
// After existing Steps 1-4 (hash, commit, sign, Merkle inclusion in claims tree):

// Step 4b: Verify claims tree root is part of identity state
signal identityState <== Poseidon(2)([claimsTreeRoot, revocationNonce]);

// Step 4c: Verify identity state is in global state tree (Light Protocol)
// This is the same Merkle inclusion check we already have, but against the global tree
MerkleProof(GLOBAL_DEPTH)(identityState, globalRoot, globalSiblings, globalPathIndices);
```

**What this unlocks:**
1. **Automatic revocation** — nonce change invalidates all old proofs
2. **Identity portability** — migrate credentials by updating identity state
3. **Root freshness enforcement** — verifier program checks `globalRoot` matches on-chain Light tree

---

### 3.1 Multi-Credential Composable Proofs (The Killer Feature)

> [!IMPORTANT]
> This is what makes SolID a **$100M protocol**. Neither Polygon ID nor World ID supports cross-issuer composable proofs well.

**What it enables:**

```
One ZK proof that combines:
  Credential A (Hospital):    age >= 21
  Credential B (Bank):        credit_score >= 700
  Credential C (Government):  kyc_level >= 2
```

**Circuit design (with identity state anchoring):**

```circom
template CredentialSetQuery(N, CLAIMS_DEPTH, GLOBAL_DEPTH, NUM_FIELDS, MAX_PREDS) {
    // Per-credential inputs
    signal input attestationData[N][NUM_FIELDS];
    signal input salt[N];
    signal input schemaHash[N];
    signal input issuerPubKeyAx[N], issuerPubKeyAy[N];
    signal input issuerSigR8x[N], issuerSigR8y[N], issuerSigS[N];
    signal input claimsSiblings[N][CLAIMS_DEPTH];
    signal input claimsPathIndices[N][CLAIMS_DEPTH];

    // Identity state (shared across all credentials for this holder)
    signal input claimsTreeRoot[N];
    signal input revocationNonce;
    signal input globalSiblings[GLOBAL_DEPTH];
    signal input globalPathIndices[GLOBAL_DEPTH];
    signal input globalRoot; // Public input — verified against on-chain Light tree

    // Cross-credential predicates
    signal input credentialIndex[MAX_PREDS];
    signal input fieldIndex[MAX_PREDS];
    signal input operator[MAX_PREDS];
    signal input value[MAX_PREDS];

    // Shared holder identity
    signal input holderBJJPrivKey;
    signal input holderBJJPubKeyAx, holderBJJPubKeyAy;

    // Outputs
    signal output compositeNullifier;

    // Per-credential verification loop
    for (var i = 0; i < N; i++) {
        // 1. Hash attestation data → dataHash[i]
        // 2. Compute commitment[i]
        // 3. Verify issuer[i] signature
        // 4a. Verify claim ∈ claimsTree[i] (depth 14)
    }

    // 4b. Compute identity state
    // identityState = Poseidon(claimsTreeRoot[0], ..., claimsTreeRoot[N-1], revocationNonce)
    // 4c. Verify identityState ∈ globalStateTree (depth 20)

    // 5. Cross-credential predicate evaluation
    // 6. Composite nullifier = Poseidon(privKey, schema[0..N], nonce)
}
```

**Constraint estimates (with identity state anchoring):**

| N (credentials) | Constraints | Feasibility |
|---|---|---|
| 2 | ~50K | ✅ Single Solana tx |
| 3 | ~70K | ✅ Single tx (tight) |
| 4 | ~90K | ⚠️ Needs compute budget increase |

**Implementation steps:**
1. Design the 2-layer identity state model
2. Implement `CredentialSetQuery` circuit in Circom
3. Add `MultiCredentialQuery` and `IdentityState` types to `solid-core`
4. Update holder SDK for multi-credential witness generation
5. Update verifier program to check `globalRoot` against on-chain Light tree
6. Run new trusted setup for the larger circuit

---

### 3.2 Proof Aggregation

Batch multiple proof verifications into fewer transactions:

| Approach | CU Cost per Proof | Status |
|---|---|---|
| Individual verify | ~200K | ✅ Current |
| Random linear combination (batch) | ~50K amortized | Not started |
| Recursive SNARKs | O(1) per batch | Future R&D |

---

### 3.3 Server-Side Rust Prover

For enterprise/mobile — replace snarkjs with `ark-circom` + `ark-groth16`:

```
crates/solid-prover/
├── Cargo.toml     ← circom-compat, ark-groth16
└── src/
    ├── lib.rs
    ├── witness.rs  ← Circuit input → witness generation
    └── prove.rs    ← Groth16 proof (2-4 seconds vs 15-30s in snarkjs)
```

---

### 3.4 Developer UX — One-Call Integration

Target API:

```typescript
import { SolID } from '@solid-protocol/sdk';

// Verifier: ONE LINE
const result = await SolID.verify({
  schema: 'basic_identity_v1',
  conditions: [
    { field: 'age', op: 'GTE', value: 21 },
    { field: 'country', op: 'EQ', value: 840 },
  ],
  proof: proofBytesFromHolder,
});
```

Requires:
- Top-level `@solid-protocol/sdk` package re-exporting everything
- Auto-bundled circuit WASM + zkey (IPFS/CDN hosted)
- Lazy WASM initialization
- Developer-friendly error messages (no "circuit", "witness", or "Merkle")

---

### 3.5 Credential Discovery & Resolution

```typescript
const schemas = await SolID.listSchemas();           // Read schema-registry on-chain
const issuers = await SolID.getApprovedIssuers('vaccination_v1');
const hash = await SolID.resolveSchema('basic_identity_v1');
```

Requires an indexer or on-chain getProgramAccounts iteration helper.

---

### 3.6 Credential Revocation Registry

Two-tier approach:
- **Immediate**: Epoch-based revocation via `revocationNonce` (integrated with identity state anchoring)
- **Future**: Sparse revocation tree (proof of non-inclusion in revocation set)

---

## Remaining: Tier 4 — Ecosystem Integration

### 4.1 Wallet SDK (React)

```
@solid-protocol/wallet-adapter/
├── WalletIdentityProvider.tsx   ← React context
├── useIdentity.ts               ← BJJ key management
├── useCredentials.ts            ← Stored credentials
└── useProof.ts                  ← On-demand proof generation
```

### 4.2 Mobile SDK

WASM doesn't run natively on mobile. Options:
- `wasm2c` → C → static library
- Rust prover (`solid-prover`) via FFI bridge

### 4.3 Anchor CPI Helpers

```rust
// Other Solana programs can verify credentials inline:
solid_verifier::cpi::verify_proof(ctx, proof, inputs, nullifier)?;
```

Requires `cpi` feature flag (already defined in zk-verifier Cargo.toml).

---

## Implementation Schedule

```
Sprint 1 (Completed):
  [x] Tier 1.1 — Groth16 verification wired
  [x] Tier 1.2 — SAS CPI types
  [x] Tier 1.3 — Light Protocol CPI (solid-light crate)
  [x] Tier 2.1 — Bloom filter nullifiers
  [x] Tier 2.2 — Separate VK storage

Sprint 2 (Next):
  [ ] 3.0 — Identity state anchoring (2-layer model design)
  [ ] 3.4 — Developer UX top-level package
  [ ] 3.5 — Credential discovery helpers

Sprint 3:
  [ ] 3.1 — Multi-credential circuit (with identity state, Circom)
  [ ] 3.3 — Rust prover (ark-circom)
  [ ] 3.6 — Revocation via revocationNonce

Post-launch:
  [ ] 3.2 — Proof aggregation
  [ ] 4.x — Ecosystem integrations
```

---

## Current Score

| Area | Before | After Tier 1+2 | After All |
|---|---|---|---|
| **Architecture** | 9/10 | 9/10 | 10/10 |
| **ZK correctness** | 6/10 | 9/10 | 10/10 |
| **On-chain trust** | 4/10 | 8/10 | 10/10 |
| **Scalability** | 3/10 | 7/10 | 9/10 |
| **Developer UX** | 6/10 | 6/10 | 9/10 |
| **Ecosystem fit** | 8/10 | 8/10 | 10/10 |
| **Multi-credential** | 0/10 | 0/10 | 9/10 |
| **Identity state** | 0/10 | 0/10 | 9/10 |
| **Overall** | **70%** | **85%** | **96%** |

---

## Endgame Architecture

```
┌──────────────────────────────────────────────────────────┐
│                    Solana Wallets                         │
│              (Phantom, Backpack, Solflare)                │
└─────────────────────────┬────────────────────────────────┘
                          │
┌─────────────────────────▼────────────────────────────────┐
│                   SolID Protocol                          │
│                                                          │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌─────────────┐ │
│  │ Verifier │ │  Issuer  │ │  Holder  │ │  Discovery  │ │
│  │ Program  │ │ Registry │ │  Prover  │ │  Indexer    │ │
│  └──────────┘ └──────────┘ └──────────┘ └─────────────┘ │
│                                                          │
│  Identity State Anchoring (2-layer)                      │
│  claim → identityState → Light Protocol global tree      │
│                                                          │
│  One call: await SolID.verify(query, proof)              │
└─────────────────────────┬────────────────────────────────┘
                          │
┌─────────────────────────▼────────────────────────────────┐
│              Foundation (Existing Infra)                   │
│  ┌───────┐  ┌────────────────┐  ┌──────────────────────┐ │
│  │  SAS  │  │ Light Protocol │  │ groth16-solana        │ │
│  │       │  │ (compression)  │  │ (alt_bn128 syscalls)  │ │
│  └───────┘  └────────────────┘  └──────────────────────┘ │
└─────────────────────────┬────────────────────────────────┘
                          │
                      Solana L1
```

---

## Known Production Risks & Mitigations

> [!WARNING]
> These three risks are the gap between "working testnet" and "production infrastructure." Each must be addressed before mainnet launch.

### Risk 1: Photon Indexer Single Point of Failure

**Severity: HIGH** — Proof generation depends on Merkle proof fetching from the Photon Indexer. If Photon is down, holders cannot generate proofs.

**Current state:** TS SDK (`@solid-protocol/light`) hardcodes a single Photon endpoint per network.

**Mitigations:**

| Phase | Approach | Effort |
|---|---|---|
| **Immediate** | Configurable endpoint — let integrators point to their own Helius/Photon instance | Low (config change) |
| **Short-term** | Multi-indexer fallback — `LightRpc` tries N endpoints in priority order, auto-failover | Medium |
| **Long-term** | Light client — holder runs a local compressed state proof verifier, no indexer needed | High (R&D) |

**Code change needed:**
```typescript
// Current (fragile):
const rpc = createLightRpc({ photonEndpoint: SINGLE_URL });

// Target (resilient):
const rpc = createLightRpc({
  photonEndpoints: [PRIMARY, BACKUP_1, BACKUP_2],
  strategy: 'failover', // or 'round-robin'
  timeoutMs: 5000,
});
```

---

### Risk 2: Groth16 Trusted Setup

**Severity: CRITICAL for production** — Our current `scripts/setup.js` runs a single-party Powers-of-Tau ceremony. If the single party's toxic waste is compromised, an attacker can forge proofs.

**Current state:** Single-party setup. Acceptable for devnet/testing. **Unacceptable for mainnet.**

**Mitigations:**

| Phase | Approach | Status |
|---|---|---|
| **Devnet** | Single-party ceremony via `scripts/setup.js` | ✅ Done |
| **Testnet** | Use an existing community Powers-of-Tau (e.g., Hermez ceremony, 54 contributors) | Not started |
| **Mainnet** | Run SolID-specific multi-party ceremony (minimum 10 contributors, at least 1 trusted) | Not started |

**Multi-party ceremony plan:**
1. Use `snarkjs powersoftau` with community contributors
2. Each contributor adds entropy and publishes their contribution hash
3. Final beacon randomness applied
4. Phase 2 ceremony specific to our circuit
5. Verification key published on-chain and IPFS (immutable)

**Security guarantee:** As long as **at least one** contributor destroys their toxic waste, the ceremony is secure. This is why more contributors = better.

---

### Risk 3: Issuer Governance Model Scaling

**Severity: MEDIUM** — Our current DAO model (stake → vote → approve/reject) works for a small ecosystem. But real-world adoption requires differentiated trust tiers.

**Current state:** Single `IssuerStatus` enum: `Pending → Approved → Revoked`. All issuers are treated equally.

**Required for production:**

| Issuer Tier | Trust Source | Staking Requirement | Verification |
|---|---|---|---|
| **Community** | DAO vote | Standard (1 SOL) | Token-weighted voting |
| **Enterprise** | Business verification + DAO | Higher (10 SOL) | KYB + attestation from existing approved issuer |
| **Regulated** | Regulatory license | Lower (0.5 SOL) | License hash on-chain, DAO ratification |
| **Government** | Sovereignty | None (exempt) | Multi-sig of existing government issuers |

**Architecture change:**
```rust
#[derive(AnchorSerialize, AnchorDeserialize)]
pub enum IssuerTier {
    Community,            // Current model
    Enterprise {          // Business-verified
        kyb_attestation: Pubkey,  // SAS attestation from a regulated issuer
    },
    Regulated {           // Licensed entity
        license_hash: [u8; 32],
        jurisdiction: String,
    },
    Government {          // Sovereign entity
        multi_sig: Pubkey,
    },
}
```

**Why this matters:** A hospital issuing vaccination records needs a different trust level than a community member issuing reputation badges. Without tiers, verifiers can't distinguish between them, which limits real-world adoption by enterprises and governments.

---
