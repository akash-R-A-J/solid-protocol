#!/usr/bin/env node
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const root = resolve(new URL('..', import.meta.url).pathname);
const manifestPath = resolve(root, 'deployments/devnet.json');
const anchorPath = resolve(root, 'Anchor.toml');
const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
const anchorToml = readFileSync(anchorPath, 'utf8');

const failures = [];
const requiredPrograms = ['schema_registry', 'issuer_registry', 'zk_verifier'];
const requiredArtifacts = [
  'batchCredentialQueryWasm',
  'batchCredentialQueryZkey',
  'batchCredentialQueryVerificationKey',
  'subgroupWasm',
  'subgroupZkey',
  'subgroupVerificationKey',
];

function expect(condition, message) {
  if (!condition) failures.push(message);
}

function parseAnchorPrograms(section) {
  const out = {};
  let inSection = false;
  for (const raw of anchorToml.split(/\r?\n/)) {
    const line = raw.trim();
    if (line === `[programs.${section}]`) {
      inSection = true;
      continue;
    }
    if (line.startsWith('[') && line !== `[programs.${section}]`) {
      inSection = false;
      continue;
    }
    if (!inSection) continue;
    const match = line.match(/^([A-Za-z0-9_]+)\s*=\s*"([^"]+)"$/);
    if (match) out[match[1]] = match[2];
  }
  return out;
}

function isPubkey(value) {
  return typeof value === 'string' && /^[1-9A-HJ-NP-Za-km-z]{32,44}$/.test(value);
}

function isSha256(value) {
  return typeof value === 'string' && /^[0-9a-f]{64}$/.test(value);
}

function isHttpsOrLocalhost(value) {
  if (value == null) return true;
  try {
    const url = new URL(value);
    const localhost = ['localhost', '127.0.0.1', '[::1]'].includes(url.hostname);
    return url.protocol === 'https:' || url.protocol === 'wss:' || localhost;
  } catch {
    return false;
  }
}

expect(manifest.schema_version === 1, 'schema_version must be 1');
expect(manifest.network === 'devnet', 'network must be devnet');
expect(isHttpsOrLocalhost(manifest.cluster), 'cluster must be HTTPS or localhost');
expect(manifest.programs && typeof manifest.programs === 'object', 'programs object is required');

const anchorDevnet = parseAnchorPrograms('devnet');
const anchorLocalnet = parseAnchorPrograms('localnet');

for (const program of requiredPrograms) {
  const id = manifest.programs?.[program]?.program_id;
  expect(isPubkey(id), `${program}.program_id is invalid`);
  expect(anchorDevnet[program] === id, `${program} does not match Anchor.toml [programs.devnet]`);
  expect(anchorLocalnet[program] === id, `${program} does not match Anchor.toml [programs.localnet]`);
}

for (const key of requiredArtifacts) {
  const artifact = manifest.artifacts?.items?.[key];
  expect(artifact, `missing artifact ${key}`);
  expect(typeof artifact?.filename === 'string' && artifact.filename.length > 0, `${key}.filename is required`);
  expect(isSha256(artifact?.sha256), `${key}.sha256 must be 64 lowercase hex chars`);
}

expect(isHttpsOrLocalhost(manifest.artifacts?.base_url), 'artifacts.base_url must be HTTPS, WSS, localhost, or null');
expect(isHttpsOrLocalhost(manifest.indexer?.url), 'indexer.url must be HTTPS, WSS, localhost, or null');
expect(Array.isArray(manifest.schemas), 'schemas must be an array');

if (failures.length > 0) {
  console.error('Devnet manifest validation failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Devnet manifest validation passed.');
