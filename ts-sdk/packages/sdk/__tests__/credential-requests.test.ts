import { afterEach, describe, expect, it, vi } from 'vitest';
import { SolidCredentialRequestClient } from '../src/credential-requests';

describe('SolidCredentialRequestClient', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('creates credential requests against the v1 API contract', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ id: 'req_1', status: 'requested' }),
    });
    vi.stubGlobal('fetch', fetchMock);

    const client = new SolidCredentialRequestClient('https://indexer.solid.example/');
    await expect(client.create({
      schemaHash: 'a'.repeat(64),
      schemaName: 'kyc_v1',
      schemaVersion: 1,
      issuerAuthority: 'issuer',
      holderChannelPublicKey: 'channel',
      holderPublicKeyX: 'x',
      holderPublicKeyY: 'y',
    })).resolves.toMatchObject({ id: 'req_1', status: 'requested' });

    expect(fetchMock).toHaveBeenCalledWith('https://indexer.solid.example/v1/credential-requests', expect.objectContaining({
      method: 'POST',
    }));
  });

  it('serializes list filters without empty values', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ requests: [] }),
    });
    vi.stubGlobal('fetch', fetchMock);

    const client = new SolidCredentialRequestClient('https://indexer.solid.example');
    await client.list({ issuerAuthority: 'issuer', schemaHash: undefined });

    expect(fetchMock).toHaveBeenCalledWith(
      'https://indexer.solid.example/v1/credential-requests?issuerAuthority=issuer',
      expect.any(Object),
    );
  });
});
