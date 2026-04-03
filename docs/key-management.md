# Key Management

> BabyJubJub identity management — generation, encryption, and multi-device support

## Design Decision: Separate BJJ Keys

SolID uses **separate BabyJubJub (BJJ) keys** from your Solana wallet. This is intentional:

| | Solana Wallet | BJJ Identity |
|---|---|---|
| **Curve** | Ed25519 | BabyJubJub (Edwards on BN254) |
| **Purpose** | Pay gas, sign transactions | Identity proofs, credential ownership |
| **Rotation** | Rarely | Can rotate without changing wallet |
| **Privacy** | Public (on-chain) | Private (never revealed on-chain) |
| **Multi-device** | One per device | Same identity across devices |

## Identity Lifecycle

### 1. Generate Identity

```typescript
import { initWasm, generateIdentity } from '@solid-protocol/core';

await initWasm();

// Generates:
//   - Random BJJ private key (scalar in BJJ field)
//   - Corresponding public key (point on BJJ curve)
//   - Encrypted with Argon2id + AES-256-GCM
const identityJson = generateIdentity('your-strong-passphrase');

// Store securely
secureStorage.save('solid-identity', identityJson);
```

### 2. Unlock for Use

```typescript
import { unlockIdentity } from '@solid-protocol/core';

// Decrypt BJJ private key (runs Argon2id + AES-256-GCM)
const privateKey = unlockIdentity(identityJson, 'your-strong-passphrase');

// Use for signing/proving
// privateKey is 32 bytes — keep in memory only, never persist decrypted
```

### 3. Bind to Wallet

```typescript
// You can bind your BJJ identity to multiple Solana wallets
// This is metadata only — the BJJ key itself doesn't change
const identity = JSON.parse(identityJson);
identity.metadata.wallet_bindings.push(walletPubkeyBytes);
```

### 4. Export / Import (Cross-Device)

```typescript
// Export as JSON (encrypted — safe to transfer)
const exportedJson = identityJson;

// Import on another device
const importedIdentity = unlockIdentity(exportedJson, 'your-passphrase');
```

## Security Architecture

```
Passphrase ──→ Argon2id(65536 KB, 3 iters, 4 threads)
                    │
                    ▼
              256-bit AES key
                    │
                    ▼
              AES-256-GCM(nonce, bjj_private_key)
                    │
                    ▼
              Encrypted IdentityBundle (JSON)
              {
                version: 1,
                public_key: { x: [...], y: [...] },
                encrypted_private_key: {
                  ciphertext: [...],
                  nonce: [12 bytes],
                  salt: [32 bytes]
                },
                metadata: {
                  created_at: 1711929600,
                  wallet_bindings: [...],
                  rotated_from: null
                }
              }
```

## Rust API (solid-core)

```rust
use solid_core::babyjubjub::BJJIdentity;

// Generate
let identity = BJJIdentity::generate(b"passphrase")?;

// Unlock
let private_key = identity.unlock(b"passphrase")?;

// Bind wallet
identity.bind_wallet(solana_pubkey_bytes);

// Export / Import
let json = identity.export_json()?;
let recovered = BJJIdentity::import_json(&json)?;
```
