/**
 * SolID Protocol Global Configuration
 *
 * v0.2 (2026-04 R-2 remediation):
 *   - Removed Photon / Light Protocol indexer URLs; SolID now reads
 *     compressed state directly from SPL Account Compression trees via
 *     standard Solana RPC endpoints.
 *   - Program IDs are the canonical `Anchor.toml` keys.  `check_program_ids.py`
 *     enforces that this file stays in lock-step with `Anchor.toml` and
 *     `deployments/devnet.json`.
 */

export const SOLID_CONFIG = {
  // Primary Solana RPC + hot standbys (priority failover).  Override per-env
  // from your dApp's runtime config.
  SOLANA_RPC_URLS: [
    'https://api.devnet.solana.com',
    // Add Helius / Triton / QuickNode endpoints here for production resilience.
  ],

  /** Back-compat alias — single URL, used by callers that don't want failover. */
  SOLANA_RPC_URL: 'https://api.devnet.solana.com',

  // Optional Merkle-proof indexer endpoint.  The holder SDK accepts any
  // `MerkleProofAdapter` implementation; `undefined` means "use the local
  // replica adapter" (suitable for localnet / E2E tests).  For mainnet, set
  // this to a Helius DAS or custom indexer URL and plug it into a
  // `HeliusDasAdapter` (SDK-side, not shipped here to keep deps minimal).
  MERKLE_PROOF_ENDPOINT: undefined as string | undefined,

  // Circuit artifacts (wasm + zkey) — CDN/IPFS in production.
  ARTIFACT_BASE_URL: 'https://cdn.solid-protocol.com/artifacts/v1',

  // Registry & Governance (Placeholder)
  AUTHORITY_PUBKEY: 'SoLid1111111111111111111111111111111111111',
  SOLID_TOKEN_MINT: 'SoLidToken11111111111111111111111111111111',

  // Canonical program IDs (MUST match Anchor.toml).  `scripts/check_program_ids.py`
  // fails CI on drift.
  PROGRAM_IDS: {
    ZK_VERIFIER: 'DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb',
    ISSUER_REGISTRY: '5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx',
    SCHEMA_REGISTRY: '4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1',
  },

  // Circuit Settings
  CIRCUIT_METADATA: {
    BATCH_QUERY: {
      WASM_PATH: '/batch_credential_query.wasm',
      ZKEY_PATH: '/batch_credential_query.zkey',
      PUBLIC_INPUTS: 31,
    },
  },
};
