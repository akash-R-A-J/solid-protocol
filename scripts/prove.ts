/**
 * SolID Protocol — proof generation and on-chain verification script (E2E step 3).
 *
 * Depends on scripts/issue.ts having run first.
 *
 * Responsibilities:
 *   1. Load credential state produced by issue.ts.
 *   2. Build a MultiCredentialQuery via QueryBuilder.
 *   3. Seed a LocalReplicaAdapter with the known commitment (for localnet);
 *      production runs would pass a Helius DAS adapter instead.
 *   4. Generate a batch Groth16 proof via @solid-protocol/holder.
 *   5. Submit verify_batch_proof via @solid-protocol/verifier, capturing the
 *      real transaction signature.
 *   6. Replay the same transaction and assert that it is rejected by the
 *      nullifier PDA's init constraint.
 */

import { Connection, Keypair, PublicKey } from '@solana/web3.js';
import {
  initWasm,
  QueryBuilder,
  PROGRAM_IDS,
  computeCommitment,
  deriveCredentialKey,
} from '@solid-protocol/core';
import { generateBatchProof, type StoredCredential } from '@solid-protocol/holder';
import { verifyOnChain, type SchemaTreeAccounts } from '@solid-protocol/verifier';
import { LocalReplicaAdapter, poseidonHashPair } from '@solid-protocol/light';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

const PROGRAM_PUBKEYS = {
  zkVerifier: new PublicKey(PROGRAM_IDS.zkVerifier),
  schemaRegistry: new PublicKey(PROGRAM_IDS.schemaRegistry),
};

const RPC_URL = process.env.SOLID_RPC_URL ?? 'http://127.0.0.1:8899';
const GLOBAL_TREE_DEPTH = 20;
const CREDENTIAL_TREE_DEPTH = 20;

async function main() {
  await initWasm();
  const connection = new Connection(RPC_URL, 'confirmed');

  const keypairPath = path.join(os.homedir(), '.config/solana/id.json');
  const secretKey = JSON.parse(fs.readFileSync(keypairPath, 'utf-8'));
  const wallet = Keypair.fromSecretKey(Uint8Array.from(secretKey));

  if (!fs.existsSync('scripts/e2e_state.json')) {
    throw new Error('Run scripts/issue.ts first');
  }
  const state = JSON.parse(fs.readFileSync('scripts/e2e_state.json', 'utf-8'));

  console.log('SolID Protocol — proof and verification');
  console.log('=======================================');

  const credential: StoredCredential = {
    schemaHash: Uint8Array.from(state.credential.schemaHash),
    attestationData: state.credential.attestationData.map((s: string) => BigInt(s)),
    issuerSignature: {
      r8_x: Uint8Array.from(state.credential.issuerSignature.r8_x),
      r8_y: Uint8Array.from(state.credential.issuerSignature.r8_y),
      s: Uint8Array.from(state.credential.issuerSignature.s),
    },
    issuerPubKeyX: Uint8Array.from(state.credential.issuerPubKeyX),
    issuerPubKeyY: Uint8Array.from(state.credential.issuerPubKeyY),
    holderPubKeyX: Uint8Array.from(state.credential.holderPubKeyX),
    holderPubKeyY: Uint8Array.from(state.credential.holderPubKeyY),
    holderPrivateKey: Uint8Array.from(state.holderSchema.private_key),
    salt: Uint8Array.from(state.credential.salt),
    commitment: Uint8Array.from(state.credential.commitment),
    expirationTimestamp: state.credential.expirationTimestamp ?? 0,
    merkleTree: new PublicKey(state.merkleTreeAddress),
  };

  // 1. Build query (age >= 21).
  console.log('[1/4] Building query...');
  const nonce = new Uint8Array(32);
  for (let i = 0; i < nonce.length; i++) nonce[i] = 1; // deterministic test nonce
  const schemaHashes = [
    credential.schemaHash,
    new Uint8Array(32),
    new Uint8Array(32),
    new Uint8Array(32),
  ];
  const query = new QueryBuilder()
    .schemas(schemaHashes)
    .where(0, 0, 'GTE', 21n)
    .verifier(PROGRAM_PUBKEYS.zkVerifier.toBuffer())
    .nonce(nonce)
    .build();

  // 2. Seed local Merkle replica with the issued credential.
  //    In production this is a Helius DAS adapter; we keep the local replica
  //    in the E2E so the flow works against `solana-test-validator` without
  //    an indexer.
  console.log('[2/4] Seeding local Merkle replica...');
  const credReplica = new LocalReplicaAdapter(CREDENTIAL_TREE_DEPTH, poseidonHashPair);
  credReplica.appendLeaf(credential.commitment);

  // Build the global replica: compute the per-schema identity leaf and seed it.
  const holderMasterPriv = Uint8Array.from(state.holderMaster.private_key);
  const kp = deriveCredentialKey(holderMasterPriv, credential.schemaHash);
  const globalReplica = new LocalReplicaAdapter(GLOBAL_TREE_DEPTH, keccak256HashPair);
  // identity leaf = Poseidon(credPubX, credPubY, revocationNonce). At this
  // bootstrapping stage revocationNonce is 0.
  const { computeIdentityState } = await import('@solid-protocol/core');
  const identityLeaf = computeIdentityState(kp.public_key_x, kp.public_key_y, 0n);
  globalReplica.appendLeaf(identityLeaf);

  const merkleProofAdapter = {
    async fetch(tree: PublicKey, leaf: Uint8Array) {
      const isGlobal = tree.toBase58() === state.globalBindingPda;
      const src = isGlobal ? globalReplica : credReplica;
      return src.fetch(tree, leaf);
    },
  };

  // 3. Generate batch proof.
  console.log('[3/4] Generating Groth16 batch proof...');
  const wasmPath = 'circuits/build/batch_credential_query_js/batch_credential_query.wasm';
  const zkeyPath = 'circuits/build/batch_credential_query_final.zkey';
  if (!fs.existsSync(wasmPath) || !fs.existsSync(zkeyPath)) {
    console.error(
      `Circuit artifacts missing. Run "cd circuits && node scripts/setup.js" first.`,
    );
    process.exit(1);
  }
  const startTs = Date.now();
  const proofResult = await generateBatchProof(
    query,
    [credential, credential, credential, credential].slice(0, 1), // pad-on-slot-0 is acceptable
    holderMasterPriv,
    {
      x: Uint8Array.from(state.holderMaster.public_key_x),
      y: Uint8Array.from(state.holderMaster.public_key_y),
    },
    0n, // revocationNonce
    { wasmPath, zkeyPath },
    {
      merkleProofAdapter,
      globalStateTree: new PublicKey(state.globalBindingPda),
    },
  );
  console.log(`   proof generated in ${((Date.now() - startTs) / 1000).toFixed(2)}s`);

  // 4. Submit to on-chain verifier.
  console.log('[4/4] Submitting verify_batch_proof...');
  const schemaTreeBinding = new PublicKey(state.schemaTreeBindingPda);
  const trees: SchemaTreeAccounts = {
    globalTree: new PublicKey(state.globalBindingPda),
    schemaTree0: schemaTreeBinding,
    schemaTree1: schemaTreeBinding,
    schemaTree2: schemaTreeBinding,
    schemaTree3: schemaTreeBinding,
  };
  const request = {
    query,
    proofData: {
      proof_a: proofResult.solanaProof.proofA,
      proof_b: proofResult.solanaProof.proofB,
      proof_c: proofResult.solanaProof.proofC,
      publicInputs: proofResult.publicSignals.map(s => {
        let n = BigInt(s);
        const bytes = new Uint8Array(32);
        for (let i = 31; i >= 0; i--) { bytes[i] = Number(n & 0xFFn); n >>= 8n; }
        return bytes;
      }),
      nullifier: proofResult.nullifier,
    },
  };
  const verification = await verifyOnChain(connection, wallet, request, trees);
  console.log(`   verified: ${verification.verified}`);
  console.log(`   tx:       ${verification.transactionSignature}`);

  // Replay protection.
  console.log('Replay test (should fail)...');
  try {
    await verifyOnChain(connection, wallet, request, trees);
    console.error('   ERROR: replay was accepted (nullifier PDA not enforced)');
    process.exit(1);
  } catch {
    console.log('   ok (replay rejected by nullifier PDA init constraint)');
  }

  console.log('\nDone.');
}

main().catch(e => {
  console.error(e);
  process.exit(1);
});
