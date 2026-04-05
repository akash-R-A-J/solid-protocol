# Integration Guide

> How to integrate SolID Protocol into your Solana dApp

## Overview

SolID Protocol enables your dApp to verify user credentials privately. Users prove they meet requirements (age ≥ 21, country = US, etc.) without revealing raw data.

**You only need `@solid-protocol/verifier`** — the holder handles proof generation on their side.

---

## Quick Integration (Verifier Side)

### Install

```bash
npm install @solid-protocol/verifier @solid-protocol/core
```

### Create a Verification Query

```typescript
import { QueryBuilder } from '@solid-protocol/core';
import { verifyOnChain, generateVerifierNonce } from '@solid-protocol/verifier';

// Define what you need to verify
const query = new QueryBuilder()
  .schema(schemaHash)                 // Which credential schema
  .where(0, 'GTE', 21n)              // age >= 21
  .and(1, 'EQ', 840n)                // country_code == US (ISO 3166-1)
  .nonce(generateVerifierNonce())     // Fresh nonce (prevents replay)
  .globalRoot(globalRoot)            // PHASE 3.1: Anchor to global state
  .revocationNonce(0n)               // Identity revocation state
  .expiration(Date.now() / 1000 + 300) // 5 minute window
  .build();
```

### Send Query to Holder

```typescript
// Send via your app's communication channel (WebSocket, HTTP, QR code, etc.)
const verificationRequest = {
  query,
  verifierProgramId: 'YOUR_ZK_VERIFIER_PROGRAM_ID',
  callbackUrl: 'https://your-app.com/api/verify-callback',
};

// Display QR code or send via WebSocket
sendToHolder(verificationRequest);
```

### Receive and Verify Proof

```typescript
// Holder sends back the proof
app.post('/api/verify-callback', async (req, res) => {
  const { proof, publicSignals, nullifier } = req.body;

  // Submit to on-chain ZK verifier
  const result = await verifyOnChain(connection, payer, programId, {
    query,
    proofData: {
      proof_a: new Uint8Array(proof.proofA),
      proof_b: new Uint8Array(proof.proofB),
      proof_c: new Uint8Array(proof.proofC),
      publicInputs: publicSignals,
      nullifier: new Uint8Array(nullifier),
    },
    // Context for Phase 5.2 Root Verification
    lightProgramId: LIGHT_PROTOCOL_PROGRAM_ID,
  });

  if (result.verified) {
    // ✅ User meets requirements! Grant access.
    res.json({ status: 'approved', tx: result.transactionSignature });
  } else {
    res.json({ status: 'denied' });
  }
});
```

---

## Holder Integration

If you're building a wallet or identity app that holds user credentials:

### Install

```bash
npm install @solid-protocol/holder @solid-protocol/core @solid-protocol/light
```

### Generate Identity

```typescript
import { initWasm, generateIdentity, unlockIdentity } from '@solid-protocol/core';

await initWasm();

// Generate encrypted BJJ identity (user sets passphrase)
const identityJson = generateIdentity('user-passphrase');

// Store identityJson securely (e.g., IndexedDB, encrypted file)
localStorage.setItem('solid-identity', identityJson);

// Unlock when needed
const privateKey = unlockIdentity(identityJson, 'user-passphrase');
```

### Receive and Store Credentials

```typescript
// Credential comes from issuer after attestation
const credential = await receiveFromIssuer();

// Store locally (encrypted)
const credentials = getStoredCredentials();
credentials.push(credential);
saveCredentials(credentials);
```

### Generate Proof on Request

```typescript
import { generateProof } from '@solid-protocol/holder';

// When verifier sends a query
const result = await generateProof(
  query,
  credential,
  {
    wasmPath: '/circuits/compound_query.wasm',
    zkeyPath: '/circuits/circuit_final.zkey',
  },
);

// Send proof back to verifier
await sendToVerifier(result);
```

---

## Issuer Integration

If you're an issuing authority (hospital, government, university):

### Install

```bash
npm install @solid-protocol/issuer @solid-protocol/core @solid-protocol/light
```

### Register as Issuer (DAO)

```typescript
// 1. Register in issuer registry (stake SOL)
await program.methods.registerIssuer(
  'Acme Hospital',
  'https://acme-hospital.com/metadata.json',
  bjjPubKeyX,
  bjjPubKeyY,
).accounts({ ... }).rpc();

// 2. Wait for DAO voting period to pass
// 3. Community votes on your registration
// 4. After approval, you can issue credentials
```

### Issue Credentials

```typescript
import { issueCredential } from '@solid-protocol/issuer';

const credential = await issueCredential(
  issuerPrivateKey,
  issuerPubKeyX,
  issuerPubKeyY,
  {
    schemaHash: vaccineSchemaHash,
    attestationData: [1n, 3n, 1711929600n, 1n, 4821n, 0n, 840n, 32n],
    holderPubKeyX: holderPubX,
    holderPubKeyY: holderPubY,
  },
  { payer: issuerKeypair },
);

// Deliver to holder (encrypted channel)
await deliverToHolder(credential, holderWalletPubkey);
```

---

## Supported Schemas

| Schema | Fields | Use Case |
|---|---|---|
| `basic_identity_v1` | age, country, region, id_type, verification_level, issued_date, nationality | Hospitality, age-gating |
| `vaccination_v1` | vaccine_type, dose, date, authority, batch, expiry, country, age | Healthcare verification |
| `product_cert_v1` | category, cert_level, audit_date, auditor, score, region, organic, validity | Supply chain |

See [Schema Reference](schemas.md) for field details.

---

## Query Operators

| Operator | Code | Example |
|---|---|---|
| `NOOP` | 0 | Selective disclosure (reveal raw value) |
| `EQ` | 1 | `country_code == 840` |
| `NE` | 2 | `vaccine_type != 0` |
| `GT` | 3 | `age > 18` |
| `GTE` | 4 | `age >= 21` |
| `LT` | 5 | `dose_number < 5` |
| `LTE` | 6 | `compliance_score <= 100` |

Compound queries support up to **4 predicates** combined with **AND** or **OR** logic.
