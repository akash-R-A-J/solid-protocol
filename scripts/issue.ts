/**
 * SolID Protocol -- issuance script (E2E step 3).
 *
 * Pipeline (ALL steps must have run first):
 *   scripts/initialize.ts         -> registry + schema + bindings + VK
 *   scripts/bootstrap_issuer.ts   -> issuer registered, staked, voted,
 *                                    approved (flips Pending -> Approved)
 *   scripts/issue.ts              -> this script
 *   scripts/prove.ts              -> proof + on-chain verify
 *
 * Responsibilities:
 *   1. Load state (schema + issuer keypairs) from the shared E2E
 *      state file.
 *   2. Derive a holder identity (per-run) and its per-schema subkey.
 *   3. Call issuer-registry::issue_credential which CPIs into SPL AC.
 *      The handler (post SOLID-SEC-003) requires schema_account +
 *      schema_tree_binding accounts; those are derived inside
 *      `@solid-protocol/issuer` from `schemaName` + `schemaVersion`.
 *   4. Persist the credential + holder identity.
 *
 * Program IDs are sourced from @solid-protocol/core::PROGRAM_IDS.
 */

import { Connection, Keypair, PublicKey } from '@solana/web3.js';
import {
  initWasm,
  generateKeypair,
  deriveCredentialKey,
  PROGRAM_IDS,
} from '@solid-protocol/core';
import { issueCredential } from '@solid-protocol/issuer';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { readState, writeState } from './lib/e2e_state';

const PROGRAM_PUBKEYS = {
  schemaRegistry: new PublicKey(PROGRAM_IDS.schemaRegistry),
  zkVerifier: new PublicKey(PROGRAM_IDS.zkVerifier),
  issuerRegistry: new PublicKey(PROGRAM_IDS.issuerRegistry),
};

const RPC_URL = process.env.SOLID_RPC_URL ?? 'http://127.0.0.1:8899';

async function main() {
  await initWasm();
  const connection = new Connection(RPC_URL, 'confirmed');

  const keypairPath = path.join(os.homedir(), '.config/solana/id.json');
  const secretKey = JSON.parse(fs.readFileSync(keypairPath, 'utf-8'));
  const wallet = Keypair.fromSecretKey(Uint8Array.from(secretKey));

  console.log('SolID Protocol -- credential issuance');
  console.log('=====================================');

  const state = readState('issue');
  if (!state.schemaHash || !state.schemaName) {
    throw new Error('state is missing schema metadata; run initialize.ts first');
  }
  if (!state.issuerBjj || !state.issuerAuthoritySecret) {
    throw new Error(
      'state is missing an approved issuer; run bootstrap_issuer.ts first',
    );
  }

  const schemaHash = Uint8Array.from(Buffer.from(state.schemaHash, 'hex'));
  const merkleTree = new PublicKey(state.merkleTreeAddress);
  if (merkleTree.equals(PublicKey.default)) {
    throw new Error(
      'state.merkleTreeAddress is unset. Create an SPL AC tree whose ' +
      'authority PDA is [b"tree-authority", schemaHash] and set ' +
      'SOLID_TREE_PUBKEY before running initialize.ts.',
    );
  }

  // 1. Load issuer identities from bootstrap_issuer state; generate a
  //    fresh holder per-run.
  console.log('[1/3] Loading keypairs...');
  const issuerBjj = {
    private_key: Uint8Array.from(state.issuerBjj.private_key),
    public_key_x: Uint8Array.from(state.issuerBjj.public_key_x),
    public_key_y: Uint8Array.from(state.issuerBjj.public_key_y),
  };
  const issuerAuthority = Keypair.fromSecretKey(
    Uint8Array.from(state.issuerAuthoritySecret),
  );
  const holderMaster = generateKeypair();

  // Derive the holder's per-schema keypair the circuit expects. We bind this
  // to the credential so the holder can prove knowledge later without needing
  // to re-derive at proof time.
  const holderSchemaKp = deriveCredentialKey(holderMaster.private_key, schemaHash);

  // 2. Define attestation payload (8 u64 fields).
  const attestationData: bigint[] = [
    25n,            // age
    840n,           // country_code (US)
    1n,             // region
    0n,             // id_type (passport)
    2n,             // verification_level (high)
    BigInt(Math.floor(Date.now() / 1000)),
    840n,
    0n,
  ];

  // 3. Issue credential (CPIs into SPL AC append).
  console.log('[2/3] Issuing credential on-chain...');
  const credential = await issueCredential(
    issuerBjj.private_key,
    issuerBjj.public_key_x,
    issuerBjj.public_key_y,
    {
      schemaHash,
      attestationData,
      holderPubKeyX: holderSchemaKp.public_key_x,
      holderPubKeyY: holderSchemaKp.public_key_y,
    },
    {
      connection,
      issuerAuthority,
      merkleTree,
      // SOLID-SEC-003: `schema_account` PDA is derived from
      // (name, version); both must match what initialize.ts passed
      // to `register_schema`.
      schemaName: state.schemaName,
      schemaVersion: state.schemaVersion,
      extraSigners: [wallet], // wallet pays fees if issuerAuthority is unfunded
    },
  );
  console.log(`   commitment: ${Buffer.from(credential.commitment).toString('hex')}`);
  console.log(`   tx:         ${credential.signature}`);

  // 4. Persist to state.
  console.log('[3/3] Saving state...');
  state.holderMaster = {
    private_key: Array.from(holderMaster.private_key),
    public_key_x: Array.from(holderMaster.public_key_x),
    public_key_y: Array.from(holderMaster.public_key_y),
  };
  state.holderSchema = {
    private_key: Array.from(holderSchemaKp.private_key),
    public_key_x: Array.from(holderSchemaKp.public_key_x),
    public_key_y: Array.from(holderSchemaKp.public_key_y),
  };
  state.credential = {
    ...credential,
    schemaHash: Array.from(credential.schemaHash),
    attestationData: credential.attestationData.map(n => n.toString()),
    issuerPubKeyX: Array.from(credential.issuerPubKeyX),
    issuerPubKeyY: Array.from(credential.issuerPubKeyY),
    holderPubKeyX: Array.from(credential.holderPubKeyX),
    holderPubKeyY: Array.from(credential.holderPubKeyY),
    salt: Array.from(credential.salt),
    commitment: Array.from(credential.commitment),
    issuerSignature: {
      r8_x: Array.from(credential.issuerSignature.r8_x),
      r8_y: Array.from(credential.issuerSignature.r8_y),
      s: Array.from(credential.issuerSignature.s),
    },
  };
  state.attestationData = attestationData.map(n => n.toString());
  writeState(state);
  console.log('Done.');
}

main().catch(e => {
  console.error(e);
  process.exit(1);
});
