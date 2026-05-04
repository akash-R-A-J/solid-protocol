import { describe, it, expect } from 'vitest';
import nacl from 'tweetnacl';
import {
  encryptCredential,
  decryptCredential,
  serializeEnvelope,
  parseEnvelope,
  deriveChannelKeyFromSeed,
  validateCredentialBundle,
} from '../src/index.js';
import { makeBundle } from './fixtures.js';

describe('encryptCredential / decryptCredential roundtrip', () => {
  it('preserves the CredentialBundle exactly', () => {
    const seed = new Uint8Array(32).fill(7);
    const holder = deriveChannelKeyFromSeed(seed);
    const bundle = makeBundle();

    const envelope = encryptCredential(bundle, holder.publicKey);
    const decrypted = decryptCredential(envelope, holder.secretKey);

    expect(decrypted).toEqual(bundle);
  });

  it('produces a different ciphertext for each call (fresh ephemeral key + nonce)', () => {
    const holder = deriveChannelKeyFromSeed(new Uint8Array(32).fill(3));
    const bundle = makeBundle();

    const a = encryptCredential(bundle, holder.publicKey);
    const b = encryptCredential(bundle, holder.publicKey);

    expect(a.ciphertext).not.toBe(b.ciphertext);
    expect(a.ephemeralPublicKey).not.toBe(b.ephemeralPublicKey);
    expect(a.nonce).not.toBe(b.nonce);
  });

  it('roundtrips through JSON serialization', () => {
    const holder = deriveChannelKeyFromSeed(new Uint8Array(32).fill(9));
    const bundle = makeBundle();

    const envelope = encryptCredential(bundle, holder.publicKey);
    const json = serializeEnvelope(envelope);
    const reparsed = parseEnvelope(json);
    const decrypted = decryptCredential(reparsed, holder.secretKey);

    expect(decrypted).toEqual(bundle);
  });
});

describe('decryptCredential authentication', () => {
  it('rejects a tampered ciphertext', () => {
    const holder = deriveChannelKeyFromSeed(new Uint8Array(32).fill(7));
    const envelope = encryptCredential(makeBundle(), holder.publicKey);

    const corrupted = nacl.util?.decodeBase64
      ? nacl.util.decodeBase64(envelope.ciphertext)
      : Uint8Array.from(Buffer.from(envelope.ciphertext, 'base64'));
    corrupted[corrupted.length - 1] ^= 0x01;
    const tampered = {
      ...envelope,
      ciphertext: Buffer.from(corrupted).toString('base64'),
    };

    expect(() => decryptCredential(tampered, holder.secretKey)).toThrow(
      /Decryption failed/,
    );
  });

  it('rejects the wrong holder secret key', () => {
    const intended = deriveChannelKeyFromSeed(new Uint8Array(32).fill(7));
    const attacker = deriveChannelKeyFromSeed(new Uint8Array(32).fill(8));
    const envelope = encryptCredential(makeBundle(), intended.publicKey);

    expect(() => decryptCredential(envelope, attacker.secretKey)).toThrow(
      /Decryption failed/,
    );
  });

  it('rejects a swapped ephemeral public key', () => {
    const holder = deriveChannelKeyFromSeed(new Uint8Array(32).fill(7));
    const envelope = encryptCredential(makeBundle(), holder.publicKey);
    const swapped = {
      ...envelope,
      ephemeralPublicKey: Buffer.from(new Uint8Array(32).fill(0xab)).toString('base64'),
    };

    expect(() => decryptCredential(swapped, holder.secretKey)).toThrow(
      /Decryption failed/,
    );
  });
});

describe('parseEnvelope structural validation', () => {
  it('rejects unsupported version', () => {
    expect(() =>
      parseEnvelope(JSON.stringify({
        version: 2,
        ephemeralPublicKey: 'aaaa',
        nonce: 'bbbb',
        ciphertext: 'cccc',
      })),
    ).toThrow(/Unsupported envelope version/);
  });

  it('rejects missing fields', () => {
    expect(() =>
      parseEnvelope(JSON.stringify({ version: 1, ephemeralPublicKey: 'a', nonce: 'b' })),
    ).toThrow(/ciphertext/);
  });

  it('rejects malformed envelope key and nonce lengths before decrypting', () => {
    expect(() =>
      parseEnvelope(JSON.stringify({
        version: 1,
        ephemeralPublicKey: Buffer.from(new Uint8Array(31)).toString('base64'),
        nonce: Buffer.from(new Uint8Array(24)).toString('base64'),
        ciphertext: Buffer.from(new Uint8Array(32)).toString('base64'),
      })),
    ).toThrow(/ephemeralPublicKey.*32 bytes/);

    expect(() =>
      parseEnvelope(JSON.stringify({
        version: 1,
        ephemeralPublicKey: Buffer.from(new Uint8Array(32)).toString('base64'),
        nonce: Buffer.from(new Uint8Array(23)).toString('base64'),
        ciphertext: Buffer.from(new Uint8Array(32)).toString('base64'),
      })),
    ).toThrow(/nonce.*24 bytes/);
  });

  it('rejects malformed JSON', () => {
    expect(() => parseEnvelope('{not json')).toThrow(/Invalid envelope JSON/);
  });
});

describe('CredentialBundle validation', () => {
  it('accepts the canonical fixture', () => {
    const result = validateCredentialBundle(makeBundle());
    expect(result).toEqual({ ok: true, errors: [] });
  });

  it('rejects reserved all-zero active schema and commitment values', () => {
    expect(() =>
      encryptCredential(makeBundle({ schemaHash: '00'.repeat(32) }), new Uint8Array(32)),
    ).toThrow(/schemaHash.*all-zero/);

    expect(() =>
      encryptCredential(makeBundle({ commitment: '00'.repeat(32) }), new Uint8Array(32)),
    ).toThrow(/commitment.*all-zero/);
  });

  it('requires exactly eight attestation fields', () => {
    expect(() =>
      encryptCredential(makeBundle({ attestationData: ['1', '2'] }), new Uint8Array(32)),
    ).toThrow(/exactly 8 fields/);
  });
});
