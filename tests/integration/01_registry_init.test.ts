/**
 * Integration test 01: RegistryConfig initialization.
 *
 * Exercises issuer_registry::initialize_registry against bankrun. Regresses
 * the 80-to-112-byte space fix: the pre-remediation registry allocated
 * 32 bytes too few for governance_token_mint, so serialization of the
 * initialised RegistryConfig silently corrupted 32 trailing bytes or
 * failed outright.
 *
 * This test is a hard gate: if anyone ever changes the `space = ...`
 * expression in programs/issuer-registry/src/lib.rs without also updating
 * RegistryConfig's fields, this test fails.
 */

import { startAnchor, BanksClient, ProgramTestContext } from 'solana-bankrun';
import { AnchorProvider, Program, BN, Wallet } from '@coral-xyz/anchor';
import { Connection, Keypair, PublicKey, SystemProgram } from '@solana/web3.js';
import * as fs from 'fs';
import * as path from 'path';

const ISSUER_REGISTRY_ID = new PublicKey(
  'CRGYfonXwDk6gKEm9fC1U33VVBkqnQVD3sPdLKzqHWoR',
);

describe('issuer_registry::initialize_registry', () => {
  let ctx: ProgramTestContext;
  let banks: BanksClient;
  let payer: Keypair;
  let program: Program;

  beforeAll(async () => {
    ctx = await startAnchor(path.resolve(__dirname, '../..'), [], []);
    banks = ctx.banksClient;
    payer = ctx.payer;

    const idl = JSON.parse(
      fs.readFileSync(
        path.resolve(__dirname, '../../target/idl/issuer_registry.json'),
        'utf-8',
      ),
    );
    if (!idl.address) idl.address = ISSUER_REGISTRY_ID.toBase58();

    // Wrap the banks client in an Anchor provider.
    const provider = new AnchorProvider(
      {
        getAccountInfo: async (pubkey: PublicKey) =>
          banks.getAccount(pubkey) as any,
      } as any as Connection,
      new Wallet(payer),
      { commitment: 'processed' },
    );
    program = new Program(idl, provider);
  });

  it('initialises the registry with the full 112-byte RegistryConfig', async () => {
    const [registryPda] = PublicKey.findProgramAddressSync(
      [Buffer.from('registry-config')],
      ISSUER_REGISTRY_ID,
    );

    const governanceMint = PublicKey.default;
    const minStake = new BN(1_000_000_000);
    const votingPeriod = new BN(86_400);
    const approvalBps = new BN(6_000);

    const ix = await program.methods
      .initializeRegistry(governanceMint, minStake, votingPeriod, approvalBps)
      .accounts({
        registryConfig: registryPda,
        authority: payer.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .instruction();

    const tx = new (await import('@solana/web3.js')).Transaction().add(ix);
    tx.feePayer = payer.publicKey;
    tx.recentBlockhash = (await banks.getLatestBlockhash())[0];
    tx.sign(payer);
    await banks.processTransaction(tx);

    const acc = await banks.getAccount(registryPda);
    expect(acc).not.toBeNull();
    expect(acc!.data.length).toBeGreaterThanOrEqual(112);

    const deserialized = program.coder.accounts.decode('RegistryConfig', acc!.data);
    expect(deserialized.authority.toBase58()).toBe(payer.publicKey.toBase58());
    expect(deserialized.governanceTokenMint.toBase58()).toBe(governanceMint.toBase58());
    expect(deserialized.minStakeLamports.toNumber()).toBe(minStake.toNumber());
    expect(deserialized.votingPeriodSeconds.toNumber()).toBe(votingPeriod.toNumber());
    expect(deserialized.approvalThresholdBps.toNumber()).toBe(approvalBps.toNumber());
    expect(deserialized.totalIssuers.toNumber()).toBe(0);
    expect(deserialized.activeIssuers.toNumber()).toBe(0);
  });
});
