# SolID — Deployment & End-to-End Testing Guide

> **Last refreshed:** 2026-04-20 (v0.2 — SPL Account Compression, Nix-pinned toolchain).

This guide walks you from a clean checkout to a verified proof on-chain,
for both **localnet** and **devnet**. Every command is idempotent; if
something already exists, the step is a no-op.

---

## 0. Prerequisites

The canonical toolchain is pinned by `flake.nix` and materialized by
`scripts/bootstrap.sh`. **Use the pinned versions below** — drift in any
one of them is the single biggest source of "works-on-my-machine"
failures we've seen in this codebase.

| Tool | Pinned version | Why |
|---|---|---|
| Rust | `1.79.0` | Writes v3 `Cargo.lock` (Anchor's bundled Cargo is pre-1.78 and cannot parse v4) |
| Solana CLI | `1.18.22` | `anchor-lang` 0.30.1 transitively depends on `solana-program` 1.18.22 |
| Anchor | `0.30.1` | Pinned in `Anchor.toml`; AVM must manage the active CLI |
| Node | `18` | ESM + fetch support, matches CI |
| pnpm / npm | pnpm 9+ or npm 10+ | Monorepo package manager for `ts-sdk/` |
| circom | `2.1.9` | Compile `.circom` → `.r1cs` + witness WASM |
| snarkjs | `0.7.5` | Powers-of-tau + Groth16 setup |
| wasm-pack | `0.13.1` | Build the WASM bridge for Node |

### Option A — Reproducible (recommended)

```bash
# If you use Nix:
nix develop

# Or without Nix (installs pinned tools into .toolchain/):
bash scripts/bootstrap.sh
export PATH="$PWD/.toolchain/bin:$PATH"

# Sanity:
solana --version           # → solana-cli 1.18.22
anchor --version           # → anchor-cli 0.30.1
circom --version           # → circom compiler 2.1.9
wasm-pack --version        # → wasm-pack 0.13.1
```

A VS Code Devcontainer at `.devcontainer/devcontainer.json` wraps the
same flake for one-click onboarding.

### Option B — Manual install

```bash
# Rust 1.79.0 (for v3 lockfile compatibility)
rustup toolchain install 1.79.0
rustup default 1.79.0
rustup target add wasm32-unknown-unknown

# Solana 1.18.22
sh -c "$(curl -sSfL https://release.anza.xyz/v1.18.22/install)"
export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"

# Anchor 0.30.1 via AVM
cargo install --git https://github.com/coral-xyz/anchor avm --force
avm install 0.30.1 && avm use 0.30.1

# Rest
npm i -g pnpm snarkjs@0.7.5 circom@2.1.9
cargo install wasm-pack --version 0.13.1
```

### Common toolchain errors

> **`lock file version 4 requires -Znext-lockfile-bump`** — Your host
> Rust (≥1.85) wrote a v4 lockfile; Anchor's platform-tools cargo only
> parses v3. `scripts/bootstrap.sh` guards against this; to re-fix by
> hand:
>
> ```bash
> rm -f Cargo.lock
> cargo +1.79.0 generate-lockfile
> anchor build
> ```
>
> Do **not** add a repo-wide `rust-toolchain.toml` pinning 1.79 — some
> transitive deps require edition2024. Using `cargo +1.79.0` only for
> `generate-lockfile` sidesteps that.

> **`feature edition2024 is required`** — Opposite problem: a too-old
> host Rust parsing a newer-dep manifest. Either upgrade host Rust and
> regenerate the lockfile with `cargo +1.79.0 generate-lockfile`, or
> bump the offending dep.

> **`anchor build` says `anchor-lang 0.30.1 and CLI 0.32.x don't match`**
> — You skipped `avm use 0.30.1`. Re-run the AVM lines above.

---

## 1. First-time build

From the repo root:

```bash
# 1a. Rust libraries + zk-verifier host tests
cargo test -p solid-core -p solid-light
cargo test -p zk-verifier --lib       # VkBuf parser + negate_g1 tests

# 1b. BPF build of the three Anchor programs
anchor build                          # produces target/deploy/*.so + IDLs

# 1c. TypeScript SDK
cd ts-sdk && npm ci && npm run build && cd ..

# 1d. WASM bridge (Node target)
wasm-pack build crates/solid-core --target nodejs \
  --out-dir ts-sdk/packages/core/wasm --release

# 1e. Circuits + Groth16 setup (one-time trusted-setup ceremony)
cd circuits && node scripts/setup.js && cd ..

# 1f. Native prover (separate workspace)
cd tools/solid-prover && cargo build --release && cd ../..
```

### Powers-of-tau

`circuits/scripts/setup.js` needs `powersOfTau28_hez_final_16.ptau` (or
bigger if `batch_credential_query.circom` grows past 2¹⁶ constraints).
Download it once from the Hermez reference ceremony:

```bash
mkdir -p circuits/ptau
wget -O circuits/ptau/pot16_final.ptau \
  https://hermez.s3-eu-west-1.amazonaws.com/powersOfTau28_hez_final_16.ptau
```

The setup script produces, per circuit:
- `circuits/build/<circuit>.r1cs`
- `circuits/build/<circuit>_js/` (witness WASM)
- `circuits/build/<circuit>_final.zkey`
- `circuits/build/<circuit>_vk.json`

---

## 2. Verify cross-language cryptographic agreement

**Before deploying, always run the cross-language vector check.** If it
fails, do not deploy — the circuit, Rust, and TS SDK disagree on the
commitment / nullifier contract. CI runs this on every push.

```bash
# Regenerate reference vectors from Rust.
cargo run --example gen_vectors -p solid-core

# Replay them through the TS SDK.
cd ts-sdk
npm run build -w @solid-protocol/core
npx ts-node ../tests/vectors/check_vectors.ts
```

Expected output:

```
✔ commitment matches Rust reference
✔ nullifier matches Rust reference

All cross-language vectors agree.
```

---

## 3. Program-ID consistency (hard gate)

Before any deploy, verify that `Anchor.toml` ↔ `declare_id!()` ↔
`deployments/*.json` all agree:

```bash
python3 scripts/check_program_ids.py
```

On drift, follow
[`PROGRAM_ID_RECONCILIATION.md`](./PROGRAM_ID_RECONCILIATION.md) — which
walks you through closing stale programs on devnet, redeploying against
the canonical keys, and regenerating the manifest with
`scripts/regen_devnet_manifest.py`.

---

## 4. Localnet deployment

```bash
# 4a. Start a local validator in one terminal.
solana-test-validator --reset

# 4b. Point the CLI at localnet and airdrop SOL.
solana config set --url localhost
solana airdrop 100

# 4c. Deploy all three programs.
anchor deploy --provider.cluster localnet

# 4d. Initialize the on-chain state.
npx ts-node scripts/initialize.ts --cluster localnet

# 4e. Upload the verification key.
npx ts-node scripts/store_vk.ts --cluster localnet \
  --vk-json circuits/build/batch_credential_query_vk.json
```

`scripts/initialize.ts` creates the `RegistryConfig`, `VerifierConfig`,
and the `GlobalStateBinding` PDA. It prints every PDA it derives so you
can pipe them into the next steps.

---

## 5. Devnet deployment

Identical to localnet except for the cluster and the wallet:

```bash
solana config set --url devnet
solana airdrop 2        # devnet faucet; may throttle

python3 scripts/check_program_ids.py     # hard gate
anchor deploy --provider.cluster devnet

npx ts-node scripts/initialize.ts --cluster devnet
npx ts-node scripts/store_vk.ts  --cluster devnet \
  --vk-json circuits/build/batch_credential_query_vk.json

# Regenerate the deployment manifest (records deployed sizes + upgrade authorities).
python3 scripts/regen_devnet_manifest.py
cat deployments/devnet.json
```

> The program IDs in `Anchor.toml` are the single source of truth. They
> match `declare_id!()` in every program and `PROGRAM_IDS` in
> `ts-sdk/packages/sdk/src/config.ts`. `check_program_ids.py` enforces
> this invariant in CI.

---

## 6. End-to-end smoke flow

The canonical happy path: an issuer is approved, a credential is issued,
a holder generates a proof, and the verifier program accepts it.

```bash
# 1. Approve an issuer (DAO-governed path).
npx ts-node scripts/register_issuer.ts
npx ts-node scripts/vote_issuer.ts         # repeat with several voters
npx ts-node scripts/finalize_issuer.ts

# 2. Create + bind a schema tree (one-time per schema, per issuer).
#    (Creates the SPL Account Compression tree and binds its root
#     into schema-registry::SchemaTreeBinding.)
npx ts-node scripts/create_schema_tree.ts --schema basic_identity_v1

# 3. Issue a credential (CPIs to SPL AC append under the protocol-owned
#    tree-authority PDA; emits CredentialIssued).
npx ts-node scripts/issue.ts \
  --schema basic_identity_v1 \
  --holder <HOLDER_PUBKEY>

# 4. Holder: generate a proof for "age >= 21 AND country == US".
npx ts-node scripts/prove.ts \
  --query compound_and \
  --holder-secret <path/to/holder.key>

# 5. Verifier: submit the proof on-chain.
npx ts-node scripts/verify_onchain.ts \
  --proof-json out/proof.json
```

The final script calls `verifyOnChain` from `@solid-protocol/verifier`,
which builds the Anchor-compatible instruction (see
`buildVerifyBatchProofIx`) and waits for confirmation. On success it
logs the transaction signature and the `CredentialVerified` event.

---

## 7. Unit & integration testing matrix

| Layer | Command | What it proves |
|---|---|---|
| Rust libs | `cargo test -p solid-core -p solid-light` | Poseidon, BJJ, commitment, nullifier, query evaluation, multi-cred packing, `SchemaTreeBinding` parsing |
| zk-verifier host | `cargo test -p zk-verifier --lib` | `VkBuf` parser (minimum size, max IC, overflow, truncation, stack budget), `negate_g1_point` involutivity |
| Prover | `cd tools/solid-prover && cargo test` | Native ark-circom Groth16 generation |
| Programs (BPF) | `anchor test` | Happy-path + failure-mode tests for all three programs (localnet) |
| SDK | `cd ts-sdk && npm test` | TS-side helpers + Anchor IX packing |
| Cross-language | `npx ts-node tests/vectors/check_vectors.ts` | Bytes agree across Rust / WASM / TS |
| Circuit | `cd circuits && npm run test` | Circom witness generation against known inputs |
| Program-ID drift | `python3 scripts/check_program_ids.py` | Anchor.toml ↔ declare_id ↔ deployments consistency |

All of these jobs run in `.github/workflows/ci.yml`, gated on the
bootstrapped toolchain.

---

## 8. Operational safety

- The `zk-verifier` program has a `paused` flag. If an incident is
  detected, the authority can flip it with `set_paused(true)`, blocking
  new proofs without touching already-recorded nullifiers.
- Authority transfer is a two-step process via `transfer_authority`.
  Rotate on a cold-key-only cadence.
- Nullifier PDAs are permanent. A slashed issuer's already-accepted
  proofs remain accepted; revocation happens in a separate path via the
  credential tree / identity state, not by invalidating nullifiers (see
  [`REVOCATION_DESIGN.md`](./REVOCATION_DESIGN.md)).
- `SchemaTreeBinding` carries a `status` byte (0 = active, 1 = frozen).
  Flipping to `frozen` blocks all proofs against that schema at the
  verifier without needing a program upgrade.

---

## 9. Common failure modes

| Symptom | Likely cause | Fix |
|---|---|---|
| `InvalidSchemaRootBinding` | Caller passed a `schema_tree_N` account that isn't the PDA owned by `schema-registry` for that schema. | Derive PDA from `(b"schema", schema_hash)` and pass the correct account. |
| `NullifierMismatch` | `public_inputs[0]` from the circuit ≠ the 5-arg nullifier argument. | Ensure TS regenerates `publicSignals[0]` from the proof and uses those exact bytes. |
| `InvalidVerifierAddress` | `verifierAddress` circuit input ≠ deployed program ID. | Run `python3 scripts/check_program_ids.py` and follow `PROGRAM_ID_RECONCILIATION.md` if it reports drift. |
| `ProofVerificationFailed` | VK mismatch or wrong public-input permutation. | Re-run `store_vk.ts` after any circuit change. The `*_vk.json` in use must match the `.zkey` the holder used for `fullProve`. |
| `Account already in use` on nullifier PDA | Replay attempt, or the same `(masterKey, revNonce, verifier, queryHash, verifierNonce)` tuple was used twice. | Regenerate `verifierNonce` per verification. |
| `InvalidTreeAuthority` on `issue_credential` | The passed `merkle_tree` wasn't created with the protocol-owned `tree-authority` PDA as its authority. | Create trees via `createCredentialTree` from `@solid-protocol/light` — it wires the authority correctly. |

---

## 10. Clean teardown

```bash
solana-test-validator --reset                      # wipes localnet
rm -rf circuits/build target ts-sdk/**/dist \
       ts-sdk/packages/core/wasm tools/solid-prover/target
# Keep tests/vectors/ — they're committed reference fixtures.
```
