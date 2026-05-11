# SolID Public Devnet Deployment Tracker

This file tracks the current `solidislive.com` deployment state and the remaining configuration work.

## Current Architecture

| Surface | Host | Status |
| --- | --- | --- |
| Indexer/API + manifest | AWS Lightsail, `https://api.solidislive.com` | Live |
| App / simulator | Vercel, `https://app.solidislive.com` | Live |
| Circuit artifacts | Vercel, `https://artifacts.solidislive.com` | Live; root index page deployed |
| Docs | Vercel, `https://docs.solidislive.com` | Pending |
| Landing page | Vercel, `https://solidislive.com` | Pending |
| Verifier SDK | npm devnet alpha | Pending after API + artifacts are live |

## AWS Indexer Status

- [x] AWS Lightsail instance created.
- [x] Static IPv4 attached.
- [x] Repo cloned to `~/solid-protocol`.
- [x] Root dependencies installed with `npm ci`.
- [x] TypeScript SDK dependencies installed with `npm --prefix ts-sdk ci`.
- [x] WASM bridge generated at `ts-sdk/packages/core/wasm/`.
- [x] TypeScript SDK build passes with `npm --prefix ts-sdk run build`.
- [x] Swap enabled for low-memory Rust/WASM builds.
- [x] Indexer starts locally on `127.0.0.1:8787`.
- [x] PM2 runs `solid-indexer` persistently.
- [x] Local health check passes at `http://127.0.0.1:8787/v1/health`.
- [x] DNS `api.solidislive.com` points to the Lightsail static IPv4.
- [x] Nginx reverse proxy installed and configured.
- [x] Nginx rate limit configured for public API traffic.
- [x] TLS issued with Certbot for `api.solidislive.com`.
- [x] Public API health check passes at `https://api.solidislive.com/v1/health`.
- [x] Public manifest check passes at `https://api.solidislive.com/v1/manifest`.

## AWS Runtime Configuration

Indexer process:

```text
PORT=8787
HOST=127.0.0.1
SOLID_MANIFEST_PATH=/home/ubuntu/solid-protocol/deployments/devnet.json
SOLID_INDEXER_STORE_PATH=/home/ubuntu/.solid-indexer/state.json
SOLID_RPC_URL=https://api.devnet.solana.com
SOLID_INDEXER_WRITE_TOKEN=<stored on server only>
SOLID_AUTO_ROOT_SYNC=0
```

Do not commit or publish:

- AWS account IDs
- SSH key names or paths
- private IPs
- `.env` files
- write tokens
- paid RPC URLs
- Solana keypair files

## Nginx API Rate Limit

Planned public API reverse proxy:

```nginx
limit_req_zone $binary_remote_addr zone=solid_api:10m rate=5r/s;

server {
    listen 80;
    server_name api.solidislive.com;

    location / {
        limit_req zone=solid_api burst=20 nodelay;

        proxy_pass http://127.0.0.1:8787;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

Future adjustment knobs:

- Increase `rate=5r/s` if normal app traffic is throttled.
- Increase `burst=20` if proof/app screens make short request bursts.
- Add stricter limits for write endpoints if Nginx path-specific locations are split later.
- Keep `SOLID_INDEXER_WRITE_TOKEN` required for protected ingestion/write endpoints.

## Vercel Artifact Status

### Artifacts Host

Prepared artifact folder:

```text
solid-protocol/public-devnet-artifacts
```

Current Vercel artifact deployment:

```text
https://solid-artifacts-g0f9p27ue-akash-rajs-projects-8a1c1fa8.vercel.app
https://solid-artifacts.vercel.app
```

Target custom domain:

```text
https://artifacts.solidislive.com
```

Published files:

```text
batch_credential_query.wasm
batch_credential_query.zkey
batch_credential_query_verification_key.json
bjj_subgroup_proof.wasm
bjj_subgroup_proof.zkey
bjj_subgroup_verification_key.json
```

The copied artifacts matched the SHA-256 pins in `deployments/devnet.json` before deployment.

An `index.html` landing page has been added locally so the artifact host root
does not show Vercel's default `404: NOT_FOUND`. The artifact project has
been redeployed and aliased to `https://artifacts.solidislive.com`.

Remaining:

- [x] Prepare clean `public-devnet-artifacts` directory.
- [x] Verify local artifact hashes against manifest pins.
- [x] Deploy artifact folder to Vercel.
- [x] Attach `artifacts.solidislive.com` custom domain.
- [x] Verify artifact downloads from `https://artifacts.solidislive.com`.
- [x] Verify hosted artifact SHA-256 values match manifest pins.
- [x] Redeploy artifact project with `index.html`.

## Vercel App Configuration Still Needed

### solid-sim App

Current Vercel deployment:

```text
https://solid-l3nfu9gxw-akash-rajs-projects-8a1c1fa8.vercel.app
https://solid-sim.vercel.app
```

Vercel environment:

```text
VITE_SOLID_RPC_URL=https://api.devnet.solana.com
VITE_SOLID_WS_URL=wss://api.devnet.solana.com
VITE_SOLID_MANIFEST_URL=https://api.solidislive.com/v1/manifest
VITE_SOLID_INDEXER_URL=https://api.solidislive.com
VITE_SOLID_ARTIFACT_BASE_URL=https://artifacts.solidislive.com
VITE_SOLID_CONSOLE_URL=https://app.solidislive.com
```

Deploy target:

```text
https://app.solidislive.com
```

Domain status:

- [x] `app.solidislive.com` added to the `solid-sim` Vercel project.
- [x] GoDaddy DNS `A app -> 76.76.21.21` configured.
- [x] Vercel certificate issued and alias points to `solid-l3nfu9gxw-akash-rajs-projects-8a1c1fa8.vercel.app`.
- [ ] Browser smoke test completed.

## Manifest Updates Still Needed

`deployments/devnet.json` on the AWS server now serves the hosted URL fields:

```json
{
  "artifacts": {
    "base_url": "https://artifacts.solidislive.com",
    "manifest_url": "https://api.solidislive.com/v1/manifest"
  },
  "indexer": {
    "url": "https://api.solidislive.com"
  },
  "console": {
    "url": "https://app.solidislive.com"
  }
}
```

Then restart PM2 so `/v1/manifest` serves the updated file:

```bash
pm2 restart solid-indexer
```

Status:

- [x] `artifacts.base_url` points to `https://artifacts.solidislive.com`.
- [x] `artifacts.manifest_url` points to `https://api.solidislive.com/v1/manifest`.
- [x] `indexer.url` points to `https://api.solidislive.com`.
- [x] `console.url` points to `https://app.solidislive.com`.
- [x] `deployer.keypair_path` removed from the public manifest.
- [x] PM2 restarted after manifest update.

## Final Verification Checklist

- [x] `curl -fsS https://api.solidislive.com/v1/health`
- [x] `curl -fsS https://api.solidislive.com/v1/manifest`
- [x] `curl -fsS https://api.solidislive.com/v1/schemas`
- [x] `curl -fsS https://api.solidislive.com/v1/issuers`
- [x] Artifact downloads return `200`.
- [x] Artifact SHA-256 values match manifest pins.
- [x] Artifact root page deployed at `https://artifacts.solidislive.com`.
- [x] `solid-sim` loads from `https://app.solidislive.com`.
- [ ] Wallet credential proof flow can reach the indexer.
- [ ] Verifier flow can load artifacts and submit verification.
- [ ] Public docs/examples do not expose private deployment details.

