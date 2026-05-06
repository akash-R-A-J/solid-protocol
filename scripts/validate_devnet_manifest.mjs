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

function isSlot(value) {
  return value == null || (Number.isInteger(value) && value >= 0);
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

for (const [index, schema] of (manifest.schemas ?? []).entries()) {
  expect(typeof schema.name === 'string' && schema.name.length > 0, `schemas[${index}].name is required`);
  expect(Number.isInteger(schema.version) && schema.version > 0, `schemas[${index}].version is invalid`);
  expect(isSha256(schema.schema_hash), `schemas[${index}].schema_hash must be a 32-byte hex hash`);
  expect(Array.isArray(schema.fields) && schema.fields.length > 0 && schema.fields.length <= 8, `schemas[${index}].fields must contain 1-8 fields`);
  expect(Array.isArray(schema.predicates), `schemas[${index}].predicates must be an array`);
  expect(schema.tree_address == null || isPubkey(schema.tree_address), `schemas[${index}].tree_address is invalid`);
  expect(schema.current_root == null || isSha256(schema.current_root), `schemas[${index}].current_root must be a 32-byte hex root or null`);
  expect(isSlot(schema.current_root_slot), `schemas[${index}].current_root_slot must be a non-negative integer or null`);
}

const globalTree = manifest.trees?.global_state_tree;
expect(globalTree == null || typeof globalTree === 'object', 'trees.global_state_tree must be an object or null');
if (globalTree) {
  expect(isPubkey(globalTree.tree_address) || isPubkey(globalTree.binding_pda), 'trees.global_state_tree must include tree_address or binding_pda');
  expect(globalTree.current_root == null || isSha256(globalTree.current_root), 'trees.global_state_tree.current_root must be a 32-byte hex root or null');
  expect(isSlot(globalTree.current_root_slot), 'trees.global_state_tree.current_root_slot must be a non-negative integer or null');
}

expect(Array.isArray(manifest.trees?.schema_trees), 'trees.schema_trees must be an array');
for (const [index, tree] of (manifest.trees?.schema_trees ?? []).entries()) {
  expect(isSha256(tree.schema_hash), `trees.schema_trees[${index}].schema_hash must be a 32-byte hex hash`);
  expect(isPubkey(tree.tree_address), `trees.schema_trees[${index}].tree_address is invalid`);
  expect(tree.binding_pda == null || isPubkey(tree.binding_pda), `trees.schema_trees[${index}].binding_pda is invalid`);
  expect(tree.current_root == null || isSha256(tree.current_root), `trees.schema_trees[${index}].current_root must be a 32-byte hex root or null`);
  expect(isSlot(tree.current_root_slot), `trees.schema_trees[${index}].current_root_slot must be a non-negative integer or null`);
}

if (failures.length > 0) {
  console.error('Devnet manifest validation failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Devnet manifest validation passed.');
