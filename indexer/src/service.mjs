import { createHash } from 'node:crypto';
import { PublicKey } from '@solana/web3.js';
import {
  decodeIssuerAccount,
  decodeSchemaAccount,
  ISSUER_ACCOUNT_DISCRIMINATOR_BASE58,
  SCHEMA_ACCOUNT_DISCRIMINATOR_BASE58,
} from '@solid-protocol/issuer/registry';
import { buildMerkleProof } from './proofs.mjs';
import {
  createCredentialRequest,
  createCustomSchemaRequest,
  createProofRequest,
  createSchemaPermissionRequest,
  getCredentialRequest,
  getCustomSchemaRequest,
  getProofRequest,
  getSchemaPermissionRequest,
  listCredentialRequests,
  listCustomSchemaRequests,
  listProofRequests,
  listSchemaPermissionRequests,
  nextLeafIndex,
  updateCredentialRequest,
  updateCustomSchemaRequest,
  updateProofRequest,
  updateSchemaPermissionRequest,
  upsertTreeLeaf,
} from './store.mjs';
import { normalizeHex32, writeJsonResponse } from './encoding.mjs';

export function createIndexerHandler({ manifest, store, connection = null, writeToken = '' }) {
  const manifestHash = createHash('sha256').update(JSON.stringify(manifest)).digest('hex');

  return async function handle(req, res) {
    try {
      const url = new URL(req.url ?? '/', `http://${req.headers.host ?? 'localhost'}`);
      if (req.method === 'OPTIONS') {
        writeJsonResponse(res, 204, {});
        return;
      }

      if (url.pathname === '/.well-known/solid-protocol.json' || url.pathname === '/v1/manifest') {
        writeJsonResponse(res, 200, manifest);
        return;
      }

      if (url.pathname === '/v1/health') {
        const rpcSlot = connection ? await connection.getSlot('confirmed').catch(() => null) : null;
        const state = store.read();
        const lastProcessedSlot = state.treeLeaves.reduce((max, entry) => Math.max(max, entry.slot ?? 0), 0);
        writeJsonResponse(res, 200, {
          ok: true,
          network: manifest.network,
          lastProcessedSlot,
          rpcSlot,
          lagSlots: rpcSlot === null ? null : Math.max(0, rpcSlot - lastProcessedSlot),
          indexedLeaves: state.treeLeaves.length,
          credentialRequests: state.credentialRequests.length,
          proofRequests: state.proofRequests.length,
          customSchemaRequests: state.customSchemaRequests.length,
          schemaPermissionRequests: state.schemaPermissionRequests.length,
          manifestHash,
        });
        return;
      }

      if (url.pathname === '/v1/schemas') {
        if (connection && url.searchParams.get('source') === 'chain') {
          writeJsonResponse(res, 200, await fetchSchemas(connection, manifest));
          return;
        }
        writeJsonResponse(res, 200, manifest.schemas ?? []);
        return;
      }

      if (url.pathname === '/v1/roots/current') {
        writeJsonResponse(res, 200, currentRoots(store, manifest));
        return;
      }

      if (url.pathname === '/v1/issuers') {
        if (!connection) {
          writeJsonResponse(res, 503, {
            ok: false,
            error: 'RPC_NOT_CONFIGURED',
            message: 'Set SOLID_RPC_URL so the indexer can read live issuer accounts.',
          });
          return;
        }
        writeJsonResponse(res, 200, await fetchIssuers(connection, manifest));
        return;
      }

      if (url.pathname === '/v1/credential-requests' && req.method === 'GET') {
        writeJsonResponse(res, 200, {
          requests: listCredentialRequests(store, Object.fromEntries(url.searchParams.entries())),
        });
        return;
      }

      if (url.pathname === '/v1/credential-requests' && req.method === 'POST') {
        const body = await readJsonBody(req);
        const record = createCredentialRequest(store, body, manifest.network ?? 'devnet');
        writeJsonResponse(res, 201, record);
        return;
      }

      if (/^\/v1\/credential-requests\/[^/]+$/.test(url.pathname)) {
        const id = decodeURIComponent(url.pathname.split('/')[3] ?? '');
        const existing = getCredentialRequest(store, id);
        if (!existing) {
          writeJsonResponse(res, 404, { ok: false, error: 'CREDENTIAL_REQUEST_NOT_FOUND', id });
          return;
        }
        if (req.method === 'GET') {
          writeJsonResponse(res, 200, existing);
          return;
        }
        if (req.method === 'PATCH') {
          const updated = updateCredentialRequest(store, id, await readJsonBody(req));
          writeJsonResponse(res, 200, updated);
          return;
        }
      }

      if (url.pathname === '/v1/proof-requests' && req.method === 'GET') {
        writeJsonResponse(res, 200, {
          requests: listProofRequests(store, Object.fromEntries(url.searchParams.entries())),
        });
        return;
      }

      if (url.pathname === '/v1/proof-requests' && req.method === 'POST') {
        const body = await readJsonBody(req);
        const record = createProofRequest(store, body, manifest.network ?? 'devnet');
        writeJsonResponse(res, 201, record);
        return;
      }

      if (/^\/v1\/proof-requests\/[^/]+$/.test(url.pathname)) {
        const id = decodeURIComponent(url.pathname.split('/')[3] ?? '');
        const existing = getProofRequest(store, id);
        if (!existing) {
          writeJsonResponse(res, 404, { ok: false, error: 'PROOF_REQUEST_NOT_FOUND', id });
          return;
        }
        if (req.method === 'GET') {
          writeJsonResponse(res, 200, existing);
          return;
        }
        if (req.method === 'PATCH') {
          const updated = updateProofRequest(store, id, await readJsonBody(req));
          writeJsonResponse(res, 200, updated);
          return;
        }
      }

      if (url.pathname === '/v1/schema-permission-requests' && req.method === 'GET') {
        writeJsonResponse(res, 200, {
          requests: listSchemaPermissionRequests(store, Object.fromEntries(url.searchParams.entries())),
        });
        return;
      }

      if (url.pathname === '/v1/schema-permission-requests' && req.method === 'POST') {
        const body = await readJsonBody(req);
        const record = createSchemaPermissionRequest(store, body, manifest.network ?? 'devnet');
        writeJsonResponse(res, 201, record);
        return;
      }

      if (url.pathname === '/v1/custom-schema-requests' && req.method === 'GET') {
        writeJsonResponse(res, 200, {
          requests: listCustomSchemaRequests(store, Object.fromEntries(url.searchParams.entries())),
        });
        return;
      }

      if (url.pathname === '/v1/custom-schema-requests' && req.method === 'POST') {
        const body = await readJsonBody(req);
        const record = createCustomSchemaRequest(store, body, manifest.network ?? 'devnet');
        writeJsonResponse(res, 201, record);
        return;
      }

      if (/^\/v1\/custom-schema-requests\/[^/]+$/.test(url.pathname)) {
        const id = decodeURIComponent(url.pathname.split('/')[3] ?? '');
        const existing = getCustomSchemaRequest(store, id);
        if (!existing) {
          writeJsonResponse(res, 404, { ok: false, error: 'CUSTOM_SCHEMA_REQUEST_NOT_FOUND', id });
          return;
        }
        if (req.method === 'GET') {
          writeJsonResponse(res, 200, existing);
          return;
        }
        if (req.method === 'PATCH') {
          const updated = updateCustomSchemaRequest(store, id, await readJsonBody(req));
          writeJsonResponse(res, 200, updated);
          return;
        }
      }

      if (/^\/v1\/schema-permission-requests\/[^/]+$/.test(url.pathname)) {
        const id = decodeURIComponent(url.pathname.split('/')[3] ?? '');
        const existing = getSchemaPermissionRequest(store, id);
        if (!existing) {
          writeJsonResponse(res, 404, { ok: false, error: 'SCHEMA_PERMISSION_REQUEST_NOT_FOUND', id });
          return;
        }
        if (req.method === 'GET') {
          writeJsonResponse(res, 200, existing);
          return;
        }
        if (req.method === 'PATCH') {
          const updated = updateSchemaPermissionRequest(store, id, await readJsonBody(req));
          writeJsonResponse(res, 200, updated);
          return;
        }
      }

      if (/^\/v1\/merkle-proof\/[^/]+\/[^/]+$/.test(url.pathname) && req.method === 'GET') {
        const [, , , treeAddress, leaf] = url.pathname.split('/');
        try {
          writeJsonResponse(res, 200, await buildMerkleProof(
            store,
            manifest,
            decodeURIComponent(treeAddress),
            decodeURIComponent(leaf),
          ));
        } catch (error) {
          if (error.code === 'LEAF_NOT_INDEXED') {
            writeJsonResponse(res, 404, { ok: false, error: 'LEAF_NOT_INDEXED', message: error.message });
          } else {
            throw error;
          }
        }
        return;
      }

      if (/^\/v1\/issuers\/[^/]+\/proof$/.test(url.pathname) && req.method === 'GET') {
        const issuer = decodeURIComponent(url.pathname.split('/')[3] ?? '');
        const issuerTree = manifest.trees?.issuer_tree?.tree_address;
        if (!issuerTree) {
          writeJsonResponse(res, 503, {
            ok: false,
            error: 'ISSUER_TREE_NOT_CONFIGURED',
            message: 'Manifest is missing trees.issuer_tree.tree_address.',
          });
          return;
        }
        const leafRecord = store.read().treeLeaves.find((entry) =>
          entry.treeAddress === issuerTree && entry.subject === issuer,
        );
        if (!leafRecord) {
          writeJsonResponse(res, 404, {
            ok: false,
            error: 'ISSUER_NOT_INDEXED',
            message: `Issuer ${issuer} has no indexed issuer-tree leaf.`,
          });
          return;
        }
        writeJsonResponse(res, 200, await buildMerkleProof(store, manifest, issuerTree, leafRecord.leaf));
        return;
      }

      if (url.pathname === '/v1/tree-leaves' && req.method === 'POST') {
        requireWriteAuth(req, writeToken);
        const body = await readJsonBody(req);
        const record = upsertTreeLeaf(store, body);
        writeJsonResponse(res, 201, record);
        return;
      }

      if (url.pathname === '/v1/events/credential-issued' && req.method === 'POST') {
        requireWriteAuth(req, writeToken);
        const body = await readJsonBody(req);
        const treeAddress = requiredString(body.treeAddress ?? body.merkleTree, 'treeAddress');
        const leafIndex = body.leafIndex === undefined || body.leafIndex === null
          ? nextLeafIndex(store, treeAddress)
          : Number(body.leafIndex);
        const record = upsertTreeLeaf(store, {
          treeAddress,
          leaf: normalizeHex32(body.commitment, 'commitment'),
          leafIndex,
          slot: body.slot ?? 0,
          signature: body.signature,
          eventIndex: body.eventIndex,
          source: 'credential-issued',
          subject: body.holderPublicKeyX ?? body.holder ?? null,
          schemaHash: body.schemaHash ?? null,
        });
        writeJsonResponse(res, 201, record);
        return;
      }

      if (/^\/v1\/schemas\/[^/]+\/tree$/.test(url.pathname)) {
        const schemaHash = decodeURIComponent(url.pathname.split('/')[3] ?? '');
        const tree = (manifest.trees?.schema_trees ?? []).find((entry) => entry.schema_hash === schemaHash)
          ?? (manifest.schemas ?? []).find((entry) => entry.schema_hash === schemaHash);
        if (!tree) {
          writeJsonResponse(res, 404, { ok: false, error: 'SCHEMA_TREE_NOT_FOUND', schemaHash });
          return;
        }
        writeJsonResponse(res, 200, tree);
        return;
      }

      writeJsonResponse(res, 404, { ok: false, error: 'NOT_FOUND' });
    } catch (error) {
      writeJsonResponse(res, statusForError(error), {
        ok: false,
        error: error.code ?? 'INDEXER_ERROR',
        message: error.message,
      });
    }
  };
}

function currentRoots(store, manifest) {
  const state = store.read();
  const latestByTree = new Map();
  for (const entry of state.treeLeaves) {
    const previous = latestByTree.get(entry.treeAddress);
    if (!previous || (entry.slot ?? 0) >= (previous.slot ?? 0)) {
      latestByTree.set(entry.treeAddress, entry);
    }
  }
  return {
    slot: Math.max(0, ...state.treeLeaves.map((entry) => entry.slot ?? 0)),
    global_state_root: manifest.trees?.global_state_tree?.current_root ?? null,
    issuer_tree_root: manifest.trees?.issuer_tree?.current_root ?? null,
    schema_tree_roots: (manifest.trees?.schema_trees ?? []).map((entry) => ({
      schema_hash: entry.schema_hash,
      tree_address: entry.tree_address,
      root: entry.current_root ?? latestByTree.get(entry.tree_address)?.leaf ?? null,
    })),
  };
}

async function fetchIssuers(connection, manifest) {
  const programId = new PublicKey(manifest.programs.issuer_registry.program_id);
  const accounts = await connection.getProgramAccounts(programId, {
    filters: [{ memcmp: { offset: 0, bytes: ISSUER_ACCOUNT_DISCRIMINATOR_BASE58 } }],
  });
  return accounts.map((entry) => decodeIssuerAccount(entry.pubkey, entry.account.data));
}

async function fetchSchemas(connection, manifest) {
  const programId = new PublicKey(manifest.programs.schema_registry.program_id);
  const accounts = await connection.getProgramAccounts(programId, {
    filters: [{ memcmp: { offset: 0, bytes: SCHEMA_ACCOUNT_DISCRIMINATOR_BASE58 } }],
  });
  return accounts.map((entry) => decodeSchemaAccount(entry.pubkey, entry.account.data));
}

async function readJsonBody(req) {
  const chunks = [];
  for await (const chunk of req) chunks.push(chunk);
  if (chunks.length === 0) return {};
  return JSON.parse(Buffer.concat(chunks).toString('utf8'));
}

function requireWriteAuth(req, writeToken) {
  if (!writeToken) return;
  const header = req.headers.authorization ?? '';
  if (header !== `Bearer ${writeToken}`) {
    const error = new Error('Missing or invalid write token.');
    error.code = 'UNAUTHORIZED';
    throw error;
  }
}

function requiredString(value, name) {
  const text = String(value ?? '').trim();
  if (!text) throw new Error(`Missing required field: ${name}`);
  return text;
}

function statusForError(error) {
  if (error.code === 'UNAUTHORIZED') return 401;
  if (/Missing|required|invalid|must be|Unsupported/.test(error.message)) return 400;
  return 500;
}
