# Issuer Guide

> How to become a trusted issuer in the SolID Protocol
> **Last refreshed:** 2026-04-20 for v0.2 (SPL Account Compression).

## Overview

Issuers are entities that attest to user credentials — hospitals,
universities, government agencies, etc. SolID uses a **DAO-governed
trust registry** where issuers must:

1. **Register** — Provide identity details and BJJ public key.
2. **Stake** — Lock SOL as collateral (minimum set by DAO).
3. **Get Approved** — Community votes on registration.
4. **Create / Bind a Tree** — One concurrent Merkle tree per schema the
   issuer writes under.
5. **Issue Credentials** — Sign with BJJ EdDSA and call `issue_credential`
   (CPIs to SPL Account Compression under a protocol-owned
   `tree-authority` PDA).
6. **Maintain Good Standing** — Bad behavior → stake slashing.

## Step-by-Step

### 1. Generate Issuer BJJ Identity

```bash
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

const identity = JSON.parse(identityJson);

await program.methods.registerIssuer(
    'Acme Medical Center',
    'https://acme-medical.com/solid.json',
    identity.public_key.x,                     // BJJ pub key X
    identity.public_key.y,                     // BJJ pub key Y
).accounts({
    registryConfig: registryConfigPda,
    issuerAccount: issuerPda,
    stakeVault: stakeVaultPda,
    issuerAuthority: wallet.publicKey,
}).rpc();
```

### 3. Wait for DAO Approval

```
Voting period: 24 hours (configurable by DAO)
Required approval: 60% (configurable by DAO)
Stake maturity before vote-counts: 100 slots (flash-loan guard)
```

```typescript
await program.methods.voteOnIssuer(
    true,                      // approve
    new anchor.BN(100),        // vote weight
).accounts({
    issuerAccount: issuerPda,
    voteRecord: votePda,
    voter: voterWallet.publicKey,
}).rpc();
```

Once the voting window closes, anyone can finalize:

```typescript
await program.methods.finalizeVoting().accounts({ ... }).rpc();
// => IssuerApproved { issuer, slot, total_for, total_against }
```

### 4. Create & Bind a Schema Tree (one-time per schema)

Each schema writes into one concurrent Merkle tree whose authority is
the `issuer-registry` `tree-authority` PDA — so only `issue_credential`
can append, not the issuer's wallet directly.

```typescript
import { createCredentialTree } from '@solid-protocol/light';

const { treePubkey } = await createCredentialTree({
  connection,
  payer: issuerKeypair,
  schemaHash,
  maxDepth: 14,        // 2^14 = 16 384 credentials
  maxBufferSize: 64,
});

// Then bind the tree to the schema via schema-registry.  The authority
// here is typically the schema owner or the DAO.
await schemaRegistry.methods.initializeTreeBinding(
  Array.from(schemaHash),
  treePubkey,
).accounts({ ... }).rpc();
```

### 5. Issue Credentials

Once approved and the tree is bound, issuance is a single call that both
signs and writes on-chain in the same transaction context:

```typescript
import { issueCredential } from '@solid-protocol/issuer';
import { unlockIdentity, computeCommitment } from '@solid-protocol/core';

const privateKey = unlockIdentity(identityJson, 'issuer-passphrase');

// 1. Compute the Poseidon commitment for this holder's attestation.
const commitment = computeCommitment({
  schemaHash,
  attestationData: [
    21n,                                         // age
    840n,                                        // country (US)
    1n,                                          // region
    0n,                                          // id_type
    3n,                                          // verification_level
    BigInt(Math.floor(Date.now() / 1000)),       // issued_date
    840n,                                        // nationality
    0n,                                          // reserved
  ],
  holderPubKeyX,
  holderPubKeyY,
  salt,
  issuerPubKeyX: identity.public_key.x,
  issuerPubKeyY: identity.public_key.y,
  issuerPrivateKey: privateKey,
});

// 2. Submit the on-chain CPI that appends `commitment` to the tree.
await issueCredential({
  connection,
  issuerAuthority: issuerKeypair,
  schemaHash,
  commitment,
  merkleTree: treePubkey,
});

// 3. Hand the full StoredCredential (including `merkleTree`) to the
//    holder via your secure delivery channel.  The holder now has
//    everything they need to generate ZK proofs.
```

### 6. Keep the `SchemaTreeBinding` root fresh (indexer path)

`issue_credential` emits a `CredentialIssued` event. A lightweight
indexer listens, reads `getCurrentTreeRoot`, and calls
`schema-registry::update_tree_root` so the verifier's on-chain trust
anchor matches the tree's current state:

```typescript
import { getCurrentTreeRoot, parseCredentialIssuedEvent } from '@solid-protocol/light';

// Simplified skeleton
connection.onLogs(issuerRegistryProgramId, async (logs) => {
  const evt = parseCredentialIssuedEvent(logs.logs);
  if (!evt) return;
  const root = await getCurrentTreeRoot(connection, evt.merkleTree);
  await schemaRegistry.methods.updateTreeRoot(evt.schemaHash, root)
    .accounts({ ... })
    .rpc();
});
```

In production you can batch this across many appends to keep write
costs amortized.

### 7. Handle Slashing

If the DAO determines you acted maliciously:

```
Offense examples:
  - Issuing false credentials
  - Backdating attestations
  - Issuing to non-verified individuals

Penalty:
  - Partial or full stake slashing (authority-gated, ≤ 4 KB evidence)
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
