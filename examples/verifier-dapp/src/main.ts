import { PublicKey } from '@solana/web3.js';
import { SolidVerifier, walletAdapterTransport } from '@solid-protocol/verifier';
import {
  DEFAULT_DEVNET_MANIFEST,
  artifactPinsFromManifest,
  programIdsFromManifest,
} from '@solid-protocol/sdk/manifest';

declare global {
  interface Window {
    solid?: {
      requestProof(request: unknown): Promise<unknown>;
    };
  }
}

const output = document.querySelector<HTMLPreElement>('#output')!;
const button = document.querySelector<HTMLButtonElement>('#request-proof')!;
const programIds = programIdsFromManifest(DEFAULT_DEVNET_MANIFEST);

const verifier = new SolidVerifier({
  cluster: 'devnet',
  artifactPins: {
    batchVk: artifactPinsFromManifest(DEFAULT_DEVNET_MANIFEST).batchVerificationKey,
    batchWasm: artifactPinsFromManifest(DEFAULT_DEVNET_MANIFEST).batchWasm,
    batchZkey: artifactPinsFromManifest(DEFAULT_DEVNET_MANIFEST).batchZkey,
    subgroupVk: artifactPinsFromManifest(DEFAULT_DEVNET_MANIFEST).subgroupVerificationKey,
    subgroupWasm: artifactPinsFromManifest(DEFAULT_DEVNET_MANIFEST).subgroupWasm,
    subgroupZkey: artifactPinsFromManifest(DEFAULT_DEVNET_MANIFEST).subgroupZkey,
  },
  artifactHostUrl: DEFAULT_DEVNET_MANIFEST.artifacts.base_url ?? undefined,
  indexerUrl: DEFAULT_DEVNET_MANIFEST.indexer.url ?? undefined,
  programIds: {
    zkVerifier: new PublicKey(programIds.zkVerifier),
    issuerRegistry: new PublicKey(programIds.issuerRegistry),
    schemaRegistry: new PublicKey(programIds.schemaRegistry),
  },
});

button.addEventListener('click', async () => {
  try {
    if (!window.solid) {
      throw new Error('SolID Wallet is not installed.');
    }

    const requirement = verifier.defineRequirement({
      schema: 'basic_identity_v1',
      predicates: [{ field: 'age', op: 'GTE', value: 18 }],
      compoundLogic: 'AND',
      action: {
        appId: location.origin,
        action: 'enter_private_launchpad',
        nonce: crypto.randomUUID(),
      },
    });

    const proof = await verifier.requestProof({
      wallet: PublicKey.default,
      requirement,
      transport: walletAdapterTransport(window.solid),
    });

    output.textContent = JSON.stringify({ ok: true, proof }, null, 2);
  } catch (error) {
    output.textContent = error instanceof Error ? error.message : String(error);
  }
});
