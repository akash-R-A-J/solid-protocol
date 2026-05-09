# SolID Indexer

Production-shaped devnet API for SolID holder proof generation and credential-request workflow.

The indexer has two responsibilities:

- Serve Merkle inclusion proofs for already-indexed credential commitments.
- Coordinate holder-to-issuer credential requests for `solid-sim`.
- Maintain devnet simulator root consistency by syncing schema/global binding
  roots service-side before returning holder proof paths.

It does not fabricate proofs. If a credential leaf has not been indexed, `/v1/merkle-proof/:tree/:leaf` returns `404 LEAF_NOT_INDEXED`.

## Run Locally

```bash
cd solid-protocol/indexer
npm install
npm test
npm start
```

Environment:

```bash
PORT=8787
HOST=127.0.0.1
SOLID_MANIFEST_PATH=../deployments/devnet.json
SOLID_INDEXER_STORE_PATH=../.solid-indexer/state.json
SOLID_RPC_URL=https://api.devnet.solana.com
SOLID_INDEXER_WRITE_TOKEN=change-me
SOLID_ROOT_SYNC_KEYPAIR_PATH=../keys/devnet/deployer.json # optional; falls back to manifest deployer keypair path
```

Point `solid-sim` and `solid-wallet` at it with:

```bash
VITE_SOLID_INDEXER_URL=http://127.0.0.1:8787
```

Use HTTPS for public tester builds.

## API

Read endpoints:

```text
GET /.well-known/solid-protocol.json
GET /v1/manifest
GET /v1/health
GET /v1/roots/current
GET /v1/schemas
GET /v1/issuers
GET /v1/schemas/:schemaHash/tree
GET /v1/merkle-proof/:tree/:leaf
GET /v1/issuers/:issuer/proof
GET /v1/root-sync/plan
GET /v1/credential-requests
GET /v1/credential-requests/:id
```

Write endpoints:

```text
POST /v1/credential-requests
PATCH /v1/credential-requests/:id
POST /v1/tree-leaves
POST /v1/events/credential-issued
```

`POST /v1/tree-leaves` and `POST /v1/events/credential-issued` require:

```text
Authorization: Bearer $SOLID_INDEXER_WRITE_TOKEN
```

Credential requests are intentionally API-level records. The issuer still performs real verification, builds the encrypted credential envelope, and submits the protocol issuance transaction before marking a request `issued`.

Root sync is operator-side, not holder-side. If the indexer can load a root-sync
keypair whose public key matches the devnet manifest deployer/authority, proof
requests may advance `SchemaTreeBinding` and `GlobalStateBinding` roots before
returning Merkle proofs. If the keypair is missing or mismatched, the holder
proof flow should fail with a setup error instead of asking the holder to connect
the DAO/admin wallet.

## Devnet Deployment Notes

For devnet rollout, host this service separately from `solid-sim`, set the public HTTPS URL in `deployments/devnet.json` at `indexer.url`, then run:

```bash
npm run validate:devnet
npm run smoke:devnet-config
```

The wallet and verifier flows should stay blocked until this endpoint is live and returning real Merkle proofs for issued credentials.
