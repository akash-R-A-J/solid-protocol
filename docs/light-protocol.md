# Light Protocol Integration

> How SolID uses Light Protocol for compressed credential storage

## Why Light Protocol?

Without compression, every credential costs **~0.002 SOL** in account rent. With Light Protocol's ZK compressed state trees, it's **~0.00001 SOL** — a **200x reduction**.

For a system issuing millions of credentials, this is the difference between viability and bankruptcy.

## How It Works

### Credential Storage

```
Traditional PDA:
  Credential PDA → 0.002 SOL rent (each)
  1M credentials → 2,000 SOL (~$300,000)

Light Protocol Compressed:
  Merkle tree leaf → 0.00001 SOL (each)
  1M credentials → 10 SOL (~$1,500)
```

### Architecture

```
┌─────────────────────────────────────────┐
│           State Merkle Tree             │
│           (depth = 20)                  │
│        ~1,048,576 leaf slots            │
│                                         │
│   Each leaf = Poseidon commitment of:   │
│     dataHash + schemaHash +             │
│     holderPubX + holderPubY + salt      │
│                                         │
│   Stored compressed on-chain.           │
│   Queried via Photon Indexer.           │
└─────────────────────────────────────────┘
```

### Leaf Insertion (Issuance)

```typescript
import { insertCredentialLeaf } from '@solid-protocol/light';

// During credential issuance, the commitment is inserted as a leaf
const { txSignature, leafIndex } = await insertCredentialLeaf(
  rpc,
  payerKeypair,
  commitment,        // 32 bytes: Poseidon commitment
  schemaHash,        // 32 bytes: schema identifier
  issuerPubkey,      // Issuer's Solana pubkey
);
```

### Merkle Proof Fetching (Proof Generation)

```typescript
import { fetchMerkleProof } from '@solid-protocol/light';

// During proof generation, the holder fetches the Merkle path
const proof = await fetchMerkleProof(rpc, commitment);

// proof.root       → public input to circuit
// proof.siblings   → private input (Merkle path)
// proof.pathIndices → private input (left/right bits)
```

### Credential Revocation

```typescript
import { revokeCredential } from '@solid-protocol/light';

// Issuer or DAO can revoke by nullifying the leaf
await revokeCredential(rpc, issuerKeypair, commitment);

// After revocation:
// - Merkle proof for this credential will fail
// - Circuit verification will reject proofs using revoked credentials
```

## Configuration

### Devnet

```typescript
import { DEVNET_CONFIG, createLightRpc } from '@solid-protocol/light';

const rpc = createLightRpc({
  rpcEndpoint: 'https://api.devnet.solana.com',
  photonEndpoint: 'https://devnet.helius-rpc.com?api-key=YOUR_KEY',
  compressionEndpoint: 'https://devnet.helius-rpc.com?api-key=YOUR_KEY',
});
```

### Mainnet

```typescript
const rpc = createLightRpc({
  rpcEndpoint: 'https://api.mainnet-beta.solana.com',
  photonEndpoint: 'https://mainnet.helius-rpc.com?api-key=YOUR_KEY',
  compressionEndpoint: 'https://mainnet.helius-rpc.com?api-key=YOUR_KEY',
});
```

## On-Chain CPI (Rust)

For trustless verification, the on-chain programs CPI into Light Protocol directly:

```rust
use solid_light::cpi_helpers;
use solid_light::credential_tree::CompressedCredential;

// Build credential data for compressed account
let data = cpi_helpers::build_insert_credential_data(
    commitment,     // Poseidon commitment
    schema_hash,    // Schema identifier
    issuer_pubkey,  // Issuer Solana pubkey
)?;

// Verify state root matches on-chain tree
let is_valid = cpi_helpers::verify_state_root_matches(
    &tree_account_data,
    &merkle_root_from_proof,
);
```

**Why CPI matters:** Without on-chain CPI, Merkle root verification is client-trusted. With CPI, the verifier program reads the actual state root from Light Protocol's tree account, making the system trustless end-to-end.

## Dependencies

| Package | Language | Version | Purpose |
|---|---|---|---|
| `solid-light` | Rust | 0.1.0 | On-chain CPI helpers |
| `@lightprotocol/stateless.js` | TypeScript | ^0.17.0 | Client-side compressed account operations |
| `@lightprotocol/compressed-token` | TypeScript | ^0.17.0 | Token compression (future: staking) |

## Circuit Compatibility

The Merkle proof from Photon Indexer maps directly to the circuit's private inputs:

| Photon Output | Circuit Input | Type |
|---|---|---|
| `root` | `merkleRoot` | Public |
| `siblings[20]` | `merkleSiblings[20]` | Private |
| `pathIndices[20]` | `merklePathIndices[20]` | Private |
| `leaf` | `commitment` (computed in-circuit) | Private |
