# @solid-protocol/sdk

Unified TypeScript SDK for SolID Protocol.

SolID lets Solana applications verify private eligibility claims without
receiving or storing the user's underlying personal data. The SDK provides a
single import surface for the protocol packages used by apps, issuers, holders,
and operators.

## Install

```bash
npm install @solid-protocol/sdk @solana/web3.js
```

For verifier-only integrations, install `@solid-protocol/verifier` directly.

## Package Surface

`@solid-protocol/sdk` re-exports the core protocol primitives and convenience
modules:

- `SolID`: high-level facade for proving and on-chain verification.
- `@solid-protocol/core`: WASM-backed Poseidon, BabyJubJub, schema, and query helpers.
- `@solid-protocol/verifier`: on-chain proof verification helpers and PDA derivation.
- `@solid-protocol/sdk/manifest`: devnet manifest helpers.
- `@solid-protocol/sdk/indexer`: typed indexer client.
- `@solid-protocol/sdk/credential-requests`: request-envelope helpers.
- `@solid-protocol/sdk/credential-integrity`: credential integrity helpers.

## Basic Usage

```ts
import { SolID, QueryBuilder } from "@solid-protocol/sdk";

await SolID.initialize();

const identity = SolID.generateMasterIdentity();
const query = new QueryBuilder()
  .where(0, 4, "GTE", 2n)
  .build();

console.log(identity.public_key_x, query.queryContextHash);
```

## Artifact Integrity

Proof generation loads circuit artifacts (`.wasm`, `.zkey`) and verifies their
SHA-256 pins before passing them to the prover. Production deployments should
publish a manifest with pinned artifact hashes and should not disable integrity
checks.

## Runtime Requirements

- Node.js 18 or newer.
- ESM-compatible TypeScript or JavaScript project.
- A Solana RPC endpoint for on-chain reads and verification.
- Published SolID circuit artifacts and manifest values for the target cluster.

## Related Packages

- `@solid-protocol/verifier`: app and verifier integrations.
- `@solid-protocol/issuer`: issuer-side credential issuance.
- `@solid-protocol/holder`: holder-side proof generation.
- `@solid-protocol/channel`: encrypted credential delivery.
- `@solid-protocol/light`: SPL Account Compression adapter and Merkle proof utilities.
- `@solid-protocol/core`: WASM-backed cryptographic primitives.
