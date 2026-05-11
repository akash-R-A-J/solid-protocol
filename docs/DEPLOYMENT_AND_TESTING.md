# Deployment, End-to-End Testing, and Verification

v0.6.1 + post-2026-04-28 circuit/ZK audit close-out.  Canonical
runbook for the complete development loop: install toolchain,
build artifacts, deploy programs, run the E2E pipeline, and
**verify** that each invariant actually holds (not just that the
pipeline exited zero).

This doc is the reference. Every command is expected to succeed as
written. No workarounds. If a step fails, follow the
`## Troubleshooting` section rather than improvising.

**Mandatory process gate (added 2026-04-28; SOLID-SEC-052 / B11):**
any commit that touches `crates/solid-core/src/babyjubjub.rs` byte-
format helpers (`affine_to_pubkey`, `pubkey_to_affine`,
`affine_to_circomlib_xy`, `SQRT_A_LE` / `BASE8_X_ARK_LE` constants)
OR `wasm/src/lib.rs` MUST also re-emit the WASM bridge:

```bash
rm -rf ts-sdk/packages/core/wasm
PATH="$PWD/.toolchain/bin:$PATH" wasm-pack build wasm/ \
    --target nodejs --out-dir ../ts-sdk/packages/core/wasm --release
(cd ts-sdk && npm ci && npm run build)
```

Skipping this step silently breaks the off-chain<->on-chain wire
contract -- the WASM bridge ships pre-fix bytes while the on-chain
code expects post-fix bytes.  Reproducer: `npm run bootstrap-issuer`
fails at `[3/8] register_issuer` with `InvalidBJJPubKey` because
the on-chain consolation gate correctly rejects bytes in the
wrong coordinate form.  See `docs/E2E_BLOCKERS.md` B11.

**Live edge as of 2026-05-01:** CLOSED.  B13 / SOLID-SEC-054 closed
via Option 2 (buffer-account chunked upload: `init_proof_buffer` +
`upload_proof_chunk` + `verify_batch_proof_v2`).  `npm run e2e`
reaches `verified: true` end-to-end on localnet; the "Expected
terminal tail" block below describes the actual achievable tail.
Reference green tx:
`tVYvkyTt55r8RCf3LhVMTmBrKzr5HFtDJcwKM5tFHXerUaxmQSMsZQoKLR8HXbDMu2gYBjNwxC9gaSpdA1oMmCX`.

---

## 0. TL;DR (clean machine, localnet)

```bash
# 1. Install the toolchain (pinned versions into .toolchain/bin).
bash scripts/bootstrap.sh
export PATH="$PWD/.toolchain/bin:$PATH"

# 2. Build everything, in order.
cd circuits && npm install && node scripts/setup.js && cd ..
wasm-pack build wasm/ --target nodejs \
    --out-dir ../ts-sdk/packages/core/wasm --release
bash scripts/sync_program_keypairs.sh --reset-state    # hydrate keys + wipe state cache
anchor build                                            # SEC-048 closed (Phase E, 2026-05-XX); no `--features` flag
(cd ts-sdk && npm ci && npm run build)
npm install                                      # root (for tsx + scripts)

# 3. Boot a fresh validator and deploy.
#    macOS Sequoia: COPYFILE_DISABLE=1 + COPY_EXTENDED_ATTRIBUTES_DISABLE=1
#    suppress Apple's auto-applied `com.apple.provenance` xattr; without
#    these the genesis tarball self-verify rejects on round-trip.
#    `--clone-upgradeable-program` for SPL AC + Noop is required because
#    solana-test-validator 1.18.22 does not bundle them.
COPYFILE_DISABLE=1 COPY_EXTENDED_ATTRIBUTES_DISABLE=1 \
  solana-test-validator --reset \
    --clone-upgradeable-program cmtDvXumGCrqC1Age74AVPhSRVXJMd8PJS91L8KbNCK \
    --clone-upgradeable-program noopb9bkMVfRPU8AsbpTUg8AQkHtKwMYZiFUjNRtMmV \
    --url https://api.devnet.solana.com &
sleep 5
solana config set --url localhost
solana airdrop 10
anchor deploy --provider.cluster localnet

# 4. Run the E2E pipeline.  Voting period must agree across initialize.ts
#    + bootstrap_issuer.ts; 120s is comfortable on localnet.
export SOLID_VOTING_PERIOD_SECONDS=120
npm run e2e

# 5. Verify the result.
#    prove.ts asserts replay-rejection before exit; see section 7.
```

Expected terminal tail from `npm run e2e`:

```
[4/4] Submitting verify_batch_proof...
   verified: true
   tx:       5k4X...
Replay test (should fail)...
   ok (replay rejected by nullifier PDA init constraint)
Done.
```

If you got that output, the E2E succeeded. Section 7 shows how to
independently confirm correctness; section 8 shows how to probe the
security invariants (SEC-006 freeze-gate, SEC-007 BJJ subgroup check,
SEC-041 VK pin).

---

## 0.1 Current Devnet + Solid-Sim Testing Surface

As of 2026-05-07, the upgraded devnet programs are deployed and the
devnet smoke path has confirmed issuance, verification, and replay
rejection.  The canonical devnet program IDs are:

```text
schema_registry  4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1
issuer_registry  5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx
zk_verifier      DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb
```

For local testing against those deployed programs, run the indexer API
and Solid-Sim in separate terminals:

```bash
cd solid-protocol/indexer
PORT=8787 HOST=127.0.0.1 \
  SOLID_MANIFEST_PATH=/absolute/path/to/solid-protocol/deployments/devnet.json \
  SOLID_RPC_URL=https://api.devnet.solana.com \
  npm run start
```

```bash
cd solid-sim
VITE_SOLID_NETWORK=devnet \
  VITE_SOLID_RPC_URL=https://api.devnet.solana.com \
  VITE_SOLID_WS_URL=wss://api.devnet.solana.com \
  VITE_SOLID_MANIFEST_URL=http://127.0.0.1:8787/v1/manifest \
  VITE_SOLID_INDEXER_URL=http://127.0.0.1:8787 \
  VITE_SOLID_ARTIFACT_BASE_URL=/artifacts \
  npm run dev -- --host 127.0.0.1 --port 5173
```

The public devnet RPC is good enough for one-off smoke calls, but it
rate-limits the full console click-through. Use a private devnet RPC for
tester builds and set it in `SOLID_RPC_URL`, `VITE_SOLID_RPC_URL`, and
`VITE_SOLID_WS_URL`.

For the current public devnet deployment, these off-chain pieces are hosted:

```text
artifact host/CDN    all six pinned circuit artifacts from circuits/build/
indexer/API         solid-protocol/indexer as an HTTPS Node service
solid-sim frontend  built with the public manifest/indexer/artifact URLs
npm packages        @solid-protocol/* SDK packages after npm pack checks
manifest            deployments/devnet.json served over HTTPS or /v1/manifest
```

Keep private values out of Git and out of frontend `VITE_*` variables.
Use local shell env for deployer keypairs and host-provider secret
stores for RPC API keys, DB credentials, and `SOLID_INDEXER_WRITE_TOKEN`.

Known testing limitation: hosted artifacts and the indexer/API are live, but
the hosted browser four-role smoke still needs to be repeated before broad
public tester traffic. The indexer must have indexed issued credential leaves
before `/v1/merkle-proof/:tree/:leaf` can return inclusion proofs.

---

## 1. Prerequisites

### 1.1 Pinned toolchain versions

Exact match required. Anything newer drifts the produced artifacts
and breaks cross-language vector equality.

| Tool        | Pinned version | Reason                                                        |
|-------------|---------------|---------------------------------------------------------------|
| rustc       | 1.79.0        | `anchor build` platform-tools expects pre-1.78 / v3 Cargo.lock |
| solana-cli  | 1.18.22       | matches `Anchor.toml` `[toolchain]`                            |
| anchor-cli  | 0.30.1        | matches `Anchor.toml` `[toolchain]`                            |
| circom      | 2.1.9         | witness generator binary-compatibility                         |
| snarkjs     | 0.7.5         | zkey / ptau format stability                                   |
| wasm-pack   | 0.13.1        | matches the `wasm/` crate build                                |
| node        | 18 (or 20)    | npm 10+; node 24 works for tsx but may drift snarkjs bigint    |
| npm         | 10+           | workspaces support                                             |

### 1.2 Install options

Three ways to get the pinned versions on your machine. Pick one.

#### Option A -- scripts/bootstrap.sh (recommended outside Nix)

```bash
bash scripts/bootstrap.sh
export PATH="$PWD/.toolchain/bin:$PATH"
```

This downloads and pins every binary into `.toolchain/bin/`.
Idempotent: re-running is a no-op once everything is present.
`.toolchain/` is in `.gitignore`; it is per-checkout.

If you have rustup, set the channel:

```bash
rustup toolchain install 1.79.0
rustup target add --toolchain 1.79.0 wasm32-unknown-unknown
rustup override set 1.79.0    # inside the repo
```

#### Option B -- Nix flake

```bash
nix develop
```

All five tools plus rustc 1.79.0 are pre-installed in the shell.
Exits back to your normal shell when you `exit`. Best for
reproducibility; slowest first-time cache warm.

#### Option C -- manual

Follow each upstream's install guide to the exact version. Confirm
every version in the preflight step before building.

### 1.3 Preflight sanity

Run this every time you start a fresh session. Any "MISSING" line
means `npm run e2e` will fail later with a less readable error.

```bash
rustc --version          # expect 1.79.0
cargo --version          # expect cargo 1.79.0 (or +nightly fine)
solana --version         # expect solana-cli 1.18.22
anchor --version         # expect anchor-cli 0.30.1
circom --version         # expect 2.1.9
snarkjs --version 2>&1 | head -1   # expect snarkjs@0.7.5
wasm-pack --version      # expect wasm-pack 0.13.1
node --version           # expect v18.x (v20 / v22 usually fine; v24 sometimes drifts)
npm --version            # expect 10+
python3 --version        # 3.9+
```

Repo-level invariants that CI also enforces:

```bash
head -3 Cargo.lock | grep -E '^version = 3($|[[:space:]])' \
  && echo "Cargo.lock v3 OK" \
  || (echo "ERROR: Cargo.lock is not v3"; exit 1)

python3 scripts/check_program_ids.py
# Expected: Program IDs consistent across Anchor.toml, declare_id!, and deployments/.
```

---

## 2. Build artifacts

Run in this order. Each step depends on outputs from the previous.

### 2.1 Host-level cryptography tests

Prove the Rust + circuit primitives are internally consistent before
investing the deploy time. This is your first line of defence
against "localnet failing for a reason that was visible in a host
test 30 seconds earlier".

```bash
cargo test -p solid-core  --lib      # expect 49/49 passed
cargo test -p solid-light --lib      # expect 25/25 passed
cargo test -p zk-verifier --lib      # expect 17/17 passed
cargo fmt --all -- --check           # expect no output
```

Numbers in the right column are the green-gate snapshot at HEAD
after Phase 3 impl 4. They only move up when new regression gates
land; any decrease is a regression and should stop you here.

### 2.2 Circuits + trusted setup

```bash
cd circuits
npm install
node scripts/setup.js
cd ..
```

What this produces (all under `circuits/build/`):

```
batch_credential_query.r1cs           compiled R1CS
batch_credential_query_js/            witness generator (WASM)
batch_credential_query_final.zkey     Groth16 proving key
verification_key.json                 Groth16 VK (uploaded on-chain)
verification_key.sha256               SOLID-SEC-041 pin (hex sha256)
```

`setup.js` is **TESTNET only**. Single-party Powers of Tau; whoever
runs this script holds the toxic waste. SOLID-SEC-012 tracks the
multi-party replacement required before mainnet.

The final console line prints both hashes:

```
VK   sha256 : 9b8a4c...
zkey sha256 : 66fd2e...
```

Record the VK sha256 in release notes / ADR text. Operators set
`SOLID_VK_SHA256=<hex>` to the published value; `initialize.ts`
refuses any other VK bytes. See section 8.3.

### 2.3 WASM bridge

The bridge lives in the top-level `wasm/` crate (NOT in
`crates/solid-core`; SOLID-SEC-028 / ADR-0002).

```bash
wasm-pack build wasm/ --target nodejs \
    --out-dir ../ts-sdk/packages/core/wasm --release
```

Output: `ts-sdk/packages/core/wasm/` containing the `.wasm` binary
and the TypeScript declaration shim. The TS SDK imports from here
exclusively; re-running `wasm-pack build` after any change to
`wasm/src/lib.rs` or `crates/solid-core` is mandatory, otherwise
the SDK and the program disagree byte-for-byte.

`wasm-pack` resolves `--out-dir` relative to the crate being built
(`wasm/` here), so use `../ts-sdk/...` when running from the repo root.
Using `ts-sdk/...` writes to `wasm/ts-sdk/...`, which does not satisfy
the SDK import from `ts-sdk/packages/core/wasm/solid_wasm.js`.

### 2.4 Anchor programs

```bash
anchor build
```

Outputs under `target/deploy/`:

```
zk_verifier.so           + zk_verifier-keypair.json
issuer_registry.so       + issuer_registry-keypair.json
schema_registry.so       + schema_registry-keypair.json
```

If `anchor build` fails with "cannot parse Cargo.lock v4", you are
on a newer cargo than the Anchor 0.30.1 platform-tools expect.
`rm Cargo.lock && cargo +1.79.0 generate-lockfile` regenerates a
v3 lockfile in place.

### 2.5 TypeScript SDK + E2E scripts

```bash
# TS SDK workspace (6 packages: core / holder / issuer / verifier / light / sdk)
(cd ts-sdk && npm ci && npm run build)

# Repo-root E2E scripts (tsx + snarkjs + ffjavascript)
npm install
```

After this point every command in section 3-8 is runnable.

---

## 3. Deploy the programs

### 3.1 Localnet

```bash
# Tear down any prior state; start fresh.
pkill -f solana-test-validator 2>/dev/null; sleep 2
solana-test-validator --reset &
sleep 5

# Point the CLI at localhost and fund the deploy keypair.
solana config set --url localhost
solana airdrop 10

# Deploy all three programs.
anchor deploy --provider.cluster localnet
```

Expected output ends with three `Program Id:` lines matching the
canonical IDs pinned in `Anchor.toml`:

```
zk_verifier:       DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb
issuer_registry:   5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx
schema_registry:   4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1
```

Mismatch means one of `Anchor.toml`, `declare_id!`, or the freshly
generated keypair under `target/deploy/` drifted.
`python3 scripts/check_program_ids.py` will tell you which one.
Follow `docs/PROGRAM_ID_RECONCILIATION.md` for the fix.

### 3.2 Devnet (optional)

```bash
solana config set --url devnet
export SOLID_KEYPAIR_PATH="$HOME/.config/solana/solid-devnet-admin.json"
export SOLANA_KEYPAIR_PATH="$SOLID_KEYPAIR_PATH"
NO_DNA=1 anchor build --no-idl
npm run build:idl

solana program deploy target/deploy/schema_registry.so \
  --program-id target/deploy/schema_registry-keypair.json \
  --upgrade-authority "$SOLID_KEYPAIR_PATH" \
  --keypair "$SOLID_KEYPAIR_PATH" \
  --fee-payer "$SOLID_KEYPAIR_PATH" \
  --url devnet --use-rpc --max-sign-attempts 10
solana program deploy target/deploy/zk_verifier.so \
  --program-id target/deploy/zk_verifier-keypair.json \
  --upgrade-authority "$SOLID_KEYPAIR_PATH" \
  --keypair "$SOLID_KEYPAIR_PATH" \
  --fee-payer "$SOLID_KEYPAIR_PATH" \
  --url devnet --use-rpc --max-sign-attempts 10
solana program deploy target/deploy/issuer_registry.so \
  --program-id target/deploy/issuer_registry-keypair.json \
  --upgrade-authority "$SOLID_KEYPAIR_PATH" \
  --keypair "$SOLID_KEYPAIR_PATH" \
  --fee-payer "$SOLID_KEYPAIR_PATH" \
  --url devnet --use-rpc --max-sign-attempts 20

# Record the manifest for CI + README consumers.
python3 scripts/regen_devnet_manifest.py > deployments/devnet.json
python3 scripts/check_program_ids.py      # must stay green after every deploy
```

The manifest is the single source of truth for the devnet program
IDs and the block the deployment landed at. If
`check_program_ids.py` fails here, the deploy raced a separate
branch; resolve by picking one truth and updating the other.

---

## 4. Run the E2E pipeline

```bash
npm run e2e
```

The script fans out into five sequential steps defined in the
repo-root `package.json`:

```
1. tsx scripts/initialize.ts          -> registry + schema + VK
2. tsx scripts/backfill_issuer_tree.ts-> SPL-AC tree + IssuerTreeBinding PDA
3. tsx scripts/bootstrap_issuer.ts    -> register + approve + enroll
4. tsx scripts/issue.ts               -> issue_credential
5. tsx scripts/prove.ts               -> generate + verify + replay-reject
```

Run them individually with `npm run init-onchain`,
`npm run backfill-issuer-tree`, `npm run bootstrap-issuer`,
`npm run issue`, `npm run prove` if you want to stop between steps
(e.g., for the hackerhouse demo narration).

### 4.1 Step 1 -- `initialize.ts`

Creates on-chain state atoms the rest of the pipeline depends on.
Idempotent at the "already initialised" boundary for steps 1-5;
step 6 (store_verification_key) is **not** idempotent -- see
section 10 if you need to re-run.

Actions:

1. `initialize_registry` on issuer-registry (112-byte RegistryConfig).
2. `register_schema` on schema-registry (reference `basic_identity_v1`).
3. `initialize_tree_binding` (SchemaTreeBinding for step 4's tree).
4. `initialize_global_binding` (GlobalStateBinding).
5. `initialize` on zk-verifier (VerifierConfig, 60 bytes post
   SEC-006 Part 1).
6. **SEC-041 gate**: reads `circuits/build/verification_key.json`,
   computes sha256, requires match against `SOLID_VK_SHA256` env
   var (authoritative) or `circuits/build/verification_key.sha256`
   (developer loop). On mismatch, refuses.
7. `store_verification_key` in 900-byte chunks.

State file written to `$XDG_RUNTIME_DIR/solid-e2e/state.json` (or
`$TMPDIR/solid-e2e-$uid/state.json` on macOS). Mode `0600` per
SOLID-SEC-020.

### 4.2 Step 2 -- `backfill_issuer_tree.ts`

Creates the SPL Account Compression issuer tree (ADR-0014) and the
`IssuerTreeBinding` PDA. Enrolls every pre-approved issuer whose
`IssuerAccount.is_tree_enrolled == false`. On a fresh localnet this
is a no-op beyond tree creation because no issuers exist yet.

### 4.3 Step 3 -- `bootstrap_issuer.ts`

Registers one fresh issuer with a clean BJJ keypair, approves it via
the trust-anchor path (bootstrap shortcut; in production the DAO
vote path applies), appends its leaf to the issuer tree, and calls
`update_issuer_tree_root` so `IssuerTreeBinding.current_root`
matches the live tree.

### 4.4 Step 4 -- `issue.ts`

Issues one credential from the bootstrapped issuer to a freshly
derived holder identity. CPIs `spl_account_compression::append` via
the `[b"tree-authority", schema_hash]` PDA.

### 4.5 Step 5 -- `prove.ts`

Runs the complete verify + replay cycle:

1. Builds the query (`age >= 21` or similar) via QueryBuilder.
2. Seeds a LocalReplicaAdapter for the credential tree and the
   global-state tree.
3. Generates a real Groth16 proof over the
   `batch_credential_query_final.zkey` produced in step 2.2.
4. Submits `verify_batch_proof` to zk-verifier; captures the tx
   signature on success.
5. Re-submits the same proof; asserts the nullifier PDA init
   constraint rejects it.

Exit code is zero iff both the initial verify succeeded AND the
replay was rejected. Non-zero exit = at least one invariant failed.

---

## 5. Build + run: single copy-paste block

For operators who want one block to copy:

```bash
# One-time toolchain
bash scripts/bootstrap.sh
export PATH="$PWD/.toolchain/bin:$PATH"

# Per-checkout build
cd circuits && npm install && node scripts/setup.js && cd ..
wasm-pack build wasm/ --target nodejs \
    --out-dir ../ts-sdk/packages/core/wasm --release
anchor build
(cd ts-sdk && npm ci && npm run build)
npm install

# Per-run localnet
pkill -f solana-test-validator 2>/dev/null; sleep 2
solana-test-validator --reset &
sleep 5
solana config set --url localhost
solana airdrop 10
anchor deploy --provider.cluster localnet

# Run the pipeline
npm run e2e
```

---

## 6. Integration test suite (optional, complements E2E)

`tests/integration/` holds `anchor-bankrun` / `litesvm` scenario
tests. One of 11 scenarios is implemented at HEAD; SEC-B5 in the
registry tracks the expansion.

```bash
(cd ts-sdk && npm run test:integration)
```

Failures here are behavioural: a change to an Anchor program
semantically regressed a Phase 1 / Phase 2 / Phase 3 invariant.
Cross-check against `tests/integration/README.md` to identify which
scenario.

---

## 7. Verify the E2E actually worked (correctness gates)

"The script printed Done" is necessary but not sufficient. Each
check below independently confirms one invariant of the successful
run.

### 7.1 The proof was verified on-chain

```bash
# The tx signature printed by prove.ts (copy from its terminal output):
SIG="5k4X...copy..."

solana confirm -v "$SIG" --url localhost
```

Expected: `Confirmation: confirmed` and a non-empty log block
ending with `success`. Look for `Program log: Proof verified` or
equivalent.

Alternative: query the verifier config and confirm `proof_count`
incremented:

```bash
# Derive the VerifierConfig PDA.
VERIFIER_CONFIG=$(solana address \
  --seed 'verifier-config' \
  --owner DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb 2>/dev/null)

# Dump its data.
solana account "$VERIFIER_CONFIG" --output json | jq .account.data
```

The first 8 bytes are the Anchor discriminator; bytes [40..48) are
`proof_count` as little-endian u64. A post-E2E value of `1` means
one proof verified.

### 7.2 Replay rejection is real

`prove.ts` already re-submits the identical transaction and exits
non-zero if the second submission is accepted. If the terminal
showed:

```
Replay test (should fail)...
   ok (replay rejected by nullifier PDA init constraint)
```

that assertion passed. If you want to prove it independently, the
nullifier PDA seeds are `[b"null", nullifier_bytes]` under
`zk_verifier`. Running a third submission also fails with
`Error: account already in use` because the PDA is now occupied.

### 7.3 State file reflects the completed pipeline

```bash
# Find the state file.
ls -la "${XDG_RUNTIME_DIR:-${TMPDIR:-/tmp}}/solid-e2e/"
# Permission must be 0700 on dir and 0600 on state.json (SOLID-SEC-020).

# Print what got written.
cat "${XDG_RUNTIME_DIR:-${TMPDIR:-/tmp}}/solid-e2e/state.json" | jq .
```

Expected keys after all five steps: `schemaHash`, `schemaPda`,
`registryPda`, `schemaTreeBindingPda`, `globalBindingPda`,
`issuerTreeBindingPda`, `issuerMerkleTreeAddress`,
`verifierConfigPda`, `vkStoragePda`, `merkleTreeAddress`, plus the
issuer + holder keypairs and the issued credential.

### 7.4 IssuerTreeBinding.current_root matches the proof

```bash
# Read the IssuerTreeBinding PDA written by backfill_issuer_tree.ts.
ISSUER_TREE_BINDING=$(jq -r .issuerTreeBindingPda \
  "${XDG_RUNTIME_DIR:-${TMPDIR:-/tmp}}/solid-e2e/state.json")

# Dump its raw bytes.
solana account "$ISSUER_TREE_BINDING" --output json \
  | jq -r .account.data[0] | base64 -d | xxd | head -6
```

Per the ADR-0014 layout documented in
`programs/issuer-registry/src/lib.rs:85-99`:

```
[0..8)     discriminator  b"issrtree"
[8..40)    tree_pubkey    (SPL AC concurrent tree)
[40..72)   current_root   <-- this is what zk-verifier owner-checks
[72..80)   last_updated_slot
[80..81)   status
[81..113)  authority
```

The 32 bytes at offset 40 must equal `publicInputs[10]` in the
proof that was just accepted. `zk-verifier`'s handler asserts this
equality in-circuit (via `ISSUER_TREE_ROOT_INPUT_INDEX`); if it
weren't equal, step 5 would have failed with
`IssuerTreeRootMismatch`.

### 7.5 Program + program-ID green gate

```bash
python3 scripts/check_program_ids.py
# Expect: Program IDs consistent across Anchor.toml, declare_id!, and deployments/.
```

This is re-runnable at any time. Any drift would produce a loud
error and block all further steps.

---

## 8. Test the security invariants (negative tests)

Verifies the Phase 1 / 2 / 3 fixes are actually load-bearing, not
just compile-time placebos. Each subsection is one SEC-NNN fix;
each expected outcome is an error, not success.

### 8.1 SEC-007 -- BJJ subgroup check rejects cofactor-8 points

Register an issuer with a known 2-torsion point `(0, -1)`. The
on-chain program must reject with
`InvalidBJJPubKey`.

```bash
# Craft the bytes: x = [0u8; 32], y = -1 mod BN254_SCALAR (LE).
# Easiest: use the host-side test that already exercises this exactly.
cargo test -p solid-core --lib babyjubjub::tests::test_subgroup_rejects_order_two_point
# Expect: ok. 1 passed.
```

On-chain dry-run: craft a `register_issuer` tx with the same
malformed bytes and send it. Expected: tx fails with
`custom program error 0x17XX` and the simulation log includes
`BJJ public key is not in the prime-order subgroup`.

Pre-Phase-3, the honest register flow continues to succeed because
`solid_core::babyjubjub::generate_keypair` always produces
prime-order points. If step 3 (`bootstrap_issuer.ts`) succeeds,
the subgroup gate is enforced for the positive case.

### 8.2 SEC-006 Part 1 -- VK freeze-gate refuses post-finalize writes

Finalize the VK, then attempt a chunk-0 write. Expected:
`VerificationKeyFinalized`.

```bash
# After the E2E, craft a finalize + an illegal store_verification_key
# attempt from a small one-off script.  The fastest path is to dry-run
# it inside prove.ts's context; for this release the integration test
# 12_vk_rotation.test.ts (SEC-B5 scope) is the dedicated harness.

# Host-test gate that covers the invariant logic right now:
cargo test -p zk-verifier --lib vk_rotation
# Expect all 4 subtests ok:
#   vk_rotation_not_expired_when_no_pending_request ... ok
#   vk_rotation_not_expired_inside_window ... ok
#   vk_rotation_expired_at_and_beyond_timelock ... ok
#   vk_rotation_handles_saturation_safely ... ok
```

### 8.3 SEC-041 -- Content-addressed VK pin refuses a mismatched hash

Tamper with the VK JSON to force a hash mismatch. Expected:
`initialize.ts` refuses to upload.

```bash
# Make a copy of the real VK so we can restore after.
cp circuits/build/verification_key.json /tmp/vk_good.json
cp circuits/build/verification_key.sha256 /tmp/vk_sha_good.txt

# Corrupt the VK (append a whitespace char -- still parses JSON, breaks hash).
printf ' ' >> circuits/build/verification_key.json

# Try to re-run initialize.ts against a fresh validator.
pkill -f solana-test-validator 2>/dev/null; sleep 2
solana-test-validator --reset &
sleep 5
anchor deploy --provider.cluster localnet
npm run init-onchain
# Expect:
#    VK sha256 mismatch (SOLID-SEC-041 gate).
#      computed : <new_hash>
#      expected : <original_hash>
#      source   : circuits/build/verification_key.sha256
#    Refusing to upload.
# Exit code non-zero.

# Restore and proceed.
cp /tmp/vk_good.json circuits/build/verification_key.json
cp /tmp/vk_sha_good.txt circuits/build/verification_key.sha256
```

### 8.4 Replay rejection (Phase 1 hardened nullifier -> Phase 2 6-input)

Already covered by prove.ts step 5. To poke it independently:

```bash
# Submit the exact same verify_batch_proof tx a second time.
# prove.ts does this automatically; any script that re-sends the
# captured transaction raw-bytes will hit the same rejection.
```

### 8.5 Host-test regression sweep (quick all-in-one)

```bash
cargo test -p solid-core  --lib     # 49/49  (includes SEC-007 subgroup tests)
cargo test -p solid-light --lib     # 25/25
cargo test -p zk-verifier --lib     # 17/17  (includes SEC-006 timelock tests)
```

Any failure here means a Phase 1-3 fix regressed. Do not proceed
to E2E.

---

## 9. Demo narration (hackerhouse / pitch)

Optional. Useful if you want the run to tell a story rather than
print tx signatures. Three beats, each ~30 seconds:

1. **Issue** (`npm run issue`). "One issuer, one credential, signed
   with BabyJubJub EdDSA. The commitment lands in an SPL Account-
   Compressed Merkle tree."

2. **Prove** (`npm run prove`). "The holder generates a real
   Groth16 proof client-side. The on-chain verifier uses
   alt_bn128 syscalls; sub-10ms verification. Replay is
   mathematically rejected by a PDA-per-nullifier."

3. **Freeze** (optional; add a `finalize_verification_key` call).
   "Once the operator finalizes, the VK is immutable for 48 hours.
   A compromised authority cannot silently rotate; they'd have to
   broadcast a `request_vk_rotation` and wait the timelock out."

If you want beat 3 wired into the E2E automatically, ask me to
add the finalize call to `initialize.ts` -- 5 min change.

---

## 10. Troubleshooting

### 10.1 `npm run e2e` fails at initialize.ts

- **"verification_key.sha256 missing ..."**: you have not rebuilt
  circuits since the SEC-041 landing. Re-run
  `cd circuits && node scripts/setup.js`.

- **"VK sha256 mismatch"**: either the on-disk VK has drifted, or
  your pin is stale. Either re-run setup.js or export the
  authoritative `SOLID_VK_SHA256=<hex>` env var.

- **"already in use" on initialize_registry**: state from a prior
  run is still live. See section 11 (reset).

### 10.2 `npm run e2e` fails at store_verification_key

- **ChunkOutOfOrder**: the validator was not reset; a prior
  partial upload left `next_vk_chunk > 0`. Reset the validator
  (section 11), redeploy, re-run.

- **VerificationKeyFinalized**: you previously called
  `finalize_verification_key` on this validator instance. Reset.

### 10.3 `npm run e2e` fails at prove.ts

- **"Signature should verify ..."**: the circuit VK on-chain and
  the zkey on disk are out of sync. Happens when you re-ran
  `setup.js` after deploying the VK. Redeploy + re-initialize.

- **IssuerTreeRootMismatch**: `IssuerTreeBinding.current_root`
  lagged the live tree. Re-run step 2 + 3.

- **NullifierMismatch**: `public_inputs[0]` and the caller-
  supplied nullifier disagree. Almost always a WASM bridge / TS
  SDK version skew. Rebuild WASM (`wasm-pack build wasm/ ...`)
  then re-`npm run build` in `ts-sdk/`.

### 10.4 `anchor build` fails

- **"Cargo.lock version 4 not supported"**: regenerate the
  lockfile. Inside Nix: `rm Cargo.lock && cargo generate-lockfile`.
  Outside Nix: `rm Cargo.lock && cargo +1.79.0 generate-lockfile`.

- **"error: ... requires Rust 1.xx"**: your active toolchain is
  newer than 1.79.0 and some dep tightened its MSRV. Add
  `rustup override set 1.79.0` inside the repo.

### 10.5 Validator logs are empty / slow

Solana test validator writes to stderr. Redirect to inspect:

```bash
solana-test-validator --reset > /tmp/validator.log 2>&1 &
tail -f /tmp/validator.log
```

Per-program program logs after a tx:

```bash
solana logs DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb --url localhost
solana logs 5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx --url localhost
solana logs 4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1 --url localhost
```

---

## 11. Reset + iterate

Fresh run from zero:

```bash
# Kill any running validator.
pkill -f solana-test-validator 2>/dev/null; sleep 2

# Wipe the E2E state file so initialize.ts does not pick up stale PDAs.
rm -rf "${XDG_RUNTIME_DIR:-${TMPDIR:-/tmp}}/solid-e2e"

# Reboot.
solana-test-validator --reset &
sleep 5
anchor deploy --provider.cluster localnet
npm run e2e
```

The state file is recreated by `initialize.ts` with the new PDAs
from the fresh validator, and downstream steps pick up from it.

---

## 12. Public devnet deployment and exhaustive testing

This is the public-devnet operator runbook. Follow it top to bottom when
you want to deploy the complete system, link every deployed piece together,
and prove the deployed product works through the DAO, issuer, holder wallet,
and verifier roles.

The key idea: `deployments/devnet.json` is the wiring contract. Programs,
SDK consumers, `solid-sim`, the embedded console wallet, indexer, artifact
host, and external dApps should all read or mirror the same values from that
file. If something is not in the manifest or in an explicit `VITE_SOLID_*`
env var, assume it is not deployed.

Current green protocol status as of 2026-05-06:

- `issuer_registry` is upgraded on devnet at
  `5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx`.
- Last known upgraded slot for `issuer_registry`: `460541773`.
- Upgrade authority: `Gdz9JLWUekrfnpT3fPu1SsWfas3b3zMhfC4frvV1QRNm`.
- Issuer/schema permission is granted for the devnet smoke issuer.
- Fresh issue -> root sync -> Groth16 proof -> on-chain
  `verify_batch_proof_v2` -> replay rejection is green.
- Remaining public-deploy work is infrastructure and wiring:
  hosted artifacts, live indexer/API, published manifest, deployed
  `solid-sim` with its embedded wallet role, and full browser E2E.
- `solid-wallet` exists as a separate browser extension, but it is not required
  for the current deployment milestone. Treat it as optional/future distribution
  unless the release explicitly chooses to test the extension path.

### 12.1 What gets deployed

There are six deployable surfaces. They are different things. Treat them
separately.

| Surface | What it is | Output you deploy | Where it can live | Why it matters |
| --- | --- | --- | --- | --- |
| Solana programs | On-chain executable BPF programs. | `target/deploy/schema_registry.so`, `target/deploy/issuer_registry.so`, `target/deploy/zk_verifier.so` plus their program keypairs. | Solana devnet via `solana program deploy`. | This is the protocol state machine: schema registry, issuer governance/issuance, and proof verification. |
| Circuit artifacts | Proving/verifying files used by browsers and scripts. | Six files: two `.wasm`, two `.zkey`, two VK `.json` files, plus hash pins. | Vercel static at `https://artifacts.solidislive.com`. | Wallet and console proof flows download these files and verify SHA-256 before using them. |
| SDK packages | TypeScript packages consumed by console, wallet, scripts, and external apps. | Built `dist/` package contents, optionally npm packages. | Local monorepo `file:` deps for dev, npm registry for external integrators. | Keeps all clients using the same manifest parser, proof APIs, issuer APIs, and crypto bridge. |
| WASM bridge | Rust crypto compiled for JS. | `ts-sdk/packages/core/wasm/` and `ts-sdk/packages/core/wasm-web/`, especially `solid_wasm_bg.wasm` and JS bindings. | Included inside `@solid-protocol/core`, then bundled by console/wallet. | Keeps off-chain key, hash, and proof input generation byte-compatible with on-chain/circuit logic. |
| Indexer/API | Node HTTP API for registry reads, credential requests, issued-event ingestion, Merkle proofs, and the canonical devnet manifest. | `solid-protocol/indexer` service. | AWS Lightsail Ubuntu behind Nginx at `https://api.solidislive.com`. | Holder proof generation needs real Merkle paths. The console request inbox also uses this API. The manifest lives at `/v1/manifest`. |
| App | Public simulator UI with DAO, issuer, holder wallet, and verifier roles. | `solid-sim/dist`. | Vercel at `https://app.solidislive.com`. | This is what current testers use. The embedded wallet in `solid-sim` must point to the published manifest, artifact host, RPC, and indexer. |
| Optional extension | Standalone browser wallet. | `solid-wallet/dist`. | Unpacked ZIP, private Chrome Web Store listing, or controlled tester download. | Not required for the current deploy. Use only when testing external dApp injection through `window.solid`. |

### 12.2 Values you must decide before deploying

Do not make up placeholders for these. If one is unknown, stop at that step.

| Value | Example | Configure it in |
| --- | --- | --- |
| Devnet RPC URL | `https://api.devnet.solana.com` or paid RPC | `config/devnet.env.example`, deploy env, `VITE_SOLID_RPC_URL`, manifest `cluster` |
| Devnet WebSocket URL | `wss://api.devnet.solana.com` | `VITE_SOLID_WS_URL`, manifest `websocket_cluster` |
| Deployer keypair path | `$HOME/.config/solana/solid-devnet-admin.json` | `SOLID_KEYPAIR_PATH`, `SOLANA_KEYPAIR_PATH`, `ANCHOR_WALLET` |
| Artifact base URL | `https://artifacts.solidislive.com` | manifest `artifacts.base_url`, `VITE_SOLID_ARTIFACT_BASE_URL`, wallet settings |
| Manifest URL | `https://api.solidislive.com/v1/manifest` | `VITE_SOLID_MANIFEST_URL`, wallet settings, indexer `/.well-known/solid-protocol.json` if mirrored |
| Indexer URL | `https://api.solidislive.com` | manifest `indexer.url`, `VITE_SOLID_INDEXER_URL`, wallet settings |
| Console URL | `https://app.solidislive.com` | manifest `console.url`, `VITE_SOLID_CONSOLE_URL` |
| Wallet release URL | private ZIP or Chrome listing URL | manifest `wallet.release_url`; optional for this milestone because the wallet role is embedded in `solid-sim` |
| Tester origins | `https://solid-sim.example.com/*` | Only needed if deploying `solid-wallet`; configure `solid-wallet/public/manifest.json` `host_permissions`, `content_scripts.matches`, `web_accessible_resources.matches` |
| Indexer write token | random secret | `SOLID_INDEXER_WRITE_TOKEN` on the indexer host and issuer/operator ingestion commands |
| Schema launch set | `basic_identity_v2`, depth 20 | protocol env, manifest `schemas`, schema tree bindings |

For the public devnet rollout, use the `solidislive.com` domain family:

| Surface | URL |
| --- | --- |
| Landing page | `https://solidislive.com` |
| App / demo | `https://app.solidislive.com` |
| Indexer / API | `https://api.solidislive.com` |
| Artifact CDN | `https://artifacts.solidislive.com` |
| Manifest | `https://api.solidislive.com/v1/manifest` |
| Docs | `https://docs.solidislive.com` |

Deployment split:

- Vercel hosts the landing page, `solid-sim`, docs, verifier-SDK-facing static content, and the immutable artifact files.
- AWS Lightsail hosts only the mutable indexer/API service and serves the canonical devnet manifest at `/v1/manifest`.
- Public endpoints must be rate limited. Apply Nginx limits on `api.solidislive.com`, Vercel/WAF limits on app/docs/artifact routes where available, and stricter token-based protection on write endpoints.
- Public documentation must stay sanitized. Do not include AWS account IDs, raw static IPs, SSH key names or paths, private IPs, wallet keypair paths, write tokens, paid RPC URLs, `.env` contents, or credentials.

Current deployment state:

| Item | State |
| --- | --- |
| API instance | AWS Lightsail running the indexer/API under PM2 |
| API DNS | `https://api.solidislive.com` configured with Nginx, TLS, and rate limits |
| Manifest source | Live at `https://api.solidislive.com/v1/manifest` |
| Artifact host | Live on Vercel at `https://artifacts.solidislive.com` |
| App host | Live on Vercel at `https://app.solidislive.com` |
| Remaining rollout work | Hosted browser smoke, public landing/docs, verifier SDK publish, monitoring |

Generate the indexer write token locally:

```bash
openssl rand -hex 32
```

Why: the read API is public, but ingestion endpoints like
`POST /v1/tree-leaves` and `POST /v1/events/credential-issued` must not be
open to random browsers.

### 12.3 One-time preflight on the operator machine

Run from `solid-protocol`.

```bash
bash scripts/bootstrap.sh
export PATH="$PWD/.toolchain/bin:$PATH"
source config/devnet.env.example

rustc --version
solana --version
anchor --version
circom --version
snarkjs --version 2>&1 | head -1
wasm-pack --version
node --version
npm --version
python3 --version
```

Expected versions are listed in section 1. If `rustc`, `anchor`, or
`wasm-pack` is missing, do not continue. Program binaries and WASM artifacts
are release inputs; building them with a drifting toolchain makes debugging
miserable.

Then run the invariant checks:

```bash
python3 scripts/check_program_ids.py
npm run validate:devnet
npm run verify:artifacts
npm run test:prove-root-plan
```

What each check proves:

- `check_program_ids.py`: `Anchor.toml`, Rust `declare_id!`, and
  `deployments/devnet.json` agree on program IDs.
- `validate:devnet`: manifest shape and required program/artifact pins are valid.
- `verify:artifacts`: local artifact bytes match the SHA-256 pins in the manifest.
- `test:prove-root-plan`: root-update transaction planning still handles the
  compute-budget and root-refresh path used by `npm run prove`.

### 12.4 Build everything you may deploy

Run from `solid-protocol`.

```bash
export PATH="$PWD/.toolchain/bin:$PATH"

# 1. Build circuit artifacts.
cd circuits
npm install
node scripts/setup.js
cd ..

# 2. Build the Rust-to-JS WASM bridge used by SDK, console, and wallet.
wasm-pack build wasm/ --target nodejs \
  --out-dir ../ts-sdk/packages/core/wasm --release

# 3. Build Solana program binaries.
NO_DNA=1 anchor build --no-idl

# 4. Build IDLs consumed by scripts and clients.
npm run build:idl

# 5. Build and test SDK packages.
cd ts-sdk
npm ci
npm run build
npm test
npm run pack:check
cd ..

# 6. Install root script dependencies.
npm install

# 7. Build indexer package dependencies.
npm install --prefix indexer
npm test --prefix indexer
```

Expected deployable files:

```text
target/deploy/schema_registry.so
target/deploy/schema_registry-keypair.json
target/deploy/issuer_registry.so
target/deploy/issuer_registry-keypair.json
target/deploy/zk_verifier.so
target/deploy/zk_verifier-keypair.json

circuits/build/batch_credential_query_js/batch_credential_query.wasm
circuits/build/batch_credential_query_final.zkey
circuits/build/verification_key.json
circuits/build/bjj_subgroup_proof_js/bjj_subgroup_proof.wasm
circuits/build/bjj_subgroup_proof_final.zkey
circuits/build/bjj_subgroup_verification_key.json

ts-sdk/packages/*/dist/
ts-sdk/packages/core/wasm/
```

Verify the files exist and match the manifest:

```bash
npm run verify:artifacts
ls -lh target/deploy/*.so
```

If `npm run verify:artifacts` passes, the circuit artifacts exist at the
local paths recorded in `deployments/devnet.json` and their bytes match
the expected SHA-256 pins.

### 12.5 Deploy or verify the Solana programs

If the current devnet programs are already deployed and green, verify them
instead of redeploying:

```bash
solana program show 4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1 --url devnet
solana program show 5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx --url devnet
solana program show DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb --url devnet
```

Expected for each program:

- `Executable: Yes`
- `Upgradeable: Yes`
- upgrade authority equals `deployments/devnet.json`
- last deployed slot is at or after the release slot you intend to test

Current `issuer_registry` green check:

```bash
solana program show 5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx --url devnet
```

Expected:

```text
Program Id: 5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx
Authority: Gdz9JLWUekrfnpT3fPu1SsWfas3b3zMhfC4frvV1QRNm
Last Deployed In Slot: 460541773 or newer
Executable: Yes
```

If you need to deploy or upgrade, use one-program deploys. They are easier
to resume than a bulk `anchor deploy`, especially on public devnet RPC.

```bash
solana config set --url devnet
export SOLID_KEYPAIR_PATH="$HOME/.config/solana/solid-devnet-admin.json"
export SOLANA_KEYPAIR_PATH="$SOLID_KEYPAIR_PATH"
export ANCHOR_WALLET="$SOLID_KEYPAIR_PATH"

solana-keygen pubkey "$SOLID_KEYPAIR_PATH"
solana balance --keypair "$SOLID_KEYPAIR_PATH" --url devnet

solana program deploy target/deploy/schema_registry.so \
  --program-id target/deploy/schema_registry-keypair.json \
  --upgrade-authority "$SOLID_KEYPAIR_PATH" \
  --keypair "$SOLID_KEYPAIR_PATH" \
  --fee-payer "$SOLID_KEYPAIR_PATH" \
  --url devnet --use-rpc --max-sign-attempts 10

solana program deploy target/deploy/zk_verifier.so \
  --program-id target/deploy/zk_verifier-keypair.json \
  --upgrade-authority "$SOLID_KEYPAIR_PATH" \
  --keypair "$SOLID_KEYPAIR_PATH" \
  --fee-payer "$SOLID_KEYPAIR_PATH" \
  --url devnet --use-rpc --max-sign-attempts 10

solana program deploy target/deploy/issuer_registry.so \
  --program-id target/deploy/issuer_registry-keypair.json \
  --upgrade-authority "$SOLID_KEYPAIR_PATH" \
  --keypair "$SOLID_KEYPAIR_PATH" \
  --fee-payer "$SOLID_KEYPAIR_PATH" \
  --url devnet --use-rpc --max-sign-attempts 20
```

After deploy:

```bash
python3 scripts/check_program_ids.py
npm run validate:devnet
```

If a deploy stalls or fails after uploading a buffer, inspect buffers:

```bash
solana program show --buffers \
  --buffer-authority "$SOLID_KEYPAIR_PATH" \
  --url devnet --output json
```

Resume with a funded buffer:

```bash
solana program deploy target/deploy/<program>.so \
  --buffer <BUFFER_ADDRESS> \
  --program-id target/deploy/<program>-keypair.json \
  --upgrade-authority "$SOLID_KEYPAIR_PATH" \
  --keypair "$SOLID_KEYPAIR_PATH" \
  --fee-payer "$SOLID_KEYPAIR_PATH" \
  --url devnet --use-rpc --max-sign-attempts 20
```

Close abandoned buffers to recover rent:

```bash
solana program close <BUFFER_ADDRESS> \
  --keypair "$SOLID_KEYPAIR_PATH" \
  --authority "$SOLID_KEYPAIR_PATH" \
  --recipient "$(solana-keygen pubkey "$SOLID_KEYPAIR_PATH")" \
  --url devnet
```

### 12.6 Initialize and seed on-chain state

Run this only after program binaries are deployed or verified.

```bash
export PATH="$PWD/.toolchain/bin:$PATH"
source config/devnet.env.example

# Use the real deployer.
export SOLID_KEYPAIR_PATH="$HOME/.config/solana/solid-devnet-admin.json"
export SOLANA_KEYPAIR_PATH="$SOLID_KEYPAIR_PATH"
export ANCHOR_WALLET="$SOLID_KEYPAIR_PATH"

# Keep the schema compatible with the batch credential circuit.
export SOLID_SCHEMA_NAME="basic_identity_v2"
export SOLID_SCHEMA_VERSION=2
export SOLID_SCHEMA_CATEGORY="Identity"
export SOLID_SCHEMA_FIELDS="age,country_code,region,id_type,verification_level,issued_date,nationality,_reserved"
export SOLID_SCHEMA_TREE_DEPTH=20

npm run build:idl
npm run init-onchain
npm run backfill-issuer-tree
npm run bootstrap-schema-tree
npm run bootstrap-issuer
npm run issue
npm run prove
```

What these scripts do:

- `init-onchain`: initializes verifier config, uploads verification key data,
  and prepares protocol config accounts.
- `backfill-issuer-tree`: syncs issuer tree state after issuer registry changes.
- `bootstrap-schema-tree`: creates or binds the schema credential tree.
- `bootstrap-issuer`: runs issuer registration, DAO vote/approval, issuer tree
  enrollment, and schema permission grant for the smoke issuer.
- `issue`: issues a real credential into the schema tree.
- `prove`: builds a real Groth16 proof, refreshes roots as needed, submits
  `verify_batch_proof_v2`, and asserts replay rejection.

Green terminal evidence:

```text
verified: true
Replay test (should fail)...
ok (replay rejected by nullifier PDA init constraint)
```

Record every important output in `deployments/devnet.json`:

- deployed program slots
- schema PDA
- schema tree address
- tree binding PDA
- current schema root and slot
- global state root and slot
- issuer account
- issuer authority
- credential commitment
- credential issue tx
- verify tx
- schema permission tx
- schema permission PDA
- replay status

Then validate:

```bash
npm run validate:devnet
npm run verify:artifacts
npm run smoke:devnet-config
```

### 12.7 Prepare circuit artifacts for hosting

Create a clean publish directory from the manifest. This avoids copying stale
or wrong filenames by hand.

```bash
rm -rf public-devnet-artifacts
mkdir -p public-devnet-artifacts

node <<'NODE'
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const manifest = JSON.parse(fs.readFileSync('deployments/devnet.json', 'utf8'));
const out = 'public-devnet-artifacts';

for (const [key, item] of Object.entries(manifest.artifacts.items)) {
  const source = path.resolve(item.local_path);
  const dest = path.join(out, item.filename);
  const bytes = fs.readFileSync(source);
  const actual = crypto.createHash('sha256').update(bytes).digest('hex');
  if (actual !== item.sha256) {
    throw new Error(`${key} hash mismatch: expected ${item.sha256}, got ${actual}`);
  }
  fs.copyFileSync(source, dest);
  fs.writeFileSync(`${dest}.sha256`, `${actual}  ${item.filename}\n`);
  console.log(`${item.filename} ${actual}`);
}
NODE

ls -lh public-devnet-artifacts
```

Expected files:

```text
batch_credential_query.wasm
batch_credential_query.wasm.sha256
batch_credential_query.zkey
batch_credential_query.zkey.sha256
batch_credential_query_verification_key.json
batch_credential_query_verification_key.json.sha256
bjj_subgroup_proof.wasm
bjj_subgroup_proof.wasm.sha256
bjj_subgroup_proof.zkey
bjj_subgroup_proof.zkey.sha256
bjj_subgroup_verification_key.json
bjj_subgroup_verification_key.json.sha256
```

Why these files matter:

- Batch `.wasm`: witness generator for holder credential query proofs.
- Batch `.zkey`: Groth16 proving key for the holder proof.
- Batch VK JSON: verification key used by verifier-side local checks.
- Subgroup `.wasm` and `.zkey`: proof that issuer BJJ keys are in the
  allowed subgroup before registration.
- Subgroup VK JSON: verification key for subgroup proof verification.
- `.sha256` sidecars: human/operator verification files. The app uses pins
  from `deployments/devnet.json`.

### 12.8 Host artifacts

Any stable HTTPS host is acceptable. Use versioned paths when possible,
for example:

```text
https://<artifact-host>/solid/devnet/2026-05-06/
```

The manifest value should be the directory, not an individual file:

```json
"artifacts": {
  "base_url": "https://<artifact-host>/solid/devnet/2026-05-06"
}
```

#### Option A: Same-origin Vercel static files

Use this only if `solid-sim` and artifacts should live under the same host.
The current deployment uses the separate artifact host
`https://artifacts.solidislive.com`.

```bash
cd ../solid-sim
rm -rf public/artifacts
mkdir -p public/artifacts
cp -R ../solid-protocol/public-devnet-artifacts/. public/artifacts/
npm ci
npm run build
```

Deploy `solid-sim` to Vercel. The artifact base URL becomes:

```text
https://<console-host>/artifacts
```

This works because `solid-sim/vercel.json` already sets immutable cache
headers for `/artifacts/(.*)`.

#### Option B: Cloudflare R2

```bash
wrangler r2 bucket create solid-devnet-artifacts
for f in public-devnet-artifacts/*; do
  wrangler r2 object put "solid-devnet-artifacts/solid/devnet/2026-05-06/$(basename "$f")" \
    --file "$f"
done
```

Then attach a public custom domain and use that HTTPS URL as
`artifacts.base_url`.

#### Option C: S3 or S3-compatible storage

```bash
aws s3 sync public-devnet-artifacts/ \
  s3://<bucket>/solid/devnet/2026-05-06/ \
  --cache-control "public,max-age=31536000,immutable"
```

Put CloudFront or another HTTPS CDN in front of the bucket. Do not use a
plain HTTP bucket URL. Browsers reject non-HTTPS artifact URLs unless they
are same-origin or localhost.

#### Verify hosted artifacts

Replace `ARTIFACT_BASE_URL` with the final URL:

```bash
export ARTIFACT_BASE_URL="https://artifacts.solidislive.com"

node <<'NODE'
const manifest = require('./deployments/devnet.json');
const base = process.env.ARTIFACT_BASE_URL.replace(/\/$/, '');
for (const item of Object.values(manifest.artifacts.items)) {
  console.log(`${base}/${item.filename}`);
}
NODE
```

Download and hash-check:

```bash
rm -rf /tmp/solid-artifact-check
mkdir -p /tmp/solid-artifact-check

node <<'NODE' > /tmp/solid-artifact-urls.txt
const manifest = require('./deployments/devnet.json');
const base = process.env.ARTIFACT_BASE_URL.replace(/\/$/, '');
for (const item of Object.values(manifest.artifacts.items)) {
  console.log(`${item.sha256} ${base}/${item.filename}`);
}
NODE

while read -r expected url; do
  file="/tmp/solid-artifact-check/$(basename "$url")"
  curl -fsSL "$url" -o "$file"
  actual="$(shasum -a 256 "$file" | awk '{print $1}')"
  test "$actual" = "$expected" || {
    echo "hash mismatch for $url expected=$expected actual=$actual"
    exit 1
  }
  echo "ok $url"
done < /tmp/solid-artifact-urls.txt
```

### 12.9 Deploy the indexer/API

The current indexer is a Node HTTP service with a file-backed store:

```text
solid-protocol/indexer/src/server.mjs
solid-protocol/indexer/src/store.mjs
```

Because it uses `SOLID_INDEXER_STORE_PATH`, deploy it to a target with
persistent storage unless you replace the store implementation. Good first
targets: Render persistent disk, Fly.io volume, Railway volume, or a VPS.
Stateless serverless such as plain Vercel functions is not enough for the
current implementation because the store will disappear between invocations.

Required env vars:

```bash
PORT=8787
HOST=0.0.0.0
SOLID_MANIFEST_PATH=/app/deployments/devnet.json
SOLID_INDEXER_STORE_PATH=/data/solid-indexer/state.json
SOLID_RPC_URL=https://api.devnet.solana.com
SOLID_INDEXER_WRITE_TOKEN=<random 32-byte hex>
```

What each env var does:

- `PORT`: HTTP port the host routes to.
- `HOST`: bind address. Use `0.0.0.0` on hosted infra, `127.0.0.1` locally.
- `SOLID_MANIFEST_PATH`: the manifest the API serves and reads for program,
  schema, tree, and artifact values.
- `SOLID_INDEXER_STORE_PATH`: persistent JSON state for credential requests
  and indexed leaves.
- `SOLID_RPC_URL`: devnet RPC used for live reads.
- `SOLID_INDEXER_WRITE_TOKEN`: bearer token required for write ingestion.

#### Local indexer smoke before cloud deploy

```bash
cd solid-protocol
npm ci

PORT=8787 \
HOST=127.0.0.1 \
SOLID_MANIFEST_PATH="$PWD/deployments/devnet.json" \
SOLID_INDEXER_STORE_PATH="$PWD/.solid-indexer/state.json" \
SOLID_RPC_URL=https://api.devnet.solana.com \
SOLID_INDEXER_WRITE_TOKEN="$(openssl rand -hex 32)" \
npm start --prefix indexer
```

In a second terminal:

```bash
curl -fsS http://127.0.0.1:8787/v1/health | python3 -m json.tool
curl -fsS http://127.0.0.1:8787/v1/manifest | python3 -m json.tool
curl -fsS http://127.0.0.1:8787/v1/schemas | python3 -m json.tool
curl -fsS http://127.0.0.1:8787/v1/issuers | python3 -m json.tool
curl -fsS http://127.0.0.1:8787/.well-known/solid-protocol.json | python3 -m json.tool
```

#### Minimal Dockerfile for VPS/Fly/Render

Create this only if your host wants Docker:

```dockerfile
FROM node:20-slim
WORKDIR /app
COPY package.json package-lock.json ./
COPY ts-sdk ./ts-sdk
COPY deployments ./deployments
COPY indexer ./indexer
RUN npm ci
ENV HOST=0.0.0.0
ENV PORT=8787
CMD ["npm", "start", "--prefix", "indexer"]
```

Runtime env:

```bash
SOLID_MANIFEST_PATH=/app/deployments/devnet.json
SOLID_INDEXER_STORE_PATH=/data/solid-indexer/state.json
SOLID_RPC_URL=https://api.devnet.solana.com
SOLID_INDEXER_WRITE_TOKEN=<secret>
```

Mount `/data` as a persistent volume.

#### Verify deployed indexer

```bash
export SOLID_INDEXER_URL="https://<indexer-host>"

curl -fsS "$SOLID_INDEXER_URL/v1/health" | python3 -m json.tool
curl -fsS "$SOLID_INDEXER_URL/v1/manifest" | python3 -m json.tool
curl -fsS "$SOLID_INDEXER_URL/v1/roots/current" | python3 -m json.tool
curl -fsS "$SOLID_INDEXER_URL/v1/schemas" | python3 -m json.tool
curl -fsS "$SOLID_INDEXER_URL/v1/issuers" | python3 -m json.tool
curl -fsS "$SOLID_INDEXER_URL/.well-known/solid-protocol.json" | python3 -m json.tool
```

Negative check, proof must fail closed for an unknown leaf:

```bash
curl -i "$SOLID_INDEXER_URL/v1/merkle-proof/4mhWLGb2KAtF1bY2mdrGb37xhAUpmRsL9bgzLRjE35sc/0000000000000000000000000000000000000000000000000000000000000000"
```

Expected: `404` with `LEAF_NOT_INDEXED` or equivalent error. If it returns
a fake proof, do not deploy. That would make proofs meaningless.

### 12.10 Publish the final manifest

Update `deployments/devnet.json` after artifact, indexer, console, and wallet
URLs exist.

Required fields to make non-null before public testing:

```json
{
  "artifacts": {
    "base_url": "https://<artifact-host>/solid/devnet/2026-05-06",
    "manifest_url": "https://<manifest-host>/solid/devnet.json"
  },
  "indexer": {
    "url": "https://<indexer-host>",
    "health_path": "/v1/health",
    "api_version": "v1"
  },
  "console": {
    "url": "https://<console-host>"
  },
  "wallet": {
    "release_url": "https://<wallet-release-url>"
  }
}
```

Also confirm these are current:

- `programs.*.program_id`
- `programs.*.upgrade_authority`
- `schemas[0].tree_address`
- `schemas[0].tree_binding_pda`
- `schemas[0].current_root`
- `schemas[0].current_root_slot`
- `trees.global_state_tree.current_root`
- `trees.issuer_tree.current_root`
- `sample.credential_tx`
- `sample.verify_tx`
- `sample.schema_permission_tx`
- `sample.verify_status`

Validate locally:

```bash
npm run validate:devnet
npm run verify:artifacts
npm run smoke:devnet-config
```

Publish the manifest to HTTPS. The current canonical manifest is:

```text
https://api.solidislive.com/v1/manifest
```

Other valid choices:

1. Same-origin under `solid-sim`, for example
   `solid-sim/public/solid/devnet.json`.
2. The indexer well-known endpoint, if it serves the same manifest:
   `https://<indexer-host>/.well-known/solid-protocol.json`.
3. Static storage/CDN:
   `https://<artifact-host>/solid/devnet.json`.

Verify the published manifest:

```bash
export SOLID_MANIFEST_URL="https://api.solidislive.com/v1/manifest"
curl -fsS "$SOLID_MANIFEST_URL" -o /tmp/solid-devnet.json
python3 -m json.tool /tmp/solid-devnet.json >/dev/null
```

### 12.11 Deploy `solid-sim`

`solid-sim` is a Vite static app. It can deploy to Vercel, Netlify,
GitHub Pages, S3+CloudFront, or any static web host. The current public
deployment is live at `https://app.solidislive.com`.

Production env vars:

```bash
VITE_SOLID_NETWORK=devnet
VITE_SOLID_CLUSTER=devnet
VITE_SOLID_RPC_URL=https://api.devnet.solana.com
VITE_SOLID_WS_URL=wss://api.devnet.solana.com
VITE_SOLID_MANIFEST_URL=https://api.solidislive.com/v1/manifest
VITE_SOLID_ARTIFACT_BASE_URL=https://artifacts.solidislive.com
VITE_SOLID_INDEXER_URL=https://api.solidislive.com
VITE_SOLID_CONSOLE_URL=https://app.solidislive.com
VITE_SOLID_EXPLORER_CLUSTER=devnet
```

What each value does:

- `VITE_SOLID_NETWORK` and `VITE_SOLID_CLUSTER`: label the app as devnet.
- `VITE_SOLID_RPC_URL`: Solana read/write RPC used by console actions.
- `VITE_SOLID_WS_URL`: websocket endpoint for account/transaction updates.
- `VITE_SOLID_MANIFEST_URL`: lets the app load the published deployment state.
- `VITE_SOLID_ARTIFACT_BASE_URL`: overrides artifact URLs if the manifest
  does not include a base URL or you want same-origin artifacts.
- `VITE_SOLID_INDEXER_URL`: request inbox, credential request workflow, and
  proof Merkle paths.
- `VITE_SOLID_CONSOLE_URL`: self URL recorded in manifest and shared links.
- `VITE_SOLID_EXPLORER_CLUSTER`: explorer link cluster parameter.

#### Local production build test

```bash
cd ../solid-sim
npm ci

VITE_SOLID_NETWORK=devnet \
VITE_SOLID_CLUSTER=devnet \
VITE_SOLID_RPC_URL=https://api.devnet.solana.com \
VITE_SOLID_WS_URL=wss://api.devnet.solana.com \
VITE_SOLID_MANIFEST_URL=https://api.solidislive.com/v1/manifest \
VITE_SOLID_ARTIFACT_BASE_URL=https://artifacts.solidislive.com \
VITE_SOLID_INDEXER_URL=https://api.solidislive.com \
VITE_SOLID_CONSOLE_URL=https://app.solidislive.com \
VITE_SOLID_EXPLORER_CLUSTER=devnet \
npm run build

npm run preview
```

Open the preview URL and check:

- Overview page loads.
- DAO page loads.
- Issuer Request Inbox loads.
- Issuer Issue Credential loads.
- Wallet page loads.
- Verifier page loads.
- No blank screens.
- No browser console error for `process is not defined`.
- No browser console error for `tweetnacl-util` named exports.
- Artifact loading either succeeds or fails with a clear pinned-artifact error.

#### Vercel deploy

`solid-sim/vercel.json` already sets:

- framework: `vite`
- build command: `npm run build`
- output directory: `dist`
- immutable cache headers for `/artifacts/(.*)`

Set the production env vars in Vercel, then deploy:

```bash
cd ../solid-sim
vercel pull --yes --environment=production
vercel build --prod
vercel deploy --prebuilt --prod
```

After deploy:

```bash
curl -I https://app.solidislive.com/
curl -I https://artifacts.solidislive.com/batch_credential_query.wasm
```

If artifacts are hosted elsewhere, the second command is only required
against the artifact host, not the console host.

### 12.12 Optional: build and distribute `solid-wallet`

Skip this section for the current `solid-sim` deployment unless you explicitly
want to test the standalone browser extension. The current wallet role lives
inside `solid-sim`, so the required public E2E path is:

```text
solid-sim DAO -> solid-sim Issuer -> solid-sim Wallet -> solid-sim Verifier
```

`solid-wallet` is a separate browser extension. Its build output is
`solid-wallet/dist`. Use it only when testing external dApp injection,
`window.solid`, or extension-specific encrypted storage.

Before building the extension for public testers, update
`solid-wallet/public/manifest.json` so the extension can talk to your deployed
app and only your deployed app.

Current local-only values look like:

```json
"host_permissions": [
  "http://localhost/*",
  "http://127.0.0.1/*"
],
"content_scripts": [
  {
    "matches": ["http://localhost/*", "http://127.0.0.1/*"]
  }
]
```

For public devnet, change them to your tester origins:

```json
"host_permissions": [
  "https://<console-host>/*",
  "https://<verifier-dapp-host>/*"
],
"content_scripts": [
  {
    "matches": ["https://<console-host>/*", "https://<verifier-dapp-host>/*"],
    "js": ["content.js"],
    "run_at": "document_start"
  }
],
"web_accessible_resources": [
  {
    "resources": ["icons/*"],
    "matches": ["https://<console-host>/*", "https://<verifier-dapp-host>/*"]
  }
]
```

Do not use `<all_urls>` for public testing unless you explicitly accept the
risk. The wallet injects `window.solid`; keep that scoped.

Build:

```bash
cd ../solid-wallet
npm ci

VITE_SOLID_NETWORK=devnet \
VITE_SOLID_CLUSTER=devnet \
VITE_SOLID_RPC_URL=https://api.devnet.solana.com \
VITE_SOLID_WS_URL=wss://api.devnet.solana.com \
VITE_SOLID_MANIFEST_URL=https://api.solidislive.com/v1/manifest \
VITE_SOLID_ARTIFACT_BASE_URL=https://artifacts.solidislive.com \
VITE_SOLID_INDEXER_URL=https://api.solidislive.com \
npm run build
```

Package an unpacked-test ZIP:

```bash
rm -f solid-wallet-devnet.zip
(cd dist && zip -r ../solid-wallet-devnet.zip .)
shasum -a 256 solid-wallet-devnet.zip
```

Install locally:

1. Open `chrome://extensions`.
2. Enable Developer Mode.
3. Click "Load unpacked".
4. Select `solid-wallet/dist`.
5. Open the extension settings page and confirm manifest URL, artifact URL,
   and indexer URL are populated.

If you do not deploy the extension, leave `deployments/devnet.json`
`wallet.release_url` as `null` and record that the active wallet for this
milestone is the embedded `solid-sim` wallet. If you do deploy it, update
`wallet.release_url` with the ZIP URL or private Chrome Web Store URL.

### 12.13 Optional verifier dApp deployment

For full external-dApp testing, deploy a simple verifier app or use
`solid-sim` verifier pages as the verifier role.

If using `examples/verifier-dapp`, remember its `package.json` currently
uses published package ranges:

```json
"@solid-protocol/sdk": "^0.3.0",
"@solid-protocol/verifier": "^0.2.0"
```

Until npm packages are published, use one of these:

1. Publish SDK packages first.
2. Temporarily switch example deps to local `file:` paths for operator testing.
3. Use `solid-sim` verifier pages instead of the example dApp.

Verifier app requirements:

- Reads the same manifest URL.
- Builds a proof request envelope with artifact pins from the manifest.
- Calls `window.solid.requestProof(...)`.
- Receives proof/public signals/nullifier.
- Submits on-chain verification to `zk_verifier`.
- Rejects access if on-chain verification fails or replay is rejected.

### 12.14 SDK package publishing gate

For monorepo deployment, local `file:` deps are fine. For external apps,
publish the SDK packages or provide tarballs.

Run:

```bash
cd solid-protocol/ts-sdk
npm ci
npm run build
npm test
npm run pack:check
npm audit --audit-level=high
```

Publish order, because packages depend on each other:

```bash
npm publish --workspace @solid-protocol/channel --access public
npm publish --workspace @solid-protocol/core --access public
npm publish --workspace @solid-protocol/light --access public
npm publish --workspace @solid-protocol/issuer --access public
npm publish --workspace @solid-protocol/holder --access public
npm publish --workspace @solid-protocol/verifier --access public
npm publish --workspace @solid-protocol/sdk --access public
```

Do not publish while `npm audit --audit-level=high` is red unless the
release notes explicitly accept the risk. Current known audit pressure comes
from transitive dependencies such as `underscore/jsonpath/bfj` and
`bigint-buffer` in Solana-related chains. Fix or document before public npm
release.

After publishing:

```bash
npm view @solid-protocol/sdk version
npm view @solid-protocol/verifier version
npm view @solid-protocol/channel version
```

### 12.15 Link everything together

After all pieces are deployed, the links should form this graph:

```text
solid-sim
  -> VITE_SOLID_MANIFEST_URL
  -> VITE_SOLID_RPC_URL / VITE_SOLID_WS_URL
  -> VITE_SOLID_INDEXER_URL
  -> VITE_SOLID_ARTIFACT_BASE_URL

embedded solid-sim wallet
  -> same solid-sim VITE_SOLID_MANIFEST_URL
  -> same solid-sim VITE_SOLID_INDEXER_URL
  -> same solid-sim VITE_SOLID_ARTIFACT_BASE_URL

optional solid-wallet extension
  -> baked VITE_SOLID_MANIFEST_URL, if extension is deployed
  -> baked or user-configured artifact base URL, if extension is deployed
  -> baked or user-configured indexer URL, if extension is deployed
  -> allowed tester origins in extension manifest, if extension is deployed

indexer
  -> SOLID_MANIFEST_PATH
  -> SOLID_RPC_URL
  -> SOLID_INDEXER_STORE_PATH
  -> SOLID_INDEXER_WRITE_TOKEN

manifest
  -> program IDs
  -> artifact base URL + hashes
  -> indexer URL
  -> console URL
  -> optional wallet release URL, null when using only the embedded console wallet
  -> schema/tree/root/sample tx state
```

Run this final wiring checklist:

```bash
export SOLID_MANIFEST_URL="https://api.solidislive.com/v1/manifest"
export SOLID_INDEXER_URL="https://api.solidislive.com"
export ARTIFACT_BASE_URL="https://artifacts.solidislive.com"
export SOLID_CONSOLE_URL="https://app.solidislive.com"

curl -fsS "$SOLID_MANIFEST_URL" -o /tmp/solid-devnet.json
curl -fsS "$SOLID_INDEXER_URL/v1/health" | python3 -m json.tool
curl -fsS "$SOLID_INDEXER_URL/v1/manifest" -o /tmp/solid-indexer-manifest.json

python3 - <<'PY'
import json
m1=json.load(open('/tmp/solid-devnet.json'))
m2=json.load(open('/tmp/solid-indexer-manifest.json'))
assert m1['programs']==m2['programs'], 'indexer manifest programs differ'
assert m1['schemas']==m2['schemas'], 'indexer manifest schemas differ'
print('manifest wiring ok')
PY

curl -I "$SOLID_CONSOLE_URL/"
```

### 12.16 Exhaustive deployed-system test plan

Run these after public URLs are live.

#### A. Static and config checks

```bash
curl -fsS "$SOLID_MANIFEST_URL" | python3 -m json.tool >/dev/null
curl -fsS "$SOLID_INDEXER_URL/v1/health" | python3 -m json.tool
curl -fsS "$SOLID_INDEXER_URL/v1/schemas" | python3 -m json.tool
curl -fsS "$SOLID_INDEXER_URL/v1/issuers" | python3 -m json.tool
curl -fsS "$SOLID_INDEXER_URL/v1/roots/current" | python3 -m json.tool
```

Expected:

- manifest JSON parses
- indexer health is OK
- schema list contains `basic_identity_v2`
- issuer list contains the approved smoke issuer
- roots match or are not older than the manifest roots

#### B. Hosted artifact checks

Use the command from section 12.8 to download every artifact and compare
SHA-256 against the manifest. Also open the console in a browser and trigger
pages that use artifacts:

- Issuer registration, subgroup proof path.
- Wallet proof generation path.
- Verifier proof verification path.

Expected:

- no mixed-content errors
- no CORS errors if artifact host differs from console host
- no hash mismatch
- no missing `.wasm` or `.zkey`

#### C. On-chain program checks

```bash
solana program show 4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1 --url devnet
solana program show 5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx --url devnet
solana program show DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb --url devnet
```

Expected:

- executable
- owned by BPF Upgradeable Loader
- expected authority
- expected recent deploy slots

#### D. Terminal E2E against deployed devnet

Run from `solid-protocol`:

```bash
source config/devnet.env.example
export SOLID_KEYPAIR_PATH="$HOME/.config/solana/solid-devnet-admin.json"
export SOLANA_KEYPAIR_PATH="$SOLID_KEYPAIR_PATH"
export ANCHOR_WALLET="$SOLID_KEYPAIR_PATH"

npm run issue
npm run prove
```

Expected:

- new credential issue tx
- schema/global roots update if needed
- local Groth16 verify passes
- on-chain verify tx succeeds
- replay attempt fails

Update manifest sample values after this if the purpose is a release smoke.

#### E. DAO flow in `solid-sim`

In browser:

1. Open deployed `solid-sim`.
2. Connect a devnet Solana wallet.
3. Go to DAO.
4. Confirm registry config loads.
5. Create or inspect issuer application.
6. Vote/approve issuer if running a fresh issuer.
7. Finalize approval.
8. Confirm issuer appears in active issuer list and issuer tree root updates.

Evidence to record:

- DAO wallet pubkey
- issuer application PDA
- vote tx
- finalize tx
- issuer account
- issuer status
- issuer tree root before/after

#### F. Issuer flow in `solid-sim`

1. Go to Issuer.
2. Open Register Issuer.
3. Generate or load issuer identity.
4. Submit registration with subgroup proof.
5. Confirm DAO approval state.
6. Open Schema Permissions.
7. Confirm issuer has permission for `basic_identity_v2`.
8. Open Request Inbox.
9. Confirm holder credential requests appear.
10. Open Issue Credential.
11. Issue a credential to the selected holder.

Evidence to record:

- issuer authority pubkey
- BJJ pubkey
- registration tx
- schema permission PDA
- issue tx
- credential commitment
- encrypted credential envelope hash

Expected failure tests:

- unapproved issuer cannot issue
- issuer without schema permission cannot issue
- malformed holder key is rejected
- expired or mismatched schema request is rejected

#### G. Holder wallet flow

For the current milestone, use the embedded Wallet section in deployed
`solid-sim`.

1. Open deployed `solid-sim`.
2. Go to the Wallet role.
3. Confirm the app is using the devnet manifest URL, artifact base URL, and
   indexer URL from the production env/manifest.
4. Create or load holder identity/material in the simulator wallet.
5. Go to Holder Discover and request a credential from an active issuer.
6. Go to Issuer Request Inbox and confirm the request appears.
7. Issue the credential from the Issuer role.
8. Return to Wallet and import/store the issued encrypted credential envelope.
9. Confirm credential integrity validation passes.
10. Trigger a proof request from the Verifier role.
11. Generate the proof using hosted artifacts and live Merkle proof data from
    the indexer.
12. Confirm proof generation succeeds.

If and only if the standalone extension is part of the test release, repeat
the same flow through `solid-wallet` and `window.solid`.

Evidence to record:

- app build hash and wallet mode: `embedded solid-sim wallet`
- holder public material, not secret seed
- credential metadata
- proof request origin
- artifact URLs and pins
- Merkle proof root/slot
- generated nullifier
- generated public signals

Expected failure tests:

- embedded wallet blocks proof generation when indexer URL is missing
- embedded wallet blocks proof generation when artifact base URL is missing
- embedded wallet rejects artifact hash mismatch
- embedded wallet rejects non-HTTPS artifact URL unless same-origin or localhost
- embedded wallet rejects credential issued to a different holder key
- embedded wallet rejects expired credential
- embedded wallet returns a clear error for missing matching credential

#### H. Verifier/dApp flow

1. Open deployed verifier page or `solid-sim` verifier role.
2. Build a requirement for `basic_identity_v2`.
3. Request proof through the embedded `solid-sim` wallet flow. If testing the
   optional extension, request proof through `window.solid`.
4. Receive proof and public signals.
5. Run local verification if the UI supports it.
6. Submit on-chain verification.
7. Confirm dApp grants access only after on-chain success.
8. Submit the same proof again.
9. Confirm replay fails because the nullifier PDA already exists.

Evidence to record:

- requirement JSON
- proof request envelope
- artifact pins
- verify tx
- nullifier PDA
- replay rejection logs

Expected failure tests:

- dApp does not grant access before on-chain success
- dApp does not grant access after local-only success if on-chain submission fails
- replay is rejected
- wrong schema hash fails
- wrong root fails
- wrong verifier program ID fails

#### I. Browser regression checks

Open every route in deployed `solid-sim`:

- System Overview
- System Protocol Flow
- System Schemas
- System Logs
- DAO Overview
- DAO Applications
- DAO Active Issuers
- DAO Schema Permissions
- DAO VK Management
- Issuer Register
- Issuer Request Inbox
- Issuer Issue Credential
- Issuer Issued Log
- Issuer Staking
- Wallet Request Credential
- Wallet Dashboard
- Verifier Query Builder
- Verifier Verify Proof
- Verifier History
- Verifier Analytics

Expected:

- no blank pages
- no uncaught exceptions
- page error boundary never appears
- wallet section renders credential state
- request inbox renders empty, loading, and populated states
- issue credential page renders even before a request exists
- Flow motion originates at DAO and progresses toward Verifier
- visual accent is consistent; only semantic states use separate colors

Browser console must not contain:

```text
process is not defined
tweetnacl-util does not provide an export named
Module "fs" has been externalized
Module "path" has been externalized
Module "os" has been externalized
```

#### J. API negative tests

```bash
# Unknown proof should fail closed.
curl -i "$SOLID_INDEXER_URL/v1/merkle-proof/<tree>/0000000000000000000000000000000000000000000000000000000000000000"

# Missing write token should be rejected.
curl -i -X POST "$SOLID_INDEXER_URL/v1/tree-leaves" \
  -H "Content-Type: application/json" \
  --data '{}'

# Bad write token should be rejected.
curl -i -X POST "$SOLID_INDEXER_URL/v1/tree-leaves" \
  -H "Authorization: Bearer wrong" \
  -H "Content-Type: application/json" \
  --data '{}'
```

Expected:

- unknown proof: `404`
- missing/bad write token: `401` or `403`
- no fake proof is returned
- no stack trace leaks secrets

#### K. Performance and size sanity

Record:

- console initial JS size
- wallet background bundle size
- artifact file sizes
- proof generation time
- on-chain verification tx time
- indexer health latency

Current build warnings may show chunks over 500 kB. This is acceptable for
devnet testing if proof flows are green, but track it before mainnet/public
growth. If the wallet feels slow, split proof generation and heavy crypto
paths with dynamic imports.

### 12.17 Final release evidence checklist

Create a release note or status doc with:

```text
Protocol:
- schema_registry program show output
- issuer_registry program show output
- zk_verifier program show output
- init tx
- issuer approval tx
- schema permission tx
- issue tx
- verify tx
- replay rejection evidence

Artifacts:
- artifact base URL
- SHA-256 for all six artifacts
- curl/hash verification output

Indexer:
- URL
- /v1/health output
- /v1/schemas output
- /v1/issuers output
- unknown leaf 404 output

Console:
- URL
- commit/build hash
- env vars used
- browser route smoke results

Wallet:
- active wallet mode: embedded `solid-sim` wallet
- solid-sim wallet route smoke result
- proof generation result
- optional extension release URL, ZIP hash, and allowed origins only if
  `solid-wallet` is included in the test release

E2E:
- DAO approval evidence
- issuer issuance evidence
- holder import/proof evidence
- verifier on-chain success evidence
- replay rejection evidence
```

### 12.18 Rollback

If public deployment fails before testers start:

1. Remove or unpublish the manifest URL.
2. Keep the broken manifest under a timestamped debug path.
3. Restore the previous console deployment.
4. Stop the indexer if it is writing bad state.
5. Do not delete tx signatures. They are evidence.
6. Publish a new manifest only after `validate:devnet`, artifact hash checks,
   indexer health, and browser smoke pass again.

If testers are using only the embedded console wallet:

1. Restore the previous console deployment.
2. Publish corrected `VITE_SOLID_*` env vars and manifest values.
3. Ask testers to hard refresh the console and retry the Wallet role.

If testers already received the optional extension:

1. Keep the old artifact URLs alive. Wallets may have cached settings.
2. Publish a new wallet ZIP/version with corrected manifest/artifact/indexer
   defaults.
3. Tell testers to remove the old unpacked extension before loading the new one.
4. Mark old roots and sample txs stale in status docs.

### 12.19 Tester bug report template

Ask testers for this exact data:

```text
Role: DAO / Issuer / Holder / Verifier
Browser + version:
OS:
Solana wallet:
Wallet mode: embedded console wallet / optional solid-wallet extension
Console build hash:
SolID Wallet version or ZIP hash, only if using extension:
Console URL:
Manifest URL:
Indexer URL:
Action attempted:
Expected:
Actual:
Error message:
Browser console logs:
Transaction signature(s):
Credential request ID:
Credential commitment, if visible:
Proof/nullifier, if visible:
Screenshot/video:
Did refresh help:
Did wallet lock/unlock help:
```

Do not accept "it doesn't work" as a complete report. The system spans
browser, console wallet state, indexer, artifact host, RPC, and three
programs. If the optional extension is included, it also spans extension
permissions and extension storage. You need the failing layer.

---

## 13. Mainnet checklist

Do not deploy to mainnet-beta until every item below is satisfied.
Each bullet maps to a registry finding or the v0.6.1 audit's
Section 6 recommendations (P0 / P1 / P2 ordering).

- All open HIGH and MEDIUM items in `sec/SECURITY_REGISTRY.md` are
  Fixed. At HEAD that means SOLID-SEC-010 (cross-language vectors
  expansion), SOLID-SEC-012 (multi-party ceremony), SOLID-SEC-013
  through -019, -021, -034, -043, -045 (atomic binding update),
  -046 (CU-budget regression gate). See also the v0.6.1 audit
  Section 6.1 for P0 ordering.
- SOLID-SEC-006 Part 2 (circuit-bound `vk_generation`) has landed
  alongside the multi-party ceremony.
- Multi-party trusted setup ceremony completed. Published
  contributor attestation chain + content-addressed artifacts on
  IPFS + Arweave.
- `AUTHORITY_PUBKEY` and `SOLID_TOKEN_MINT` in
  `ts-sdk/packages/sdk/src/config.ts` are production keys, not the
  `SoLid1111...` placeholder.
- All three program upgrade authorities transferred to a Squads
  3-of-5 multisig. `IssuerTreeBinding.operator` + `RegistryConfig.
  authority` + `VerifierConfig.authority` all point at the same
  multisig PDA.
- At least one third-party audit (OtterSec, Zellic, Halborn, Trail
  of Bits) has completed with no open findings at HIGH or above.
  Per the IMPLEMENTATION_PLAN Section 0 non-negotiables, mainnet
  date slips by one sprint after audit close-out, not after
  submission.
- `HeliusDasAdapter` or equivalent production MerkleProofAdapter
  wired into `@solid-protocol/light`.
- Monitoring + alerting configured for `CredentialIssued`,
  `IssuerApproved`, `IssuerLeafReplaced`, and any error logs from
  the verifier.
- `deployments/mainnet.json` populated by
  `scripts/regen_devnet_manifest.py --cluster mainnet-beta`;
  `check_program_ids.py` green.

---

*For the canonical state-of-protocol assessment, read
`sec/audits/2026-04-25_v0.6.1_deep_comprehensive_audit.md`.
For the registry, `sec/SECURITY_REGISTRY.md`. For the implementation
plan, `plan/IMPLEMENTATION_PLAN.md`.*
