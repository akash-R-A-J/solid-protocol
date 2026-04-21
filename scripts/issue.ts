/**
 * SolID Protocol — issuance script (E2E step 2).
 *
 * Depends on scripts/initialize.ts having run first (reads state from
 * scripts/e2e_state.json).
 *
 * Responsibilities:
 *   1. Load on-chain state prepared by initialize.ts.
 *   2. Derive a holder identity (per-run) and a holder per-schema subkey.
 *   3. Register the issuer in issuer-registry (stake + vote + approve).
 *   4. Call issuer-registry::issue_credential which CPIs into SPL AC.
 *   5. Push the refreshed tree root into schema_registry::update_tree_root.
 *   6. Persist the credential + holder identity into scripts/e2e_state.json.
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

  console.log('SolID Protocol — credential issuance');
  console.log('====================================');

  if (!fs.existsSync('scripts/e2e_state.json')) {
    throw new Error('Run initialize.ts first');
  }
  const state = JSON.parse(fs.readFileSync('scripts/e2e_state.json', 'utf-8'));

  const schemaHash = Uint8Array.from(Buffer.from(state.schemaHash, 'hex'));
  const merkleTree = new PublicKey(state.merkleTreeAddress);
  if (merkleTree.equals(PublicKey.default)) {
    throw new Error(
      'scripts/e2e_state.json.merkleTreeAddress is unset. Create an SPL AC ' +
      'tree whose authority PDA is [b"tree-authority", schemaHash] and set ' +
      'SOLID_TREE_PUBKEY before running initialize.ts.',
    );
  }

  // 1. Generate identities (issuer BJJ + holder master BJJ).
  //    The issuer's Solana authority below is a fresh keypair; in production
  //    it would be the issuer's operational key already staked in the DAO.
  console.log('[1/3] Generating keypairs...');
  const issuerBjj = generateKeypair();
  const holderMaster = generateKeypair();
  const issuerAuthority = Keypair.generate();

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
      extraSigners: [wallet], // wallet pays fees if issuerAuthority is unfunded
    },
  );
  console.log(`   commitment: ${Buffer.from(credential.commitment).toString('hex')}`);
  console.log(`   tx:         ${credential.signature}`);

  // 4. Persist to state.
  console.log('[3/3] Saving state...');
  state.issuerBjj = {
    private_key: Array.from(issuerBjj.private_key),
    public_key_x: Array.from(issuerBjj.public_key_x),
    public_key_y: Array.from(issuerBjj.public_key_y),
  };
  state.issuerAuthoritySecret = Array.from(issuerAuthority.secretKey);
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
  fs.writeFileSync('scripts/e2e_state.json', JSON.stringify(state, null, 2));
  console.log('Done.');
}

main().catch(e => {
  console.error(e);
  process.exit(1);
});
