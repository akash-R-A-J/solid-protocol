# SolID Devnet Status

Last updated: 2026-05-02 (post-Phase-E close-out).

This page is the public, machine-friendly source of truth for the
SolID devnet deployment. If a value here disagrees with code in the
repository, the repository wins -- file an issue and we will fix
this page.

For the full state-of-the-protocol audit see
[`sec/audits/2026-05-02_v0.6.1_post_phase_e_full_system_audit.md`](../sec/audits/2026-05-02_v0.6.1_post_phase_e_full_system_audit.md).

## Cluster

| Field             | Value                                |
| ----------------- | ------------------------------------ |
| Network           | devnet                               |
| Cluster RPC       | https://api.devnet.solana.com        |
| Last live deploy  | not yet (devnet manifest is staged)  |
| Reproducible      | localnet e2e green on 2026-05-02; devnet first-deploy is the next milestone (see "What is missing" below) |

The `deployments/devnet.json` manifest in this repository declares
the canonical program IDs, PDAs, and toolchain pins for devnet.
The `deployer.address` and `deployed_at` fields will be populated
by `scripts/regen_devnet_manifest.py` after the first sanctioned
devnet deploy.

## Program IDs

These are the only program IDs that should be trusted for SolID on
devnet. They match `Anchor.toml`, the `declare_id!` literals in
each `programs/*/src/lib.rs`, and `deployments/devnet.json`. CI
gates the three-source agreement via
`scripts/check_program_ids.py`.

| Program            | ID                                              |
| ------------------ | ----------------------------------------------- |
| `zk_verifier`      | `DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb`   |
| `issuer_registry`  | `5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx`  |
| `schema_registry`  | `4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1`  |

External dependencies (cloned from devnet for localnet runs):

| Program                  | ID                                              |
| ------------------------ | ----------------------------------------------- |
| SPL Account Compression  | `cmtDvXumGCrqC1Age74AVPhSRVXJMd8PJS91L8KbNCK`   |
| SPL Noop                 | `noopb9bkMVfRPU8AsbpTUg8AQkHtKwMYZiFUjNRtMmV`   |

## Verification keys (load-bearing)

Both circuits ship through the SEC-058 artifact pin chain
(`ts-sdk/packages/sdk/src/artifact_integrity.ts`). Pin sources, in
priority order: `SOLID_*_SHA256` env vars > sidecar `*.sha256`
files in `circuits/build/` > config defaults.

| Artifact                                  | SHA-256                                                            |
| ----------------------------------------- | ------------------------------------------------------------------ |
| Batch circuit verification key (`batch_credential_query`) | `8385b82b032f65e505c784b28486ca8bec7da3f3d4b97b82724e697734565146` |
| Batch circuit witness wasm                | `add8cb0390511405faf2ffb1213d3792b858c7b2082d5b4b92a84dcd627e61b4` |
| Batch circuit zkey                        | `7f43bbac249e8c913ac384ff4b007138d1ffb5bc489a16be42737598d96395e8` |
| Subgroup circuit verification key (`bjj_subgroup_proof`)  | `938ab39020f31156fa7e8fc230fc458adba5f13e08c64d41d9dbffdbc3643ce9` |
| Subgroup circuit witness wasm             | `2c00e5a455a3b6fe1dc2a8761737acf2911baf396aab70165de3abd2673608b0` |
| Subgroup circuit zkey                     | `ea401ea9cbeb9ef829be30e0080b83a3c82544af53367b95a17f24a75845bd4a` |

The batch VK was re-pinned 2026-04-28 after the SEC-050 padding-canonicality
constraint was added; the subgroup VK was first pinned 2026-05-02 from the
Phase E live ceremony. Bumping either VK requires a chunked rotation under
the 48-hour ADR-0015 timelock. See `CLAUDE.md` "Hard invariants".

## Public-input contract

`circuits/batch_credential_query.circom` emits 32 public inputs.
The on-chain `verify_batch_proof_v2` handler addresses them by
named index. Key indices, all enforced by static asserts in
`programs/zk-verifier/src/lib.rs`:

| Slot | Field                          |
| ---- | ------------------------------ |
| 0    | `nullifierHash`                |
| 1    | `globalRoot`                   |
| 2-5  | `merkleRoots[0..3]`            |
| 6-9  | `schemaHashes[0..3]`           |
| 10   | `issuerTreeRoot` (ADR-0014)    |
| 11-18 | predicate query indices/fields |
| 19-22 | predicate operators           |
| 23-26 | predicate values              |
| 27   | `numPredicates`                |
| 28   | `compoundLogic`                |
| 29   | `verifierAddress` (BE bytes)   |
| 30   | `verifierNonce`                |
| 31   | `currentTimestamp`             |

## What works today (clean-slate localnet e2e)

The full pipeline reaches `verified: true` end-to-end on a fresh
`solana-test-validator`:

| Step                       | Status |
| -------------------------- | ------ |
| `init-onchain` (registry + main VK + subgroup VK chunked upload + freeze) | green |
| `backfill-issuer-tree` (SPL AC tree + binding init) | green |
| `bootstrap-schema-tree` (per-schema SPL AC tree)    | green |
| `bootstrap-issuer` (register + DAO-approve + enroll, with on-chain Groth16 subgroup proof) | green |
| `issue` (CPI'd `issue_credential` into the schema tree) | green |
| `prove` (Groth16 fullProve + chunked-buffer upload + `verify_batch_proof_v2`) | green |
| Replay test (nullifier PDA init constraint)         | rejects (expected) |

Reference green `verify_batch_proof_v2` tx (clean-slate localnet,
2026-05-02):
`5hckZo1xsRPrzt3obKG5cmVLV42psxdxTjhZzcJZjEyJAeJNo2wvxPLs6haJfSXNA327SZefbw2kz1qNM5k4WiHJ`.

Compute-unit baselines for all 21 instructions are pinned in
`tests/cu_baselines.json` and CI-gated at 1.10x via
`scripts/measure_cu.py` (workflow step "SOLID-SEC-046 CU regression
gate" appended to the `e2e_localnet` job).

## What is missing (devnet rollout punch list)

These items are tracked in
[`plan/PRODUCT_SURFACE_DEVNET_LAUNCH_PLAN.md`](../plan/PRODUCT_SURFACE_DEVNET_LAUNCH_PLAN.md)
(product surface) and
[`plan/INTEGRATION_AND_TEST_STATUS_2026-05-02.md`](../plan/INTEGRATION_AND_TEST_STATUS_2026-05-02.md)
(test gaps).

Protocol-side: zero blockers. Mainnet blockers (multi-party
trusted-setup ceremony, governance multisig on slash/fraud, multisig
on the issuer-tree operator) do **not** apply to devnet.

Product-side, in rollout order:

1. `@solid-protocol/verifier` high-level wrapper -- one call
   `verifyRequirement({ wallet, schema, predicates, action })` that
   hides accounts, proof buffers, Merkle reconstruction, nullifiers,
   and artifact pins.
2. Hosted devnet defaults -- artifact CDN with SHA-256 pins,
   schema/issuer registry API, Merkle proof / indexer API, status
   ping endpoint.
3. Holder credential storage + claim/prove web app (today there is
   no canonical holder UI; see `docs/HOLDER_FLOW.md` once it is
   written).
4. Issuer CLI or minimal dashboard so issuers do not need to run
   `bootstrap_issuer.ts` directly.
5. `examples/private-launchpad-gate` Next.js demo deployed to a
   public URL.
6. `docs/DEVNET_QUICKSTART.md`, `docs/VERIFIER_INTEGRATION.md`,
   `docs/ISSUER_GUIDE.md`, `docs/HOLDER_FLOW.md`,
   `docs/ERROR_CODES.md`.
7. 90-second product video + 5-minute developer quickstart video.

## Sample issuer / schema (placeholders)

Will be populated once the devnet first-deploy lands. Each will
include:

- Issuer authority (Solana pubkey) + BabyJubJub pubkey + registry
  status + tree leaf index.
- Schema name + version + 5-input Poseidon hash + tree pubkey.
- Sample credential commitment for end-to-end testing.

## Known limitations on devnet

- Trusted setup is single-party. The multi-party ceremony
  (SOLID-SEC-012) is a mainnet blocker only; devnet artifacts are
  honestly labelled "TESTNET ONLY".
- The all-padding `[0,0,0,0]` proof acceptance edge case
  (SOLID-SEC-051) is a one-line constraint deferred to the next
  trusted-setup cycle. Soundness impact is bounded; honest holders
  do not produce all-padding proofs.
- The issuer-tree operator (`IssuerTreeBinding.operator`) is a
  single signer (SOLID-SEC-043). Acceptable for devnet; replaced by
  Squads 3-of-5 before mainnet.
- The high-level verifier SDK does not exist yet -- integrators
  consume the chunked-upload `verifyOnChainV2` orchestration in
  `ts-sdk/packages/verifier/src/index.ts` directly. This is the
  single biggest source of integration friction and is the next
  product priority.

## Reporting an issue

GitHub issues are the canonical channel. For privacy- or
cryptography-soundness reports, follow the disclosure process at
the top of `sec/SECURITY_REGISTRY.md`.
