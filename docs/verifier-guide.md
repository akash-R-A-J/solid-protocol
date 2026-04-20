# Verifier Guide

> How to integrate SolID proof verification into your dApp
> **Last refreshed:** 2026-04-20 for v0.2 (SPL Account Compression).

## Overview

As a verifier, you ask holders to prove they meet certain criteria
without revealing raw data. The entire verification is on-chain and
cryptographically guaranteed.

## Quick Setup

### 1. Install

```bash
npm install @solid-protocol/verifier @solid-protocol/core @solana/web3.js
```

### 2. Build a Query

Use the `QueryBuilder` DSL to define what you need:

```typescript
import { QueryBuilder } from '@solid-protocol/core';
import { generateVerifierNonce } from '@solid-protocol/verifier';

// "Prove you are 21+ and a US resident"
const query = new QueryBuilder()
  .schema(basicIdentitySchemaHash)
  .where(0, 'GTE', 21n)                               // age >= 21
  .and(1, 'EQ', 840n)                                 // country_code == US
  .nonce(generateVerifierNonce())                     // Fresh nonce per session
  .expiration(Math.floor(Date.now() / 1000) + 300)    // 5-minute window
  .build();
```

### 3. Request Proof from Holder

Send the query to the holder via your app's communication channel:

```typescript
// Option A: QR Code
const qrData = JSON.stringify({
  protocol: 'solid-v1',
  action: 'verify',
  query: serializeQuery(query),
  callbackUrl: 'https://your-app.com/api/solid-callback',
});

// Option B: WebSocket
ws.send(JSON.stringify({ type: 'verify-request', query }));

// Option C: Deep Link
const deepLink = `solid://verify?query=${encodeURIComponent(JSON.stringify(query))}`;
```

### 4. Receive and Verify Proof

```typescript
import { verifyOnChain, checkIssuerStatus } from '@solid-protocol/verifier';

async function handleProof(proofData) {
  // 1. Verify the issuer is DAO-approved.
  const issuerStatus = await checkIssuerStatus(
    connection,
    issuerRegistryProgramId,
    proofData.issuerAuthority,
  );

  if (!issuerStatus.approved) {
    throw new Error('Issuer not approved by DAO');
  }

  // 2. Submit proof to on-chain ZK verifier.
  //    The verifier reads the SPL-AC tree root through the
  //    schema-registry SchemaTreeBinding PDA that `query.schemaHash`
  //    points at, so the caller does not pass any backend-specific
  //    context.
  const result = await verifyOnChain(
    connection,
    payer,
    zkVerifierProgramId,
    {
      query,
      proofData: {
        proof_a: proofData.proofA,
        proof_b: proofData.proofB,
        proof_c: proofData.proofC,
        publicInputs: proofData.publicSignals,
        nullifier: proofData.nullifier,
      },
    },
  );

  return result; // { verified: true, nullifier, transactionSignature }
}
```

## Anti-Replay Protection

Each proof generates a **hardened 5-argument nullifier**:

```
nullifier = Poseidon(
  holderMasterPrivKey,
  revocationNonce,
  verifierAddress,
  queryContextHash,
  verifierNonce,
)
```

- **Same holder + same verifier + same query + same nonce** ⇒ same
  nullifier ⇒ rejected (replay).
- **Same holder + different nonce** ⇒ different nullifier ⇒ accepted
  (new session).
- **Different verifiers** ⇒ different nullifiers ⇒ cross-verifier linkage
  is impossible.

Under the hood, the `zk-verifier` program creates a **fresh PDA per
nullifier**: `(b"nullifier", nullifier_bytes)`. Replay attempts hit
`Account already in use`, which aborts the whole tx — there is no
Bloom-filter false-positive risk and no growing state to prune.

**Best practice:** Generate a fresh `verifierNonce` for each verification session.

## Example Use Cases

### Age-Gated Access

```typescript
const query = new QueryBuilder()
  .schema(basicIdentitySchemaHash)
  .where(0, 'GTE', 21n)  // age >= 21
  .nonce(generateVerifierNonce())
  .build();
```

### Vaccination Proof

```typescript
const query = new QueryBuilder()
  .schema(vaccinationSchemaHash)
  .where(0, 'EQ', 0n)       // vaccine_type == COVID
  .and(1, 'GTE', 2n)        // dose_number >= 2
  .nonce(generateVerifierNonce())
  .build();
```

### Supply Chain Compliance

```typescript
const query = new QueryBuilder()
  .schema(productCertSchemaHash)
  .where(4, 'GTE', 80n)     // compliance_score >= 80
  .and(6, 'EQ', 1n)         // organic == true
  .nonce(generateVerifierNonce())
  .build();
```

### DeFi KYC (Region Restriction)

```typescript
const query = new QueryBuilder()
  .schema(basicIdentitySchemaHash)
  .where(1, 'NE', 408n)     // country != North Korea
  .and(4, 'GTE', 2n)        // verification_level >= KYC
  .nonce(generateVerifierNonce())
  .build();
```

## Production hardening

- **Priority failover.** Wrap your `Connection` in
  `ResilientConnection` from `@solid-protocol/sdk` so a single-provider
  outage doesn't take verification offline.
- **Program-ID consistency.** Run `python3 scripts/check_program_ids.py`
  in CI. The canonical program IDs live in `Anchor.toml`; if
  `deployments/devnet.json` or `ts-sdk/.../config.ts` drift, the script
  fails and prints the reconciliation runbook.
- **Pause switch.** `zk-verifier` has a `paused` flag. If an incident is
  detected, the authority can flip it with `set_paused(true)` without
  touching nullifier PDAs.
