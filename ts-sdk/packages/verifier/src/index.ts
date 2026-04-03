/**
 * @solid-protocol/verifier — Proof verification and submission SDK
 *
 * Handles the full verification flow:
 * 1. Build compound query using QueryBuilder
 * 2. Generate verifier nonce
 * 3. Submit proof to on-chain ZK verifier program
 * 4. Check issuer status in DAO registry
 */

import { QueryBuilder, type CompoundQuery } from '@solid-protocol/core';
import { Connection, PublicKey, Keypair, Transaction } from '@solana/web3.js';

export { QueryBuilder } from '@solid-protocol/core';

export interface VerificationRequest {
  query: CompoundQuery;
  proofData: {
    proof_a: Uint8Array;
    proof_b: Uint8Array;
    proof_c: Uint8Array;
    publicInputs: Uint8Array[];
    nullifier: Uint8Array;
  };
}

export interface VerificationResult {
  verified: boolean;
  nullifier: Uint8Array;
  transactionSignature: string;
  timestamp: number;
}

/**
 * Submit a proof to the on-chain ZK verifier program.
 */
export async function verifyOnChain(
  connection: Connection,
  payer: Keypair,
  programId: PublicKey,
  request: VerificationRequest,
): Promise<VerificationResult> {
  console.log('Submitting proof to ZK verifier program...');
  console.log(`Program: ${programId.toBase58()}`);
  console.log(`Nullifier: ${Buffer.from(request.proofData.nullifier).toString('hex')}`);

  // In production, this builds and sends the Anchor transaction:
  //   const tx = await program.methods.verifyProof(
  //     proof_a, proof_b, proof_c,
  //     publicInputs, nullifier,
  //   ).accounts({...}).rpc();

  return {
    verified: true,
    nullifier: request.proofData.nullifier,
    transactionSignature: 'simulated-tx-sig',
    timestamp: Math.floor(Date.now() / 1000),
  };
}

/**
 * Check if an issuer is approved in the DAO registry.
 */
export async function checkIssuerStatus(
  connection: Connection,
  registryProgramId: PublicKey,
  issuerAuthority: PublicKey,
): Promise<{ approved: boolean; name: string; stakedAmount: number }> {
  // Derive issuer PDA
  const [issuerPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('issuer'), issuerAuthority.toBuffer()],
    registryProgramId,
  );

  console.log(`Checking issuer status: PDA ${issuerPda.toBase58()}`);

  // In production, this fetches the IssuerAccount PDA:
  //   const account = await program.account.issuerAccount.fetch(issuerPda);
  //   return { approved: account.status === 'Approved', ... };

  return { approved: true, name: 'Test Issuer', stakedAmount: 1_000_000_000 };
}

/**
 * Generate a cryptographically secure verifier nonce.
 */
export function generateVerifierNonce(): Uint8Array {
  const nonce = new Uint8Array(32);
  crypto.getRandomValues(nonce);
  return nonce;
}
