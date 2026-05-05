# Verifier Integration

Verifiers should integrate through `@solid-protocol/verifier`, not by
constructing proof request JSON manually.

## Install

```bash
npm install @solid-protocol/verifier @solid-protocol/sdk @solana/web3.js
```

## Request A Proof

```ts
import { PublicKey } from "@solana/web3.js";
import { SolidVerifier, walletAdapterTransport } from "@solid-protocol/verifier";
import {
  DEFAULT_DEVNET_MANIFEST,
  artifactPinsFromManifest,
  programIdsFromManifest,
} from "@solid-protocol/sdk/manifest";

const programIds = programIdsFromManifest(DEFAULT_DEVNET_MANIFEST);
const pins = artifactPinsFromManifest(DEFAULT_DEVNET_MANIFEST);

const solid = new SolidVerifier({
  cluster: "devnet",
  artifactPins: {
    batchVk: pins.batchVerificationKey,
    batchWasm: pins.batchWasm,
    batchZkey: pins.batchZkey,
    subgroupVk: pins.subgroupVerificationKey,
    subgroupWasm: pins.subgroupWasm,
    subgroupZkey: pins.subgroupZkey,
  },
  artifactHostUrl: DEFAULT_DEVNET_MANIFEST.artifacts.base_url ?? undefined,
  indexerUrl: DEFAULT_DEVNET_MANIFEST.indexer.url ?? undefined,
  programIds: {
    zkVerifier: new PublicKey(programIds.zkVerifier),
    issuerRegistry: new PublicKey(programIds.issuerRegistry),
    schemaRegistry: new PublicKey(programIds.schemaRegistry),
  },
});

const requirement = solid.defineRequirement({
  schema: "basic_identity_v1",
  predicates: [{ field: "age", op: "GTE", value: 18 }],
  compoundLogic: "AND",
  action: {
    appId: window.location.origin,
    action: "enter_private_launchpad",
    nonce: crypto.randomUUID(),
  },
});

const proof = await solid.requestProof({
  wallet: PublicKey.default,
  requirement,
  transport: walletAdapterTransport(window.solid),
});
```

## Verify

For public devnet feedback, prefer on-chain verification so testers see a
real Solana transaction and replay protection.

The verifier must fail closed when:

- wallet is missing
- artifact host is missing
- artifact pin mismatches
- indexer is stale
- schema is unsupported
- proof public inputs do not match the requirement
- nullifier replay is detected

See `examples/verifier-dapp` for a minimal browser example.
