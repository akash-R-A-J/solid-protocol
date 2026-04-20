# State Compression (SPL Account Compression)

> **v0.2 — April 2026.** This document was previously titled
> *"Light Protocol Integration"*. Starting with v0.2 the compressed-state
> backend is **SPL Account Compression**. The filename is preserved so
> existing links still work; the content below is the current source of
> truth.
>
> The core thesis is unchanged: credentials are stored as leaves in a
> compressed Merkle tree so per-credential rent is negligible compared to
> full PDA storage. Only the program that owns the tree and the indexer
> you read it from have changed.

## Why compressed state?

Without compression every credential would own an account (~0.002 SOL of
rent). For a system issuing millions of credentials that is
~$300k of locked rent. With SPL Account Compression each credential is a
single leaf in a concurrent Merkle tree — storage is **amortized** across
the tree's fixed allocation, which we configure per schema.

## Why SPL Account Compression (and not Light Protocol)?

| Criterion | SPL Account Compression | Light Protocol stateless |
|---|---|---|
| Program maintainer | Solana Foundation / Solana Labs | Light Protocol Labs |
| Indexer required to read root | No — any RPC can parse `ConcurrentMerkleTreeAccount` | Yes (Photon) |
| Battle-tested by | Metaplex Bubblegum, millions of cNFTs | Smaller, newer deployment surface |
| Client library | `@solana/spl-account-compression` | `@lightprotocol/stateless.js` |
| Failure mode if indexer is down | Reads still work against plain RPC | Photon is a hard dependency for proof fetch |

The verifier is **backend-agnostic** (it reads roots through
`schema-registry::SchemaTreeBinding` PDAs), so this migration changed no
circuit constraints, no public inputs, and no nullifier derivation —
only the owner of the tree and the adapter used to fetch Merkle proofs.

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│          Per-schema concurrent Merkle tree                   │
│          Owned by: SPL Account Compression program           │
│          Authority: issuer-registry tree-authority PDA       │
│          Depth / buffer size: configurable per schema        │
│                                                              │
│  Each leaf = Poseidon commitment of:                         │
│     dataHash · schemaHash · holderPubX · holderPubY · salt   │
│                                                              │
│  Root is pinned into schema-registry::SchemaTreeBinding PDA  │
│  (discriminator "schmtree"), which is what the verifier      │
│  trusts on-chain.                                            │
└──────────────────────────────────────────────────────────────┘
```

The **global-state tree** (used by the identity-cohesion step in the
batch circuit) lives in a separate SPL AC tree whose root is pinned into
`schema-registry::GlobalStateBinding` PDA (discriminator `globroot`).

## Issuer: creating a tree

Before an issuer can write credentials under a schema, they create the
concurrent tree and bind it to the schema.

```typescript
import { Connection, Keypair } from '@solana/web3.js';
import {
  createCredentialTree,
  deriveSchemaTreeBinding,
} from '@solid-protocol/light';

const connection = new Connection('https://api.devnet.solana.com', 'confirmed');

const { treePubkey, signature } = await createCredentialTree({
  connection,
  payer: issuerKeypair,
  schemaHash,             // 32 bytes
  maxDepth: 14,           // 2^14 = 16 384 credentials per schema
  maxBufferSize: 64,      // concurrent writes in flight
});

// Then bind the tree to the schema (authority = issuer / DAO):
// schema-registry::initialize_tree_binding(schemaHash, treePubkey)
```

`createCredentialTree` sets the tree's `authority` to the
`issuer-registry`-owned `tree-authority` PDA, so only `issue_credential`
can append — never the issuer's wallet directly. This makes the issuer
trust boundary enforceable purely by PDA, not by off-chain policy.

## Issuer: appending a credential

```typescript
import { issueCredential } from '@solid-protocol/issuer';

await issueCredential({
  connection,
  issuerAuthority: issuerKeypair,
  schemaHash,
  commitment,               // 32-byte Poseidon commitment
  merkleTree: treePubkey,   // from createCredentialTree
});
```

Under the hood this sends `issuer-registry::issue_credential`, which:

1. Checks the issuer is `Approved` in the registry.
2. Derives the `tree-authority` PDA from `(b"tree-authority", schema_hash)`.
3. CPIs into `spl-account-compression::append` with that PDA as signer.
4. Increments `issuer.credentials_issued`.
5. Emits a `CredentialIssued` event (consumed by indexers / the
   `schema-registry::update_tree_root` path to refresh the binding).

## Holder: fetching a Merkle proof

Proof fetching is decoupled via the `MerkleProofAdapter` interface so the
same holder SDK works against a local validator in E2E tests, a custom
indexer, or a DAS endpoint in production.

```typescript
import {
  fetchMerkleProof,
  LocalReplicaAdapter,
  type MerkleProofAdapter,
} from '@solid-protocol/light';

// Test / localnet — reconstructs the tree in-memory from on-chain state.
const adapter: MerkleProofAdapter = new LocalReplicaAdapter(connection);

// Production — plug in your DAS-backed or custom indexer adapter.
// const adapter = new HeliusDasAdapter(rpcEndpoint, apiKey);

const { root, siblings, pathIndices, leafIndex } = await fetchMerkleProof(
  adapter,
  merkleTree,   // tree pubkey
  commitment,   // 32-byte leaf
);
```

The returned structure maps 1:1 onto the circuit's Merkle inputs:

| Adapter Output | Circuit Input | Type |
|---|---|---|
| `root` | `merkleRoot` | public |
| `siblings[d]` | `merkleSiblings[d]` | private |
| `pathIndices[d]` | `merklePathIndices[d]` | private |
| `leafIndex` | (context only) | — |

## Reading the current root directly

If you need the current root without a proof (e.g. to refresh a
`SchemaTreeBinding` after a batch of appends), go straight through
`@solana/spl-account-compression`:

```typescript
import { getCurrentTreeRoot } from '@solid-protocol/light';

const root: Uint8Array = await getCurrentTreeRoot(connection, merkleTree);
// 32 bytes, big-endian, suitable for schema-registry::update_tree_root.
```

## Revocation

v1 uses the **rotate-identity** model: revoking a credential bumps
`revocationNonce`, which changes `identity_leaf` in the global-state
tree, which changes the global-tree root, which invalidates every
outstanding nullifier for that holder.

v1.1 will add a **Sparse Merkle Tree non-membership check** in the
circuit. See [`REVOCATION_DESIGN.md`](./REVOCATION_DESIGN.md).

## Configuration

### Devnet / localnet

```typescript
import { Connection } from '@solana/web3.js';

const connection = new Connection('https://api.devnet.solana.com', 'confirmed');
```

### Production (with failover)

```typescript
import { ResilientConnection } from '@solid-protocol/sdk';

const rpc = new ResilientConnection(
  [
    'https://api.mainnet-beta.solana.com',
    'https://mainnet.helius-rpc.com?api-key=YOUR_KEY',
    'https://your-fallback-endpoint.com',
  ],
  'confirmed',
);

// Use `rpc.connection` for one-shot calls, `rpc.call(fn)` for failover-guarded calls.
```

## On-Chain CPI (Rust)

The verifier trusts the root via `schema-registry::SchemaTreeBinding`
parsing — there is no direct CPI into SPL Account Compression from the
verifier. Writes happen in `issuer-registry::issue_credential`, which
does the SPL-AC `append` CPI with a manually derived discriminator to
avoid adding `spl-account-compression` to the dependency graph:

```rust
// programs/issuer-registry/src/lib.rs (excerpt)
let (tree_authority, bump) = Pubkey::find_program_address(
    &[b"tree-authority", schema_hash.as_ref()],
    ctx.program_id,
);

let ix = Instruction {
    program_id: SPL_AC_PROGRAM_ID,
    accounts: vec![
        AccountMeta::new(ctx.accounts.merkle_tree.key(), false),
        AccountMeta::new_readonly(tree_authority, true),  // signer via PDA
        AccountMeta::new_readonly(ctx.accounts.noop_program.key(), false),
    ],
    data: [SPL_AC_APPEND_DISCRIMINATOR, commitment.as_ref()].concat(),
};

invoke_signed(&ix, &[...], &[&[b"tree-authority", schema_hash.as_ref(), &[bump]]])?;
```

And the verifier reads roots through `solid-light::cpi_helpers`:

```rust
use solid_light::cpi_helpers;

let root_ok = cpi_helpers::verify_schema_root_binding(
    &ctx.accounts.schema_tree_binding,
    schema_hash,
    root_from_proof,
)?;
```

## Dependencies

| Package | Language | Version | Purpose |
|---|---|---|---|
| `solid-light` | Rust | 0.1.0 | `SchemaTreeBinding` / `GlobalStateBinding` parsers + root CPI helpers |
| `@solana/spl-account-compression` | TypeScript | ^0.2 | Concurrent-Merkle-tree client primitives |
| `@solid-protocol/light` | TypeScript | 0.2.0 | SolID-specific adapter + `MerkleProofAdapter` interface |
| `bn.js` | TypeScript | ^5 | Big-int handling for compression primitives |

## Circuit Compatibility

Unchanged from v0.1 — the circuit contract is independent of the
compressed-state backend:

| Adapter Output | Circuit Input | Type |
|---|---|---|
| `root` | `merkleRoot` | Public |
| `siblings[d]` | `merkleSiblings[d]` | Private |
| `pathIndices[d]` | `merklePathIndices[d]` | Private |
| `leaf` | `commitment` (computed in-circuit) | Private |
