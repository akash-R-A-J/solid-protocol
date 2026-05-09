import { mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';
import { randomUUID } from 'node:crypto';

const EMPTY_STATE = Object.freeze({
  credentialRequests: [],
  proofRequests: [],
  customSchemaRequests: [],
  schemaPermissionRequests: [],
  treeLeaves: [],
  processedTransactions: [],
  metadata: {
    createdAt: null,
    updatedAt: null,
  },
});

export class FileIndexerStore {
  constructor(path) {
    this.path = path;
  }

  read() {
    try {
      const parsed = JSON.parse(readFileSync(this.path, 'utf8'));
      return normalizeState(parsed);
    } catch {
      return normalizeState({});
    }
  }

  write(nextState) {
    const state = normalizeState(nextState);
    const now = new Date().toISOString();
    state.metadata = {
      createdAt: state.metadata.createdAt ?? now,
      updatedAt: now,
    };
    mkdirSync(dirname(this.path), { recursive: true });
    const tmp = `${this.path}.${process.pid}.${Date.now()}.tmp`;
    writeFileSync(tmp, JSON.stringify(state, null, 2));
    renameSync(tmp, this.path);
    return state;
  }
}

export class MemoryIndexerStore {
  constructor(initialState = {}) {
    this.state = normalizeState(initialState);
  }

  read() {
    return normalizeState(JSON.parse(JSON.stringify(this.state)));
  }

  write(nextState) {
    this.state = normalizeState(JSON.parse(JSON.stringify(nextState)));
    const now = new Date().toISOString();
    this.state.metadata.createdAt ??= now;
    this.state.metadata.updatedAt = now;
    return this.read();
  }
}

export function createCredentialRequest(store, input, network = 'devnet') {
  const state = store.read();
  const now = new Date().toISOString();
  const record = {
    id: randomUUID(),
    network,
    schemaHash: normalizeSchemaHash(input.schemaHash),
    schemaName: requiredString(input.schemaName, 'schemaName'),
    schemaVersion: integer(input.schemaVersion, 'schemaVersion'),
    issuerAuthority: requiredString(input.issuerAuthority, 'issuerAuthority'),
    issuerAccount: nullableString(input.issuerAccount),
    holderChannelPublicKey: requiredString(input.holderChannelPublicKey, 'holderChannelPublicKey'),
    holderPublicKeyX: requiredString(input.holderPublicKeyX, 'holderPublicKeyX'),
    holderPublicKeyY: requiredString(input.holderPublicKeyY, 'holderPublicKeyY'),
    holderLabel: nullableString(input.holderLabel),
    status: 'requested',
    requestedAt: now,
    updatedAt: now,
    notes: nullableString(input.notes),
    rejectionReason: null,
    envelopeJson: null,
    commitment: null,
    transactionSignature: null,
  };
  state.credentialRequests = [record, ...state.credentialRequests];
  store.write(state);
  return record;
}

export function listCredentialRequests(store, filters = {}) {
  return store.read().credentialRequests
    .filter((record) => requestMatches(record, filters))
    .sort((a, b) => String(b.updatedAt).localeCompare(String(a.updatedAt)));
}

export function getCredentialRequest(store, id) {
  return store.read().credentialRequests.find((record) => record.id === id) ?? null;
}

export function updateCredentialRequest(store, id, patch) {
  const state = store.read();
  const index = state.credentialRequests.findIndex((record) => record.id === id);
  if (index < 0) return null;
  const allowedStatuses = new Set(['requested', 'reviewing', 'approved', 'rejected', 'issued', 'cancelled']);
  if (patch.status !== undefined && !allowedStatuses.has(patch.status)) {
    throw new Error(`Unsupported credential request status: ${patch.status}`);
  }
  const mutableKeys = ['status', 'notes', 'rejectionReason', 'envelopeJson', 'commitment', 'transactionSignature'];
  const next = { ...state.credentialRequests[index] };
  for (const key of mutableKeys) {
    if (Object.prototype.hasOwnProperty.call(patch, key)) {
      next[key] = patch[key] === undefined ? next[key] : patch[key];
    }
  }
  next.updatedAt = new Date().toISOString();
  state.credentialRequests[index] = next;
  store.write(state);
  return next;
}

export function createProofRequest(store, input, network = 'devnet') {
  const state = store.read();
  const now = new Date().toISOString();
  const predicates = Array.isArray(input.predicates) ? input.predicates : [];
  if (predicates.length === 0) {
    throw new Error('proof request requires at least one predicate');
  }
  const record = {
    id: randomUUID(),
    network,
    dappName: requiredString(input.dappName, 'dappName'),
    action: requiredString(input.action, 'action'),
    reason: nullableString(input.reason),
    schemaHash: normalizeSchemaHash(input.schemaHash),
    schemaName: requiredString(input.schemaName, 'schemaName'),
    schemaVersion: integer(input.schemaVersion, 'schemaVersion'),
    holderPublicKeyX: nullableString(input.holderPublicKeyX),
    predicates: predicates.map((predicate, index) => ({
      fieldIndex: integer(predicate.fieldIndex ?? index, `predicates[${index}].fieldIndex`),
      fieldName: requiredString(predicate.fieldName, `predicates[${index}].fieldName`),
      operator: requiredString(predicate.operator, `predicates[${index}].operator`),
      value: requiredString(predicate.value, `predicates[${index}].value`),
    })),
    status: 'requested',
    requestedAt: now,
    updatedAt: now,
    expiresAt: nullableString(input.expiresAt),
    proofJson: null,
    publicSignalsJson: null,
    solanaProofJson: null,
    nullifier: null,
    verificationStatus: null,
    verificationTxSignatures: [],
  };
  state.proofRequests = [record, ...state.proofRequests];
  store.write(state);
  return record;
}

export function listProofRequests(store, filters = {}) {
  return store.read().proofRequests
    .filter((record) => requestMatches(record, filters))
    .sort((a, b) => String(b.updatedAt).localeCompare(String(a.updatedAt)));
}

export function getProofRequest(store, id) {
  return store.read().proofRequests.find((record) => record.id === id) ?? null;
}

export function updateProofRequest(store, id, patch) {
  const state = store.read();
  const index = state.proofRequests.findIndex((record) => record.id === id);
  if (index < 0) return null;
  const allowedStatuses = new Set(['requested', 'approved', 'rejected', 'proof_ready', 'verified', 'cancelled']);
  if (patch.status !== undefined && !allowedStatuses.has(patch.status)) {
    throw new Error(`Unsupported proof request status: ${patch.status}`);
  }
  const mutableKeys = [
    'status',
    'holderPublicKeyX',
    'proofJson',
    'publicSignalsJson',
    'solanaProofJson',
    'nullifier',
    'verificationStatus',
    'verificationTxSignatures',
  ];
  const next = { ...state.proofRequests[index] };
  for (const key of mutableKeys) {
    if (Object.prototype.hasOwnProperty.call(patch, key)) {
      next[key] = patch[key] === undefined ? next[key] : patch[key];
    }
  }
  next.updatedAt = new Date().toISOString();
  state.proofRequests[index] = next;
  store.write(state);
  return next;
}

export function createSchemaPermissionRequest(store, input, network = 'devnet') {
  const state = store.read();
  const now = new Date().toISOString();
  const record = {
    id: randomUUID(),
    network,
    issuerAuthority: requiredString(input.issuerAuthority, 'issuerAuthority'),
    issuerAccount: requiredString(input.issuerAccount, 'issuerAccount'),
    issuerName: nullableString(input.issuerName),
    schemaHash: normalizeSchemaHash(input.schemaHash),
    schemaName: requiredString(input.schemaName, 'schemaName'),
    schemaVersion: integer(input.schemaVersion, 'schemaVersion'),
    reason: nullableString(input.reason),
    status: 'requested',
    requestedAt: now,
    updatedAt: now,
    transactionSignature: null,
    rejectionReason: null,
  };
  state.schemaPermissionRequests = [record, ...state.schemaPermissionRequests];
  store.write(state);
  return record;
}

export function createCustomSchemaRequest(store, input, network = 'devnet') {
  const state = store.read();
  const now = new Date().toISOString();
  const fields = Array.isArray(input.fields) ? input.fields : [];
  if (fields.length === 0 || fields.length > 8) {
    throw new Error('custom schema requires 1-8 fields');
  }
  const record = {
    id: randomUUID(),
    network,
    proposerAuthority: requiredString(input.proposerAuthority, 'proposerAuthority'),
    proposerIssuerAccount: nullableString(input.proposerIssuerAccount),
    proposerIssuerName: nullableString(input.proposerIssuerName),
    name: validateSchemaName(input.name),
    displayName: nullableString(input.displayName),
    version: integer(input.version, 'version'),
    category: validateSchemaCategory(input.category),
    fields: fields.map((field, index) => ({
      name: validateFieldName(field?.name, `fields[${index}].name`),
      type: validateFieldType(field?.type, `fields[${index}].type`),
      description: nullableString(field?.description),
      rangeQueryable: Boolean(field?.rangeQueryable),
    })),
    schemaHash: normalizeSchemaHash(input.schemaHash),
    reason: nullableString(input.reason),
    status: 'requested',
    requestedAt: now,
    updatedAt: now,
    registeredAt: null,
    registeredBy: null,
    registerSchemaSignature: null,
    initializeTreeSignature: null,
    schemaPda: null,
    schemaTreeBindingPda: null,
    treeAddress: null,
    rejectionReason: null,
  };
  state.customSchemaRequests = [record, ...state.customSchemaRequests];
  store.write(state);
  return record;
}

export function listCustomSchemaRequests(store, filters = {}) {
  return store.read().customSchemaRequests
    .filter((record) => requestMatches(record, filters))
    .sort((a, b) => String(b.updatedAt).localeCompare(String(a.updatedAt)));
}

export function getCustomSchemaRequest(store, id) {
  return store.read().customSchemaRequests.find((record) => record.id === id) ?? null;
}

export function updateCustomSchemaRequest(store, id, patch) {
  const state = store.read();
  const index = state.customSchemaRequests.findIndex((record) => record.id === id);
  if (index < 0) return null;
  const allowedStatuses = new Set(['requested', 'registered', 'rejected', 'cancelled']);
  if (patch.status !== undefined && !allowedStatuses.has(patch.status)) {
    throw new Error(`Unsupported custom schema request status: ${patch.status}`);
  }
  const mutableKeys = [
    'status',
    'registeredAt',
    'registeredBy',
    'registerSchemaSignature',
    'initializeTreeSignature',
    'schemaPda',
    'schemaTreeBindingPda',
    'treeAddress',
    'rejectionReason',
  ];
  const next = { ...state.customSchemaRequests[index] };
  for (const key of mutableKeys) {
    if (Object.prototype.hasOwnProperty.call(patch, key)) {
      next[key] = patch[key] === undefined ? next[key] : patch[key];
    }
  }
  next.updatedAt = new Date().toISOString();
  state.customSchemaRequests[index] = next;
  store.write(state);
  return next;
}

export function listSchemaPermissionRequests(store, filters = {}) {
  return store.read().schemaPermissionRequests
    .filter((record) => requestMatches(record, filters))
    .sort((a, b) => String(b.updatedAt).localeCompare(String(a.updatedAt)));
}

export function getSchemaPermissionRequest(store, id) {
  return store.read().schemaPermissionRequests.find((record) => record.id === id) ?? null;
}

export function updateSchemaPermissionRequest(store, id, patch) {
  const state = store.read();
  const index = state.schemaPermissionRequests.findIndex((record) => record.id === id);
  if (index < 0) return null;
  const allowedStatuses = new Set(['requested', 'approved', 'rejected', 'cancelled']);
  if (patch.status !== undefined && !allowedStatuses.has(patch.status)) {
    throw new Error(`Unsupported schema permission request status: ${patch.status}`);
  }
  const mutableKeys = ['status', 'transactionSignature', 'rejectionReason'];
  const next = { ...state.schemaPermissionRequests[index] };
  for (const key of mutableKeys) {
    if (Object.prototype.hasOwnProperty.call(patch, key)) {
      next[key] = patch[key] === undefined ? next[key] : patch[key];
    }
  }
  next.updatedAt = new Date().toISOString();
  state.schemaPermissionRequests[index] = next;
  store.write(state);
  return next;
}

export function upsertTreeLeaf(store, input) {
  const treeAddress = requiredString(input.treeAddress, 'treeAddress');
  const leaf = normalizeSchemaHash(input.leaf ?? input.commitment);
  const leafIndex = integer(input.leafIndex, 'leafIndex');
  const slot = integer(input.slot ?? 0, 'slot');
  const source = nullableString(input.source) ?? 'manual';
  const signature = nullableString(input.signature);
  const eventIndex = input.eventIndex === undefined || input.eventIndex === null
    ? null
    : integer(input.eventIndex, 'eventIndex');
  const state = store.read();
  const existing = state.treeLeaves.find((entry) =>
    entry.treeAddress === treeAddress
    && entry.leafIndex === leafIndex
    && entry.leaf === leaf
  );
  if (existing) return existing;
  const record = {
    id: randomUUID(),
    treeAddress,
    leaf,
    leafIndex,
    slot,
    signature,
    eventIndex,
    source,
    subject: nullableString(input.subject),
    schemaHash: nullableString(input.schemaHash),
    indexedAt: new Date().toISOString(),
  };
  state.treeLeaves.push(record);
  store.write(state);
  return record;
}

export function nextLeafIndex(store, treeAddress) {
  const leaves = store.read().treeLeaves.filter((entry) => entry.treeAddress === treeAddress);
  if (leaves.length === 0) return 0;
  return Math.max(...leaves.map((entry) => entry.leafIndex)) + 1;
}

export function activeLeavesForTree(store, treeAddress) {
  const latestByIndex = new Map();
  const ordered = store.read().treeLeaves
    .filter((entry) => entry.treeAddress === treeAddress)
    .sort((a, b) =>
      a.slot - b.slot
      || (a.eventIndex ?? 0) - (b.eventIndex ?? 0)
      || String(a.indexedAt).localeCompare(String(b.indexedAt)),
    );
  for (const entry of ordered) {
    latestByIndex.set(entry.leafIndex, entry);
  }
  return Array.from(latestByIndex.values()).sort((a, b) => a.leafIndex - b.leafIndex);
}

function normalizeState(value) {
  return {
    credentialRequests: Array.isArray(value.credentialRequests) ? value.credentialRequests : [...EMPTY_STATE.credentialRequests],
    proofRequests: Array.isArray(value.proofRequests) ? value.proofRequests : [...EMPTY_STATE.proofRequests],
    customSchemaRequests: Array.isArray(value.customSchemaRequests) ? value.customSchemaRequests : [...EMPTY_STATE.customSchemaRequests],
    schemaPermissionRequests: Array.isArray(value.schemaPermissionRequests) ? value.schemaPermissionRequests : [...EMPTY_STATE.schemaPermissionRequests],
    treeLeaves: Array.isArray(value.treeLeaves) ? value.treeLeaves : [...EMPTY_STATE.treeLeaves],
    processedTransactions: Array.isArray(value.processedTransactions) ? value.processedTransactions : [...EMPTY_STATE.processedTransactions],
    metadata: {
      ...EMPTY_STATE.metadata,
      ...(value.metadata && typeof value.metadata === 'object' ? value.metadata : {}),
    },
  };
}

function requestMatches(record, filters) {
  for (const key of ['issuerAuthority', 'issuerAccount', 'proposerAuthority', 'holderPublicKeyX', 'schemaHash', 'status']) {
    const expected = filters[key];
    if (expected && String(record[key]) !== String(expected)) return false;
  }
  return true;
}

function normalizeSchemaHash(value) {
  const normalized = String(value ?? '').trim().replace(/^0x/i, '').toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(normalized)) {
    throw new Error('schemaHash/leaf must be a 32-byte hex string');
  }
  return normalized;
}

function requiredString(value, name) {
  const text = String(value ?? '').trim();
  if (!text) throw new Error(`Missing required field: ${name}`);
  return text;
}

function nullableString(value) {
  const text = value === undefined || value === null ? '' : String(value).trim();
  return text ? text : null;
}

function integer(value, name) {
  const number = Number(value);
  if (!Number.isSafeInteger(number) || number < 0) {
    throw new Error(`${name} must be a non-negative safe integer`);
  }
  return number;
}

function validateSchemaName(value) {
  const text = requiredString(value, 'name');
  if (text.length > 64) throw new Error('schema name must be 64 characters or less');
  if (!/^[a-z][a-z0-9_]*$/.test(text)) {
    throw new Error('schema name must use lowercase letters, numbers, and underscores, starting with a letter');
  }
  return text;
}

function validateSchemaCategory(value) {
  const text = requiredString(value, 'category');
  if (text.length > 64) throw new Error('schema category must be 64 characters or less');
  return text;
}

function validateFieldName(value, name) {
  const text = requiredString(value, name);
  if (text.length > 32) throw new Error(`${name} must be 32 characters or less`);
  if (!/^[a-z][a-z0-9_]*$/.test(text)) {
    throw new Error(`${name} must use lowercase letters, numbers, and underscores, starting with a letter`);
  }
  return text;
}

function validateFieldType(value, name) {
  const text = requiredString(value, name);
  if (!['uint64', 'enum', 'timestamp', 'boolean'].includes(text)) {
    throw new Error(`${name} must be one of uint64, enum, timestamp, boolean`);
  }
  return text;
}
