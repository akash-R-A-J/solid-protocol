export interface IndexerHealth {
  ok: boolean;
  network: string;
  slot: number;
  indexed_slot: number;
  lag_slots: number;
}

export interface IndexedRootSet {
  slot: number;
  global_state_root: string | null;
  issuer_tree_root: string | null;
  schema_tree_roots: Array<{
    schema_hash: string;
    tree_address: string;
    root: string;
  }>;
}

export interface IndexedMerkleProof {
  tree: string;
  leaf: string;
  root: string;
  siblings: string[];
  pathIndices: number[];
  leafIndex: number;
  slot: number;
}

export interface IndexedSchema {
  name: string;
  version: number;
  schema_hash: string;
  schema_pda: string;
  tree_address: string;
  fields: string[];
  predicates: string[];
}

export interface IndexedIssuer {
  authority: string;
  issuer_pda: string;
  status: string;
  name: string | null;
  metadata_uri: string | null;
  bjj_pubkey_x: string | null;
  bjj_pubkey_y: string | null;
  tree_leaf_index: string | null;
}

export class SolidIndexerClient {
  readonly baseUrl: string;

  constructor(baseUrl: string) {
    if (!baseUrl) throw new Error('SolidIndexerClient requires a base URL');
    this.baseUrl = baseUrl.replace(/\/$/, '');
  }

  health(): Promise<IndexerHealth> {
    return this.get('/v1/health');
  }

  roots(): Promise<IndexedRootSet> {
    return this.get('/v1/roots/current');
  }

  schemas(): Promise<IndexedSchema[]> {
    return this.get('/v1/schemas');
  }

  issuers(): Promise<IndexedIssuer[]> {
    return this.get('/v1/issuers');
  }

  credentialProof(tree: string, commitment: string): Promise<IndexedMerkleProof> {
    return this.get(`/v1/merkle-proof/${encodeURIComponent(tree)}/${encodeURIComponent(commitment)}`);
  }

  issuerProof(issuer: string): Promise<IndexedMerkleProof> {
    return this.get(`/v1/issuers/${encodeURIComponent(issuer)}/proof`);
  }

  schemaTree(schemaHash: string): Promise<{ schema_hash: string; tree_address: string; binding_pda: string | null }> {
    return this.get(`/v1/schemas/${encodeURIComponent(schemaHash)}/tree`);
  }

  private async get<T>(path: string): Promise<T> {
    const response = await fetch(`${this.baseUrl}${path}`, {
      headers: { accept: 'application/json' },
    });
    if (!response.ok) {
      throw new Error(`SolID indexer request failed: GET ${path} returned HTTP ${response.status}`);
    }
    return response.json() as Promise<T>;
  }
}
