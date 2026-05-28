# @solid-protocol/light

SPL Account Compression adapter and Merkle proof utilities for SolID Protocol.
This package is the bridge between the SolID programs and the SPL Account
Compression concurrent Merkle trees that store credential commitments.

It handles tree PDA derivation, root reads, tree state lookups, and fetching
Merkle proofs in the byte order the on-chain Poseidon-Merkle recompute
expects.

## Install

```bash
npm install @solid-protocol/light @solana/web3.js
```

## Minimal example — fetch a Merkle proof for a credential commitment

```ts
import { Connection, PublicKey } from "@solana/web3.js";
import {
  deriveSchemaTreeBinding,
  fetchMerkleProof,
  LocalReplicaAdapter,
  poseidonHashPair,
} from "@solid-protocol/light";

const connection = new Connection("https://api.devnet.solana.com", "confirmed");

const { pda: schemaTreeBinding } = deriveSchemaTreeBinding(schemaHash);
const treeBindingAccount = await connection.getAccountInfo(schemaTreeBinding);
const treeAddress = new PublicKey(/* decoded from treeBindingAccount.data */);

const adapter = new LocalReplicaAdapter({ /* configured Merkle replica */ });
const proof = await fetchMerkleProof(adapter, treeAddress, leafCommitment);

// proof.root      : 32-byte current root (must match SchemaTreeBinding.current_root)
// proof.path      : 20 sibling hashes (one per tree level)
// proof.leafIndex : the leaf's index in the tree
```

## Public surface

| Group | Items |
| --- | --- |
| Program IDs | `ISSUER_REGISTRY_PROGRAM_ID`, `SCHEMA_REGISTRY_PROGRAM_ID`, `SPL_ACCOUNT_COMPRESSION_PROGRAM_ID`, `SPL_NOOP_PROGRAM_ID` |
| PDA derivers | `deriveTreeAuthority`, `deriveSchemaAccount`, `deriveSchemaTreeBinding`, `deriveGlobalBinding`, `deriveIssuerTreeBinding`, `deriveIssuerTreeAuthority` |
| Tree operations | `createCredentialTree`, `getCurrentTreeRoot`, `getTreeState` |
| Merkle proof | `MerkleProofAdapter` (interface), `LocalReplicaAdapter`, `fetchMerkleProof` |
| Helpers | `poseidonHashPair`, `TreeParams`, `DEFAULT_TREE_PARAMS` |

## Why a separate adapter layer

Production deployments cannot rely on a single RPC to maintain a current
Merkle proof — proofs require either the full tree state in a replica or an
indexed leaf store. The `MerkleProofAdapter` interface keeps the proof source
swappable so a hosted indexer (`api.solidislive.com`), a local replica, or a
custom store all expose the same shape to consumers like
`@solid-protocol/holder`.

## Related packages

- `@solid-protocol/core` — WASM Poseidon used by `poseidonHashPair`.
- `@solid-protocol/holder` — consumes Merkle proofs during witness assembly.
- `@solid-protocol/issuer` — uses tree-binding derivers when appending leaves.
- `@solid-protocol/sdk` — unified facade.

## License

MIT
