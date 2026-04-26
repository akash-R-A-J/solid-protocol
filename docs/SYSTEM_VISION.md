> SolID Protocol — System Vision
> ============================
>
> Final-state outlook of the protocol as it will operate on mainnet,
> framed so every script, doc, and ADR in this repo can be checked
> against it.
>
> Status: aspirational. Items marked `(open)` are not yet in code.
> A side document, [`CURRENT_STATE.md`](./CURRENT_STATE.md), maps each
> bullet to its current implementation status.
>
> Conventions used here:
>
>   - `(closed)` — implemented, in CI, audited internally.
>   - `(impl)`   — implemented, lacks audit/regression coverage.
>   - `(open)`   — not yet in code; tracked elsewhere
>     (see [`IMPROVEMENTS_ROADMAP.md`](./IMPROVEMENTS_ROADMAP.md) and
>      ADR list).
>
> ASCII-only per CLAUDE.md.

# 1. What SolID solves

A regulated dApp on Solana wants to gate access on real-world identity
attributes (age, residency, accreditation, KYC level) without:

  1. learning *which* user it is gating;
  2. trusting any single off-chain identity provider;
  3. paying linear-in-attributes data costs on-chain.

Existing answers either leak the user's identity to the dApp (Civic,
Privado-style direct disclosures), trust a single oracle (Worldcoin
attestations on rollups), or rely on EVM-only proof systems that don't
fit Solana's compute-unit budget.

SolID's answer is a three-program stack:

  - `issuer-registry`  -- DAO-governed list of who can issue what,
                          with stake at risk and atomic revocation.
  - `schema-registry`  -- canonical attribute layouts and their
                          per-schema SPL Account Compression trees.
  - `zk-verifier`      -- Groth16 verifier over alt_bn128 syscalls,
                          owner-checking the trees + a 6-input
                          Poseidon nullifier for replay protection.

A holder accumulates credentials off-chain, generates a Groth16 proof
locally in WASM, and a verifier dApp consumes the proof in a single
Solana instruction. The dApp learns only the boolean answer and a
verifier-specific nullifier. No PII reaches the chain; no oracle is
in the trust path.

# 2. The four actors

```
     +------------------+         issues credentials       +-------------+
     |   Issuer (org)   |---------------------------------> |   Holder    |
     |  (KYC provider,  |  encrypted off-chain envelope     |  (end-user) |
     |   univ., bank)   |                                   |             |
     +--------+---------+                                   +------+------+
              |                                                    |
              | append commitment (CPI to SPL AC)                  | Groth16 proof
              v                                                    v
     +------------------+         verifies proof          +-------------+
     |     Solana       |<--------------------------------|  Verifier   |
     |   programs +     |   verify_batch_proof tx         |  (dApp)     |
     |   SPL AC trees   |                                 |             |
     +--------+---------+                                 +-------------+
              ^
              | revoke / admit / rotate VK
              |
     +--------+---------+
     |       DAO        |  Squads multisig OR token-staked
     |  (multisig+vote) |  governance, depending on phase
     +------------------+
```

## 2.1 DAO

Squads 3-of-5 multisig in the immediate term; token-staked governance
threshold in the longer term (post-audit).

Responsibilities:

  - Admit new issuers into the registry: review stake, signed
    application bundle, and on-chain `register_issuer` payload.
  - Revoke misbehaving issuers atomically (single-instruction:
    bump revocation_nonce, replace issuer-tree leaf, push new root).
  - Rotate the verification key of `zk-verifier` (48-hour timelock,
    freeze gate during rotation, see ADR-0015).

Identification of misbehaviour (real-world):

  - *Cryptographic proof.* Anyone presenting two `CredentialIssued`
    events signed by the same issuer authority that contradict each
    other on the same holder pubkey has irrefutable evidence of
    issuer-key compromise or fraud.
  - *Indexer anomalies.* A public indexer tracks per-issuer rate,
    schema mix, geography, stake history; visible spikes flag review.
  - *Out-of-band reports.* Holders, regulators, accreditors file
    evidence to an IPFS-pinned grievance document; a DAO member opens
    a revoke proposal that references the IPFS hash; voting period
    (longer than admission, suggested 7 days vs 24h) runs; on quorum,
    multisig executes `revoke_issuer_atomic`.

Per ADR-0014, `revoke_issuer_atomic` is one transaction that:

  1. Bumps `IssuerAccount.revocation_nonce` (this changes the
     issuer-tree leaf preimage).
  2. CPIs SPL AC `replace_leaf` under the singleton
     `issuer-tree-authority` PDA.
  3. Pushes the new issuer-tree root into the binding.

Because the 6-input Poseidon nullifier preimage includes
`issuer_tree_root`, every pre-revocation proof for that issuer's
credentials becomes nullifier-incompatible the moment the tree
moves -- there is no replay path through the old root.

## 2.2 Issuer

An organization with a wallet, an SPL stake position, and a
BabyJubJub EdDSA keypair whose public key is committed to the
issuer-tree leaf.

Responsibilities:

  - Run KYC / credentialing off-chain, in whatever form the schema
    says (passport scan, university registrar lookup, SOC-2 audit
    proof, etc.).
  - Sign the credential payload with the BJJ key.
  - Compute `commitment = Poseidon(issuer_pubkey || holder_pubkey ||
    schema_hash || attributes_hash || nonce)`.
  - Submit `issue_credential(commitment, schema_account, ...)` to the
    issuer-registry program; the program CPIs SPL AC `append` under
    the per-schema `tree-authority` PDA.
  - Hand the cleartext credential JSON + signature off to the holder
    over a secure channel.

Secure delivery channels (off-chain envelope):

  - HTTPS portal with OAuth login -- the standard case.
  - End-to-end encrypted message: the issuer encrypts the credential
    payload to a Curve25519 key the holder derives deterministically
    from their Solana seed phrase. Transport is whatever the issuer
    wants (signed download URL, Dialect/Light DM, even email).
  - In-person: signed QR code, short-lived TTL.

The on-chain `issue_credential` only writes a 32-byte commitment hash;
the cleartext never touches the chain. Compromise of any transport
layer reveals one credential, never the BJJ signing key.

## 2.3 Holder

An end-user with a Solana wallet plus a deterministically-derived
BabyJubJub EdDSA key. The BJJ key is the cryptographic anchor of
their identity; every credential commitment binds it.

SDK custody:

  - *Browser:* IndexedDB encrypted with a key derived from a fixed
    wallet-signed message (`wallet.signMessage("solid:vault:v1")` ->
    SHA-256 -> AES-GCM key). Resists XSS because the wallet signs each
    unlock.
  - *Mobile / desktop:* OS secure enclave (iOS Keychain, Android
    Keystore, macOS Keychain, libsecret on Linux).
  - *Recovery:* BJJ key is derived from the wallet seed via a fixed
    HD path. Restoring the wallet seed restores the BJJ key. Holders
    do not need a separate backup.
  - *Credentials:* JSON blobs alongside the BJJ key, indexed by
    `(issuer_pubkey, schema_hash)`.

Proof generation:

  - Inputs: credential plaintext + BJJ secret key + verifier_address
    + current_timestamp + an SPL-AC Merkle proof for the credential
    commitment.
  - Output: `(proof_a, proof_b, proof_c, public_inputs[32])`.
  - Cost: WASM-based, ~1-2 seconds on a modern laptop, no network.

**Coordinate contract (implementation).** Groth16 circuits and
`circomlib` serialize BabyJubJub points in **native** twisted Edwards
form (`a = 168700`). The Rust `solid-core` crate uses `ark-ed-on-bn254`
in an isomorphic **normalized** model (`a' = 1`). Every byte the
chain, the circuits, or the TS SDK sees is therefore in circomlib-
native `(x, y)` form; the SDK applies a fixed `sqrt(168700)` field map
at boundaries. Batch-circuit schema ordering and per-schema
`BabyPbk254` key derivation are regression-tested against the same
Rust ground truth the prover uses (`docs/CURRENT_STATE.md` §5.3).

The proof itself is *not* a secret. It reveals nothing about the
holder beyond the public inputs (root commitments, verifier address,
nullifier, current timestamp). It can be transported however the
verifier dApp prefers.

## 2.4 Verifier

A Solana dApp (or any service running a Solana RPC) that wants to
gate access on a SolID credential.

Submission options:

  - *Holder pays.* The holder's wallet signs `verify_batch_proof`
    and pays gas. Cleanest, but requires the holder to have SOL.
  - *Verifier pays.* The dApp accepts the proof bytes from the
    holder and submits `verify_batch_proof` itself, signing as fee
    payer. The proof's public-input slot for `verifier_address`
    binds the proof to that exact dApp, so the dApp can't replay
    a leaked proof elsewhere.
  - *Relayer pays.* A Helius/Triton-style relayer holds verifier
    funds, accepts proof submissions, and bills the verifier later.

In every case, only one signature is needed (the fee payer). The
holder's BJJ key never touches the chain.

# 3. On-chain footprint

## 3.1 PDAs

| PDA                    | Seed                                          | Owner program     | Purpose                                              |
| ---------------------- | --------------------------------------------- | ----------------- | ---------------------------------------------------- |
| `RegistryConfig`       | `["registry-config"]`                         | issuer-registry   | DAO config: voting period, min stake, gov mint.       |
| `GovernanceVault`      | `["governance-vault", registry_config]`       | issuer-registry   | SPL TokenAccount holding all stake.                   |
| `IssuerAccount`        | `["issuer", authority_pubkey]`                | issuer-registry   | Per-issuer state (status, BJJ pubkey, nonce, leaf).   |
| `IssuerTreeBinding`    | `["issuer-tree-binding"]`                     | issuer-registry   | Binds the singleton issuer-tree pubkey + last root.   |
| `IssuerTreeAuthority`  | `["issuer-tree-authority"]`                   | issuer-registry   | SPL AC append/replace_leaf signer for issuer-tree.    |
| `SchemaAccount`        | `["schema", name, [version]]`                 | schema-registry   | Schema definition; identifies the schema-tree.        |
| `SchemaTreeBinding`    | `["schema-tree-binding", schema_hash]`        | schema-registry   | Binds the per-schema tree pubkey + last root.         |
| `GlobalStateBinding`   | `["global-binding"]`                          | schema-registry   | Cross-schema commitment root (future-proofing).       |
| `TreeAuthority`        | `["tree-authority", schema_hash]`             | issuer-registry   | SPL AC append signer for *this* schema's tree.        |
| `VerifierConfig`       | `["verifier-config"]`                         | zk-verifier       | VK rotation state, generation counter, freeze gate.   |
| `VkStorage`            | `["vk-storage", verifier_config]`             | zk-verifier       | Active and (during rotation) staged VK bytes.         |
| `Nullifier`            | `["nullifier", nullifier_hash]`               | zk-verifier       | Per-spend uniqueness marker.                          |

## 3.2 SPL Account Compression trees

Two classes of tree:

  - **Issuer tree.** Singleton. Leaf = Poseidon(authority || bjj_x ||
    bjj_y || status_epoch || revocation_nonce). Authority is
    `IssuerTreeAuthority` PDA. Depth 16 by default (64K issuers).
  - **Per-schema credential trees.** One per `SchemaAccount`. Leaf =
    Poseidon-derived credential commitment. Authority is
    `["tree-authority", schema_hash]` PDA owned by issuer-registry.
    Depth 20 by default (1M credentials per schema).

Each tree is *bound* to its `*-tree-binding` PDA. The binding records
the tree pubkey at creation and is **immutable** (init-only).
This is intentional and load-bearing: the verifier owner-checks the
tree pubkey against the binding on every proof. If the binding could
be re-pointed, an attacker could swap in a tree they control.

## 3.3 Hard invariants

| Invariant                                                              | Enforced by                                              |
| ---------------------------------------------------------------------- | -------------------------------------------------------- |
| Anchor.toml + declare_id! + deployments/*.json all match               | scripts/check_program_ids.py (CI)                        |
| Rust + TypeScript primitives byte-for-byte                             | tests/vectors/ (CI; SOLID-SEC-010, currently 3/10 open)  |
| Verifier owner-checks global_tree                                      | zk-verifier::verify_batch_proof                          |
| Verifier owner-checks schema_tree_N                                    | zk-verifier::verify_batch_proof                          |
| Verifier owner-checks issuer_tree_binding (ADR-0014)                   | zk-verifier::verify_batch_proof                          |
| 6-input Poseidon nullifier (includes issuer_tree_root)                 | circuits/nullifier_*.circom + zk-verifier consts         |
| VK rotation 48h timelock + freeze gate (ADR-0015 part 1)               | zk-verifier::request/commit_vk_rotation                  |
| `vk_generation` bound into circuit publics (ADR-0015 part 2)           | (open) requires next setup ceremony                      |
| `verification_key.sha256` content-addressed gate                       | initialize.ts SOLID-SEC-041 gate                         |
| Squads 3-of-5 governance for issuer-tree-operator (SOLID-SEC-043)      | (open)                                                   |
| `request_withdrawal_atomic` mirrors `revoke_issuer_atomic`             | (open) SOLID-SEC-044                                     |
| Multi-party trusted setup ceremony                                     | (open) mainnet blocker                                   |

# 4. Five lifecycle flows

## 4.1 Issuer admission (DAO-governed)

```
issuer wallet                      DAO multisig                     chain
     |                                  |                            |
     |--register_issuer(stake, bjj_pk)-+>                            |
     |                                  |--vote yes/no--+>           |
     |                                  |              ...           |
     |                                  |--cast_vote--+>             |
     |                                  |--finalize_voting----------+>
     |                                  |--append_issuer_leaf-------+>  (CPI: SPL AC append)
     |                                  |--update_issuer_tree_root-+>
     |   <----- approved & enrolled ----+                            |
```

After this, the issuer's leaf is in the issuer tree. Their next
`issue_credential` call passes the issuer-tree owner-check.

## 4.2 Credential issuance (issuer to holder)

```
holder                       issuer                              chain
   |                            |                                  |
   |--bjj_pubkey, holder_sol--->|                                  |
   |                            |  (off-chain) sign credential,    |
   |                            |  compute commitment, hash attrs  |
   |                            |--issue_credential(commitment)---+>  (CPI: SPL AC append)
   |                            |--update_schema_tree_root-------+>
   |  <-----encrypted bundle---|                                  |
   |   (cred JSON + sig +       |                                  |
   |    SPL AC proof path)      |                                  |
```

Cleartext credential is delivered off-chain through the secure
channel of the issuer's choice. The chain only sees the commitment
and the new tree root.

## 4.3 Verification

```
holder SDK (browser)        verifier dApp                       chain
   |                            |                                 |
   |  generate Groth16 proof    |                                 |
   |  (32 publics)              |                                 |
   |--proof bytes via HTTPS---->|                                 |
   |                            |--verify_batch_proof-----------+>  (alt_bn128 syscalls)
   |                            |   - owner-check trees           |
   |                            |   - verify Groth16              |
   |                            |   - check timestamp + verifier  |
   |                            |   - mark Nullifier PDA          |
   |                            |<----- ok / err ----------------+|
   |  <----- access granted ----|                                 |
```

Proof transport: anything bytes-compatible (HTTPS POST is the default).
Submission: holder, verifier, or relayer pays gas; only one signer.

## 4.4 Issuer revocation (atomic)

```
DAO multisig                                                      chain
     |--revoke_issuer_atomic(issuer_pda)-----------------------+>
     |     (one ix performs all of):                            |
     |     (1) bump IssuerAccount.revocation_nonce              |
     |     (2) CPI: SPL AC replace_leaf with new preimage       |
     |     (3) push new root into IssuerTreeBinding             |
     |  <----- ok ---------------------------------------------+|
```

Effect: every Groth16 proof generated under the *old* issuer-tree
root has a nullifier preimage that no longer matches the on-chain
verifier's expected `issuer_tree_root`. Replay is impossible without
forging a Merkle path -- which the SPL AC root rejects.

## 4.5 Verifier-key rotation (DAO + 48h timelock)

```
DAO multisig                                                      chain
     |--request_vk_rotation(new_vk_chunk_0..N)-----------------+>
     |     (vk_finalized=false, freeze gate ON)                 |
     | ... 48h audit window ...                                 |
     |--commit_vk_rotation------------------------------------+>
     |     (vk_generation++, vk_finalized=true, freeze OFF)    |
```

ADR-0015 part 1 is closed (timelock + freeze gate).
ADR-0015 part 2 (binding `vk_generation` into the circuit's public
inputs to make cross-VK replay impossible) requires a circuit
revision and batches with the next trusted-setup ceremony.

# 5. Bootstrap sequence

The bootstrap sequence is the same on mainnet and localnet, with
just different governance and durations.

## 5.1 Mainnet (one-time, governed)

  1. DAO multisig deploys all three programs and issues
     `initialize_registry` (sets governance mint + voting period).
  2. DAO multisig calls `register_schema` for each well-known
     schema (basic_identity_v1, hospitality_v1, etc.).
  3. For each schema, the DAO runs the canonical "schema-tree
     bootstrap" sequence:
       a. Create a fresh tree keypair.
       b. `SystemProgram::createAccount` under SPL AC's program ID.
       c. SPL AC `init_empty_merkle_tree` (DAO multisig is temp
          authority for this single instruction).
       d. SPL AC `transfer_authority` -> `["tree-authority",
          schema_hash]` PDA owned by issuer-registry.
       e. `schema-registry::initialize_tree_binding(schema_hash,
          tree_pubkey)`.
  4. The DAO performs the singleton "issuer-tree bootstrap":
       a. Fresh keypair, createAccount, init_empty,
          transfer_authority -> `["issuer-tree-authority"]` PDA.
       b. `issuer-registry::initialize_issuer_tree_binding(tree_pubkey)`.
  5. DAO uploads the verification key (post-trusted-setup):
       a. `zk-verifier::initialize`.
       b. `zk-verifier::store_verification_key` in chunks until
          `is_finalized=true`.

After step 5 the protocol is live. Steps 3 and 4 are one-shot per
schema and per protocol-instance respectively. Once a binding is
created, its `tree_pubkey` is immutable.

## 5.2 Localnet (E2E test sequence)

The same sequence, packaged into idempotent scripts:

  - `npm run init-onchain`         (= `tsx scripts/initialize.ts`)
      Steps 1, 2, plus the parts of 5 that don't depend on a tree.
  - `npm run backfill-issuer-tree` (= `tsx scripts/backfill_issuer_tree.ts`)
      Step 4 in full, plus enroll any approved issuers.
  - `npm run bootstrap-schema-tree` (= `tsx scripts/bootstrap_schema_tree.ts`)
      Step 3 in full, for the active schema.
  - `npm run bootstrap-issuer`     (= `tsx scripts/bootstrap_issuer.ts`)
      Compresses the DAO-governed admission flow into one script.
  - `npm run issue` then `npm run prove` to exercise the runtime.

The crucial design rule, restated: **bindings are immutable**.
Scripts never write a placeholder binding "to be updated later",
because the contract correctly does not allow that. Each script
that touches a binding holds the real tree pubkey at the time it
calls `initialize_*_tree_binding`.

# 6. Where we are

See [`CURRENT_STATE.md`](./CURRENT_STATE.md) for the per-component
implementation map.

# 7. Anti-patterns explicitly avoided

  1. *Re-pointable bindings.* Would let an attacker swap trees.
     The verifier owner-check would still pass; the binding would
     point to a tree the attacker authored. Hard no.
  2. *On-chain credential plaintext.* Defeats the privacy guarantee.
  3. *Single-sig issuer-tree-operator.* Tracked SOLID-SEC-043.
  4. *VK upload without sha256 pin.* Catches stale builds; tracked
     SOLID-SEC-041 (closed in initialize.ts).
  5. *Defensive `try { initialize_binding } catch (already)` in
     scripts.* If a script tried to bind a placeholder pubkey and
     "fix it later", that "later" call would silently fail (the
     contract is init-only) and leave the system in a broken state.
     Scripts must hold the real pubkey at bind time.
