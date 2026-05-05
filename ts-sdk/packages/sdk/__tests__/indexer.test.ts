import { afterEach, describe, expect, it, vi } from 'vitest';
import { SolidIndexerClient } from '../src/indexer';

describe('SolidIndexerClient', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('normalizes the base URL and requests the v1 health endpoint', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({
        ok: true,
        network: 'devnet',
        slot: 20,
        indexed_slot: 19,
        lag_slots: 1,
      }),
    });
    vi.stubGlobal('fetch', fetchMock);

    const client = new SolidIndexerClient('https://indexer.solid.example/');
    await expect(client.health()).resolves.toMatchObject({ network: 'devnet', lag_slots: 1 });
    expect(fetchMock).toHaveBeenCalledWith('https://indexer.solid.example/v1/health', {
      headers: { accept: 'application/json' },
    });
  });

  it('surfaces HTTP failures with the requested route', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: false, status: 503 }));

    await expect(new SolidIndexerClient('https://indexer.solid.example').schemas())
      .rejects.toThrow('GET /v1/schemas returned HTTP 503');
  });
});
