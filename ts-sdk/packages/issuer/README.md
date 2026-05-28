# @solid-protocol/issuer

Issuer-side credential issuance SDK for SolID Protocol. Handles the steps an
approved issuer takes after a holder requests a credential: derive the issuer
BabyJubJub identity, generate the prime-order-subgroup proof, sign the Poseidon
commitment, encrypt the envelope to the holder's channel key, append the
commitment into the SPL Account Compression tree, and record the issuance
on-chain via the `issuer-registry` program.

Issuance is permissioned: the issuer must already be staked, DAO-approved, and
granted schema-write permission. This SDK never silently skips those checks.

## Install

```bash
npm install @solid-protocol/issuer @solid-protocol/core @solana/web3.js
```

## Minimal example — issue one credential

```ts
import { initWasm } from "@solid-protocol/core";
import { issueCredential, generateSubgroupProof } from "@solid-protocol/issuer";

await initWasm();

// Once per issuer wallet: produce the BJJ subgroup proof that
// `register_issuer` consumes (SEC-048 Phase E).
const subgroupProof = await generateSubgroupProof({
  bjjKeypair: myIssuerBjjKeypair,
  artifactBaseUrl: "https://artifacts.solidislive.com",
});

const { signature, envelope, commitment } = await issueCredential({
  connection,
  payer,
  issuerKeypair: solanaIssuerKeypair,
  issuerBjjKeypair: myIssuerBjjKeypair,
  request: {
    schemaHash,
    schemaName: "basic_identity_v2",
    schemaVersion: 1,
    holderChannelPublicKey,  // from the credential request
    holderPublicKeyX,
    holderPublicKeyY,
    fields: {
      age: 25,
      country_code: 356,
      region: 1,
      id_type: 3,
      verification_level: 2,
      issued_date: 1778315365,
      nationality: 356,
      _reserved: 0,
    },
  },
});

// `envelope` is the ECIES-encrypted credential bundle to send back to the
// holder via the indexer's request queue.
// `commitment` is the Poseidon hash that lives on the SPL-AC Merkle tree.
// `signature` is the Solana tx signature for the on-chain issuance record.
```

## Public surface

| Function | Purpose |
| --- | --- |
| `issueCredential` | High-level single-credential issuance: builds tx, signs envelope, returns commitment + sig. |
| `batchIssueCredentials` | Issue multiple credentials to multiple holders in a single tx where possible. |
| `buildIssueCredentialIx` | Lower-level: just construct the `issue_credential` instruction. |
| `generateSubgroupProof` | Build the BJJ prime-order subgroup proof required by `register_issuer`. |
| `deriveIssuerAccount` | Derive the `IssuerAccount` PDA for a given authority. |
| `deriveIssuerSchemaPermission` | Derive the schema-permission PDA for an `(issuer, schema)` pair. |

Schema-registry typed re-exports live under `@solid-protocol/issuer/registry`
(used by the indexer to decode `IssuerAccount` / `SchemaAccount`).

## Related packages

- `@solid-protocol/core` — WASM crypto primitives (always required).
- `@solid-protocol/channel` — produce the encrypted envelope sent to the holder.
- `@solid-protocol/light` — SPL Account Compression adapter for tree appends.
- `@solid-protocol/sdk` — unified facade.

## License

MIT
