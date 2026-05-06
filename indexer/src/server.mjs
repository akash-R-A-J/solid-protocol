import { createServer } from 'node:http';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Connection } from '@solana/web3.js';
import { validateSolidManifest } from '@solid-protocol/sdk/manifest';
import { createIndexerHandler } from './service.mjs';
import { FileIndexerStore } from './store.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, '..', '..');
const manifestPath = resolve(process.env.SOLID_MANIFEST_PATH ?? resolve(repoRoot, 'deployments/devnet.json'));
const storePath = resolve(process.env.SOLID_INDEXER_STORE_PATH ?? resolve(repoRoot, '.solid-indexer/state.json'));
const port = Number(process.env.PORT ?? '8787');
const host = process.env.HOST ?? '127.0.0.1';

if (!Number.isSafeInteger(port) || port <= 0 || port > 65535) {
  throw new Error(`Invalid PORT: ${String(process.env.PORT)}`);
}

const manifest = validateSolidManifest(JSON.parse(readFileSync(manifestPath, 'utf8')));
const rpcUrl = process.env.SOLID_RPC_URL || manifest.cluster;
const connection = rpcUrl ? new Connection(rpcUrl, 'confirmed') : null;
const store = new FileIndexerStore(storePath);
const handler = createIndexerHandler({
  manifest,
  store,
  connection,
  writeToken: process.env.SOLID_INDEXER_WRITE_TOKEN ?? '',
});

createServer(handler).listen(port, host, () => {
  console.log(`SolID indexer listening on http://${host}:${port}`);
  console.log(`Manifest: ${manifestPath}`);
  console.log(`Store: ${storePath}`);
});
