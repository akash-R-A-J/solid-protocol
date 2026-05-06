import { mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';
import { randomUUID } from 'node:crypto';

const EMPTY_STATE = Object.freeze({
  credentialRequests: [],
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
    treeLeaves: Array.isArray(value.treeLeaves) ? value.treeLeaves : [...EMPTY_STATE.treeLeaves],
    processedTransactions: Array.isArray(value.processedTransactions) ? value.processedTransactions : [...EMPTY_STATE.processedTransactions],
    metadata: {
      ...EMPTY_STATE.metadata,
      ...(value.metadata && typeof value.metadata === 'object' ? value.metadata : {}),
    },
  };
}

function requestMatches(record, filters) {
  for (const key of ['issuerAuthority', 'issuerAccount', 'holderPublicKeyX', 'schemaHash', 'status']) {
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
