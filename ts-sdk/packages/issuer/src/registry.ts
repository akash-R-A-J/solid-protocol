import { Buffer } from 'buffer';
import { PROGRAM_IDS as CORE_PROGRAM_IDS } from '@solid-protocol/core';
import {
  ComputeBudgetProgram,
  PublicKey,
  SYSVAR_RENT_PUBKEY,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from '@solana/web3.js';

export const ISSUER_REGISTRY_PROGRAM_ID = new PublicKey(CORE_PROGRAM_IDS.issuerRegistry);
export const SCHEMA_REGISTRY_PROGRAM_ID = new PublicKey(CORE_PROGRAM_IDS.schemaRegistry);
export const TOKEN_PROGRAM_ID = new PublicKey('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
export const ASSOCIATED_TOKEN_PROGRAM_ID = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
export const SPL_ACCOUNT_COMPRESSION_PROGRAM_ID = new PublicKey('cmtDvXumGCrqC1Age74AVPhSRVXJMd8PJS91L8KbNCK');
export const SPL_NOOP_PROGRAM_ID = new PublicKey('noopb9bkMVfRPU8AsbpTUg8AQkHtKwMYZiFUjNRtMmV');

export const REGISTER_ISSUER_DISCRIMINATOR = Uint8Array.from([145, 117, 52, 59, 189, 27, 127, 18]);
export const STAKE_TOKENS_DISCRIMINATOR = Uint8Array.from([136, 126, 91, 162, 40, 131, 13, 127]);
export const VOTE_ON_ISSUER_DISCRIMINATOR = Uint8Array.from([153, 66, 52, 109, 141, 13, 129, 61]);
export const FINALIZE_VOTING_DISCRIMINATOR = Uint8Array.from([195, 61, 27, 72, 252, 138, 175, 13]);
export const GRANT_SCHEMA_PERMISSION_DISCRIMINATOR = Uint8Array.from([225, 219, 158, 116, 212, 110, 206, 108]);
export const REVOKE_SCHEMA_PERMISSION_DISCRIMINATOR = Uint8Array.from([127, 210, 35, 147, 68, 83, 2, 217]);

export const ISSUER_ACCOUNT_DISCRIMINATOR = Uint8Array.from([126, 234, 14, 239, 71, 204, 88, 61]);
export const ISSUER_SCHEMA_PERMISSION_DISCRIMINATOR = Uint8Array.from([132, 162, 241, 163, 114, 123, 177, 183]);
export const REGISTRY_CONFIG_DISCRIMINATOR = Uint8Array.from([23, 118, 10, 246, 173, 231, 243, 156]);
export const STAKER_ACCOUNT_DISCRIMINATOR = Uint8Array.from([12, 152, 43, 218, 164, 11, 150, 174]);
export const SUBGROUP_VERIFIER_CONFIG_DISCRIMINATOR = Uint8Array.from([136, 192, 171, 162, 185, 137, 180, 109]);
export const SCHEMA_ACCOUNT_DISCRIMINATOR = Uint8Array.from([216, 80, 253, 155, 77, 255, 31, 57]);

export const ISSUER_ACCOUNT_DISCRIMINATOR_BASE58 = base58Encode(ISSUER_ACCOUNT_DISCRIMINATOR);
export const ISSUER_SCHEMA_PERMISSION_DISCRIMINATOR_BASE58 = base58Encode(ISSUER_SCHEMA_PERMISSION_DISCRIMINATOR);
export const SCHEMA_ACCOUNT_DISCRIMINATOR_BASE58 = base58Encode(SCHEMA_ACCOUNT_DISCRIMINATOR);

export const ISSUER_TREE_BINDING_DISCRIMINATOR = new TextEncoder().encode('issrtree');
export const ISSUER_TREE_BINDING_SIZE = 113;

export interface IssuerRegistryProgramIdOverrides {
  issuerRegistry?: PublicKey;
  schemaRegistry?: PublicKey;
  tokenProgram?: PublicKey;
  associatedTokenProgram?: PublicKey;
}

interface ResolvedIssuerRegistryProgramIds {
  issuerRegistry: PublicKey;
  schemaRegistry: PublicKey;
  tokenProgram: PublicKey;
  associatedTokenProgram: PublicKey;
}

const pdaCache = new Map<string, PublicKey>();

function resolveProgramIds(overrides?: IssuerRegistryProgramIdOverrides): ResolvedIssuerRegistryProgramIds {
  return {
    issuerRegistry: overrides?.issuerRegistry ?? ISSUER_REGISTRY_PROGRAM_ID,
    schemaRegistry: overrides?.schemaRegistry ?? SCHEMA_REGISTRY_PROGRAM_ID,
    tokenProgram: overrides?.tokenProgram ?? TOKEN_PROGRAM_ID,
    associatedTokenProgram: overrides?.associatedTokenProgram ?? ASSOCIATED_TOKEN_PROGRAM_ID,
  };
}

function memoPda(key: string, derive: () => PublicKey): PublicKey {
  const cached = pdaCache.get(key);
  if (cached) return cached;
  const pda = derive();
  pdaCache.set(key, pda);
  return pda;
}

export type IssuerTierName = 'community' | 'enterprise' | 'regulated' | 'government';

export const ISSUER_TIER_OPTIONS: Array<{
  value: IssuerTierName;
  label: string;
  multiplier: bigint;
  description: string;
}> = [
  {
    value: 'community',
    label: 'Community',
    multiplier: 1n,
    description: 'Open registration path for early ecosystem issuers.',
  },
  {
    value: 'enterprise',
    label: 'Enterprise',
    multiplier: 10n,
    description: 'Higher stake path for commercial issuers.',
  },
  {
    value: 'regulated',
    label: 'Regulated',
    multiplier: 5n,
    description: 'Compliance-heavy issuers with regulated workflows.',
  },
  {
    value: 'government',
    label: 'Government',
    multiplier: 0n,
    description: 'DAO-trusted public-sector issuers; no SOL stake transfer.',
  },
];

export interface RegistryConfigSummary {
  authority: string;
  governanceTokenMint: string;
  minStakeLamports: bigint;
  votingPeriodSeconds: bigint;
  approvalThresholdBps: bigint;
  totalIssuers: bigint;
  activeIssuers: bigint;
  nextIssuerLeafIndex: bigint;
}

export interface IssuerAccountSummary {
  authority: string;
  pda: string;
  name: string;
  metadataUri: string;
  bjjPubKeyX: string;
  bjjPubKeyY: string;
  tier: 'community' | 'enterprise' | 'regulated' | 'government' | 'unknown';
  status: 'pending' | 'approved' | 'cooldown' | 'rejected' | 'revoked' | 'unknown';
  stakedAmount: bigint;
  registeredAt: bigint;
  creationSlot: bigint;
  cooldownEndsAt: bigint;
  votesFor: bigint;
  votesAgainst: bigint;
  votingEndsAt: bigint;
  credentialsIssued: bigint;
  slashCount: bigint;
  revocationNonce: bigint;
  statusEpoch: bigint;
  issuerTreeLeafIndex: bigint;
  isTreeEnrolled: boolean;
  dataSize: number;
}

export interface IssuerSchemaPermissionSummary {
  pda: string;
  issuer: string;
  issuerAuthority: string;
  schemaHash: string;
  schemaAccount: string;
  grantedBy: string;
  grantedAt: bigint;
  revokedAt: bigint;
  active: boolean;
  bump: number;
  dataSize: number;
}

export interface SchemaAccountSummary {
  pda: string;
  authority: string;
  name: string;
  version: number;
  category: string;
  fieldNames: string[];
  schemaHash: string;
  deprecated: boolean;
  createdAt: bigint;
  usageCount: bigint;
  dataSize: number;
}

export interface StakerAccountSummary {
  pda: string;
  voter: string;
  amountStaked: bigint;
  activeVotesCount: number;
  lastStakeSlot: bigint;
}

export interface SubgroupVerifierConfigSummary {
  authority: string;
  bump: number;
  paused: boolean;
  vkInitialized: boolean;
  nextVkChunk: number;
  vkFinalized: boolean;
  vkGeneration: number;
  rotateRequestTs: bigint;
}

export interface IssuerTreeBindingSummary {
  currentRoot: string;
  lastUpdatedSlot: bigint;
  treeAddress: string;
  status: 'active' | 'frozen' | 'unknown';
}

export function deriveIssuerAccountPda(
  authority: PublicKey,
  programIds?: IssuerRegistryProgramIdOverrides,
): PublicKey {
  const ids = resolveProgramIds(programIds);
  return memoPda(`issuer:${ids.issuerRegistry.toBase58()}:${authority.toBase58()}`, () =>
    PublicKey.findProgramAddressSync(
      [Buffer.from('issuer'), authority.toBuffer()],
      ids.issuerRegistry,
    )[0],
  );
}

export function deriveSchemaAccountPda(
  name: string,
  version: number,
  programIds?: IssuerRegistryProgramIdOverrides,
): PublicKey {
  if (!Number.isInteger(version) || version < 0 || version > 0xff) {
    throw new Error(`schema version must be a u8 in [0, 255], got ${version}`);
  }
  const ids = resolveProgramIds(programIds);
  return memoPda(`schema:${ids.schemaRegistry.toBase58()}:${name}:${version}`, () =>
    PublicKey.findProgramAddressSync(
      [Buffer.from('schema'), Buffer.from(name, 'utf8'), Uint8Array.from([version])],
      ids.schemaRegistry,
    )[0],
  );
}

export function deriveIssuerSchemaPermissionPda(
  issuerAccount: PublicKey,
  schemaHash: Uint8Array,
  programIds?: IssuerRegistryProgramIdOverrides,
): PublicKey {
  if (schemaHash.length !== 32) {
    throw new Error(`schemaHash must be 32 bytes, got ${schemaHash.length}`);
  }
  const ids = resolveProgramIds(programIds);
  const schemaHashHex = Buffer.from(schemaHash).toString('hex');
  return memoPda(`issuer-schema:${ids.issuerRegistry.toBase58()}:${issuerAccount.toBase58()}:${schemaHashHex}`, () =>
    PublicKey.findProgramAddressSync(
      [Buffer.from('issuer-schema'), issuerAccount.toBuffer(), Buffer.from(schemaHash)],
      ids.issuerRegistry,
    )[0],
  );
}

export function deriveStakerAccountPda(
  voter: PublicKey,
  programIds?: IssuerRegistryProgramIdOverrides,
): PublicKey {
  const ids = resolveProgramIds(programIds);
  return memoPda(`staker:${ids.issuerRegistry.toBase58()}:${voter.toBase58()}`, () =>
    PublicKey.findProgramAddressSync(
      [Buffer.from('staker'), voter.toBuffer()],
      ids.issuerRegistry,
    )[0],
  );
}

export function deriveRegistryConfigPda(programIds?: IssuerRegistryProgramIdOverrides): PublicKey {
  const ids = resolveProgramIds(programIds);
  return memoPda(`registry-config:${ids.issuerRegistry.toBase58()}`, () =>
    PublicKey.findProgramAddressSync(
      [Buffer.from('registry-config')],
      ids.issuerRegistry,
    )[0],
  );
}

export function deriveStakeVaultPda(programIds?: IssuerRegistryProgramIdOverrides): PublicKey {
  const ids = resolveProgramIds(programIds);
  return memoPda(`stake-vault:${ids.issuerRegistry.toBase58()}`, () =>
    PublicKey.findProgramAddressSync(
      [Buffer.from('stake-vault')],
      ids.issuerRegistry,
    )[0],
  );
}

export function deriveSubgroupVerifierConfigPda(programIds?: IssuerRegistryProgramIdOverrides): PublicKey {
  const ids = resolveProgramIds(programIds);
  return memoPda(`subgroup-verifier-config:${ids.issuerRegistry.toBase58()}`, () =>
    PublicKey.findProgramAddressSync(
      [Buffer.from('subgroup-verifier-config')],
      ids.issuerRegistry,
    )[0],
  );
}

export function deriveSubgroupVkStoragePda(
  configPda?: PublicKey,
  programIds?: IssuerRegistryProgramIdOverrides,
): PublicKey {
  const ids = resolveProgramIds(programIds);
  const config = configPda ?? deriveSubgroupVerifierConfigPda(programIds);
  return memoPda(`subgroup-vk-storage:${ids.issuerRegistry.toBase58()}:${config.toBase58()}`, () =>
    PublicKey.findProgramAddressSync(
      [Buffer.from('subgroup-vk-storage'), config.toBuffer()],
      ids.issuerRegistry,
    )[0],
  );
}

export function deriveIssuerTreeBindingPda(programIds?: IssuerRegistryProgramIdOverrides): PublicKey {
  const ids = resolveProgramIds(programIds);
  return memoPda(`issuer-tree-binding:${ids.issuerRegistry.toBase58()}`, () =>
    PublicKey.findProgramAddressSync(
      [Buffer.from('issuer-tree-binding')],
      ids.issuerRegistry,
    )[0],
  );
}

export function deriveIssuerTreeAuthorityPda(programIds?: IssuerRegistryProgramIdOverrides): PublicKey {
  const ids = resolveProgramIds(programIds);
  return memoPda(`issuer-tree-authority:${ids.issuerRegistry.toBase58()}`, () =>
    PublicKey.findProgramAddressSync(
      [Buffer.from('issuer-tree-authority')],
      ids.issuerRegistry,
    )[0],
  );
}

export function deriveTreeAuthorityPda(
  schemaHash: Uint8Array,
  programIds?: IssuerRegistryProgramIdOverrides,
): PublicKey {
  if (schemaHash.length !== 32) {
    throw new Error(`schemaHash must be 32 bytes, got ${schemaHash.length}`);
  }
  const ids = resolveProgramIds(programIds);
  const schemaHashHex = Buffer.from(schemaHash).toString('hex');
  return memoPda(`tree-authority:${ids.issuerRegistry.toBase58()}:${schemaHashHex}`, () =>
    PublicKey.findProgramAddressSync(
      [Buffer.from('tree-authority'), Buffer.from(schemaHash)],
      ids.issuerRegistry,
    )[0],
  );
}

export function deriveSchemaTreeBindingPda(
  schemaHash: Uint8Array,
  programIds?: IssuerRegistryProgramIdOverrides,
): PublicKey {
  if (schemaHash.length !== 32) {
    throw new Error(`schemaHash must be 32 bytes, got ${schemaHash.length}`);
  }
  const ids = resolveProgramIds(programIds);
  const schemaHashHex = Buffer.from(schemaHash).toString('hex');
  return memoPda(`schema-tree-binding:${ids.schemaRegistry.toBase58()}:${schemaHashHex}`, () =>
    PublicKey.findProgramAddressSync(
      [Buffer.from('schema-tree-binding'), Buffer.from(schemaHash)],
      ids.schemaRegistry,
    )[0],
  );
}

export function deriveGlobalBindingPda(programIds?: IssuerRegistryProgramIdOverrides): PublicKey {
  const ids = resolveProgramIds(programIds);
  return memoPda(`global-binding:${ids.schemaRegistry.toBase58()}`, () =>
    PublicKey.findProgramAddressSync(
      [Buffer.from('global-binding')],
      ids.schemaRegistry,
    )[0],
  );
}

export function deriveGovernanceVaultPda(programIds?: IssuerRegistryProgramIdOverrides): PublicKey {
  const ids = resolveProgramIds(programIds);
  const registryConfig = deriveRegistryConfigPda(programIds);
  return memoPda(`governance-vault:${ids.issuerRegistry.toBase58()}:${registryConfig.toBase58()}`, () =>
    PublicKey.findProgramAddressSync(
      [Buffer.from('governance-vault'), registryConfig.toBuffer()],
      ids.issuerRegistry,
    )[0],
  );
}

export function deriveVoteRecordPda(
  issuerAccount: PublicKey,
  voter: PublicKey,
  programIds?: IssuerRegistryProgramIdOverrides,
): PublicKey {
  const ids = resolveProgramIds(programIds);
  return memoPda(`vote:${ids.issuerRegistry.toBase58()}:${issuerAccount.toBase58()}:${voter.toBase58()}`, () =>
    PublicKey.findProgramAddressSync(
      [Buffer.from('vote'), issuerAccount.toBuffer(), voter.toBuffer()],
      ids.issuerRegistry,
    )[0],
  );
}

export function deriveAssociatedTokenAddress(
  owner: PublicKey,
  mint: PublicKey,
  programIds?: IssuerRegistryProgramIdOverrides,
): PublicKey {
  const ids = resolveProgramIds(programIds);
  return memoPda(`ata:${ids.associatedTokenProgram.toBase58()}:${ids.tokenProgram.toBase58()}:${owner.toBase58()}:${mint.toBase58()}`, () =>
    PublicKey.findProgramAddressSync(
      [owner.toBuffer(), ids.tokenProgram.toBuffer(), mint.toBuffer()],
      ids.associatedTokenProgram,
    )[0],
  );
}

export function estimateIssuerStakeLamports(
  registryConfig: RegistryConfigSummary,
  tier: IssuerTierName,
): bigint {
  const option = ISSUER_TIER_OPTIONS.find((item) => item.value === tier);
  if (!option) throw new Error(`Unsupported issuer tier: ${tier}`);
  return registryConfig.minStakeLamports * option.multiplier;
}

export function buildRegisterIssuerTransaction(params: {
  authority: PublicKey;
  name: string;
  metadataUri: string;
  bjjPubKeyX: Uint8Array;
  bjjPubKeyY: Uint8Array;
  tier: IssuerTierName;
  subgroupProof: Uint8Array;
  programIds?: IssuerRegistryProgramIdOverrides;
}): Transaction {
  const ids = resolveProgramIds(params.programIds);
  const name = params.name.trim();
  const metadataUri = params.metadataUri.trim();
  if (!name) throw new Error('Issuer name is required.');
  if (new TextEncoder().encode(name).length > 64) throw new Error('Issuer name must be 64 bytes or less.');
  if (new TextEncoder().encode(metadataUri).length > 128) throw new Error('Metadata URI must be 128 bytes or less.');
  if (params.bjjPubKeyX.length !== 32 || params.bjjPubKeyY.length !== 32) {
    throw new Error('Issuer BJJ public key coordinates must be 32 bytes each.');
  }
  if (params.subgroupProof.length === 0) throw new Error('Subgroup proof is required.');

  const data = concatBytes(
    REGISTER_ISSUER_DISCRIMINATOR,
    borshString(name),
    borshString(metadataUri),
    params.bjjPubKeyX,
    params.bjjPubKeyY,
    Uint8Array.of(tierToAnchorEnum(params.tier)),
    borshBytes(params.subgroupProof),
  );

  return new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 650_000 }),
    new TransactionInstruction({
      programId: ids.issuerRegistry,
      keys: [
        { pubkey: deriveRegistryConfigPda(params.programIds), isSigner: false, isWritable: true },
        { pubkey: deriveIssuerAccountPda(params.authority, params.programIds), isSigner: false, isWritable: true },
        { pubkey: deriveStakeVaultPda(params.programIds), isSigner: false, isWritable: true },
        { pubkey: deriveSubgroupVerifierConfigPda(params.programIds), isSigner: false, isWritable: false },
        { pubkey: deriveSubgroupVkStoragePda(undefined, params.programIds), isSigner: false, isWritable: false },
        { pubkey: params.authority, isSigner: true, isWritable: true },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      data: Buffer.from(data),
    }),
  );
}

export function buildStakeTokensTransaction(params: {
  voter: PublicKey;
  governanceMint: PublicKey;
  amount: bigint;
  programIds?: IssuerRegistryProgramIdOverrides;
}): Transaction {
  const ids = resolveProgramIds(params.programIds);
  if (params.amount <= 0n) throw new Error('Stake amount must be positive.');
  const voterTokenAccount = deriveAssociatedTokenAddress(params.voter, params.governanceMint, params.programIds);
  return new Transaction().add(
    buildCreateAssociatedTokenAccountIdempotentIx({
      payer: params.voter,
      owner: params.voter,
      mint: params.governanceMint,
      ata: voterTokenAccount,
      programIds: params.programIds,
    }),
    new TransactionInstruction({
      programId: ids.issuerRegistry,
      keys: [
        { pubkey: deriveRegistryConfigPda(params.programIds), isSigner: false, isWritable: false },
        { pubkey: deriveStakerAccountPda(params.voter, params.programIds), isSigner: false, isWritable: true },
        { pubkey: deriveGovernanceVaultPda(params.programIds), isSigner: false, isWritable: true },
        { pubkey: params.governanceMint, isSigner: false, isWritable: false },
        { pubkey: voterTokenAccount, isSigner: false, isWritable: true },
        { pubkey: params.voter, isSigner: true, isWritable: true },
        { pubkey: ids.tokenProgram, isSigner: false, isWritable: false },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
      ],
      data: Buffer.from(concatBytes(STAKE_TOKENS_DISCRIMINATOR, u64Le(params.amount))),
    }),
  );
}

export function buildVoteOnIssuerTransaction(params: {
  voter: PublicKey;
  issuerAccount: PublicKey;
  approve: boolean;
  programIds?: IssuerRegistryProgramIdOverrides;
}): Transaction {
  const ids = resolveProgramIds(params.programIds);
  return new Transaction().add(new TransactionInstruction({
    programId: ids.issuerRegistry,
    keys: [
      { pubkey: deriveRegistryConfigPda(params.programIds), isSigner: false, isWritable: false },
      { pubkey: params.issuerAccount, isSigner: false, isWritable: true },
      { pubkey: deriveVoteRecordPda(params.issuerAccount, params.voter, params.programIds), isSigner: false, isWritable: true },
      { pubkey: deriveStakerAccountPda(params.voter, params.programIds), isSigner: false, isWritable: true },
      { pubkey: params.voter, isSigner: true, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: Buffer.from(concatBytes(VOTE_ON_ISSUER_DISCRIMINATOR, Uint8Array.of(params.approve ? 1 : 0))),
  }));
}

export function buildFinalizeVotingTransaction(params: {
  payer: PublicKey;
  issuerAccount: PublicKey;
  programIds?: IssuerRegistryProgramIdOverrides;
}): Transaction {
  const ids = resolveProgramIds(params.programIds);
  return new Transaction().add(new TransactionInstruction({
    programId: ids.issuerRegistry,
    keys: [
      { pubkey: deriveRegistryConfigPda(params.programIds), isSigner: false, isWritable: true },
      { pubkey: params.issuerAccount, isSigner: false, isWritable: true },
      { pubkey: params.payer, isSigner: true, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: Buffer.from(FINALIZE_VOTING_DISCRIMINATOR),
  }));
}

export function buildGrantSchemaPermissionTransaction(params: {
  registryAuthority: PublicKey;
  issuerAccount: PublicKey;
  schemaName: string;
  schemaVersion: number;
  schemaHash: Uint8Array;
  programIds?: IssuerRegistryProgramIdOverrides;
}): Transaction {
  const ids = resolveProgramIds(params.programIds);
  if (params.schemaHash.length !== 32) {
    throw new Error(`schemaHash must be 32 bytes, got ${params.schemaHash.length}`);
  }
  const schemaAccount = deriveSchemaAccountPda(params.schemaName, params.schemaVersion, params.programIds);
  const permission = deriveIssuerSchemaPermissionPda(params.issuerAccount, params.schemaHash, params.programIds);
  return new Transaction().add(new TransactionInstruction({
    programId: ids.issuerRegistry,
    keys: [
      { pubkey: deriveRegistryConfigPda(params.programIds), isSigner: false, isWritable: false },
      { pubkey: params.issuerAccount, isSigner: false, isWritable: false },
      { pubkey: schemaAccount, isSigner: false, isWritable: false },
      { pubkey: permission, isSigner: false, isWritable: true },
      { pubkey: params.registryAuthority, isSigner: true, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: Buffer.from(concatBytes(GRANT_SCHEMA_PERMISSION_DISCRIMINATOR, params.schemaHash)),
  }));
}

export function buildRevokeSchemaPermissionTransaction(params: {
  registryAuthority: PublicKey;
  issuerAccount: PublicKey;
  schemaHash: Uint8Array;
  programIds?: IssuerRegistryProgramIdOverrides;
}): Transaction {
  const ids = resolveProgramIds(params.programIds);
  if (params.schemaHash.length !== 32) {
    throw new Error(`schemaHash must be 32 bytes, got ${params.schemaHash.length}`);
  }
  const permission = deriveIssuerSchemaPermissionPda(params.issuerAccount, params.schemaHash, params.programIds);
  return new Transaction().add(new TransactionInstruction({
    programId: ids.issuerRegistry,
    keys: [
      { pubkey: deriveRegistryConfigPda(params.programIds), isSigner: false, isWritable: false },
      { pubkey: params.issuerAccount, isSigner: false, isWritable: false },
      { pubkey: permission, isSigner: false, isWritable: true },
      { pubkey: params.registryAuthority, isSigner: true, isWritable: false },
    ],
    data: Buffer.from(concatBytes(REVOKE_SCHEMA_PERMISSION_DISCRIMINATOR, params.schemaHash)),
  }));
}

export function decodeRegistryConfig(data: Buffer | Uint8Array): RegistryConfigSummary {
  assertDiscriminator(data, REGISTRY_CONFIG_DISCRIMINATOR, 'RegistryConfig');
  const reader = new BorshReader(data);
  reader.skip(8);
  return {
    authority: reader.publicKey(),
    governanceTokenMint: reader.publicKey(),
    minStakeLamports: reader.u64('registry.min_stake_lamports'),
    votingPeriodSeconds: reader.i64('registry.voting_period_seconds'),
    approvalThresholdBps: reader.u64('registry.approval_threshold_bps'),
    totalIssuers: reader.u64('registry.total_issuers'),
    activeIssuers: reader.u64('registry.active_issuers'),
    nextIssuerLeafIndex: reader.u64('registry.next_issuer_leaf_index'),
  };
}

export function decodeIssuerAccount(pubkey: PublicKey, data: Buffer | Uint8Array): IssuerAccountSummary {
  assertDiscriminator(data, ISSUER_ACCOUNT_DISCRIMINATOR, 'IssuerAccount');
  const reader = new BorshReader(data);
  reader.skip(8);
  const authority = reader.publicKey();
  const name = reader.string(64, 'issuer.name');
  const metadataUri = reader.string(128, 'issuer.metadata_uri');
  const bjjPubKeyX = bytesToHex(reader.bytes(32, 'issuer.bjj_pub_key_x'));
  const bjjPubKeyY = bytesToHex(reader.bytes(32, 'issuer.bjj_pub_key_y'));
  const tier = issuerTierName(reader.u8('issuer.tier'));
  const status = issuerStatusName(reader.u8('issuer.status'));
  return {
    authority,
    pda: pubkey.toBase58(),
    name,
    metadataUri,
    bjjPubKeyX,
    bjjPubKeyY,
    tier,
    status,
    stakedAmount: reader.u64('issuer.staked_amount'),
    registeredAt: reader.i64('issuer.registered_at'),
    creationSlot: reader.u64('issuer.creation_slot'),
    cooldownEndsAt: reader.i64('issuer.cooldown_ends_at'),
    votesFor: reader.u64('issuer.votes_for'),
    votesAgainst: reader.u64('issuer.votes_against'),
    votingEndsAt: reader.i64('issuer.voting_ends_at'),
    credentialsIssued: reader.u64('issuer.credentials_issued'),
    slashCount: reader.u64('issuer.slash_count'),
    revocationNonce: reader.u64('issuer.revocation_nonce'),
    statusEpoch: reader.u64('issuer.status_epoch'),
    issuerTreeLeafIndex: reader.u64('issuer.issuer_tree_leaf_index'),
    isTreeEnrolled: reader.bool('issuer.is_tree_enrolled'),
    dataSize: data.length,
  };
}

export function decodeIssuerSchemaPermission(
  pubkey: PublicKey,
  data: Buffer | Uint8Array,
): IssuerSchemaPermissionSummary {
  assertDiscriminator(data, ISSUER_SCHEMA_PERMISSION_DISCRIMINATOR, 'IssuerSchemaPermission');
  const reader = new BorshReader(data);
  reader.skip(8);
  return {
    pda: pubkey.toBase58(),
    issuer: reader.publicKey(),
    issuerAuthority: reader.publicKey(),
    schemaHash: bytesToHex(reader.bytes(32, 'issuer_schema_permission.schema_hash')),
    schemaAccount: reader.publicKey(),
    grantedBy: reader.publicKey(),
    grantedAt: reader.i64('issuer_schema_permission.granted_at'),
    revokedAt: reader.i64('issuer_schema_permission.revoked_at'),
    active: reader.bool('issuer_schema_permission.active'),
    bump: reader.u8('issuer_schema_permission.bump'),
    dataSize: data.length,
  };
}

export function decodeSchemaAccount(pubkey: PublicKey, data: Buffer | Uint8Array): SchemaAccountSummary {
  assertDiscriminator(data, SCHEMA_ACCOUNT_DISCRIMINATOR, 'SchemaAccount');
  const reader = new BorshReader(data);
  reader.skip(8);
  const authority = reader.publicKey();
  const name = reader.string(64, 'schema.name');
  const version = reader.u8('schema.version');
  const category = reader.string(64, 'schema.category');
  const fieldCount = reader.u32('schema.field_names length');
  if (fieldCount < 1 || fieldCount > 8) {
    throw new Error(`schema.field_names length must be 1-8, got ${fieldCount}`);
  }
  const fieldNames = Array.from({ length: fieldCount }, (_, index) =>
    reader.string(32, `schema.field_names[${index}]`),
  );
  return {
    pda: pubkey.toBase58(),
    authority,
    name,
    version,
    category,
    fieldNames,
    schemaHash: bytesToHex(reader.bytes(32, 'schema.schema_hash')),
    deprecated: reader.bool('schema.deprecated'),
    createdAt: reader.i64('schema.created_at'),
    usageCount: reader.u64('schema.usage_count'),
    dataSize: data.length,
  };
}

export function decodeStakerAccount(pubkey: PublicKey, data: Buffer | Uint8Array): StakerAccountSummary {
  assertDiscriminator(data, STAKER_ACCOUNT_DISCRIMINATOR, 'StakerAccount');
  const reader = new BorshReader(data);
  reader.skip(8);
  return {
    pda: pubkey.toBase58(),
    voter: reader.publicKey(),
    amountStaked: reader.u64('staker.amount_staked'),
    activeVotesCount: reader.u32('staker.active_votes_count'),
    lastStakeSlot: reader.u64('staker.last_stake_slot'),
  };
}

export function decodeSubgroupVerifierConfig(data: Buffer | Uint8Array): SubgroupVerifierConfigSummary {
  assertDiscriminator(data, SUBGROUP_VERIFIER_CONFIG_DISCRIMINATOR, 'SubgroupVerifierConfig');
  const reader = new BorshReader(data);
  reader.skip(8);
  return {
    authority: reader.publicKey(),
    bump: reader.u8('subgroup.bump'),
    paused: reader.bool('subgroup.paused'),
    vkInitialized: reader.bool('subgroup.vk_initialized'),
    nextVkChunk: reader.u16('subgroup.next_vk_chunk'),
    vkFinalized: reader.bool('subgroup.vk_finalized'),
    vkGeneration: reader.u16('subgroup.vk_generation'),
    rotateRequestTs: reader.i64('subgroup.rotate_request_ts'),
  };
}

export function decodeIssuerTreeBinding(data: Buffer | Uint8Array): IssuerTreeBindingSummary {
  if (data.length < ISSUER_TREE_BINDING_SIZE) {
    throw new Error(`IssuerTreeBinding account is malformed: expected ${ISSUER_TREE_BINDING_SIZE} bytes, got ${data.length}`);
  }
  if (!bytesEqual(data.subarray(0, 8), ISSUER_TREE_BINDING_DISCRIMINATOR)) {
    throw new Error('IssuerTreeBinding discriminator mismatch.');
  }
  const treeAddress = new PublicKey(data.subarray(8, 40)).toBase58();
  const currentRoot = bytesToHex(data.subarray(40, 72));
  const lastUpdatedSlot = readBigUInt64Le(data, 72);
  const status = data[80] === 0 ? 'active' : data[80] === 1 ? 'frozen' : 'unknown';
  return { currentRoot, lastUpdatedSlot, treeAddress, status };
}

function buildCreateAssociatedTokenAccountIdempotentIx(params: {
  payer: PublicKey;
  owner: PublicKey;
  mint: PublicKey;
  ata: PublicKey;
  programIds?: IssuerRegistryProgramIdOverrides;
}): TransactionInstruction {
  const ids = resolveProgramIds(params.programIds);
  return new TransactionInstruction({
    programId: ids.associatedTokenProgram,
    keys: [
      { pubkey: params.payer, isSigner: true, isWritable: true },
      { pubkey: params.ata, isSigner: false, isWritable: true },
      { pubkey: params.owner, isSigner: false, isWritable: false },
      { pubkey: params.mint, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: ids.tokenProgram, isSigner: false, isWritable: false },
    ],
    data: Buffer.from([1]),
  });
}

function tierToAnchorEnum(tier: IssuerTierName): number {
  const index = ISSUER_TIER_OPTIONS.findIndex((item) => item.value === tier);
  if (index < 0) throw new Error(`Unsupported issuer tier: ${tier}`);
  return index;
}

function issuerTierName(value: number): IssuerAccountSummary['tier'] {
  return ['community', 'enterprise', 'regulated', 'government'][value] as IssuerAccountSummary['tier'] ?? 'unknown';
}

function issuerStatusName(value: number): IssuerAccountSummary['status'] {
  return ['pending', 'approved', 'cooldown', 'rejected', 'revoked'][value] as IssuerAccountSummary['status'] ?? 'unknown';
}

function borshString(value: string): Uint8Array {
  const bytes = new TextEncoder().encode(value);
  return concatBytes(u32Le(bytes.length), bytes);
}

function borshBytes(value: Uint8Array): Uint8Array {
  return concatBytes(u32Le(value.length), value);
}

function u32Le(value: number): Uint8Array {
  if (!Number.isInteger(value) || value < 0 || value > 0xffffffff) throw new Error('u32 value out of range.');
  const out = new Uint8Array(4);
  out[0] = value & 0xff;
  out[1] = (value >> 8) & 0xff;
  out[2] = (value >> 16) & 0xff;
  out[3] = (value >> 24) & 0xff;
  return out;
}

function u64Le(value: bigint): Uint8Array {
  if (value < 0n || value > 0xffffffffffffffffn) throw new Error('u64 value out of range.');
  const out = new Uint8Array(8);
  let remaining = value;
  for (let i = 0; i < 8; i++) {
    out[i] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
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

  string(maxBytes: number, name: string): string {
    const length = this.u32(`${name} length`);
    if (length > maxBytes) throw new Error(`${name} exceeds ${maxBytes} bytes`);
    return new TextDecoder().decode(this.bytes(length, name));
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
    throw new Error(`${name} discriminator mismatch.`);
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

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
}

function base58Encode(bytes: Uint8Array): string {
  const alphabet = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
  let value = 0n;
  for (const byte of bytes) value = value * 256n + BigInt(byte);
  let encoded = '';
  while (value > 0n) {
    const mod = Number(value % 58n);
    encoded = alphabet[mod] + encoded;
    value /= 58n;
  }
  for (const byte of bytes) {
    if (byte === 0) encoded = alphabet[0] + encoded;
    else break;
  }
  return encoded || alphabet[0];
}
