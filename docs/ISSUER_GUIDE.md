# Issuer Guide

Issuers publish credentials by targeting holder keys derived inside the SolID
wallet. Do not issue to guessed, pasted, or random BabyJubJub keys.

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
    schemaName: "basic_identity_v1",
    schemaVersion: 1,
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
