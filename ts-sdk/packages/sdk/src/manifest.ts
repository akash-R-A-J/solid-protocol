export type SolidNetwork = 'localnet' | 'devnet' | 'testnet' | 'mainnet-beta';

export type SolidProgramName = 'schema_registry' | 'issuer_registry' | 'zk_verifier';

export type SolidArtifactKey =
  | 'batchCredentialQueryWasm'
  | 'batchCredentialQueryZkey'
  | 'batchCredentialQueryVerificationKey'
  | 'subgroupWasm'
  | 'subgroupZkey'
  | 'subgroupVerificationKey';

export interface SolidProgramDeployment {
  program_id: string;
  upgrade_authority: string | null;
  idl_metadata: unknown | null;
  binary: string;
  description: string;
}

export interface SolidArtifactManifestEntry {
  filename: string;
  sha256: string;
  content_type: string;
  local_path?: string;
}

export interface SolidSchemaManifestEntry {
  name: string;
  display_name?: string;
  version: number;
  category?: string;
  schema_hash: string;
  schema_pda: string | null;
  tree_address: string | null;
  fields: string[];
  predicates: string[];
  field_metadata?: Array<{
    name: string;
    type?: 'uint64' | 'enum' | 'timestamp' | 'boolean';
    description?: string;
    range_queryable?: boolean;
  }>;
}

export interface SolidDevnetManifest {
  schema_version: 1;
  network: SolidNetwork;
  cluster: string;
  websocket_cluster: string | null;
  deployed_at: string | null;
  git_commit: string | null;
  note?: string;
  deployer: {
    address: string | null;
    keypair_path?: string;
  };
  programs: Record<SolidProgramName, SolidProgramDeployment>;
  artifacts: {
    base_url: string | null;
    manifest_url: string | null;
    items: Record<SolidArtifactKey, SolidArtifactManifestEntry>;
  };
  indexer: {
    url: string | null;
    health_path: string;
    api_version: 'v1';
  };
  console: {
    url: string | null;
  };
  wallet: {
    release_url: string | null;
  };
  schemas: SolidSchemaManifestEntry[];
  trees: {
    global_state_tree: string | null;
    issuer_tree: {
      tree_address: string | null;
      binding_pda: string | null;
      current_root: string | null;
      current_root_slot: number | null;
    };
    schema_trees: Array<{
      schema_hash: string;
      tree_address: string;
      binding_pda: string | null;
    }>;
  };
  pdas: Record<string, { seeds: string[]; program: string; note?: string }>;
  toolchain: Record<string, string>;
}

export interface SolidManifestOverrides {
  cluster?: string;
  websocketCluster?: string | null;
  artifactBaseUrl?: string | null;
  indexerUrl?: string | null;
  consoleUrl?: string | null;
  walletReleaseUrl?: string | null;
}

export const DEFAULT_DEVNET_MANIFEST: SolidDevnetManifest = {
  schema_version: 1,
  network: 'devnet',
  cluster: 'https://api.devnet.solana.com',
  websocket_cluster: 'wss://api.devnet.solana.com',
  deployed_at: null,
  git_commit: null,
  note: 'Staged devnet manifest. Deployment fields remain null until the first sanctioned devnet deploy.',
  deployer: {
    address: null,
    keypair_path: '~/.config/solana/id.json',
  },
  programs: {
    schema_registry: {
      program_id: '4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1',
      upgrade_authority: null,
      idl_metadata: null,
      binary: 'target/deploy/schema_registry.so',
      description: 'On-chain credential schema registry plus SchemaTreeBinding and GlobalStateBinding PDAs',
    },
    issuer_registry: {
      program_id: '5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx',
      upgrade_authority: null,
      idl_metadata: null,
      binary: 'target/deploy/issuer_registry.so',
      description: 'DAO-governed issuer registry with staking, voting, and credential issuance',
    },
    zk_verifier: {
      program_id: 'DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb',
      upgrade_authority: null,
      idl_metadata: null,
      binary: 'target/deploy/zk_verifier.so',
      description: 'Groth16 batch verifier with PDA-per-nullifier replay protection',
    },
  },
  artifacts: {
    base_url: null,
    manifest_url: null,
    items: {
      batchCredentialQueryWasm: {
        filename: 'batch_credential_query.wasm',
        local_path: 'circuits/build/batch_credential_query_js/batch_credential_query.wasm',
        sha256: 'add8cb0390511405faf2ffb1213d3792b858c7b2082d5b4b92a84dcd627e61b4',
        content_type: 'application/wasm',
      },
      batchCredentialQueryZkey: {
        filename: 'batch_credential_query.zkey',
        local_path: 'circuits/build/batch_credential_query_final.zkey',
        sha256: '7f43bbac249e8c913ac384ff4b007138d1ffb5bc489a16be42737598d96395e8',
        content_type: 'application/octet-stream',
      },
      batchCredentialQueryVerificationKey: {
        filename: 'batch_credential_query_verification_key.json',
        local_path: 'circuits/build/verification_key.json',
        sha256: '8385b82b032f65e505c784b28486ca8bec7da3f3d4b97b82724e697734565146',
        content_type: 'application/json',
      },
      subgroupWasm: {
        filename: 'bjj_subgroup_proof.wasm',
        local_path: 'circuits/build/bjj_subgroup_proof_js/bjj_subgroup_proof.wasm',
        sha256: '2c00e5a455a3b6fe1dc2a8761737acf2911baf396aab70165de3abd2673608b0',
        content_type: 'application/wasm',
      },
      subgroupZkey: {
        filename: 'bjj_subgroup_proof.zkey',
        local_path: 'circuits/build/bjj_subgroup_proof_final.zkey',
        sha256: 'ea401ea9cbeb9ef829be30e0080b83a3c82544af53367b95a17f24a75845bd4a',
        content_type: 'application/octet-stream',
      },
      subgroupVerificationKey: {
        filename: 'bjj_subgroup_verification_key.json',
        local_path: 'circuits/build/bjj_subgroup_verification_key.json',
        sha256: '938ab39020f31156fa7e8fc230fc458adba5f13e08c64d41d9dbffdbc3643ce9',
        content_type: 'application/json',
      },
    },
  },
  indexer: {
    url: null,
    health_path: '/v1/health',
    api_version: 'v1',
  },
  console: {
    url: null,
  },
  wallet: {
    release_url: null,
  },
  schemas: [],
  trees: {
    global_state_tree: null,
    issuer_tree: {
      tree_address: null,
      binding_pda: null,
      current_root: null,
      current_root_slot: null,
    },
    schema_trees: [],
  },
  pdas: {
    verifier_config: {
      seeds: ['verifier-config'],
      program: 'DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb',
    },
    vk_storage: {
      seeds: ['vk-storage', '<verifier_config_pubkey>'],
      program: 'DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb',
    },
    nullifier_record: {
      seeds: ['null', '<nullifier_bytes>'],
      program: 'DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb',
    },
    registry_config: {
      seeds: ['registry-config'],
      program: '5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx',
    },
    dao_treasury: {
      seeds: ['dao-treasury'],
      program: '5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx',
    },
    global_binding: {
      seeds: ['global-binding'],
      program: '4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1',
    },
    schema_tree_binding: {
      seeds: ['schema-tree-binding', '<schema_hash>'],
      program: '4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1',
    },
  },
  toolchain: {
    anchor_cli: '0.30.1',
    anchor_lang: '0.30.1',
    solana_cli: '1.18.22',
    rust: '1.79.0',
    node: '18',
    circom: '2.1.9',
    snarkjs: '0.7.5',
    wasm_pack: '0.13.1',
  },
};

export function validateSolidManifest(input: unknown): SolidDevnetManifest {
  const manifest = input as SolidDevnetManifest;
  if (!manifest || typeof manifest !== 'object') {
    throw new Error('SolID manifest must be a JSON object');
  }
  if (manifest.schema_version !== 1) {
    throw new Error(`Unsupported SolID manifest schema_version: ${String(manifest.schema_version)}`);
  }
  if (!manifest.network || !manifest.cluster) {
    throw new Error('SolID manifest requires network and cluster');
  }
  for (const program of ['schema_registry', 'issuer_registry', 'zk_verifier'] as const) {
    const entry = manifest.programs?.[program];
    if (!entry?.program_id || !isLikelySolanaPublicKey(entry.program_id)) {
      throw new Error(`SolID manifest has invalid program ID for ${program}`);
    }
  }
  for (const key of Object.keys(DEFAULT_DEVNET_MANIFEST.artifacts.items) as SolidArtifactKey[]) {
    const artifact = manifest.artifacts?.items?.[key];
    if (!artifact?.filename) {
      throw new Error(`SolID manifest missing artifact entry ${key}`);
    }
    if (!isSha256Hex(artifact.sha256)) {
      throw new Error(`SolID manifest artifact ${key} has invalid SHA-256 pin`);
    }
  }
  if (manifest.artifacts.base_url != null) {
    requireHttpsOrLocalhost(manifest.artifacts.base_url, 'artifacts.base_url');
  }
  if (!manifest.indexer || manifest.indexer.api_version !== 'v1') {
    throw new Error('SolID manifest requires indexer.api_version = v1');
  }
  if (manifest.indexer?.url != null) {
    requireHttpsOrLocalhost(manifest.indexer.url, 'indexer.url');
  }
  if (!Array.isArray(manifest.schemas)) {
    throw new Error('SolID manifest schemas must be an array');
  }
  manifest.schemas.forEach(validateSchemaEntry);
  return manifest;
}

export async function loadSolidManifest(
  manifestUrl: string,
  fetchImpl: typeof fetch = fetch,
): Promise<SolidDevnetManifest> {
  requireHttpsOrLocalhost(manifestUrl, 'manifest URL');
  const response = await fetchImpl(manifestUrl, { headers: { accept: 'application/json' } });
  if (!response.ok) {
    throw new Error(`Failed to load SolID manifest from ${manifestUrl}: HTTP ${response.status}`);
  }
  return validateSolidManifest(await response.json());
}

export function withManifestOverrides(
  manifest: SolidDevnetManifest,
  overrides: SolidManifestOverrides,
): SolidDevnetManifest {
  return validateSolidManifest({
    ...manifest,
    cluster: overrides.cluster ?? manifest.cluster,
    websocket_cluster: overrides.websocketCluster === undefined
      ? manifest.websocket_cluster
      : overrides.websocketCluster,
    artifacts: {
      ...manifest.artifacts,
      base_url: overrides.artifactBaseUrl === undefined
        ? manifest.artifacts.base_url
        : emptyToNull(overrides.artifactBaseUrl),
    },
    indexer: {
      ...manifest.indexer,
      url: overrides.indexerUrl === undefined
        ? manifest.indexer.url
        : emptyToNull(overrides.indexerUrl),
    },
    console: {
      ...manifest.console,
      url: overrides.consoleUrl === undefined
        ? manifest.console.url
        : emptyToNull(overrides.consoleUrl),
    },
    wallet: {
      ...manifest.wallet,
      release_url: overrides.walletReleaseUrl === undefined
        ? manifest.wallet.release_url
        : emptyToNull(overrides.walletReleaseUrl),
    },
  });
}

export function programIdsFromManifest(manifest: SolidDevnetManifest) {
  return {
    zkVerifier: manifest.programs.zk_verifier.program_id,
    issuerRegistry: manifest.programs.issuer_registry.program_id,
    schemaRegistry: manifest.programs.schema_registry.program_id,
  } as const;
}

export function artifactUrl(
  manifest: SolidDevnetManifest,
  key: SolidArtifactKey,
): string | null {
  const base = manifest.artifacts.base_url;
  const artifact = manifest.artifacts.items[key];
  if (!base) return null;
  return `${base.replace(/\/$/, '')}/${artifact.filename}`;
}

export function artifactPinsFromManifest(manifest: SolidDevnetManifest) {
  return {
    batchWasm: manifest.artifacts.items.batchCredentialQueryWasm.sha256,
    batchZkey: manifest.artifacts.items.batchCredentialQueryZkey.sha256,
    batchVerificationKey: manifest.artifacts.items.batchCredentialQueryVerificationKey.sha256,
    subgroupWasm: manifest.artifacts.items.subgroupWasm.sha256,
    subgroupZkey: manifest.artifacts.items.subgroupZkey.sha256,
    subgroupVerificationKey: manifest.artifacts.items.subgroupVerificationKey.sha256,
  } as const;
}

function isSha256Hex(value: unknown): value is string {
  return typeof value === 'string' && /^[0-9a-f]{64}$/.test(value);
}

function isLikelySolanaPublicKey(value: unknown): value is string {
  return typeof value === 'string' && /^[1-9A-HJ-NP-Za-km-z]{32,44}$/.test(value);
}

function validateSchemaEntry(schema: SolidSchemaManifestEntry, index: number): void {
  const label = `schemas[${index}]`;
  if (!schema || typeof schema !== 'object') {
    throw new Error(`${label} must be an object`);
  }
  if (!schema.name || typeof schema.name !== 'string') {
    throw new Error(`${label}.name is required`);
  }
  if (!Number.isInteger(schema.version) || schema.version < 1 || schema.version > 255) {
    throw new Error(`${label}.version must be an integer from 1 to 255`);
  }
  if (!isSha256Hex(schema.schema_hash)) {
    throw new Error(`${label}.schema_hash must be a 32-byte hex hash`);
  }
  if (schema.schema_pda != null && !isLikelySolanaPublicKey(schema.schema_pda)) {
    throw new Error(`${label}.schema_pda must be a Solana public key or null`);
  }
  if (schema.tree_address != null && !isLikelySolanaPublicKey(schema.tree_address)) {
    throw new Error(`${label}.tree_address must be a Solana public key or null`);
  }
  if (!Array.isArray(schema.fields) || schema.fields.length < 1 || schema.fields.length > 8) {
    throw new Error(`${label}.fields must contain 1-8 fields`);
  }
  for (const field of schema.fields) {
    if (!field || typeof field !== 'string') {
      throw new Error(`${label}.fields must contain strings`);
    }
  }
  if (!Array.isArray(schema.predicates)) {
    throw new Error(`${label}.predicates must be an array`);
  }
  for (const predicate of schema.predicates) {
    if (!predicate || typeof predicate !== 'string') {
      throw new Error(`${label}.predicates must contain strings`);
    }
  }
  if (schema.field_metadata !== undefined) {
    if (!Array.isArray(schema.field_metadata)) {
      throw new Error(`${label}.field_metadata must be an array when present`);
    }
    for (const [metadataIndex, metadata] of schema.field_metadata.entries()) {
      const metadataLabel = `${label}.field_metadata[${metadataIndex}]`;
      if (!metadata || typeof metadata !== 'object' || typeof metadata.name !== 'string') {
        throw new Error(`${metadataLabel}.name is required`);
      }
      if (!schema.fields.includes(metadata.name)) {
        throw new Error(`${metadataLabel}.name must match a schema field`);
      }
      if (metadata.type !== undefined && !['uint64', 'enum', 'timestamp', 'boolean'].includes(metadata.type)) {
        throw new Error(`${metadataLabel}.type is unsupported`);
      }
      if (metadata.description !== undefined && typeof metadata.description !== 'string') {
        throw new Error(`${metadataLabel}.description must be a string`);
      }
      if (metadata.range_queryable !== undefined && typeof metadata.range_queryable !== 'boolean') {
        throw new Error(`${metadataLabel}.range_queryable must be a boolean`);
      }
    }
  }
}

function emptyToNull(value: string | null | undefined): string | null {
  if (value == null) return null;
  const trimmed = value.trim();
  return trimmed.length === 0 ? null : trimmed;
}

function requireHttpsOrLocalhost(url: string, label: string): void {
  const parsed = new URL(url, 'http://localhost');
  const localhost = ['localhost', '127.0.0.1', '[::1]'].includes(parsed.hostname);
  if (parsed.protocol !== 'https:' && parsed.protocol !== 'http:' && parsed.protocol !== 'wss:' && parsed.protocol !== 'ws:') {
    throw new Error(`${label} must be an HTTP(S) or WS(S) URL`);
  }
  if ((parsed.protocol === 'http:' || parsed.protocol === 'ws:') && !localhost) {
    throw new Error(`${label} must use HTTPS/WSS outside localhost`);
  }
}
