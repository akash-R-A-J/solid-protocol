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

  /**
   * SOLID-SEC-058 (CRIT-3): SHA-256 pins for the off-chain prover artifacts.
   *
   * Empty string means "not pinned via this source".  The artifact-integrity
   * gate prefers env vars (`SOLID_CIRCUIT_WASM_SHA256`,
   * `SOLID_CIRCUIT_ZKEY_SHA256`, `SOLID_VK_SHA256`) and sidecar files
   * (`circuits/build/*.sha256`) over these constants -- those are populated
   * at build/CI time, while these are the production pins published with
   * a tagged release.  Mismatch on any source is a hard failure.
   *
   * For local dev: leave these empty and let the sidecars (written by
   * `circuits/scripts/setup.js`) provide the pin.  For dev artifacts that
   * are not pinned anywhere, set `SOLID_CIRCUIT_ARTIFACT_INTEGRITY=skip`
   * with eyes wide open -- see `artifact_integrity.ts`.
   */
  ARTIFACT_SHA256: {
    BATCH_QUERY_WASM: '' as string,
    BATCH_QUERY_ZKEY: '' as string,
    BATCH_QUERY_VK: '' as string,
    // SEC-048 Phase E.4 (2026-05-XX): subgroup-circuit artifacts.
    // Empty by default -- local dev gets pins from the in-tree sidecar
    // files at `circuits/build/bjj_subgroup_proof.{wasm,zkey}.sha256`
    // and `circuits/build/bjj_subgroup_verification_key.sha256`.
    // Tagged releases populate these constants from the canonical
    // ceremony output.
    SUBGROUP_WASM: '' as string,
    SUBGROUP_ZKEY: '' as string,
    SUBGROUP_VK: '' as string,
  },

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
  //
  // M10 / SOLID-SEC-072 (closed 2026-05-01): PUBLIC_INPUTS was 31
  // (pre-ADR-0014); ADR-0014 added `issuerTreeRoot` at slot [10] and
  // grew the contract to 32.  The canonical authoritative source is
  // `NR_PUBLIC_INPUTS` in `@solid-protocol/verifier`; this duplicate
  // is removed.  Callers that referenced `SOLID_CONFIG.CIRCUIT_METADATA.BATCH_QUERY.PUBLIC_INPUTS`
  // should import `NR_PUBLIC_INPUTS` from `@solid-protocol/verifier`
  // instead.
  CIRCUIT_METADATA: {
    BATCH_QUERY: {
      WASM_PATH: '/batch_credential_query.wasm',
      ZKEY_PATH: '/batch_credential_query.zkey',
    },
  },
};
