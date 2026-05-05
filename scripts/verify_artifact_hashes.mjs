#!/usr/bin/env node
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const root = resolve(new URL('..', import.meta.url).pathname);
const manifestPath = resolve(root, 'deployments/devnet.json');
const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));

const items = manifest?.artifacts?.items ?? {};
const failures = [];

for (const [key, artifact] of Object.entries(items)) {
  const localPath = artifact.local_path;
  if (!localPath) {
    failures.push(`${key}: missing local_path`);
    continue;
  }
  const expected = String(artifact.sha256 ?? '').trim().toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(expected)) {
    failures.push(`${key}: invalid SHA-256 pin ${expected}`);
    continue;
  }
  try {
    const bytes = readFileSync(resolve(root, localPath));
    const actual = createHash('sha256').update(bytes).digest('hex');
    if (actual !== expected) {
      failures.push(`${key}: hash mismatch expected ${expected}, got ${actual}`);
    } else {
      console.log(`[artifact] ${key} ok ${actual}`);
    }
  } catch (error) {
    failures.push(`${key}: cannot read ${localPath}: ${error.message}`);
  }
}

if (failures.length > 0) {
  console.error('\nArtifact hash verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('\nAll manifest artifact hashes match local files.');
