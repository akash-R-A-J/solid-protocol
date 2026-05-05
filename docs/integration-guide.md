# Integration Guide

> How to integrate SolID Protocol into your Solana dApp
> **Last refreshed:** 2026-04-20 for v0.2 (SPL Account Compression).

## Overview

SolID Protocol enables your dApp to verify user credentials privately.
Users prove they meet requirements (age ≥ 21, country = US, etc.) without
revealing raw data.

**You only need `@solid-protocol/verifier`** — the holder handles proof
generation on their side.

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
  .schema(schemaHash)                  // Which credential schema
  .where(0, 'GTE', 21n)                // age >= 21
  .and(1, 'EQ', 840n)                  // country_code == US (ISO 3166-1)
  .nonce(generateVerifierNonce())      // Fresh nonce (prevents replay)
  .globalRoot(globalRoot)              // Current global-state-tree root
  .revocationNonce(0n)                 // Identity revocation state
  .expiration(Date.now() / 1000 + 300) // 5-minute window
  .build();
```

### Send Query to Holder

```typescript
// Send via your app's communication channel (WebSocket, HTTP, QR code, etc.)
const verificationRequest = {
  query,
  verifierProgramId: 'DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb', // zk-verifier
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

  const result = await verifyOnChain(connection, payer, programId, {
    query,
    proofData: {
      proof_a: new Uint8Array(proof.proofA),
      proof_b: new Uint8Array(proof.proofB),
      proof_c: new Uint8Array(proof.proofC),
      publicInputs: publicSignals,
      nullifier: new Uint8Array(nullifier),
    },
    // The verifier reads the SPL-AC tree root indirectly, through the
    // schema-registry SchemaTreeBinding PDA that `query.schemaHash`
    // points at. No Light Protocol context is required any more.
  });

  if (result.verified) {
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

// Store identityJson securely (e.g. IndexedDB, encrypted file)
localStorage.setItem('solid-identity', identityJson);

// Unlock when needed
const privateKey = unlockIdentity(identityJson, 'user-passphrase');
```

### Receive and Store Credentials

Credentials now carry their tree address so the holder can prove
against multiple schema-scoped trees simultaneously in a batch proof:

```typescript
import type { StoredCredential } from '@solid-protocol/holder';
import { PublicKey } from '@solana/web3.js';

const credential: StoredCredential = {
  schemaHash,
  attestationData: [21n, 840n, /* ... */],
  issuerSignature: { r8_x, r8_y, s },
  issuerPubKeyX,
  issuerPubKeyY,
  holderPubKeyX,
  holderPubKeyY,
  holderPrivateKey,
  salt,
  commitment,
  expirationTimestamp: 1735689600,
  merkleTree: new PublicKey(treePubkeyBase58), // <-- NEW in v0.2
};

saveCredential(credential);
```

### Generate Proof on Request

```typescript
import { generateProof } from '@solid-protocol/holder';
import { LocalReplicaAdapter } from '@solid-protocol/light';
import { Connection } from '@solana/web3.js';

const connection = new Connection('https://api.devnet.solana.com', 'confirmed');
const merkleProofAdapter = new LocalReplicaAdapter(connection);

const result = await generateProof(
  query,
  credential,
  {
    wasmPath: '/circuits/compound_query.wasm',
    zkeyPath: '/circuits/circuit_final.zkey',
  },
  { merkleProofAdapter },
);

await sendToVerifier(result);
```

For **batch proofs** across multiple schemas:

```typescript
import { generateBatchProof } from '@solid-protocol/holder';
import { deriveGlobalBinding } from '@solid-protocol/light';

// The global-state tree address is the one bound in schema-registry.
// Read it once per session from the GlobalStateBinding PDA.
const [globalBinding] = deriveGlobalBinding();

const batch = await generateBatchProof(
  query,
  credentials,                // up to 4, padded with zero placeholders
  masterPrivateKey,
  masterPublicKey,
  revocationNonce,
  { wasmPath, zkeyPath },
  { merkleProofAdapter, globalStateTree: globalStateTreeAddress },
);
```

---

## Issuer Integration

If you're an issuing authority (hospital, government, university):

### Install

```bash
npm install @solid-protocol/issuer @solid-protocol/light @solid-protocol/core
```

### Register as Issuer (DAO)

```typescript
await program.methods.registerIssuer(
  'Acme Hospital',
  'https://acme-hospital.com/metadata.json',
  bjjPubKeyX,
  bjjPubKeyY,
).accounts({ ... }).rpc();

// Wait for the DAO voting period to pass, then:
// await program.methods.finalizeVoting().accounts({ ... }).rpc();
// => IssuerApproved event
```

### Create a Merkle Tree for Your Schema (one-time)

Each schema that an issuer writes under needs an SPL Account Compression
tree **once**, bound to the schema by `schema-registry::initialize_tree_binding`.

```typescript
import { createCredentialTree } from '@solid-protocol/light';

const { treePubkey } = await createCredentialTree({
  connection,
  payer: issuerKeypair,
  schemaHash,
  maxDepth: 14,        // 16 384 credentials per tree
  maxBufferSize: 64,
});

// schema-registry::initialize_tree_binding(schemaHash, treePubkey)
// (authority = DAO or schema owner)
```

### Issue Credentials

```typescript
import { issueCredential } from '@solid-protocol/issuer';

await issueCredential({
  connection,
  issuerAuthority: issuerKeypair,
  schemaHash,
  commitment,          // 32-byte Poseidon commitment (from @solid-protocol/core)
  merkleTree: treePubkey,
});

// Under the hood this calls issuer-registry::issue_credential, which CPIs
// to spl-account-compression::append under an issuer-registry-owned
// `tree-authority` PDA.  Emits a CredentialIssued event.
```

---

## Supported Schemas

| Schema | Fields | Use Case |
|---|---|---|
| `basic_identity_v1` | age, country, region, id_type, verification_level, issued_date, nationality | Hospitality, age-gating |
| `vaccination_v1` | vaccine_type, dose, date, authority, batch, expiry, country, age | Healthcare verification |
| `product_cert_v1` | category, cert_level, audit_date, auditor, score, region, organic, validity | Supply chain |
| `dao_membership_v1` | dao_id, membership_tier, joined_at, voting_power_band, contribution_score, role_code, valid_until | DAO and community access |
| `accredited_investor_v1` | jurisdiction, accreditation_level, income_band, net_worth_band, professional_status, verification_date, valid_until | Private finance and gated investment flows |

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

Compound queries support up to **4 predicates** combined with **AND** or
**OR** logic.
