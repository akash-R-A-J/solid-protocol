# @solid-protocol/channel

Secure ECIES credential delivery for SolID Protocol. The issuer encrypts a
credential bundle to the holder's channel public key (X25519); the holder
decrypts it locally using a key derived from their wallet signature. The
indexer relays the encrypted envelope; it never sees the cleartext credential
fields.

This package is the privacy boundary between issuer and holder. It is not
optional — encrypted envelopes are the only way the issuer can deliver a
credential without trusting any intermediary.

## Install

```bash
npm install @solid-protocol/channel
```

## Minimal example

### Issuer side — encrypt the credential bundle

```ts
import { encryptCredential, serializeEnvelope } from "@solid-protocol/channel";

const envelope = encryptCredential(credentialBundle, holderChannelPublicKey);
const envelopeJson = serializeEnvelope(envelope);

// Post `envelopeJson` to the indexer's credential-request inbox.
```

### Holder side — derive channel key, decrypt envelope

```ts
import {
  deriveChannelKeyFromWallet,
  decryptCredential,
  parseEnvelope,
  CHANNEL_DERIVATION_MESSAGE,
} from "@solid-protocol/channel";

const channelKeys = await deriveChannelKeyFromWallet(wallet.signMessage);
// channelKeys.publicKey is the value the holder gives the issuer during a
// credential request; channelKeys.secretKey stays on the holder's device.

const envelope = parseEnvelope(envelopeJson);
const credentialBundle = decryptCredential(envelope, channelKeys.secretKey);
```

## Key derivation

Three derivation paths are exposed:

- `deriveChannelKeyFromWallet(signMessage)` — production path. The wallet
  signs the canonical `CHANNEL_DERIVATION_MESSAGE` and the result becomes the
  X25519 seed. Same wallet → same channel key, always.
- `deriveChannelKeyFromSeed(seed)` — when the holder has already derived a
  seed deterministically (testing, ad-hoc holders).
- `deriveChannelKeyFromMasterSeed(masterSeed)` — keystore-style master-seed
  derivation for multi-credential holder wallets.

## Public surface

| Function | Purpose |
| --- | --- |
| `deriveChannelKeyFromWallet`, `deriveChannelKeyFromSeed`, `deriveChannelKeyFromMasterSeed` | X25519 channel keypair derivation. |
| `encryptCredential` (issuer) | Encrypt a credential bundle to a holder's channel public key. |
| `decryptCredential` (holder) | Decrypt an envelope using the holder's channel secret key. |
| `serializeEnvelope`, `parseEnvelope` | JSON serialization for envelopes (what the indexer stores and transports). |
| `CHANNEL_DERIVATION_MESSAGE` | Constant signed by the wallet during derivation; clients should display it before signing. |

## Why this lives in its own package

- It's a privacy primitive used by both `issuer` and `holder` callers, so
  factoring it out avoids importing the full SDK on either side.
- It carries an ECIES + curve25519-js dependency footprint that should not
  bleed into `@solid-protocol/core`'s WASM-only contract.
- The derivation message is a public-facing constant that wallet UIs surface
  to users at signing time; keeping it in one place avoids drift.

## Related packages

- `@solid-protocol/holder` — uses `decryptCredential` to import the envelope before proving.
- `@solid-protocol/issuer` — uses `encryptCredential` to seal the credential bundle.
- `@solid-protocol/sdk` — unified facade.

## License

MIT
