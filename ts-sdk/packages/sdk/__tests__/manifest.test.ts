import { describe, expect, it } from 'vitest';
import {
  DEFAULT_DEVNET_MANIFEST,
  artifactPinsFromManifest,
  artifactUrl,
  programIdsFromManifest,
  validateSolidManifest,
  withManifestOverrides,
  type SolidDevnetManifest,
} from '../src/manifest';

describe('devnet manifest', () => {
  it('accepts the staged default manifest', () => {
    const manifest = validateSolidManifest(DEFAULT_DEVNET_MANIFEST);

    expect(manifest.schema_version).toBe(1);
    expect(programIdsFromManifest(manifest)).toEqual({
      schemaRegistry: manifest.programs.schema_registry.program_id,
      issuerRegistry: manifest.programs.issuer_registry.program_id,
      zkVerifier: manifest.programs.zk_verifier.program_id,
    });
    expect(artifactPinsFromManifest(manifest).batchWasm).toMatch(/^[0-9a-f]{64}$/);
  });

  it('keeps artifact URLs manifest-driven', () => {
    const manifest = withManifestOverrides(DEFAULT_DEVNET_MANIFEST, {
      artifactBaseUrl: 'https://cdn.solid.example/artifacts/',
    });

    expect(artifactUrl(manifest, 'subgroupWasm')).toBe(
      'https://cdn.solid.example/artifacts/bjj_subgroup_proof.wasm',
    );
  });

  it('keeps localnet and devnet runtime profiles explicit', () => {
    const manifest = withManifestOverrides(DEFAULT_DEVNET_MANIFEST, {
      network: 'localnet',
      cluster: 'http://127.0.0.1:8899',
      websocketCluster: 'ws://127.0.0.1:8900',
      artifactBaseUrl: 'http://127.0.0.1:5173/artifacts',
      indexerUrl: 'http://127.0.0.1:8787',
    });

    expect(manifest.network).toBe('localnet');
    expect(manifest.cluster).toBe('http://127.0.0.1:8899');
    expect(manifest.websocket_cluster).toBe('ws://127.0.0.1:8900');
    expect(manifest.indexer.url).toBe('http://127.0.0.1:8787');
  });

  it('rejects non-local plaintext network endpoints', () => {
    expect(() =>
      withManifestOverrides(DEFAULT_DEVNET_MANIFEST, {
        artifactBaseUrl: 'http://example.com/artifacts',
      }),
    ).toThrow(/HTTPS/);
  });

  it('validates schema catalog entries instead of accepting arbitrary JSON', () => {
    const goodManifest: SolidDevnetManifest = {
      ...DEFAULT_DEVNET_MANIFEST,
      schemas: [
        {
          name: 'kyc_v1',
          display_name: 'KYC v1',
          version: 1,
          category: 'Compliance',
          schema_hash: 'a'.repeat(64),
          schema_pda: null,
          tree_address: null,
          tree_depth: 20,
          current_root: null,
          current_root_slot: null,
          fields: ['age', 'country_code'],
          predicates: ['GTE', 'EQ'],
          field_metadata: [
            { name: 'age', type: 'uint64', description: 'Age in years', range_queryable: true },
            { name: 'country_code', type: 'uint64', description: 'ISO numeric country code' },
          ],
        },
      ],
    };

    expect(validateSolidManifest(goodManifest).schemas[0].field_metadata?.[0].name).toBe('age');

    expect(() =>
      validateSolidManifest({
        ...goodManifest,
        schemas: [{ ...goodManifest.schemas[0], schema_hash: 'not-a-hash' }],
      }),
    ).toThrow(/schema_hash/);

    expect(() =>
      validateSolidManifest({
        ...goodManifest,
        schemas: [{ ...goodManifest.schemas[0], fields: [] }],
      }),
    ).toThrow(/fields/);
  });

  it('validates tree root metadata used by wallet proof generation', () => {
    const manifest: SolidDevnetManifest = {
      ...DEFAULT_DEVNET_MANIFEST,
      trees: {
        global_state_tree: {
          binding_pda: '11111111111111111111111111111112',
          current_root: 'b'.repeat(64),
          current_root_slot: 1,
          depth: 20,
        },
        issuer_tree: {
          tree_address: '11111111111111111111111111111113',
          binding_pda: '11111111111111111111111111111114',
          current_root: 'c'.repeat(64),
          current_root_slot: 2,
        },
        schema_trees: [
          {
            schema_hash: 'd'.repeat(64),
            tree_address: '11111111111111111111111111111115',
            binding_pda: '11111111111111111111111111111116',
            current_root: 'e'.repeat(64),
            current_root_slot: 3,
          },
        ],
      },
    };

    expect(validateSolidManifest(manifest).trees.global_state_tree?.binding_pda).toBe(
      '11111111111111111111111111111112',
    );

    expect(() =>
      validateSolidManifest({
        ...manifest,
        trees: {
          ...manifest.trees,
          global_state_tree: { current_root: 'b'.repeat(64) },
        },
      }),
    ).toThrow(/tree_address or binding_pda/);

    expect(() =>
      validateSolidManifest({
        ...manifest,
        trees: {
          ...manifest.trees,
          schema_trees: [{ ...manifest.trees.schema_trees[0], current_root: 'not-a-root' }],
        },
      }),
    ).toThrow(/current_root/);
  });
});
