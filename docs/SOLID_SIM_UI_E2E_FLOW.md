# SolID Sim UI E2E Flow

This is the end-to-end flow for testing `solid-sim` locally against the deployed devnet programs.

Current intended setup:

- Programs run on devnet: `schema_registry`, `issuer_registry`, `zk_verifier`.
- UI runs locally: `solid-sim` on Vite.
- Indexer/API runs locally: `solid-protocol/indexer` on `http://127.0.0.1:8787`.
- Proof artifacts are served by `solid-sim` from `/artifacts`.
- Wallet roles are separate even if you test them in one browser.

## Mental Model

There are four actors:

| Actor | Wallet | What it does | What it needs |
| --- | --- | --- | --- |
| DAO voter | External Solana wallet, usually Phantom | Stakes governance tokens, votes on issuer applications, finalizes approvals, grants schema permissions | Devnet SOL for fees, governance tokens, governance tokens staked in `DAO -> Governance Stake` |
| Issuer | External Solana wallet, usually Phantom | Registers as issuer, escrows issuer SOL stake, requests schema permission, issues credentials | More than `1 SOL` on devnet for community-tier registration, plus fees |
| Holder | Embedded wallet inside `solid-sim` | Requests credentials, stores encrypted credentials, reviews dapp proof requests, generates proofs | No external wallet required for normal holder flow |
| Verifier / dapp | External Solana wallet for on-chain verify | Creates access/proof requests, verifies holder proof locally or on-chain | Devnet SOL only when submitting on-chain verification |

Important distinction:

- Issuer SOL stake is paid during issuer registration. Current community tier requires `1 SOL` because the deployed registry config has a `1 SOL` minimum stake and community uses a `1x` multiplier.
- DAO governance-token stake is separate. It gives voting power for DAO approval and finalization.

## Local Services Required

### 1. Run the local indexer/API

From the repo root:

```bash
cd /Users/rajakash/Desktop/testing/solid-protocol/indexer
npm install
SOLID_MANIFEST_PATH=../deployments/devnet.json \
SOLID_INDEXER_STORE_PATH=../.solid-indexer/state.json \
SOLID_RPC_URL="https://devnet.helius-rpc.com/?api-key=<YOUR_HELIUS_KEY>" \
SOLID_INDEXER_WRITE_TOKEN="local-dev-token" \
npm start
```

Expected result:

```text
SolID indexer listening on http://127.0.0.1:8787
```

Check:

```bash
curl http://127.0.0.1:8787/v1/health
```

What it does:

- Stores credential requests.
- Stores verifier proof requests.
- Stores indexed credential leaves.
- Serves real Merkle proofs for holder proof generation.

If this is not running, holder request delivery and proof generation will not work naturally.

### 2. Run `solid-sim`

`solid-sim/.env.local` should have:

```bash
VITE_SOLID_NETWORK=devnet
VITE_SOLID_CLUSTER=devnet
VITE_SOLID_RPC_URL="https://devnet.helius-rpc.com/?api-key=<YOUR_HELIUS_KEY>"
VITE_SOLID_WS_URL="wss://devnet.helius-rpc.com/?api-key=<YOUR_HELIUS_KEY>"
VITE_SOLID_ARTIFACT_BASE_URL=/artifacts
VITE_SOLID_INDEXER_URL=http://127.0.0.1:8787
VITE_SOLID_CONSOLE_URL=http://localhost:5173
VITE_SOLID_EXPLORER_CLUSTER=devnet
```

Then run:

```bash
cd /Users/rajakash/Desktop/testing/solid-sim
npm install
npm run dev
```

Expected result:

```text
Local: http://localhost:5173
```

## Wallet Setup Before Testing

You need at least two external Solana accounts:

1. DAO wallet
2. Issuer wallet

You can use Phantom and switch accounts, or use two browser profiles. In one browser, Phantom can only expose the currently selected account, so switch accounts before each role action.

### DAO wallet requirements

The DAO wallet needs:

- Devnet SOL for transaction fees.
- Governance tokens.
- Governance tokens staked in `DAO -> Governance Stake`.

Holding governance tokens is not enough. Voting reads the on-chain staker account, not just the token wallet balance.

### Issuer wallet requirements

The issuer wallet needs:

- More than `1 SOL` on devnet for community registration.
- Extra SOL for transaction fees and rent.

If the issuer wallet has less than this, registration should now fail before submit with an `INSUFFICIENT_SOL` message.

## Full UI E2E Flow

### Step 0. System health check

Go to:

```text
System -> Overview
```

Required:

- RPC is devnet.
- Programs are deployed.
- Artifact base URL is `/artifacts`.
- Indexer URL is `http://127.0.0.1:8787`.

What happens:

- The UI reads the manifest.
- The UI checks devnet program accounts.
- The UI checks the local indexer.

Expected result:

- No blank pages.
- No missing program warnings.
- Indexer is reachable.

### Step 1. Register issuer

Switch Phantom to the issuer wallet.

Go to:

```text
Issuer -> Register
```

Required:

- Issuer wallet connected.
- Issuer wallet has more than `1 SOL`.
- Metadata URI is a valid URL and no more than 128 bytes.
- Organization name is no more than 64 bytes.

Actions:

1. Fill organization name.
2. Fill metadata URI.
3. Select trust tier, usually `Community`.
4. Click `Derive issuer identity`.
5. Approve the wallet message signature.
6. Confirm the UI shows:
   - Issuer PDA
   - Required stake
   - Subgroup VK finalized
   - Registry authority
7. Click `Register issuer on-chain`.
8. Approve the transaction in Phantom.

What happens:

- The wallet signature deterministically derives the issuer BabyJubJub identity.
- The browser generates the subgroup Groth16 proof locally.
- The UI builds and sends `register_issuer`.
- The issuer registry creates an `IssuerAccount`.
- The issuer wallet escrows the issuer SOL stake.

Expected result:

- Registration succeeds.
- The issuer appears in `DAO -> Applications` as pending.

If it fails:

- `INSUFFICIENT_SOL` means the issuer wallet needs more devnet SOL.
- `USER_REJECTED` means Phantom rejected the signature or transaction.
- `already registered` means this wallet already has an issuer PDA.

### Step 2. Stake DAO governance tokens

Switch Phantom to the DAO wallet.

Go to:

```text
DAO -> Governance Stake
```

Required:

- DAO wallet connected.
- DAO wallet has governance tokens.
- DAO wallet has devnet SOL for fees.

Actions:

1. Enter a governance token stake amount.
2. Click `Stake Governance Tokens`.
3. Approve the transaction.

What happens:

- The UI creates the DAO wallet associated token account if needed.
- The issuer registry moves governance tokens into the governance vault.
- The registry creates or updates the DAO wallet `StakerAccount`.
- DAO voting power becomes non-zero.

Expected result:

- `DAO -> Applications` no longer says the connected DAO wallet has zero governance stake.
- Voting buttons are enabled.

### Step 3. Approve issuer application

Keep Phantom on the DAO wallet.

Go to:

```text
DAO -> Applications
```

Required:

- DAO wallet has staked governance tokens.
- The issuer application is pending.
- Voting period rules allow vote/finalize.

Actions:

1. Click `Refresh`.
2. Find the pending issuer.
3. Click approve/vote.
4. Approve the transaction.
5. Wait until the voting period is complete if finalization is blocked by time.
6. Click `Finalize`.
7. Approve the transaction.

What happens:

- DAO vote is recorded on-chain.
- Finalization evaluates the vote result.
- If approved, the issuer becomes active/trusted.

Expected result:

- Issuer moves from pending to active.
- Issuer appears in `DAO -> Active Issuers`.

### Step 4. Issuer requests schema permission

Switch Phantom to the issuer wallet.

Go to:

```text
Issuer -> Request Schema
```

Required:

- Issuer is approved/finalized.
- Local indexer is running.
- The schema exists in the manifest catalog, or a custom schema proposal has already been registered.

Actions:

1. Select the schema the issuer wants to issue, for example `basic_identity_v2`.
2. Write a reason for the DAO.
3. Click `Request DAO permission`.

What happens:

- The issuer creates a schema permission request in the local indexer.
- No on-chain permission is granted yet.
- The DAO can now review the request from `DAO -> Schema Permissions`.

Expected result:

- The request appears in the issuer's request list.
- The DAO sees it in the schema request inbox.

#### Optional: issuer proposes a custom schema

Use this when the issuer needs a schema that is not already in the manifest catalog.

Stay on:

```text
Issuer -> Request Schema
```

Required:

- Issuer is approved/finalized.
- Local indexer is running.
- Schema has 1-8 fields. Field names must be lowercase snake_case and each field must fit the on-chain length limits.

Actions:

1. Fill `Propose custom schema`.
2. Enter schema name, display name, version, category, and fields.
3. Click `Propose custom schema`.
4. Switch Phantom to the registry authority wallet.
5. Go to `DAO -> Schema Permissions`.
6. In `Custom schema proposals`, click `Register`.
7. Approve the schema registration transaction.
8. Approve the credential tree creation transaction.
9. Approve the tree authority/binding transaction.
10. Refresh `Issuer -> Request Schema`.
11. Select the newly registered custom schema and request DAO permission.

What happens:

- The issuer proposal is stored in the local indexer.
- Registry authority registers the `SchemaAccount` on-chain.
- The console creates an SPL Account Compression tree for that schema.
- The console transfers tree authority to the protocol PDA and initializes `SchemaTreeBinding`.
- The registered custom schema becomes selectable in schema dropdowns, with a real tree address.

Expected result:

- `System -> Schemas` shows the custom schema with source `custom`.
- `DAO -> Schema Permissions` can grant issuer permission for the custom schema.
- `Issuer -> Issue Credential` can issue against the custom schema without pasting a tree address manually.

### Step 5. DAO grants schema permission

Keep Phantom on the DAO wallet.

Go to:

```text
DAO -> Schema Permissions
```

Required:

- Issuer is active.
- Schema exists in the manifest/on-chain catalog.
- Connected wallet is the registry authority shown in `DAO -> Schema Permissions`.

Current program note:

- Issuer approval voting uses DAO governance-token stake.
- Schema permission grants are still gated by `registry_config.authority`, not by any DAO voter.
- If your connected wallet can vote but is not the registry authority, the approve button stays disabled.

Actions:

1. Switch Phantom to the registry authority wallet.
2. Find the issuer schema request.
3. Click approve.
4. Approve the transaction.

What happens:

- The registry creates an issuer-schema permission PDA.
- The issuer can now issue credentials for that schema.
- The schema permission request is marked approved in the local indexer.

Expected result:

- Permission appears active in the permissions table.
- Holder request flow can now list that issuer for the same schema.

### Step 6. Create holder embedded wallet

No Phantom switch required.

Go to:

```text
Wallet -> Wallet Vault
```

Required:

- Browser localStorage available.

Actions:

1. Click create/import wallet if no holder wallet exists.
2. Confirm holder wallet material is visible.

What happens:

- `solid-sim` creates a local embedded holder seed.
- Holder credential keys are derived locally.
- Holder credentials are stored encrypted at rest in browser localStorage.

Expected result:

- Wallet vault shows ready state.
- No external Solana wallet is required for holder custody.

### Step 7. Holder requests credential

Go to:

```text
Wallet -> Request Credential
```

Required:

- Holder embedded wallet exists.
- Active issuer exists.
- Issuer has schema permission.
- Local indexer is running.

Actions:

1. Select issuer.
2. Select schema.
3. Derive/use holder material.
4. Submit credential request.

What happens:

- Holder request is written to the local indexer.
- Request contains the holder channel public key and schema request data.
- Issuer can see it in the request inbox.

Expected result:

- Request is created.
- Issuer can see it in `Issuer -> Request Inbox`.

### Step 8. Issuer issues credential from request

Switch Phantom to the issuer wallet.

Go to:

```text
Issuer -> Request Inbox
```

Required:

- Issuer wallet connected.
- Issuer is active.
- Issuer has schema permission.
- The holder request exists in the local indexer.
- Local indexer is running.

Actions:

1. Click refresh.
2. Find the holder request.
3. Review the request.
4. Issue the credential from the request.
5. Approve the issuance transaction.

What happens:

- Issuer signs the credential data.
- The credential commitment is inserted into the schema/global tree.
- The local indexer stores the issued envelope and indexed leaf.
- The request status changes to `issued`.

Expected result:

- Request shows issued.
- Issued credential appears in `Issuer -> Issued Log`.
- Holder can receive it without manual copy/paste.

### Step 9. Holder receives credential automatically

Go to:

```text
Wallet -> Request Credential
```

or:

```text
Wallet -> Wallet Vault
```

Required:

- Same browser holder wallet that made the request.
- Local indexer still has the issued request record.

Actions:

1. Refresh holder request state.
2. Open wallet vault if needed.

What happens:

- The UI sees issued credential requests with encrypted envelopes.
- The holder wallet decrypts/imports the credential.
- The credential is stored encrypted at rest.

Expected result:

- Credential appears in wallet vault.
- No manual envelope copy/paste is needed in the happy path.

### Step 10. Dapp/verifier requests access proof

Switch Phantom to verifier wallet if you plan to verify on-chain. For local-only verification, external wallet is not needed until submit.

Go to:

```text
Verifier -> Query Builder
```

Required:

- Schema exists.
- Holder has a matching credential.
- Local indexer is running.

Actions:

1. Select schema.
2. Set dapp/verifier name, for example `Demo DeFi Pool`.
3. Set action, for example `Request access`.
4. Add predicate, for example `age >= 18`.
5. Click `Request access from holder`.

What happens:

- Verifier creates a proof request in the local indexer.
- The proof request describes the exact dapp, action, schema, and predicate.

Expected result:

- Proof request appears in the holder wallet.

### Step 11. Holder reviews proof request and generates proof

Go to:

```text
Wallet -> Wallet Vault
```

Required:

- Holder wallet has matching credential.
- Proof request exists.
- Local indexer can serve Merkle proof for the credential commitment.
- Proof artifacts are available from `/artifacts`.

Actions:

1. Click refresh requests.
2. Find the dapp access request.
3. Click `Approve proof`.
4. Review the modal carefully:
   - Dapp/verifier name
   - Action
   - Schema
   - Predicate
   - Credential being used
   - Privacy note
5. Click approve/generate proof.

What happens:

- Holder explicitly approves the proof purpose.
- Browser loads pinned Groth16 artifacts.
- Browser fetches real Merkle paths from the local indexer.
- Browser generates the proof locally.
- Proof result is written back to the proof request record.

Expected result:

- Proof request status becomes `proof_ready`.
- Holder sees proof generated and returned to verifier.

### Step 12. Verifier loads and verifies proof

Switch Phantom to verifier wallet if doing on-chain verification.

Go to:

```text
Verifier -> Verify Proof
```

Required for local verification:

- Proof request status is `proof_ready`.
- Proof JSON and public signals exist.
- Verification key artifact is available.

Required for on-chain verification:

- Verifier wallet connected.
- Verifier wallet has devnet SOL for fees.
- `zk_verifier` is deployed and configured.

Actions:

1. Refresh proof request inbox.
2. Click `Load proof`.
3. Choose `Local` verification first.
4. Click verify.
5. If local verification passes, choose `On-chain Solana`.
6. Click verify.
7. Approve the transaction.

What happens:

- Local verification uses `snarkjs` and the pinned verification key.
- On-chain verification submits to `zk_verifier`.
- `zk_verifier` creates a nullifier PDA to prevent replay.
- Proof request is marked verified.

Expected result:

- Local verification passes.
- On-chain verification passes.
- Reusing the same proof/nullifier should be rejected by replay protection.

### Step 13. Check history and logs

Go to:

```text
Verifier -> History
System -> Logs
System -> Protocol Flow
```

Expected result:

- Verification event appears.
- Protocol flow shows DAO trust, issuer credential, holder credential, verifier proof.
- Logs show recent request/issuance/proof events.

## Expected Happy Path Summary

1. Issuer wallet registers and escrows SOL.
2. DAO wallet stakes governance tokens.
3. DAO approves and finalizes issuer.
4. Issuer requests schema permission, or proposes/registers a custom schema first.
5. DAO grants schema permission.
6. Holder creates embedded wallet.
7. Holder requests credential.
8. Issuer issues credential from request.
9. Holder receives credential automatically.
10. Verifier requests access proof.
11. Holder reviews proof modal and generates proof.
12. Verifier loads proof and verifies locally/on-chain.
13. App grants access based on verification success.

## Common Failure Causes

| Symptom | Cause | Fix |
| --- | --- | --- |
| `INSUFFICIENT_SOL` during issuer registration | Issuer wallet does not have enough SOL for escrow and fees | Fund issuer wallet with more than `1 SOL` |
| DAO cannot vote | DAO wallet has zero staked governance tokens | Go to `DAO -> Governance Stake` and stake governance tokens |
| Issuer not visible in request flow | Issuer is not finalized active or lacks schema permission | Approve/finalize issuer, then grant schema permission |
| Custom schema is not visible in dropdowns | Proposal is not registered yet, or the local indexer is stale | Register it from `DAO -> Schema Permissions`, then refresh |
| Custom schema registration fails during tree creation | Registry authority wallet lacks enough SOL for schema account, tree rent, and fees | Fund the registry authority wallet and retry with a new proposal if the partial tx state is unclear |
| Issuer cannot find where to ask for schema permission | Old UI did not expose the request step | Use `Issuer -> Request Schema`, then approve it from `DAO -> Schema Permissions` |
| Holder request does not appear for issuer | Local indexer is not running or UI points to wrong indexer URL | Start indexer and set `VITE_SOLID_INDEXER_URL=http://127.0.0.1:8787` |
| Holder cannot generate proof | Credential leaf is not indexed or artifacts are unavailable | Keep indexer running, ensure issuance indexed the leaf, ensure `/artifacts` works |
| Verifier cannot load proof | Holder has not approved/generated proof yet | Go to wallet vault and approve the proof request |
| On-chain verification fails with replay/nullifier error | Same proof was already used | Generate a fresh proof/request |

## What Is Persisted

| Data | Storage | Notes |
| --- | --- | --- |
| Issuer registration draft | Browser localStorage, keyed by issuer wallet | Keeps org name, metadata URI, tier, derived BJJ public key |
| Holder wallet seed | Browser localStorage | Local sim wallet only |
| Holder credentials | Browser localStorage, encrypted with AES-GCM | Credentials are encrypted at rest |
| Credential requests | Local indexer file store | Survives UI refresh if indexer store path remains the same |
| Custom schema proposals | Local indexer file store | Issuer proposes; registry authority registers schema and tree on-chain |
| Schema permission requests | Local indexer file store | Issuer asks; DAO approves on-chain and marks request approved |
| Proof requests | Local indexer file store | Verifier and wallet share this API-backed flow |
| On-chain issuer/schema/proof state | Devnet programs | Survives browser refresh and local service restart |

## Minimum Wallet Balances

For devnet testing:

- Issuer wallet: more than `1 SOL`.
- DAO wallet: enough SOL for transaction fees, plus governance tokens staked.
- Verifier wallet: enough SOL for on-chain verification fees.
- Holder embedded wallet: no SOL required for normal local holder actions.

## Restart Checklist

If the UI gets stale or you switch wallets:

1. Restart indexer if it was stopped.
2. Restart `solid-sim` after changing `.env.local`.
3. Refresh the browser.
4. Confirm Phantom is on the correct account for the current role.
5. Check `System -> Overview`.
6. Continue from the step where the state actually lives:
   - On-chain state: DAO/issuer/schema/proof transactions.
   - Indexer state: credential requests, proof requests, indexed leaves.
   - Browser state: holder wallet and encrypted credentials.
