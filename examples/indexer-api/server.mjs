import { createServer } from 'node:http';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { randomUUID } from 'node:crypto';
import { dirname, resolve } from 'node:path';

const repoRoot = resolve(new URL('../..', import.meta.url).pathname);
const manifest = JSON.parse(readFileSync(resolve(repoRoot, 'deployments/devnet.json'), 'utf8'));
const port = Number(process.env.PORT ?? '8787');
const requestStorePath = resolve(
  process.env.SOLID_REQUEST_STORE ?? resolve(repoRoot, '.solid-indexer/credential-requests.json'),
);

function json(res, status, body) {
  const payload = JSON.stringify(body, null, 2);
  res.writeHead(status, {
    'content-type': 'application/json; charset=utf-8',
    'cache-control': 'no-store',
    'access-control-allow-origin': '*',
    'access-control-allow-methods': 'GET,POST,PATCH,OPTIONS',
    'access-control-allow-headers': 'content-type,accept',
  });
  res.end(payload);
}

function notIndexed(res, resource) {
  json(res, 503, {
    ok: false,
    error: 'NOT_INDEXED',
    resource,
    message: 'This reference server exposes the SolID indexer contract and refuses to return fake proofs.',
  });
}

async function readBody(req) {
  const chunks = [];
  for await (const chunk of req) chunks.push(chunk);
  if (chunks.length === 0) return {};
  return JSON.parse(Buffer.concat(chunks).toString('utf8'));
}

function readRequests() {
  try {
    return JSON.parse(readFileSync(requestStorePath, 'utf8'));
  } catch {
    return [];
  }
}

function writeRequests(requests) {
  mkdirSync(dirname(requestStorePath), { recursive: true });
  writeFileSync(requestStorePath, JSON.stringify(requests, null, 2));
}

function requestMatches(record, url) {
  for (const key of ['issuerAuthority', 'issuerAccount', 'holderPublicKeyX', 'schemaHash', 'status']) {
    const expected = url.searchParams.get(key);
    if (expected && String(record[key]) !== expected) return false;
  }
  return true;
}

function validateCreateCredentialRequest(input) {
  const required = [
    'schemaHash',
    'schemaName',
    'schemaVersion',
    'issuerAuthority',
    'holderChannelPublicKey',
    'holderPublicKeyX',
    'holderPublicKeyY',
  ];
  for (const key of required) {
    if (input[key] === undefined || input[key] === null || String(input[key]).trim() === '') {
      throw new Error(`Missing credential request field: ${key}`);
    }
  }
  if (!/^[0-9a-fA-F]{64}$/.test(String(input.schemaHash).replace(/^0x/i, ''))) {
    throw new Error('schemaHash must be a 32-byte hex string');
  }
  if (!Number.isInteger(Number(input.schemaVersion))) {
    throw new Error('schemaVersion must be an integer');
  }
}

createServer(async (req, res) => {
  const url = new URL(req.url ?? '/', `http://${req.headers.host ?? 'localhost'}`);
  if (req.method === 'OPTIONS') {
    json(res, 204, {});
    return;
  }

  if (url.pathname === '/v1/health') {
    json(res, 200, {
      ok: true,
      network: manifest.network,
      slot: 0,
      indexed_slot: 0,
      lag_slots: 0,
      mode: 'contract-only',
    });
    return;
  }

  if (url.pathname === '/v1/manifest') {
    json(res, 200, manifest);
    return;
  }

  if (url.pathname === '/v1/schemas') {
    json(res, 200, manifest.schemas ?? []);
    return;
  }

  if (url.pathname === '/v1/issuers') {
    notIndexed(res, 'issuers');
    return;
  }

  if (url.pathname === '/v1/roots/current') {
    json(res, 200, {
      slot: 0,
      global_state_root: null,
      issuer_tree_root: manifest.trees?.issuer_tree?.current_root ?? null,
      schema_tree_roots: [],
    });
    return;
  }

  if (/^\/v1\/merkle-proof\//.test(url.pathname)) {
    notIndexed(res, 'merkle-proof');
    return;
  }

  if (url.pathname === '/v1/credential-requests' && req.method === 'GET') {
    const requests = readRequests()
      .filter((record) => requestMatches(record, url))
      .sort((a, b) => String(b.updatedAt).localeCompare(String(a.updatedAt)));
    json(res, 200, { requests });
    return;
  }

  if (url.pathname === '/v1/credential-requests' && req.method === 'POST') {
    try {
      const input = await readBody(req);
      validateCreateCredentialRequest(input);
      const now = new Date().toISOString();
      const record = {
        id: randomUUID(),
        network: input.network ?? manifest.network ?? 'devnet',
        schemaHash: String(input.schemaHash).replace(/^0x/i, '').toLowerCase(),
        schemaName: String(input.schemaName),
        schemaVersion: Number(input.schemaVersion),
        issuerAuthority: String(input.issuerAuthority),
        issuerAccount: input.issuerAccount ? String(input.issuerAccount) : null,
        holderChannelPublicKey: String(input.holderChannelPublicKey),
        holderPublicKeyX: String(input.holderPublicKeyX),
        holderPublicKeyY: String(input.holderPublicKeyY),
        holderLabel: input.holderLabel ? String(input.holderLabel) : null,
        status: 'requested',
        requestedAt: now,
        updatedAt: now,
        notes: input.notes ? String(input.notes) : null,
        rejectionReason: null,
        envelopeJson: null,
        commitment: null,
        transactionSignature: null,
      };
      const requests = [record, ...readRequests()];
      writeRequests(requests);
      json(res, 201, record);
    } catch (error) {
      json(res, 400, { ok: false, error: 'INVALID_CREDENTIAL_REQUEST', message: error.message });
    }
    return;
  }

  if (/^\/v1\/credential-requests\/[^/]+$/.test(url.pathname)) {
    const id = decodeURIComponent(url.pathname.split('/')[3] ?? '');
    const requests = readRequests();
    const index = requests.findIndex((record) => record.id === id);
    if (index < 0) {
      json(res, 404, { ok: false, error: 'CREDENTIAL_REQUEST_NOT_FOUND', id });
      return;
    }
    if (req.method === 'GET') {
      json(res, 200, requests[index]);
      return;
    }
    if (req.method === 'PATCH') {
      try {
        const input = await readBody(req);
        const allowedStatuses = new Set(['requested', 'reviewing', 'approved', 'rejected', 'issued', 'cancelled']);
        if (input.status !== undefined && !allowedStatuses.has(input.status)) {
          throw new Error(`Unsupported credential request status: ${input.status}`);
        }
        const updated = {
          ...requests[index],
          ...Object.fromEntries(
            ['status', 'notes', 'rejectionReason', 'envelopeJson', 'commitment', 'transactionSignature']
              .filter((key) => Object.prototype.hasOwnProperty.call(input, key))
              .map((key) => [key, input[key] === undefined ? requests[index][key] : input[key]]),
          ),
          updatedAt: new Date().toISOString(),
        };
        requests[index] = updated;
        writeRequests(requests);
        json(res, 200, updated);
      } catch (error) {
        json(res, 400, { ok: false, error: 'INVALID_CREDENTIAL_REQUEST_UPDATE', message: error.message });
      }
      return;
    }
  }

  if (/^\/v1\/issuers\/[^/]+\/proof$/.test(url.pathname)) {
    notIndexed(res, 'issuer-proof');
    return;
  }

  if (/^\/v1\/schemas\/[^/]+\/tree$/.test(url.pathname)) {
    const schemaHash = decodeURIComponent(url.pathname.split('/')[3] ?? '');
    const tree = (manifest.trees?.schema_trees ?? []).find((entry) => entry.schema_hash === schemaHash);
    if (!tree) {
      json(res, 404, { ok: false, error: 'SCHEMA_TREE_NOT_FOUND', schema_hash: schemaHash });
      return;
    }
    json(res, 200, tree);
    return;
  }

  json(res, 404, { ok: false, error: 'NOT_FOUND' });
}).listen(port, () => {
  console.log(`SolID indexer contract server listening on http://127.0.0.1:${port}`);
});
