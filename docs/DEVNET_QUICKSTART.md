# SolID Devnet Quickstart

This guide is for controlled public devnet testers and integrators.
Current status as of 2026-05-12: the three programs are deployed and
initialized on devnet, fresh `basic_identity_v2` issue/prove/replay is
green, the API/manifest is hosted at `https://api.solidislive.com`, the
artifacts are hosted at `https://artifacts.solidislive.com`, and the app
is hosted at `https://app.solidislive.com`.

Broader public onboarding should wait until the hosted browser smoke run is
complete and the public docs/landing pages are live.

## 1. Read The Manifest

The manifest is the source of truth for:

- Solana RPC
- program IDs
- artifact URLs and SHA-256 pins
- schema catalog
- tree addresses
- indexer URL
- solid-sim and wallet release URLs

Local file:

```bash
cat deployments/devnet.json
```

Hosted file:

```bash
curl https://api.solidislive.com/v1/manifest
```

Current live smoke values are listed in `docs/DEVNET_STATUS.md`.

## 2. Install SDK Packages

```bash
npm install @solid-protocol/sdk @solid-protocol/verifier @solid-protocol/channel
```

Use published npm versions for external apps. Local `file:` dependencies are
only for monorepo development.

Until npm packages are published, integrators testing from this monorepo
should build the SDK workspace first:

```bash
cd solid-protocol/ts-sdk
npm install
npm run build
```

## 2a. Manual Terminal E2E

Operators can reproduce the current devnet terminal flow from the
`solid-protocol` repo root:

```bash
export PATH="$PWD/.toolchain/bin:$PATH"
export SOLID_KEYPAIR_PATH="$HOME/.config/solana/solid-devnet-admin.json"
export SOLANA_KEYPAIR_PATH="$SOLID_KEYPAIR_PATH"
export SOLID_RPC_URL="https://api.devnet.solana.com"
export SOLID_ALLOW_NON_LOCALNET=1
export SOLID_VOTING_PERIOD_SECONDS=120
export SOLID_SCHEMA_NAME="basic_identity_v2"
export SOLID_SCHEMA_VERSION=2
export SOLID_SCHEMA_CATEGORY="Identity"
export SOLID_SCHEMA_FIELDS="age,country_code,region,id_type,verification_level,issued_date,nationality,_reserved"
export SOLID_SCHEMA_TREE_DEPTH=20

bash scripts/sync_program_keypairs.sh --reset-state
NO_DNA=1 anchor build --no-idl
npm run build:idl
npm install

npm run init-onchain
npm run backfill-issuer-tree
npm run bootstrap-schema-tree
npm run bootstrap-issuer
npm run issue
npm run prove
```

Expected current result:

- `npm run issue` issues a `basic_identity_v2` credential into the devnet
  schema tree.
- `npm run prove` builds a witness, passes local `snarkjs.groth16.verify`,
  submits `verify_batch_proof_v2`, and confirms replay rejection.
- Hosted app/API/artifact smoke is separate from terminal E2E and should be
  repeated before sending broad public tester traffic.

The repo now ships explicit runtime profiles:

```bash
# Protocol scripts
source config/devnet.env.example
source config/localnet.env.example

# solid-sim
cp ../solid-sim/.env.devnet.example ../solid-sim/.env
cp ../solid-sim/.env.localnet.example ../solid-sim/.env

# extension wallet
cp ../solid-wallet/.env.devnet.example ../solid-wallet/.env
cp ../solid-wallet/.env.localnet.example ../solid-wallet/.env
```

Use only one profile at a time. `VITE_SOLID_NETWORK` controls whether the
client identifies itself as `devnet` or `localnet`; `VITE_SOLID_RPC_URL`
controls the actual RPC endpoint.

## 3. Install The Wallet

Install the devnet wallet extension from the release link in the manifest.
Then configure it with:

- manifest URL
- artifact base URL
- indexer URL
- issuer tree address/root
- global state tree

The wallet must not generate proofs until artifacts and indexer settings are
present.

For a local operator build:

```bash
cd solid-wallet
npm install
VITE_SOLID_CLUSTER=devnet \
VITE_SOLID_NETWORK=devnet \
VITE_SOLID_RPC_URL=https://api.devnet.solana.com \
VITE_SOLID_MANIFEST_URL=http://localhost:8080/devnet.json \
VITE_SOLID_ARTIFACT_BASE_URL=http://localhost:8080/artifacts \
VITE_SOLID_INDEXER_URL=http://localhost:8787 \
npm run build
```

Then open `chrome://extensions`, enable Developer Mode, and load
`solid-wallet/dist` as an unpacked extension.

## 4. Open SolID Sim

Open the hosted app from the manifest:

```text
https://app.solidislive.com
```

Connect a devnet Solana wallet and check the status page before issuing
credentials or verifying proofs.

Required live checks:

- `schema_registry` executable
- `issuer_registry` executable
- `zk_verifier` executable
- verifier VK finalized
- artifact pins loaded
- indexer reachable
- schema catalog loaded

For a local operator run:

```bash
cd solid-sim
npm install
VITE_SOLID_CLUSTER=devnet \
VITE_SOLID_NETWORK=devnet \
VITE_SOLID_RPC_URL=https://api.devnet.solana.com \
VITE_SOLID_WS_URL=wss://api.devnet.solana.com \
VITE_SOLID_MANIFEST_URL=https://api.solidislive.com/v1/manifest \
VITE_SOLID_ARTIFACT_BASE_URL=https://artifacts.solidislive.com \
VITE_SOLID_INDEXER_URL=https://api.solidislive.com \
npm run dev
```

The current app is now `solid-sim`: one shared simulator with System, Flow,
DAO, Issuer, Wallet, Verifier, Schemas, and Logs. Use Flow to present the
DAO -> Issuer -> Wallet -> Verifier path, then use each role subsection to
perform the real action. The Wallet section can derive holder material, import
encrypted credential envelopes, validate credential integrity, and generate
proofs only when real artifacts and an indexer are configured.

## 5. Real End-To-End Flow

1. DAO/operator registers launch schemas.
2. Issuer registers from solid-sim.
3. DAO approves issuer.
4. Holder creates/imports an identity in the Wallet section, or installs the
   external wallet extension.
5. Issuer asks Wallet for holder material.
6. Issuer issues a credential to that holder key.
7. Holder imports the encrypted credential package.
8. Verifier dApp builds a requirement.
9. Wallet generates proof using hosted pinned artifacts and live Merkle proofs.
10. Verifier receives the proof and public signals.
11. Verifier submits proof on-chain.
12. Replay/nullifier reuse fails.

The current public-devnet milestone is reached when steps 9-12 work through
Wallet and Verifier using public URLs, not local files. Terminal devnet is
green; hosted browser smoke is the next acceptance gate.

For the exact injected-provider path behind step 5, see
`docs/WALLET_PROVIDER_FLOW.md`.

## Local Operator Checks

```bash
cd solid-protocol
npm run validate:devnet
npm run verify:artifacts
npm run smoke:devnet-config
```

## Public Tester Bundle

Send testers:

- console URL
- wallet extension release link
- verifier demo URL
- manifest URL
- npm package versions
- known limitations
- feedback link

## Public Tester Onboarding

Use this flow after the operator publishes a live manifest, console URL,
wallet build, artifact host, and indexer. Do not use mainnet funds.

### Tester prerequisites

- Chrome or Brave.
- Phantom or Solflare with a devnet wallet.
- A small devnet SOL balance.
- SolID Wallet extension build from the operator.
- SolID Console URL from the operator.
- Published manifest URL.

The operator should provide:

```text
Console URL:
Manifest URL:
Artifact base URL:
Indexer URL:
Wallet build/version:
Known launch schema:
Known demo issuer:
```

### Install SolID Wallet

1. Open `chrome://extensions`.
2. Enable Developer Mode.
3. Click "Load unpacked".
4. Select `solid-wallet/dist`.
5. Pin the extension.
6. Open it and create or unlock the vault.

If manual settings are required, open Settings and enter:

- Indexer URL
- Artifact Base URL
- Global State Tree
- Issuer Merkle Tree
- Issuer Tree Root

### Issuer test flow

1. Go to Issuer -> Register Issuer.
2. Enter organization name and metadata URL.
3. Click "Derive issuer identity".
4. Approve the wallet signature.
5. Submit registration and save the transaction signature.
6. Wait for DAO approval/finalization.
7. Go to Issuer -> Issue Credential.
8. Click "Pull from SolID Wallet" to read holder material.
9. Fill credential fields.
10. Issue and download/copy the encrypted credential envelope.
11. Send the envelope to the holder tester.

Expected success: issuer registration confirms, issuer appears in DAO
application or active issuer list, and the credential envelope is generated.

### DAO test flow

1. Confirm the DAO wallet has governance stake.
2. Go to DAO -> Applications.
3. Refresh pending applications.
4. Vote approve or reject.
5. After the voting period, finalize the application.
6. Save tx signatures.
7. Confirm issuer status and active issuer list update.

Expected success: vote and finalize transactions confirm, issuer status
updates, and issuer-tree root changes if enrollment occurs.

### Holder test flow

1. Unlock SolID Wallet.
2. Receive encrypted credential envelope from issuer.
3. Open SolID Wallet -> Import.
4. Paste the envelope and import.
5. Confirm the credential appears in the dashboard.
6. Open the verifier page or demo app.
7. Approve the proof request popup.
8. Wait for proof generation.

Expected success: credential import validates holder binding, issuer
signature, and commitment; proof request preview shows origin, schema,
predicates, and nonce; wallet returns proof, public signals, nullifier,
and Solana proof bytes.

### Verifier test flow

1. Open Console -> Verifier.
2. Create or load a requirement.
3. Request proof from the holder wallet.
4. Verify locally first if available.
5. Submit on-chain verification.
6. Save every proof-buffer tx signature.
7. Confirm the nullifier PDA was created.
8. Try replaying the same proof and confirm it fails.

Expected success: local Groth16 verification succeeds against the pinned
VK, on-chain proof-buffer sequence confirms, and replay is rejected.

### Bug report template

Ask testers to report:

- Role: DAO, issuer, holder, verifier.
- Browser and OS.
- Solana wallet used.
- SolID Wallet version.
- Console URL.
- Manifest URL.
- Approximate time.
- Error message or code.
- Transaction signatures.
- Screenshot or screen recording.
- Browser console logs if available.
- Whether refresh/reunlock fixed it.

Never ask testers to send private keys, seed phrases, or recovery material.
