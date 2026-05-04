# `@solid-protocol/verifier` -- High-level SDK Shape

Status: **Shipped** (2026-05-04 audit pass-2 verification). Originally
drafted 2026-05-02 as a design sketch; the 2026-05-04 SDK audit pass-2
verified the implementation lives at
`ts-sdk/packages/verifier/src/index.ts:1186-1450` (`SolidVerifier`
class) with `defineRequirement` (:1217), `requestProof` (:1255),
`verifyProof` (:1286), `verifyRequirement` (:1336), `health` (:1356),
`loadArtifact` (:1408), `walletAdapterTransport` (:1452),
`httpTransport` (:1479), `explainVerificationError` (:1499),
typed `VerificationError` (16 variants, :973-989), and
`SolidVerificationError` (:1009). Remaining work to flip the
`DEVNET_ROLLOUT_PUNCHLIST.md` A3 status from `[~]` to `[X]` is
publish + devnet acceptance, not implementation.

This document is the contract the wrapper satisfies. It is preserved
as a design reference so integrators can understand the principles
behind the API; the source of truth for behavior is the implementation
at the citations above.

The current shipped surface includes both:
- The high-level `SolidVerifier` (this document's subject).
- The lower-level `verifyOnChainV2` orchestration
  (`ts-sdk/packages/verifier/src/index.ts:698`) for callers that need
  raw access. `SolidVerifier.verifyProof` delegates to it internally.

The audience for this document is two readers:
- a Solana app developer who has 15 minutes to evaluate SolID and is
  writing the integration call,
- the SolID team writing the wrapper that backs that call.

If both readers can read this document and agree on what the call
does, the design is correct.

## Design principles

1. **The developer thinks in requirements, not circuits.** Replace
   "merkleRoots", "schemaHashes", "issuerTreeRoot", "publicSignals"
   with "schema", "predicates", "action".
2. **One default config that works on devnet.** The integrator
   should not have to know the Solana program IDs, the VK SHA-256
   pins, or the artifact CDN URL. `new SolidVerifier({ cluster:
   "devnet" })` is enough.
3. **Typed errors over raw RPC failures.** Every reason a
   verification can fail maps to a typed enum. The integrator never
   reads a `SendTransactionError` log to figure out what went wrong.
4. **Same shape on the server and the client.** A backend that
   verifies a proof submitted by a frontend uses the same call as
   the frontend that requested the proof.
5. **The wrapper does NOT generate proofs.** Proof generation lives
   in the holder, where the credential lives. The verifier wrapper
   only requests, transmits, and submits proofs.
6. **Honesty about disclosure.** The wrapper exposes a method that
   tells the holder, in plain language, exactly what the proof
   reveals -- so the holder UI can show it before the user clicks
   "Prove".

## Two-actor flow

```
        [Verifier app]                       [Holder app]
              |                                    |
   1. defineRequirement()                          |
              |                                    |
   2. requestProof(req) ----[wallet adapter]-----> |
              |                                    |
              |                          3. canSatisfy(req)
              |                          4. previewDisclosure(req)
              |                          5. (user accepts)
              |                          6. generateProof(req)
              | <----[serialized proof]----------- |
              |                                    |
   7. verifyProof(proof, req)                      |
              |                                    |
   8. -> { verified: true, reason?: ... }
```

Verifier-side methods are in `@solid-protocol/verifier`; holder-side
methods are in `@solid-protocol/holder` (already partially shipped).
Both packages share types from `@solid-protocol/core`.

## Verifier-side API

### `class SolidVerifier`

```ts
class SolidVerifier {
  constructor(config: SolidVerifierConfig);

  // Build a typed requirement that can be sent to a holder, stored,
  // or fingerprinted for replay scoping.
  defineRequirement(spec: RequirementSpec): Requirement;

  // Request a proof from a holder over a wallet-adapter channel.
  // Returns a request handle the verifier can poll or await.
  requestProof(args: {
    wallet: PublicKey;
    requirement: Requirement;
    transport?: ProofRequestTransport;  // default: wallet-adapter event
  }): Promise<ProofRequestHandle>;

  // Verify a holder-supplied proof. Submits to chain, waits for
  // confirmation, post-fetches the nullifier PDA as defense in
  // depth, and returns a typed result.
  verifyProof(args: {
    proof: SerializedProof;
    requirement: Requirement;
    payer: Keypair;             // verifier pays the verify CU + rent
  }): Promise<VerificationResult>;

  // Convenience: do all of (1) defineRequirement, (2) requestProof,
  // (3) verifyProof in a single awaited call. Used in demo apps and
  // single-action launchpad gates.
  verifyRequirement(args: VerifyRequirementArgs): Promise<VerificationResult>;

  // Health probe: pings the artifact pin chain, the program IDs,
  // and the indexer. Used by the devnet status page.
  health(): Promise<HealthReport>;
}
```

### `SolidVerifierConfig`

```ts
type SolidVerifierConfig = {
  // Required.
  cluster: "devnet" | "mainnet" | "localnet";
  connection?: Connection;        // default: clusterApiUrl(cluster)

  // Optional overrides. Defaults pull from
  // ts-sdk/packages/sdk/src/artifact_integrity.ts pin chain.
  programIds?: {
    zkVerifier?: PublicKey;
    issuerRegistry?: PublicKey;
    schemaRegistry?: PublicKey;
  };
  artifactPins?: {
    batchVk?: string;             // SHA-256 hex
    batchWasm?: string;
    batchZkey?: string;
    subgroupVk?: string;
    subgroupWasm?: string;
    subgroupZkey?: string;
  };
  artifactHostUrl?: string;       // hosted CDN; defaults to canonical SolID host
  indexerUrl?: string;             // Merkle proof / replica API
  computeUnitLimit?: number;       // default: 500_000 (per CU baselines)
  confirmCommitment?: "confirmed" | "finalized";  // default: "confirmed"
  trustedIssuers?: PublicKey[];    // optional whitelist; default: any approved
  trustedSchemas?: SchemaRef[];    // optional whitelist
};
```

### `RequirementSpec`

This is the shape an app developer writes. It is the only object
the developer authors by hand.

```ts
type RequirementSpec = {
  schema: SchemaRef;               // "kyc_basic_v1" or { name, version }
  predicates: Predicate[];          // 1..MAX_PREDICATES
  compoundLogic?: "AND" | "OR";    // default: "AND"
  action: ActionScope;              // verifier-bound replay scope
  expiresAt?: number;               // optional unix ts; default: never
  trustedIssuers?: PublicKey[];    // override config-level whitelist
};

type Predicate =
  | { field: FieldName; op: "=="; value: number | string | boolean }
  | { field: FieldName; op: "!="; value: number | string | boolean }
  | { field: FieldName; op: ">="; value: number }
  | { field: FieldName; op: "<="; value: number }
  | { field: FieldName; op: ">";  value: number }
  | { field: FieldName; op: "<";  value: number }
  | { field: FieldName; op: "in"; value: ReadonlyArray<number | string> };

type ActionScope = {
  appId: string;                    // verifier program / dApp identifier
  action: string;                   // "join_launchpad_pool" | "vote_proposal_42"
  nonce?: Uint8Array;               // optional; default: random per-proof
};

type SchemaRef = string | { name: string; version: number };
type FieldName = string;            // schema-defined field name
```

The wrapper resolves a `RequirementSpec` to:
- the on-chain schema PDA,
- the schema hash (5-input Poseidon),
- the predicate index map (which schema field each predicate
  references),
- the operator + value encoding for the circuit,
- the verifierAddress (BE-encoded program-derived) and verifierNonce,
- the action scope hash for replay binding.

That resolution is what the SDK hides.

### `Requirement` (resolved)

```ts
type Requirement = {
  // Opaque to the developer; passed to requestProof / verifyProof.
  readonly spec: RequirementSpec;
  readonly fingerprint: string;        // 32-byte hex hash
  readonly schemaHash: string;
  readonly verifierAddress: PublicKey;
  readonly publicInputs: PublicInputBlueprint;
};
```

### `VerificationResult`

```ts
type VerificationResult =
  | {
      verified: true;
      signature: TransactionSignature;
      nullifier: string;               // hex
      slot: number;
      issuer: { pubkey: PublicKey; status: IssuerStatus };
      schema: { hash: string; ref: SchemaRef };
    }
  | {
      verified: false;
      reason: VerificationError;
      signature?: TransactionSignature; // present if rejected on-chain
      detail?: string;                  // free-form for debugging
    };
```

### `VerificationError` (typed enum)

Every reason a verification can fail. Stable across versions.

```ts
type VerificationError =
  // -- holder-side --
  | "MISSING_CREDENTIAL"             // holder has no credential for the schema
  | "EXPIRED_CREDENTIAL"             // credential past expiration
  | "REVOKED_CREDENTIAL"             // credential's revocation nonce bumped
  | "REVOKED_ISSUER"                 // issuer revoked or in cooldown
  | "UNSUPPORTED_SCHEMA"             // schema not registered or not whitelisted
  | "PREDICATE_NOT_SATISFIED"        // honest holder cannot meet the requirement
  | "USER_REJECTED"                  // holder declined to prove

  // -- protocol-side --
  | "PROOF_GENERATION_FAILED"        // snarkjs witness or fullProve threw
  | "PROOF_REPLAYED"                 // nullifier PDA already exists
  | "ARTIFACT_PIN_MISMATCH"          // wasm/zkey/VK SHA-256 disagrees with pin
  | "VK_FROZEN"                      // verifier in mid-rotation; retry later
  | "INVALID_PUBLIC_INPUTS"          // wire-vs-witness disagreement

  // -- transport / RPC --
  | "RPC_UNAVAILABLE"
  | "TRANSACTION_TIMEOUT"
  | "INSUFFICIENT_PAYER_BALANCE"

  // -- catch-all (always also include detail) --
  | "UNKNOWN";
```

The wrapper also exports:

```ts
function explainVerificationError(
  err: VerificationError,
  detail?: string,
): { humanReadable: string; suggestedAction: string };
```

so app UI never has to ship its own copy of these strings.

### `requestProof` transport

```ts
interface ProofRequestTransport {
  send(req: ProofRequestEnvelope): Promise<SerializedProof>;
}
```

Default transports shipped:
- `walletAdapterTransport(wallet)` -- emits a custom event the
  Wallet Standard / @solana/wallet-adapter ecosystem can pick up.
- `qrCodeTransport()` -- encodes `ProofRequestEnvelope` into a
  Solana Pay-style URL the holder app can scan.
- `httpTransport(holderEndpoint)` -- POST to a holder daemon
  (used for issuer dashboards or backend-to-backend testing).

The `ProofRequestEnvelope` includes the resolved `Requirement`, the
artifact pins the holder must use, and an expiry timestamp.

## Holder-side API

These live in `@solid-protocol/holder`. Already partially shipped
(`generateProof`); the wrapper adds the user-facing methods.

```ts
class SolidHolder {
  constructor(config: SolidHolderConfig);

  importCredential(pkg: CredentialPackage): Promise<CredentialId>;
  listCredentials(): Promise<CredentialSummary[]>;

  // Returns true if the holder has at least one credential that
  // satisfies the requirement; does NOT generate a proof. Cheap.
  canSatisfy(req: Requirement): Promise<boolean>;

  // Returns a plain-language description of what a proof for this
  // requirement would reveal. Powers the holder disclosure UI.
  previewDisclosure(req: Requirement): DisclosurePreview;

  // Generate the proof. Throws a typed error if cannot.
  generateProof(req: Requirement): Promise<SerializedProof>;

  // Encrypted backup / restore.
  exportEncryptedBackup(passphrase: string): Promise<Uint8Array>;
  restoreFromBackup(blob: Uint8Array, passphrase: string): Promise<void>;
}

type DisclosurePreview = {
  // Plain-language sentences for the user.
  reveals: string[];                 // ["You hold a KYC credential at level >= 1"]
  hides: string[];                   // ["Your name", "Your ID number", "Your DoB"]
  scope: string;                     // "This proof is scoped to <appId>:<action>"
  reuse: "single-use";               // nullifier semantics in plain words
};
```

## Issuer-side API

These live in `@solid-protocol/issuer`. Phase E.4 shipped
`generateSubgroupProof`; the wrapper adds the user-facing
issuance flow.

```ts
class SolidIssuer {
  constructor(config: SolidIssuerConfig);

  // Idempotent. Generates BJJ keypair if missing; runs subgroup
  // proof; submits register_issuer; returns a registration handle.
  register(args: {
    authority: Keypair;
    metadata: IssuerMetadata;
    stakeAmount: bigint;
  }): Promise<RegistrationHandle>;

  // Issues a credential to a holder pubkey, returns a delivery
  // package the holder can claim.
  issueCredential(args: {
    holder: PublicKey;
    schema: SchemaRef;
    fields: Record<FieldName, number | string | boolean>;
    expiresAt?: number;
    delivery: "encrypted-link" | "package-bytes" | "wallet-handoff";
  }): Promise<CredentialPackage>;

  revokeCredential(credentialId: CredentialId): Promise<TransactionSignature>;
  rotateMetadata(updates: Partial<IssuerMetadata>): Promise<TransactionSignature>;

  // Operator dispute window (SEC-061).
  withdrawAfterRevoke(): Promise<TransactionSignature>;
}
```

## Example: Solana app integration

This is what the integrator writes (the canonical 15-minute
quickstart).

```ts
import {
  SolidVerifier,
  walletAdapterTransport,
} from "@solid-protocol/verifier";

const solid = new SolidVerifier({ cluster: "devnet" });

const requirement = solid.defineRequirement({
  schema: "kyc_basic_v1",
  predicates: [
    { field: "kyc_level", op: ">=", value: 1 },
    { field: "restricted_jurisdiction", op: "==", value: 0 },
  ],
  action: { appId: "my-launchpad", action: "join_pool_42" },
});

async function onJoinClicked(wallet) {
  const result = await solid.verifyRequirement({
    wallet: wallet.publicKey,
    requirement,
    transport: walletAdapterTransport(wallet),
    payer: backendPayerKeypair,
  });

  if (result.verified) {
    return allowUser(wallet.publicKey);
  }

  const { humanReadable, suggestedAction } =
    explainVerificationError(result.reason, result.detail);
  return showError(humanReadable, suggestedAction);
}
```

That's the whole integration. Everything else is the SDK's job.

## What is intentionally hidden

Each item below is something the current `verifyOnChainV2`
integrator is forced to know. The wrapper hides every one.

| Hidden                                | Currently exposed at            |
| ------------------------------------- | ------------------------------- |
| Public-input slot ordering            | verifier/src/index.ts:62-64     |
| BE/LE byte conversion for verifier address (slot 29 is BE; rest are LE) | the SOLID-SEC-031 narrative |
| Issuer tree root reconstruction        | scripts/prove.ts                |
| Schema tree root reconstruction        | scripts/prove.ts                |
| Global root reconstruction              | scripts/prove.ts                |
| Proof buffer init / chunked upload     | verifier/src/index.ts::verifyOnChainV2 |
| ALT (address lookup table) cache       | verifier/src/index.ts            |
| Nullifier PDA derivation               | verifier/src/index.ts            |
| Compute-unit ix attachment              | verifier/src/index.ts            |
| Confirmation polling + revert detection | SOLID-SEC-064                    |
| Artifact pin chain (env > sidecar > config) | sdk/src/artifact_integrity.ts:168-207 |
| G2 imag/real swap on alt_bn128         | SOLID-SEC-067 fix                |
| Verify-vs-replay distinction            | scripts/prove.ts                 |

## Open design questions

These are the things I'm not yet sure about. Listing them up so
the SolID team can pick before any code is written.

1. **Should `requestProof` block until a holder responds, or return a
   handle the caller polls?** Default: block with a timeout. Polling
   API available via `requestProof(...).handle.cancel()`.
2. **Should the wrapper attempt local proof verification before
   submitting on-chain, to fail fast on invalid proofs?** Cost:
   extra ~150ms host snarkjs.verify. Benefit: better error messages
   without paying a verify CU. Default: yes; opt-out via config.
3. **Should the wrapper cache nullifier PDA existence checks?** A
   "proof already used" answer is cheap if cached. Default: in-memory
   LRU; explicit `solid.cache.clear()` available.
4. **How do we handle issuer whitelisting on a per-requirement
   basis?** Spec proposes `trustedIssuers?: PublicKey[]` at the
   requirement level. Open question: should we also support an
   issuer-tier predicate ("any issuer at tier >= silver"), and
   how does that interact with the on-chain `IssuerTier`?
5. **Subscription / event API.** Some integrators want
   `solid.onProofVerified(callback)` for indexers. Out of scope for
   v1, but the architecture should not preclude it.
6. **Rate limiting / DoS.** If the hosted artifact CDN is the only
   way artifacts are served, a verifier with bad pins becomes a
   pin-mismatch storm. The wrapper should treat repeated
   `ARTIFACT_PIN_MISMATCH` errors as a soft circuit-breaker.

## Acceptance criteria for the wrapper to ship

- A new developer can copy-paste the example above and reach
  `result.verified === true` against devnet in under 15 minutes
  starting from `npm install`.
- No method in the public API mentions: merkleRoot, schemaHash,
  publicSignals, proofBuffer, ALT, nullifier preimage, or
  G2 endianness. (Internals can still mention them; the
  developer-facing API cannot.)
- Every `VerificationError` value has a unit test that triggers it
  and asserts the typed value + a `humanReadable` string.
- The wrapper has zero runtime dependencies on
  `scripts/prove.ts` or any other repo script -- it must be
  consumable as a published npm package.
- The wrapper passes `npm run e2e` end-to-end against the
  high-level call, not just the low-level `verifyOnChainV2`.

## What I'm asking from you

If you sketched the same SDK before reading this, send me your
sketch. We compare. The two interesting axes:

- **Where I might be wrong.** The `VerificationError` enum, the
  `Predicate` shape, the `ActionScope` shape -- any of these could
  be cleaner than what I wrote.
- **Where I might be over-scoped for v1.** `qrCodeTransport`,
  `solid.health()`, `previewDisclosure` -- could any of these
  defer to v1.1 without losing the 15-minute integration claim?

Once we agree on the shape, the implementation order is:
1. Resolved `Requirement` + public-input encoder (1-2 days).
2. `verifyProof` wrapping `verifyOnChainV2` + typed errors (2-3 days).
3. `requestProof` over wallet-adapter transport (2 days).
4. `defineRequirement` + `verifyRequirement` convenience (1 day).
5. Holder-side `canSatisfy` + `previewDisclosure` (3 days).
6. Health probe + docs + example app (2-3 days).

Total: ~2 weeks of focused product work to ship the wrapper that
unlocks every demo, video, and partner pilot in
`plan/PRODUCT_SURFACE_DEVNET_LAUNCH_PLAN.md`.
