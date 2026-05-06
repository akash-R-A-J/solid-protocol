# SolID Devnet Status

Last updated: 2026-05-06 (public devnet protocol deploy in progress).

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
| Last live deploy  | 2026-05-06 partial protocol deploy  |
| Reproducible      | localnet e2e green on 2026-05-02; fresh devnet issue/prove/replay green on 2026-05-07 |

The `deployments/devnet.json` manifest in this repository declares
the canonical program IDs, PDAs, launch schema, tree bindings, and
toolchain pins for devnet. Public artifact, indexer, solid-sim, and
wallet URLs remain null until those services are hosted.

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

## What works today

### Devnet protocol state

The following public devnet state has been created with deployer
`Gdz9JLWUekrfnpT3fPu1SsWfas3b3zMhfC4frvV1QRNm`:

| Step                       | Status |
| -------------------------- | ------ |
| Program deployment (`schema_registry`, `issuer_registry`, `zk_verifier`) | green |
| `init-onchain` (registry + main VK + subgroup VK upload/finalize) | green |
| Issuer-tree binding + SPL Account Compression tree | green |
| `basic_identity_v2` schema + depth-20 schema tree | green |
| Issuer registration, DAO vote/finalize, issuer-tree enrollment | green |
| Credential issuance into `basic_identity_v2` tree | green |
| Local witness generation + local `snarkjs.groth16.verify` | green |
| On-chain `verify_batch_proof_v2` | green for fresh sample: `4gp49ttdgCeZeegBiE3LsJN3uBXhsHYCiQqQW58F9YjJYhbvhAd6eLRdKRedvnfP8v8eqZnT67b2X1oYZPsre6yE` |
| Replay test (nullifier PDA init constraint) | green: replay of the same proof was rejected as expected |
| Fresh issuance with current SDK/source | green: `gUFgazuUQnMu89rGMVSvS2ZfmBhUxWd2Ki9aCsECDaGDCe7GE5YB1tEEV2nz1NgCNCtp8KSsTRbWt5SNcXjbS3g` |

### SolID Sim state

`solid-console` has been reworked as `solid-sim` for local devnet testing.
It now has four sections: DAO, Issuer, Wallet, and Verifier. The Wallet
section creates/imports a simulator identity, derives real holder channel and
schema-bound BabyJubJub public keys, imports encrypted credential envelopes,
validates holder binding/commitment/subgroup/signature integrity, and generates
proofs only when real artifacts plus a Merkle proof indexer are configured.

### Clean-slate localnet

The full pipeline reaches `verified: true` end-to-end on a fresh
`solana-test-validator`:

| Step                       | Status |
| -------------------------- | ------ |
| `init-onchain` | green |
| `backfill-issuer-tree` | green |
| `bootstrap-schema-tree` | green |
| `bootstrap-issuer` | green |
| `issue` | green |
| `prove` | green |
| Replay test | rejects (expected) |

Reference green `verify_batch_proof_v2` tx (clean-slate localnet,
2026-05-02):
`5hckZo1xsRPrzt3obKG5cmVLV42psxdxTjhZzcJZjEyJAeJNo2wvxPLs6haJfSXNA327SZefbw2kz1qNM5k4WiHJ`.

Compute-unit baselines for all 21 instructions are pinned in
`tests/cu_baselines.json` and CI-gated at 1.10x via
`scripts/measure_cu.py` (workflow step "SOLID-SEC-046 CU regression
gate" appended to the `e2e_localnet` job).

## What is missing (public devnet rollout punch list)

These items are tracked in
[`plan/PRODUCT_SURFACE_DEVNET_LAUNCH_PLAN.md`](../plan/PRODUCT_SURFACE_DEVNET_LAUNCH_PLAN.md)
(product surface) and
[`plan/INTEGRATION_AND_TEST_STATUS_2026-05-02.md`](../plan/INTEGRATION_AND_TEST_STATUS_2026-05-02.md)
(test gaps).

Protocol-side: the fresh issue -> root sync -> Groth16 proof -> on-chain
verify -> replay rejection path is green on devnet for `basic_identity_v2`.
The remaining blockers before public tester onboarding are hosted artifacts,
the indexer/API, solid-sim deployment, public URLs in the manifest, and a
four-role solid-sim smoke. Mainnet blockers (multi-party trusted setup
ceremony, governance multisig on slash/fraud, multisig on the issuer-tree
operator) do **not** apply to devnet.

Product-side, in rollout order:

1. Host pinned circuit artifacts and publish their HTTPS base URL in
   `deployments/devnet.json`.
2. Deploy the indexer/API and publish its URL in the manifest.
3. Deploy solid-sim with the devnet manifest/artifact/indexer URLs.
4. Run a four-role solid-sim smoke: DAO grants issuer/schema permission,
   holder requests credential, issuer issues, wallet stores, verifier
   requests proof, holder proves, verifier verifies on-chain, replay fails.
5. Register the full launch schema set -- `basic_identity_v1`,
   `vaccination_v1`, `product_cert_v1`, `dao_membership_v1`, and
   `accredited_investor_v1` with canonical schema hashes, field
   indices, schema PDAs, and tree addresses. Current live smoke schema
   is `basic_identity_v2` because its credential tree depth matches the
   batch circuit (`TREE_DEPTH = 20`).
6. Hosted devnet defaults -- artifact CDN with SHA-256 pins,
   schema/issuer registry API, Merkle proof / indexer API, current
   issuer-tree root, and status ping endpoint.
7. `@solid-protocol/verifier` package publish -- high-level wrapper
   code now exists locally, but public devnet needs installable
   `@solid-protocol/verifier@0.2.0` and `@solid-protocol/sdk@0.3.0`
   plus devnet defaults.
8. `solid-wallet` devnet configuration -- wallet code now imports
   real holder credentials, stores them encrypted, handles proof
   envelopes, and calls `@solid-protocol/holder`; it still needs live
   indexer/artifact/tree config and a real issuer-issued credential
   package for E2E acceptance.
9. `solid-console` real control plane -- replace demo/simulation
   issuer, issuance, verifier, schema, and DAO/status flows with real
   devnet calls.
10. Issuer CLI or minimal dashboard so issuers do not need to run
   `bootstrap_issuer.ts` directly.
11. `examples/private-launchpad-gate` Next.js demo deployed to a
   public URL.
12. `docs/DEVNET_QUICKSTART.md`, `docs/VERIFIER_INTEGRATION.md`,
   `docs/ISSUER_GUIDE.md`, `docs/HOLDER_FLOW.md`,
   `docs/ERROR_CODES.md`.
13. 90-second product video + 5-minute developer quickstart video.

## Current live smoke values

These values are public devnet smoke-test state. They are not mainnet
configuration.

| Field | Value |
| ----- | ----- |
| Governance mint | `5PrqMfDCWqMWnX1WdqHUuaa3kx29NeGLJFazB2hXGDzu` |
| Registry config PDA | `7CVeXUKeiWBhkGUuvte89GVCkXJHnR5D2YNCLWvVgSmH` |
| Verifier config PDA | `G4jFquTUyeRqzvsnNwRdDRE3TeiGyzKSGZ9q4PbwJPmW` |
| VK storage PDA | `8qjhn8Ze3Mhj9xdgAajMVxKbnXxHhmY6nTQf5N5yG4Rf` |
| Global binding PDA | `68Twk6dXwbahut6VDv1wSRkQMdFZRaeTbhUMNPiaJM8o` |
| Global root | `610b2e31e33bc10e3e7e0cf0e6fbeaae0c23b42c24266f1bc3a80685e805ce01` |
| Issuer tree binding PDA | `ExJ2PLDf8qrDxYsYRJbuKNTJ38xPxt1USJpe7cdfgL6h` |
| Issuer tree address | `FajCWko9tc6dhdPtwrS5kfkehPoEV8pn5LdL4kLL7k7f` |
| Issuer tree root | `bfc02634df26b227601eabeccb97d6d79f39fb23ec19d7b62ec1565f669f5b27` |
| Smoke schema | `basic_identity_v2` |
| Smoke schema hash | `6b5014bf611a025a4693b196a517ece9f2d0672672a2eb38d7a50481474e6823` |
| Smoke schema PDA | `EmdhTJt5VcQ4FiTy9xitTBBNa3XaBvwXsAB34FJWkx4` |
| Smoke schema tree binding | `FiiYYq2gysBiwhVYptS5GkmKvMJETXNXthz9SrTciuDb` |
| Smoke schema tree | `4mhWLGb2KAtF1bY2mdrGb37xhAUpmRsL9bgzLRjE35sc` |
| Smoke schema tree root | `5304536b69350abe799a36ed8104efbfa66a5198f8ab6cef6bc3e6c48357b919` |
| Sample issuer account | `FEdBRzX1junyhKf5Co15XKtHeAezT8i49zXSK6cGCEwL` |
| Sample issuer authority | `GwqjvUSmPKeNnPSWzFBPnURGXVkpLAzdujMuPCEPMyNi` |
| Sample credential commitment | `69cf17f65415a057480ab7b9a84bba23ddae3d5764dafc5c3a9db8a97c476425` |
| Sample credential tx | `gUFgazuUQnMu89rGMVSvS2ZfmBhUxWd2Ki9aCsECDaGDCe7GE5YB1tEEV2nz1NgCNCtp8KSsTRbWt5SNcXjbS3g` |
| Sample verify tx | `4gp49ttdgCeZeegBiE3LsJN3uBXhsHYCiQqQW58F9YjJYhbvhAd6eLRdKRedvnfP8v8eqZnT67b2X1oYZPsre6yE` |
| Issuer/schema permission PDA | `4Eo32kQPu9mvVypVRM83FV76ZZa3RSe5LWBwgpZcxx5H` |
| Issuer/schema permission tx | `2x9CefTUgwfthgqob9NpLYMhL3prGsd88c1wrbLsrb6w4r2yxykeveFnpK4Sw2Rzm4XZhynJJ4XuCppJbVzFyX81` |

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
- Public npm packages are not published yet. The high-level verifier
  SDK exists locally, but outside integrators still need either npm
  publication or an explicit local-package install path for devnet
  pilots.
- The current public manifest still uses null URLs for artifacts,
  indexer, console, and wallet releases. External platforms can inspect
  the on-chain programs now, but independent holder/verifier use needs
  hosted artifacts and a Merkle proof indexer.
- The `basic_identity_v1` binding created during early deployment used
  a depth-16 credential tree and should not be used with the current
  batch circuit. Use `basic_identity_v2` for the current devnet smoke
  path until launch schemas are re-registered with depth-20 trees.
- The wallet proof path is real code, not a mock, but runtime proof
  generation depends on a live Merkle proof indexer, hosted artifacts,
  current tree roots, and a real holder credential package.
- The fresh smoke credential verifies on-chain, but public holder proving
  still depends on hosted artifacts plus the indexer/API.
- `solid-sim` has real DAO, issuer, wallet, and verifier paths in code, but
  public testing still depends on hosted artifacts, the indexer/API, and a
  fresh four-role smoke with current deployed programs.

## Reporting an issue

GitHub issues are the canonical channel. For privacy- or
cryptography-soundness reports, follow the disclosure process at
the top of `sec/SECURITY_REGISTRY.md`.
