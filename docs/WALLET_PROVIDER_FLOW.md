# Wallet Provider Flow

This document explains exactly where `window.solid.getHolderPublicKey(schemaHash)`
lives and how an issuer app uses it without guessing holder keys. For local
devnet testing, `solid-sim` also provides an integrated Wallet section that
derives the same public holder material without requiring the browser extension.

## Runtime Path

1. A web issuer opens in the browser and calls:

   ```ts
   const holder = await window.solid.getHolderPublicKey(schemaHash);
   ```

2. The SolID Wallet content script injects `window.solid` into the page.
   Source: `solid-wallet/src/content/inject.ts`.

3. The injected provider posts a `SOLID_GET_HOLDER_PUBLIC_KEY` browser message
   with the schema hash.

4. The content script relays that request to the extension service worker as
   `GET_HOLDER_PUBLIC_KEY`.
   Source: `solid-wallet/src/content/inject.ts`.

5. The wallet service worker handles `GET_HOLDER_PUBLIC_KEY` and calls
   `getHolderPublicKey(schemaHash)`.
   Source: `solid-wallet/src/background/service-worker.ts`.

6. The proof engine unlocks the holder identity seed and derives the
   schema-specific BabyJubJub key with:

   ```ts
   deriveCredentialKey(identitySeed, schemaHash)
   ```

   Source: `solid-wallet/src/background/proof-engine.ts`.

7. The service worker returns `{ x, y }` to the page. The issuer uses those
   coordinates as the credential holder public key.

## Issuer-Side Usage

Issuers need two wallet-derived holder values:

- `window.solid.getChannelPublicKey()` for encrypted delivery.
- `window.solid.getHolderPublicKey(schemaHash)` for the schema-specific
  BabyJubJub holder key used inside the credential commitment.

In solid-sim this is wrapped by:

```ts
const material = await requestHolderMaterial(schemaHash);
```

Source: `solid-sim/src/lib/solid-wallet-provider.ts`.

That helper returns:

```ts
{
  channelPublicKey,
  holderPublicKeyX,
  holderPublicKeyY,
}
```

`IssueCredential` then signs the credential commitment and encrypts the
credential envelope for the holder wallet. If `window.solid` is not present,
the same helper falls back to `solid-sim/src/lib/sim-wallet.ts`, which
uses the integrated Wallet simulator seed.

## Security Rules

- The issuer must pass the registered schema hash, not a schema name.
- The wallet derives a different holder key per schema hash.
- The holder private key never leaves the wallet.
- The issuer receives only public coordinates.
- The encrypted channel public key is separate from the BabyJubJub holder key.
- If the external wallet is locked, missing, or cannot derive the key, the issuer
  UI may use the integrated Wallet simulator. Manual paste remains explicit.

## Devnet Test

1. Open `solid-sim` locally or at `https://app.solidislive.com`.
2. Go to Wallet and create or import a simulator identity.
3. Derive material for the active schema, or load `solid-wallet/dist` as an
   unpacked browser extension to test the injected provider path.
4. Go to Issuer -> Issue Credential.
5. Select a schema from the devnet manifest catalog.
6. Click "Pull from Wallet".
7. solid-sim should fill:
   - channel public key
   - holder BJJ public key X
   - holder BJJ public key Y

Local operator build commands are in `docs/DEVNET_QUICKSTART.md`.

Public tester builds must use public HTTPS manifest/artifact/indexer URLs.
The current protocol deploy, hosted artifacts, Merkle proof indexer, and
devnet verifier transaction are live. Broader tester traffic still needs the
hosted browser smoke pass.

If those fields do not populate, inspect the wallet service worker logs and the
browser console for `SOLID_GET_HOLDER_PUBLIC_KEY` or `GET_HOLDER_PUBLIC_KEY`.
