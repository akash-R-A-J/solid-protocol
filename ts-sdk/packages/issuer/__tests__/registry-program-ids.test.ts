import { describe, expect, it } from 'vitest';
import { Keypair, PublicKey } from '@solana/web3.js';
import {
  buildGrantSchemaPermissionTransaction,
  deriveIssuerAccountPda,
  deriveIssuerSchemaPermissionPda,
  deriveSchemaAccountPda,
  deriveSchemaTreeBindingPda,
  deriveTreeAuthorityPda,
} from '../src/registry';
import { buildIssueCredentialIx } from '../src/index';

describe('issuer registry program-id overrides', () => {
  it('derives cached PDAs from the supplied deployment program IDs', () => {
    const issuerRegistry = Keypair.generate().publicKey;
    const schemaRegistry = Keypair.generate().publicKey;
    const authority = Keypair.generate().publicKey;
    const schemaHash = new Uint8Array(32).fill(9);
    const programIds = { issuerRegistry, schemaRegistry };

    const issuerPda = deriveIssuerAccountPda(authority, programIds);
    const issuerPdaAgain = deriveIssuerAccountPda(authority, programIds);
    const expectedIssuer = PublicKey.findProgramAddressSync(
      [Buffer.from('issuer'), authority.toBuffer()],
      issuerRegistry,
    )[0];

    expect(issuerPda).toBe(issuerPdaAgain);
    expect(issuerPda.toBase58()).toBe(expectedIssuer.toBase58());
    expect(deriveSchemaTreeBindingPda(schemaHash, programIds).toBase58()).toBe(
      PublicKey.findProgramAddressSync(
        [Buffer.from('schema-tree-binding'), Buffer.from(schemaHash)],
        schemaRegistry,
      )[0].toBase58(),
    );
  });

  it('builds grant and issue instructions against the same override set', () => {
    const issuerRegistry = Keypair.generate().publicKey;
    const schemaRegistry = Keypair.generate().publicKey;
    const registryAuthority = Keypair.generate().publicKey;
    const issuerAuthority = Keypair.generate().publicKey;
    const merkleTree = Keypair.generate().publicKey;
    const schemaHash = new Uint8Array(32).fill(7);
    const commitment = new Uint8Array(32).fill(8);
    const programIds = { issuerRegistry, schemaRegistry };
    const issuerAccount = deriveIssuerAccountPda(issuerAuthority, programIds);

    const grantIx = buildGrantSchemaPermissionTransaction({
      registryAuthority,
      issuerAccount,
      schemaName: 'accredited_investor',
      schemaVersion: 1,
      schemaHash,
      programIds,
    }).instructions[0];

    expect(grantIx.programId.toBase58()).toBe(issuerRegistry.toBase58());
    expect(grantIx.keys[2].pubkey.toBase58()).toBe(
      deriveSchemaAccountPda('accredited_investor', 1, programIds).toBase58(),
    );
    expect(grantIx.keys[3].pubkey.toBase58()).toBe(
      deriveIssuerSchemaPermissionPda(issuerAccount, schemaHash, programIds).toBase58(),
    );

    const issueIx = buildIssueCredentialIx(
      issuerAuthority,
      'accredited_investor',
      1,
      schemaHash,
      commitment,
      merkleTree,
      programIds,
    );

    expect(issueIx.programId.toBase58()).toBe(issuerRegistry.toBase58());
    expect(issueIx.keys[0].pubkey.toBase58()).toBe(issuerAccount.toBase58());
    expect(issueIx.keys[2].pubkey.toBase58()).toBe(
      deriveIssuerSchemaPermissionPda(issuerAccount, schemaHash, programIds).toBase58(),
    );
    expect(issueIx.keys[3].pubkey.toBase58()).toBe(
      deriveSchemaAccountPda('accredited_investor', 1, programIds).toBase58(),
    );
    expect(issueIx.keys[4].pubkey.toBase58()).toBe(
      deriveSchemaTreeBindingPda(schemaHash, programIds).toBase58(),
    );
    expect(issueIx.keys[5].pubkey.toBase58()).toBe(
      deriveTreeAuthorityPda(schemaHash, programIds).toBase58(),
    );
  });
});
