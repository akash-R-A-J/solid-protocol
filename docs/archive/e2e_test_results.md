# SolID Protocol — E2E Test Results

> Tracking end-to-end testing progress. Updated live as each step completes.
>
> **HISTORICAL SNAPSHOT.** This is the v0.1 (Light-backend) E2E attempt.
> v0.2 ships a new E2E suite based on SPL Account Compression; see
> [`DEPLOYMENT_AND_TESTING.md`](./DEPLOYMENT_AND_TESTING.md) for the
> current canonical flow and `.github/workflows/ci.yml` for the
> automated gates.

**Environment:** Ubuntu (WSL), Solana devnet  
**Date:** 2026-04-04

---

## Progress

| # | Step | Status | Notes |
|---|---|---|---|
| 1 | Prerequisites installed | ✅ Pass | Rust, Solana CLI, Anchor, Circom, snarkjs, Node.js |
| 2 | `cargo test -p solid-core` | ✅ Pass | 41 tests passed, 0 failed |
| 3 | Circom circuit compilation | ✅ Pass | See details below |
| 4 | Trusted setup ceremony | ✅ Pass | VK + zkey generated, intermediates cleaned |
| 5 | WASM build (`wasm-pack`) | ✅ Pass | web (14min) + nodejs (13min) both done |
| 6 | Anchor build | ✅ Pass | All 3 programs compiled (warnings only) |
| 7 | Deploy to devnet | ✅ Pass | All 3 programs deployed, IDLs initialized |
| 8 | TS SDK build | ✅ Pass | All 5 packages compiled clean |
| 9 | Initialize on-chain state | ⬜ Pending | Registry + Schema + Verifier + VK + Bloom |
| 10 | Issue credential | ⬜ Pending | `issueCredential()` with Light Protocol |
| 11 | Generate ZK proof | ⬜ Pending | `generateProof()` with snarkjs |
| 12 | Verify on-chain | ⬜ Pending | `verifyOnChain()` → Groth16 verify + nullifier |
| 13 | Replay rejection test | ⬜ Pending | Same proof → should fail |

---

## Step 2: solid-core Unit Tests

```
cargo test -p solid-core

test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.67s
```

| Module | Tests | Result |
|---|---|---|
| poseidon | 9 | ✅ |
| babyjubjub | 10 | ✅ |
| commitment | 5 | ✅ |
| nullifier | 4 | ✅ |
| query | 5 | ✅ |
| schema | 4 | ✅ |
| credential | 2 | ✅ |
| sas | 1 | ✅ |
| **Total** | **41** | **✅ All pass** |

---

## Step 3: Circom Circuit Compilation ✅

```bash
circom compound_query.circom \
    --r1cs --wasm --sym --c \
    --output build/ \
    -l node_modules/circomlib/circuits
```

### Compilation Output

```
template instances: 340
non-linear constraints: 17480
linear constraints: 8805
public inputs: 20
private inputs: 56 (36 belong to witness)
public outputs: 1
wires: 26310
labels: 49281
Everything went okay
```

### R1CS Info (`snarkjs r1cs info`)

| Metric | Value |
|---|---|
| Curve | bn-128 |
| Wires | 26,310 |
| **Constraints** | **26,285** |
| Private Inputs | 56 |
| Public Inputs | 20 |
| Labels | 49,281 |
| Outputs | 1 |

### Build Artifacts

```
build/
├── compound_query.r1cs          ← R1CS constraint system
├── compound_query.sym           ← Symbol table (debugging)
├── compound_query_js/
│   └── compound_query.wasm      ← Witness generator (used by snarkjs)
└── compound_query_cpp/
    ├── compound_query.cpp       ← C++ witness generator (optional, faster)
    └── ...
```

### Bugs Fixed During This Step

| Bug | File | Fix |
|---|---|---|
| Non-quadratic constraint (7 signal products summed) | `lib/predicate_evaluator.circom:49` | Split into 7 intermediate `prod[i]` signals, sum linearly |
| Non-quadratic constraint (4-way AND multiplication) | `compound_query.circom:151` | Chained pairwise: `and01`, `and012`, `andResult` |
| Non-quadratic constraint (2-product mux) | `compound_query.circom:167` | Split into `selectAnd` + `selectOr` intermediates |
| Missing import for `IsZero`/`LessEqThan` | `lib/nullifier_expiry.circom` | Added `comparators.circom` import |

---

## Step 4: Trusted Setup ✅

```bash
node scripts/setup.js
```

```
═══════════════════════════════════════
  SolID Protocol — Trusted Setup
═══════════════════════════════════════

[1/5] Generating Powers of Tau (2^16)...
[2/5] Contributing to ceremony...
[3/5] Preparing Phase 2...
[4/5] Generating circuit-specific keys...
[5/5] Exporting verification key...

✅ Trusted setup complete!
   Verification key: circuits/build/verification_key.json
   Proving key:      circuits/build/circuit_final.zkey
   Powers of Tau:    circuits/trusted_setup/pot_final.ptau
   Intermediate files cleaned up.
```

---

## Step 5: WASM Build ✅

```bash
wasm-pack build --target web --out-dir pkg
```

```
✨ Done in 13m 55s
📦 Your wasm pkg is ready to publish at /home/rajakash/solid-protocol/wasm/pkg.
```

- 1 warning: `SasAttestationBuilder.attester` field unused (non-blocking)
- Node.js target (`pkg-node/`) building in parallel

---

## Step 5b: WASM Build (Node.js) ✅

```bash
wasm-pack build --target nodejs --out-dir pkg-node
```

```
✨ Done in 12m 42s
📦 Your wasm pkg is ready to publish at /home/rajakash/solid-protocol/wasm/pkg-node.
```

---

## Step 6: Anchor Build ✅

**5 attempts total** — each uncovered a different issue:

| Attempt | Issue | Fix |
|---|---|---|
| 1 | Missing `overflow-checks = true` | Added `[profile.release]` to workspace Cargo.toml |
| 2 | Program ID mismatch | `anchor keys sync` |
| 3 | Platform-tools download timeout | Network retry |
| 4 | `groth16-solana` API mismatch (no `::new()`, const generic) | Full rewrite of `zk-verifier/src/lib.rs` |
| 5 | `init_if_needed` missing on `issuer-registry` | Added `features = ["init-if-needed"]` |

**Final result:** All 3 programs compiled (warnings only from anchor/solana version mismatch — harmless).

```
target/deploy/schema_registry.so   ✅
target/deploy/issuer_registry.so   ✅
target/deploy/zk_verifier.so       ✅
```

---

## Step 8: TS SDK Build ✅

```bash
cd ts-sdk && npm run build
```

```
> @solid-protocol/core@0.1.0 build     ✅
> @solid-protocol/holder@0.1.0 build   ✅
> @solid-protocol/issuer@0.1.0 build   ✅
> @solid-protocol/light@0.1.0 build    ✅
> @solid-protocol/verifier@0.1.0 build ✅
```

All 5 packages compiled with zero errors.

---

## Step 7: Deploy to Devnet ✅

```bash
anchor deploy --provider.cluster devnet
```

| Program | Program ID | IDL Metadata |
|---|---|---|
| `schema_registry` | `2oma2yU2vfSYNR8sPDq5JvRbrtetyk5wuuGzwGYupAsH` | `3JeBDMjUUJCTDiV2zfdUPzNwTALMZ6W8v1g5Wkrnw5rJ` |
| `zk_verifier` | `FhtEvsUwxRRT8nf3uaxK6iAefFqvqCYimc9vqnH5XEgr` | `DLYCcVqWBMT5XLDV8bKUFHYaEsDRuad9AZ7KEzZUWqHx` |
| `issuer_registry` | `6ewriDVJLTaeBG7AsjuobAfyhDWfCz681RDWMFq5Laeo` | `6GFK3AB8ZdTtu3zAGtgcdtQph5nW4Y9gBMXPkaXy7swK` |

**Deployer/Authority:** `Hz4uJrCLqs9rqMHJyvD9tqWgSNc92TjsBYANUUNxLWWv`
**Balance after deploy:** ~6.98 SOL
**Full manifest:** `deployments/devnet.json`

---

## Issues Log

| # | Issue | Severity | Status | Resolution |
|---|---|---|---|---|
| 1 | R1CS non-quadratic constraints in predicates | Blocker | ✅ Fixed | Split signal products into intermediates |
| 2 | Missing `comparators.circom` import | Blocker | ✅ Fixed | Added import |
| 3 | `snarkjs.bn128` undefined in snarkjs 0.7+ | Blocker | ✅ Fixed | Use `getCurveFromName('bn128')` from ffjavascript |
| 4 | `getrandom` WASM compile error | Blocker | ✅ Fixed | Added `getrandom = { features = ["js"] }` to wasm/Cargo.toml |
| 5 | Anchor build: `overflow-checks` not enabled | Blocker | ✅ Fixed | Added `[profile.release]` to workspace Cargo.toml |
| 6 | Anchor build: program ID mismatch | Blocker | ✅ Fixed | `anchor keys sync` |
| 7 | `BigUint64Array` needs `bigint[]` not `number[]` | Blocker | ✅ Fixed | Removed `.map(Number)` in core/index.ts |
| 8 | Light SDK API property name changes | Blocker | ✅ Fixed | Used `any` + nullish coalescing for version compat |
| 9 | groth16-solana API: const generic + struct VK | Blocker | ✅ Fixed | Full rewrite using `Groth16Verifier::<NR_PUBLIC_INPUTS>`, VK deserialization from bytes |
| 10 | `init_if_needed` feature missing on issuer-registry | Blocker | ✅ Fixed | Added `features = ["init-if-needed"]` to anchor-lang dep |
| | | | | |
