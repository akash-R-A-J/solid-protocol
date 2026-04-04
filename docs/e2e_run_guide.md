# SolID Protocol — End-to-End Run Guide

> Run the complete Private Onchain Identity system on Solana devnet.  
> **No local servers required.** Everything runs client-side — programs are on devnet, crypto runs in WASM/Node.js.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    NO LOCAL SERVERS NEEDED                       │
│                                                                 │
│  Your Machine (Node.js scripts or browser dapp)                 │
│  ├── @solid-protocol/core    → WASM crypto (Poseidon, BJJ)     │
│  ├── @solid-protocol/issuer  → Issue + compress credentials     │
│  ├── @solid-protocol/holder  → Build queries + generate proofs  │
│  ├── @solid-protocol/verifier→ Submit proofs on-chain           │
│  └── snarkjs                 → Groth16 proving (circuit WASM)   │
│                                                                 │
│  Talks directly to ↓                                            │
│                                                                 │
│  Solana Devnet (RPC: https://api.devnet.solana.com)             │
│  ├── schema_registry    (2oma2y...)                             │
│  ├── issuer_registry    (6ewriD...)                             │
│  └── zk_verifier        (FhtEvs...)                             │
│                                                                 │
│  Light Protocol (for compressed accounts / Merkle proofs)       │
│  └── Uses Helius RPC or public Photon indexer — no self-host    │
└─────────────────────────────────────────────────────────────────┘
```

> [!TIP]
> **Do I need to run anything locally?**
> **No.** The 3 Anchor programs are already deployed on devnet. The TS SDK and WASM module
> run in your Node.js process or browser. Solana RPC is public. Light Protocol's Photon
> indexer is available at `https://devnet.helius-rpc.com` or the default ZK compression endpoint.
> You just run scripts or build a simple dapp.

---

## Deployment Reference

| Program | Program ID | Explorer |
|---|---|---|
| `schema_registry` | `2oma2yU2vfSYNR8sPDq5JvRbrtetyk5wuuGzwGYupAsH` | [View](https://explorer.solana.com/address/2oma2yU2vfSYNR8sPDq5JvRbrtetyk5wuuGzwGYupAsH?cluster=devnet) |
| `zk_verifier` | `FhtEvsUwxRRT8nf3uaxK6iAefFqvqCYimc9vqnH5XEgr` | [View](https://explorer.solana.com/address/FhtEvsUwxRRT8nf3uaxK6iAefFqvqCYimc9vqnH5XEgr?cluster=devnet) |
| `issuer_registry` | `6ewriDVJLTaeBG7AsjuobAfyhDWfCz681RDWMFq5Laeo` | [View](https://explorer.solana.com/address/6ewriDVJLTaeBG7AsjuobAfyhDWfCz681RDWMFq5Laeo?cluster=devnet) |

**Authority:** `Hz4uJrCLqs9rqMHJyvD9tqWgSNc92TjsBYANUUNxLWWv`  
**Full manifest:** `deployments/devnet.json`

---

## Phase 1: Build Everything (Steps 1–8) ✅ COMPLETE

These steps are already done. Included for reproducibility.

### Step 1: Prerequisites

```bash
# Rust 1.94+ with WASM target
rustup update stable && rustup target add wasm32-unknown-unknown
cargo install wasm-pack

# Solana CLI 3.x + Anchor CLI 1.0
sh -c "$(curl -sSfL https://release.anza.xyz/stable/install)"
cargo install --git https://github.com/coral-xyz/anchor anchor-cli --locked

# Node.js 20+ (via nvm)
nvm install 20 && nvm use 20

# Circom 2.1+ and snarkjs
npm install -g snarkjs
# Build circom from source: https://github.com/iden3/circom

# Configure for devnet
solana config set --url devnet
solana airdrop 5
```

### Step 2: Rust Core Tests

```bash
cargo test -p solid-core   # 41 tests, 0 failures
```

### Step 3: Circuit Compilation

```bash
cd circuits && npm install
circom compound_query.circom --r1cs --wasm --sym --c --output build/ -l node_modules/circomlib/circuits
```

### Step 4: Trusted Setup

```bash
node scripts/setup.js
# Outputs: build/circuit_final.zkey + build/verification_key.json
```

### Step 5: WASM Build

```bash
cd wasm
wasm-pack build --target web --out-dir pkg
wasm-pack build --target nodejs --out-dir pkg-node
```

### Step 6: Anchor Build

```bash
cd solid-protocol
anchor keys sync && anchor build
# Builds: schema_registry.so, issuer_registry.so, zk_verifier.so
```

### Step 7: Deploy to Devnet

```bash
anchor deploy --provider.cluster devnet
```

### Step 8: TS SDK Build

```bash
cd ts-sdk
cd packages/core && npm link ../../../wasm/pkg && cd ../..
npm run build   # Builds all 5 packages
```

---

## Phase 2: On-Chain Initialization (Steps 9) ⬜ NEXT

> [!IMPORTANT]
> **This is the next step.** Create a single Node.js script that initializes all on-chain state.
> No servers needed — just `node scripts/initialize.ts`.

### Step 9: Initialize On-Chain State

Create `scripts/initialize.ts` at the project root:

```typescript
import { Connection, Keypair, PublicKey, SystemProgram } from '@solana/web3.js';
import * as anchor from '@coral-xyz/anchor';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

// ─── Config ────────────────────────────────────────────────────────────────
const PROGRAM_IDS = {
    schemaRegistry:  new PublicKey('2oma2yU2vfSYNR8sPDq5JvRbrtetyk5wuuGzwGYupAsH'),
    zkVerifier:      new PublicKey('FhtEvsUwxRRT8nf3uaxK6iAefFqvqCYimc9vqnH5XEgr'),
    issuerRegistry:  new PublicKey('6ewriDVJLTaeBG7AsjuobAfyhDWfCz681RDWMFq5Laeo'),
};

const RPC_URL = 'https://api.devnet.solana.com';

// ─── Load wallet ───────────────────────────────────────────────────────────
const keypairPath = path.join(os.homedir(), '.config/solana/id.json');
const secretKey = JSON.parse(fs.readFileSync(keypairPath, 'utf-8'));
const wallet = Keypair.fromSecretKey(Uint8Array.from(secretKey));
const connection = new Connection(RPC_URL, 'confirmed');

console.log('═══════════════════════════════════════');
console.log('  SolID Protocol — On-Chain Init');
console.log('═══════════════════════════════════════');
console.log(`Wallet: ${wallet.publicKey.toBase58()}`);
console.log(`Balance: ${await connection.getBalance(wallet.publicKey) / 1e9} SOL`);

// ─── Setup Anchor provider ────────────────────────────────────────────────
const provider = new anchor.AnchorProvider(
    connection,
    new anchor.Wallet(wallet),
    { commitment: 'confirmed' }
);
anchor.setProvider(provider);

// Load IDLs from target/idl/ (generated by anchor build)
const issuerIdl = JSON.parse(fs.readFileSync('target/idl/issuer_registry.json', 'utf-8'));
const schemaIdl = JSON.parse(fs.readFileSync('target/idl/schema_registry.json', 'utf-8'));
const zkIdl     = JSON.parse(fs.readFileSync('target/idl/zk_verifier.json', 'utf-8'));

const issuerProgram = new anchor.Program(issuerIdl, PROGRAM_IDS.issuerRegistry, provider);
const schemaProgram = new anchor.Program(schemaIdl, PROGRAM_IDS.schemaRegistry, provider);
const zkProgram     = new anchor.Program(zkIdl,     PROGRAM_IDS.zkVerifier,     provider);

// ─── Step 9a: Initialize Issuer Registry ───────────────────────────────────
console.log('\n[1/5] Initializing Issuer Registry...');
const [registryPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('registry-config')],
    PROGRAM_IDS.issuerRegistry
);
try {
    await issuerProgram.methods.initializeRegistry(
        new anchor.BN(1_000_000_000),  // min stake: 1 SOL
        new anchor.BN(86400),           // voting: 24h
        new anchor.BN(6000),            // approval: 60%
    ).accounts({
        registryConfig: registryPda,
        authority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
    }).rpc();
    console.log('   ✅ Issuer Registry initialized');
} catch (e: any) {
    if (e.message?.includes('already in use')) {
        console.log('   ⚠️  Already initialized (skipping)');
    } else throw e;
}

// ─── Step 9b: Register Schema ──────────────────────────────────────────────
console.log('\n[2/5] Registering basic_identity_v1 schema...');
const schemaName = 'basic_identity_v1';
const [schemaPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('schema'), Buffer.from(schemaName)],
    PROGRAM_IDS.schemaRegistry
);
try {
    await schemaProgram.methods.registerSchema(
        schemaName,
        1,                    // version
        'Identity',           // category
        ['age', 'country_code', 'region', 'id_type', 'verification_level', 'issued_date', 'nationality', '_reserved'],
        Array.from(new Uint8Array(32)),  // schema hash placeholder (compute via solid-core)
    ).accounts({
        schemaAccount: schemaPda,
        authority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
    }).rpc();
    console.log('   ✅ Schema registered');
} catch (e: any) {
    if (e.message?.includes('already in use')) {
        console.log('   ⚠️  Already registered (skipping)');
    } else throw e;
}

// ─── Step 9c: Initialize ZK Verifier ───────────────────────────────────────
console.log('\n[3/5] Initializing ZK Verifier...');
const [verifierConfigPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('verifier-config')],
    PROGRAM_IDS.zkVerifier
);
try {
    await zkProgram.methods.initialize()
        .accounts({
            verifierConfig: verifierConfigPda,
            authority: wallet.publicKey,
            systemProgram: SystemProgram.programId,
        }).rpc();
    console.log('   ✅ ZK Verifier config initialized');
} catch (e: any) {
    if (e.message?.includes('already in use')) {
        console.log('   ⚠️  Already initialized (skipping)');
    } else throw e;
}

// ─── Step 9d: Upload Verification Key ──────────────────────────────────────
console.log('\n[4/5] Uploading verification key...');
const [vkStoragePda] = PublicKey.findProgramAddressSync(
    [Buffer.from('vk-storage'), verifierConfigPda.toBuffer()],
    PROGRAM_IDS.zkVerifier
);

// Serialize VK from snarkjs JSON to the on-chain binary format expected by zk-verifier:
//   vk_alpha_g1 (64B) || vk_beta_g2 (128B) || vk_gamme_g2 (128B) ||
//   vk_delta_g2 (128B) || vk_ic (N * 64B)
//
// snarkjs exports G1 points as [x, y, "1"] (projective, string-encoded field elements)
// snarkjs exports G2 points as [[x0, x1], [y0, y1], ["1", "0"]] (projective)
// groth16-solana expects affine big-endian byte arrays: G1 = 64B, G2 = 128B
const vkJson = JSON.parse(fs.readFileSync('circuits/build/verification_key.json', 'utf-8'));

/** Convert a decimal string field element to 32-byte big-endian Uint8Array */
function fieldToBytes(s: string): Uint8Array {
    let n = BigInt(s);
    const bytes = new Uint8Array(32);
    for (let i = 31; i >= 0; i--) {
        bytes[i] = Number(n & 0xFFn);
        n >>= 8n;
    }
    return bytes;
}

/** Serialize a G1 point [x, y, z] to 64 bytes (big-endian affine x || y) */
function serializeG1(point: string[]): Uint8Array {
    const buf = new Uint8Array(64);
    buf.set(fieldToBytes(point[0]), 0);   // x: 32 bytes
    buf.set(fieldToBytes(point[1]), 32);  // y: 32 bytes
    return buf;
}

/** Serialize a G2 point [[x0,x1],[y0,y1],[z0,z1]] to 128 bytes (big-endian) */
function serializeG2(point: string[][]): Uint8Array {
    const buf = new Uint8Array(128);
    buf.set(fieldToBytes(point[0][0]), 0);   // x0: 32 bytes
    buf.set(fieldToBytes(point[0][1]), 32);  // x1: 32 bytes
    buf.set(fieldToBytes(point[1][0]), 64);  // y0: 32 bytes
    buf.set(fieldToBytes(point[1][1]), 96);  // y1: 32 bytes
    return buf;
}

const vkAlphaG1 = serializeG1(vkJson.vk_alpha_1);       // 64 bytes
const vkBetaG2  = serializeG2(vkJson.vk_beta_2);        // 128 bytes
const vkGammaG2 = serializeG2(vkJson.vk_gamma_2);       // 128 bytes
const vkDeltaG2 = serializeG2(vkJson.vk_delta_2);       // 128 bytes
const vkIc = Buffer.concat(vkJson.IC.map(serializeG1)); // N * 64 bytes

const vkBytes = Buffer.concat([vkAlphaG1, vkBetaG2, vkGammaG2, vkDeltaG2, vkIc]);
console.log(`   VK size: ${vkBytes.length} bytes (alpha:64 + beta:128 + gamma:128 + delta:128 + IC:${vkJson.IC.length}*64=${vkJson.IC.length * 64})`);

const CHUNK_SIZE = 900;
const chunks = Math.ceil(vkBytes.length / CHUNK_SIZE);
for (let i = 0; i < chunks; i++) {
    const chunk = vkBytes.subarray(i * CHUNK_SIZE, (i + 1) * CHUNK_SIZE);
    await zkProgram.methods.storeVerificationKey(
        i,                     // chunk_index
        Array.from(chunk),     // chunk_data
        i === chunks - 1,      // is_final_chunk
    ).accounts({
        verifierConfig: verifierConfigPda,
        vkStorage: vkStoragePda,
        authority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
    }).rpc();
}
console.log(`   ✅ VK stored: ${vkBytes.length} bytes in ${chunks} chunk(s)`);

// ─── Step 9e: Initialize Bloom Filter ──────────────────────────────────────
console.log('\n[5/5] Initializing Bloom filter nullifier registry...');
const [nullifierBloomPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('nullifier-bloom'), verifierConfigPda.toBuffer()],
    PROGRAM_IDS.zkVerifier
);
try {
    await zkProgram.methods.initNullifierBloom()
        .accounts({
            nullifierBloom: nullifierBloomPda,
            verifierConfig: verifierConfigPda,
            authority: wallet.publicKey,
            systemProgram: SystemProgram.programId,
        }).rpc();
    console.log('   ✅ Bloom filter initialized (32KB, ~100K capacity)');
} catch (e: any) {
    if (e.message?.includes('already in use')) {
        console.log('   ⚠️  Already initialized (skipping)');
    } else throw e;
}

console.log('\n═══════════════════════════════════════');
console.log('  ✅ All on-chain state initialized!');
console.log('═══════════════════════════════════════');
```

**Run it:**
```bash
npx ts-node scripts/initialize.ts
# Or: npx tsx scripts/initialize.ts
```

---

## Phase 3: E2E Identity Flow (Steps 10–13) ⬜ PENDING

> [!NOTE]
> These steps can be run as standalone Node.js scripts OR as part of a browser dapp.
> No local servers needed — all communication goes directly to Solana devnet RPC.

### Step 10: Issue a Credential

```typescript
import { initWasm, generateKeypair } from '@solid-protocol/core';
import { issueCredential } from '@solid-protocol/issuer';

await initWasm();

// Generate BJJ identities
const issuerKp = generateKeypair();
const holderKp = generateKeypair();

// Issue — this calls Light Protocol to compress the credential on-chain
const credential = await issueCredential(
    issuerKp.private_key,
    issuerKp.public_key_x,
    issuerKp.public_key_y,
    {
        schemaHash: schemaHash,
        attestationData: [21n, 840n, 1n, 0n, 2n, BigInt(Date.now()), 840n, 0n],
        holderPubKeyX: holderKp.public_key_x,
        holderPubKeyY: holderKp.public_key_y,
    },
    { payer: wallet },  // Pays for Light Protocol tree insertion (< 0.01 SOL)
);

console.log('Credential commitment:', Buffer.from(credential.commitment).toString('hex'));
```

### Step 11: Generate ZK Proof

```typescript
import { QueryBuilder } from '@solid-protocol/core';
import { generateProof } from '@solid-protocol/holder';

// Build compound query: age >= 21 AND country == US
const query = new QueryBuilder()
    .schema(schemaHash)
    .where(0, 'GTE', 21n)     // field[0] "age" >= 21
    .and(1, 'EQ', 840n)       // field[1] "country_code" == 840 (US)
    .nonce(verifierNonce)
    .build();

// Generate Groth16 proof (snarkjs + circuit WASM)
const result = await generateProof(
    query,
    { ...credential, holderPrivateKey: holderKp.private_key },
    {
        wasmPath: './circuits/build/compound_query_js/compound_query.wasm',
        zkeyPath: './circuits/build/circuit_final.zkey',
    },
);

console.log('Proof generated! Nullifier:', Buffer.from(result.nullifier).toString('hex'));
```

### Step 12: Verify On-Chain

```typescript
import { verifyOnChain } from '@solid-protocol/verifier';

const verification = await verifyOnChain(connection, wallet, PROGRAM_IDS.zkVerifier, {
    query,
    proofData: {
        proof_a: result.solanaProof.proofA,    // [u8; 64]
        proof_b: result.solanaProof.proofB,    // [u8; 128]
        proof_c: result.solanaProof.proofC,    // [u8; 64]
        publicInputs: result.publicSignals,     // [[u8; 32]; 21]
        nullifier: result.nullifier,            // [u8; 32]
    },
});

console.log('✅ Verified:', verification.verified);
console.log('TX:', verification.transactionSignature);
```

### Step 13: Replay Protection Test

```typescript
// Try same proof again — should fail!
try {
    await verifyOnChain(connection, wallet, PROGRAM_IDS.zkVerifier, {
        /* same proofData as step 12 */
    });
    console.error('❌ BUG: Replay should have been rejected!');
} catch (e) {
    console.log('✅ Replay correctly rejected:', e.message);
    // Expected: "This nullifier has already been used — proof replay detected"
}
```

---

## Running as a DApp (Alternative to Scripts)

Instead of scripts, you can wrap everything in a simple web app:

```bash
# Create a Vite app
npx -y create-vite@latest solid-demo --template vanilla-ts
cd solid-demo

# Install SDK
npm install @solana/web3.js @coral-xyz/anchor
npm link ../../wasm/pkg  # Link WASM

# Copy SDK packages
# Import and use exactly like the scripts above

npm run dev  # Opens at http://localhost:5173
```

The key difference: in a browser, use `wasm-pack --target web` output and load WASM with `initWasm()`. Everything else is identical.

---

## Project Structure

```
solid-protocol/
├── Cargo.toml                 # Rust workspace root
├── Anchor.toml                # Anchor configuration
├── deployments/
│   └── devnet.json            # 🔑 Program IDs, authority, PDA seeds
│
├── crates/
│   ├── solid-core/            # Core crypto (41 tests ✅)
│   └── solid-light/           # Light Protocol CPI helpers
│
├── programs/
│   ├── zk-verifier/           # Groth16 + Bloom nullifier (devnet ✅)
│   ├── issuer-registry/       # DAO governance (devnet ✅)
│   └── schema-registry/       # Credential schemas (devnet ✅)
│
├── circuits/
│   ├── compound_query.circom  # Main circuit (26,285 constraints)
│   ├── build/                 # R1CS, WASM, zkey, VK
│   └── scripts/setup.js       # Trusted setup
│
├── wasm/                      # solid-core → WASM (browser + Node.js)
│
├── ts-sdk/packages/
│   ├── core/                  # WASM loader + QueryBuilder DSL
│   ├── light/                 # Light Protocol compression
│   ├── issuer/                # Credential issuance
│   ├── holder/                # Proof generation
│   └── verifier/              # On-chain submission
│
├── schemas/                   # Pre-built vertical schemas (JSON)
└── docs/                      # Developer documentation
```

---

## Troubleshooting

| Issue | Fix |
|---|---|
| `cargo check` fails with `light-poseidon` version | Use `light-poseidon = "0.2"` exactly |
| `circom` not found | Add to PATH: `export PATH=$HOME/circom/target/release:$PATH` |
| `anchor build` fails with overflow | Add `overflow-checks = true` to `[profile.release]` |
| Devnet airdrop fails | Try `solana airdrop 2` or wait a few minutes |
| `init_if_needed` Bumps error | Add `features = ["init-if-needed"]` to anchor-lang in Cargo.toml |
| groth16 `Groth16Verifyingkey::new` not found | v0.2 uses struct literal, no `::new()` constructor |
| Nullifier already used | Expected — use a different verifier nonce for new sessions |

> [!TIP]
> **Two different WASMs in this project:**
> 1. `wasm/pkg/` — Rust crypto (Poseidon, BJJ). Built with `wasm-pack`. Used by TS SDK.
> 2. `circuits/build/compound_query_js/` — Circom witness generator. Built by `circom --wasm`. Used by `snarkjs`.
