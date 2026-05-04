import { describe, it, expect } from 'vitest';
import {
  deriveChannelKeyFromSeed,
  deriveChannelKeyFromWallet,
  deriveChannelKeyFromMasterSeed,
  CHANNEL_DERIVATION_MESSAGE,
} from '../src/index.js';

describe('deriveChannelKeyFromSeed', () => {
  it('is deterministic for the same seed', () => {
    const seed = new Uint8Array(32).fill(0x42);
    const a = deriveChannelKeyFromSeed(seed);
    const b = deriveChannelKeyFromSeed(seed);

    expect(a.publicKey).toEqual(b.publicKey);
    expect(a.secretKey).toEqual(b.secretKey);
    expect(a.publicKey.length).toBe(32);
    expect(a.secretKey.length).toBe(32);
  });

  it('produces distinct keypairs for different seeds', () => {
    const a = deriveChannelKeyFromSeed(new Uint8Array(32).fill(1));
    const b = deriveChannelKeyFromSeed(new Uint8Array(32).fill(2));
    expect(a.publicKey).not.toEqual(b.publicKey);
  });

  it('rejects seeds of the wrong length', () => {
    expect(() => deriveChannelKeyFromSeed(new Uint8Array(31))).toThrow(/32 bytes/);
    expect(() => deriveChannelKeyFromSeed(new Uint8Array(33))).toThrow(/32 bytes/);
  });
});

describe('deriveChannelKeyFromWallet', () => {
  it('signs the documented domain message and SHA-256s the signature', async () => {
    const fakeSignature = new Uint8Array(64);
    for (let i = 0; i < 64; i++) fakeSignature[i] = i;
    let signedMessage: Uint8Array | null = null;

    const fromWallet = await deriveChannelKeyFromWallet(async (msg) => {
      signedMessage = msg;
      return fakeSignature;
    });

    const expectedMessage = new TextEncoder().encode(CHANNEL_DERIVATION_MESSAGE);
    expect(signedMessage).toEqual(expectedMessage);

    const seed = new Uint8Array(await crypto.subtle.digest('SHA-256', fakeSignature));
    const fromSeed = deriveChannelKeyFromSeed(seed);

    expect(fromWallet.publicKey).toEqual(fromSeed.publicKey);
    expect(fromWallet.secretKey).toEqual(fromSeed.secretKey);
  });
});

describe('deriveChannelKeyFromMasterSeed', () => {
  it('is deterministic for the same master seed', async () => {
    const seed = new Uint8Array(32).fill(0x99);
    const a = await deriveChannelKeyFromMasterSeed(seed);
    const b = await deriveChannelKeyFromMasterSeed(seed);

    expect(a.publicKey).toEqual(b.publicKey);
    expect(a.secretKey).toEqual(b.secretKey);
  });

  it('produces distinct keypairs for different master seeds', async () => {
    const a = await deriveChannelKeyFromMasterSeed(new Uint8Array(32).fill(1));
    const b = await deriveChannelKeyFromMasterSeed(new Uint8Array(32).fill(2));
    expect(a.publicKey).not.toEqual(b.publicKey);
  });

  it('produces distinct keypairs for different domain labels', async () => {
    const seed = new Uint8Array(32).fill(0x77);
    const v1 = await deriveChannelKeyFromMasterSeed(seed, 'solid:channel:v1');
    const other = await deriveChannelKeyFromMasterSeed(seed, 'solid:channel:experiment');
    expect(v1.publicKey).not.toEqual(other.publicKey);
  });

  it('rejects master seeds of the wrong length', async () => {
    await expect(deriveChannelKeyFromMasterSeed(new Uint8Array(31))).rejects.toThrow(/32 bytes/);
  });

  it('matches the documented domain-prefix recipe', async () => {
    const seed = new Uint8Array(32).fill(0x42);
    const fromHelper = await deriveChannelKeyFromMasterSeed(seed);

    // Manually replay the recipe.
    const domain = new TextEncoder().encode('solid:channel:v1');
    const material = new Uint8Array(domain.length + seed.length);
    material.set(domain, 0);
    material.set(seed, domain.length);
    const buf = material.buffer.slice(0) as ArrayBuffer;
    const derived = new Uint8Array(await crypto.subtle.digest('SHA-256', buf));
    const fromManual = deriveChannelKeyFromSeed(derived);

    expect(fromHelper.publicKey).toEqual(fromManual.publicKey);
    expect(fromHelper.secretKey).toEqual(fromManual.secretKey);
  });

  it('is independent of deriveChannelKeyFromWallet for the same seed', async () => {
    // Sanity: master-seed path != signature-of-master-seed path. Both are
    // valid; this test pins that they are intentionally distinct.
    const seed = new Uint8Array(32).fill(0x11);
    const fromMaster = await deriveChannelKeyFromMasterSeed(seed);
    const fromWalletWithMaster = await deriveChannelKeyFromWallet(async () => seed);
    expect(fromMaster.publicKey).not.toEqual(fromWalletWithMaster.publicKey);
  });
});

describe('CHANNEL_DERIVATION_MESSAGE', () => {
  it('is the load-bearing v1 constant', () => {
    expect(CHANNEL_DERIVATION_MESSAGE).toBe('solid:channel:v1');
  });
});
