# SolID Deployment Command Log

This file records the commands used during the `solidislive.com` public devnet deployment so the setup can be repeated or audited later.

## AWS Lightsail Server

Server user and repo path:

```text
ubuntu@<lightsail-host>:~/solid-protocol
```

Public API target:

```text
https://api.solidislive.com
```

## Initial Repo Setup

```bash
cd ~/solid-protocol
npm ci
npm --prefix ts-sdk ci
```

## Rust/WASM Toolchain

```bash
curl https://sh.rustup.rs -sSf | sh -s -- -y
source "$HOME/.cargo/env"
rustup toolchain install 1.79.0 --profile minimal
rustup target add --toolchain 1.79.0 wasm32-unknown-unknown
rustup override set 1.79.0
cargo install wasm-pack --version 0.13.1 --locked
```

## Swap For Low-Memory Build

The Lightsail instance has around 1 GiB RAM, so the Rust/WASM release build was killed until swap was added.

```bash
free -h
sudo fallocate -l 4G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
free -h
echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
```

## WASM Bridge Build

First attempt wrote to the wrong path because `wasm-pack` resolves `--out-dir` relative to the `wasm/` crate:

```bash
CARGO_BUILD_JOBS=1 wasm-pack build wasm/ --target nodejs \
  --out-dir ts-sdk/packages/core/wasm --release --no-opt
```

Temporary copy used to unblock the current server:

```bash
mkdir -p ts-sdk/packages/core/wasm
cp -a wasm/ts-sdk/packages/core/wasm/. ts-sdk/packages/core/wasm/
ls ts-sdk/packages/core/wasm/solid_wasm.js
```

Correct command for future rebuilds:

```bash
CARGO_BUILD_JOBS=1 wasm-pack build wasm/ --target nodejs \
  --out-dir ../ts-sdk/packages/core/wasm --release --no-opt
```

SDK build:

```bash
npm --prefix ts-sdk run build
```

## Indexer Runtime Setup

```bash
cd ~/solid-protocol
mkdir -p ~/.solid-indexer
export SOLID_INDEXER_WRITE_TOKEN="$(openssl rand -hex 32)"
printf '%s\n' "$SOLID_INDEXER_WRITE_TOKEN" > ~/.solid-indexer/write-token.txt
chmod 600 ~/.solid-indexer/write-token.txt
```

Sanity checks before starting:

```bash
echo "$SOLID_INDEXER_WRITE_TOKEN"
ls "$PWD/deployments/devnet.json"
ls "$PWD/ts-sdk/packages/core/wasm/solid_wasm.js"
```

Foreground indexer start:

```bash
PORT=8787 \
HOST=127.0.0.1 \
SOLID_MANIFEST_PATH="$PWD/deployments/devnet.json" \
SOLID_INDEXER_STORE_PATH="$HOME/.solid-indexer/state.json" \
SOLID_RPC_URL=https://api.devnet.solana.com \
SOLID_INDEXER_WRITE_TOKEN="$SOLID_INDEXER_WRITE_TOKEN" \
npm start --prefix indexer
```

Local API checks from the AWS server:

```bash
curl -fsS http://127.0.0.1:8787/v1/health | python3 -m json.tool
curl -fsS http://127.0.0.1:8787/v1/manifest | python3 -m json.tool
curl -fsS http://127.0.0.1:8787/v1/schemas | python3 -m json.tool
curl -fsS http://127.0.0.1:8787/v1/issuers | python3 -m json.tool
```

## PM2 Persistent Process

```bash
sudo npm install -g pm2
cd ~/solid-protocol
export SOLID_INDEXER_WRITE_TOKEN="$(cat ~/.solid-indexer/write-token.txt)"
```

Created `ecosystem.config.cjs` on the server:

```bash
cat > ecosystem.config.cjs <<'EOF'
module.exports = {
  apps: [
    {
      name: 'solid-indexer',
      cwd: '/home/ubuntu/solid-protocol',
      script: 'indexer/src/server.mjs',
      interpreter: 'node',
      env: {
        PORT: '8787',
        HOST: '127.0.0.1',
        SOLID_MANIFEST_PATH: '/home/ubuntu/solid-protocol/deployments/devnet.json',
        SOLID_INDEXER_STORE_PATH: '/home/ubuntu/.solid-indexer/state.json',
        SOLID_RPC_URL: 'https://api.devnet.solana.com',
        SOLID_INDEXER_WRITE_TOKEN: process.env.SOLID_INDEXER_WRITE_TOKEN,
        SOLID_AUTO_ROOT_SYNC: '0'
      }
    }
  ]
};
EOF
```

Started and persisted PM2:

```bash
pm2 start ecosystem.config.cjs
pm2 save
pm2 startup
pm2 save
pm2 status
curl -fsS http://127.0.0.1:8787/v1/health | python3 -m json.tool
```

## Nginx And Certbot

Installed packages:

```bash
sudo apt install -y nginx certbot python3-certbot-nginx
```

Create API site config:

```bash
sudo nano /etc/nginx/sites-available/solid-api
```

Expected config:

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

Enable and reload:

```bash
sudo ln -s /etc/nginx/sites-available/solid-api /etc/nginx/sites-enabled/solid-api
sudo nginx -t
sudo systemctl reload nginx
```

TLS, after DNS points `api.solidislive.com` to the Lightsail static IPv4:

```bash
sudo certbot --nginx -d api.solidislive.com
```

Public API checks:

```bash
curl -fsS https://api.solidislive.com/v1/health | python3 -m json.tool
curl -fsS https://api.solidislive.com/v1/manifest | python3 -m json.tool
curl -fsS https://api.solidislive.com/v1/schemas | python3 -m json.tool
curl -fsS https://api.solidislive.com/v1/issuers | python3 -m json.tool
```

## Vercel Artifact Deployment

Prepared the clean artifact directory locally:

```bash
cd /Users/rajakash/Desktop/testing/solid-protocol
rm -rf public-devnet-artifacts
mkdir -p public-devnet-artifacts

node <<'NODE'
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const manifest = JSON.parse(fs.readFileSync('deployments/devnet.json', 'utf8'));
const out = 'public-devnet-artifacts';

for (const [key, item] of Object.entries(manifest.artifacts.items)) {
  const source = path.resolve(item.local_path);
  const dest = path.join(out, item.filename);
  const bytes = fs.readFileSync(source);
  const actual = crypto.createHash('sha256').update(bytes).digest('hex');
  if (actual !== item.sha256) {
    throw new Error(`${key} hash mismatch: expected ${item.sha256}, got ${actual}`);
  }
  fs.copyFileSync(source, dest);
  fs.writeFileSync(`${dest}.sha256`, `${actual}  ${item.filename}\n`);
  console.log(`${item.filename} ${actual}`);
}
NODE

ls -lh public-devnet-artifacts
```

Created artifact Vercel config:

```bash
cd /Users/rajakash/Desktop/testing/solid-protocol/public-devnet-artifacts

cat > vercel.json <<'EOF'
{
  "headers": [
    {
      "source": "/(.*)",
      "headers": [
        { "key": "Access-Control-Allow-Origin", "value": "*" },
        { "key": "Cache-Control", "value": "public, max-age=31536000, immutable" },
        { "key": "X-Content-Type-Options", "value": "nosniff" }
      ]
    }
  ]
}
EOF
```

Logged into Vercel and deployed:

```bash
npx vercel login
npx vercel --prod
```

Deployment result:

```text
Project: solid-artifacts
Production: https://solid-artifacts-iurw0y51u-akash-rajs-projects-8a1c1fa8.vercel.app
Alias: https://solid-artifacts.vercel.app
```

Remaining artifact-domain command:

```bash
cd /Users/rajakash/Desktop/testing/solid-protocol/public-devnet-artifacts
npx vercel domains add artifacts.solidislive.com
npx vercel alias set https://solid-artifacts-iurw0y51u-akash-rajs-projects-8a1c1fa8.vercel.app artifacts.solidislive.com
```

Actual domain setup:

```bash
npx vercel domains add artifacts.solidislive.com
dig +short artifacts.solidislive.com
npx vercel alias set solid-artifacts-iurw0y51u-akash-rajs-projects-8a1c1fa8.vercel.app artifacts.solidislive.com
```

DNS record used in GoDaddy:

```text
Type: A
Name: artifacts
Value: 76.76.21.21
TTL: 1/2 hr
```

Result:

```text
https://artifacts.solidislive.com -> https://solid-artifacts-iurw0y51u-akash-rajs-projects-8a1c1fa8.vercel.app
```

Verified artifact downloads:

```bash
curl -I https://artifacts.solidislive.com/batch_credential_query.wasm
curl -I https://artifacts.solidislive.com/batch_credential_query.zkey
curl -I https://artifacts.solidislive.com/bjj_subgroup_proof.wasm
curl -I https://artifacts.solidislive.com/bjj_subgroup_proof.zkey
```

Verified pinned hashes:

```bash
curl -fsS https://artifacts.solidislive.com/batch_credential_query.wasm -o /tmp/batch_credential_query.wasm
shasum -a 256 /tmp/batch_credential_query.wasm

curl -fsS https://artifacts.solidislive.com/batch_credential_query.zkey -o /tmp/batch_credential_query.zkey
shasum -a 256 /tmp/batch_credential_query.zkey
```

Expected hashes:

```text
add8cb0390511405faf2ffb1213d3792b858c7b2082d5b4b92a84dcd627e61b4  batch_credential_query.wasm
7f43bbac249e8c913ac384ff4b007138d1ffb5bc489a16be42737598d96395e8  batch_credential_query.zkey
```

Artifact host root:

```text
https://artifacts.solidislive.com
```

The root initially returned Vercel's `404: NOT_FOUND` because only artifact
files were deployed. Added `public-devnet-artifacts/index.html` locally so the
root can show an intentional artifact landing page, then redeployed the
artifact project:

```bash
cd /Users/rajakash/Desktop/testing/solid-protocol/public-devnet-artifacts
npx vercel --prod
```

Redeploy result:

```text
Production: https://solid-artifacts-g0f9p27ue-akash-rajs-projects-8a1c1fa8.vercel.app
Aliased: https://artifacts.solidislive.com
```

## Public Manifest Update

Updated the AWS-hosted manifest:

```bash
cd ~/solid-protocol

node <<'NODE'
const fs = require('fs');
const path = 'deployments/devnet.json';
const manifest = JSON.parse(fs.readFileSync(path, 'utf8'));

manifest.note = 'Public devnet protocol smoke is green for basic_identity_v2. API and artifacts are hosted at solidislive.com. solid-sim app deployment is pending.';
manifest.artifacts.base_url = 'https://artifacts.solidislive.com';
manifest.artifacts.manifest_url = 'https://api.solidislive.com/v1/manifest';
manifest.indexer.url = 'https://api.solidislive.com';
manifest.console.url = 'https://app.solidislive.com';

if (manifest.deployer) {
  delete manifest.deployer.keypair_path;
}

fs.writeFileSync(path, JSON.stringify(manifest, null, 2) + '\n');
NODE

pm2 restart solid-indexer
curl -fsS https://api.solidislive.com/v1/manifest | python3 -m json.tool
```

## solid-sim Static Vercel Deployment

Built the app locally with public devnet URLs:

```bash
cd /Users/rajakash/Desktop/testing/solid-sim

VITE_SOLID_NETWORK=devnet \
VITE_SOLID_CLUSTER=devnet \
VITE_SOLID_RPC_URL=https://api.devnet.solana.com \
VITE_SOLID_WS_URL=wss://api.devnet.solana.com \
VITE_SOLID_MANIFEST_URL=https://api.solidislive.com/v1/manifest \
VITE_SOLID_INDEXER_URL=https://api.solidislive.com \
VITE_SOLID_ARTIFACT_BASE_URL=https://artifacts.solidislive.com \
VITE_SOLID_CONSOLE_URL=https://app.solidislive.com \
npm run build
```

Deployed the built `dist/` directory directly so Vercel did not need to resolve local SDK `file:` dependencies remotely:

```bash
cd /Users/rajakash/Desktop/testing/solid-sim/dist

cat > vercel.json <<'EOF'
{
  "rewrites": [
    { "source": "/(.*)", "destination": "/index.html" }
  ],
  "headers": [
    {
      "source": "/(.*)",
      "headers": [
        { "key": "X-Content-Type-Options", "value": "nosniff" },
        { "key": "Referrer-Policy", "value": "strict-origin-when-cross-origin" }
      ]
    }
  ]
}
EOF

npx vercel --prod
```

Deployment result:

```text
Project: solid-sim
Production: https://solid-l3nfu9gxw-akash-rajs-projects-8a1c1fa8.vercel.app
Alias: https://solid-sim.vercel.app
```

Custom domain setup:

```bash
cd /Users/rajakash/Desktop/testing/solid-sim/dist
npx vercel domains add app.solidislive.com
npx vercel alias set solid-l3nfu9gxw-akash-rajs-projects-8a1c1fa8.vercel.app app.solidislive.com
```

DNS record used in GoDaddy:

```text
Type: A
Name: app
Value: 76.76.21.21
TTL: 1/2 hr
```

Result:

```text
https://app.solidislive.com -> https://solid-l3nfu9gxw-akash-rajs-projects-8a1c1fa8.vercel.app
```

HTTP check:

```bash
curl -I https://app.solidislive.com
```

Result:

```text
HTTP/2 200
server: Vercel
content-type: text/html; charset=utf-8
```

