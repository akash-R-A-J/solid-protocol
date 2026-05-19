import { createHash } from 'node:crypto';
import {
  ComputeBudgetProgram,
  PublicKey,
  Transaction,
  TransactionInstruction,
} from '@solana/web3.js';
import {
  createMintToInstruction,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountInstruction,
} from '@solana/spl-token';
import {
  computeIdentityState,
  computeIssuerLeaf,
  initWasm,
} from '@solid-protocol/core';
import {
  decodeIssuerAccount,
  decodeIssuerTreeBinding,
  decodeSchemaAccount,
  ISSUER_ACCOUNT_DISCRIMINATOR_BASE58,
  SCHEMA_ACCOUNT_DISCRIMINATOR_BASE58,
} from '@solid-protocol/issuer/registry';
import { buildMerkleProof, buildRootSyncPlan } from './proofs.mjs';
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
  activeLeavesForTree,
  nextLeafIndex,
  updateCredentialRequest,
  updateCustomSchemaRequest,
  updateProofRequest,
  updateSchemaPermissionRequest,
  upsertTreeLeaf,
} from './store.mjs';
import { bytesToHex, hexToBytes32, normalizeHex32, writeJsonResponse } from './encoding.mjs';

const UPDATE_TREE_ROOT_DISCRIMINATOR = Buffer.from([96, 226, 40, 157, 60, 110, 3, 1]);
const UPDATE_GLOBAL_ROOT_DISCRIMINATOR = Buffer.from([75, 70, 121, 86, 205, 171, 173, 156]);
const ROOT_SYNC_COMPUTE_UNITS = 1_200_000;

export function createIndexerHandler({ manifest, store, connection = null, writeToken = '', rootSyncKeypair = null }) {
  const manifestHash = createHash('sha256').update(JSON.stringify(manifest)).digest('hex');

  return async function handle(req, res) {
    try {
      const url = new URL(req.url ?? '/', `http://${req.headers.host ?? 'localhost'}`);
      if (req.method === 'OPTIONS') {
        writeJsonResponse(res, 204, {});
        return;
      }

      if (url.pathname === '/.well-known/solid-protocol.json' || url.pathname === '/v1/manifest') {
        writeJsonResponse(res, 200, await manifestWithLiveRoots(manifest, connection));
        return;
      }

      if (url.pathname === '/v1/faucet/governance' && req.method === 'POST') {
        if (!connection || !rootSyncKeypair) {
          writeJsonResponse(res, 503, { ok: false, error: 'FAUCET_UNAVAILABLE', message: 'No RPC or deployer keypair' });
          return;
        }
        try {
          const body = await readJsonBody(req);
          const wallet = new PublicKey(body.wallet);
          const mint = new PublicKey(manifest.programs.issuer_registry.governance_token_mint);
          const ata = getAssociatedTokenAddressSync(mint, wallet);
          const accountInfo = await connection.getAccountInfo(ata);
          const tx = new Transaction();
          if (!accountInfo) {
            tx.add(createAssociatedTokenAccountInstruction(
              rootSyncKeypair.publicKey, // payer
              ata,
              wallet,
              mint
            ));
          }
          // Mint 10,000 tokens (assuming 6 decimals)
          tx.add(createMintToInstruction(mint, ata, rootSyncKeypair.publicKey, 10_000_000_000n));
          
          const latest = await connection.getLatestBlockhash();
          tx.recentBlockhash = latest.blockhash;
          tx.feePayer = rootSyncKeypair.publicKey;
          tx.sign(rootSyncKeypair);
          const signature = await connection.sendRawTransaction(tx.serialize());
          writeJsonResponse(res, 200, { ok: true, signature });
        } catch (e) {
          writeJsonResponse(res, 500, { ok: false, error: e.message });
        }
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
        writeJsonResponse(res, 200, currentRoots(store, await manifestWithLiveRoots(manifest, connection)));
        return;
      }

      if (url.pathname === '/v1/root-sync/plan' && req.method === 'GET') {
        writeJsonResponse(res, 200, await buildRootSyncResponse(store, manifest, connection));
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
          await maybeIndexIssuedCredentialRequest(store, manifest, updated);
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
          await maybeBackfillProofLeaf(
            store,
            manifest,
            connection,
            decodeURIComponent(treeAddress),
            decodeURIComponent(leaf),
          );
          let proof = await buildMerkleProof(
            store,
            manifest,
            decodeURIComponent(treeAddress),
            decodeURIComponent(leaf),
          );
          try {
            await assertProofRootMatchesLiveBinding(
              store,
              manifest,
              connection,
              decodeURIComponent(treeAddress),
              proof.root,
            );
          } catch (error) {
            if (error.code !== 'ROOT_OUT_OF_SYNC') throw error;
            if (!rootSyncKeypair) throw error;
            await autoSyncRootsForProof({
              store,
              manifest,
              connection,
              rootSyncKeypair,
              treeAddress: decodeURIComponent(treeAddress),
            });
            proof = await buildMerkleProof(
              store,
              manifest,
              decodeURIComponent(treeAddress),
              decodeURIComponent(leaf),
            );
            await assertProofRootMatchesLiveBinding(
              store,
              manifest,
              connection,
              decodeURIComponent(treeAddress),
              proof.root,
            );
          }
          writeJsonResponse(res, 200, proof);
        } catch (error) {
          if (error.code === 'LEAF_NOT_INDEXED') {
            const decodedTreeAddress = decodeURIComponent(treeAddress);
            const isIssuerTree = manifest.trees?.issuer_tree?.tree_address === decodedTreeAddress;
            writeJsonResponse(res, 404, {
              ok: false,
              error: isIssuerTree ? 'ISSUER_LEAF_NOT_INDEXED' : 'LEAF_NOT_INDEXED',
              message: isIssuerTree
                ? 'Issuer leaf is not indexed in the live issuer tree. The credential issuer may not be issuer-tree enrolled.'
                : error.message,
            });
          } else if (error.code === 'ROOT_OUT_OF_SYNC') {
            writeJsonResponse(res, 409, {
              ok: false,
              error: 'ROOT_OUT_OF_SYNC',
              message: error.message,
            });
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
          ?? (manifest.schemas ?? []).find((entry) => entry.schema_hash === schemaHash)
          ?? registeredCustomSchemaTrees(store).find((entry) => entry.schemaHash === schemaHash)
          ?? registeredCustomSchemas(store).find((entry) => entry.schema_hash === schemaHash);
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

async function maybeBackfillProofLeaf(store, manifest, connection, treeAddress, leaf) {
  const normalizedLeaf = normalizeHex32(leaf, 'leaf');
  const credentialLeaf = await maybeBackfillIssuedCredentialLeaf(store, manifest, treeAddress, normalizedLeaf);
  if (credentialLeaf) return credentialLeaf;
  const holderIdentityLeaf = await maybeBackfillHolderIdentityLeaf(store, manifest, treeAddress, normalizedLeaf);
  if (holderIdentityLeaf) return holderIdentityLeaf;
  return maybeBackfillIssuerLeaf(store, manifest, connection, treeAddress, normalizedLeaf);
}

async function assertProofRootMatchesLiveBinding(store, manifest, connection, treeAddress, proofRoot) {
  if (!connection) return;
  const bindingPda = registeredCustomSchemaTrees(store).find((entry) => entry.treeAddress === treeAddress)?.bindingPda;
  const liveRoot = await readLiveBindingRoot(manifest, connection, treeAddress, bindingPda);
  if (!liveRoot || liveRoot === proofRoot) return;

  const error = new Error(
    `Indexer proof root ${proofRoot} does not match live on-chain root ${liveRoot} for tree ${treeAddress}. ` +
    'The local indexer leaf history is out of sync with the root binding; rebuild or sync the root history before generating an on-chain proof.',
  );
  error.code = 'ROOT_OUT_OF_SYNC';
  throw error;
}

async function readLiveBindingRoot(manifest, connection, treeAddress, explicitBindingPda = null) {
  const globalStateTree = manifest.trees?.global_state_tree;
  if (globalStateTree && (
    globalStateTree.tree_address === treeAddress
    || globalStateTree.binding_pda === treeAddress
  )) {
    const account = await connection.getAccountInfo(new PublicKey(globalStateTree.binding_pda ?? globalStateTree.tree_address), 'confirmed');
    return account?.data?.length >= 40 ? bytesToHex(account.data.subarray(8, 40)) : null;
  }

  const schemaTree = (manifest.trees?.schema_trees ?? [])
    .find((entry) => entry.tree_address === treeAddress || entry.binding_pda === treeAddress);
  if (schemaTree?.binding_pda) {
    const account = await connection.getAccountInfo(new PublicKey(schemaTree.binding_pda), 'confirmed');
    return account?.data?.length >= 104 ? bytesToHex(account.data.subarray(72, 104)) : null;
  }

  if (explicitBindingPda) {
    const account = await connection.getAccountInfo(new PublicKey(explicitBindingPda), 'confirmed');
    return account?.data?.length >= 104 ? bytesToHex(account.data.subarray(72, 104)) : null;
  }

  const issuerTree = manifest.trees?.issuer_tree;
  if (issuerTree && (
    issuerTree.tree_address === treeAddress
    || issuerTree.binding_pda === treeAddress
  ) && issuerTree.binding_pda) {
    const account = await connection.getAccountInfo(new PublicKey(issuerTree.binding_pda), 'confirmed');
    return account ? decodeIssuerTreeBinding(account.data).currentRoot : null;
  }

  return null;
}

async function buildRootSyncResponse(store, manifest, connection, options = {}) {
  if (!connection) {
    return {
      ok: false,
      error: 'RPC_NOT_CONFIGURED',
      message: 'Set SOLID_RPC_URL so the indexer can compare local leaf history with live root bindings.',
      operations: [],
    };
  }

  const candidates = [];
  const globalTree = manifest.trees?.global_state_tree;
  if (globalTree?.tree_address && globalTree?.binding_pda) {
    candidates.push({
      type: 'global',
      treeAddress: globalTree.tree_address,
      bindingPda: globalTree.binding_pda,
      schemaHash: null,
    });
  }
  for (const schemaTree of manifest.trees?.schema_trees ?? []) {
    if (!schemaTree.tree_address || !schemaTree.binding_pda) continue;
    candidates.push({
      type: 'schema',
      treeAddress: schemaTree.tree_address,
      bindingPda: schemaTree.binding_pda,
      schemaHash: schemaTree.schema_hash,
    });
  }
  for (const schemaTree of registeredCustomSchemaTrees(store)) {
    candidates.push(schemaTree);
  }

  if (options.backfillChainHistory !== false) {
    for (const candidate of candidates) {
      await backfillRootUpdatesFromChain(store, manifest, connection, candidate);
    }
  }

  for (const request of listCredentialRequests(store, {})) {
    await maybeIndexIssuedCredentialRequest(store, manifest, request);
  }

  const operations = [];
  for (const candidate of candidates) {
    const liveRoot = await readLiveBindingRoot(manifest, connection, candidate.treeAddress, candidate.bindingPda);
    if (!liveRoot) continue;
    const plan = await buildRootSyncPlan(store, manifest, candidate.treeAddress, liveRoot);
    operations.push({
      ...candidate,
      ...plan,
      liveRoot,
    });
  }

  return {
    ok: true,
    operations,
  };
}

async function backfillRootUpdatesFromChain(store, manifest, connection, candidate) {
  const schemaRegistry = manifest.programs?.schema_registry?.program_id;
  if (!schemaRegistry || !candidate.bindingPda) return;

  const signatures = await connection.getSignaturesForAddress(
    new PublicKey(candidate.bindingPda),
    { limit: 100 },
    'confirmed',
  );
  for (const sigInfo of signatures.reverse()) {
    const tx = await connection.getTransaction(sigInfo.signature, {
      commitment: 'confirmed',
      maxSupportedTransactionVersion: 0,
    });
    if (!tx) continue;
    const keys = transactionKeys(tx);
    for (const ix of tx.transaction.message.compiledInstructions) {
      const programId = keys[ix.programIdIndex];
      if (programId !== schemaRegistry) continue;
      const firstAccount = keys[ix.accountKeyIndexes?.[0]];
      if (firstAccount !== candidate.bindingPda) continue;
      const data = Buffer.from(ix.data);
      const parsed = parseRootUpdateInstruction(candidate, data);
      if (!parsed) continue;
      upsertTreeLeaf(store, {
        treeAddress: candidate.treeAddress,
        leaf: parsed.newLeaf,
        leafIndex: parsed.leafIndex,
        slot: sigInfo.slot,
        signature: sigInfo.signature,
        eventIndex: ix.programIdIndex,
        source: `${candidate.type}-root-update-chain`,
        subject: null,
        schemaHash: candidate.schemaHash,
      });
    }
  }
}

async function autoSyncRootsForProof({ store, manifest, connection, rootSyncKeypair, treeAddress }) {
  if (!connection || !rootSyncKeypair) {
    const error = new Error(
      'ROOT_SYNC_REQUIRED: The indexer has pending schema/global root updates, but no service-side root sync keypair is configured.',
    );
    error.code = 'ROOT_SYNC_REQUIRED';
    throw error;
  }

  for (let attempt = 0; attempt < 12; attempt++) {
    const response = await buildRootSyncResponse(store, manifest, connection, {
      backfillChainHistory: false,
    });
    const pending = response.operations.filter((operation) =>
      operation.kind === 'append'
      && (!treeAddress || operation.treeAddress === treeAddress || operation.type === 'global')
    );
    if (pending.length === 0) return;

    for (const operation of pending) {
      await sendRootSyncOperation(connection, manifest, rootSyncKeypair, operation);
    }
  }

  const error = new Error('ROOT_SYNC_REQUIRED: Root sync did not converge after 12 service-side updates.');
  error.code = 'ROOT_SYNC_REQUIRED';
  throw error;
}

async function sendRootSyncOperation(connection, manifest, authority, operation) {
  for (let attempt = 0; attempt < 5; attempt++) {
    await waitForBindingWritableSlot(connection, operation);
    try {
      const latest = await connection.getLatestBlockhash('confirmed');
      const tx = buildRootSyncTransaction(manifest, authority.publicKey, operation);
      tx.feePayer = authority.publicKey;
      tx.recentBlockhash = latest.blockhash;
      tx.sign(authority);
      const signature = await connection.sendRawTransaction(tx.serialize(), {
        skipPreflight: false,
        preflightCommitment: 'confirmed',
      });
      const confirmation = await connection.confirmTransaction({
        signature,
        blockhash: latest.blockhash,
        lastValidBlockHeight: latest.lastValidBlockHeight,
      }, 'confirmed');
      if (confirmation.value.err) {
        const error = new Error(`ROOT_SYNC_REQUIRED: root sync transaction ${signature} failed: ${JSON.stringify(confirmation.value.err)}`);
        error.code = 'ROOT_SYNC_REQUIRED';
        throw error;
      }
      return;
    } catch (error) {
      if (!/RootSlotNotMonotonic|0x177b/i.test(String(error?.message ?? error)) || attempt === 4) {
        throw error;
      }
      await sleep(500);
    }
  }
}

async function waitForBindingWritableSlot(connection, operation) {
  const binding = await connection.getAccountInfo(new PublicKey(operation.bindingPda), 'confirmed');
  if (!binding?.data) return;
  const offset = operation.type === 'schema' ? 104 : 40;
  if (binding.data.length < offset + 8) return;
  const lastUpdatedSlot = Number(binding.data.readBigUInt64LE(offset));
  for (let attempt = 0; attempt < 20; attempt++) {
    const currentSlot = await connection.getSlot('confirmed');
    if (currentSlot > lastUpdatedSlot) return;
    await sleep(250);
  }
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function buildRootSyncTransaction(manifest, authority, operation) {
  const schemaRegistry = manifest.programs?.schema_registry?.program_id;
  if (!schemaRegistry) throw new Error('Manifest is missing schema_registry program id.');
  const data = operation.type === 'schema'
    ? encodeUpdateTreeRootData(operation)
    : encodeUpdateGlobalRootData(operation);
  return new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: ROOT_SYNC_COMPUTE_UNITS }),
    new TransactionInstruction({
      programId: new PublicKey(schemaRegistry),
      keys: [
        { pubkey: new PublicKey(operation.bindingPda), isSigner: false, isWritable: true },
        { pubkey: authority, isSigner: true, isWritable: false },
      ],
      data,
    }),
  );
}

function encodeUpdateTreeRootData(operation) {
  return Buffer.concat([
    UPDATE_TREE_ROOT_DISCRIMINATOR,
    hexToBytes32(operation.schemaHash, 'schemaHash'),
    requiredBytes(operation.newRoot, 'newRoot'),
    requiredBytes(operation.oldLeaf, 'oldLeaf'),
    requiredBytes(operation.newLeaf, 'newLeaf'),
    u64Le(operation.leafIndex),
    vecBytes(flattenSiblings(operation.siblings ?? [])),
  ]);
}

function encodeUpdateGlobalRootData(operation) {
  return Buffer.concat([
    UPDATE_GLOBAL_ROOT_DISCRIMINATOR,
    requiredBytes(operation.newRoot, 'newRoot'),
    requiredBytes(operation.oldLeaf, 'oldLeaf'),
    requiredBytes(operation.newLeaf, 'newLeaf'),
    u64Le(operation.leafIndex),
    vecBytes(flattenSiblings(operation.siblings ?? [])),
  ]);
}

function requiredBytes(value, name) {
  if (!value) throw new Error(`Root sync operation is missing ${name}.`);
  return Buffer.from(hexToBytes32(value, name));
}

function flattenSiblings(siblings) {
  return Buffer.concat(siblings.map((sibling, index) =>
    Buffer.from(hexToBytes32(sibling, `siblings[${index}]`)),
  ));
}

function vecBytes(bytes) {
  return Buffer.concat([u32Le(bytes.length), bytes]);
}

function u32Le(value) {
  const out = Buffer.alloc(4);
  out.writeUInt32LE(value);
  return out;
}

function u64Le(value) {
  const out = Buffer.alloc(8);
  out.writeBigUInt64LE(BigInt(value));
  return out;
}

function transactionKeys(tx) {
  return [
    ...tx.transaction.message.staticAccountKeys.map((key) => key.toBase58()),
    ...(tx.meta?.loadedAddresses?.writable ?? []).map((key) => key.toBase58()),
    ...(tx.meta?.loadedAddresses?.readonly ?? []).map((key) => key.toBase58()),
  ];
}

function parseRootUpdateInstruction(candidate, data) {
  const discriminator = candidate.type === 'global'
    ? UPDATE_GLOBAL_ROOT_DISCRIMINATOR
    : UPDATE_TREE_ROOT_DISCRIMINATOR;
  if (data.length < discriminator.length || !data.subarray(0, 8).equals(discriminator)) return null;

  const offset = candidate.type === 'schema' ? 8 + 32 + 32 + 32 : 8 + 32 + 32;
  if (data.length < offset + 32 + 8) return null;
  const newLeaf = bytesToHex(data.subarray(offset, offset + 32));
  const leafIndex = Number(data.readBigUInt64LE(offset + 32));
  return { newLeaf, leafIndex };
}

async function maybeBackfillIssuedCredentialLeaf(store, manifest, treeAddress, normalizedLeaf) {
  const schema = schemaForCredentialTree(store, manifest, treeAddress);
  if (!schema) return null;
  const match = listCredentialRequests(store, {})
    .find((request) =>
      request.status === 'issued'
      && request.commitment === normalizedLeaf
      && request.schemaHash === schema.schema_hash,
    );
  if (!match) return null;
  return maybeIndexIssuedCredentialRequest(store, manifest, match, treeAddress);
}

async function maybeIndexIssuedCredentialRequest(store, manifest, request, explicitTreeAddress = null) {
  if (!request || request.status !== 'issued' || !request.commitment) return null;
  const schema = schemaForCredentialRequest(store, manifest, request.schemaHash);
  const treeAddress = explicitTreeAddress ?? schema?.tree_address;
  if (!treeAddress) return null;
  const leaf = normalizeHex32(request.commitment, 'commitment');
  const activeLeaf = activeLeavesForTree(store, treeAddress).find((entry) => entry.leaf === leaf);
  if (activeLeaf) {
    await maybeIndexHolderIdentityLeaf(store, manifest, request);
    return activeLeaf;
  }
  const existing = store.read().treeLeaves.find((entry) =>
    entry.treeAddress === treeAddress
    && entry.leaf === leaf
    && entry.source !== 'credential-request-issued'
  );
  const credentialLeaf = existing ?? upsertTreeLeaf(store, {
    treeAddress,
    leaf,
    leafIndex: nextLeafIndex(store, treeAddress),
    slot: 0,
    signature: request.transactionSignature,
    source: 'credential-request-issued',
    subject: request.holderPublicKeyX,
    schemaHash: request.schemaHash,
  });
  await maybeIndexHolderIdentityLeaf(store, manifest, request);
  return credentialLeaf;
}

function schemaForCredentialRequest(store, manifest, schemaHash) {
  return (manifest.schemas ?? []).find((entry) => entry.schema_hash === schemaHash)
    ?? registeredCustomSchemas(store).find((entry) => entry.schema_hash === schemaHash)
    ?? null;
}

function schemaForCredentialTree(store, manifest, treeAddress) {
  return (manifest.schemas ?? []).find((entry) => entry.tree_address === treeAddress)
    ?? registeredCustomSchemas(store).find((entry) => entry.tree_address === treeAddress)
    ?? null;
}

function registeredCustomSchemas(store) {
  return listCustomSchemaRequests(store, { status: 'registered' })
    .filter((request) => request.treeAddress)
    .map((request) => ({
      name: request.name,
      version: request.version,
      schema_hash: request.schemaHash,
      schema_pda: request.schemaPda,
      tree_address: request.treeAddress,
      tree_depth: 20,
      fields: request.fields.map((field) => field.name),
      predicates: request.fields.some((field) => field.rangeQueryable)
        ? ['EQ', 'GTE', 'LTE']
        : ['EQ'],
    }));
}

function registeredCustomSchemaTrees(store) {
  return listCustomSchemaRequests(store, { status: 'registered' })
    .filter((request) => request.treeAddress && request.schemaTreeBindingPda)
    .map((request) => ({
      type: 'schema',
      treeAddress: request.treeAddress,
      bindingPda: request.schemaTreeBindingPda,
      schemaHash: request.schemaHash,
    }));
}

async function maybeBackfillHolderIdentityLeaf(store, manifest, treeAddress, normalizedLeaf) {
  if (!isGlobalStateTree(manifest, treeAddress)) return null;
  for (const request of listCredentialRequests(store, {})) {
    const record = await maybeIndexHolderIdentityLeaf(store, manifest, request);
    if (record?.leaf === normalizedLeaf) return record;
  }
  return null;
}

async function maybeIndexHolderIdentityLeaf(store, manifest, request) {
  if (!request || request.status !== 'issued') return null;
  const treeAddress = manifest.trees?.global_state_tree?.tree_address
    ?? manifest.trees?.global_state_tree?.binding_pda;
  if (!treeAddress) return null;
  if (!isHex32(request.holderPublicKeyX) || !isHex32(request.holderPublicKeyY)) return null;
  await initWasm();
  const leaf = bytesToHex(computeIdentityState(
    hexToBytes32(request.holderPublicKeyX, 'holderPublicKeyX'),
    hexToBytes32(request.holderPublicKeyY, 'holderPublicKeyY'),
    0n,
  ));
  const activeLeaf = activeLeavesForTree(store, treeAddress).find((entry) =>
    entry.treeAddress === treeAddress && entry.leaf === leaf,
  );
  if (activeLeaf) return activeLeaf;
  return upsertTreeLeaf(store, {
    treeAddress,
    leaf,
    leafIndex: nextLeafIndex(store, treeAddress),
    slot: 0,
    signature: request.transactionSignature,
    source: 'credential-request-holder-identity',
    subject: request.holderPublicKeyX,
    schemaHash: request.schemaHash,
  });
}

async function maybeBackfillIssuerLeaf(store, manifest, connection, treeAddress, normalizedLeaf) {
  if (!connection || manifest.trees?.issuer_tree?.tree_address !== treeAddress) return null;
  const issuers = await fetchIssuers(connection, manifest);
  await initWasm();
  let target = null;
  for (const issuer of issuers) {
    if (!issuer.isTreeEnrolled) continue;
    if (!isHex32(issuer.bjjPubKeyX) || !isHex32(issuer.bjjPubKeyY)) continue;
    const leaf = bytesToHex(computeIssuerLeaf(
      new PublicKey(issuer.authority).toBytes(),
      hexToBytes32(issuer.bjjPubKeyX, 'issuer.bjjPubKeyX'),
      hexToBytes32(issuer.bjjPubKeyY, 'issuer.bjjPubKeyY'),
      BigInt(issuer.statusEpoch),
      BigInt(issuer.revocationNonce),
    ));
    const leafIndex = Number(issuer.issuerTreeLeafIndex);
    const existing = store.read().treeLeaves.find((entry) =>
      entry.treeAddress === treeAddress
      && entry.leaf === leaf
      && entry.leafIndex === leafIndex,
    );
    const record = existing ?? upsertTreeLeaf(store, {
      treeAddress,
      leaf,
      leafIndex,
      slot: 0,
      source: 'issuer-registry-chain',
      subject: issuer.pda,
      schemaHash: null,
    });
    if (leaf === normalizedLeaf) target = record;
  }
  return target;
}

function isGlobalStateTree(manifest, treeAddress) {
  const globalStateTree = manifest.trees?.global_state_tree;
  return !!globalStateTree && (
    globalStateTree.tree_address === treeAddress
    || globalStateTree.binding_pda === treeAddress
  );
}

function isHex32(value) {
  return /^[0-9a-f]{64}$/i.test(String(value ?? '').trim().replace(/^0x/i, ''));
}

async function manifestWithLiveRoots(manifest, connection) {
  if (!connection) return manifest;
  const issuerBindingPda = manifest.trees?.issuer_tree?.binding_pda;
  if (!issuerBindingPda) return manifest;
  try {
    const account = await connection.getAccountInfo(new PublicKey(issuerBindingPda), 'confirmed');
    if (!account) return manifest;
    const binding = decodeIssuerTreeBinding(account.data);
    return {
      ...manifest,
      trees: {
        ...manifest.trees,
        issuer_tree: {
          ...manifest.trees.issuer_tree,
          tree_address: binding.treeAddress || manifest.trees.issuer_tree.tree_address,
          current_root: binding.currentRoot,
          current_root_slot: Number(binding.lastUpdatedSlot),
        },
      },
    };
  } catch {
    return manifest;
  }
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
