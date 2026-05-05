# SolID Devnet Quickstart

This guide is for public devnet testers and integrators. It assumes the
protocol has already been deployed and `deployments/devnet.json` has been
published as the canonical manifest.

## 1. Read The Manifest

The manifest is the source of truth for:

- Solana RPC
- program IDs
- artifact URLs and SHA-256 pins
- schema catalog
- tree addresses
- indexer URL
- console and wallet release URLs

Local file:

```bash
cat deployments/devnet.json
```

Hosted file:

```bash
curl https://your-domain.example/solid/devnet.json
```

## 2. Install SDK Packages

```bash
npm install @solid-protocol/sdk @solid-protocol/verifier @solid-protocol/channel
```

Use published npm versions for external apps. Local `file:` dependencies are
only for monorepo development.

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

## 4. Open The Console

Open the console URL from the manifest. Connect a devnet Solana wallet and
check the status page before issuing credentials or verifying proofs.

Required live checks:

- `schema_registry` executable
- `issuer_registry` executable
- `zk_verifier` executable
- verifier VK finalized
- artifact pins loaded
- indexer reachable
- schema catalog loaded

## 5. Real End-To-End Flow

1. DAO/operator registers launch schemas.
2. Issuer registers from console.
3. DAO approves issuer.
4. Holder installs wallet.
5. Issuer asks wallet for `window.solid.getHolderPublicKey(schemaHash)`.
6. Issuer issues a credential to that holder key.
7. Holder imports the encrypted credential package.
8. Verifier dApp builds a requirement.
9. Wallet receives `window.solid.requestProof(envelope)`.
10. Holder approves.
11. Wallet generates proof using hosted pinned artifacts and live Merkle proofs.
12. Verifier submits proof on-chain.
13. Replay/nullifier reuse fails.

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
