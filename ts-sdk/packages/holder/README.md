# @solid-protocol/holder

Holder-side proof generation SDK for SolID Protocol credentials. This is what
runs in the user's browser (or Node, for tests) when a verifier asks for a
private predicate proof. It assembles the witness, fetches the Merkle path for
the credential, calls snarkjs against the protocol-published `.wasm` /
`.zkey`, and returns a Groth16 proof + the 32-slot public-input vector that
the on-chain `zk_verifier` program accepts.

The holder never reveals the underlying credential value to the verifier — the
proof itself is the gate.

## Install

```bash
npm install @solid-protocol/holder @solid-protocol/core @solana/web3.js
```

## Minimal example

```ts
import { initWasm, QueryBuilder } from "@solid-protocol/core";
import { generateProof } from "@solid-protocol/holder";

await initWasm();

const query = new QueryBuilder()
  .schemas([schemaHash])
  .where(0, /* field index */ 4, "GTE", 2n)
  .verifier(verifierAddressBytes)
  .nonce(verifierNonceBytes)
  .expiration(Math.floor(Date.now() / 1000) + 600)
  .build();

const proof = await generateProof({
  credential,              // StoredCredential (decrypted envelope)
  query,                   // MultiCredentialQuery from QueryBuilder
  merkleProofSource,       // fetches the SPL-AC Merkle path
  artifactBaseUrl: "https://artifacts.solidislive.com",
});

// proof.proof  : Groth16 proof (snarkjs format)
// proof.publicSignals : 32-slot public input vector
// proof.nullifier     : 32-byte Poseidon nullifier (already in publicSignals)
```

## What the SDK hides

- Witness assembly across `batch_credential_query.circom`'s 32 public inputs.
- Per-schema credential key derivation
  (`credentialPrivKey = Poseidon(masterKey, schemaHash)`).
- Identity-leaf computation against the global state root.
- Issuer-tree root binding (ADR-0014 / SOLID-SEC-008 epoch bind).
- Merkle path fetching via the configurable `MerkleProofSource`.
- snarkjs proof generation against the pinned WASM/zkey artifacts.

## Artifact integrity

The SDK downloads the circuit artifacts (`batch_credential_query.wasm`,
`batch_credential_query.zkey`) from `artifactBaseUrl`. Production deployments
should keep the manifest SHA-256 pins active so a CDN compromise cannot
substitute a malicious prover. Hosted artifacts:
`https://artifacts.solidislive.com`.

## Public surface

| Function | Purpose |
| --- | --- |
| `generateProof` | Single-credential proof against one schema. |
| `generateBatchProof` | Up to `MAX_CREDENTIALS` proofs in one Groth16 call. |
| `verifyProofLocally` | Sanity-check a proof against the local verification key before submitting. |

## Related packages

- `@solid-protocol/core` — WASM crypto primitives (always required).
- `@solid-protocol/channel` — receive and decrypt the credential envelope from the issuer before proving.
- `@solid-protocol/verifier` — submit the produced proof on-chain.
- `@solid-protocol/sdk` — unified facade.

## License

MIT
