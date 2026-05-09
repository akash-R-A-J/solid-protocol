import assert from 'node:assert/strict';
import { Readable } from 'node:stream';
import { beforeEach, describe, it } from 'node:test';
import { computeIdentityState, initWasm } from '@solid-protocol/core';
import { bytesToHex, hexToBytes32, writeJsonResponse } from '../src/encoding.mjs';
import { createIndexerHandler } from '../src/service.mjs';
import { treeDepthFor } from '../src/proofs.mjs';
import { MemoryIndexerStore, upsertTreeLeaf } from '../src/store.mjs';

const TREE_ADDRESS = '4mhWLGb2KAtF1bY2mdrGb37xhAUpmRsL9bgzLRjE35sc';
const CUSTOM_TREE_ADDRESS = '6CpYRkZMmQyS7ourERFU112jMpbLebam9iXjdafJpnJS';
const GLOBAL_TREE_ADDRESS = '68Twk6dXwbahut6VDv1wSRkQMdFZRaeTbhUMNPiaJM8o';
const SCHEMA_HASH = '6b5014bf611a025a4693b196a517ece9f2d0672672a2eb38d7a50481474e6823';
const CUSTOM_SCHEMA_HASH = '1f233b8f0cb1a1bbe747b3b7a8ec8b424025714dc9f4a9b355db83d60568f5eb';
const LEAF = '902efdd15064fcafa0da19d8bb0f6ed516f14b245fe92d3f5279f063b0a6e81e';
const HOLDER_PUBLIC_KEY_X = '1111111111111111111111111111111111111111111111111111111111111111';
const HOLDER_PUBLIC_KEY_Y = '2222222222222222222222222222222222222222222222222222222222222222';

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
    global_state_tree: { tree_address: GLOBAL_TREE_ADDRESS, current_root: null },
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
      holderPublicKeyX: HOLDER_PUBLIC_KEY_X,
      holderPublicKeyY: HOLDER_PUBLIC_KEY_Y,
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

    const proof = await get(`/v1/merkle-proof/${TREE_ADDRESS}/${LEAF}`);
    assert.equal(proof.leafIndex, 0);
    assert.match(proof.root, /^[0-9a-f]{64}$/);

    await initWasm();
    const holderIdentityLeaf = bytesToHex(computeIdentityState(
      hexToBytes32(HOLDER_PUBLIC_KEY_X, 'holderPublicKeyX'),
      hexToBytes32(HOLDER_PUBLIC_KEY_Y, 'holderPublicKeyY'),
      0n,
    ));
    const globalProof = await get(`/v1/merkle-proof/${GLOBAL_TREE_ADDRESS}/${holderIdentityLeaf}`);
    assert.equal(globalProof.leafIndex, 0);
    assert.match(globalProof.root, /^[0-9a-f]{64}$/);
  });

  it('re-appends holder identity leaves when stale local rows are no longer active', async () => {
    const created = await post('/v1/credential-requests', {
      schemaHash: SCHEMA_HASH,
      schemaName: 'basic_identity_v2',
      schemaVersion: 2,
      issuerAuthority: 'GwqjvUSmPKeNnPSWzFBPnURGXVkpLAzdujMuPCEPMyNi',
      holderChannelPublicKey: 'holder-channel-key',
      holderPublicKeyX: HOLDER_PUBLIC_KEY_X,
      holderPublicKeyY: HOLDER_PUBLIC_KEY_Y,
      notes: 'KYC checked off platform',
    });
    await patch(`/v1/credential-requests/${created.id}`, {
      status: 'issued',
      commitment: LEAF,
      transactionSignature: 'demo-signature',
    });

    await initWasm();
    const holderIdentityLeaf = bytesToHex(computeIdentityState(
      hexToBytes32(HOLDER_PUBLIC_KEY_X, 'holderPublicKeyX'),
      hexToBytes32(HOLDER_PUBLIC_KEY_Y, 'holderPublicKeyY'),
      0n,
    ));
    const activeChainLeaf = '3333333333333333333333333333333333333333333333333333333333333333';
    await post('/v1/tree-leaves', {
      treeAddress: GLOBAL_TREE_ADDRESS,
      leaf: activeChainLeaf,
      leafIndex: 0,
      slot: 10,
      source: 'global-root-update-chain',
    }, true);

    const proof = await get(`/v1/merkle-proof/${GLOBAL_TREE_ADDRESS}/${holderIdentityLeaf}`);
    assert.equal(proof.leafIndex, 1);
    assert.equal(proof.siblings.length, 20);
  });

  it('creates, lists, and registers custom schema requests', async () => {
    const created = await post('/v1/custom-schema-requests', {
      proposerAuthority: 'GwqjvUSmPKeNnPSWzFBPnURGXVkpLAzdujMuPCEPMyNi',
      proposerIssuerAccount: '8z3pKLPbB8Rc3o55dWmfZbXgwTpMhwowpP9dzVtywp55',
      proposerIssuerName: 'Example Issuer',
      name: 'proof_of_humanity',
      displayName: 'Proof of Humanity',
      version: 1,
      category: 'Identity',
      fields: [
        { name: 'is_unique', type: 'boolean', description: '1 if unique human', rangeQueryable: false },
        { name: 'issued_at', type: 'timestamp', description: 'Unix timestamp', rangeQueryable: true },
      ],
      schemaHash: SCHEMA_HASH,
      reason: 'Need custom app-gating credential.',
    });

    assert.equal(created.status, 'requested');
    assert.equal(created.name, 'proof_of_humanity');
    assert.equal(created.fields.length, 2);

    const listed = await get(`/v1/custom-schema-requests?proposerAuthority=${created.proposerAuthority}`);
    assert.equal(listed.requests.length, 1);
    assert.equal(listed.requests[0].id, created.id);

    const updated = await patch(`/v1/custom-schema-requests/${created.id}`, {
      status: 'registered',
      registeredBy: 'Gdz9JLWUekrfnpT3fPu1SsWfas3b3zMhfC4frvV1QRNm',
      registerSchemaSignature: 'register-schema-signature',
      initializeTreeSignature: 'initialize-tree-signature',
      schemaPda: 'BDqhj8WoFQ1VfRWErJVV6cR48TsTf4amXzngGXXy5UmB',
      schemaTreeBindingPda: 'AKDjcDE3YwRdJrkCFwYEZDXs9WMeXMCvxX7aERvynpVx',
      treeAddress: TREE_ADDRESS,
    });
    assert.equal(updated.status, 'registered');
    assert.equal(updated.treeAddress, TREE_ADDRESS);
  });

  it('indexes issued credentials for registered custom schema trees', async () => {
    await post('/v1/custom-schema-requests', {
      proposerAuthority: 'GwqjvUSmPKeNnPSWzFBPnURGXVkpLAzdujMuPCEPMyNi',
      proposerIssuerAccount: '8z3pKLPbB8Rc3o55dWmfZbXgwTpMhwowpP9dzVtywp55',
      proposerIssuerName: 'Example Issuer',
      name: 'dao_membership',
      displayName: 'DAO Membership',
      version: 1,
      category: 'Governance',
      fields: [
        { name: 'is_valid', type: 'boolean', description: '1 if active member', rangeQueryable: false },
        { name: 'issued_at', type: 'timestamp', description: 'Unix timestamp', rangeQueryable: true },
      ],
      schemaHash: CUSTOM_SCHEMA_HASH,
      reason: 'Need DAO membership proofs.',
    });
    const custom = (await get('/v1/custom-schema-requests')).requests[0];
    await patch(`/v1/custom-schema-requests/${custom.id}`, {
      status: 'registered',
      registeredBy: 'Gdz9JLWUekrfnpT3fPu1SsWfas3b3zMhfC4frvV1QRNm',
      registerSchemaSignature: 'register-schema-signature',
      initializeTreeSignature: 'initialize-tree-signature',
      schemaPda: 'BDqhj8WoFQ1VfRWErJVV6cR48TsTf4amXzngGXXy5UmB',
      schemaTreeBindingPda: 'AKDjcDE3YwRdJrkCFwYEZDXs9WMeXMCvxX7aERvynpVx',
      treeAddress: CUSTOM_TREE_ADDRESS,
    });

    const request = await post('/v1/credential-requests', {
      schemaHash: CUSTOM_SCHEMA_HASH,
      schemaName: 'dao_membership_v1',
      schemaVersion: 1,
      issuerAuthority: 'GwqjvUSmPKeNnPSWzFBPnURGXVkpLAzdujMuPCEPMyNi',
      holderChannelPublicKey: 'holder-channel-key',
      holderPublicKeyX: HOLDER_PUBLIC_KEY_X,
      holderPublicKeyY: HOLDER_PUBLIC_KEY_Y,
    });
    await patch(`/v1/credential-requests/${request.id}`, {
      status: 'issued',
      commitment: LEAF,
      transactionSignature: 'custom-credential-signature',
    });

    const proof = await get(`/v1/merkle-proof/${CUSTOM_TREE_ADDRESS}/${LEAF}`);
    assert.equal(proof.leafIndex, 0);
    assert.equal(proof.siblings.length, 20);
  });

  it('preserves verifier predicate logic on proof requests', async () => {
    const created = await post('/v1/proof-requests', {
      dappName: 'Demo DeFi Pool',
      action: 'Request access',
      schemaHash: SCHEMA_HASH,
      schemaName: 'basic_identity_v2',
      schemaVersion: 2,
      compoundLogic: 'OR',
      predicates: [
        { fieldIndex: 0, fieldName: 'age', operator: 'GTE', value: '18' },
        { fieldIndex: 4, fieldName: 'verification_level', operator: 'GTE', value: '2' },
      ],
    });
    assert.equal(created.compoundLogic, 'OR');
    assert.equal(created.predicates.length, 2);
  });

  it('defaults proof request predicate logic to AND', async () => {
    const created = await post('/v1/proof-requests', {
      dappName: 'Demo DeFi Pool',
      action: 'Request access',
      schemaHash: SCHEMA_HASH,
      schemaName: 'basic_identity_v2',
      schemaVersion: 2,
      predicates: [
        { fieldIndex: 0, fieldName: 'age', operator: 'GTE', value: '18' },
      ],
    });
    assert.equal(created.compoundLogic, 'AND');
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

  it('refuses proofs whose local root does not match the live schema binding root', async () => {
    await initWasm();
    const store = new MemoryIndexerStore();
    const liveBindingRoot = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
    const manifestWithBinding = {
      ...manifest,
      trees: {
        ...manifest.trees,
        schema_trees: [{
          ...manifest.trees.schema_trees[0],
          binding_pda: 'FiiYYq2gysBiwhVYptS5GkmKvMJETXNXthz9SrTciuDb',
        }],
      },
    };
    const schemaBindingData = Buffer.alloc(145);
    schemaBindingData.set(Buffer.from(liveBindingRoot, 'hex'), 72);
    handler = createIndexerHandler({
      manifest: manifestWithBinding,
      store,
      writeToken: 'test-token',
      connection: {
        async getAccountInfo() {
          return { data: schemaBindingData };
        },
      },
    });

    upsertTreeLeaf(store, {
      treeAddress: TREE_ADDRESS,
      leaf: LEAF,
      leafIndex: 0,
      slot: 7,
      source: 'test',
    });

    const response = await request(`/v1/merkle-proof/${TREE_ADDRESS}/${LEAF}`);
    assert.equal(response.status, 409);
    assert.equal(response.body.error, 'ROOT_OUT_OF_SYNC');
    assert.match(response.body.message, /does not match live on-chain root/);
  });

  it('uses the protocol issuer-tree depth when the manifest omits explicit depth', () => {
    assert.equal(treeDepthFor(manifest, manifest.trees.issuer_tree.tree_address), 16);
    assert.equal(treeDepthFor(manifest, TREE_ADDRESS), 2);
    assert.equal(treeDepthFor(manifest, GLOBAL_TREE_ADDRESS), 20);
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

  it('serves issuer proofs when a stale same-leaf row exists at the wrong index', async () => {
    const store = new MemoryIndexerStore();
    handler = createIndexerHandler({ manifest, store, writeToken: 'test-token' });
    const issuerTree = manifest.trees.issuer_tree.tree_address;
    const staleIssuerLeaf = '49d0a6f02b63000120afd324e0c5538bdcd6362ea77cdb5ef0f730558441fe01';
    const activeIndexZeroLeaf = 'e2ce08fcb6450fbdf3dd24ef275981d50cc038fdb31a0e44188e96ba24cc740c';

    upsertTreeLeaf(store, {
      treeAddress: issuerTree,
      leaf: staleIssuerLeaf,
      leafIndex: 0,
      slot: 0,
      source: 'issuer-registry-chain',
    });
    upsertTreeLeaf(store, {
      treeAddress: issuerTree,
      leaf: activeIndexZeroLeaf,
      leafIndex: 0,
      slot: 0,
      source: 'issuer-registry-chain',
    });
    upsertTreeLeaf(store, {
      treeAddress: issuerTree,
      leaf: staleIssuerLeaf,
      leafIndex: 3,
      slot: 0,
      source: 'issuer-registry-chain',
    });

    const proof = await get(`/v1/merkle-proof/${issuerTree}/${staleIssuerLeaf}`);
    assert.equal(proof.leafIndex, 3);
    assert.equal(proof.siblings.length, 16);
  });

  it('serializes BigInt account fields without partially writing a broken response', async () => {
    const response = await captureJsonResponse({
      amountStaked: 123n,
      nested: { credentialsIssued: 456n },
    });

    assert.equal(response.status, 200);
    assert.equal(response.body.amountStaked, '123');
    assert.equal(response.body.nested.credentialsIssued, '456');
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

  async function captureJsonResponse(body) {
    return new Promise((resolve) => {
      const res = {
        status: 0,
        headers: {},
        writeHead(status, headers) {
          this.status = status;
          this.headers = headers;
        },
        end(payload) {
          resolve({
            status: this.status,
            headers: this.headers,
            body: JSON.parse(payload),
          });
        },
      };
      writeJsonResponse(res, 200, body);
    });
  }
});
