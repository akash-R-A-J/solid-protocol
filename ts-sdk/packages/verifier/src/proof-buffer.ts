import { Buffer } from 'buffer';
import { PROGRAM_IDS } from '@solid-protocol/core';
import {
  PublicKey,
  SystemProgram,
  TransactionInstruction,
} from '@solana/web3.js';

export const NR_PUBLIC_INPUTS = 32;
export const NR_WIRE_INPUTS = 21;
export const WIRE_INPUT_SLOTS = [
  0, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 30, 31,
] as const;

export const PROOF_BUFFER_PAYLOAD_SIZE = 64 + 128 + 64 + 32 + NR_WIRE_INPUTS * 32;
export const DEFAULT_PROOF_CHUNK_SIZE = 480;

export const INIT_PROOF_BUFFER_DISCRIMINATOR = Uint8Array.from([49, 27, 28, 88, 19, 99, 133, 194]);
export const UPLOAD_PROOF_CHUNK_DISCRIMINATOR = Uint8Array.from([60, 215, 88, 47, 168, 107, 123, 150]);
export const VERIFY_BATCH_PROOF_V2_DISCRIMINATOR = Uint8Array.from([130, 180, 34, 65, 88, 72, 187, 204]);
export const VERIFIER_CONFIG_DISCRIMINATOR = Uint8Array.from([176, 103, 248, 36, 138, 167, 176, 220]);

const VERIFIER_CONFIG_SEED = Buffer.from('verifier-config');
const VK_STORAGE_SEED = Buffer.from('vk-storage');
const NULLIFIER_SEED = Buffer.from('null');
const PROOF_BUFFER_SEED = Buffer.from('proof-buffer');

export interface SchemaTreeAccounts {
  schemaTree0: PublicKey;
  schemaTree1: PublicKey;
  schemaTree2: PublicKey;
  schemaTree3: PublicKey;
  globalTree: PublicKey;
  issuerTreeBinding: PublicKey;
}

export interface VerifierConfigSummary {
  authority: string;
  proofCount: number;
  vkInitialized: boolean;
  paused: boolean;
  bump: number;
  nextVkChunk: number;
  timestampSkewSeconds: number;
  vkFinalized: boolean;
  vkGeneration: number;
  rotateRequestTs: bigint;
}

export function verifierProgramId(): PublicKey {
  return new PublicKey(PROGRAM_IDS.zkVerifier);
}

export function deriveVerifierConfigPda(programId = verifierProgramId()): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([VERIFIER_CONFIG_SEED], programId);
}

export function deriveVkStoragePda(
  configPda: PublicKey,
  programId = verifierProgramId(),
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [VK_STORAGE_SEED, configPda.toBuffer()],
    programId,
  );
}

export function deriveNullifierPda(
  nullifier: Uint8Array,
  programId = verifierProgramId(),
): [PublicKey, number] {
  if (nullifier.length !== 32) throw new Error('nullifier must be 32 bytes');
  return PublicKey.findProgramAddressSync(
    [NULLIFIER_SEED, Buffer.from(nullifier)],
    programId,
  );
}

export function deriveProofBufferPda(
  payer: PublicKey,
  programId = verifierProgramId(),
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [PROOF_BUFFER_SEED, payer.toBuffer()],
    programId,
  );
}

export function extractWirePublicInputs(publicSignals: Uint8Array[]): Uint8Array[] {
  if (publicSignals.length !== NR_PUBLIC_INPUTS) {
    throw new Error(
      `extractWirePublicInputs expects exactly ${NR_PUBLIC_INPUTS} entries, got ${publicSignals.length}`,
    );
  }
  return WIRE_INPUT_SLOTS.map(i => publicSignals[i]);
}

export function encodeProofBufferPayload(
  proofA: Uint8Array,
  proofB: Uint8Array,
  proofC: Uint8Array,
  nullifier: Uint8Array,
  wireInputs: Uint8Array[],
): Uint8Array {
  if (proofA.length !== 64) throw new Error('proofA must be 64 bytes');
  if (proofB.length !== 128) throw new Error('proofB must be 128 bytes');
  if (proofC.length !== 64) throw new Error('proofC must be 64 bytes');
  if (nullifier.length !== 32) throw new Error('nullifier must be 32 bytes');
  if (wireInputs.length !== NR_WIRE_INPUTS) {
    throw new Error(`wireInputs must have ${NR_WIRE_INPUTS} entries, got ${wireInputs.length}`);
  }
  for (const input of wireInputs) {
    if (input.length !== 32) throw new Error('each wire input must be 32 bytes');
  }

  const payload = new Uint8Array(PROOF_BUFFER_PAYLOAD_SIZE);
  let offset = 0;
  payload.set(proofA, offset); offset += 64;
  payload.set(proofB, offset); offset += 128;
  payload.set(proofC, offset); offset += 64;
  payload.set(nullifier, offset); offset += 32;
  for (const input of wireInputs) {
    payload.set(input, offset);
    offset += 32;
  }
  return payload;
}

export function buildInitProofBufferIx(params: {
  payer: PublicKey;
  programId?: PublicKey;
}): TransactionInstruction {
  const programId = params.programId ?? verifierProgramId();
  const [bufferPda] = deriveProofBufferPda(params.payer, programId);
  return new TransactionInstruction({
    programId,
    keys: [
      { pubkey: bufferPda, isSigner: false, isWritable: true },
      { pubkey: params.payer, isSigner: true, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: Buffer.from(INIT_PROOF_BUFFER_DISCRIMINATOR),
  });
}

export function buildUploadProofChunkIx(params: {
  payer: PublicKey;
  offset: number;
  bytes: Uint8Array;
  programId?: PublicKey;
}): TransactionInstruction {
  if (!Number.isInteger(params.offset) || params.offset < 0) throw new Error('offset must be a non-negative integer');
  const programId = params.programId ?? verifierProgramId();
  const [bufferPda] = deriveProofBufferPda(params.payer, programId);
  const data = Buffer.concat([
    Buffer.from(UPLOAD_PROOF_CHUNK_DISCRIMINATOR),
    Buffer.from(u32Le(params.offset)),
    Buffer.from(u32Le(params.bytes.length)),
    Buffer.from(params.bytes),
  ]);
  return new TransactionInstruction({
    programId,
    keys: [
      { pubkey: bufferPda, isSigner: false, isWritable: true },
      { pubkey: params.payer, isSigner: true, isWritable: false },
    ],
    data,
  });
}

export function buildVerifyBatchProofV2Ix(params: {
  payer: PublicKey;
  nullifier: Uint8Array;
  trees: SchemaTreeAccounts;
  programId?: PublicKey;
}): TransactionInstruction {
  const programId = params.programId ?? verifierProgramId();
  if (params.nullifier.length !== 32) throw new Error('nullifier must be 32 bytes');

  const [configPda] = deriveVerifierConfigPda(programId);
  const [vkPda] = deriveVkStoragePda(configPda, programId);
  const [nullifierPda] = deriveNullifierPda(params.nullifier, programId);
  const [bufferPda] = deriveProofBufferPda(params.payer, programId);

  return new TransactionInstruction({
    programId,
    keys: [
      { pubkey: configPda, isSigner: false, isWritable: true },
      { pubkey: vkPda, isSigner: false, isWritable: false },
      { pubkey: nullifierPda, isSigner: false, isWritable: true },
      { pubkey: params.trees.globalTree, isSigner: false, isWritable: false },
      { pubkey: params.trees.schemaTree0, isSigner: false, isWritable: false },
      { pubkey: params.trees.schemaTree1, isSigner: false, isWritable: false },
      { pubkey: params.trees.schemaTree2, isSigner: false, isWritable: false },
      { pubkey: params.trees.schemaTree3, isSigner: false, isWritable: false },
      { pubkey: params.trees.issuerTreeBinding, isSigner: false, isWritable: false },
      { pubkey: bufferPda, isSigner: false, isWritable: true },
      { pubkey: params.payer, isSigner: true, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: Buffer.from(concatBytes(VERIFY_BATCH_PROOF_V2_DISCRIMINATOR, params.nullifier)),
  });
}

export function decodeVerifierConfig(data: Buffer | Uint8Array): VerifierConfigSummary {
  assertDiscriminator(data, VERIFIER_CONFIG_DISCRIMINATOR, 'VerifierConfig');
  const reader = new BorshReader(data);
  reader.skip(8);
  return {
    authority: reader.publicKey(),
    proofCount: Number(reader.u64('verifier.proof_count')),
    vkInitialized: reader.bool('verifier.vk_initialized'),
    paused: reader.bool('verifier.paused'),
    bump: reader.u8('verifier.bump'),
    nextVkChunk: reader.u16('verifier.next_vk_chunk'),
    timestampSkewSeconds: reader.u32('verifier.timestamp_skew_seconds'),
    vkFinalized: reader.bool('verifier.vk_finalized'),
    vkGeneration: reader.u16('verifier.vk_generation'),
    rotateRequestTs: reader.i64('verifier.rotate_request_ts'),
  };
}

function u32Le(value: number): Uint8Array {
  if (!Number.isInteger(value) || value < 0 || value > 0xffffffff) throw new Error('u32 value out of range');
  const out = new Uint8Array(4);
  out[0] = value & 0xff;
  out[1] = (value >> 8) & 0xff;
  out[2] = (value >> 16) & 0xff;
  out[3] = (value >> 24) & 0xff;
  return out;
}

function concatBytes(...parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0));
  let offset = 0;
  for (const part of parts) {
    out.set(part, offset);
    offset += part.length;
  }
  return out;
}

class BorshReader {
  private offset = 0;
  private readonly data: Uint8Array;

  constructor(data: Buffer | Uint8Array) {
    this.data = data;
  }

  skip(bytes: number): void {
    this.ensure(bytes, 'skip');
    this.offset += bytes;
  }

  bytes(length: number, name: string): Uint8Array {
    this.ensure(length, name);
    const out = this.data.slice(this.offset, this.offset + length);
    this.offset += length;
    return out;
  }

  publicKey(): string {
    return new PublicKey(this.bytes(32, 'pubkey')).toBase58();
  }

  u8(name: string): number {
    this.ensure(1, name);
    return this.data[this.offset++];
  }

  bool(name: string): boolean {
    const value = this.u8(name);
    if (value !== 0 && value !== 1) throw new Error(`${name} must be a Borsh bool`);
    return value === 1;
  }

  u16(name: string): number {
    this.ensure(2, name);
    const value = this.data[this.offset] | (this.data[this.offset + 1] << 8);
    this.offset += 2;
    return value;
  }

  u32(name: string): number {
    this.ensure(4, name);
    const value = this.data[this.offset]
      | (this.data[this.offset + 1] << 8)
      | (this.data[this.offset + 2] << 16)
      | (this.data[this.offset + 3] << 24 >>> 0);
    this.offset += 4;
    return value;
  }

  u64(name: string): bigint {
    this.ensure(8, name);
    const value = readBigUInt64Le(this.data, this.offset);
    this.offset += 8;
    return value;
  }

  i64(name: string): bigint {
    this.ensure(8, name);
    const unsigned = readBigUInt64Le(this.data, this.offset);
    this.offset += 8;
    return unsigned > 0x7fffffffffffffffn ? unsigned - 0x10000000000000000n : unsigned;
  }

  private ensure(bytes: number, name: string): void {
    if (this.offset + bytes > this.data.length) {
      throw new Error(`${name} exceeds account data length`);
    }
  }
}

function assertDiscriminator(data: Buffer | Uint8Array, expected: Uint8Array, name: string): void {
  if (data.length < expected.length || !bytesEqual(data.subarray(0, expected.length), expected)) {
    throw new Error(`${name} discriminator mismatch`);
  }
}

function readBigUInt64Le(data: Uint8Array, offset: number): bigint {
  let value = 0n;
  for (let i = 7; i >= 0; i--) {
    value = (value << 8n) | BigInt(data[offset + i]);
  }
  return value;
}

function bytesEqual(left: Uint8Array, right: Uint8Array): boolean {
  if (left.length !== right.length) return false;
  return left.every((byte, index) => byte === right[index]);
}
