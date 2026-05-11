# SolID Protocol Real E2E Runbook

This is the practical operator and integrator guide for running SolID
locally, understanding the four actor model, and testing real end-to-end
credential flows with different schemas and predicates.

The goal is simple: after cloning this repository, a developer should be
able to run the protocol locally, watch each actor do its job, issue a
credential, generate a zero-knowledge proof, verify it on-chain, and then
change the schema or query to test a different angle.

This file is intentionally root-level because it is the first thing a new
protocol contributor, issuer, verifier, or DAO operator should be able to
find.

---

## 1. Current Reality

SolID Protocol has three on-chain programs and a low-level TypeScript SDK:

| Layer | Component | Purpose |
| --- | --- | --- |
| Program | `schema-registry` | Registers credential schemas, binds schema trees, and stores global identity root state. |
| Program | `issuer-registry` | DAO-governed issuer registry, issuer staking, issuer approval, issuer-tree binding, and credential issuance via SPL Account Compression CPI. |
| Program | `zk-verifier` | Verifies Groth16 batch proofs and creates one nullifier PDA per proof to stop replay. |
| SDK | `ts-sdk/packages/*` | Core cryptography, holder proof generation, issuer issuance helpers, verifier transaction helpers, and SPL AC adapters. |
| Circuits | `circuits/*` | Batch credential query circuit plus issuer BJJ subgroup proof circuit. |
| Scripts | `scripts/*.ts` | Local operator path: initialize, create trees, approve issuer, issue credential, prove, and verify. |

Important distinction:

- The protocol E2E path in this repository is real and reaches on-chain
  verification on localnet.
- The product wrappers, hosted APIs, wallet UX, and one-call verifier
  facade are still evolving outside this runbook.
- Current local scripts are intentionally opinionated and mostly use
  `basic_identity_v1`. Testing new schemas today usually means editing
  the local scripts or copying them into scenario-specific scripts.

---

## 2. Actor Model

SolID has four main actors:

1. DAO
2. Issuer
3. Holder
4. Verifier

The protocol only works when these actors line up correctly. A verifier
cannot just trust any proof; it must trust the schema, the issuer, the
issuer-tree root, the schema-tree root, and the nullifier contract.

### 2.1 DAO

The DAO is the trust and governance layer.

What the DAO controls:

- Program initialization parameters.
- Governance token mint used for issuer voting.
- Minimum issuer stake.
- Voting period.
- Approval threshold.
- Trusted issuer admission.
- Issuer slashing and revocation.
- Schema authority policy.
- Verification key upload, finalization, and rotation policy.
- Issuer-tree binding operator policy.

In local E2E, the DAO is represented by the local wallet at
`~/.config/solana/id.json` and by the scripts:

```bash
npm run init-onchain
npm run bootstrap-issuer
```

In a real deployment, the DAO should not be a single local key. For devnet
testing, a single key is acceptable if it is clearly labeled. For mainnet,
this must become multisig, timelock, or a stronger governance path.

### 2.2 Issuer

An issuer is an entity allowed to issue credentials. Examples:

- KYC provider
- University
- Hospital
- DAO
- Government or residency provider
- Accredited investor verifier
- Game or reputation authority

What an issuer does:

1. Generates a BabyJubJub issuer identity.
2. Proves the BJJ public key is valid and in the prime-order subgroup.
3. Registers with `issuer-registry`.
4. Stakes collateral.
5. Waits for DAO approval.
6. Issues holder credentials after verifying off-chain facts.
7. Appends credential commitments to the schema credential tree through
   `issuer-registry::issue_credential`.
8. Delivers the full private credential package to the holder off-chain.

The issuer must never publish raw user PII on-chain. The on-chain leaf is
only a commitment and related registry state.

### 2.3 Holder

The holder is the user who owns credentials and generates proofs.

What a holder does:

1. Creates or imports a master identity key.
2. Derives a per-schema credential key.
3. Receives a private credential package from an issuer.
4. Stores the credential locally.
5. Receives a query from a verifier.
6. Chooses whether to answer the query.
7. Generates a Groth16 proof locally.
8. Sends proof bytes and public signals to the verifier.

The holder should reveal only the predicate result. For example, the user
can prove `age >= 21` without sending the verifier the actual age.

### 2.4 Verifier

The verifier is the app that asks, "Can this wallet perform this action?"

Examples:

- Launchpad checking jurisdiction or accreditation.
- DeFi protocol checking KYC level.
- DAO checking membership.
- Game checking proof of human or proof of region.
- Marketplace checking seller certification.

What a verifier does:

1. Builds a query.
2. Adds a fresh verifier nonce.
3. Sends the query to a holder.
4. Receives proof data from the holder.
5. Submits the proof to `zk-verifier`.
6. Checks the transaction result and `verified: true`.
7. Allows or denies the app action.

The verifier does not need to see raw credential data. It only needs the
proof, public signals, trust-root accounts, and a successful on-chain
verification result.

---

## 3. Real End-to-End Flow

This is the intended real protocol path.

### Step 1: DAO initializes the system

The DAO deploys and initializes:

- `issuer-registry`
- `schema-registry`
- `zk-verifier`
- Governance mint and governance vault.
- Batch verification key.
- Subgroup verification key.
- Global state binding.

Local script:

```bash
npm run init-onchain
```

What it does:

- Calls `issuer_registry::initialize_registry`.
- Registers the default schema.
- Initializes global binding.
- Initializes `zk-verifier`.
- Uploads and finalizes the batch verification key.
- Uploads and finalizes the subgroup verification key.
- Writes local E2E state to a secure temp path.

### Step 2: DAO or operator creates trust trees

Two tree families matter:

| Tree | Why it exists |
| --- | --- |
| Issuer tree | Binds approved issuer authority, BJJ key, status epoch, and revocation nonce into the proof system. |
| Schema credential tree | Stores credential commitments for one schema. |

Local scripts:

```bash
npm run backfill-issuer-tree
npm run bootstrap-schema-tree
```

The scripts create SPL Account Compression trees and bind them to the
registry PDAs. These bindings are init-only. If a binding points to the
wrong tree, use a fresh local validator or a new schema version.

### Step 3: Issuer registers and gets approved

Local script:

```bash
npm run bootstrap-issuer
```

What it does:

- Creates or reuses a governance mint from local state.
- Generates an issuer BJJ keypair.
- Generates a subgroup Groth16 proof for the issuer BJJ public key.
- Registers the issuer.
- Stakes test lamports.
- Mints governance tokens for voting.
- Votes to approve the issuer.
- Waits for the local voting period.
- Finalizes the vote.
- Enrolls the approved issuer into the issuer tree.

For local E2E, use a short voting period:

```bash
export SOLID_VOTING_PERIOD_SECONDS=120
```

### Step 4: Issuer issues a credential

Local script:

```bash
npm run issue
```

The default local credential is `basic_identity_v1` with field values like:

| Index | Field | Example value |
| --- | --- | --- |
| 0 | `age` | `25` |
| 1 | `country_code` | `840` |
| 2 | `region` | `1` |
| 3 | `id_type` | `0` |
| 4 | `verification_level` | `2` |
| 5 | `issued_date` | current Unix timestamp |
| 6 | `nationality` | `840` |
| 7 | `_reserved` | `0` |

The issuer signs the credential with its BJJ key and calls
`issuer-registry::issue_credential`, which CPIs into SPL Account
Compression to append the commitment.

The full private credential package is persisted only for local E2E state.
In production, this package is delivered to the holder through an encrypted
off-chain channel.

### Step 5: Holder generates a proof

Local script:

```bash
npm run prove
```

The script reconstructs holder state from the E2E state file, builds a
query, seeds a local Merkle replica for test-validator, generates a Groth16
proof, uploads the proof to a buffer account, and calls
`verify_batch_proof_v2`.

The default local query is:

```ts
const query = new QueryBuilder()
  .schemas(schemaHashes)
  .where(0, 0, 'GTE', 21n)
  .verifier(PROGRAM_PUBKEYS.zkVerifier.toBuffer())
  .nonce(nonce)
  .build();
```

Meaning:

- Credential slot `0`.
- Field index `0`.
- Operator `GTE`.
- Value `21`.
- In plain English: prove `age >= 21`.

### Step 6: Verifier submits proof on-chain

The verifier submits:

- Groth16 proof bytes.
- Public signals.
- Schema tree binding accounts.
- Global binding account.
- Issuer tree binding account.
- Nullifier PDA.
- Proof buffer PDA.

`zk-verifier` checks:

- Proof verifies against the finalized VK.
- Trust-root accounts are owned by the expected registry programs.
- Schema roots and issuer roots match public inputs.
- Timestamp is inside allowed bounds.
- Nullifier PDA does not already exist.

Expected successful tail:

```text
[4/4] Submitting verify_batch_proof...
   verified: true
Replay test (should fail)...
   ok (replay rejected by nullifier PDA init constraint)
Done.
```

The replay check matters. It proves the same holder, query, verifier, and
nonce cannot be reused twice.

---

## 4. Local Setup After Cloning

### 4.1 Clone and enter the repo

```bash
git clone <your-solid-protocol-repo-url> solid-protocol
cd solid-protocol
```

All commands below assume you are in the root of `solid-protocol`.

### 4.2 Install pinned toolchain

Recommended:

```bash
bash scripts/bootstrap.sh
export PATH="$PWD/.toolchain/bin:$PATH"
```

Alternative if you use Nix:

```bash
nix develop
```

Expected core versions:

```bash
rustc --version
solana --version
anchor --version
circom --version
snarkjs --version
wasm-pack --version
node --version
npm --version
```

The repository is pinned around:

- Solana CLI `1.18.22`
- Anchor `0.30.1`
- Rust `1.79.0`
- circom `2.1.9`
- snarkjs `0.7.x`
- wasm-pack `0.13.x`
- Node `18` or `20`

### 4.3 Build circuits

```bash
cd circuits
npm install
node scripts/setup.js
cd ..
```

This produces Groth16 proving and verification artifacts in
`circuits/build/`.

Important: the current trusted setup path is for local/devnet testing. A
single-party ceremony is not acceptable for mainnet.

### 4.4 Build WASM bridge

```bash
wasm-pack build wasm/ --target nodejs \
  --out-dir ../ts-sdk/packages/core/wasm --release
```

Run this whenever Rust crypto or WASM bridge code changes. `wasm-pack`
resolves `--out-dir` relative to the `wasm/` crate, so the repo-root
command must use `../ts-sdk/...`.

### 4.5 Sync program keypairs and build programs

```bash
bash scripts/sync_program_keypairs.sh --reset-state
anchor build
```

If you only want a fast compile check without IDL regeneration:

```bash
anchor build --no-idl
```

### 4.6 Build TypeScript SDK

```bash
cd ts-sdk
npm ci
npm run build
cd ..
```

### 4.7 Install root script dependencies

```bash
npm install
```

---

## 5. Run Clean Localnet E2E

Open one terminal for the validator.

```bash
export PATH="$PWD/.toolchain/bin:$PATH"

COPYFILE_DISABLE=1 COPY_EXTENDED_ATTRIBUTES_DISABLE=1 \
  solana-test-validator --reset \
    --clone-upgradeable-program cmtDvXumGCrqC1Age74AVPhSRVXJMd8PJS91L8KbNCK \
    --clone-upgradeable-program noopb9bkMVfRPU8AsbpTUg8AQkHtKwMYZiFUjNRtMmV \
    --url https://api.devnet.solana.com
```

The cloned programs are required for SPL Account Compression and Noop on
local test-validator.

Open a second terminal in the same repo.

```bash
export PATH="$PWD/.toolchain/bin:$PATH"
solana config set --url localhost
solana airdrop 10

bash scripts/sync_program_keypairs.sh --reset-state
anchor deploy --provider.cluster localnet

export SOLID_VOTING_PERIOD_SECONDS=120
npm run e2e
```

`npm run e2e` expands to:

```bash
npm run build:idl
npm run init-onchain
npm run backfill-issuer-tree
npm run bootstrap-schema-tree
npm run bootstrap-issuer
npm run issue
npm run prove
```

If the final output says `verified: true` and the replay attempt is
rejected, the local end-to-end protocol path is working.

---

## 6. Run the E2E Manually, One Actor at a Time

Use this when debugging or teaching the system.

### 6.1 Build IDLs

```bash
npm run build:idl
```

Actor view:

- Operator prepares the TypeScript scripts to call Anchor programs.

### 6.2 DAO initializes on-chain config

```bash
export SOLID_VOTING_PERIOD_SECONDS=120
npm run init-onchain
```

Actor view:

- DAO initializes registry config.
- DAO registers default schema.
- DAO initializes verifier.
- DAO uploads/finalizes VKs.

### 6.3 Operator creates issuer tree

```bash
npm run backfill-issuer-tree
```

Actor view:

- Operator creates issuer tree.
- Operator binds issuer tree to `issuer-registry`.

### 6.4 Operator creates schema credential tree

```bash
npm run bootstrap-schema-tree
```

Actor view:

- Operator creates the per-schema credential commitment tree.
- Operator transfers tree authority to the protocol PDA.
- Operator binds the tree to `schema-registry`.

### 6.5 Issuer registers and DAO approves

```bash
npm run bootstrap-issuer
```

Actor view:

- Issuer creates BJJ identity.
- Issuer proves subgroup membership.
- Issuer stakes.
- DAO votes.
- DAO finalizes.
- Approved issuer enters issuer tree.

### 6.6 Issuer issues credential

```bash
npm run issue
```

Actor view:

- Issuer prepares attestation data.
- Issuer signs holder credential.
- Issuer appends the commitment on-chain.
- Holder receives private credential package in local state.

### 6.7 Holder proves and verifier verifies

```bash
npm run prove
```

Actor view:

- Verifier builds query.
- Holder generates proof.
- Verifier submits proof on-chain.
- `zk-verifier` accepts once.
- Replay with same nullifier fails.

---

## 7. E2E State File

The scripts share local-only state through a temp file, not through the git
working tree.

Default path is selected by `scripts/lib/e2e_state.ts`:

- `$SOLID_E2E_STATE_FILE`, if set.
- `$XDG_RUNTIME_DIR/solid-e2e/state.json`, if available.
- `$TMPDIR/solid-e2e-<uid>/state.json`.
- `/tmp/solid-e2e-<uid>/state.json`.

To make a scenario explicit:

```bash
export SOLID_E2E_STATE_FILE=/tmp/solid-basic-identity/state.json
```

To reset a local run:

```bash
bash scripts/sync_program_keypairs.sh --reset-state
```

For a truly clean E2E, also restart `solana-test-validator --reset`.

---

## 8. Testing Different Schemas

The current scripted E2E is centered on `basic_identity_v1`.

To test another schema locally, change or copy three script surfaces:

1. Schema definition in `scripts/initialize.ts`.
2. Credential values in `scripts/issue.ts`.
3. Verifier query in `scripts/prove.ts`.

For clean experiments, copy the scripts first:

```bash
cp scripts/initialize.ts scripts/initialize_accredited.ts
cp scripts/issue.ts scripts/issue_accredited.ts
cp scripts/prove.ts scripts/prove_accredited.ts
```

Then run them with `npx tsx`:

```bash
npx tsx scripts/initialize_accredited.ts
npx tsx scripts/bootstrap_schema_tree.ts
npx tsx scripts/bootstrap_issuer.ts
npx tsx scripts/issue_accredited.ts
npx tsx scripts/prove_accredited.ts
```

If you keep using the standard `bootstrap_schema_tree.ts`, make sure the
state file contains the schema hash and schema PDA produced by your custom
initialize script.

### 8.1 Schema constraints

The current batch circuit expects:

- Up to 4 credential slots per batch.
- Up to 8 fields per credential.
- Field values as `u64`-style numeric values.
- Booleans encoded as `0` or `1`.
- Enums encoded as numeric tags.
- Timestamps encoded as Unix seconds.

### 8.2 Example schema: accredited investor

In a copied initialize script, replace:

```ts
const SCHEMA_NAME = 'basic_identity_v1';
const SCHEMA_VERSION = 1;
const SCHEMA_FIELDS = [
  'age', 'country_code', 'region', 'id_type',
  'verification_level', 'issued_date', 'nationality', '_reserved',
];
```

with:

```ts
const SCHEMA_NAME = 'accredited_investor_v1';
const SCHEMA_VERSION = 1;
const SCHEMA_FIELDS = [
  'accredited_status',
  'country_code',
  'investor_type',
  'verification_level',
  'verified_at',
  'expires_at',
  'issuer_tier',
  '_reserved',
];
```

In the copied issue script, replace the default attestation data with:

```ts
const now = Math.floor(Date.now() / 1000);
const attestationData: bigint[] = [
  1n,                 // accredited_status: 1 = yes
  840n,               // country_code: US
  0n,                 // investor_type: individual
  2n,                 // verification_level
  BigInt(now),        // verified_at
  BigInt(now + 31536000), // expires_at: one year
  1n,                 // issuer_tier
  0n,                 // _reserved
];
```

In the copied prove script, replace the query with:

```ts
const query = new QueryBuilder()
  .schemas(schemaHashes)
  .where(0, 0, 'EQ', 1n)      // accredited_status == yes
  .and(0, 5, 'GTE', BigInt(Math.floor(Date.now() / 1000)))
  .verifier(PROGRAM_PUBKEYS.zkVerifier.toBuffer())
  .nonce(nonce)
  .build();
```

This checks:

- The holder is accredited.
- The credential has not expired.

### 8.3 Example schema: geography gate

Fields:

```ts
const SCHEMA_NAME = 'geo_eligibility_v1';
const SCHEMA_VERSION = 1;
const SCHEMA_FIELDS = [
  'country_code',
  'region_code',
  'kyc_level',
  'sanctions_clear',
  'verified_at',
  'expires_at',
  'provider_code',
  '_reserved',
];
```

Credential data:

```ts
const now = Math.floor(Date.now() / 1000);
const attestationData: bigint[] = [
  356n,               // country_code: India
  29n,                // region_code
  2n,                 // kyc_level
  1n,                 // sanctions_clear
  BigInt(now),
  BigInt(now + 7776000), // 90 days
  7n,                 // provider_code
  0n,
];
```

Query:

```ts
const query = new QueryBuilder()
  .schemas(schemaHashes)
  .where(0, 0, 'EQ', 356n)    // country_code == India
  .and(0, 2, 'GTE', 2n)       // kyc_level >= 2
  .and(0, 3, 'EQ', 1n)        // sanctions_clear == true
  .verifier(PROGRAM_PUBKEYS.zkVerifier.toBuffer())
  .nonce(nonce)
  .build();
```

### 8.4 When to reset localnet

Reset localnet when:

- You reuse a schema name/version with different fields.
- A tree binding was initialized with the wrong tree.
- Program keypairs were regenerated.
- You want to clear nullifier PDAs.
- You want to replay the exact same proof from scratch.

Use:

```bash
# Stop validator, then restart:
COPYFILE_DISABLE=1 COPY_EXTENDED_ATTRIBUTES_DISABLE=1 \
  solana-test-validator --reset \
    --clone-upgradeable-program cmtDvXumGCrqC1Age74AVPhSRVXJMd8PJS91L8KbNCK \
    --clone-upgradeable-program noopb9bkMVfRPU8AsbpTUg8AQkHtKwMYZiFUjNRtMmV \
    --url https://api.devnet.solana.com
```

Then redeploy:

```bash
bash scripts/sync_program_keypairs.sh --reset-state
anchor deploy --provider.cluster localnet
```

---

## 9. Testing Different Angles

### 9.1 Happy path

Run:

```bash
npm run e2e
```

Expected:

- Issuer approved.
- Credential issued.
- Proof generated.
- `verified: true`.
- Replay rejected.

### 9.2 Replay protection

The current `prove.ts` already performs a replay test. It submits the
same proof/nullifier again and expects failure.

Expected:

```text
Replay test (should fail)...
   ok (replay rejected by nullifier PDA init constraint)
```

### 9.3 Predicate failure

In `scripts/prove.ts`, change:

```ts
.where(0, 0, 'GTE', 21n)
```

to:

```ts
.where(0, 0, 'GTE', 26n)
```

The default issued age is `25`, so the proof should not reach a successful
`verified: true` result for the same expected query semantics.

### 9.4 Wrong country

For `basic_identity_v1`, field `1` is `country_code`.

Default credential:

```text
country_code = 840
```

Query for a different country:

```ts
const query = new QueryBuilder()
  .schemas(schemaHashes)
  .where(0, 1, 'EQ', 356n)
  .verifier(PROGRAM_PUBKEYS.zkVerifier.toBuffer())
  .nonce(nonce)
  .build();
```

Expected:

- The proof should not verify as eligible for that query.

### 9.5 Multi-predicate AND

```ts
const query = new QueryBuilder()
  .schemas(schemaHashes)
  .where(0, 0, 'GTE', 21n) // age >= 21
  .and(0, 1, 'EQ', 840n)   // country == US
  .and(0, 4, 'GTE', 2n)    // verification_level >= 2
  .verifier(PROGRAM_PUBKEYS.zkVerifier.toBuffer())
  .nonce(nonce)
  .build();
```

Expected:

- Default local credential should pass.

### 9.6 Fresh verifier nonce

In `scripts/prove.ts`, the nonce is deterministic for repeatable tests:

```ts
const nonce = new Uint8Array(32);
for (let i = 0; i < nonce.length; i++) nonce[i] = 1;
```

For real verifier sessions, generate a fresh nonce per request:

```ts
crypto.getRandomValues(nonce);
```

or in Node:

```ts
import { randomBytes } from 'crypto';
const nonce = Uint8Array.from(randomBytes(32));
```

Fresh nonce means a new nullifier universe for that verifier session.

### 9.7 Expiration checks

To test expiration, set a credential expiration timestamp when issuing and
then prove after the expiration window. The holder SDK carries
`expirationTimestamp`, and the batch circuit includes expiration in the
witness.

Local scripts default to `0` unless the issuance request sets a value, so
add an explicit expiration in your copied issue scenario before expecting
expiration behavior.

### 9.8 Issuer revocation angle

To test issuer revocation properly:

1. Issue and verify a credential.
2. Revoke or slash the issuer.
3. Update the issuer tree root.
4. Try proving again with the old issuer state.

Expected:

- Old issuer-root proofs should fail under the updated issuer tree binding.
- New proofs use a different issuer root and nullifier universe.

This is a more advanced test than the default `npm run e2e` path.

---

## 10. Verification and Audit Commands

Run these before trusting a local branch.

### 10.1 Program ID consistency

```bash
python3 scripts/check_program_ids.py
python3 scripts/test_check_program_ids.py
```

### 10.2 Rust formatting and unit tests

```bash
cargo fmt --all -- --check
cargo clippy -p solid-core -p solid-light -- -D warnings

cargo test -p solid-core -p solid-light --no-fail-fast
cargo test -p zk-verifier --lib --no-fail-fast
cargo test -p issuer-registry --lib --no-fail-fast
cargo test -p schema-registry --lib --no-fail-fast
```

### 10.3 Circuit tests

```bash
cd circuits
PATH="$PWD/../.toolchain/bin:$PATH" npm test
cd ..
```

### 10.4 TypeScript SDK build

```bash
cd ts-sdk
npm run build
cd ..
```

### 10.5 Cross-language vectors

```bash
npm run check-vectors
```

### 10.6 Selected TypeScript regression tests

```bash
npx tsx tests/unit/artifact_integrity.test.ts
npx tsx tests/unit/discriminator_recompute.test.ts
npx tsx tests/unit/holder_debug_redaction.test.ts
```

### 10.7 Anchor BPF build

```bash
anchor build --no-idl
```

### 10.8 Compute budget measurement

After a successful local E2E, inspect or measure CU with:

```bash
python3 scripts/measure_cu.py --help
```

Use the documented CU baselines in `docs/CU_BUDGET.md` and
`tests/cu_baselines.json`.

---

## 11. Environment Variables

Common variables:

| Variable | Used by | Meaning |
| --- | --- | --- |
| `SOLID_RPC_URL` | scripts | RPC endpoint. Defaults to `http://127.0.0.1:8899`. |
| `SOLID_E2E_STATE_FILE` | scripts | Explicit path for local E2E shared state. |
| `SOLID_VOTING_PERIOD_SECONDS` | initialize/bootstrap issuer | Local voting period. Use `120` for comfortable E2E runs. |
| `SOLID_STAKE_AMOUNT` | bootstrap issuer | Test stake amount for issuer registration. |
| `SOLID_MIN_STAKE_LAMPORTS` | initialize/bootstrap issuer | Registry minimum stake. |
| `SOLID_GOVERNANCE_MINT` | initialize/bootstrap issuer | Override governance token mint. |
| `SOLID_SCHEMA_TREE_DEPTH` | bootstrap schema tree | Credential tree depth. Must remain compatible with circuit depth. |
| `SOLID_SCHEMA_TREE_BUFFER` | bootstrap schema tree | SPL AC buffer size. |
| `SOLID_SCHEMA_TREE_CANOPY` | bootstrap schema tree | SPL AC canopy depth. |
| `SOLID_ISSUER_TREE_DEPTH` | backfill issuer tree | Issuer tree depth. |
| `SOLID_ISSUER_TREE_BUFFER` | backfill issuer tree | Issuer tree buffer size. |
| `SOLID_ISSUER_TREE_CANOPY` | backfill issuer tree | Issuer tree canopy depth. |
| `SOLID_TREE_PUBKEY` | initialize | Use an existing schema credential tree binding. |
| `SOLID_ISSUER_TREE_PUBKEY` | initialize | Use an existing issuer tree binding. |
| `SOLID_VK_SHA256` | initialize | Expected batch VK hash. |
| `SOLID_SUBGROUP_VK_SHA256` | initialize | Expected subgroup VK hash. |
| `SOLID_ALLOW_NON_LOCALNET` | bootstrap scripts | Allows scripts to run against non-localnet RPC. Use carefully. |

For local E2E, the most common exports are:

```bash
export SOLID_RPC_URL=http://127.0.0.1:8899
export SOLID_VOTING_PERIOD_SECONDS=120
export SOLID_E2E_STATE_FILE=/tmp/solid-e2e/state.json
```

---

## 12. Local Development Checklist

Use this checklist when making changes.

### Protocol or Rust change

```bash
cargo fmt --all -- --check
cargo clippy -p solid-core -p solid-light -- -D warnings
cargo test -p solid-core -p solid-light --no-fail-fast
cargo test -p zk-verifier --lib --no-fail-fast
cargo test -p issuer-registry --lib --no-fail-fast
cargo test -p schema-registry --lib --no-fail-fast
anchor build --no-idl
```

### Circuit change

```bash
cd circuits
npm install
node scripts/setup.js
npm test
cd ..

npm run check-vectors
npm run e2e
```

### WASM or cryptography bridge change

```bash
wasm-pack build wasm/ --target nodejs \
  --out-dir ../ts-sdk/packages/core/wasm --release

cd ts-sdk
npm ci
npm run build
cd ..

npm run check-vectors
```

### SDK change

```bash
cd ts-sdk
npm ci
npm run build
cd ..

npx tsx tests/unit/artifact_integrity.test.ts
npx tsx tests/unit/discriminator_recompute.test.ts
npx tsx tests/unit/holder_debug_redaction.test.ts
```

### Full local release candidate

```bash
python3 scripts/check_program_ids.py
cargo fmt --all -- --check
cargo test -p solid-core -p solid-light --no-fail-fast
cargo test -p zk-verifier --lib --no-fail-fast
cargo test -p issuer-registry --lib --no-fail-fast
cargo test -p schema-registry --lib --no-fail-fast

cd circuits && PATH="$PWD/../.toolchain/bin:$PATH" npm test && cd ..
cd ts-sdk && npm run build && cd ..
npm run check-vectors
anchor build --no-idl
npm run e2e
```

---

## 13. Production Notes

Before public devnet feedback:

- Deploy the three programs to devnet.
- Regenerate `deployments/devnet.json` with real deploy metadata.
- Host circuit artifacts with hash pins.
- Replace manual local Merkle replicas with a real indexer or DAS adapter.
- Ship a high-level verifier wrapper.
- Ship holder credential storage and proof-request UX.
- Make issuer registration and credential issuance usable from an issuer
  CLI or dashboard.

Before mainnet:

- Replace single-party trusted setup with a multi-party ceremony.
- Move DAO/slash/fraud authority to multisig or governance.
- Move issuer-tree operator to multisig, DAO, or hardened automation.
- Close all high and medium findings in `sec/SECURITY_REGISTRY.md` that
  affect production operation.
- Run an external audit on programs, circuits, SDK, scripts, and product
  surfaces together.

---

## 14. The Mental Model to Keep

SolID is not just "a proof verifies."

A real SolID verification means:

1. The schema exists and is the schema the verifier asked for.
2. The credential commitment exists in the schema credential tree.
3. The issuer is approved in the issuer tree.
4. The holder knows the private credential data.
5. The holder satisfies the verifier's predicate.
6. The proof is bound to the verifier address and verifier nonce.
7. The proof is bound to the current issuer root.
8. The proof is bound to the registered schema roots.
9. The proof has not expired.
10. The nullifier has never been used before.

If all ten are true, the verifier gets a usable answer:

```text
eligible: true
```

without receiving the holder's private data.

