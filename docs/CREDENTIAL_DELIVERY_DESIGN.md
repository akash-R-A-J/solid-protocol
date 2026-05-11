# Credential Delivery Design

Status: Implemented (channel package + wallet/console wiring landed
2026-05-04). The original 2026-05-02 design specified ChaCha20-Poly1305
with HKDF-from-signature key derivation; the implementation that landed
in `ts-sdk/packages/channel` uses NaCl `box` (X25519 + XSalsa20-Poly1305)
with SHA-256-based seed derivation. This doc has been reconciled to
match the implementation; the wire format here is the canonical contract.
The cryptographic guarantees (authenticated encryption, forward secrecy
via per-envelope ephemeral keypairs, AAD-equivalent binding via the
ECIES recipient pubkey, replay resistance) are unchanged from the
original spec; only the primitives differ.

This document specifies how an issuer transfers a freshly-issued
credential to its intended holder. It covers the canonical wire format,
the three supported delivery channels, security analysis, and
open questions.

The audience for this document is two readers: (1) the SolID team
implementing the issuer SDK, the holder app, and the claim-link
service; (2) external integrators who want to issue credentials into
SolID using their own tooling and need to know the wire format.

## What this is not

This document does **not** specify:

- The on-chain `issue_credential` instruction itself -- that lives in
  `programs/issuer-registry/src/lib.rs` and is described in
  `docs/ARCHITECTURE.md`.
- Holder-side storage -- see `docs/HOLDER_STORAGE_AND_WALLET.md`.
- Verification semantics -- see `plan/VERIFIER_SDK_SHAPE.md` and
  `docs/integration-guide.md`.
- Revocation mechanics -- see `docs/REVOCATION_DESIGN.md`.

This document is exclusively about the bytes that move from the
issuer to the holder, after the on-chain `issue_credential` succeeds.

## Design principles

1. **The issuer never sees the holder's master BJJ key.** The
   credential is committed against a derived per-schema subkey. The
   delivery package contains the issuer's signature over a public
   commitment + the holder-decryptable field data; the master key is
   never on the wire.
2. **The package is self-describing.** A holder app can decrypt and
   import a package without consulting any external service to know
   the schema, the issuer, or the field semantics. Once the package
   is decrypted, the holder has everything needed to generate a proof.
3. **The default delivery channel does not require infrastructure
   from the issuer.** A single static-host claim link is enough to
   onboard the first 100 issuers. Issuers should not have to run a
   server.
4. **Channels degrade gracefully.** If a wallet does not support the
   Wallet Standard credential handoff, the same package falls back
   to a downloadable JSON the user manually imports.
5. **Replay-resistant by design.** A delivery package is bound to a
   specific holder pubkey. Reusing a package against a different
   wallet does not produce a usable credential.

## Canonical wire format

Two TypeScript types in `ts-sdk/packages/channel/src/types.ts` are the
source of truth: `CredentialBundle` (the cleartext payload) and
`EncryptedEnvelope` (the wire-safe wrapper). The bundle is JSON-encoded,
encrypted via NaCl `box`, and packaged into the envelope; the envelope
is what crosses any transport.

### EncryptedEnvelope (the wire shape)

```ts
type EncryptedEnvelope = {
  version: 1;
  ephemeralPublicKey: string;       // base64; 32 bytes; X25519
  nonce: string;                    // base64; 24 bytes
  ciphertext: string;               // base64; nacl.box(JSON(bundle), ...)
};
```

The envelope is self-contained. Any transport (HTTPS, QR, email, file
download, Dialect DM, Wallet-Standard handoff) can carry it. The
recipient uses the issuer's ephemeral public key together with their
own X25519 secret key to ECDH a shared secret, which decrypts the
ciphertext via XSalsa20-Poly1305.

### CredentialBundle (the cleartext payload)

```ts
type CredentialBundle = {
  version: number;

  // Schema binding.
  schemaHash: string;                  // 32-byte hex
  schemaName: string;                  // human-readable, e.g. "basic_identity_v1"

  // Attestation field values, decimal strings of BN254 field elements.
  attestationData: string[];

  // BJJ EdDSA signature by the issuer over the credential commitment.
  issuerSignature: { R8x: string; R8y: string; S: string };

  // Issuer's BabyJubJub public key (decimal strings, circomlib-native form).
  issuerPublicKey: { x: string; y: string };

  // Holder's BabyJubJub public key, bound to this credential.
  // The holder's *X25519 channel* public key (which encrypts the envelope)
  // is a separate key derived via deriveChannelKeyFromWallet or
  // deriveChannelKeyFromMasterSeed; it is not stored in the bundle.
  holderPublicKey: { x: string; y: string };

  // Commitment + tree position.
  salt: string;                        // decimal string
  commitment: string;                  // 32-byte hex; Poseidon
  treeAddress: string;                 // SPL Account-Compression tree, base58
  merkleProof: {
    siblings: string[];                // hex, bottom-up
    pathIndices: number[];             // 0=left, 1=right
    leafIndex: number;
  };

  // Lifecycle.
  expirationTimestamp: number;         // unix seconds; 0 = never

  // Issuer metadata for display + circuit witness.
  issuerName: string;
  issuerAuthority: string;             // Solana base58
  issuerStatusEpoch: string;           // decimal string
  issuerRevocationNonce: string;       // decimal string
};
```

Notes:

- The bundle does **not** carry any holder private key. The holder
  derives the per-schema BJJ private key from their master seed at
  proof generation time via `@solid-protocol/core::deriveCredentialKey`.
  Issuers never see and cannot recover the holder's master seed.
- The merkle proof is a hint captured at issuance time; production
  holder code refreshes it from a `MerkleProofAdapter` before each
  proof generation, since the tree may have grown since issuance.
- The `holderPublicKey` here is the BJJ pubkey bound to the credential
  (used as a circuit witness). This is distinct from the holder's
  X25519 channel public key, which is what the issuer used to encrypt
  the envelope. Both keys are deterministically derivable from the
  same wallet seed but live on different curves and serve different
  purposes.

### Encryption: NaCl box (X25519 + XSalsa20-Poly1305)

The implementation uses `tweetnacl.box`, which combines:

- **X25519 ECDH**: a per-envelope ephemeral keypair on the issuer side,
  combined with the holder's X25519 channel public key, derives a
  shared secret. The ephemeral public key ships in the envelope.
- **XSalsa20-Poly1305**: authenticated encryption of
  `JSON.stringify(bundle)` under that shared secret with a 24-byte
  random nonce.

Properties:

- **Forward secrecy**: a fresh ephemeral keypair per envelope means a
  long-term issuer compromise does not retroactively decrypt past
  envelopes.
- **Authentication**: tampering with the ciphertext, the nonce, or the
  ephemeral public key breaks Poly1305 verification and the holder's
  decryption fails closed.
- **Recipient binding**: the envelope can only be decrypted by the
  holder X25519 secret key paired with the public key the issuer
  encrypted to. Retargeting the envelope to a different holder is
  impossible without re-encrypting under that holder's public key,
  which requires the issuer's cooperation.

The earlier draft of this doc specified ChaCha20-Poly1305 with separate
HKDF key derivation and explicit AAD binding to schemaHash + holder
wallet. The implemented NaCl-box approach achieves equivalent security
properties more compactly: the recipient binding is enforced
cryptographically by the X25519 ECHD step (an envelope encrypted to
holder A's X25519 pubkey simply cannot be opened by holder B), and
schema binding is enforced by the holder app validating
`bundle.schemaHash` against the issuer's on-chain registration before
accepting the import.

### Validation contract

Every encrypted envelope and decrypted bundle is validated structurally
before any further work. The checks live in
`ts-sdk/packages/channel/src/validation.ts` and run automatically inside
`encryptCredential` (issuer side) and `decryptCredential` (holder side).
Any consumer that synthesises an envelope or hand-crafts a bundle MUST
satisfy these invariants -- they are part of the wire-format contract.

**EncryptedEnvelope (`assertEncryptedEnvelope`):**

- `version === 1`. No other versions are accepted by v1 holders.
- `ephemeralPublicKey`: non-empty base64 that decodes to exactly 32 bytes
  (X25519 public key length).
- `nonce`: non-empty base64 that decodes to exactly 24 bytes
  (NaCl box nonce length).
- `ciphertext`: non-empty base64 that decodes to at least 17 bytes
  (16-byte Poly1305 tag + at least one plaintext byte).

A malformed envelope fails fast with a typed error before any ECDH or
decryption work is done; this protects the holder from spending compute
on adversarial inputs.

**CredentialBundle (`assertCredentialBundle`):**

- `version === 1`.
- `schemaHash`: 32-byte hex (with or without `0x` prefix).
  **Reserved:** the all-zero value `00...00` is rejected. The on-chain
  `SchemaTreeBinding` uses a zero schema hash as the "uninitialised"
  sentinel, and accepting it in a bundle would create an ambiguous
  credential.
- `commitment`: 32-byte hex. **Reserved:** the all-zero value is rejected
  for the same reason -- a zero commitment overlaps with the SPL AC empty
  leaf representation.
- `attestationData`: array of **exactly 8** decimal-string entries
  (`NUM_FIELDS = 8`, mirrors the circuit's `NUM_FIELDS`). Each entry must
  fit in a u64 (the circuit bounds attestation values to that range
  pre-Poseidon).
- `salt`, `issuerSignature.{R8x, R8y, S}`, `issuerPublicKey.{x, y}`,
  `holderPublicKey.{x, y}`: canonical unsigned decimal strings (no
  leading zeros except for `"0"` itself, no `+`/`-` signs, no scientific
  notation), bounded by the BN254 field prime
  `21888242871839275222246405745257275088548364400416034343698204186575808495617`.
- `expirationTimestamp`: non-negative safe integer. `0` means "never
  expires"; any other value is treated as Unix seconds.
- `treeAddress`, `issuerAuthority`: base58 Solana public keys, length
  32-44 chars within the standard base58 alphabet. (The wallet decodes +
  validates the actual 32-byte length downstream via
  `new PublicKey(...)`; the channel layer just sanity-gates.)
- `merkleProof`: object with `siblings` (hex-32 array, bottom-up),
  `pathIndices` (each entry strictly `0` or `1`), `leafIndex`
  (non-negative safe integer). `siblings.length === pathIndices.length`,
  and the depth is bounded by `MAX_MERKLE_PROOF_DEPTH = 64` (a soft
  upper bound; the actual circuit uses `TREE_DEPTH = 20`).
- `issuerStatusEpoch`, `issuerRevocationNonce`, `issuerTreeLeafIndex`:
  canonical unsigned decimal strings.
- `issuerName` and `schemaName`: non-empty strings.

These checks are deliberately structural, not cryptographic. The bundle's
authenticity (issuer signature verifies, commitment matches, holder key
is in the prime-order subgroup) is the responsibility of the *holder*
package -- see "Holder-side credential integrity" in
`docs/HOLDER_STORAGE_AND_WALLET.md`. Splitting structural and
cryptographic concerns this way keeps the channel package free of WASM
dependencies and makes it auditable in isolation.

**Constants exported alongside the validators:**

- `ENVELOPE_VERSION = 1`
- `CREDENTIAL_BUNDLE_VERSION = 1`
- `NUM_FIELDS = 8`
- `X25519_PUBLIC_KEY_BYTES = 32`
- `X25519_SECRET_KEY_BYTES = 32`
- `NACL_BOX_NONCE_BYTES = 24`

Bumping any of these is a wire-format break and requires a bundle or
envelope `version` increment per the Versioning section below.

### Channel key derivation

The holder's X25519 channel keypair is derived deterministically from
their wallet. Two helpers exist in `@solid-protocol/channel`:

- `deriveChannelKeyFromWallet(signMessage)`. The wallet signs the
  fixed string `"solid:channel:v1"`, the signature is SHA-256ed into
  a 32-byte seed, and that seed becomes the X25519 secret key. Use
  this with external wallets (Phantom, Solflare, Ledger) where the
  application does not hold the underlying seed.
- `deriveChannelKeyFromMasterSeed(masterSeed)`. SHA-256 of the domain
  label `"solid:channel:v1"` concatenated with the master seed is the
  X25519 secret key. Use this with self-custodial wallets that hold
  their identity seed directly (e.g. solid-wallet's encrypted vault).

Both paths produce a `ChannelKeyPair = { publicKey: Uint8Array(32);
secretKey: Uint8Array(32) }`. The two paths are deliberately distinct;
within a single wallet the choice of derivation must be stable, since
switching invalidates every envelope previously encrypted to the old
public key.

The holder's channel public key is what the issuer must know in order
to encrypt. The holder gives this key to the issuer out-of-band (paste
into a request form, scan a QR code, or via a future proof-request
exchange). The base64 encoding (32 bytes -> 44 chars) is the canonical
display form.

## Three delivery channels

### Channel 1 -- Encrypted claim link (devnet primary)

This is the default and the one almost all devnet issuance will use.

**Flow:**

1. The issuer builds a `CredentialBundle` and calls
   `encryptCredential(bundle, holderChannelPubKey)` to produce an
   `EncryptedEnvelope`.
2. The issuer uploads the JSON-serialized envelope to a claim-link
   service. The service returns a one-time URL of the form
   `https://claim.solid.example/c/<token>` where `<token>` is a
   high-entropy random ID.
3. The issuer delivers the URL to the user out-of-band (email,
   Discord, in-product handoff). The URL is sensitive: anyone with
   the URL can fetch the envelope, but only the holder's X25519
   secret key can decrypt it.
4. The user clicks the link. The holder web app at
   `claim.solid.example` (or `holder.solid.example/claim`) prompts
   the user to connect their wallet.
5. The holder app fetches the envelope, derives the channel keypair
   from the wallet, calls `decryptCredential(envelope, channelKeys.secretKey)`
   to recover the bundle, validates the issuer signature against the
   commitment, and imports the credential into local encrypted storage.
6. The claim-link service marks the token as consumed (one-time-use)
   and deletes the envelope after import is acknowledged. The token
   is invalid for any future fetch.

**What the claim-link service has to do:**

- Accept POSTs from approved issuers (authenticated by the issuer's
  Solana key signing the upload). Store the envelope JSON keyed
  by token. Return the token URL to the issuer.
- Serve the envelope to anyone presenting the token, exactly
  once, until expiry (default 7 days).
- Run no key material. The service never sees the plaintext fields.
  The encryption is end-to-end issuer-to-holder.

**What the claim-link service does not need to do:**

- Authenticate users. Decrypting the envelope requires the holder's
  X25519 channel secret, which only that wallet holds.
- Track holder identities. The token is the only identifier.
- Be highly available. If the service is down, the issuer can
  re-upload to a fallback service or fall back to Channel 2.

This channel is the right default because:

- Issuers have nothing to deploy. After calling `issueCredential` on
  chain, they call `encryptCredential` and POST the resulting envelope
  to the claim-link service.
- Holders have nothing to install beyond the holder web app.
- The cryptographic guarantees do not depend on the claim-link
  service: a malicious operator cannot read fields, cannot mint new
  credentials (the issuer signature is required and is verified
  against the on-chain registry), and cannot retarget the envelope
  (the X25519 ECDH binds it to one specific holder pubkey).

### Channel 2 -- Downloadable JSON (fallback)

For air-gapped issuers, sensitive credentials, or when the claim-link
service is unreachable.

**Flow:**

1. Issuer generates an `EncryptedEnvelope` exactly as in Channel 1.
2. Issuer downloads it as a `.solid-credential.json` file (or copies
   the serialized envelope to clipboard).
3. Issuer transmits the file to the user via any out-of-band channel
   (encrypted email, USB, Signal, etc.).
4. User opens the holder app, pastes or selects the envelope JSON,
   the holder derives its channel keypair, the envelope decrypts,
   the credential is imported.

The JSON file is exactly the same `EncryptedEnvelope` shape as
Channel 1. Channels 1 and 2 are interchangeable from the holder
perspective: the holder accepts both via the same `parseEnvelope` +
`decryptCredential` pipeline.

This channel is **the only channel implemented today**.
`solid-sim` emits envelopes via Channel 2 (download / clipboard copy);
`solid-wallet` imports envelopes via Channel 2 (textarea paste).
Channels 1 and 3 build on top of the same envelope shape and ship
when their respective infrastructure (claim-link service, Wallet
Standard adoption) lands.

This channel is also the canonical backup format. A holder export
of an existing credential produces the same envelope shape as a fresh
issuance, modulo the per-envelope ephemeral keypair and nonce (which
are regenerated per-export).

### Channel 3 -- Wallet Standard handoff (mainnet target)

For production-grade UX where the wallet itself participates in
credential storage. Spec'd in `docs/HOLDER_STORAGE_AND_WALLET.md`.

**Flow:**

1. Issuer generates an `EncryptedEnvelope`.
2. Issuer dApp calls `wallet.features["solid:importCredential"](envelope)`
   via the Wallet Standard.
3. Wallet decrypts, validates the issuer signature, prompts the user
   to confirm, imports into wallet-managed storage.
4. No third-party claim-link service involved.

Wallets that implement this feature need to vendor the
`@solid-protocol/holder` reference implementation (or write their
own conforming code). The first wallet to ship native support is
likely a forked Phantom, Backpack, or a SolID-aware wallet.

This channel is **not a devnet blocker**. The Wallet Standard
extension proposal lives in `docs/HOLDER_STORAGE_AND_WALLET.md` and
ships when the first wallet vendor adopts.

### Channel 4 (deferred) -- SAS attestation surface

Solana Attestation Service can publish the *existence* of a SolID
credential as a Solana-native, indexable attestation -- without
revealing the field data. The pattern:

1. Issuer issues a SolID credential as usual (Channel 1, 2, or 3).
2. Issuer also publishes a SAS attestation with `data = bundle.commitment`
   and `subject = holderSolanaWallet`.
3. Indexers and explorer tools can show "this wallet holds N SolID
   credentials" by querying SAS, without ever seeing the credential
   content.

`crates/solid-core/src/sas.rs` already has the type stubs. This
channel ships when SAS finalizes its program deployment and the
SAS_PROGRAM_ID constant in `sas.rs` is updated.

This is a discoverability surface, not a delivery channel. The actual
credential bytes still flow through Channels 1-3.

## Security analysis

### What the issuer gives away

The issuer signs a commitment to the credential and encrypts the field
data. The issuer **does not** hold the holder's master BJJ key, so
the issuer cannot generate proofs on the holder's behalf.

### What the claim-link service can do

A malicious or compromised claim-link service can:

- See envelope blobs, opaque tokens, the issuer's ephemeral X25519
  pubkeys, and the URL access pattern. The envelope does NOT encode
  the holder's identity in the clear; only the issuer's ephemeral
  key is visible.
- Refuse to serve an envelope (denial-of-service against a single
  credential delivery; recoverable via Channel 2 fallback).
- Attempt to swap an envelope for a different one. The bundle inside
  is signed by the issuer (BJJ EdDSA over the commitment), and the
  holder verifies that signature against the on-chain registry before
  accepting the import; swapping fails closed.

A malicious service **cannot**:

- Decrypt field data (no access to the holder's X25519 secret key).
- Forge issuer signatures (no access to issuer BJJ private key).
- Retarget an envelope to a different holder. The X25519 ECDH binds
  the envelope to the specific holder pubkey it was encrypted to;
  decryption with any other secret key fails Poly1305 authentication.
- Mint an "uninvited" credential (the on-chain `issue_credential`
  must run independently and the bundle must reference the
  resulting tree leaf).

### What the holder gives away on import

When the holder fetches a claim link, the claim-link service learns:

- The token (which it gave out and already knew).
- The IP address and user-agent from which the import happened.

Notably, the holder's X25519 channel public key is NOT revealed by
fetching the envelope. Decryption happens entirely client-side after
the fetch, so the service has no way to correlate the envelope with
the holder unless the holder also identifies itself separately
(e.g. via wallet sign-in to the holder web app). Hosting the holder
app over Tor or a privacy-respecting CDN further reduces correlation
opportunities.

### Defenses against credential phishing

A user could be tricked into pasting a malicious claim link into a
fake holder app. Mitigations:

- The holder app domain (`holder.solid.example`) is fixed and
  documented. Users learn to verify the domain, same as any web app.
- The bundle's issuer signature is verified against the on-chain
  registry. A bundle signed by an unregistered or revoked issuer
  is rejected with a typed error.
- A future revision can extend the channel derivation to mix in the
  holder app's origin (e.g. `solid:channel:v1@holder.solid.example`),
  so that channel keys derived under a malicious origin don't decrypt
  envelopes encrypted to the canonical origin's keys.

### Replay across devices

The same encrypted package can be imported into the same holder
wallet on multiple devices. This is **intended** -- it lets a user
restore credentials onto a new device. Each device runs the same
key derivation, so the same wallet decrypts the same blob.

A package that is imported once and then deleted from the holder app
can be re-imported as long as the claim-link service has not yet
expired the token. After token expiry, the user must restore from a
backup.

### What the on-chain transaction reveals

`issue_credential` reveals:

- The issuer authority (signer).
- The credential commitment.
- The schema hash (via the schema_account binding).
- The leaf index in the schema tree (incremented per-issuance).

It does **not** reveal:

- The holder wallet (the credential is anchored to the holder via
  the encrypted package's `holder.wallet` field, but the on-chain
  ix does not name a recipient).
- The credential field values.
- Anything about future verifications.

External analytics can correlate `issue_credential` events with later
`verify_batch_proof_v2` events only via the issuer pubkey. The
holder identity is not on-chain at issuance.

## Open questions

These are decisions that need to be made before A4 (holder app) and
A5 (issuer CLI) ship. Listed for the design review.

1. **Should the claim-link service be SolID-hosted or
   issuer-hosted?** Default proposal: SolID-hosted at
   `claim.solid.example` for devnet; issuer-hosted optional via a
   `--claim-link-service` flag in the issuer CLI. This trades off
   convenience (one URL to remember) vs decentralization (SolID
   becomes a central point of failure for issuance).
2. **What happens if a holder loses their wallet seed?** The
   credentials are unrecoverable by design (the wallet derives the
   decryption key). Mitigation: the holder app exports an encrypted
   backup that the user can store in cloud storage protected by a
   passphrase the user remembers separately. Open question: should
   we provide a cloud-sync API for backups, or stay strictly local?
3. **Should the package include a one-time delivery PIN?** A PIN
   reduces the risk of a stolen claim link being used by someone
   with the same wallet. But it requires the issuer to deliver the
   PIN out-of-band, which adds friction. Default: no PIN; rely on
   wallet ownership being sufficient. Issuers with stricter
   requirements can implement out-of-band PIN gating in their own
   issuance flow.
4. **How are the schema field descriptors validated?** Today the
   schema is registered on-chain as a hash; the plaintext field
   list is off-chain. We should pin the field list in a
   schema-registry metadata account or in the SDK so the holder app
   can render the disclosure preview correctly. Decision pending.
5. **Should the package format include a `revocationListUrl` for
   indexer-free revocation checks?** Today revocation requires a
   round-trip to the registry. A signed revocation list URL lets the
   holder check status offline, with a freshness window. Useful for
   mobile holder apps; defer to v1.1.

## Implementation order

Status as of 2026-05-04:

1. **Channel 2 (download / clipboard)** -- LANDED. The
   `@solid-protocol/channel` package implements the wire format;
   `solid-sim` emits envelopes and `solid-wallet` imports them via
   pasted JSON or `.solid-credential.json` files. Cross-codebase
   contract gated by `__tests__/cross-codebase.test.ts`.
2. **Channel 1 (claim-link service)** -- DEFERRED. Requires a
   minimal upload + token + serve API and a holder claim page.
   Builds directly on the same `EncryptedEnvelope`; no wire-format
   change.
3. **Channel 3 (Wallet Standard)** -- DEFERRED until the first
   wallet vendor adopts. Not a devnet blocker.
4. **Channel 4 (SAS)** -- DEFERRED until SAS program finalizes.

## Versioning

The `version: 1` field in `EncryptedEnvelope` is the wire-format
version. Any breaking change to envelope semantics (new required
fields, removed fields, changed encryption primitive, changed
serialization) bumps to `version: 2`. Holder code must support every
shipped version indefinitely; old envelopes cannot be re-issued.

The `version` field inside `CredentialBundle` is independent and
tracks the bundle schema. The two versions move on different cadences:
the envelope version covers the transport layer; the bundle version
covers the credential payload schema.

Non-breaking additions (new optional metadata fields on the bundle,
new schema field types) do not bump the bundle version. Holder code
must tolerate unknown optional fields. Algorithm rotation (e.g. moving
to a post-quantum primitive) requires an envelope version bump and
parallel decoder support during the migration.

## Reference implementations

- `@solid-protocol/channel` is the canonical wire-format package.
  Exports: `encryptCredential`, `decryptCredential`, `serializeEnvelope`,
  `parseEnvelope`, `deriveChannelKeyFromWallet`,
  `deriveChannelKeyFromMasterSeed`, `deriveChannelKeyFromSeed`,
  `CHANNEL_DERIVATION_MESSAGE`, plus the `CredentialBundle`,
  `EncryptedEnvelope`, and `ChannelKeyPair` types.
- `solid-wallet/src/background/proof-engine.ts` consumes the channel
  package on the holder side: derives the channel keypair via
  `deriveChannelKeyFromMasterSeed(identitySeed)` and ingests envelopes
  via `parseEnvelope` + `decryptCredential`.
- `solid-sim/src/roles/issuer/pages/IssueCredential.tsx` consumes
  the channel package on the issuer side: builds a `CredentialBundle`,
  calls `encryptCredential(bundle, holderChannelPubKey)`, and exposes
  the resulting envelope via Channel 2 (file download / clipboard).
- `crates/solid-core/src/credential.rs` (TODO) is the Rust
  ground-truth implementation that the TS SDK will be byte-checked
  against via the cross-language vector suite (SOLID-SEC-010).
  The Rust side has not landed yet; until it does, the channel
  package's vitest suite is the only contract gate.

## Test surface

`ts-sdk/packages/channel/__tests__/` holds the contract gates:

- `channel.roundtrip.test.ts`: encrypt/decrypt roundtrip preserves the
  bundle; tampered ciphertexts, swapped ephemeral keys, and wrong
  recipient secrets all fail closed; JSON serialization is lossless;
  malformed envelopes (wrong version, wrong base64-decoded byte
  lengths for ephemeralPublicKey + nonce, missing fields) are rejected
  with typed errors; the all-zero schemaHash and commitment sentinels
  are rejected at encryption time; the exactly-8 `attestationData`
  invariant is enforced.
- `derive-key.test.ts`: both derivation paths are deterministic, are
  domain-separated, and are interoperable with `deriveChannelKeyFromSeed`.

Cross-codebase contract gate: `solid-wallet/__tests__/cross-codebase.test.ts`
exercises encrypt-side bundle construction -> envelope serialization ->
holder decrypt -> wallet storage shape end-to-end without React,
IndexedDB, or chrome.* APIs. Both wire-format drift and wallet-mapping
drift fail this test.

Run via `npm run test` from each package root.

---

*End of design. The wire format is the contract; the channels are
the policy.*
