/**
 * Integration test 01: RegistryConfig initialization.
 *
 * Exercises issuer_registry::initialize_registry against bankrun.
 *
 * Hard gates this test enforces:
 *
 * 1.  Layout: the 80-to-112-byte RegistryConfig fix (pre-remediation
 *     allocated 32 bytes too few for governance_token_mint).  Any future
 *     change to `space = ...` in programs/issuer-registry/src/lib.rs
 *     without updating RegistryConfig fields fails this test.
 *
 * 2.  Contract (post 2026-04-26 redesign — see B10 in
 *     docs/E2E_BLOCKERS.md):
 *       - `governance_mint` must be passed as an `Account<Mint>`.
 *         Anchor's deserializer rejects anything that isn't owned by
 *         the SPL Token program, so `Pubkey::default()` can never end
 *         up in the registry.
 *       - The `governance_vault` TokenAccount is born atomically with
 *         the registry under PDA `["governance-vault", registry_config]`.
 *       - `voting_period > 0` and `approval_threshold <= 10_000`
 *         (basis points) are enforced inside the handler.
 */

import { startAnchor, BanksClient, ProgramTestContext } from 'solana-bankrun';
import { AnchorProvider, Program, BN, Wallet } from '@coral-xyz/anchor';
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  SYSVAR_RENT_PUBKEY,
} from '@solana/web3.js';
import { TOKEN_PROGRAM_ID } from '@solana/spl-token';
import * as fs from 'fs';
import * as path from 'path';

const ISSUER_REGISTRY_ID = new PublicKey(
  '5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx',
);

/**
 * Build a raw 82-byte SPL Mint account body (initialised, no freeze auth).
 *
 *   layout (spl-token v3 / v4):
 *     0..4    COption mint_authority discriminant (LE u32: 1 = Some)
 *     4..36   mint_authority pubkey
 *     36..44  supply (LE u64)
 *     44..45  decimals (u8)
 *     45..46  is_initialized (u8: 1)
 *     46..50  COption freeze_authority discriminant (LE u32: 0 = None)
 *     50..82  freeze_authority pubkey (zeroed when None)
 */
function encodeSplMint(mintAuthority: PublicKey, decimals: number): Buffer {
  const buf = Buffer.alloc(82);
  buf.writeUInt32LE(1, 0); // Some
  mintAuthority.toBuffer().copy(buf, 4);
  // supply: 0
  buf.writeUInt8(decimals, 44);
  buf.writeUInt8(1, 45); // is_initialized
  buf.writeUInt32LE(0, 46); // None
  return buf;
}

function seedMint(
  ctx: ProgramTestContext,
  mint: PublicKey,
  authority: PublicKey,
  decimals = 6,
): void {
  ctx.setAccount(mint, {
    lamports: 10_000_000_000, // 10 SOL: way above rent-exempt for 82 bytes
    data: encodeSplMint(authority, decimals),
    owner: TOKEN_PROGRAM_ID,
    executable: false,
  });
}

describe('issuer_registry::initialize_registry', () => {
  let ctx: ProgramTestContext;
  let banks: BanksClient;
  let payer: Keypair;
  let program: Program;
  let registryPda: PublicKey;
  let governanceVaultPda: PublicKey;

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

    const provider = new AnchorProvider(
      {
        getAccountInfo: async (pubkey: PublicKey) =>
          banks.getAccount(pubkey) as any,
      } as any as Connection,
      new Wallet(payer),
      { commitment: 'processed' },
    );
    program = new Program(idl, provider);

    [registryPda] = PublicKey.findProgramAddressSync(
      [Buffer.from('registry-config')],
      ISSUER_REGISTRY_ID,
    );
    [governanceVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from('governance-vault'), registryPda.toBuffer()],
      ISSUER_REGISTRY_ID,
    );
  });

  it('initialises the registry with the full 112-byte RegistryConfig and atomic vault', async () => {
    const mint = Keypair.generate().publicKey;
    seedMint(ctx, mint, payer.publicKey);

    const minStake = new BN(1_000_000_000);
    const votingPeriod = new BN(86_400);
    const approvalBps = new BN(6_000);

    const ix = await program.methods
      .initializeRegistry(minStake, votingPeriod, approvalBps)
      .accounts({
        registryConfig: registryPda,
        governanceMint: mint,
        governanceVault: governanceVaultPda,
        authority: payer.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .instruction();

    const tx = new Transaction().add(ix);
    tx.feePayer = payer.publicKey;
    tx.recentBlockhash = (await banks.getLatestBlockhash())[0];
    tx.sign(payer);
    await banks.processTransaction(tx);

    // Registry layout invariant.
    const regAcc = await banks.getAccount(registryPda);
    expect(regAcc).not.toBeNull();
    expect(regAcc!.data.length).toBeGreaterThanOrEqual(112);

    const cfg = program.coder.accounts.decode('RegistryConfig', Buffer.from(regAcc!.data));
    expect(cfg.authority.toBase58()).toBe(payer.publicKey.toBase58());
    expect(cfg.governanceTokenMint.toBase58()).toBe(mint.toBase58());
    expect(cfg.governanceTokenMint.equals(PublicKey.default)).toBe(false);
    expect(cfg.minStakeLamports.toNumber()).toBe(minStake.toNumber());
    expect(cfg.votingPeriodSeconds.toNumber()).toBe(votingPeriod.toNumber());
    expect(cfg.approvalThresholdBps.toNumber()).toBe(approvalBps.toNumber());
    expect(cfg.totalIssuers.toNumber()).toBe(0);
    expect(cfg.activeIssuers.toNumber()).toBe(0);

    // Vault must exist, owned by SPL Token program, bound to the mint.
    const vaultAcc = await banks.getAccount(governanceVaultPda);
    expect(vaultAcc).not.toBeNull();
    expect(new PublicKey(vaultAcc!.owner).equals(TOKEN_PROGRAM_ID)).toBe(true);
    // Mint pubkey is at offset 0 of the SPL TokenAccount layout.
    const vaultMint = new PublicKey(Buffer.from(vaultAcc!.data).slice(0, 32));
    expect(vaultMint.toBase58()).toBe(mint.toBase58());
  });

  it('rejects approval_threshold > 10000 bps', async () => {
    // Reset state for an independent check by using a separate test context.
    const ctx2 = await startAnchor(path.resolve(__dirname, '../..'), [], []);
    const banks2 = ctx2.banksClient;
    const payer2 = ctx2.payer;
    const provider2 = new AnchorProvider(
      {
        getAccountInfo: async (pubkey: PublicKey) =>
          banks2.getAccount(pubkey) as any,
      } as any as Connection,
      new Wallet(payer2),
      { commitment: 'processed' },
    );
    const program2 = new Program(
      JSON.parse(
        fs.readFileSync(
          path.resolve(__dirname, '../../target/idl/issuer_registry.json'),
          'utf-8',
        ),
      ),
      provider2,
    );

    const mint = Keypair.generate().publicKey;
    seedMint(ctx2, mint, payer2.publicKey);

    const ix = await program2.methods
      .initializeRegistry(new BN(0), new BN(86_400), new BN(10_001))
      .accounts({
        registryConfig: registryPda,
        governanceMint: mint,
        governanceVault: governanceVaultPda,
        authority: payer2.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .instruction();
    const tx = new Transaction().add(ix);
    tx.feePayer = payer2.publicKey;
    tx.recentBlockhash = (await banks2.getLatestBlockhash())[0];
    tx.sign(payer2);
    await expect(banks2.processTransaction(tx)).rejects.toThrow(/InvalidThreshold|0x.*/);
  });

  it('rejects voting_period == 0', async () => {
    const ctx3 = await startAnchor(path.resolve(__dirname, '../..'), [], []);
    const banks3 = ctx3.banksClient;
    const payer3 = ctx3.payer;
    const provider3 = new AnchorProvider(
      {
        getAccountInfo: async (pubkey: PublicKey) =>
          banks3.getAccount(pubkey) as any,
      } as any as Connection,
      new Wallet(payer3),
      { commitment: 'processed' },
    );
    const program3 = new Program(
      JSON.parse(
        fs.readFileSync(
          path.resolve(__dirname, '../../target/idl/issuer_registry.json'),
          'utf-8',
        ),
      ),
      provider3,
    );

    const mint = Keypair.generate().publicKey;
    seedMint(ctx3, mint, payer3.publicKey);

    const ix = await program3.methods
      .initializeRegistry(new BN(0), new BN(0), new BN(6_000))
      .accounts({
        registryConfig: registryPda,
        governanceMint: mint,
        governanceVault: governanceVaultPda,
        authority: payer3.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .instruction();
    const tx = new Transaction().add(ix);
    tx.feePayer = payer3.publicKey;
    tx.recentBlockhash = (await banks3.getLatestBlockhash())[0];
    tx.sign(payer3);
    await expect(banks3.processTransaction(tx)).rejects.toThrow(/InvalidVotingPeriod|0x.*/);
  });

  it('rejects a non-Mint account passed as governance_mint', async () => {
    const ctx4 = await startAnchor(path.resolve(__dirname, '../..'), [], []);
    const banks4 = ctx4.banksClient;
    const payer4 = ctx4.payer;
    const provider4 = new AnchorProvider(
      {
        getAccountInfo: async (pubkey: PublicKey) =>
          banks4.getAccount(pubkey) as any,
      } as any as Connection,
      new Wallet(payer4),
      { commitment: 'processed' },
    );
    const program4 = new Program(
      JSON.parse(
        fs.readFileSync(
          path.resolve(__dirname, '../../target/idl/issuer_registry.json'),
          'utf-8',
        ),
      ),
      provider4,
    );

    // Pubkey::default() is now structurally rejected by Anchor's
    // `Account<Mint>` deserializer (account is owned by System Program,
    // not SPL Token).  This is the regression gate for the original bug.
    const fakeMint = PublicKey.default;

    const ix = await program4.methods
      .initializeRegistry(new BN(0), new BN(86_400), new BN(6_000))
      .accounts({
        registryConfig: registryPda,
        governanceMint: fakeMint,
        governanceVault: governanceVaultPda,
        authority: payer4.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .instruction();
    const tx = new Transaction().add(ix);
    tx.feePayer = payer4.publicKey;
    tx.recentBlockhash = (await banks4.getLatestBlockhash())[0];
    tx.sign(payer4);
    await expect(banks4.processTransaction(tx)).rejects.toThrow();
  });
});
