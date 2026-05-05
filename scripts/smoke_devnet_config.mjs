#!/usr/bin/env node
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const root = resolve(new URL('..', import.meta.url).pathname);
const manifest = JSON.parse(readFileSync(resolve(root, 'deployments/devnet.json'), 'utf8'));

const checks = [
  ['cluster', manifest.cluster],
  ['schema_registry', manifest.programs?.schema_registry?.program_id],
  ['issuer_registry', manifest.programs?.issuer_registry?.program_id],
  ['zk_verifier', manifest.programs?.zk_verifier?.program_id],
  ['batch wasm pin', manifest.artifacts?.items?.batchCredentialQueryWasm?.sha256],
  ['batch zkey pin', manifest.artifacts?.items?.batchCredentialQueryZkey?.sha256],
  ['subgroup wasm pin', manifest.artifacts?.items?.subgroupWasm?.sha256],
  ['subgroup zkey pin', manifest.artifacts?.items?.subgroupZkey?.sha256],
];

for (const [label, value] of checks) {
  if (!value) {
    console.error(`Missing devnet config value: ${label}`);
    process.exit(1);
  }
}

const publicBundle = {
  network: manifest.network,
  cluster: manifest.cluster,
  programs: manifest.programs,
  artifacts: manifest.artifacts,
  indexer: manifest.indexer,
  schemas: manifest.schemas,
  trees: manifest.trees,
};

console.log(JSON.stringify(publicBundle, null, 2));
