import { createServer } from 'node:http';
import { readFileSync, existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Connection, Keypair } from '@solana/web3.js';
import { validateSolidManifest } from '@solid-protocol/sdk/manifest';
import { createIndexerHandler } from './service.mjs';
import { FileIndexerStore } from './store.mjs';

const here = dirname(fileURLToPath(import.meta.url));

// Minimal dependency-free .env file loader for Node.js 18+
try {
  const envPath = resolve(here, '..', '.env');
  if (existsSync(envPath)) {
    const content = readFileSync(envPath, 'utf8');
    for (const line of content.split('\r\n').join('\n').split('\n')) {
      const trimmed = line.trim();
      if (!trimmed || trimmed.startsWith('#')) continue;
      const index = trimmed.indexOf('=');
      if (index > 0) {
        const key = trimmed.slice(0, index).trim();
        let val = trimmed.slice(index + 1).trim();
        if ((val.startsWith('"') && val.endsWith('"')) || (val.startsWith("'") && val.endsWith("'"))) {
          val = val.slice(1, -1);
        }
        if (process.env[key] === undefined) {
          process.env[key] = val;
        }
      }
    }
  }
} catch (err) {
  console.warn(`Failed to load .env file: ${err.message}`);
}
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
const rootSyncKeypair = loadRootSyncKeypair(manifest);
const handler = createIndexerHandler({
  manifest,
  store,
  connection,
  writeToken: process.env.SOLID_INDEXER_WRITE_TOKEN ?? '',
  rootSyncKeypair,
});

createServer(handler).listen(port, host, () => {
  console.log(`SolID indexer listening on http://${host}:${port}`);
  console.log(`Manifest: ${manifestPath}`);
  console.log(`Store: ${storePath}`);
  console.log(`Root sync: ${rootSyncKeypair ? rootSyncKeypair.publicKey.toBase58() : 'disabled'}`);
});

function loadRootSyncKeypair(manifest) {
  if (process.env.SOLID_AUTO_ROOT_SYNC === '0') return null;
  const keypairPath = process.env.SOLID_ROOT_SYNC_KEYPAIR_PATH || manifest.deployer?.keypair_path;
  if (!keypairPath) return null;
  try {
    const resolved = resolveTilde(keypairPath);
    const secret = Uint8Array.from(JSON.parse(readFileSync(resolved, 'utf8')));
    const keypair = Keypair.fromSecretKey(secret);
    const expected = manifest.deployer?.address;
    if (expected && keypair.publicKey.toBase58() !== expected) {
      console.warn(`Root sync keypair ${keypair.publicKey.toBase58()} does not match manifest deployer ${expected}; auto root sync disabled.`);
      return null;
    }
    return keypair;
  } catch (error) {
    console.warn(`Root sync keypair could not be loaded: ${error.message}`);
    return null;
  }
}

function resolveTilde(value) {
  if (value === '~') return homedir();
  if (value.startsWith('~/')) return resolve(homedir(), value.slice(2));
  return resolve(value);
}
