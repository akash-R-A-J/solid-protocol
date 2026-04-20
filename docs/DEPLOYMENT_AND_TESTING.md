# SolID — Deployment & End-to-End Testing Guide

This guide walks you from a clean checkout to a verified proof on-chain, for
both **localnet** and **devnet**. Every command is idempotent; if something
already exists, the step is a no-op.

---

## 0. Prerequisites

| Tool | Min version | Why |
|---|---|---|
| Rust | 1.75 (stable) | Build crates + programs |
| Solana CLI | 1.18.x | `solana-test-validator`, `solana program deploy` |
| Anchor | 0.30.1 | Build & deploy Solana programs |
| Node | 18+ | Run the TS SDK and scripts |
| pnpm | 9+ | Monorepo package manager for `ts-sdk/` |
| circom | 2.1.x | Compile `.circom` → `.r1cs` + witness WASM |
| snarkjs | 0.7.x | Powers-of-tau + Groth16 setup |
| wasm-pack | 0.12+ | Build the `wasm/` crate for Node |

```bash
# Rust
curl -fsSL https://sh.rustup.rs | sh

# Solana CLI — pin to 1.18.22 because anchor-lang 0.30.1 pulls
# solana-program 1.18.22 transitively. Newer CLIs sometimes link-fail.
sh -c "$(curl -sSfL https://release.anza.xyz/v1.18.22/install)"
# Make sure the Solana binaries (including `cargo-build-sbf`) are on your PATH.
# Add to ~/.zshrc or ~/.bashrc:
export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"
solana --version          # → solana-cli 1.18.22 ...
cargo build-sbf --version # → solana-cargo-build-sbf 1.18.22 ...

# Anchor — use AVM so the CLI matches the repo's pinned 0.30.1.
cargo install --git https://github.com/coral-xyz/anchor avm --force
avm install 0.30.1 && avm use 0.30.1
anchor --version          # → anchor-cli 0.30.1

# Rest of the toolchain
npm i -g pnpm snarkjs circom
cargo install wasm-pack
```

> **If `anchor build` errors with `no such command: build-sbf`**, your Solana
> CLI is either missing or not on PATH. Re-run the two install lines above
> and confirm `cargo build-sbf --version` works from the same shell you run
> `anchor build` in.
>
> **If `anchor build` warns `anchor-lang 0.30.1 and CLI 0.32.x don't match`**,
> you skipped the `avm install 0.30.1 && avm use 0.30.1` step. The repo's
> `Anchor.toml` already declares `anchor_version = "0.30.1"`, but AVM must
> be the one managing the active CLI for that pin to take effect.
>
> **If `anchor build` errors with `lock file version 4 requires -Znext-lockfile-bump`**,
> your host Rust is ≥ 1.85 (which writes v4 lockfiles) but your Solana
> platform-tools cargo is pre-1.78 (can only parse v3). Anchor 0.30.1 was
> built against Solana 1.18.22, whose bundled cargo is pre-1.78 — so we
> must produce a **v3** lockfile. Fix:
>
> ```bash
> # Install Rust 1.79 once; it writes v3 by default.
> rustup toolchain install 1.79.0
> rm -f Cargo.lock
> cargo +1.79.0 generate-lockfile   # v3 lockfile
> anchor build                       # platform-tools cargo is happy
> ```
>
> Do **not** add a `rust-toolchain.toml` that forces the whole repo to 1.79
> — some transitive deps require `edition2024` (Rust 1.85+) to parse. Using
> `cargo +1.79.0` just for `generate-lockfile` sidesteps that because the
> pinned deps we already ship in `Cargo.toml` are pre-edition2024; the
> resolver never reaches the newer alternatives.
>
> **If `anchor build` errors with `feature edition2024 is required`**, you
> have the opposite problem: a too-old host Rust is trying to parse a dep
> manifest that requires 1.85+. Either (a) remove `rust-toolchain.toml` and
> let your system Rust (≥1.85) handle `cargo metadata`, then regenerate the
> lockfile via `cargo +1.79.0 generate-lockfile` as above, or (b) bump the
> dep that pulled in the edition2024 crate.

---

## 1. First-time build

From the repo root:

```bash
# 1a. Compile all on-chain programs + crates (Rust).
cargo check --workspace                 # fast sanity check, no errors expected
anchor build                            # produces target/deploy/*.so + IDLs

# 1b. Compile the TypeScript SDK monorepo.
cd ts-sdk
pnpm install
pnpm -r build                           # builds core, holder, issuer, verifier, light

# 1c. Build the WASM bridge (Node target).
cd ..
wasm-pack build wasm --target nodejs --out-dir ts-sdk/packages/wasm/pkg

# 1d. Compile the circuits + Groth16 setup (one-time ceremony).
cd circuits
pnpm install                            # installs circomlib
node scripts/setup.js                   # see "Powers-of-tau" below
```

### Powers-of-tau

`circuits/scripts/setup.js` needs `powersOfTau28_hez_final_16.ptau` (or bigger
if your `compound_query.circom` grows beyond 2¹⁶ constraints). Download it
once from the Hermez reference ceremony:

```bash
mkdir -p circuits/ptau
wget -O circuits/ptau/pot16_final.ptau https://hermez.s3-eu-west-1.amazonaws.com/powersOfTau28_hez_final_16.ptau
```

Then the setup script will produce, per circuit:
- `circuits/build/<circuit>.r1cs`
- `circuits/build/<circuit>_js/` (witness WASM)
- `circuits/build/<circuit>_final.zkey`
- `circuits/build/<circuit>_vk.json`

---

## 2. Verify cross-language cryptographic agreement

Before deploying, always run the cross-language vector check. If it fails, do
not deploy — the circuit, Rust, and TS SDK will not agree on the proof
contract.

```bash
# Re-generate the reference vectors from Rust.
cargo run --example gen_vectors -p solid-core

# Replay them through the TS SDK.
cd ts-sdk
pnpm -F @solid-protocol/core build
npx ts-node ../tests/vectors/check_vectors.ts
```

Expected output:

```
✔ commitment matches Rust reference
✔ nullifier matches Rust reference

All cross-language vectors agree.
```

---

## 3. Localnet deployment

```bash
# 3a. Start a local validator in one terminal.
solana-test-validator --reset

# 3b. Point the CLI at localnet and airdrop SOL.
solana config set --url localhost
solana airdrop 100

# 3c. Deploy all three programs.
anchor deploy --provider.cluster localnet

# 3d. Initialize the on-chain state.
pnpm --filter scripts ts-node scripts/initialize.ts --cluster localnet

# 3e. Store the verification key produced by the circuit setup.
pnpm --filter scripts ts-node scripts/store_vk.ts --cluster localnet \
  --vk-json circuits/build/compound_query_vk.json
```

`scripts/initialize.ts` creates the `RegistryConfig`, the `VerifierConfig`,
and one `GlobalStateBinding` PDA. It prints every PDA it derives so you can
pipe them into the next steps.

---

## 4. Devnet deployment

Identical to localnet except for the cluster and the wallet:

```bash
solana config set --url devnet
solana airdrop 2        # devnet faucet; may throttle

anchor deploy --provider.cluster devnet
pnpm --filter scripts ts-node scripts/initialize.ts --cluster devnet
pnpm --filter scripts ts-node scripts/store_vk.ts  --cluster devnet \
  --vk-json circuits/build/compound_query_vk.json

# Record the deployed addresses.
cat deployments/devnet.json
```

> The program IDs in `Anchor.toml` match `PROGRAM_IDS` in
> `ts-sdk/packages/core/src/index.ts`. If you redeploy under a new key, update
> both files in the same commit.

---

## 5. End-to-end smoke flow

This is the canonical happy path: an issuer is approved, a credential is
issued, a holder generates a proof, and the verifier program accepts it.

```bash
# Approve an issuer (DAO-governed path).
pnpm --filter scripts ts-node scripts/register_issuer.ts
pnpm --filter scripts ts-node scripts/vote_issuer.ts    # repeat with several voters
pnpm --filter scripts ts-node scripts/finalize_issuer.ts

# Issue a credential to a holder.
pnpm --filter scripts ts-node scripts/issue.ts \
  --schema basic_identity_v1 \
  --holder <HOLDER_PUBKEY>

# Holder: generate a proof for "age >= 21 AND country == US".
pnpm --filter scripts ts-node scripts/prove.ts \
  --query compound_and \
  --holder-secret <path/to/holder.key>

# Verifier: submit the proof on-chain.
pnpm --filter scripts ts-node scripts/verify_onchain.ts \
  --proof-json out/proof.json
```

The final script calls `verifyOnChain` from `@solid-protocol/verifier`, which
builds the Anchor-compatible instruction (see
`buildVerifyBatchProofIx`) and waits for confirmation. On success it logs the
transaction signature and the `CredentialVerified` event.

---

## 6. Unit & integration testing matrix

| Layer | Command | What it proves |
|---|---|---|
| Rust crates | `cargo test --workspace` | Poseidon, BJJ, commitment, nullifier, query evaluation, multi-cred packing |
| Programs | `anchor test` | Happy-path + failure-mode tests for all three programs (localnet) |
| SDK | `pnpm -F ts-sdk test` | TS-side helpers + Anchor IX packing |
| Cross-language | `ts-node tests/vectors/check_vectors.ts` | Bytes agree across Rust / WASM / TS |
| Circuit | `cd circuits && npm run test` | Circom witness generation against known inputs |

---

## 7. Operational safety

- The `zk-verifier` program has a `paused` flag. If an issue is detected,
  the authority can flip it with `set_paused(true)`, blocking new proofs
  without touching already-recorded nullifiers.
- Authority transfer is a two-step process via `transfer_authority`. Update
  it on a cold-key-only cadence.
- Nullifier PDAs are permanent. A slashed issuer's already-accepted proofs
  remain accepted; revocation happens in a separate path via the credential
  tree, not by invalidating nullifiers.

---

## 8. Common failure modes

| Symptom | Likely cause | Fix |
|---|---|---|
| `InvalidSchemaRootBinding` | Caller passed a `schema_tree_N` account that is not the PDA owned by `schema-registry` for that schema. | Derive PDA from `(b"schema", schema_hash)` and pass the correct account. |
| `NullifierMismatch` | `public_inputs[0]` from the circuit does not equal the 5th arg. | Ensure TS regenerates `publicSignals[0]` from the proof and uses those exact bytes. |
| `InvalidVerifierAddress` | `verifierAddress` circuit input != program ID. | Set `verifierAddress = new PublicKey(PROGRAM_IDS.zkVerifier).toBytes()` — `@solid-protocol/holder` does this automatically now. |
| `ProofVerificationFailed` | VK mismatch or wrong public-input permutation. | Re-run `store_vk.ts` after any circuit change. The `compound_query_vk.json` in use must match the `.zkey` the holder used for `fullProve`. |
| `Account already in use` on `nullifier_record` | Replay attempt, or the same `(masterKey, verifierAddress, queryHash, verifierNonce)` was used twice. | Regenerate `verifierNonce` per verification. |

---

## 9. Clean teardown

```bash
solana-test-validator --reset         # wipes localnet
rm -rf circuits/build target ts-sdk/node_modules ts-sdk/**/dist tests/vectors
```
