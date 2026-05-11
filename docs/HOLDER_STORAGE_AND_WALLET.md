# Holder Storage and Wallet Integration

Status: Design + partial implementation. Last reconciled 2026-05-04.

This document specifies where credentials live on the holder side and
how SolID expects wallets to integrate. It is the authority for the
two-layer architecture (web holder app + Wallet Standard interface)
and for the deterministic key-derivation scheme.

## Current implementation state (2026-05-04)

What has shipped is the M1 channel-fix milestone, which delivers a
narrower slice of this design:

- **Form factor**: a Chrome extension (`solid-wallet`), not the Layer 1
  web holder app at `holder.solid.example`. The extension owns its own
  identity seed inside an encrypted vault rather than deriving keys via
  external `wallet.signMessage`. Layer 1 (web holder app) and Layer 2
  (Wallet Standard) below remain forward design.
- **Channel**: `@solid-protocol/channel` is the canonical wire format.
  Issuer-to-holder delivery uses an `EncryptedEnvelope` (NaCl box / X25519
  + XSalsa20-Poly1305) wrapping a JSON-serialized `CredentialBundle`.
  References to `CredentialPackage` and `encryptedFields` later in this
  doc are pre-implementation naming and have been superseded -- see
  `docs/CREDENTIAL_DELIVERY_DESIGN.md` for the canonical wire format.
- **Channel public key**: the wallet exposes a base64-encoded X25519
  public key derived deterministically from the unlocked identity seed
  via `deriveChannelKeyFromMasterSeed("solid:channel:v1" || seed)`. The
  user copies this from the wallet's Settings page and pastes it into
  the issuer console at issue time. See `Settings.tsx` -> "Channel
  Public Key" affordance.
- **Holder-side cryptographic verification at import time**: every
  decrypted bundle is run through `assertCredentialIsSoundForWallet`
  before it touches IndexedDB. See "Holder-side credential integrity
  gate" below.
- **No `holderPrivateKey` on the wire or in storage**: the wallet derives
  the per-schema BJJ private key just-in-time at proof generation via
  `deriveCredentialKey(masterSeed, schemaHash)`. Nothing in the bundle
  or in the encrypted IndexedDB record carries a holder secret. A
  defense-in-depth check at proof time (`fromJsonCredential`) re-derives
  the holder pubkey and refuses to proceed if the stored
  `holderPubKeyX/Y` no longer matches the wallet identity.
- **Holder revocation nonce**: the wallet's `WalletSettings` has a new
  `holderRevocationNonce: string` (decimal) field that feeds the
  6-input Poseidon nullifier preimage at proof time. Default is `'0'`
  for fresh identities. The settings field is plumbed end to end
  (background -> popup) but is not yet user-editable through the UI; a
  user who has rotated their identity-state revocation nonce must
  currently edit the value through `chrome.storage.local`. UI input is
  a tracked gap.
- **The 3-key derivation scheme described under "The derivation scheme"
  below has not shipped as written.** The extension currently derives
  three things from the unlocked vault seed: (a) a storage key
  (`deriveCredentialStorageKey`); (b) the channel keypair via
  `deriveChannelKeyFromMasterSeed`; (c) per-schema BJJ keypairs via
  `deriveCredentialKey(masterSeed, schemaHash)`. These are
  domain-separated but the canonical-message + HKDF wrapping below is
  forward design.

The remainder of this document describes the longer-term target. Where
the target diverges from what is implemented, the divergence is
intentional -- this doc is the destination, not a status report.

----

The audience is two readers: (1) the SolID team building the holder
web app and the Wallet Standard reference implementation; (2) wallet
vendors (Phantom, Backpack, Solflare, etc.) deciding whether to adopt
native SolID support.

## Companion docs

- `docs/CREDENTIAL_DELIVERY_DESIGN.md` -- the issuer-to-holder package
  format and delivery channels.
- `plan/VERIFIER_SDK_SHAPE.md` -- the high-level verifier SDK that
  initiates proof requests against this storage.
- `plan/DEVNET_ROLLOUT_PUNCHLIST.md` Tier A4 -- the holder web app
  implementation task.

## Design principles

1. **The holder's wallet is the root of credential identity.** The
   master BabyJubJub key, all encryption keys, and all credential
   imports are derived deterministically from the wallet's
   `signMessage` capability. Lose the wallet seed, lose the
   credentials -- this matches the user's existing mental model
   ("my wallet holds my money and my IDs").
2. **No new passphrases.** The holder app never asks the user to
   create or remember a passphrase. Anything that needs derivation
   comes from a wallet signature.
3. **Storage is local-first, encrypted at rest, recoverable from the
   wallet alone.** A user installing the holder app on a new device
   should be able to restore every credential by signing the same
   deterministic message -- without uploading anything to a SolID
   server.
4. **Wallets are integration targets, not requirements.** The holder
   web app works with any wallet that implements `signMessage` over
   the Wallet Standard. Native SolID-aware wallets get a better UX,
   but they are not required for v1.
5. **Privacy by default.** The holder app reveals nothing to SolID
   infrastructure beyond what is strictly required for proof
   submission. No analytics, no error reporting that includes
   credential content, no remote logging.

## Holder-side credential integrity gate

This is the structural + cryptographic admission policy every imported
credential must pass before it is encrypted into IndexedDB.

The check lives in
`solid-wallet/src/shared/credential-integrity.ts::assertCredentialIsSoundForWallet`
and runs after `decryptCredential` and before
`bundleToCredentialRecord` in
`solid-wallet/src/background/proof-engine.ts::importCredentialEnvelope`.

**Inputs:**

- The decrypted `CredentialBundle` produced by the channel package.
- The wallet's unlocked master identity seed (32 bytes, from the
  encrypted vault).

**Checks (each fails closed with a typed error):**

1. **WRONG_HOLDER**: the bundle's `holderPublicKey.{x,y}` must equal
   `deriveCredentialKey(masterSeed, bundle.schemaHash).public_key_{x,y}`.
   This proves the credential was issued to *this wallet's* derived BJJ
   pubkey for *this* schema. A bundle whose holderPublicKey was set to
   a different identity is rejected.
2. **INVALID_HOLDER_KEY**: the holder BJJ pubkey is in the BabyJubJub
   prime-order subgroup (via `isInPrimeOrderSubgroup`). Closes any
   small-subgroup attacks on the holder side.
3. **INVALID_ISSUER_KEY**: the issuer BJJ pubkey is in the prime-order
   subgroup. Required because the issuer-tree leaf is computed from the
   issuer pubkey, and the EdDSA verifier later treats the pubkey as
   prime-order.
4. **INVALID_COMMITMENT**: the bundle's `commitment` (32-byte hex)
   equals `computeCommitment(attestationData, schemaHash, holderPubKeyX,
   holderPubKeyY, salt)`. This binds the commitment to its declared
   inputs -- a tampered attestation, schema, holder, or salt fails the
   recompute.
5. **INVALID_ISSUER_SIGNATURE**: the issuer's BJJ EdDSA signature
   `{R8x, R8y, S}` verifies over `commitment` under the issuer's BJJ
   pubkey. This is the actual unforgeability gate.

**What this check does NOT verify (out of scope for the holder gate):**

- That the issuer is currently approved on-chain. The wallet does not
  consult the issuer-registry at import time; staleness is acceptable
  here and the verifier-side enforces fresh issuer-tree roots at proof
  time anyway.
- That the credential's commitment is actually present at
  `merkleProof.leafIndex` in `treeAddress`. The merkle proof is a hint
  captured at issuance; the wallet refreshes from a `MerkleProofAdapter`
  before each proof generation, and the on-chain verifier rejects any
  proof whose tree root does not match the chain.
- Subgroup checks on the issuer signature's R8 point. Those are
  enforced inside `@solid-protocol/core::verify` per the SOLID-SEC-053
  cofactor-8 fix (CLAUDE.md hard invariants).

**Why these checks belong in the wallet, not the channel package:**

The channel package is intentionally crypto-WASM-free so it can be
audited and ship independently of the BJJ + Poseidon primitives. The
wallet imports `@solid-protocol/core`'s WASM bridge for these checks.
Splitting this way also lets a future verifier-side service or `solid-sim`
reuse the same channel package without dragging in the
holder-only integrity rules.

## Two-layer architecture

The holder side is intentionally layered so we can ship v1 without
wallet vendor coordination, then deepen the integration in v1.1+.

### Layer 1 -- Web holder app (devnet primary)

A standalone web application that any wallet can use. Lives at
`https://holder.solid.example` (also reachable from the SolID
project landing page).

**Storage:** Encrypted IndexedDB.

**Capabilities:**
- Import credentials (claim link, JSON file).
- List credentials with status, issuer, expiration.
- Generate proofs on demand for verifier requests.
- Export encrypted backups.
- Delete credentials.

**Key material:** All derived from the wallet's `signMessage` over
deterministic domain-separated messages. Never persisted in
plaintext anywhere.

**Trust:** Treat the holder app domain as part of the trust
boundary. The user must verify they are on `holder.solid.example`
before importing credentials. SolID publishes the canonical domain
in CLAUDE.md, the README, and on the protocol's landing page.

### Layer 2 -- Wallet Standard SolID feature (mainnet target)

A wallet that implements the SolID feature interface stores
credentials inside the wallet itself. The user does not visit
`holder.solid.example`; the wallet IS the holder app.

**Capabilities:** Same as Layer 1, plus:
- Native UI integration (no separate browser tab).
- Mobile-first via wallet's existing mobile app.
- Hardware-wallet-friendly (the wallet's secure element handles BJJ
  key storage and proof signing if the wallet supports it).

**Adoption path:**
1. Publish the spec (this document, plus
   `@solid-protocol/wallet-standard-spec`).
2. Ship a reference implementation in `@solid-protocol/holder` that
   wallets can vendor as a dependency.
3. Provide a fallback adapter so apps integrate against one API
   regardless of whether the wallet has native support.
4. Lobby first wallet vendor (probably Backpack -- developer-friendly
   and SolID has Solana ecosystem alignment).

The two layers share the `@solid-protocol/holder` package. Wallets
adopt it; the standalone web app embeds it. The exact same code path
runs in both.

## Deterministic key derivation

The most important design choice in this document: every key the
holder needs is derived from the wallet's signature over a
deterministic message.

### Why deterministic

If we let users generate random BJJ keys and back them up by
passphrase, we have:

- Lost-passphrase support tickets, forever.
- A second seed phrase the user has to remember.
- A backup-recovery UX that is the most common cause of user
  failure in crypto wallets.

By deriving from `wallet.signMessage(...)`, the user only has to
remember their wallet seed phrase -- which they already do. A user
who can recover their wallet can recover their credentials.

### The trade-off

Anyone with the wallet seed phrase can decrypt and use the
credentials. A user who shares their wallet (e.g. with a malicious
browser extension) loses their credentials too. This is **the
correct trade-off** because:

- The user already faces this risk for their funds.
- Splitting the trust model across two seed phrases does not actually
  reduce risk -- attackers will compromise both, or social-engineer
  whichever is weaker.
- The mental model "my wallet holds my IDs" is what users expect.

The alternative (a separate credential passphrase) is left as a v2
opt-in for users with elevated threat models who want hardware
wallets, multi-party computation, or HSM-backed key storage.

### The derivation scheme

The holder needs three derived keys:

1. **Master BJJ private key** -- used to derive per-schema subkeys
   that prove identity in the circuit.
2. **Storage encryption key** -- used to encrypt every credential
   blob in IndexedDB.
3. **Per-credential field encryption key** -- used to decrypt
   `CredentialPackage.encryptedFields` (one per credential).

All three derive from the same wallet signature primitive but with
distinct domain-separated inputs.

```
PRIMITIVE = wallet.signMessage(canonicalMessage)

canonicalMessage = JSON.stringify({
  domain: "SolID-key-v1",
  use: "<master-bjj | storage-encryption | credential-decrypt>",
  network: "<solana-devnet | solana-mainnet>",
  walletPubkey: "<base58>",
  // For credential-decrypt only:
  credentialNonce?: "<hex from CredentialPackage.encryptedFields.keyDerivation.nonce>",
}, sortKeys: true)

KEY_BYTES = HKDF-SHA256(
  ikm = PRIMITIVE,
  salt = sha256("SolID-derive-salt"),
  info = canonicalMessage,
  length = 32 bytes
)
```

The `use` field separates the three keys cryptographically. The
`network` field separates devnet from mainnet keys (so a user's
devnet credentials cannot be silently used on mainnet or vice versa).
The `walletPubkey` field binds to the specific wallet (so wallet
rotation produces fresh keys, by design).

### What the wallet must support

The bare minimum: `wallet.signMessage(messageBytes)` returning a
deterministic 64-byte Ed25519 signature.

Wallet Standard `solana:signMessage` already provides this. Phantom,
Backpack, Solflare, and every Wallet Standard implementer support
it natively.

The signature must be deterministic. Solana's Ed25519 is
deterministic by spec; this is fine. (If a wallet ever ships
non-deterministic signing, SolID's deterministic derivation breaks --
we'd surface that as a `WALLET_NONDETERMINISTIC_SIGNATURE`
error and refuse to derive keys.)

### What the wallet does not need to know

The wallet does not need to know:
- That the signature is being used to derive keys.
- The structure of the canonical message.
- The HKDF parameters.
- Anything about BabyJubJub, Poseidon, or Groth16.

Until and unless the wallet adopts the Wallet Standard SolID feature,
it just sees `signMessage` requests with `SolID-key-v1` in the
domain. From the wallet's perspective, this is a normal signed
message, no different from a sign-in-with-Solana flow.

## IndexedDB schema

The holder web app uses IndexedDB. Schema:

```ts
// Object store: "credentials"
type StoredCredential = {
  id: string;                       // UUID; primary key
  encryptedBlob: Uint8Array;        // ChaCha20-Poly1305 over the full CredentialPackage
  iv: Uint8Array;                   // 12-byte
  // Index fields (encrypted, but indexed for list view).
  encryptedSummary: Uint8Array;     // separate cipher; small, decrypted on list view
  schemaHashHint: string;           // first 4 bytes; for fast filtering, not load-bearing for security
  importedAt: number;
  expiresAtHint: number | null;     // unencrypted timestamp for "show me expired" queries
  status: "active" | "revoked" | "expired";
};

// Object store: "metadata"
type AppMetadata = {
  walletPubkey: string;             // current wallet
  storageVersion: 1;
  lastBackupAt: number | null;
  trustedClaimDomains: string[];    // default: ["claim.solid.example"]
};
```

The encrypted blob's plaintext is the full `CredentialPackage` (after
field decryption). The `encryptedSummary` is a small subset
(schema name, issuer name, expiration) used to render the
credential list without decrypting every full credential on every
list view.

The `schemaHashHint` is unencrypted but only the first 4 bytes; this
balances list-view query speed against leakage. An attacker with read
access to IndexedDB learns "this user has credentials matching
roughly N schemas" but not which ones.

The `expiresAtHint` is unencrypted because users want to see "this
credential expires in 30 days" without an extra decryption.

## Backup and restore

### Export

The holder app's "export backup" feature produces a single JSON file:

```ts
type EncryptedBackup = {
  version: 1;
  walletPubkey: string;             // for cross-checking on restore
  network: "solana-devnet" | "solana-mainnet";
  createdAt: number;
  credentialCount: number;
  // Encrypted with a passphrase-derived key (NOT the wallet-derived
  // key) so the backup is portable across wallet rotations.
  passphraseKeyDerivation: {
    method: "argon2id";
    salt: string;                   // hex
    memoryKiB: 65536;
    iterations: 3;
    parallelism: 4;
  };
  iv: string;                       // hex
  ciphertext: string;                // hex; ChaCha20-Poly1305 over array of credentials
  aad: string;                       // hex; binds to walletPubkey + network
};
```

The user is asked for a passphrase (the only passphrase in the
entire flow, and only for backup/restore). Argon2id derives a 32-byte
key.

### Import (restore)

A user installing the holder app on a new device can:

1. Connect their wallet (the same wallet that created the backup).
2. Open the backup file. The app derives the passphrase key,
   decrypts, validates the AAD against the connected wallet, and
   imports each credential.
3. The credentials are re-encrypted with the new device's
   wallet-derived storage key (which is identical to the old
   device's because the wallet is the same -- but the
   `encryptedSummary` IV gets a fresh nonce per device).

A user installing the holder app on a new device **without** a
backup can:

1. Connect their wallet.
2. Re-claim each credential from the original claim links (if the
   tokens have not expired).
3. Or ask the issuers to re-issue, providing the credential is the
   sort that can be re-issued (KYC: yes, an issued attestation
   that's expired: probably yes, a single-use credential: no).

This means a wallet-only restore is possible for credentials that
are still re-claimable. For credentials that are not re-claimable,
the user MUST have a backup. The holder app prompts the user to
back up after every credential import; refusing the prompt logs a
"backup-skipped" event in app metadata.

## Wallet Standard feature interface

This is the spec wallet vendors implement to become a SolID-native
holder.

### Feature name

`solid:credentials@1`

The `@1` suffix is the spec version. Bumping the major version is a
breaking change; minor additions are backward-compatible.

### Feature methods

```ts
interface SolidCredentialsFeature {
  version: "1.x.y";

  // Inspection.
  listCredentials(filter?: CredentialFilter): Promise<CredentialSummary[]>;
  getCredential(id: string): Promise<CredentialSummary | null>;

  // Import.
  importCredential(pkg: CredentialPackage): Promise<{ id: string }>;
  importFromClaimLink(url: string): Promise<{ id: string }>;

  // Proof flow.
  canSatisfy(req: SerializedRequirement): Promise<boolean>;
  previewDisclosure(req: SerializedRequirement): Promise<DisclosurePreview>;
  generateProof(req: SerializedRequirement): Promise<SerializedProof>;

  // Backup.
  exportBackup(passphrase: string): Promise<Uint8Array>;
  restoreBackup(blob: Uint8Array, passphrase: string): Promise<{ count: number }>;

  // Lifecycle.
  deleteCredential(id: string): Promise<void>;
}

type CredentialFilter = {
  schemaRef?: string | { name: string; version: number };
  issuer?: string;                  // base58 pubkey
  status?: "active" | "expired" | "revoked";
};

type CredentialSummary = {
  id: string;
  schema: { ref: string; hash: string };
  issuer: { authority: string; name: string };
  expiresAt: number | null;
  status: "active" | "expired" | "revoked";
  importedAt: number;
};
```

### Required user prompts

A conforming wallet MUST display a user-confirmation UI for each of:

- `importCredential` and `importFromClaimLink` (display issuer name,
  schema, what data is being stored).
- `generateProof` (display the disclosure preview from
  `previewDisclosure` -- "you are revealing X, hiding Y, scoped to
  app Z").
- `exportBackup` and `restoreBackup` (display "this is reversible
  / irreversible" guidance).
- `deleteCredential` (warn about loss).

The wallet may NOT auto-approve any of these. SolID's privacy
guarantees depend on the user being shown the disclosure before
proof generation; a wallet that auto-approves breaks the
contract.

### Discovery

Wallets advertise the feature via the standard Wallet Standard
features object:

```ts
wallet.features["solid:credentials@1"] = { ... };
```

Apps detect via:

```ts
const isNative = "solid:credentials@1" in wallet.features;
```

If `isNative`, the app talks directly to the wallet. If not, the app
falls back to the Layer 1 web holder app via the fallback adapter.

## Fallback adapter

The fallback bridges apps that want to call the Wallet Standard
SolID interface against wallets that don't implement it natively.

```ts
import { solidWalletAdapter } from "@solid-protocol/holder";

const holder = solidWalletAdapter({
  wallet,                           // any Wallet Standard wallet
  fallbackOrigin: "https://holder.solid.example",
});

// Same interface as native:
const proof = await holder.generateProof(requirement);
```

When called, the adapter:

1. Checks if the wallet has `"solid:credentials@1"` in features. If
   yes, delegates directly.
2. If no, opens `holder.solid.example/proof-request` in a popup,
   passing the serialized requirement via `postMessage`.
3. The popup runs the standard holder web app, prompts the user to
   approve, generates the proof, posts it back via `postMessage`.
4. The adapter returns the proof to the calling app.

This is intentionally cross-origin so the popup's IndexedDB
(holding the credentials) is isolated from any other domain. The
calling app never sees credential content.

## Mobile and browser-extension futures

Both deferred to v1.1+, not devnet blockers.

**Mobile:** The holder web app is responsive and works in mobile
browsers. The longer-term path is React Native + same
`@solid-protocol/holder` core. Solana Mobile Stack's seed-vault
integration would let the master BJJ key live in the device's
secure enclave.

**Browser extension:** Lower priority than wallet integration. A
SolID-specific extension exists in tension with the goal of having
wallets adopt the Wallet Standard feature. We will revisit if
wallet adoption stalls past mid-2026.

## Threat model

### What the holder app must not do

- Send credential content to any server (SolID-operated or otherwise).
- Send wallet signatures used for key derivation to any server.
- Log to remote analytics with credential identifiers.
- Cache the wallet-derived keys longer than the proof-generation
  scope (clear after every operation).

### What an attacker with browser RCE can do

If the holder app's domain is compromised, the attacker can read
the IndexedDB (encrypted at rest, but the wallet-derived key is
recoverable on signMessage). Mitigations:

- Subresource Integrity (SRI) on every script.
- Content-Security-Policy locking down origins.
- Minimal third-party JS dependencies.
- Cloudflare Pages or Vercel deployment with deploy-time
  verification of bundle hashes.

If the user's wallet itself is compromised, the credentials are
gone. We do not defend against compromised wallets; we trust the
wallet's security model.

### What an attacker on the network can do

All claim-link traffic is HTTPS. The encrypted package is
end-to-end-encrypted from issuer to holder, so MITM gives nothing.

An attacker observing the holder app's traffic learns the user
visited `holder.solid.example` and (potentially) which on-chain
verify transactions they submitted. They do not learn the
credential content or which credentials the user holds.

### What an attacker with physical device access can do

If the device is unlocked and the wallet is unlocked, all bets are
off. Standard advice applies: lock devices, use OS-level encryption.

The holder app does NOT add a separate device-level passphrase --
that would violate the "no new passphrases" principle and produce
a worse UX without a meaningful security gain. Users who want
extra protection should use a hardware wallet (which prompts for
PIN per signature) or run the holder app in a sandboxed browser
profile.

## Privacy guarantees

The holder app reveals to SolID infrastructure:

- The act of visiting `holder.solid.example` (HTTP request).
- The act of fetching an artifact from
  `artifacts.solid.example` (HTTP request, content-addressed).
- The act of fetching a Merkle proof from `indexer.solid.example`
  (HTTP request, with leaf index).

It does NOT reveal:

- Which credential was used for a proof.
- The credential's field values.
- The wallet's identity (no auth tokens exchanged with SolID infra).

The leaf-index leak via the Merkle-proof fetch is mitigable by
batching ("give me Merkle proofs for these N leaves") -- noted as
a v1.1 hardening item.

## Implementation order

Tier A4 (`plan/DEVNET_ROLLOUT_PUNCHLIST.md`) ships:

1. IndexedDB schema + storage adapter.
2. Wallet-signature key derivation (3 keys: master BJJ, storage,
   credential-decrypt).
3. Import flow for Channel 2 (JSON file).
4. Import flow for Channel 1 (claim link).
5. List view + detail view with summary decryption.
6. `canSatisfy` + `previewDisclosure` against locally-stored
   credentials.
7. Proof generation via `@solid-protocol/holder`.
8. Backup / restore flow.
9. Wallet-adapter transport receiver (for verifier proof requests).

Wallet Standard feature spec lives at
`docs/WALLET_STANDARD_SOLID_SPEC.md` (TODO; written when first
wallet vendor opts in). The reference implementation is the
`@solid-protocol/holder` package; wallets vendor it directly.

## Open questions

1. **Should the holder app run a service worker for offline proof
   generation?** Useful for travelers and air-gapped scenarios.
   Adds complexity (service worker debugging is painful). Default:
   no service worker for v1; revisit if a partner asks for it.
2. **Should we publish the holder app as a static IPFS bundle?**
   Reduces SolID's domain trust burden. Adds DNS+CID rotation
   overhead. Default: standard HTTPS hosting for v1; IPFS mirror
   shipped as a parallel option in v1.1.
3. **Can the wallet-derived storage key be cached for a session, or
   must we re-derive on every operation?** Re-derivation is cheaper
   (Ed25519 sig + HKDF; ~10ms) but produces wallet-prompt fatigue.
   Default: cache the storage key for the active tab session;
   re-derive on tab close. Configurable per-wallet.
4. **What happens if the user has multiple wallets connected?** The
   holder app keys are bound to a single wallet (per derivation).
   We need either (a) per-wallet IndexedDB partitions or (b) a
   "switch wallet" UI that re-decrypts everything. Default: (b),
   because (a) leaves credentials orphaned when wallets are
   disconnected.
5. **Should wallet vendors be required to use a hardware-backed
   keystore for the BJJ master key if they implement Layer 2?** The
   spec recommends but does not require it. A hardware-backed
   implementation is unconditionally better for users; making it
   mandatory blocks early adoption. Revisit at v2.

---

*End of design. The wallet is the root; everything else is derived.*
