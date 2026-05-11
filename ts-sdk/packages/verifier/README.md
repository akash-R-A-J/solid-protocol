# @solid-protocol/verifier

High-level private eligibility verification for Solana apps. This package is
currently published as a devnet alpha surface for verifier dApp pilots.

This package is the verifier-facing integration surface for SolID. App
developers should think in requirements, not circuits, Merkle paths, proof
buffers, nullifiers, account ordering, or verification-key artifacts.

## Install

```bash
npm install @solid-protocol/verifier @solana/web3.js
```

## 15-minute integration

```ts
import {
  SolidVerifier,
  explainVerificationError,
  walletAdapterTransport,
} from "@solid-protocol/verifier";

const solid = new SolidVerifier({
  cluster: "devnet",
  // Alpha note: pass the hosted URLs until canonical devnet defaults are
  // published in the npm package.
  artifactHostUrl: "https://artifacts.solidislive.com",
  indexerUrl: "https://api.solidislive.com",
});

const requirement = solid.defineRequirement({
  schema: "basic_identity_v2",
  predicates: [
    { field: "verification_level", op: ">=", value: 2 },
    { field: "country_code", op: "!=", value: 840 },
  ],
  action: { appId: "my-launchpad", action: "join_pool_42" },
});

const result = await solid.verifyRequirement({
  wallet: wallet.publicKey,
  payer: backendPayer,
  requirement,
  transport: walletAdapterTransport(wallet),
});

if (result.verified) {
  allowUser();
} else {
  const message = explainVerificationError(result.reason, result.detail);
  showError(message.humanReadable, message.suggestedAction);
}
```

## What the SDK hides

- Proof-buffer chunk upload and `verify_batch_proof_v2`.
- Public input slot ordering and 21-of-32 wire extraction.
- Schema/global/issuer tree PDA derivation.
- Nullifier PDA replay checks.
- Artifact SHA-256 pins for wasm, zkey, and verification keys.
- Raw Solana errors, mapped into stable `VerificationError` values.

## Health check

```ts
const report = await solid.health();
if (!report.ok) console.error(report.errors);
```

`health()` checks the configured program IDs, verifier config PDA, artifact
host, indexer endpoint, and active artifact pins.

## Devnet alpha scope

The verifier SDK hides the low-level proof-buffer upload, public input slicing,
PDA derivation, nullifier replay checks, and typed error mapping. For the alpha,
integrators should still expect to configure the hosted artifact and indexer
URLs, provide a verifier payer from their backend, and use either
`walletAdapterTransport`, `httpTransport`, or a custom proof transport
connected to a SolID-capable holder.
