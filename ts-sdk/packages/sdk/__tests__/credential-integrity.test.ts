import { describe, expect, it } from 'vitest';
import {
  computeCommitment,
  deriveCredentialKey,
  generateKeypair,
  initWasm,
  sign,
} from '@solid-protocol/core';
import {
  assertCredentialIsSoundForWallet,
  type CredentialIntegrityBundle,
} from '../src/credential-integrity';

describe('assertCredentialIsSoundForWallet', () => {
  it('accepts a credential signed by an issuer and issued to this wallet-derived holder key', async () => {
    const masterSeed = new Uint8Array(32).fill(7);
    await expect(assertCredentialIsSoundForWallet(await makeBundle(masterSeed), masterSeed))
      .resolves.toBeUndefined();
  });

  it('rejects a credential issued to a different wallet identity', async () => {
    const bundle = await makeBundle(new Uint8Array(32).fill(7));
    await expect(assertCredentialIsSoundForWallet(bundle, new Uint8Array(32).fill(8)))
      .rejects.toThrow(/WRONG_HOLDER/);
  });

  it('rejects commitment drift before storage', async () => {
    const masterSeed = new Uint8Array(32).fill(7);
    const bundle = await makeBundle(masterSeed);
    await expect(assertCredentialIsSoundForWallet({
      ...bundle,
      attestationData: ['26', ...bundle.attestationData.slice(1)],
    }, masterSeed)).rejects.toThrow(/INVALID_COMMITMENT/);
  });

  it('rejects issuer signature drift before storage', async () => {
    const masterSeed = new Uint8Array(32).fill(7);
    const bundle = await makeBundle(masterSeed);
    await expect(assertCredentialIsSoundForWallet({
      ...bundle,
      issuerSignature: { ...bundle.issuerSignature, S: (BigInt(bundle.issuerSignature.S) + 1n).toString() },
    }, masterSeed)).rejects.toThrow(/INVALID_ISSUER_SIGNATURE/);
  });
});

async function makeBundle(masterSeed: Uint8Array): Promise<CredentialIntegrityBundle> {
  await initWasm();
  const schemaHash = new Uint8Array(32);
  for (let i = 0; i < 32; i++) schemaHash[i] = i + 1;
  const holder = deriveCredentialKey(masterSeed, schemaHash);
  const issuer = generateKeypair();
  const salt = new Uint8Array(32);
  salt[0] = 0x42;
  const attestationData = [25n, 840n, 2n, 1n, 0n, 0n, 0n, 0n];
  const commitment = computeCommitment(
    attestationData,
    schemaHash,
    holder.public_key_x,
    holder.public_key_y,
    salt,
  );
  const signature = sign(issuer.private_key, commitment);

  return {
    schemaHash: bytesToHex(schemaHash),
    attestationData: attestationData.map((value) => value.toString()),
    issuerSignature: {
      R8x: leBytesToDecimal(signature.r8_x),
      R8y: leBytesToDecimal(signature.r8_y),
      S: leBytesToDecimal(signature.s),
    },
    issuerPublicKey: {
      x: leBytesToDecimal(issuer.public_key_x),
      y: leBytesToDecimal(issuer.public_key_y),
    },
    holderPublicKey: {
      x: leBytesToDecimal(holder.public_key_x),
      y: leBytesToDecimal(holder.public_key_y),
    },
    salt: leBytesToDecimal(salt),
    commitment: bytesToHex(commitment),
  };
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
}

function leBytesToDecimal(bytes: Uint8Array): string {
  let result = 0n;
  for (let i = bytes.length - 1; i >= 0; i--) {
    result = (result << 8n) | BigInt(bytes[i]);
  }
  return result.toString();
}
