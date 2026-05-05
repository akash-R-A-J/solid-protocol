# SolID TypeScript SDK Release Notes

## 0.3.0 Devnet Preparation

- Added `@solid-protocol/sdk/manifest` as the browser-safe devnet manifest
  contract shared by SDK, console, wallet, and examples.
- Added `@solid-protocol/sdk/indexer` types and a small client for the
  required Merkle proof/status API.
- Added package `exports`, `files`, `license`, and `publishConfig` metadata
  across SDK packages so npm publication can be checked with
  `npm run pack:check`.
- Removed fake default verifier artifact/indexer hosts. Integrators must
  configure real hosted infrastructure or the SDK reports missing config.
- Kept artifact SHA-256 pins as load-bearing defaults so wallet and console
  can reject tampered circuit files.

## Publication Gate

Before public devnet:

```bash
cd solid-protocol/ts-sdk
npm ci
npm run build
npm run pack:check
```

Publish only after all packages build and the generated tarballs contain
`dist` files, type declarations, and no local `file:` dependencies.
