# Credential Delivery Design

Status: Design. Drafted 2026-05-02 alongside the post-Phase-E rollout
planning. Implementation lands as part of Tier A (`plan/DEVNET_ROLLOUT_PUNCHLIST.md`
items A4 + A5).

This document specifies how an issuer transfers a freshly-issued
credential to its intended holder. It covers the canonical package
format, the three supported delivery channels, security analysis, and
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

## Canonical package format

A credential package is a single encrypted JSON object. The TypeScript
type is the source of truth.

```ts
type CredentialPackage = {
  version: 1;

  // Issuer metadata. All public.
  issuer: {
    authority: string;             // Solana base58 pubkey
    bjjPubkey: { x: string; y: string }; // hex; circomlib-native form
    registrySlot: number;           // slot when issuer was approved
    metadata: {
      name: string;
      website?: string;
      logoUrl?: string;
      contactEmail?: string;
    };
  };

  // Schema metadata. All public.
  schema: {
    ref: string | { name: string; version: number };
    hash: string;                   // 32-byte hex; 5-input Poseidon
    fields: SchemaFieldDescriptor[]; // for the holder UI
  };

  // Holder binding. All public.
  holder: {
    wallet: string;                 // Solana base58 pubkey of intended recipient
  };

  // Public credential commitment. Verifiable by anyone holding the
  // issuer pubkey, schema hash, and the encrypted blob's plaintext.
  commitment: string;               // 32-byte hex; 5-input Poseidon
  signature: {                      // BJJ EdDSA over (commitment || schemaHash)
    R8x: string;
    R8y: string;
    S: string;
  };

  // Tree position. Used by the holder to build Merkle proofs.
  treeLocation: {
    schemaTreePubkey: string;       // SPL AC tree
    leafIndex: number;
    expectedRoot: string;           // 32-byte hex; the root at issuance time
  };

  // Lifecycle metadata. Public.
  expiresAt: number | null;          // unix seconds; null = never
  revocationNonce: number;           // current value when issued

  // Encrypted field data. Decryptable only by the intended holder.
  encryptedFields: {
    algorithm: "chacha20-poly1305";
    keyDerivation: {
      method: "wallet-signature";
      domain: "SolID-credential-key-v1";
      nonce: string;                 // 16-byte hex; per-package
    };
    iv: string;                      // 12-byte hex
    ciphertext: string;              // hex
    aad: string;                     // hex; binds to schemaHash + holder.wallet
  };

  // Self-integrity. SHA-256 of the canonical encoding of every other
  // field in this object (excluding `packageHash` itself).
  packageHash: string;
};

type SchemaFieldDescriptor = {
  name: string;                     // matches what the circuit indexes by
  type: "uint" | "string" | "bool";
  enumValues?: string[];             // for human-readable display
  description?: string;
};
```

### Canonical encoding

`packageHash` and the issuer signature both bind a canonical encoding.
The canonical encoding rule is: JSON with sorted keys, no extra
whitespace, all numbers as decimal strings (no scientific notation),
all hex lowercase. A reference encoder lives in
`@solid-protocol/issuer::canonicalEncode`.

Two packages are equal if and only if their canonical encodings are
byte-identical.

### Encrypted field data

The plaintext under `encryptedFields.ciphertext` is a JSON object
mapping `SchemaFieldDescriptor.name` -> field value:

```json
{ "kyc_level": 1, "country": "US", "date_of_birth": "1990-01-01" }
```

Field values match the types declared in `SchemaFieldDescriptor.type`.
The plaintext also includes a `_credentialAtomInputs` private subobject
holding the values the circuit needs at proof time (revocation nonce,
issuer signature components, master-key witness derivation hints).

Encryption: ChaCha20-Poly1305. Key derivation: see
`docs/HOLDER_STORAGE_AND_WALLET.md` -- the holder wallet signs a
deterministic message including this package's `keyDerivation.nonce`,
and the resulting signature is HKDF-extracted into a 32-byte key.

AAD binds:
- the schema hash (so the encrypted blob cannot be reattached to a
  different schema),
- the holder.wallet (so the package cannot be retargeted to a
  different holder).

If either binding is wrong, ChaCha20-Poly1305 decryption fails and
the holder app surfaces a typed error.

## Three delivery channels

### Channel 1 -- Encrypted claim link (devnet primary)

This is the default and the one almost all devnet issuance will use.

**Flow:**

1. The issuer generates a `CredentialPackage` (full plaintext,
   encrypted with the holder's wallet-derived key).
2. The issuer uploads the encrypted package to a claim-link service.
   The service returns a one-time URL of the form
   `https://claim.solid.example/c/<token>` where `<token>` is a
   high-entropy random ID.
3. The issuer delivers the URL to the user out-of-band (email,
   Discord, in-product handoff). The URL is sensitive: anyone with
   the URL can fetch the encrypted package, but only the holder
   wallet can decrypt it.
4. The user clicks the link. The holder web app at
   `claim.solid.example` (or `holder.solid.example/claim`) prompts
   the user to connect their wallet.
5. The holder app fetches the encrypted package, asks the wallet to
   sign the deterministic message, decrypts the field data, validates
   the issuer signature against the commitment, and imports the
   credential into local encrypted storage.
6. The claim-link service marks the token as consumed (one-time-use)
   and deletes the encrypted blob after import is acknowledged. The
   token is invalid for any future fetch.

**What the claim-link service has to do:**

- Accept POSTs from approved issuers (authenticated by the issuer's
  Solana key signing the upload). Store the encrypted package keyed
  by token. Return the token URL to the issuer.
- Serve the encrypted package to anyone presenting the token, exactly
  once, until expiry (default 7 days).
- Run no key material. The service never sees the plaintext fields.
  The encryption is end-to-end issuer-to-holder.

**What the claim-link service does not need to do:**

- Authenticate users. The wallet signature decrypting the package is
  the auth.
- Track holder identities. The token is the only identifier.
- Be highly available. If the service is down, the issuer can
  re-upload to a fallback service or fall back to Channel 2.

This channel is the right default because:

- Issuers have nothing to deploy. They call
  `@solid-protocol/issuer::issueCredential({ delivery: "encrypted-link" })`
  and get a URL back. The CLI prints it.
- Holders have nothing to install beyond the holder web app.
- The cryptographic guarantees do not depend on the claim-link
  service: a malicious operator cannot read fields, cannot mint new
  credentials (issuer signature is required), and cannot retarget
  the package (AAD binding rejects).

### Channel 2 -- Downloadable JSON (fallback)

For air-gapped issuers, sensitive credentials, or when the claim-link
service is unreachable.

**Flow:**

1. Issuer generates the `CredentialPackage` exactly as in Channel 1.
2. Issuer downloads it as a `.solid-credential.json` file.
3. Issuer transmits the file to the user via any out-of-band channel
   (encrypted email, USB, Signal, etc.).
4. User opens the holder web app, clicks "Import credential", selects
   the file. Wallet signs, package decrypts, credential is imported.

The JSON file is exactly the same `CredentialPackage` shape as
Channel 1. Channels 1 and 2 are interchangeable from the holder
perspective: the holder app accepts both with one method.

This channel is also the canonical backup format. A holder export
of an existing credential produces the same bytes as a fresh issuance,
modulo the `keyDerivation.nonce` (which is regenerated per-export).

### Channel 3 -- Wallet Standard handoff (mainnet target)

For production-grade UX where the wallet itself participates in
credential storage. Spec'd in `docs/HOLDER_STORAGE_AND_WALLET.md`.

**Flow:**

1. Issuer generates the `CredentialPackage`.
2. Issuer dApp calls `wallet.features["solid:importCredential"](pkg)`
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
2. Issuer also publishes a SAS attestation with `data = packageHash`
   and `subject = holder.wallet`.
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

- See encrypted blobs, opaque tokens, holder-wallet pubkeys, and
  issuer pubkeys. (Privacy: low impact -- the holder wallet pubkey
  is already public on-chain.)
- Refuse to serve a package (denial-of-service against a single
  credential delivery; recoverable via Channel 2 fallback).
- Attempt to swap a package for a different one. The packageHash
  is signed by the issuer; the holder app verifies the signature
  before import; swapping fails closed.

A malicious service **cannot**:

- Decrypt field data (no access to wallet-derived key).
- Forge issuer signatures (no access to issuer BJJ private key).
- Retarget a package to a different holder (AAD binding rejects).
- Mint an "uninvited" credential (the on-chain `issue_credential`
  must run independently and the package must reference the
  resulting tree leaf).

### What the holder gives away on import

When the holder fetches a claim link, the claim-link service learns:

- The token (which it gave out and already knew).
- The holder wallet's pubkey (revealed by the wallet signature).
- The IP address and user-agent from which the import happened.

The first two are unavoidable. The third is mitigable by hosting
the holder app over Tor or a privacy-respecting CDN.

### Defenses against credential phishing

A user could be tricked into pasting a malicious claim link into a
fake holder app. Mitigations:

- The holder app domain (`holder.solid.example`) is fixed and
  documented. Users learn to verify the domain, same as any web app.
- The signed message includes the domain (`SolID-credential-key-v1
  domain=holder.solid.example`), so a malicious app on a different
  domain produces a different decryption key and fails.
- The package's issuer signature is verified against the on-chain
  registry. A package signed by an unregistered or revoked issuer
  is rejected with a typed error.

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

This design is implemented as part of Tier A:

1. **A5 (Issuer CLI)** ships `issueCredential({ delivery:
   "package-bytes" })` first -- returns the raw `CredentialPackage`
   bytes. Lets us iterate on the format without depending on the
   claim-link service.
2. **A4 (Holder web app)** ships JSON file import (Channel 2) first.
   Same reason.
3. **A4 + claim-link service** ship Channel 1 once Channels 1 and 2
   round-trip cleanly.
4. **Channel 3 (Wallet Standard)** ships only after the first wallet
   vendor agrees to adopt. Not a devnet blocker.
5. **Channel 4 (SAS)** ships when SAS finalizes.

## Versioning

The `version: 1` field in `CredentialPackage` is the wire-format
version. Any breaking change to field semantics (new required fields,
removed fields, changed encryption) bumps to `version: 2`. The
holder app must support every shipped version indefinitely; old
credentials cannot be re-issued.

Non-breaking additions (new optional metadata fields, new schema
field types) do not bump the version. Holder apps must tolerate
unknown optional fields.

The `algorithm` field under `encryptedFields` exists so we can add a
post-quantum encryption option later without a wire-format version
bump.

## Reference implementations

- `@solid-protocol/issuer` exports `buildCredentialPackage(...)` and
  `canonicalEncode(...)`.
- `@solid-protocol/holder` exports `importCredentialPackage(...)`
  and `verifyCredentialPackage(...)`.
- `crates/solid-core/src/credential.rs` (TODO) is the Rust
  ground-truth implementation that the TS SDK is byte-checked
  against via the cross-language vector suite (SOLID-SEC-010).

---

*End of design. The wire format is the contract; the channels are
the policy.*
