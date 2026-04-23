/**
 * SolID Protocol -- issuer-approval bootstrap (E2E step 1.5).
 *
 * Runs after scripts/initialize.ts and before scripts/issue.ts.
 *
 * On a freshly initialised registry, every new issuer starts in
 * `IssuerStatus::Pending`. `issuer_registry::issue_credential` refuses
 * to execute unless the issuer is `Approved`.  This script walks the
 * full DAO flow end-to-end so `issue.ts` can just call issueCredential
 * without worrying about state:
 *
 *   1. Create a dedicated governance SPL-Token mint (unless
 *      `SOLID_GOVERNANCE_MINT` is already set) and mint a supply to
 *      the calling wallet.
 *   2. Call `initialize_registry` with a *short* voting period
 *      (`SOLID_VOTING_PERIOD_SECONDS`; default 20s) so the end-to-end
 *      path finishes in under a minute on a local validator.  Idempotent
 *      -- reuses the existing config if one is already installed.
 *   3. `register_issuer` (Community tier; 1-lamport min stake) under
 *      a fresh BJJ keypair + fresh Solana authority keypair.
 *   4. `stake_tokens` as the wallet voter.
 *   5. Wait >= 100 slots (flash-loan protection in vote_on_issuer).
 *   6. `vote_on_issuer` approve=true with majority weight.
 *   7. Wait for `voting_ends_at` to pass.
 *   8. `finalize_voting`  --> flips Pending -> Approved.
 *   9. Persist issuer state (BJJ keypair, authority secret, PDAs) into
 *      the shared E2E state file.
 *
 * Re-run-safe: if the same BJJ/authority keypairs are already in state,
 * re-use them; only the steps that have not completed are re-executed.
 *
 * Note on trusted-setup path: `approve_via_trust_anchor` would be
 * faster, but it itself requires a pre-existing approved
 * Government-tier anchor.  A clean-registry bootstrap therefore must
 * start with the DAO-vote path at least once; after that, further
 * issuers can be approved by either path.
 */

import { Connection, Keypair, PublicKey, SystemProgram, LAMPORTS_PER_SOL } from '@solana/web3.js';
import * as anchor from '@coral-xyz/anchor';
import {
  createMint,
  mintTo,
  getOrCreateAssociatedTokenAccount,
  TOKEN_PROGRAM_ID,
} from '@solana/spl-token';
import { initWasm, generateKeypair, PROGRAM_IDS } from '@solid-protocol/core';
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
const VOTING_PERIOD_SECONDS = Number(
  process.env.SOLID_VOTING_PERIOD_SECONDS ?? '20',
);
const STAKE_AMOUNT = BigInt(
  process.env.SOLID_STAKE_AMOUNT ?? `${1_000_000_000n}`, // 1e9 test units
);
const MIN_STAKE_LAMPORTS = new anchor.BN(1); // Community tier multiplier = 1

const keypairPath = path.join(os.homedir(), '.config/solana/id.json');
const wallet = Keypair.fromSecretKey(
  Uint8Array.from(JSON.parse(fs.readFileSync(keypairPath, 'utf-8'))),
);
const connection = new Connection(RPC_URL, 'confirmed');
const provider = new anchor.AnchorProvider(
  connection, new anchor.Wallet(wallet), { commitment: 'confirmed' },
);
anchor.setProvider(provider);

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

function sleep(ms: number): Promise<void> {
  return new Promise(r => setTimeout(r, ms));
}

/// Poll `getSlot()` until `minSlot` has passed or `timeoutMs` elapses.
async function waitUntilSlot(target: number, timeoutMs: number): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const s = await connection.getSlot('confirmed');
    if (s >= target) return;
    await sleep(250);
  }
  throw new Error(`waitUntilSlot timed out (target=${target})`);
}

async function main() {
  await initWasm();
  console.log('SolID Protocol -- issuer-approval bootstrap');
  console.log('===========================================');
  console.log(`Wallet: ${wallet.publicKey.toBase58()}`);
  console.log(`RPC:    ${RPC_URL}`);

  const state = readState('bootstrap_issuer');
  const issuerIdl = loadIdl('issuerRegistry');
  const issuerProgram = new anchor.Program(issuerIdl, provider);

  // ─── 1. Governance mint ──────────────────────────────────────────
  console.log('\n[1/9] Governance mint');
  let governanceMint: PublicKey;
  if (state.governanceMint) {
    governanceMint = new PublicKey(state.governanceMint);
    console.log(`   re-used ${governanceMint.toBase58()}`);
  } else if (process.env.SOLID_GOVERNANCE_MINT) {
    governanceMint = new PublicKey(process.env.SOLID_GOVERNANCE_MINT);
    console.log(`   env-supplied ${governanceMint.toBase58()}`);
  } else {
    governanceMint = await createMint(connection, wallet, wallet.publicKey, null, 6);
    console.log(`   created ${governanceMint.toBase58()}`);
  }
  state.governanceMint = governanceMint.toBase58();

  // ─── 2. initialize_registry (idempotent) ────────────────────────
  console.log('\n[2/9] initialize_registry');
  const [registryConfigPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('registry-config')],
    PROGRAM_PUBKEYS.issuerRegistry,
  );
  try {
    await issuerProgram.methods.initializeRegistry(
      governanceMint,
      MIN_STAKE_LAMPORTS,
      new anchor.BN(VOTING_PERIOD_SECONDS),
      new anchor.BN(6000),
    ).accounts({
      registryConfig: registryConfigPda,
      authority: wallet.publicKey,
      systemProgram: SystemProgram.programId,
    }).rpc();
    console.log('   ok (initialised)');
  } catch (e: any) {
    const m = String(e?.message ?? e);
    if (!m.includes('already in use') && !m.includes('already initialized')) {
      throw e;
    }
    console.log('   ok (already active)');
  }

  // ─── 3. register_issuer ───────────────────────────────────────────
  console.log('\n[3/9] register_issuer');
  const issuerBjj = state.issuerBjj
    ? {
        private_key: Uint8Array.from(state.issuerBjj.private_key),
        public_key_x: Uint8Array.from(state.issuerBjj.public_key_x),
        public_key_y: Uint8Array.from(state.issuerBjj.public_key_y),
      }
    : generateKeypair();
  const issuerAuthority = state.issuerAuthoritySecret
    ? Keypair.fromSecretKey(Uint8Array.from(state.issuerAuthoritySecret))
    : Keypair.generate();

  // Fund issuer authority so it can pay for the register_issuer
  // `init` allocation (its PDA rent).  One transfer, idempotent via
  // balance check.
  const bal = await connection.getBalance(issuerAuthority.publicKey);
  if (bal < 0.01 * LAMPORTS_PER_SOL) {
    const fundSig = await connection.sendTransaction(
      new anchor.web3.Transaction().add(
        SystemProgram.transfer({
          fromPubkey: wallet.publicKey,
          toPubkey: issuerAuthority.publicKey,
          lamports: 0.05 * LAMPORTS_PER_SOL,
        }),
      ),
      [wallet],
    );
    await connection.confirmTransaction(fundSig, 'confirmed');
  }

  const [issuerAccountPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('issuer'), issuerAuthority.publicKey.toBuffer()],
    PROGRAM_PUBKEYS.issuerRegistry,
  );
  const [stakeVaultPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('stake-vault')],
    PROGRAM_PUBKEYS.issuerRegistry,
  );
  try {
    await issuerProgram.methods.registerIssuer(
      'e2e-test-issuer',
      'ipfs://none',
      Array.from(issuerBjj.public_key_x),
      Array.from(issuerBjj.public_key_y),
      { community: {} },
    ).accounts({
      registryConfig: registryConfigPda,
      issuerAccount: issuerAccountPda,
      stakeVault: stakeVaultPda,
      issuerAuthority: issuerAuthority.publicKey,
      systemProgram: SystemProgram.programId,
    }).signers([issuerAuthority]).rpc();
    console.log(`   ok (issuer=${issuerAccountPda.toBase58()})`);
  } catch (e: any) {
    const m = String(e?.message ?? e);
    if (!m.includes('already in use')) throw e;
    console.log('   ok (already registered)');
  }

  // ─── 4. stake_tokens ─────────────────────────────────────────────
  console.log('\n[4/9] stake_tokens (voter = wallet)');
  const [stakerPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('staker'), wallet.publicKey.toBuffer()],
    PROGRAM_PUBKEYS.issuerRegistry,
  );
  const [governanceVaultPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('governance-vault'), registryConfigPda.toBuffer()],
    PROGRAM_PUBKEYS.issuerRegistry,
  );
  const voterAta = await getOrCreateAssociatedTokenAccount(
    connection, wallet, governanceMint, wallet.publicKey,
  );
  if (voterAta.amount < STAKE_AMOUNT * 2n) {
    await mintTo(
      connection, wallet, governanceMint, voterAta.address, wallet,
      STAKE_AMOUNT * 10n,
    );
  }

  const stakeStartSlot = await connection.getSlot('confirmed');
  await issuerProgram.methods.stakeTokens(new anchor.BN(STAKE_AMOUNT.toString()))
    .accounts({
      registryConfig: registryConfigPda,
      stakerAccount: stakerPda,
      governanceVault: governanceVaultPda,
      governanceMint,
      voterTokenAccount: voterAta.address,
      voter: wallet.publicKey,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
      rent: anchor.web3.SYSVAR_RENT_PUBKEY,
    }).rpc();
  console.log(`   ok (staked ${STAKE_AMOUNT})`);

  // ─── 5. wait 100 slots (flash-loan protection) ───────────────────
  console.log('\n[5/9] waiting 100 slots for flash-loan cool-off...');
  await waitUntilSlot(stakeStartSlot + 102, 120_000);
  console.log('   ok');

  // ─── 6. vote_on_issuer (approve=true) ────────────────────────────
  console.log('\n[6/9] vote_on_issuer');
  const [voteRecordPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('vote'), issuerAccountPda.toBuffer(), wallet.publicKey.toBuffer()],
    PROGRAM_PUBKEYS.issuerRegistry,
  );
  try {
    await issuerProgram.methods.voteOnIssuer(true).accounts({
      registryConfig: registryConfigPda,
      issuerAccount: issuerAccountPda,
      voteRecord: voteRecordPda,
      stakerAccount: stakerPda,
      voter: wallet.publicKey,
      systemProgram: SystemProgram.programId,
    }).rpc();
    console.log('   ok (voted APPROVE)');
  } catch (e: any) {
    const m = String(e?.message ?? e);
    if (!m.includes('already in use')) throw e;
    console.log('   ok (already voted)');
  }

  // ─── 7. wait past voting_ends_at ─────────────────────────────────
  console.log('\n[7/9] waiting for voting period to end...');
  // Voting window started at issuer.voting_ends_at - voting_period.
  // Simpler: sleep `VOTING_PERIOD_SECONDS + 2` from now, which is
  // always sufficient on a fresh run.  On a re-run the finalize call
  // below will be idempotent.
  await sleep((VOTING_PERIOD_SECONDS + 2) * 1000);
  console.log('   ok');

  // ─── 8. finalize_voting ──────────────────────────────────────────
  console.log('\n[8/9] finalize_voting');
  try {
    await issuerProgram.methods.finalizeVoting().accounts({
      registryConfig: registryConfigPda,
      issuerAccount: issuerAccountPda,
      payer: wallet.publicKey,
      systemProgram: SystemProgram.programId,
    }).rpc();
    console.log('   ok');
  } catch (e: any) {
    const m = String(e?.message ?? e);
    // "Issuer is not in Pending status" is acceptable on re-run (already finalised).
    if (!m.includes('IssuerNotPending')) throw e;
    console.log('   ok (already finalised)');
  }

  // Sanity-check the final status.
  const issuerAccount: any = await (issuerProgram.account as any)
    .issuerAccount.fetch(issuerAccountPda);
  const isApproved = issuerAccount.status && typeof issuerAccount.status === 'object'
    ? Object.keys(issuerAccount.status)[0].toLowerCase() === 'approved'
    : String(issuerAccount.status).toLowerCase().includes('approved');
  if (!isApproved) {
    throw new Error(
      `issuer did not reach Approved status; final=${JSON.stringify(issuerAccount.status)}`,
    );
  }

  // ─── 9. persist state ─────────────────────────────────────────────
  console.log('\n[9/9] persisting state');
  state.governanceMint = governanceMint.toBase58();
  state.registryConfigPda = registryConfigPda.toBase58();
  state.issuerAccountPda = issuerAccountPda.toBase58();
  state.stakeVaultPda = stakeVaultPda.toBase58();
  state.issuerBjj = {
    private_key: Array.from(issuerBjj.private_key),
    public_key_x: Array.from(issuerBjj.public_key_x),
    public_key_y: Array.from(issuerBjj.public_key_y),
  };
  state.issuerAuthoritySecret = Array.from(issuerAuthority.secretKey);
  state.issuerAuthorityPubkey = issuerAuthority.publicKey.toBase58();
  writeState(state);

  console.log('\nDone. Summary:');
  console.log(`  issuer_account     : ${issuerAccountPda.toBase58()}`);
  console.log(`  issuer_authority   : ${issuerAuthority.publicKey.toBase58()}`);
  console.log(`  stake_vault        : ${stakeVaultPda.toBase58()}`);
  console.log(`  governance_mint    : ${governanceMint.toBase58()}`);
}

main().catch(e => {
  console.error(e);
  process.exit(1);
});
