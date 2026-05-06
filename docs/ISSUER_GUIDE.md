# Issuer Guide

Issuers publish credentials by targeting holder keys derived inside the SolID
wallet. Do not issue to guessed, pasted, or random BabyJubJub keys.

Current devnet smoke schema: `basic_identity_v2`
(`6b5014bf611a025a4693b196a517ece9f2d0672672a2eb38d7a50481474e6823`).
Use it only for operator smoke tests until the full launch schema set is
registered and published in `deployments/devnet.json`.

## Required Inputs

- approved issuer authority on devnet
- issuer BabyJubJub signing key
- schema hash
- schema name and version
- schema credential tree address
- holder BJJ public key from wallet
- eight field values matching the schema

## Holder Key Request

In an issuer web app:

```ts
const holderKey = await window.solid.getHolderPublicKey(schemaHash);
```

The issuer then uses `holderKey.x` and `holderKey.y` in the credential
issuance request.

The real path is browser page -> wallet content script -> extension service
worker -> wallet proof engine. The holder private key is derived inside the
wallet from the unlocked identity seed and never leaves the extension. See
`docs/WALLET_PROVIDER_FLOW.md` for the exact files and message names.

## Issue From SDK

```ts
import { issueCredential } from "@solid-protocol/issuer";

const issued = await issueCredential(
  issuerBjjPrivateKey,
  issuerBjjPublicKeyX,
  issuerBjjPublicKeyY,
  {
    schemaHash,
    attestationData,
    holderPubKeyX,
    holderPubKeyY,
    expirationTimestamp: 0,
  },
  {
    connection,
    issuerAuthority,
    merkleTree,
    schemaName: "basic_identity_v2",
    schemaVersion: 2,
  },
);
```

## Deliver To Wallet

Credential delivery should use the channel package:

```ts
import { encryptCredential } from "@solid-protocol/channel";
```

The issuer encrypts the credential bundle to the wallet channel public key.
The wallet imports only the encrypted envelope.

## Devnet Safety Rules

- Register and approve the issuer before issuance.
- Use the registered schema hash and tree address.
- Use wallet-derived holder keys only.
- Keep issuer BJJ private key outside the browser when possible.
- Record every issuance transaction signature for feedback/debugging.

See `examples/issuer-cli` for a minimal CLI shape.
