import assert from 'node:assert/strict';
import { Readable } from 'node:stream';
import { beforeEach, describe, it } from 'node:test';
import { createIndexerHandler } from '../src/service.mjs';
import { MemoryIndexerStore } from '../src/store.mjs';

const TREE_ADDRESS = '4mhWLGb2KAtF1bY2mdrGb37xhAUpmRsL9bgzLRjE35sc';
const SCHEMA_HASH = '6b5014bf611a025a4693b196a517ece9f2d0672672a2eb38d7a50481474e6823';
const LEAF = '902efdd15064fcafa0da19d8bb0f6ed516f14b245fe92d3f5279f063b0a6e81e';

const manifest = {
  schema_version: 1,
  network: 'devnet',
  cluster: 'http://127.0.0.1:8899',
  websocket_cluster: null,
  deployed_at: null,
  git_commit: null,
  deployer: { address: null },
  programs: {
    schema_registry: {
      program_id: '4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1',
      upgrade_authority: null,
      idl_metadata: null,
      binary: '',
      description: '',
    },
    issuer_registry: {
      program_id: '5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx',
      upgrade_authority: null,
      idl_metadata: null,
      binary: '',
      description: '',
    },
    zk_verifier: {
      program_id: 'DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb',
      upgrade_authority: null,
      idl_metadata: null,
      binary: '',
      description: '',
    },
  },
  artifacts: { base_url: null, manifest_url: null, items: {} },
  indexer: { url: null, health_path: '/v1/health', api_version: 'v1' },
  console: { url: null },
  wallet: { release_url: null },
  schemas: [{
    name: 'basic_identity_v2',
    version: 2,
    schema_hash: SCHEMA_HASH,
    schema_pda: null,
    tree_address: TREE_ADDRESS,
    tree_depth: 2,
    fields: ['age'],
    predicates: ['GTE'],
  }],
  trees: {
    global_state_tree: { current_root: null },
    issuer_tree: {
      tree_address: 'FajCWko9tc6dhdPtwrS5kfkehPoEV8pn5LdL4kLL7k7f',
      binding_pda: null,
      current_root: null,
      current_root_slot: null,
    },
    schema_trees: [{
      schema_hash: SCHEMA_HASH,
      tree_address: TREE_ADDRESS,
      binding_pda: null,
      current_root: null,
      current_root_slot: null,
    }],
  },
  pdas: {},
  toolchain: {},
};

describe('SolID indexer API', () => {
  let handler;

  beforeEach(() => {
    handler = createIndexerHandler({
      manifest,
      store: new MemoryIndexerStore(),
      writeToken: 'test-token',
    });
  });

  it('creates, lists, and updates credential requests', async () => {
    const created = await post('/v1/credential-requests', {
      schemaHash: SCHEMA_HASH,
      schemaName: 'basic_identity_v2',
      schemaVersion: 2,
      issuerAuthority: 'GwqjvUSmPKeNnPSWzFBPnURGXVkpLAzdujMuPCEPMyNi',
      holderChannelPublicKey: 'holder-channel-key',
      holderPublicKeyX: '1',
      holderPublicKeyY: '2',
      notes: 'KYC checked off platform',
    });

    assert.equal(created.status, 'requested');
    assert.equal(created.schemaHash, SCHEMA_HASH);

    const listed = await get(`/v1/credential-requests?issuerAuthority=${created.issuerAuthority}`);
    assert.equal(listed.requests.length, 1);
    assert.equal(listed.requests[0].id, created.id);

    const updated = await patch(`/v1/credential-requests/${created.id}`, {
      status: 'issued',
      commitment: LEAF,
      transactionSignature: 'demo-signature',
    });
    assert.equal(updated.status, 'issued');
    assert.equal(updated.commitment, LEAF);
  });

  it('requires the write token for tree ingestion and never returns fake proofs', async () => {
    const denied = await request('/v1/tree-leaves', {
      method: 'POST',
      body: { treeAddress: TREE_ADDRESS, leaf: LEAF, leafIndex: 0, slot: 7 },
    });
    assert.equal(denied.status, 401);

    const missing = await request(`/v1/merkle-proof/${TREE_ADDRESS}/${LEAF}`);
    assert.equal(missing.status, 404);
    assert.equal(missing.body.error, 'LEAF_NOT_INDEXED');
  });

  it('indexes leaves idempotently and returns a real Poseidon Merkle proof', async () => {
    const first = await post('/v1/tree-leaves', {
      treeAddress: TREE_ADDRESS,
      leaf: LEAF,
      leafIndex: 0,
      slot: 7,
      source: 'test',
    }, true);
    const second = await post('/v1/tree-leaves', {
      treeAddress: TREE_ADDRESS,
      leaf: LEAF,
      leafIndex: 0,
      slot: 7,
      source: 'test',
    }, true);
    assert.equal(second.id, first.id);

    const proof = await get(`/v1/merkle-proof/${TREE_ADDRESS}/${LEAF}`);
    assert.match(proof.root, /^[0-9a-f]{64}$/);
    assert.equal(proof.siblings.length, 2);
    assert.deepEqual(proof.pathIndices, [0, 0]);
    assert.equal(proof.leafIndex, 0);
    assert.equal(proof.slot, 7);
  });

  async function get(path) {
    const response = await request(path);
    assert.equal(response.ok, true, `${path} returned ${response.status}: ${JSON.stringify(response.body)}`);
    return response.body;
  }

  async function post(path, body, authorized = false) {
    const response = await request(path, {
      method: 'POST',
      body,
      headers: authorized ? { authorization: 'Bearer test-token' } : {},
    });
    assert.equal(response.ok, true, `${path} returned ${response.status}: ${JSON.stringify(response.body)}`);
    return response.body;
  }

  async function patch(path, body) {
    const response = await request(path, { method: 'PATCH', body });
    assert.equal(response.ok, true, `${path} returned ${response.status}: ${JSON.stringify(response.body)}`);
    return response.body;
  }

  async function request(path, options = {}) {
    const payload = options.body === undefined ? '' : JSON.stringify(options.body);
    const req = Readable.from(payload ? [Buffer.from(payload)] : []);
    req.method = options.method ?? 'GET';
    req.url = path;
    req.headers = {
      host: 'solid-indexer.test',
      accept: 'application/json',
      'content-type': 'application/json',
      ...(options.headers ?? {}),
    };

    return new Promise((resolve) => {
      const res = {
        status: 200,
        headers: {},
        writeHead(status, headers) {
          this.status = status;
          this.headers = headers;
        },
        end(body) {
          const parsed = body ? JSON.parse(body) : null;
          resolve({
            status: this.status,
            ok: this.status >= 200 && this.status < 300,
            headers: this.headers,
            body: parsed,
          });
        },
      };
      handler(req, res);
    });
  }
});
