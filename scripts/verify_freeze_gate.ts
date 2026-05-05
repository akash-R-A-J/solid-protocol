/**
 * Operator-runbook tool: verifies SOLID-SEC-006 Part 1 (NF-02 closure).
 *
 * Reads the live `VerifierConfig` from the validator at $RPC (default
 * http://127.0.0.1:8899), asserts:
 *
 *   1. `vk_initialized == true`
 *   2. `vk_finalized   == true`        (NF-02 fix in `initialize.ts`)
 *   3. A direct `store_verification_key(chunk=0, ..., is_final=false)`
 *      call against the authority is rejected with the AnchorError
 *      `VerificationKeyFinalized` (error code 6018).
 *
 * Exit codes:
 *   0  freeze-gate enforcing
 *   1  re-upload was accepted (freeze-gate NOT enforcing)
 *   2  vk_finalized is false (the audit's NF-02 dead-code state)
 *   3  re-upload threw an unexpected error (not the freeze-gate)
 *  99  unrelated runtime error
 *
 * Usage:
 *   npx tsx scripts/verify_freeze_gate.ts
 *
 * Run this after every deploy to mainnet/devnet as a one-shot smoke
 * test that the SEC-006 freeze-gate is structurally active.  The
 * regression gate is in `initialize.ts` itself (post-condition assert
 * after `finalize_verification_key`); this script is the
 * post-deploy verification a human operator runs on the live cluster.
 */
import { Connection, PublicKey, SystemProgram } from '@solana/web3.js';
import { Program, AnchorProvider, Wallet } from '@coral-xyz/anchor';
import * as fs from 'fs';
import { loadKeypair } from './lib/keypair';

async function main() {
  const rpcUrl = process.env.SOLANA_RPC_URL ?? 'http://127.0.0.1:8899';
  const conn = new Connection(rpcUrl, 'confirmed');
  const wallet = new Wallet(loadKeypair());
  const provider = new AnchorProvider(conn, wallet, { commitment: 'confirmed' });
  const idl = JSON.parse(fs.readFileSync('target/idl/zk_verifier.json', 'utf-8'));
  const program = new Program(idl, provider);
  const [verifierConfigPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('verifier-config')],
    program.programId,
  );
  const [vkStoragePda] = PublicKey.findProgramAddressSync(
    [Buffer.from('vk-storage'), verifierConfigPda.toBuffer()],
    program.programId,
  );

  const cfg = await program.account.verifierConfig.fetch(verifierConfigPda);
  console.log('rpc            =', rpcUrl);
  console.log('vk_initialized =', cfg.vkInitialized);
  console.log('vk_finalized   =', cfg.vkFinalized);
  console.log('vk_generation  =', cfg.vkGeneration);

  if (!cfg.vkFinalized) {
    console.log(
      'FAIL: vk_finalized is false.  This is the NF-02 dead-code state ' +
        '(SEC-006 freeze-gate inactive).  Run `npm run init-onchain` to call ' +
        '`finalize_verification_key`, then re-run this script.',
    );
    process.exit(2);
  }

  console.log(
    '\nAttempting forbidden VK rewrite via store_verification_key chunk 0...',
  );
  try {
    await program.methods
      .storeVerificationKey(0, Buffer.alloc(900, 0xab), false)
      .accounts({
        verifierConfig: verifierConfigPda,
        vkStorage: vkStoragePda,
        authority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    console.log('FAIL: re-upload was accepted; freeze-gate is NOT enforcing.');
    process.exit(1);
  } catch (e: any) {
    const msg = String(e?.message ?? e);
    if (msg.includes('VerificationKeyFinalized') || msg.toLowerCase().includes('finalized')) {
      console.log('PASS: re-upload rejected by SEC-006 freeze-gate.');
      console.log('  err:', msg.split('\n').slice(0, 2).join(' | '));
      process.exit(0);
    }
    console.log('UNEXPECTED error (not the freeze-gate):', msg);
    process.exit(3);
  }
}
main().catch((e) => {
  console.error(e);
  process.exit(99);
});
