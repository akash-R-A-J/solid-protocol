import { describe, expect, it, vi } from 'vitest';
import { PublicKey } from '@solana/web3.js';
import {
  SolidVerificationError,
  SolidVerifier,
  explainVerificationError,
  httpTransport,
  walletAdapterTransport,
  type ProofRequestEnvelope,
  type SerializedProof,
} from '../src/index';

const fixedNonce = '11'.repeat(32);

function verifier() {
  return new SolidVerifier({
    cluster: 'devnet',
    artifactHostUrl: 'https://solid.example/artifacts',
  });
}

describe('SolidVerifier', () => {
  it('resolves the live devnet basic_identity_v2 schema by default', () => {
    const solid = verifier();

    const requirement = solid.defineRequirement({
      schema: 'basic_identity_v2',
      predicates: [
        { field: 'age', op: '>=', value: 18 },
        { field: 'region', op: '==', value: 356 },
      ],
      compoundLogic: 'AND',
      action: { appId: 'demo-defi', action: 'join_pool', nonce: fixedNonce },
    });

    expect(requirement.schema.name).toBe('basic_identity_v2');
    expect(requirement.schema.version).toBe(2);
    expect(requirement.schemaHash).toBe('6b5014bf611a025a4693b196a517ece9f2d0672672a2eb38d7a50481474e6823');
    expect(requirement.publicInputs.predicateEncodings).toEqual([
      { field: 'age', fieldIndex: 0, op: 'GTE', value: '18' },
      { field: 'region', fieldIndex: 2, op: 'EQ', value: '356' },
    ]);
    expect(requirement.fingerprint).toMatch(/^[0-9a-f]{64}$/);
  });

  it('accepts partner schemas from an explicit schema catalog override', () => {
    const schemaHash = 'a'.repeat(64);
    const solid = new SolidVerifier({
      cluster: 'devnet',
      artifactHostUrl: 'https://solid.example/artifacts',
      schemaCatalog: [
        {
          name: 'partner_schema',
          version: 1,
          category: 'Partner',
          hash: schemaHash,
          fields: [{ name: 'score', index: 0, type: 'uint64' }],
        },
      ],
    });

    const requirement = solid.defineRequirement({
      schema: 'partner_schema',
      predicates: [{ field: 'score', op: '>=', value: 90 }],
      action: { appId: 'partner', action: 'gate', nonce: fixedNonce },
    });

    expect(requirement.schema.name).toBe('partner_schema');
    expect(requirement.schemaHash).toBe(schemaHash);
    expect(requirement.publicInputs.predicateEncodings[0]).toEqual({
      field: 'score',
      fieldIndex: 0,
      op: 'GTE',
      value: '90',
    });
  });

  it('uses walletAdapterTransport for direct wallet proof requests', async () => {
    const proof: SerializedProof = { publicSignals: [] };
    const requestProof = vi.fn<[ProofRequestEnvelope], Promise<SerializedProof>>()
      .mockResolvedValue(proof);
    const request = {
      version: 1,
      requirement: verifier().defineRequirement({
        schema: 'basic_identity_v2',
        predicates: [{ field: 'verification_level', op: '>=', value: 2 }],
        action: { appId: 'demo', action: 'access', nonce: fixedNonce },
      }),
      artifactPins: verifier().artifactPins,
      expiresAt: Date.now() + 60_000,
    } satisfies ProofRequestEnvelope;

    await expect(walletAdapterTransport({ requestProof }).send(request)).resolves.toBe(proof);
    expect(requestProof).toHaveBeenCalledWith(request);
  });

  it('reports a typed error when no wallet proof transport is available', async () => {
    const request = {
      version: 1,
      requirement: verifier().defineRequirement({
        schema: 'basic_identity_v2',
        predicates: [{ field: 'verification_level', op: '>=', value: 2 }],
        action: { appId: 'demo', action: 'access', nonce: fixedNonce },
      }),
      artifactPins: verifier().artifactPins,
      expiresAt: Date.now() + 60_000,
    } satisfies ProofRequestEnvelope;

    await expect(walletAdapterTransport({}).send(request)).rejects.toMatchObject({
      reason: 'MISSING_CREDENTIAL',
    });
  });

  it('posts proof requests through httpTransport', async () => {
    const proof: SerializedProof = { publicSignals: ['1'] };
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => proof,
    });
    vi.stubGlobal('fetch', fetchMock);
    const request = {
      version: 1,
      requirement: verifier().defineRequirement({
        schema: 'basic_identity_v2',
        predicates: [{ field: 'country_code', op: '!=', value: 840 }],
        action: { appId: 'demo', action: 'access', nonce: fixedNonce },
      }),
      artifactPins: verifier().artifactPins,
      expiresAt: Date.now() + 60_000,
    } satisfies ProofRequestEnvelope;

    await expect(httpTransport('https://holder.example/proof').send(request)).resolves.toBe(proof);
    expect(fetchMock).toHaveBeenCalledWith('https://holder.example/proof', expect.objectContaining({
      method: 'POST',
      body: JSON.stringify(request),
    }));

    vi.unstubAllGlobals();
  });

  it('keeps verifier errors explainable for app UIs', () => {
    const error = new SolidVerificationError('PROOF_REPLAYED', 'nullifier already exists');
    const message = explainVerificationError(error.reason, error.detail);

    expect(message.humanReadable).toMatch(/already .*used/i);
    expect(message.suggestedAction).toMatch(/fresh proof/i);
    expect(message.suggestedAction).toContain('nullifier already exists');
  });
});
