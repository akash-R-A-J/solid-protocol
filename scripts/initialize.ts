/**
 * SolID Protocol — on-chain initialization script.
 *
 * Bootstraps the three programs on a freshly deployed cluster. Safe to re-run:
 * every step is idempotent and ignores "already initialised" failures.
 *
 * Steps:
 *   1. initialize_registry (issuer-registry)
 *   2. register_schema (schema-registry)
 *   3. initialize_tree_binding (schema-registry)
 *   4. initialize_global_binding (schema-registry)
 *   5. initialize (zk-verifier)
 *   6. store_verification_key in chunks (zk-verifier)
 *
 * Program IDs are sourced from @solid-protocol/core::PROGRAM_IDS, the single
 * source of truth mirrored in Anchor.toml and enforced by CI via
 * scripts/check_program_ids.py.
 */

import { Connection, Keypair, PublicKey, SystemProgram } from '@solana/web3.js';
import * as anchor from '@coral-xyz/anchor';
import { initWasm, poseidonHashBytes, PROGRAM_IDS } from '@solid-protocol/core';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { stateFilePath, writeState } from './lib/e2e_state';

const PROGRAM_PUBKEYS = {
  schemaRegistry: new PublicKey(PROGRAM_IDS.schemaRegistry),
  zkVerifier: new PublicKey(PROGRAM_IDS.zkVerifier),
  issuerRegistry: new PublicKey(PROGRAM_IDS.issuerRegistry),
};

const RPC_URL = process.env.SOLID_RPC_URL ?? 'http://127.0.0.1:8899';
const SCHEMA_NAME = 'basic_identity_v1';
const SCHEMA_VERSION = 1;
const SCHEMA_FIELDS = [
  'age', 'country_code', 'region', 'id_type',
  'verification_level', 'issued_date', 'nationality', '_reserved',
];

const keypairPath = path.join(os.homedir(), '.config/solana/id.json');
const secretKey = JSON.parse(fs.readFileSync(keypairPath, 'utf-8'));
const wallet = Keypair.fromSecretKey(Uint8Array.from(secretKey));
const connection = new Connection(RPC_URL, 'confirmed');

const provider = new anchor.AnchorProvider(
  connection,
  new anchor.Wallet(wallet),
  { commitment: 'confirmed' },
);
anchor.setProvider(provider);

function fieldToBytesBE(s: string): Uint8Array {
  let n = BigInt(s);
  const bytes = new Uint8Array(32);
  for (let i = 31; i >= 0; i--) {
    bytes[i] = Number(n & 0xFFn);
    n >>= 8n;
  }
  return bytes;
}

function serializeG1(point: string[]): Uint8Array {
  const buf = new Uint8Array(64);
  buf.set(fieldToBytesBE(point[0]), 0);
  buf.set(fieldToBytesBE(point[1]), 32);
  return buf;
}

function serializeG2(point: string[][]): Uint8Array {
  const buf = new Uint8Array(128);
  buf.set(fieldToBytesBE(point[0][0]), 0);
  buf.set(fieldToBytesBE(point[0][1]), 32);
  buf.set(fieldToBytesBE(point[1][0]), 64);
  buf.set(fieldToBytesBE(point[1][1]), 96);
  return buf;
}

function loadIdl(name: string): any {
  const snake = name.replace(/[A-Z]/g, l => `_${l.toLowerCase()}`);
  for (const filename of [name, snake]) {
    for (const subdir of ['idl', 'types', 'deploy']) {
      const p = path.join('target', subdir, `${filename}.json`);
      if (fs.existsSync(p)) {
        const idl = JSON.parse(fs.readFileSync(p, 'utf-8'));
        if (!idl.address) {
          const pid = PROGRAM_PUBKEYS[name as keyof typeof PROGRAM_PUBKEYS];
          if (pid) idl.address = pid.toBase58();
        }
        return idl;
      }
    }
  }
  throw new Error(`IDL for ${name} not found in target/`);
}

function isAlreadyInitialised(e: any): boolean {
  const m = String(e?.message ?? e);
  return m.includes('already in use') || m.includes('already initialized');
}

async function main() {
  console.log('SolID Protocol on-chain initialization');
  console.log('======================================');
  console.log(`Wallet: ${wallet.publicKey.toBase58()}`);
  console.log(`RPC:    ${RPC_URL}`);

  const issuerIdl = loadIdl('issuerRegistry');
  const schemaIdl = loadIdl('schemaRegistry');
  const zkIdl = loadIdl('zkVerifier');

  const issuerProgram = new anchor.Program(issuerIdl, provider);
  const schemaProgram = new anchor.Program(schemaIdl, provider);
  const zkProgram = new anchor.Program(zkIdl, provider);

  await initWasm();

  // 1. Initialize issuer registry.
  console.log('\n[1/6] initialize_registry');
  const [registryPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('registry-config')],
    PROGRAM_PUBKEYS.issuerRegistry,
  );
  const governanceMint = process.env.SOLID_GOVERNANCE_MINT
    ? new PublicKey(process.env.SOLID_GOVERNANCE_MINT)
    : PublicKey.default;
  try {
    await issuerProgram.methods.initializeRegistry(
      governanceMint,
      new anchor.BN(1_000_000_000),
      new anchor.BN(86400),
      new anchor.BN(6000),
    ).accounts({
      registryConfig: registryPda,
      authority: wallet.publicKey,
      systemProgram: SystemProgram.programId,
    }).rpc();
    console.log('   ok (initialised)');
  } catch (e: any) {
    if (!isAlreadyInitialised(e)) throw e;
    console.log('   ok (already active)');
  }

  // 2. Register schema.
  console.log('\n[2/6] register_schema');
  const metadataBytes = Buffer.from(SCHEMA_FIELDS.join(','));
  const chunkCount = Math.ceil(metadataBytes.length / 32);
  const schemaChunks: Uint8Array[] = [];
  for (let i = 0; i < chunkCount; i++) {
    const chunk = new Uint8Array(32);
    chunk.set(metadataBytes.subarray(i * 32, (i + 1) * 32));
    schemaChunks.push(chunk);
  }
  const schemaHash = poseidonHashBytes(schemaChunks);
  console.log(`   schema_hash: ${Buffer.from(schemaHash).toString('hex')}`);
  const [schemaPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('schema'), Buffer.from(SCHEMA_NAME), Buffer.from([SCHEMA_VERSION])],
    PROGRAM_PUBKEYS.schemaRegistry,
  );
  try {
    await schemaProgram.methods.registerSchema(
      SCHEMA_NAME,
      SCHEMA_VERSION,
      'Identity',
      SCHEMA_FIELDS,
      Array.from(schemaHash),
    ).accounts({
      schemaAccount: schemaPda,
      authority: wallet.publicKey,
      systemProgram: SystemProgram.programId,
    }).rpc();
    console.log('   ok (registered)');
  } catch (e: any) {
    if (!isAlreadyInitialised(e)) throw e;
    console.log('   ok (already active)');
  }

  // 3. SchemaTreeBinding.
  console.log('\n[3/6] initialize_tree_binding');
  const treePubkey = process.env.SOLID_TREE_PUBKEY
    ? new PublicKey(process.env.SOLID_TREE_PUBKEY)
    : PublicKey.default;
  const [bindingPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('schema-tree-binding'), Buffer.from(schemaHash)],
    PROGRAM_PUBKEYS.schemaRegistry,
  );
  try {
    await schemaProgram.methods.initializeTreeBinding(
      Array.from(schemaHash),
      treePubkey,
    ).accounts({
      schemaAccount: schemaPda,
      schemaTreeBinding: bindingPda,
      authority: wallet.publicKey,
      systemProgram: SystemProgram.programId,
    }).rpc();
    console.log('   ok (binding created)');
  } catch (e: any) {
    if (!isAlreadyInitialised(e)) throw e;
    console.log('   ok (already active)');
  }

  // 4. GlobalStateBinding.
  console.log('\n[4/6] initialize_global_binding');
  const [globalBindingPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('global-binding')],
    PROGRAM_PUBKEYS.schemaRegistry,
  );
  try {
    await schemaProgram.methods.initializeGlobalBinding().accounts({
      globalBinding: globalBindingPda,
      authority: wallet.publicKey,
      systemProgram: SystemProgram.programId,
    }).rpc();
    console.log('   ok (global binding created)');
  } catch (e: any) {
    if (!isAlreadyInitialised(e)) throw e;
    console.log('   ok (already active)');
  }

  // 5. Verifier config.
  console.log('\n[5/6] zk_verifier.initialize');
  const [verifierConfigPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('verifier-config')],
    PROGRAM_PUBKEYS.zkVerifier,
  );
  try {
    await zkProgram.methods.initialize().accounts({
      verifierConfig: verifierConfigPda,
      authority: wallet.publicKey,
      systemProgram: SystemProgram.programId,
    }).rpc();
    console.log('   ok (initialised)');
  } catch (e: any) {
    if (!isAlreadyInitialised(e)) throw e;
    console.log('   ok (already active)');
  }

  // 6. Upload verification key.
  console.log('\n[6/6] store_verification_key');
  const [vkStoragePda] = PublicKey.findProgramAddressSync(
    [Buffer.from('vk-storage'), verifierConfigPda.toBuffer()],
    PROGRAM_PUBKEYS.zkVerifier,
  );
  const vkJsonPath = 'circuits/build/verification_key.json';
  if (!fs.existsSync(vkJsonPath)) {
    console.error(
      `   verification_key.json missing at ${vkJsonPath}. ` +
      `Run "cd circuits && node scripts/setup.js" first.`,
    );
    process.exit(1);
  }
  const vkJson = JSON.parse(fs.readFileSync(vkJsonPath, 'utf-8'));
  const icLen = vkJson.IC.length;
  const vkBytes = Buffer.concat([
    Buffer.from(Uint32Array.from([icLen]).buffer),
    Buffer.from(serializeG1(vkJson.vk_alpha_1)),
    Buffer.from(serializeG2(vkJson.vk_beta_2)),
    Buffer.from(serializeG2(vkJson.vk_gamma_2)),
    Buffer.from(serializeG2(vkJson.vk_delta_2)),
    Buffer.concat((vkJson.IC as string[][]).map((p: string[]) => Buffer.from(serializeG1(p)))),
  ]);
  const CHUNK_SIZE = 900;
  const totalChunks = Math.ceil(vkBytes.length / CHUNK_SIZE);
  for (let i = 0; i < totalChunks; i++) {
    const chunk = vkBytes.subarray(i * CHUNK_SIZE, (i + 1) * CHUNK_SIZE);
    await zkProgram.methods.storeVerificationKey(
      i,
      Array.from(chunk),
      i === totalChunks - 1,
    ).accounts({
      verifierConfig: verifierConfigPda,
      vkStorage: vkStoragePda,
      authority: wallet.publicKey,
      systemProgram: SystemProgram.programId,
    }).rpc();
    process.stdout.write(`   uploaded chunk ${i + 1}/${totalChunks}\r`);
  }
  console.log(`\n   ok (${vkBytes.length} bytes)`);

  // Persist state for downstream scripts.  Written under
  // `$XDG_RUNTIME_DIR` / `$TMPDIR` with mode 0600 (SOLID-SEC-020);
  // never under the repo working tree.
  const state = {
    schemaName: SCHEMA_NAME,
    schemaVersion: SCHEMA_VERSION,
    schemaHash: Buffer.from(schemaHash).toString('hex'),
    schemaPda: schemaPda.toBase58(),
    registryPda: registryPda.toBase58(),
    schemaTreeBindingPda: bindingPda.toBase58(),
    globalBindingPda: globalBindingPda.toBase58(),
    verifierConfigPda: verifierConfigPda.toBase58(),
    vkStoragePda: vkStoragePda.toBase58(),
    merkleTreeAddress: treePubkey.toBase58(),
  };
  const written = writeState(state);
  console.log(`\nWrote ${written}`);
  console.log(`(SOLID_E2E_STATE_FILE can override; default is ${stateFilePath()})`);
  console.log('Done.');
}

main().catch(e => {
  console.error(e);
  process.exit(1);
});
