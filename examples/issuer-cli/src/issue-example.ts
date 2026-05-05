import { Connection, Keypair, PublicKey } from '@solana/web3.js';
import { issueCredential } from '@solid-protocol/issuer';
import {
  DEFAULT_DEVNET_MANIFEST,
  programIdsFromManifest,
} from '@solid-protocol/sdk/manifest';

const required = [
  'SOLID_ISSUER_KEYPAIR_JSON',
  'SOLID_SCHEMA_HASH',
  'SOLID_SCHEMA_TREE',
  'SOLID_HOLDER_BJJ_X',
  'SOLID_HOLDER_BJJ_Y',
  'SOLID_ISSUER_BJJ_PRIVATE_KEY',
  'SOLID_ISSUER_BJJ_X',
  'SOLID_ISSUER_BJJ_Y',
] as const;

for (const key of required) {
  if (!process.env[key]) throw new Error(`Missing ${key}`);
}

const issuer = Keypair.fromSecretKey(
  Uint8Array.from(JSON.parse(process.env.SOLID_ISSUER_KEYPAIR_JSON!) as number[]),
);
const connection = new Connection(process.env.SOLID_RPC_URL ?? DEFAULT_DEVNET_MANIFEST.cluster, 'confirmed');
const programIds = programIdsFromManifest(DEFAULT_DEVNET_MANIFEST);
const fields = parseFields(process.env.SOLID_FIELDS ?? '');

const issued = await issueCredential(
  hexToBytes(process.env.SOLID_ISSUER_BJJ_PRIVATE_KEY!),
  hexToBytes(process.env.SOLID_ISSUER_BJJ_X!),
  hexToBytes(process.env.SOLID_ISSUER_BJJ_Y!),
  {
    schemaHash: hexToBytes(process.env.SOLID_SCHEMA_HASH!),
    attestationData: fields,
    holderPubKeyX: hexToBytes(process.env.SOLID_HOLDER_BJJ_X!),
    holderPubKeyY: hexToBytes(process.env.SOLID_HOLDER_BJJ_Y!),
    expirationTimestamp: Number(process.env.SOLID_EXPIRATION_TS ?? '0'),
  },
  {
    connection,
    issuerAuthority: issuer,
    merkleTree: new PublicKey(process.env.SOLID_SCHEMA_TREE!),
    schemaName: process.env.SOLID_SCHEMA_NAME ?? 'basic_identity_v1',
    schemaVersion: Number(process.env.SOLID_SCHEMA_VERSION ?? '1'),
  },
);

console.log(JSON.stringify({
  action: 'credential-issued',
  rpc: connection.rpcEndpoint,
  issuer: issuer.publicKey.toBase58(),
  issuerRegistry: programIds.issuerRegistry,
  schemaRegistry: programIds.schemaRegistry,
  schemaHash: process.env.SOLID_SCHEMA_HASH,
  schemaTree: issued.merkleTree,
  txSignature: issued.signature,
  commitment: bytesToHex(issued.commitment),
}, null, 2));

function parseFields(value: string): bigint[] {
  const parts = value.split(',').map((part) => part.trim()).filter(Boolean);
  if (parts.length !== 8) {
    throw new Error('SOLID_FIELDS must contain exactly 8 comma-separated integer field values');
  }
  return parts.map((part) => BigInt(part));
}

function hexToBytes(hex: string): Uint8Array {
  const clean = hex.startsWith('0x') ? hex.slice(2) : hex;
  if (!/^[0-9a-fA-F]+$/.test(clean) || clean.length % 2 !== 0) {
    throw new Error(`Invalid hex string: ${hex}`);
  }
  return Uint8Array.from(clean.match(/../g)!.map((byte) => parseInt(byte, 16)));
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
}
