# Verifier Guide

> How to integrate SolID proof verification into your dApp

## Overview

As a verifier, you ask holders to prove they meet certain criteria without revealing raw data. The entire verification is on-chain and cryptographically guaranteed.

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
  .where(0, 'GTE', 21n)       // age >= 21
  .and(1, 'EQ', 840n)          // country_code == US
  .nonce(generateVerifierNonce()) // Fresh nonce per session
  .expiration(Math.floor(Date.now() / 1000) + 300) // 5 min window
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
  // 1. Verify the issuer is DAO-approved
  const issuerStatus = await checkIssuerStatus(
    connection,
    issuerRegistryProgramId,
    proofData.issuerAuthority,
  );

  if (!issuerStatus.approved) {
    throw new Error('Issuer not approved by DAO');
  }

  // 2. Submit proof to on-chain ZK verifier
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

Each proof generates a **nullifier** = `Poseidon(holderPrivKey, schemaHash, verifierNonce)`.

- **Same holder + same nonce** = same nullifier → rejected (replay)
- **Same holder + different nonce** = different nullifier → accepted (new session)
- **Different holders** = different nullifiers → can't link across verifiers

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
