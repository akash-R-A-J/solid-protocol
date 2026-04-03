# Issuer Guide

> How to become a trusted issuer in the SolID Protocol

## Overview

Issuers are entities that attest to user credentials — hospitals, universities, government agencies, etc. SolID uses a **DAO-governed trust registry** where issuers must:

1. **Register** — Provide identity details and BJJ public key
2. **Stake** — Lock SOL as collateral (minimum set by DAO)
3. **Get Approved** — Community votes on registration
4. **Issue Credentials** — Sign and compress attestations
5. **Maintain Good Standing** — Bad behavior → stake slashing

## Step-by-Step

### 1. Generate Issuer BJJ Identity

```bash
# Using the SDK (Node.js)
npx @solid-protocol/cli generate-identity \
  --passphrase "your-secure-passphrase" \
  --output issuer-identity.json
```

Or in code:

```typescript
import { initWasm, generateIdentity } from '@solid-protocol/core';

await initWasm();
const identity = generateIdentity('issuer-passphrase');
// Save identity.json securely — this is your signing key
```

### 2. Register in DAO Registry

```typescript
import { Connection, Keypair } from '@solana/web3.js';
import * as anchor from '@coral-xyz/anchor';

const program = new anchor.Program(issuerRegistryIdl, programId, provider);

// Parse identity for BJJ public key
const identity = JSON.parse(identityJson);

await program.methods.registerIssuer(
    'Acme Medical Center',                    // Name
    'https://acme-medical.com/solid.json',    // Metadata URI
    identity.public_key.x,                     // BJJ pub key X
    identity.public_key.y,                     // BJJ pub key Y
).accounts({
    registryConfig: registryConfigPda,
    issuerAccount: issuerPda,
    stakeVault: stakeVaultPda,
    issuerAuthority: wallet.publicKey,
}).rpc();

console.log('Registration submitted! Voting period started.');
```

### 3. Wait for DAO Approval

```
Voting period: 24 hours (configurable by DAO)
Required approval: 60% (configurable by DAO)
```

DAO members vote on your registration:

```typescript
// Any DAO token holder can vote
await program.methods.voteOnIssuer(
    true,           // approve
    new anchor.BN(100), // vote weight
).accounts({
    issuerAccount: issuerPda,
    voteRecord: votePda,
    voter: voterWallet.publicKey,
}).rpc();
```

### 4. Issue Credentials

Once approved, start issuing:

```typescript
import { issueCredential } from '@solid-protocol/issuer';
import { unlockIdentity } from '@solid-protocol/core';

// Unlock issuer BJJ key
const privateKey = unlockIdentity(identityJson, 'issuer-passphrase');

// Issue credential to a holder
const credential = await issueCredential(
    privateKey,
    identity.public_key.x,
    identity.public_key.y,
    {
        schemaHash: basicIdentitySchemaHash,
        attestationData: [
            21n,         // age
            840n,        // country (US)
            1n,          // region
            0n,          // id_type (Passport)
            3n,          // verification_level (InPerson)
            BigInt(Math.floor(Date.now() / 1000)), // issued_date
            840n,        // nationality
            0n,          // reserved
        ],
        holderPubKeyX: holderBjjPubKeyX,
        holderPubKeyY: holderBjjPubKeyY,
    },
    { payer: issuerKeypair },
);

// Deliver credential to holder securely
```

### 5. Handle Slashing

If the DAO determines you acted maliciously:

```
Offense examples:
- Issuing false credentials
- Backdating attestations
- Issuing to non-verified individuals

Penalty:
- Partial or full stake slashing
- If stake drops below minimum → automatic revocation
```

## Metadata URI Format

Your `metadata_uri` should point to a JSON file:

```json
{
  "name": "Acme Medical Center",
  "description": "Licensed healthcare provider since 1995",
  "website": "https://acme-medical.com",
  "logo": "https://acme-medical.com/logo.png",
  "credentials": ["vaccination_v1", "basic_identity_v1"],
  "jurisdiction": "US",
  "license_number": "MD-1234567",
  "contact": "solid@acme-medical.com"
}
```
