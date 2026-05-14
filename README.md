# SolID Protocol

**Private credential verification for Solana apps.**

SolID lets a Solana app privately check whether a wallet satisfies an
issuer-signed eligibility claim -- KYC, accreditation, residency, age,
membership, jurisdiction -- without the app ever receiving the underlying
data. The check is a Groth16 proof verified by an on-chain program, with
revocation-aware issuer trust roots, schema governance, and one-shot
nullifier replay protection.

   [Simulator](https://app.solidislive.com) | 
   [Indexer](https://api.solidislive.com) | 
   [Circuit artifacts](https://artifacts.solidislive.com) | 
   [Manifest](https://api.solidislive.com/v1/manifest)

## What problem this solves

Solana apps that gate participation today have four bad options:

1. Collect and store PII themselves (regulatory exposure).
2. Outsource to a centralized KYC iframe (still hold a token; centralized
   trust).
3. Maintain an allowlist (operationally painful; non-portable).
4. Use a non-private soulbound token (privacy is gone).

SolID gives them a fifth option:

> A Solana program privately verifies an issuer-signed claim and learns
> only `eligible: true` plus a few public predicate parameters.
> The user's name, ID number, address, and date of birth never leave the
> holder.

## Where SolID fits

Four roles, four products:

| Role     | Question they need answered                                | Today's answer                                                                                |
| -------- | ---------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| Verifier | "Can this wallet do this action right now?"                | Submit a holder-generated proof to `verify_batch_proof_v2`; check `verified: true`.           |
| Issuer   | "Can I issue and revoke a credential for this user?"       | `register_issuer` (with on-chain BJJ subgroup proof) + DAO approval + `issue_credential` CPI. |
| Holder   | "What am I revealing, and what stays private?"             | Hold credential locally; generate a Groth16 proof scoped to the verifier and a query.         |
| Operator | "Are the trust roots and artifacts what they should be?"   | All program IDs + VK + artifact hashes pinned and CI-gated; see `docs/DEVNET_STATUS.md`.      |

Today the devnet protocol, public artifact host, indexer/API, manifest,
and `solid-sim` product surface are live under `solidislive.com`.
Verifier SDK publication is the next external-developer milestone.


## How a verification flow looks today

App developers should start with the high-level verifier SDK. It builds a
typed requirement, requests a holder proof through a transport, submits the
proof on-chain, and returns a stable success/failure result:

```ts
import { SolidVerifier, walletAdapterTransport } from "@solid-protocol/verifier";

const solid = new SolidVerifier({
  cluster: "devnet",
  artifactHostUrl: "https://artifacts.solidislive.com",
  indexerUrl: "https://api.solidislive.com",
});

const result = await solid.verifyRequirement({
  wallet: wallet.publicKey,
  payer: verifierPayer,
  spec: {
    schema: "basic_identity_v2",
    predicates: [
      { field: "verification_level", op: ">=", value: 2 },
      { field: "country_code", op: "!=", value: 840 },
    ],
    action: { appId: "my-launchpad", action: "join_pool_42" },
  },
  transport: walletAdapterTransport(wallet),
});

if (result.verified) allowUser();
```

This is the verifier alpha path. The hosted API and artifacts are live,
but verifier SDK npm publication is still pending.

## Quick start

Prerequisites: the toolchain pinned by `flake.nix` and
`scripts/bootstrap.sh` provisions `.toolchain/bin/`. You need
exactly:

- Rust 1.79.0 + `wasm32-unknown-unknown`
- Solana CLI 1.18.22, Anchor 0.30.1
- circom 2.1.9, snarkjs 0.7.5, wasm-pack 0.13.1
- Node 18 + npm 10

```
nix develop                    # or: bash scripts/bootstrap.sh
export PATH="$PWD/.toolchain/bin:$PATH"

# Build
cd circuits && npm install && node scripts/setup.js && cd ..
wasm-pack build wasm/ --target nodejs --out-dir ../ts-sdk/packages/core/wasm --release
bash scripts/sync_program_keypairs.sh
anchor build
cd ts-sdk && npm ci && npm run build && cd ..

# E2E (clean-slate localnet)
COPYFILE_DISABLE=1 COPY_EXTENDED_ATTRIBUTES_DISABLE=1 \
  solana-test-validator --reset \
    --clone-upgradeable-program cmtDvXumGCrqC1Age74AVPhSRVXJMd8PJS91L8KbNCK \
    --clone-upgradeable-program noopb9bkMVfRPU8AsbpTUg8AQkHtKwMYZiFUjNRtMmV \
    --url https://api.devnet.solana.com &
bash scripts/sync_program_keypairs.sh --reset-state
anchor deploy --provider.cluster localnet
export SOLID_VOTING_PERIOD_SECONDS=120
npm run e2e
```

`npm run e2e` runs `init-onchain` -> `backfill-issuer-tree` ->
`bootstrap-schema-tree` -> `bootstrap-issuer` -> `issue` -> `prove`
end-to-end and exits 0 with a final `verified: true` log line on
the local validator.

## Architecture

Layered top to bottom; each layer has exactly one source of truth.

```
Apps          Eligibility-gated Solana apps
              (DeFi, launchpads, DAO membership, gated mints)

TS SDK        @solid-protocol/sdk          (unified facade + manifest helpers)
              @solid-protocol/verifier     (SolidVerifier + on-chain verification)
              @solid-protocol/issuer       (issueCredential + generateSubgroupProof)
              @solid-protocol/holder       (Groth16 fullProve + Merkle adapters)
              @solid-protocol/light        (SPL AC adapter; PDA derivers)
              @solid-protocol/core         (WASM-built Poseidon / BJJ / nullifier)

Programs      zk-verifier                  Groth16 verify; nullifier PDA replay reject
              issuer-registry              DAO stake/vote, on-chain BJJ subgroup proof,
                                           SPL AC issue_credential, atomic revoke
              schema-registry              Schemas + SchemaTreeBinding + GlobalStateBinding

Circuits      batch_credential_query       32 public inputs; ADR-0014 issuer-tree binding
              bjj_subgroup_proof           Phase E live ceremony; 2 public inputs

Foundation    SPL Account Compression      concurrent Merkle trees (depth 20 / 16)
              groth16-solana               alt_bn128 syscall verification
              circomlib                    audited circuit primitives
```

Programs and circuits are the contract; everything else is a
consumer. See `CLAUDE.md` for the hard invariants and
`docs/CURRENT_STATE.md` for the per-component snapshot.

## Cryptographic primitives (for protocol integrators)

- **Groth16 over alt_bn128.** On-chain verification via Solana's
  `alt_bn128_*` syscalls. G2 elements use `(imag, real)` ordering
  per the syscall ABI; SDKs translate snarkjs's `(real, imag)`
  output (SOLID-SEC-067).
- **BabyJubJub EdDSA-Poseidon, cofactor-8.** Off-chain sign produces
  `S = r + h * 8 * sk`; the in-circuit `EdDSAPoseidonVerifier` checks
  `S * Base8 == R8 + h * 8 * A` (SOLID-SEC-053).
- **Poseidon hashes.** 6-input nullifier preimage
  `Poseidon(masterKey, revNonce, verifierAddr, queryCtxHash,
  verifierNonce, issuerTreeRoot)` (ADR-0006 revision); 5-input
  schema hash; 5-input credential commitment; 2-input Merkle.
- **Nullifier replay protection.** One PDA per proof, init-only.
  Atomic O(1).
- **Owner-checks.** The on-chain verifier rejects any `global_tree`,
  `schema_tree_N`, or `issuer_tree_binding` account whose owner is
  not the expected registry program. This is what makes the trust
  roots forge-resistant (ADR-0014).
- **VK rotation.** ADR-0015 freeze gate + 48-hour timelock; chunked
  upload mirror for both the batch and subgroup verification keys.

## Trust model

- **DAO-governed issuers.** Issuers stake SOL, are admitted via
  token-weighted vote with a 100-slot flash-loan window, and prove
  on-chain that their BabyJubJub key sits in the prime-order subgroup
  before they can issue (Phase E close-out, 2026-05-02).
- **Atomic revocation.** `revoke_issuer_atomic` flips status, bumps
  the revocation nonce, CPIs `replace_leaf` into the SPL AC issuer
  tree, and writes the new Poseidon root into `IssuerTreeBinding`
  in a single instruction. Old proofs become un-replayable
  immediately because their nullifier universe is keyed on the
  pre-revocation `issuerTreeRoot`.
- **Slashing.** Lamports move atomically from the issuer's stake
  vault to the DAO treasury PDA via `slash_issuer` and
  `submit_fraud_proof`. (Both will be governance-multisig-gated
  before mainnet; SOLID-SEC-013.)
- **Voting discipline.** `vote_on_issuer` enforces the deadline,
  increments `active_votes_count`, and refuses unstake until
  `release_vote` is called -- voters cannot withdraw governance
  weight to a winning side mid-vote.
- **Backend-agnostic verifier.** The verifier reads trust roots by
  byte offset and rejects accounts whose owner is not the expected
  registry program. The SPL AC backend can be swapped without a
  circuit change.

## License

[MIT](LICENSE-MIT)
at your option.
