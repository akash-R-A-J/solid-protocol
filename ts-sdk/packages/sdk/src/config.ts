/**
 * SolID Protocol Global Configuration
 * 
 * Production-grade parameters with placeholders as requested.
 * These will be replaced with real values before mainnet deployment.
 */

export const SOLID_CONFIG = {
  // Solana RPC (Placeholder)
  SOLANA_RPC_URL: 'https://api.devnet.solana.com',

  // Light Protocol / Photon Indexer (Risk 1 Resilience)
  // Support for multiple endpoints with Priority Failover.
  PHOTON_RPC_URLS: [
    'https://photon.devnet.solana.com',      // Primary
    'https://photon.helius-rpc.com/devnet',  // Fallback 1
    'https://photon.tatum.io/devnet',        // Fallback 2
  ],

  // Circuit Artifacts Storage (Placeholder)
  ARTIFACT_BASE_URL: 'https://cdn.solid-protocol.com/artifacts/v1',

  // Registry & Governance (Placeholder)
  AUTHORITY_PUBKEY: 'SoLid1111111111111111111111111111111111111',
  SOLID_TOKEN_MINT: 'SoLidToken11111111111111111111111111111111',

  // Programs
  PROGRAM_IDS: {
    ZK_VERIFIER: 'VERify1111111111111111111111111111111111111',
    ISSUER_REGISTRY: 'CRGYfonXwDk6gKEm9fC1U33VVBkqnQVD3sPdLKzqHWoR', // Real ID from lib.rs
    SCHEMA_REGISTRY: 'SCHemA11111111111111111111111111111111111',
  },

  // Circuit Settings
  CIRCUIT_METADATA: {
    BATCH_QUERY: {
      WASM_PATH: '/batch_credential_query.wasm',
      ZKEY_PATH: '/batch_credential_query.zkey',
      PUBLIC_INPUTS: 31,
    }
  }
};
