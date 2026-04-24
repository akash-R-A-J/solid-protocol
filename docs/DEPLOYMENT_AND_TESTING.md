# Deployment, End-to-End Testing, and Verification

v0.6 (Phase 3 impl 4 landed 2026-04-25). Canonical runbook for the
complete development loop: install toolchain, build artifacts, deploy
programs, run the E2E pipeline, and **verify** that each invariant
actually holds (not just that the pipeline exited zero).

This doc is the reference. Every command is expected to succeed as
written. No workarounds. If a step fails, follow the
`## Troubleshooting` section rather than improvising.

---

## 0. TL;DR (clean machine, localnet)

```bash
# 1. Install the toolchain (pinned versions into .toolchain/bin).
bash scripts/bootstrap.sh
export PATH="$PWD/.toolchain/bin:$PATH"

# 2. Build everything, in order.
cd circuits && npm install && node scripts/setup.js && cd ..
wasm-pack build wasm/ --target nodejs \
    --out-dir ts-sdk/packages/core/wasm --release
anchor build
(cd ts-sdk && npm ci && npm run build)
npm install                                      # root (for tsx + scripts)

# 3. Boot a fresh validator and deploy.
solana-test-validator --reset &
sleep 5
solana config set --url localhost
solana airdrop 10
anchor deploy --provider.cluster localnet

# 4. Run the E2E pipeline.
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
    --out-dir ts-sdk/packages/core/wasm --release
```

Output: `ts-sdk/packages/core/wasm/` containing the `.wasm` binary
and the TypeScript declaration shim. The TS SDK imports from here
exclusively; re-running `wasm-pack build` after any change to
`wasm/src/lib.rs` or `crates/solid-core` is mandatory, otherwise
the SDK and the program disagree byte-for-byte.

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
zk_verifier:       BZkVFdMhAEeGMvEAhXNjt3r3bEA2sCPqEFcsEbSbFGj2
issuer_registry:   CRGYfonXwDk6gKEm9fC1U33VVBkqnQVD3sPdLKzqHWoR
schema_registry:   DPk6XUH6CArLWt4KMqJmpNBnPwQ3gG9P3dBd3MDVE3bT
```

Mismatch means one of `Anchor.toml`, `declare_id!`, or the freshly
generated keypair under `target/deploy/` drifted.
`python3 scripts/check_program_ids.py` will tell you which one.
Follow `docs/PROGRAM_ID_RECONCILIATION.md` for the fix.

### 3.2 Devnet (optional)

```bash
solana config set --url devnet
solana airdrop 2 && solana airdrop 2
anchor deploy --provider.cluster devnet

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
    --out-dir ts-sdk/packages/core/wasm --release
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
  --owner BZkVFdMhAEeGMvEAhXNjt3r3bEA2sCPqEFcsEbSbFGj2 2>/dev/null)

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
solana logs BZkVFdMhAEeGMvEAhXNjt3r3bEA2sCPqEFcsEbSbFGj2 --url localhost
solana logs CRGYfonXwDk6gKEm9fC1U33VVBkqnQVD3sPdLKzqHWoR --url localhost
solana logs DPk6XUH6CArLWt4KMqJmpNBnPwQ3gG9P3dBd3MDVE3bT --url localhost
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

## 12. Mainnet checklist

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
