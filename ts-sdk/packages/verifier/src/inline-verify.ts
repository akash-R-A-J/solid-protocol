/**
 * SolID Verifier SDK — Inline (1-Transaction) Proof Verification
 *
 * Uses the existing `verify_batch_proof` instruction (NOT the v2 buffer path)
 * with Versioned Transactions + Address Lookup Tables to fit the proof data
 * into a single Solana transaction.
 *
 * The legacy verify_batch_proof instruction was already deployed and functional,
 * but its instruction data (1269 bytes with ComputeBudget) exceeded Solana's
 * 1232-byte legacy tx limit by 37 bytes. Versioned Transactions (v0) with ALTs
 * compress account keys, recovering ~300 bytes of headroom.
 *
 * This module provides:
 * - `buildSingleTxVerification()` — builds a single VersionedTransaction
 * - `getOrCreateVerifierALT()` — ALT management for common verifier accounts
 */
import { Buffer } from 'buffer';
import { PROGRAM_IDS } from '@solid-protocol/core';
import {
  AddressLookupTableAccount,
  AddressLookupTableProgram,
  ComputeBudgetProgram,
  Connection,
  PublicKey,
  SystemProgram,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';

import {
  NR_WIRE_INPUTS,
  WIRE_INPUT_SLOTS,
  NR_PUBLIC_INPUTS,
  deriveVerifierConfigPda,
  deriveVkStoragePda,
  deriveNullifierPda,
  extractWirePublicInputs,
  type SchemaTreeAccounts,
} from './proof-buffer';

// The discriminator for the ORIGINAL verify_batch_proof instruction (not v2).
// This is the 8-byte Anchor discriminator derived from
// `global:verify_batch_proof` using SHA-256.
const VERIFY_BATCH_PROOF_DISCRIMINATOR = Uint8Array.from([
  245, 139, 191, 33, 110, 104, 4, 71,
]);

/**
 * Build the original `verify_batch_proof` instruction that accepts proof
 * data inline in the instruction data. This is the instruction at lib.rs:435.
 *
 * Instruction data layout:
 *   [0..8)    discriminator
 *   [8..72)   proof_a (64 bytes, fixed array)
 *   [72..200) proof_b (128 bytes, fixed array)
 *   [200..264) proof_c (64 bytes, fixed array)
 *   [264..268) public_inputs Vec length prefix (u32 LE = NR_WIRE_INPUTS * 32)
 *   [268..940) public_inputs flat bytes (21 * 32 = 672 bytes)
 *   [940..972) nullifier (32 bytes, fixed array)
 *
 * Total: 972 bytes
 */
export function buildVerifyBatchProofInlineIx(params: {
  payer: PublicKey;
  proofA: Uint8Array;
  proofB: Uint8Array;
  proofC: Uint8Array;
  wireInputs: Uint8Array[];
  nullifier: Uint8Array;
  trees: SchemaTreeAccounts;
  programId?: PublicKey;
}): TransactionInstruction {
  const programId = params.programId ?? new PublicKey(PROGRAM_IDS.zkVerifier);

  if (params.proofA.length !== 64) throw new Error('proofA must be 64 bytes');
  if (params.proofB.length !== 128) throw new Error('proofB must be 128 bytes');
  if (params.proofC.length !== 64) throw new Error('proofC must be 64 bytes');
  if (params.nullifier.length !== 32) throw new Error('nullifier must be 32 bytes');
  if (params.wireInputs.length !== NR_WIRE_INPUTS) {
    throw new Error(`wireInputs must have ${NR_WIRE_INPUTS} entries`);
  }

  // Borsh-serialize the instruction data
  const wireInputsFlat = new Uint8Array(NR_WIRE_INPUTS * 32);
  for (let i = 0; i < NR_WIRE_INPUTS; i++) {
    if (params.wireInputs[i].length !== 32) throw new Error(`wireInput[${i}] must be 32 bytes`);
    wireInputsFlat.set(params.wireInputs[i], i * 32);
  }

  const vecLenPrefix = new Uint8Array(4);
  new DataView(vecLenPrefix.buffer).setUint32(0, wireInputsFlat.length, true);

  const data = Buffer.concat([
    Buffer.from(VERIFY_BATCH_PROOF_DISCRIMINATOR),
    Buffer.from(params.proofA),       // proof_a: [u8; 64]
    Buffer.from(params.proofB),       // proof_b: [u8; 128]
    Buffer.from(params.proofC),       // proof_c: [u8; 64]
    Buffer.from(vecLenPrefix),        // Vec<u8> length prefix
    Buffer.from(wireInputsFlat),      // Vec<u8> payload (672 bytes)
    Buffer.from(params.nullifier),    // nullifier: [u8; 32]
  ]);

  const [configPda] = deriveVerifierConfigPda(programId);
  const [vkPda] = deriveVkStoragePda(configPda, programId);
  const [nullifierPda] = deriveNullifierPda(params.nullifier, programId);

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
      { pubkey: params.payer, isSigner: true, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });
}

/**
 * Build a single VersionedTransaction (v0) that verifies a batch proof
 * in one transaction using the original `verify_batch_proof` instruction
 * plus an Address Lookup Table for account compression.
 *
 * Returns null if the ALT is not available (caller should fall back to v2 buffer).
 */
export async function buildSingleTxVerification(params: {
  connection: Connection;
  payer: PublicKey;
  proofA: Uint8Array;
  proofB: Uint8Array;
  proofC: Uint8Array;
  publicSignals: Uint8Array[];
  nullifier: Uint8Array;
  trees: SchemaTreeAccounts;
  altAddress?: PublicKey;
}): Promise<VersionedTransaction | null> {
  const wireInputs = extractWirePublicInputs(params.publicSignals);

  const verifyIx = buildVerifyBatchProofInlineIx({
    payer: params.payer,
    proofA: params.proofA,
    proofB: params.proofB,
    proofC: params.proofC,
    wireInputs,
    nullifier: params.nullifier,
    trees: params.trees,
  });

  const computeBudgetIx = ComputeBudgetProgram.setComputeUnitLimit({
    units: 800_000,
  });

  // If no ALT provided, try to find one in the manifest
  let altAddress: PublicKey | undefined = params.altAddress;
  if (!altAddress) {
    altAddress = await findDeployedALT() ?? undefined;
  }
  if (!altAddress) {
    // No ALT available — cannot fit in one tx
    return null;
  }

  // Fetch the ALT
  const altAccountInfo = await params.connection.getAddressLookupTable(altAddress);
  if (!altAccountInfo.value) {
    // ALT doesn't exist on-chain
    return null;
  }

  const lookupTable = altAccountInfo.value;

  // Build the versioned transaction
  const { blockhash } = await params.connection.getLatestBlockhash('confirmed');

  const messageV0 = new TransactionMessage({
    payerKey: params.payer,
    recentBlockhash: blockhash,
    instructions: [computeBudgetIx, verifyIx],
  }).compileToV0Message([lookupTable]);

  const tx = new VersionedTransaction(messageV0);

  // Verify the tx fits in the packet limit
  const serialized = tx.serialize();
  if (serialized.length > 1232) {
    console.warn(
      `[SolID] Inline verify tx is ${serialized.length} bytes (limit 1232). Falling back to buffer path.`,
    );
    return null;
  }

  return tx;
}

/**
 * Try to find a deployed ALT address from the environment or manifest.
 */
async function findDeployedALT(): Promise<PublicKey | null> {
  // Check env var first (works in both Vite and Node environments)
  let envAlt: string | undefined;
  try {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    envAlt = (import.meta as any)?.env?.VITE_SOLID_VERIFIER_ALT;
  } catch {
    // import.meta.env not available (Node / non-Vite)
  }
  if (!envAlt && typeof process !== 'undefined') {
    envAlt = process.env.SOLID_VERIFIER_ALT;
  }

  if (envAlt) {
    try {
      return new PublicKey(envAlt);
    } catch {
      // Invalid pubkey, ignore
    }
  }

  return null;
}

/**
 * Create an Address Lookup Table containing all common verifier accounts.
 * This is a one-time setup operation for each deployment.
 *
 * The ALT includes:
 * - zkVerifier program ID
 * - VerifierConfig PDA
 * - VkStorage PDA
 * - SystemProgram
 * - GlobalTree binding PDA
 * - IssuerTreeBinding PDA
 * - Schema tree binding PDAs (from manifest)
 *
 * Returns the ALT address for storage in deployments/devnet.json.
 */
export async function createVerifierALT(params: {
  connection: Connection;
  payer: PublicKey;
  additionalAddresses?: PublicKey[];
}): Promise<{
  createIx: TransactionInstruction;
  extendIx: TransactionInstruction;
  altAddress: PublicKey;
}> {
  const programId = new PublicKey(PROGRAM_IDS.zkVerifier);
  const [configPda] = deriveVerifierConfigPda(programId);
  const [vkPda] = deriveVkStoragePda(configPda, programId);

  const slot = await params.connection.getSlot('confirmed');
  const [createIx, altAddress] = AddressLookupTableProgram.createLookupTable({
    authority: params.payer,
    payer: params.payer,
    recentSlot: slot,
  });

  const addresses = [
    programId,
    configPda,
    vkPda,
    SystemProgram.programId,
    ...(params.additionalAddresses ?? []),
  ];

  const extendIx = AddressLookupTableProgram.extendLookupTable({
    payer: params.payer,
    authority: params.payer,
    lookupTable: altAddress,
    addresses,
  });

  return { createIx, extendIx, altAddress };
}
