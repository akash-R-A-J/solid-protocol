# SolID Protocol — Integration Test Suite

End-to-end tests that run the three Anchor programs against solana-test-validator
via the anchor-bankrun or litesvm harness. These tests replace the absence of
on-chain behavioural coverage noted by the 2026-04 audits.

## Running

Prerequisites (same as the root README):

- rust 1.79.0, anchor-cli 0.30.1, solana-cli 1.18.22
- circom 2.1.9, snarkjs 0.7.5, wasm-pack 0.13.1
- node 18, npm 10+

```
# From repo root
anchor build
cd circuits && npm install && node scripts/setup.js && cd ..
wasm-pack build crates/solid-core --target nodejs \
    --out-dir ts-sdk/packages/core/wasm --release
cd ts-sdk && npm ci && npm run build && cd ..

# Run integration suite
npm run test:integration
```

## Suite structure

Each test starts a fresh localnet, deploys all three programs, then exercises
one scenario end-to-end. The suite is organised so that a failure localises to
one specific instruction or account-ownership invariant.

| File | Scenario |
|---|---|
| `01_registry_init.test.ts` | initialize_registry with the 112-byte RegistryConfig. Fails if the space fix regressed. |
| `02_issuer_lifecycle.test.ts` | register_issuer, stake, vote_on_issuer, finalize_voting, release_vote, unstake_tokens. Verifies active_votes_count increments and voting deadline check. |
| `03_slash_transfers_lamports.test.ts` | slash_issuer moves lamports from stake_vault to dao_treasury. Regression gate for HIGH-01. |
| `04_schema_and_bindings.test.ts` | register_schema, initialize_tree_binding, initialize_global_binding, update_tree_root monotonicity (negative test included). |
| `05_issue_credential.test.ts` | issue_credential CPI into SPL AC, CredentialIssued event emitted. |
| `06_verify_happy_path.test.ts` | Generate real Groth16 proof with the fixed per-schema identity leaf, submit via verify_batch_proof, confirm a transaction. |
| `07_verify_replay_rejected.test.ts` | Resubmit the same proof, expect the nullifier PDA init to fail. |
| `08_verify_forged_global_tree_rejected.test.ts` | Craft a system-program-owned account with a valid globroot discriminator; assert owner-check rejects it. Regression gate for P0-2. |
| `09_verify_forged_schema_tree_rejected.test.ts` | Same as above for schema_tree_N. Regression gate for P0-2. |
| `10_verify_expired_credential_rejected.test.ts` | Supply expirationTimestamp strictly less than currentTimestamp, assert the new batch ExpirationChecker fails the proof. |
| `11_cross_language_vectors.test.ts` | Re-runs tests/vectors/check_vectors.ts inside the suite to keep Rust-TS byte agreement part of the same run. |

## How the harness talks to the programs

Tests use anchor-bankrun for speed (no external validator process).
Each file opens with:

```ts
import { startAnchor } from 'solana-bankrun';
import { Program, AnchorProvider } from '@coral-xyz/anchor';

const ctx = await startAnchor('.', [], []);
const provider = new AnchorProvider(ctx.banksClient as any, ctx.payer as any, {});
```

For proof generation, tests rely on the real WASM and zkey produced by
circuits/scripts/setup.js. In CI, the circuits job caches these artifacts so
the integration job does not re-run the trusted setup.

## What is explicitly NOT tested here

- Multi-party trusted setup (a physical ceremony).
- Mainnet-only behaviours (actual upgrade authority transfer to a multisig).
- Helius DAS adapter (requires an external API key, runs in a separate
  `test:integration:online` target).

These are covered in docs/DEPLOYMENT_AND_TESTING.md.
