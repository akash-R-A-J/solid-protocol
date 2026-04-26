# SolID Protocol -- Module Contracts

Every component in this system has exactly one contract: a precise definition of what it
accepts, what it produces, what invariants it enforces, and what it deliberately does NOT do.
Keeping these contracts honest is what makes integration possible without workarounds.

Read this document before touching any component. When a contract changes, the corresponding
tests, docs, and downstream contracts must change in the same PR.

Date written: 2026-04-23
Based on: v0.4 audit (sec/audits/2026-04-22_v0.4_comprehensive_audit_and_build_plan.md)

---

## Table of Contents

1. Feature Inventory (must-have, left to implement, not started)
2. System Data Flow (one-page map)
3. Circuit Contracts
   3.1 batch_credential_query.circom
   3.2 compound_query.circom
   3.3 lib/identity_anchor.circom
   3.4 lib/credential_atom.circom
   3.5 lib/predicate_evaluator.circom (includes FieldSelector, BatchFieldSelector)
   3.6 lib/nullifier_expiry.circom (includes ExpirationChecker)
4. On-Chain Program Contracts
   4.1 zk-verifier
   4.2 issuer-registry
   4.3 schema-registry
5. Rust Crate Contracts
   5.1 solid-core
   5.2 solid-light
   5.3 wasm/ (WASM bridge)
6. TypeScript SDK Contracts
   6.1 @solid-protocol/core (wasm wrapper + types)
   6.2 @solid-protocol/holder (proof generation)
   6.3 @solid-protocol/verifier (on-chain submission)
   6.4 @solid-protocol/sdk (top-level facade)
7. Script Contracts
   7.1 scripts/bootstrap_issuer.ts
   7.2 scripts/initialize.ts
   7.3 scripts/issue.ts
   7.4 scripts/prove.ts
8. Test Contracts
   8.1 Integration tests (tests/integration/)
   8.2 Unit tests (in crates + programs)
   8.3 Cross-language vector tests (tests/vectors/)
9. Integration Invariants (what must be true at every boundary)

---

## 1. Feature Inventory

### 1.1 Must-Have for Any Real Deployment (Core)

These are non-negotiable. The system is a ZK identity protocol. Without these it is not
a ZK identity protocol.

| Feature | Status | Component | Blocking Finding |
|---------|--------|-----------|-----------------|
| Schema registration with integrity check | BROKEN | schema-registry | SOLID-SEC-002 |
| Credential issuance bound to registered schema | BROKEN | issuer-registry | SOLID-SEC-003 |
| Batch credential proof (up to 4 creds) | BROKEN (soundness) | circuits | SOLID-SEC-001 |
| Proof of fewer than 4 credentials | BROKEN | circuits | SOLID-SEC-029 |
| On-chain proof verification | WORKING | zk-verifier | -- |
| Nullifier replay protection | WORKING | zk-verifier | -- |
| Schema-tree root binding | WORKING | schema-registry + zk-verifier | -- |
| Global identity state root binding | WORKING | schema-registry + zk-verifier | -- |
| DAO-governed issuer approval (voting) | WORKING | issuer-registry | -- |
| Trust-anchor fast-path approval | WORKING | issuer-registry | -- |
| Issuer slashing + lamport transfer | WORKING | issuer-registry | -- |
| WASM bridge for crypto primitives | WORKING (code) | wasm/ | SOLID-SEC-028 (path) |
| Holder key derivation (per-schema BJJ) | WORKING | wasm/ + holder SDK | -- |
| 6-arg hardened nullifier (ADR-0006 rev) | WORKING | wasm/ + circuits | -- |
| Cross-language vector gate | PARTIAL (3/10) | tests/vectors/ | SOLID-SEC-010 |
| Timestamp expiry enforcement | WORKING (circuit) | circuits | -- |
| Timestamp bound to on-chain Clock | WORKING | zk-verifier | SOLID-SEC-005 (Fixed) |
| E2E scripts runnable by stranger | WORKING (`npm run e2e`) | scripts/ + ts-sdk | SOLID-SEC-011 (Fixed) |

### 1.2 Left to Implement (Specified, Not Wired)

These are designed and partially implemented. Completing them makes the system fully functional.

| Feature | What Exists | What Is Missing |
|---------|-------------|-----------------|
| Revocation v1 | Circuit + on-chain root hooks | Holder SDK helper, Issuer SDK helper, RevocationEvent |
| In-circuit issuer pubkey binding | `check_issuer_status` instruction (unused) | Compressed issuer tree, Merkle public input, CPI wiring |
| Clock-bind timestamp | -- | 5 lines in verify_batch_proof |
| VK versioning (freeze-gate) | Chunk upload exists | vk_frozen flag, vk_generation field, grace-window logic |
| Multi-predicate E2E proof | Circuit supports 4 | No E2E test exercising >1 predicate |
| Integration tests 02-11 | Specs written in README | 10 test files not created yet |
| Cross-language vectors for 8 more primitives | 2 exist | 8 more in gen_vectors.rs |
| Schema events (for indexer) | -- | 5 emit!() calls in schema-registry |
| Authority two-step transfer | -- | propose_authority + accept_authority instructions |
| bootstrap_issuer.ts script | -- | New script needed |

### 1.3 Not Started (Phase 3/4 Scope)

These are strategic items with no code. Do not conflate with the above.

| Feature | Why It Matters |
|---------|---------------|
| Multi-party trusted setup ceremony | Soundness guarantee for mainnet |
| Per-issuer stake vaults | Isolation of slashing risk |
| Governance multisig (Squads) | Decentralize authority keys |
| Recursive proof aggregation | Scale to 1000+ verifies/sec |
| Native mobile prover (uniffi) | Consumer identity below 10s prove time |
| Attested TLS credential minting | Largest feature unlock |
| Nullifier scale (epoch Bloom / SMT) | PDA-per-nullifier does not scale past ~10^5/month |
| External audit | Gate for mainnet |
| W3C VC translation layer | Interoperability |
| Cross-chain root anchoring | Portability |

---

## 2. System Data Flow

```
ISSUANCE SIDE                             VERIFICATION SIDE
--------------                            -----------------

[Issuer] holds BJJ keypair (solid-core)
    |
    | signs commitment = Poseidon(dataHash, schemaHash, holderPubKeyAx, holderPubKeyAy, salt)
    |
    v
[issuer-registry::issue_credential]
    Inputs:  schema_hash [u8;32], commitment [u8;32], merkle_tree account
    Checks:  issuer.status == Approved
             issuer.authority == signer
             tree_authority PDA correct
             schema_account exists + hash matches  (SEC-003 fix pending)
             SchemaTreeBinding.tree_pubkey matches  (SEC-003 fix pending)
    Action:  SPL AC append(commitment) via CPI
    Output:  emit!(CredentialIssued), issuer.credentials_issued += 1

    |
    | SPL AC Merkle tree updated (off-chain indexer watches CredentialIssued,
    |   computes new root, calls schema-registry::update_tree_root)
    v
[schema-registry::SchemaTreeBinding]
    Stores:  discriminator | schema_hash | tree_pubkey | last_slot | current_root | status | authority

    |
    | Holder SDK reads tree, computes Merkle path
    v
[wasm/src/lib.rs] + [circuits/batch_credential_query.circom]
    Private inputs:  masterIdentityKey, per-credential data, salts, issuer sigs,
                     Merkle siblings (credential tree), Merkle siblings (global tree),
                     revocationNonce,
                     ADR-0014: issuerAuthorities[4], issuerStatusEpochs[4],
                     issuerRevocationNonces[4], issuerSiblings[4][16],
                     issuerPathIndices[4][16]
    Public inputs:   [0] nullifierHash (output; Poseidon(6) post ADR-0014)
                     [1] globalRoot
                     [2..5] merkleRoots[4]
                     [6..9] schemaHashes[4]
                     [10] issuerTreeRoot                         (ADR-0014)
                     [11..14] queryCredentialIndices[4]
                     [15..18] queryFieldIndices[4]
                     [19..22] queryOperators[4]
                     [23..26] queryValues[4]
                     [27] numPredicates
                     [28] compoundLogic (0=AND, 1=OR)
                     [29] verifierAddress (this program's pubkey as field element)
                     [30] verifierNonce
                     [31] currentTimestamp
    Output:          Groth16 proof (proof_a:64, proof_b:128, proof_c:64) + 32 public signals

    |
    v
[zk-verifier::verify_batch_proof]
    Inputs:  proof_a, proof_b, proof_c, public_inputs[32][32], nullifier[32]
    Checks:  nullifier == public_inputs[0]
             public_inputs[VERIFIER_ADDRESS_INPUT_INDEX=29] == ID.to_bytes() (scope binding)
             global_tree.owner == SCHEMA_REGISTRY_ID
             global_tree data matches public_inputs[1]
             schema_tree_N.owner == SCHEMA_REGISTRY_ID  (per active slot)
             schema_tree_N data matches public_inputs[2+i]
             schema ordering strictly ascending
             ADR-0014: issuer_tree_binding.owner == ISSUER_REGISTRY_ID
             ADR-0014: issuer_tree_binding data matches public_inputs[10]
             Groth16 pairing (alt_bn128 syscalls)
             nullifier PDA does not exist (init = replay guard)
             Clock check: public_inputs[CURRENT_TIMESTAMP_INPUT_INDEX=31]
                           within skew of now  (SEC-005 LANDED)
    Output:  emit!(ProofVerified), NullifierAccount PDA created

[Verifier / relayer] reads proof result and makes application-level decision
```

---

## 3. Circuit Contracts

### 3.1 batch_credential_query.circom

**What it is:** The root circuit. Proves a holder has n<=4 credentials that satisfy a
compound predicate without revealing the credentials or the holder's identity.

**Parameters (compile-time):**
```
TREE_DEPTH     = 20   Depth of per-schema SPL AC credential trees
GLOBAL_DEPTH   = 20   Depth of the global identity state tree
NUM_FIELDS     = 8    Attribute fields per credential
NUM_CREDS      = 4    Max credentials per batch proof
MAX_PREDICATES = 4    Max predicate clauses per query
```

**Public inputs (NR_PUBLIC_INPUTS = 32, 0-indexed; ADR-0014 revision):**
```
[0]      nullifierHash     output signal; Poseidon(6) with issuerTreeRoot
[1]      globalRoot        current global identity tree root
[2]      merkleRoots[0]    Merkle root of credential tree for cred slot 0
[3]      merkleRoots[1]    ... slot 1
[4]      merkleRoots[2]    ... slot 2
[5]      merkleRoots[3]    ... slot 3
[6]      schemaHashes[0]   schema identifier for cred slot 0
[7]      schemaHashes[1]
[8]      schemaHashes[2]
[9]      schemaHashes[3]
[10]     issuerTreeRoot    ADR-0014: singleton issuer-tree root; SEC-004 / SEC-008
[11]     queryCredentialIndices[0]   which credential slot predicate 0 targets
[12]     queryCredentialIndices[1]
[13]     queryCredentialIndices[2]
[14]     queryCredentialIndices[3]
[15]     queryFieldIndices[0]   which field within the selected credential
[16]     queryFieldIndices[1]
[17]     queryFieldIndices[2]
[18]     queryFieldIndices[3]
[19]     queryOperators[0]  0=NOOP 1=EQ 2=NE 3=GT 4=GTE 5=LT 6=LTE
[20]     queryOperators[1]
[21]     queryOperators[2]
[22]     queryOperators[3]
[23]     queryValues[0]    right-hand-side value for predicate 0
[24]     queryValues[1]
[25]     queryValues[2]
[26]     queryValues[3]
[27]     numPredicates     how many of the 4 predicate slots are active (0..4)
[28]     compoundLogic     0=AND, 1=OR
[29]     verifierAddress   this verifier program's pubkey encoded as BN254 field element
[30]     verifierNonce     caller-chosen anti-relay nonce
[31]     currentTimestamp  Unix timestamp in seconds, must match on-chain Clock
```

**Private inputs:**
```
masterIdentityKey            BabyJubJub master private key scalar (field element)
revocationNonce              u64 nonce, bumped to revoke all credentials at once
globalSiblings[4][20]        Merkle siblings for each credential's identity leaf in global tree
globalPathIndices[4][20]     Path directions
data[4][8]                   Attestation attributes (field elements, LE)
salts[4]                     Poseidon salt per credential
issuerSigR8xs[4]             EdDSA signature R8.x per credential
issuerSigR8ys[4]             EdDSA signature R8.y
issuerSigSs[4]               EdDSA signature S scalar
issuerPubKeyAxs[4]           Issuer BJJ public key Ax
issuerPubKeyAys[4]           Issuer BJJ public key Ay
merkleSiblings[4][20]        Merkle siblings for each credential in its schema tree
merklePathIndices[4][20]     Path directions
expirationTimestamps[4]      Per-credential expiry in Unix seconds (0 = no expiry)
```

**Output:**
```
nullifierHash   field element = Poseidon(masterKey, revocNonce, verifierAddr,
                                         queryContextHash, verifierNonce)
```

**What it enforces:**
1. numPredicates is in range [0, MAX_PREDICATES]
2. compoundLogic is binary {0, 1}
3. For each active credential slot (schemaHash != 0):
   - Per-schema BJJ key derived correctly from masterKey
   - Commitment computed correctly (Poseidon of data, schema, derived key, salt)
   - Issuer EdDSA signature over commitment is valid
   - Commitment is a leaf in the credential Merkle tree with root merkleRoots[i]
   - Identity leaf Poseidon(derivedAx, derivedAy, revocNonce) is in the global tree
   - Credential has not expired (currentTimestamp <= expirationTimestamp)
4. Padding slots (schemaHash == 0) have all private inputs zeroed
5. Active schemaHashes are strictly ascending (canonical ordering)
6. queryCredentialIndices and queryFieldIndices are in range  [PENDING: SEC-001 fix]
7. Identity global inclusion proof gated by enabled signal  [PENDING: SEC-029 fix]
8. Compound predicate logic (AND/OR) over active predicates evaluates to true

**What it does NOT do:**
- Does NOT verify the issuer is in the DAO-approved registry (SEC-004)
- Does NOT enforce the timestamp against on-chain Clock (that is the verifier's job)
- Does NOT commit to a specific VK version (SEC-006)
- Does NOT include epoch in nullifier (SEC-008)

**Consumes:**
- circomlib/poseidon.circom, circomlib/comparators.circom, circomlib/babyjub.circom
- lib/identity_anchor.circom, lib/credential_atom.circom
- lib/predicate_evaluator.circom, lib/nullifier_expiry.circom

**Produced artifacts (build outputs):**
```
target/circuits/batch_credential_query.r1cs
target/circuits/batch_credential_query.wasm   (witness generator)
target/circuits/batch_credential_query_final.zkey
target/circuits/verification_key.json
```

---

### 3.2 compound_query.circom

**What it is:** A simpler single-credential variant of the batch circuit for use cases
where only one credential is needed.

**Parameters:** TREE_DEPTH=20, GLOBAL_DEPTH=20, NUM_FIELDS=8, MAX_PREDICATES=4

**Public inputs (NR_PUBLIC_INPUTS_COMPOUND = 22, TBD -- not yet wired to verifier):**
```
[0]  nullifierHash
[1]  globalRoot
[2]  merkleRoot
[3]  schemaHash
[4..7]  queryFieldIndices[4]
[8..11] queryOperators[4]
[12..15] queryValues[4]
[16] numPredicates
[17] compoundLogic
[18] verifierAddress
[19] verifierNonce
[20] currentTimestamp
[21] issuerPubKeyAx  (public here unlike batch -- single issuer, smaller circuit)
```

**Status: Circuit exists. NOT wired to zk-verifier. NOT in scope for Phase 1.**

The `FieldSelector` in this circuit has the SAME index range-check bug as `BatchFieldSelector`
(SEC-001). Fix both in the same Phase 1 circuit PR.

---

### 3.3 lib/identity_anchor.circom

**What it is:** Sub-circuit. Derives the per-schema BabyJubJub keypair from the master
identity key and proves the derived identity leaf is in the global state tree.

**Inputs:**
```
signal input masterIdentityKey    BabyJubJub master private scalar
signal input revocationNonce       u64 (as field element)
signal input schemaHash            32-byte hash as field element
signal input globalRoot            globalRoot from GlobalStateBinding PDA
signal input globalSiblings[GLOBAL_DEPTH]
signal input globalPathIndices[GLOBAL_DEPTH]
signal input enabled               1 for active slot, 0 for padding  [PENDING: SEC-029]
```

**Outputs (intermediate, used by batch circuit):**
```
signal output credentialPubKeyAx   Ax of derived keypair (passed to CredentialAtom)
signal output credentialPubKeyAy   Ay of derived keypair
```

**Enforces:**
1. credentialPrivKey = Poseidon(masterIdentityKey, schemaHash)
2. (credentialPubKeyAx, credentialPubKeyAy) = BabyPbk(credentialPrivKey)
3. identityLeaf = Poseidon(credentialPubKeyAx, credentialPubKeyAy, revocationNonce)
4. MerkleInclusion(identityLeaf, globalRoot, siblings, indices) == true
   ONLY when enabled == 1. When enabled == 0, Merkle inclusion is skipped.  [PENDING fix]

**Does NOT enforce:**
- Any check on the issuer's identity
- The schemaHash matching a registered schema (program does this)

---

### 3.4 lib/credential_atom.circom

**What it is:** Sub-circuit. Validates one credential: commitment integrity, issuer
signature, and Merkle inclusion in the credential tree.

**Inputs:**
```
signal input schemaHash
signal input merkleRoot
signal input issuerPubKeyAx, issuerPubKeyAy
signal input attestationData[NUM_FIELDS]
signal input salt
signal input holderBJJPubKeyAx, holderBJJPubKeyAy   (derived key from IdentityAnchor)
signal input issuerSigR8x, issuerSigR8y, issuerSigS
signal input merkleSiblings[TREE_DEPTH], merklePathIndices[TREE_DEPTH]
```

**Outputs:**
```
signal output enabled   1 if schemaHash != 0 (active slot), 0 if padding
```

**Enforces (when enabled == 1):**
1. commitment = Poseidon(dataHash, schemaHash, holderAx, holderAy, salt)
   where dataHash = Poseidon(attestationData[0..NUM_FIELDS-1])
2. EdDSA-Poseidon signature (issuerSig, issuerPubKey) over commitment is valid
3. MerkleInclusion(commitment, merkleRoot, merkleSiblings, merklePathIndices) == true

**Enforces (for padding slots, enabled == 0):**
- All private inputs floored to zero via `isZero.out * input === 0` constraints

**Does NOT enforce:**
- That the issuer pubkey is in the DAO-approved registry (SEC-004)

**Note on IsZero:** local `IsZero` template at lines 83-90 shadows circomlib. Fix: delete the
local template and use the included one. (SEC-022)

---

### 3.5 lib/predicate_evaluator.circom

Contains three templates:

**PredicateEvaluator()**

Input: fieldValue, operator, queryValue
Output (binary): result (1 if predicate passes, 0 if fails)
Operators: 0=NOOP 1=EQ 2=NE 3=GT 4=GTE 5=LT 6=LTE
Uses: IsEqual, GreaterThan(252), LessThan(252) from circomlib

**FieldSelector(NUM_FIELDS)**

Input: data[NUM_FIELDS], index
Output: value (the field at position index; 0 if index out of range)
WARNING: No range check on `index`. Out-of-range index silently returns 0. (Part of SEC-001)
Fix: add `LessThan(8)` constraint on `index` before the mux.

**BatchFieldSelector(NUM_CREDS, NUM_FIELDS)**

Input: data[NUM_CREDS][NUM_FIELDS], credIndex, fieldIndex
Output: value (the field at [credIndex][fieldIndex]; 0 if either index out of range)
WARNING: No range check on `credIndex` OR `fieldIndex`. (Core of SEC-001)
Fix: add `LessThan(8)` constraints on both indices.

The `credIndex` range check must use NUM_CREDS as the upper bound.
The `fieldIndex` range check must use NUM_FIELDS as the upper bound.
Both checks must appear BEFORE the credential-selection mux (lines 95-107).

---

### 3.6 lib/nullifier_expiry.circom

Contains two templates:

**NullifierComputer() -- DEAD CODE**

Status: defined but never instantiated. The batch circuit computes its nullifier inline.
DO NOT use this template. It implements the old 3-arg formula:
  Poseidon(holderPrivKey, schemaHash, verifierNonce)
which disagrees with the circuit's 5-arg formula. Delete it to avoid confusion.

**ExpirationChecker()**

Input: currentTimestamp, expirationTimestamp
Output (binary): valid
Enforces:
  valid == 1 iff (expirationTimestamp == 0 OR currentTimestamp <= expirationTimestamp)
The check for expirationTimestamp == 0 is done via IsZero (from circomlib).
Used by batch_credential_query.circom:192-198 against EVERY credential slot (active + padding).
Padding slots trivially pass because expirationTimestamp is forced to 0 by the integrity constraints.

---

## 4. On-Chain Program Contracts

### 4.1 zk-verifier

**Program ID:** DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb

**What it is:** The on-chain Groth16 verifier. Accepts a proof and public inputs, runs
alt_bn128 pairing check, and atomically registers the nullifier to prevent replay.

**State accounts:**

`VerifierConfig` PDA seeds=[b"verifier-config"], space=49:
```
authority: Pubkey             (32)
proof_count: u64              (8)
vk_initialized: bool          (1)
paused: bool                  (1)
bump: u8                      (1)
next_vk_chunk: u16            (2)
timestamp_skew_seconds: u32   (4)
```
Source of truth: `programs/zk-verifier/src/lib.rs:595-616`.

`VkStorage` PDA seeds=[b"vk-storage", config_pda]:
```
data: Vec<u8>   raw serialized Groth16 VK bytes (capped at VK_MAX_BYTES)
```

`NullifierAccount` PDA seeds=[b"null", nullifier_bytes[32]]:
```
bump: u8   (existence == proof was verified; init on first verify = replay guard)
```

**Instructions:**

`initialize_verifier(authority: Pubkey)`
  - Payer: signer
  - Creates: VerifierConfig
  - Does NOT: set a VK (must call upload_vk_chunk)

`upload_vk_chunk(chunk_data: Vec<u8>, chunk_index: u8, is_final_chunk: bool)`
  - Gate: authority == signer
  - Enforces: chunk_index == next_vk_chunk (sequential)
  - Enforces: total size <= VK_MAX_BYTES
  - On final chunk: parses VK, sets vk_initialized=true
  - MISSING: freeze gate, vk_generation, grace-window (SEC-006)

`verify_batch_proof(proof_a, proof_b, proof_c, public_inputs[32][32], nullifier[32])`
  - Gate: !paused, vk_initialized
  - Accounts: verifier_config (mut), global_tree, schema_tree_info[4],
              issuer_tree_binding (ADR-0014), nullifier_account (init), ...
  - Enforces (in order):
    1. nullifier == public_inputs[0]
    2. public_inputs[VERIFIER_ADDRESS_INPUT_INDEX=29] == ID.to_bytes() (scope binding, SEC-13)
    3. Clock.unix_timestamp within skew of public_inputs[CURRENT_TIMESTAMP_INPUT_INDEX=31]
       (SEC-005 LANDED in `402fb4e`)
    4. global_tree.owner == SCHEMA_REGISTRY_ID
    5. verify_state_root_matches(global_tree.data, public_inputs[1])
    6. ADR-0014: issuer_tree_binding.owner == ISSUER_REGISTRY_ID AND
                 verify_issuer_tree_binding_for_proof(data, public_inputs[10])
    7. For each active schema slot i=0..3:
       a. schema ordering: schema[i] > schema[i-1]
       b. schema_tree_N.owner == SCHEMA_REGISTRY_ID
       c. verify_schema_root_binding(tree.data, schema_hash, public_inputs[2+i])
    8. Groth16 pairing (alt_bn128 precompile)
    9. NullifierAccount init (atomically fails if already exists)
  - Output: emit!(ProofVerified), NullifierAccount created

`pause_verifier()` / `unpause_verifier()`
  - Gate: authority == signer

`transfer_authority(new_authority: Pubkey)`
  - Gate: current authority == signer
  - MISSING: two-step propose/accept (SEC-016)

**Does NOT:**
- Decode or interpret the proof semantics (that is the circuit's job)
- Check the issuer is DAO-approved (SEC-004)
- Know anything about the holder's identity

**Consumes from:**
- solid-light/src/cpi_helpers.rs: verify_state_root_matches, verify_schema_root_binding,
  SCHEMA_REGISTRY_ID constant
- sys: alt_bn128_addition, alt_bn128_multiplication, alt_bn128_pairing (Solana syscalls)

---

### 4.2 issuer-registry

**Program ID:** 5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx

**What it is:** DAO-governed registry of approved credential issuers. Controls who may
call issue_credential. Implements token-weighted voting, staking, slashing, and trust-anchor
fast-path approval.

**State accounts:**

`RegistryConfig` PDA seeds=[b"registry-config"], space=112:
```
authority: Pubkey           (32)
governance_token_mint: Pubkey (32)
min_stake: u64              (8)
voting_period: i64          (8)
approval_threshold: u64     (8)
total_issuers: u64          (8)
active_issuers: u64         (8)
```

`IssuerAccount` PDA seeds=[b"issuer", authority.as_ref()]:
```
authority: Pubkey
name: String
bjj_pub_key_x: [u8;32]
bjj_pub_key_y: [u8;32]
status: IssuerStatus  (Pending=0, Approved=1, Revoked=2, Slashed=3, Cooldown=4)
tier: IssuerTier  (Community=0, Enterprise=1, Regulated=2, Government=3)
staked_amount: u64
credentials_issued: u64
voting_ends_at: i64
vote_count: u64
votes_for: u64
cooldown_ends_at: i64
creation_slot: u64
```

`VoteRecord` PDA seeds=[b"vote", issuer_pda.as_ref(), voter.as_ref()]:
```
voter: Pubkey, issuer: Pubkey, vote: bool, released: bool
```

`StakerAccount` PDA seeds=[b"staker", voter.as_ref()]:
```
voter: Pubkey, amount_staked: u64, active_votes_count: u8, last_stake_slot: u64
```

`DaoTreasury` PDA seeds=[b"dao-treasury"], space=0 (lamport-only account)

`StakeVault` PDA seeds=[b"stake-vault"], space=0 (shared lamport vault, SEC-014)

**Instructions:**

`initialize_registry(authority, governance_token_mint, min_stake, voting_period, approval_threshold)`
  Creates RegistryConfig.

`register_issuer(name, bjj_pub_key_x, bjj_pub_key_y, tier)`
  Creates IssuerAccount with status=Pending. Transfers min_stake SOL to stake_vault.
  MISSING: BJJ subgroup check on pub key (SEC-007).

`stake_tokens(amount)`
  Transfers SPL tokens from voter to governance_vault. Creates/updates StakerAccount.
  Flash-loan guard: last_stake_slot must be >= 100 slots ago before voting.

`unstake_tokens(amount)`
  Gate: active_votes_count == 0.
  Uses raw -= (SEC-024, should be checked_sub).

`vote_on_issuer(vote: bool, amount: u64)`
  Gate: now < issuer.voting_ends_at, not yet voted, stake age >= 100 slots.
  Updates active_votes_count with checked arithmetic. Creates VoteRecord.

`release_vote()`
  Gate: vote_record.released == false, issuer.status != Pending.
  Decrements active_votes_count with checked_sub.

`finalize_voting()`
  Gate: now >= voting_ends_at, status == Pending.
  Tallies votes, sets status = Approved or Revoked based on threshold.
  Emits IssuerApproved or IssuerRevoked.  active_issuers incremented if Approved.

`approve_via_trust_anchor()`
  Gate: trust_anchor.status == Approved, tier >= Community.
  MISSING: minimum-tier gate on target (SEC-015).
  Sets status = Approved, emits IssuerApproved.

`slash_issuer(slash_amount)`
  Gate: registry_config.authority == signer.
  MISSING: PDA seed constraint on issuer_account (SEC-003 analog).
  Calls transfer_slashed_lamports(stake_vault -> dao_treasury, slash_amount).
  MISSING: rent floor check on stake_vault (SEC-030).
  Sets issuer.status = Slashed.  Decrements active_issuers.

`submit_fraud_proof(slash_amount, evidence_commitment)`
  Gate: authority == signer.
  MISSING: PDA seed constraint on issuer_account.
  Calls transfer_slashed_lamports. Same SEC-030 risk.

`issue_credential(schema_hash, commitment)`
  Gate: issuer.status == Approved, issuer.authority == signer.
  Derives tree_authority PDA = PDA([b"tree-authority", schema_hash], THIS_PROGRAM).
  CPI: SPL AC append(commitment) using tree_authority as PDA-signer.
  MISSING: schema_account lookup (SEC-003).
  MISSING: SchemaTreeBinding.tree_pubkey == merkle_tree.key() check (SEC-003).
  Emits CredentialIssued.

`revoke_issuer()`
  Gate: authority == signer.  Decrements active_issuers (SEC-023: Cooldown path misses this).

`request_withdrawal()` / `withdraw_after_cooldown()`
  Moves issuer through Cooldown state. Lamports returned to issuer authority.

`check_issuer_status(authority: Pubkey)` (view)
  Returns issuer.status. No signer required. Never called on-chain (SEC-025).

**Does NOT:**
- Verify ZK proofs
- Handle schema metadata

---

### 4.3 schema-registry

**Program ID:** 4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1

**What it is:** Registry of credential schemas and their SPL AC Merkle tree bindings.
Maintains the on-chain root state that the ZK verifier reads.

**State accounts:**

`SchemaAccount` PDA seeds=[b"schema", name.as_bytes(), &[version]]:
```
authority: Pubkey
name: String
version: u8
category: String
field_names: Vec<String>
schema_hash: [u8;32]
deprecated: bool
created_at: i64
usage_count: u64
```

`SchemaTreeBinding` PDA seeds=[b"schema-tree-binding", schema_hash]:
  Written as raw bytes (145 bytes). Parsed in solid-light::cpi_helpers:
```
[0..8)    discriminator b"schmtree"
[8..40)   schema_hash [u8;32]
[40..72)  tree_pubkey [u8;32]
[72..104) current_root [u8;32]
[104..112) last_slot u64 LE
[112]     status u8  (0=Active, 1=Frozen)
[113..145) authority [u8;32]  (forward-compatible extension)
```
Total: 145 bytes. Reader (cpi_helpers) only requires [0..113). Forward-compatible.

`GlobalStateBinding` PDA seeds=[b"globroot"]:
  Raw bytes (104 bytes):
```
[0..8)    discriminator b"globroot"
[8..40)   global_tree_pubkey [u8;32]
[40..72)  current_root [u8;32]
[72..104) last_slot u64 LE (but also: high bytes reserved)
```

**Instructions:**

`register_schema(name, version, category, field_names, schema_hash)`
  Gate: payer is signer.
  Computes expected hash = Poseidon(name_chunks, version, num_fields).
  SHOULD enforce: computed_hash == schema_hash.  [DISABLED -- SEC-002 fix pending]
  Creates SchemaAccount.

`deprecate_schema()`
  Gate: schema.authority == signer.  Sets schema.deprecated = true.

`increment_usage()`
  Gate: schema.authority == signer (P1-2 fix).  Bumps schema.usage_count.

`initialize_tree_binding(schema_hash, tree_pubkey)`
  Gate: schema.authority == signer (via seeds constraint).
  Writes SchemaTreeBinding PDA as raw bytes.

`initialize_global_binding(global_tree_pubkey)`
  Gate: payer is signer.
  Writes GlobalStateBinding PDA as raw bytes.

`update_tree_root(schema_hash, new_root, new_slot)`
  Gate: tree binding exists, binding.authority == signer, now_slot > last_slot (monotonicity).
  Updates [72..104) of SchemaTreeBinding.
  MISSING: emit!(TreeRootUpdated) event.

`update_global_root(new_root, new_slot)`
  Updates GlobalStateBinding.  Monotonicity enforced.

`set_binding_status(schema_hash, status)`
  Reuses UpdateTreeRoot context (seeds-constrained to schema_hash).
  MISSING: re-assertion of data[8..40] == schema_hash (SEC-019).
  MISSING: emit!(SchemaBindingFrozen) event.

`transfer_tree_binding_authority(schema_hash, new_authority)`
  MISSING: re-assertion of data[8..40] == schema_hash (SEC-019).
  MISSING: single-step transfer risk (SEC-016 analog).

**Does NOT:**
- Verify credentials or proofs
- Interact with issuer-registry directly
- Know about the ZK circuit parameters

---

## 5. Rust Crate Contracts

### 5.1 solid-core (crates/solid-core/)

**What it is:** Pure Rust cryptographic primitives library. No Solana, no WASM, no async.
Compiled both for BPF/SBF (on-chain in solid-light helpers) and for native (host tests,
WASM bridge).

**Modules and their contracts:**

`poseidon`
  Input:  &[[u8;32]] (field elements as 32-byte LE arrays), or &[u64] (field integers)
  Output: [u8;32] LE hash
  Uses: light-poseidon with BN254 parameters
  Guarantees: byte-for-byte identical to Circom's Poseidon with the same inputs
  Vector tested: yes (2/10 primitives -- commitments and nullifiers use poseidon internally)

`babyjubjub`
  Key gen:  BJJKeypair { private_key: [u8;32], public_key: BJJPublicKey { x, y } }
  Derive:   derive_key(master: &[u8;32], context: &[u8;32]) -> [u8;32]
            (= Poseidon(master, context) as scalar)
  Derive pubkey: derive_public_key(priv: &[u8;32]) -> BJJPublicKey
  Sign:     sign(priv: &[u8;32], message: &[u8;32]) -> EdDSASignature { r8_x, r8_y, s }
  Verify:   verify(pk: &BJJPublicKey, msg: &[u8;32], sig: &EdDSASignature) -> Result<bool>
  MISSING: subgroup check in pubkey_to_affine (SEC-007)
  Vector tested: NO -- add to gen_vectors.rs (SEC-010)

`commitment`
  compute_attestation_commitment(data_fields: &[u64], schema_hash: &[u8;32],
                                  holder_pk: &BJJPublicKey, salt: &[u8;32]) -> [u8;32]
  Algorithm:
    dataHash   = Poseidon(data_fields[0..NUM_FIELDS-1])
    commitment = Poseidon(dataHash, schema_hash_field, holderAx_field, holderAy_field, salt_field)
  Vector tested: yes (via cross-language vector test)

`nullifier`
  compute_nullifier(master_key, revoc_nonce, verifier_addr, query_context_hash,
                    verifier_nonce: all [u8;32]) -> [u8;32]
  Algorithm: Poseidon(masterKey, revocNonce, verifierAddr, queryContextHash, verifierNonce)
  NOTE: revoc_nonce is a u64 packed into [u8;32] LE for the Poseidon input
  Vector tested: yes (via cross-language vector test)

`identity`
  IdentityState::new(pk: BJJPublicKey, revocation_nonce: u64)
  IdentityState::commitment() -> [u8;32]
  Algorithm: Poseidon(pk.x, pk.y, revocation_nonce as field element)
  Vector tested: NO -- add to gen_vectors.rs

`credential`
  Credential struct (attestation data + metadata)
  verify_integrity() -> bool  (SDK-internal only, not an on-chain consensus check)

**Used by:** solid-light (on-chain helpers), wasm/ (JS bridge), tools/solid-prover (proof gen)
**NOT used by:** any Anchor program directly (use solid-light for on-chain)

---

### 5.2 solid-light (crates/solid-light/)

**What it is:** Thin on-chain helpers. BPF-safe. No heap allocation beyond what SPL allows.
Provides account-layout parsers and cross-program verification helpers for use INSIDE
Anchor programs.

**Modules and their contracts:**

`cpi_helpers`
  verify_state_root_matches(account_data: &[u8], expected_root: &[u8;32]) -> bool
    Checks:  data[0..8] == b"globroot"
             data[40..72] == expected_root
    Does NOT check: account ownership (caller must check .owner == SCHEMA_REGISTRY_ID)

  verify_schema_root_binding(account_data, schema_hash, expected_root) -> bool
    Checks:  data[0..8] == b"schmtree"
             data[8..40] == schema_hash
             data[72..104] == expected_root
             data[112] == STATUS_ACTIVE (0)
    Does NOT check: account ownership

  SCHEMA_REGISTRY_ID: [u8;32]  (CRITICAL: hard-coded byte array, no runtime validation)
  SCHEMA_TREE_DISCRIMINATOR: [u8;8] = b"schmtree"
  GLOBAL_STATE_DISCRIMINATOR: [u8;8] = b"globroot"

`credential_tree`
  CompressedCredential, CompressedNullifier, CompressedIdentity, CompressedIssuer
  Type definitions for SPL AC state trees. Used by indexers and CLIs.

**Used by:** programs/zk-verifier (the only program that needs CPI-safe helpers)
**NOT used by:** schema-registry or issuer-registry directly

---

### 5.3 wasm/ (WASM bridge crate)

**Crate:** wasm/ (package `solid-wasm`). Separate workspace member.
`crates/solid-core` stays strictly BPF-compatible and has zero
`#[wasm_bindgen]` exports (ADR-0002 / SOLID-SEC-009 / SOLID-SEC-028).
**Build:** `wasm-pack build wasm/ --target nodejs --out-dir ts-sdk/packages/core/wasm --release`
**Output:** `ts-sdk/packages/core/wasm/` -- consumed by `@solid-protocol/core`
via the relative runtime import `../wasm/solid_wasm.js`. There is no
`wasm/pkg/` dir and no separate `@solid-protocol/wasm` npm package.

**What it is:** wasm-bindgen bridge exposing solid-core primitives to JavaScript via WASM.

**All exported functions (current, as of wasm/src/lib.rs):**

| JS name | Rust fn | What it does |
|---------|---------|-------------|
| `poseidonHash(fields: BigUint64Array)` | poseidon_hash | Hash u64 field values. Returns 32-byte LE. |
| `poseidonHashBytes(inputs: Uint8Array)` | poseidon_hash_bytes | Hash 32-byte LE chunks (concatenated flat buffer, length must be multiple of 32). |
| `generateBJJKeypair()` | generate_bjj_keypair | Returns {privateKey, publicKeyX, publicKeyY} as Uint8Arrays. |
| `signMessage(priv, msg: Uint8Array)` | sign_message | EdDSA-Poseidon sign. Returns {r8x, r8y, s}. |
| `verifySignature(pkx, pky, msg, r8x, r8y, s)` | verify_signature | Returns boolean. |
| `computeCommitment(dataFields, schemaHash, holderPkX, holderPkY, salt)` | compute_commitment | Returns 32-byte LE commitment. |
| `computeHardenedNullifier(masterKey, revocNonce, verifierAddr, queryCtxHash, verifierNonce, issuerTreeRoot)` | compute_hardened_nullifier | 6-arg Poseidon (ADR-0006 Phase 2 revision; 6th input is ADR-0014 issuerTreeRoot). Returns 32-byte LE. |
| `generateIdentity(passphrase)` | generate_identity | Encrypted BJJ identity JSON string. |
| `unlockIdentity(json, passphrase)` | unlock_identity | Returns 32-byte master private key. |
| `deriveKey(masterKey, context)` | derive_key | Poseidon(master, context). Returns 32-byte scalar. |
| `deriveCredentialKey(masterKey, schemaHash)` | derive_credential_key | Returns {privateKey, publicKeyX, publicKeyY}. |
| `computeIdentityState(pkX, pkY, revocNonce)` | compute_identity_state | Poseidon(pkX, pkY, nonce). Returns 32-byte. |

**All inputs/outputs are `Uint8Array` (32 bytes LE) for field elements.**
**All u64 values are passed as `BigInt` in JS.**

**Does NOT:**
- Generate Groth16 proofs (that is snarkjs + the circuit wasm)
- Make any network calls
- Access the Solana runtime

**Endianness contract:** ALL field elements (BJJ scalars, Poseidon hashes, commitments,
nullifiers) are 32-byte LITTLE-ENDIAN, matching the BN254 field element convention used by
both light-poseidon and circomlib.

Solana pubkeys (base58-decoded) are 32-byte BIG-ENDIAN. When passing a pubkey as a field
element to the circuit or to computeHardenedNullifier, the caller must re-encode it as LE
or treat the full 32 bytes as a raw field element. The circuit uses verifierAddress as a raw
field element -- it must match what ID.to_bytes() returns in the on-chain check (SEC-006).

---

## 6. TypeScript SDK Contracts

### 6.1 @solid-protocol/core

**Package:** ts-sdk/packages/core/
**What it is:** TypeScript wrapper around the WASM bridge. Re-exports all WASM functions
with TypeScript type safety. Also provides type definitions for the protocol data structures
and the QueryBuilder utility.

**Key exports:**
```typescript
// WASM re-exports (all call through to wasm/src/lib.rs)
export { poseidonHash, computeCommitment, computeHardenedNullifier,
         deriveCredentialKey, computeIdentityState, signMessage, verifySignature,
         generateBJJKeypair, generateIdentity, unlockIdentity } from './wasm';

// Types
export type BJJKeypair = { privateKey: Uint8Array; publicKeyX: Uint8Array; publicKeyY: Uint8Array };
export type Credential = { schemaHash: Uint8Array; data: bigint[]; salt: Uint8Array; ... };
export type Query = { predicates: Predicate[]; logic: 'AND' | 'OR' };

// QueryBuilder (MUST use WASM Poseidon for context hash -- no JS reimplementation)
export class QueryBuilder {
  add(credIndex: number, fieldIndex: number, op: Operator, value: bigint): this
  build(): CompiledQuery
  toCircuitInputs(credentials: Credential[]): CircuitInputs  // calls wasm.poseidonHash for ctx hash
}
```

**Contract:** All crypto in this package MUST go through the WASM bridge. If a function
computes a Poseidon hash or BJJ operation in pure JavaScript, that is a bug and a
vector-drift risk (SEC-010 in aggregate). The only JS-level operations are: bigint arithmetic
for encoding, Buffer manipulation for endianness conversion, and array layout for circuit inputs.

---

### 6.2 @solid-protocol/holder

**Package:** ts-sdk/packages/holder/
**What it is:** Proof generation for credential holders. Takes credentials and a query,
assembles circuit inputs, runs snarkjs Groth16 prover, returns a proof.

**Key exports:**
```typescript
export async function generateBatchProof(params: {
  masterPrivKey: Uint8Array;    // 32-byte BJJ master private key
  revocationNonce: bigint;      // from user's identity state
  credentials: Credential[];    // 1..4 credentials (padded to 4 internally)
  query: CompiledQuery;         // from QueryBuilder.build()
  verifierAddress: Uint8Array;  // 32 bytes, zk-verifier program pubkey as LE field element
  verifierNonce: Uint8Array;    // 32 bytes, caller-chosen anti-relay nonce
  currentTimestamp: bigint;     // Unix timestamp, must be within clock skew of now
  zkeyPath: string;             // path to batch_credential_query_final.zkey
  wasmPath: string;             // path to batch_credential_query.wasm
}) => Promise<BatchProofResult>

export type BatchProofResult = {
  proof_a: Uint8Array;          // 64 bytes
  proof_b: Uint8Array;          // 128 bytes
  proof_c: Uint8Array;          // 64 bytes
  publicSignals: string[];      // 31 decimal strings
  nullifier: Uint8Array;        // 32 bytes (= publicSignals[0] as LE bytes)
}
```

**Internal contract:**
1. Pad credentials to NUM_CREDS=4 by appending zero-schema dummy entries.
2. Sort active credentials by schemaHash ascending (canonical ordering).
3. For each active credential, derive per-schema BJJ key via `deriveCredentialKey()`.
4. Build identity leaves and fetch Merkle paths from the passed-in tree data.
5. Build circuit input object matching `batch_credential_query.circom` signal names exactly.
6. Call `snarkjs.groth16.fullProve(input, wasmPath, zkeyPath)`.
7. Extract nullifier from `publicSignals[0]` as bigint, convert to 32-byte LE via
   `bigintToBytes32(BigInt(publicSignals[0]))`.
8. Return structured result.

**DOES NOT:**
- Submit the proof on-chain (that is @solid-protocol/verifier)
- Store credentials (the application does this)
- Generate the zkey or wasm artifacts (those come from the circuit build)

**Identity Cohesion check (current, being verified):**
Per-credential: `cred.holderPubKeyX` SHOULD equal the derived pubkey for that schema.
Current check at index.ts:250-255 verifies against per-schema key (fix from BUG-NEW-01 was
applied in the April 22 remediation). Verify this is still the case after the latest pull.

---

### 6.3 @solid-protocol/verifier

**Package:** ts-sdk/packages/verifier/
**What it is:** On-chain submission and result reading. Builds the `verify_batch_proof`
transaction, signs, submits, and waits for confirmation.

**Key exports:**
```typescript
export async function buildVerifyBatchProofIx(params: {
  proof: BatchProofResult;
  connection: Connection;
  payer: PublicKey;
  globalTree: PublicKey;     // GlobalStateBinding PDA
  schemaTrees: PublicKey[];  // SchemaTreeBinding PDAs (4 entries, Pubkey.default() for inactive)
}): Promise<TransactionInstruction>

export async function verifyOnChain(params: {
  connection: Connection;
  wallet: Keypair | Wallet;
  proof: BatchProofResult;
  globalTree: PublicKey;
  schemaTrees: PublicKey[];
}): Promise<TransactionSignature>

export async function checkIssuerStatus(
  connection: Connection,
  issuerAuthority: PublicKey
): Promise<IssuerStatus>

export function deriveNullifierPDA(nullifier: Uint8Array): PublicKey
export function deriveSchemaTreeBindingPDA(schemaHash: Uint8Array): PublicKey
export function deriveGlobalBindingPDA(): PublicKey
export function deriveIssuerPDA(authority: PublicKey): PublicKey
```

**Account ordering contract (MUST match VerifyBatchProof struct in zk-verifier):**
```
0: verifier_config (mut)
1: vk_storage
2: nullifier_account (init, seeds=[b"null", nullifier])
3: global_tree
4: schema_tree_info[0]
5: schema_tree_info[1]
6: schema_tree_info[2]
7: schema_tree_info[3]
8: payer (mut, signer)
9: system_program
```

**DOES NOT:**
- Generate proofs
- Manage issuer registration

---

### 6.4 @solid-protocol/sdk (top-level facade)

**Package:** ts-sdk/packages/sdk/
**What it is:** Unified entry point combining holder + verifier. Used when a single
process both generates and submits proofs (e.g., a relayer or test script).

**Key exports:**
```typescript
export class SolID {
  constructor(config: SolIDConfig)

  async prove(query: Query, credentials: Credential[]): Promise<BatchProofResult>
    // delegates to @solid-protocol/holder::generateBatchProof

  async verifyOnChain(proof: BatchProofResult): Promise<TransactionSignature>
    // delegates to @solid-protocol/verifier::verifyOnChain

  async resolveSchema(schemaHash: Uint8Array): Promise<SchemaAccount | null>
    // reads schema-registry via getProgramAccounts + Borsh deserializer

  async listIssuers(statusFilter?: IssuerStatus): Promise<IssuerAccount[]>
    // reads issuer-registry
}
```

---

## 7. Script Contracts

### 7.1 scripts/initialize.ts

**Purpose:** One-time setup of all three programs on a fresh localnet or devnet.
**Inputs:** Keypair file, cluster URL (from environment)
**Actions:**
1. `zk-verifier::initialize_verifier(authority)`
2. `schema-registry::initialize_global_binding(global_tree_pubkey)`
3. `issuer-registry::initialize_registry(authority, governance_token_mint, ...)`
**Outputs:** Logs PDAs. Writes to stdout only.
**Does NOT:** Register issuers, schemas, or upload a VK.

### 7.2 scripts/bootstrap_issuer.ts [DOES NOT EXIST YET -- must be created]

**Purpose:** Bootstrap a single issuer through the full approval lifecycle.
**Inputs:** Issuer keypair, cluster URL, DAO token mint
**Actions:**
1. `register_issuer(name, bjj_pub_key, tier)`
2. `stake_tokens(amount)` (voter = issuer, using DAO token)
3. Either `vote_on_issuer(vote=true, ...)` from multiple voters, then `finalize_voting()`
   OR `approve_via_trust_anchor()` from an existing trust anchor
**Outputs:** Saves issuer authority keypair + BJJ keypair to /tmp/solid-e2e-state-issuer.json
**Does NOT:** Issue credentials (that is issue.ts).

### 7.3 scripts/issue.ts

**Purpose:** Issue a credential from an approved issuer.
**Inputs:** Issuer keypair + BJJ key (from bootstrap state), holder BJJ public key,
           schema hash, credential data, cluster URL
**Actions:**
1. Reads the credential tree for the schema
2. Calls `issue_credential(schema_hash, commitment)` via the SDK
3. Emits CredentialIssued event
**Outputs:** Saves credential to /tmp/solid-e2e-state-credential.json (NOT scripts/ dir)
**Bug (SEC-011c):** Missing the issuer approval sequence. Must call bootstrap_issuer first.

### 7.4 scripts/prove.ts

**Purpose:** Generate a batch proof and submit for on-chain verification.
**Inputs:** Holder BJJ master key, credential(s), query definition, cluster URL,
           zkey path, wasm path
**Actions:**
1. Fetches Merkle paths for each credential
2. Calls `generateBatchProof()` via holder SDK
3. Calls `verifyOnChain()` via verifier SDK
**Outputs:** Prints verification transaction signature
**Bug (SEC-011a):** reads `batch_credential_query_final.zkey` but setup.js writes `circuit_final.zkey`
**Bug (SEC-011b):** references `keccak256HashPair` which is not imported

---

## 8. Test Contracts

### 8.1 Integration tests (tests/integration/)

Each test starts a fresh anchor-bankrun context. No external validator process.

| File | What It Proves | Depends On |
|------|---------------|-----------|
| 01_registry_init.test.ts | RegistryConfig space=112 correct | anchor build |
| 02_issuer_lifecycle.test.ts | register->stake->vote->finalize->release->unstake | -- |
| 03_slash_transfers_lamports.test.ts | slash moves lamports; vault floor respected | SEC-030 fix |
| 04_schema_and_bindings.test.ts | register_schema hash check; wrong hash rejected | SEC-002 fix |
| 05_issue_credential.test.ts | issue with valid schema+tree binding; without fails | SEC-003 fix |
| 06_verify_happy_path.test.ts | real Groth16 proof verifies | circuit + WASM fix |
| 07_verify_replay_rejected.test.ts | same proof resubmitted fails | 06 |
| 08_verify_forged_global_tree_rejected.test.ts | forged system-owned account rejected | -- |
| 09_verify_forged_schema_tree_rejected.test.ts | same for schema tree | -- |
| 10_verify_expired_credential_rejected.test.ts | expired timestamp rejected | SEC-005 fix |
| 11_cross_language_vectors.test.ts | all 8 Rust primitives match TS WASM | WASM + vectors |

**Harness:** solana-bankrun. Each test must explicitly set up all required accounts. No shared
state between test files. Every test must pass in isolation.

### 8.2 Unit tests (embedded in crates + programs)

| Location | What Is Tested | Count |
|----------|---------------|-------|
| solid-core (embedded) | Poseidon, BJJ sign/verify, commitment, nullifier | ~30 |
| solid-light/cpi_helpers | Account layout parsing, discriminator checks | 8 |
| zk-verifier (lib) | VkBuf parser, G1 negation | 9 |
| New (needed) | BJJ subgroup rejection (SEC-007) | -- |
| New (needed) | checked arithmetic in staking (SEC-024) | -- |
| New (needed) | stake_vault floor enforcement (SEC-030) | -- |

### 8.3 Cross-language vector tests (tests/vectors/)

**Current coverage (2/10):** attestation_commitment, nullifier_hash
(the nullifier vector was regenerated with the ADR-0014 6-input
shape in Phase 2 commit `df33ffe`; other primitives still missing
-- tracked as SOLID-SEC-010, scheduled for Phase 3).
**Required coverage (10/10):**
  1. poseidon_raw (3-input)
  2. bjj_sign + bjj_verify (EdDSA round-trip)
  3. derive_credential_key
  4. compute_identity_state
  5. query_context_hash (matching circuit Step 4: two 8-input Poseidons + 4-input final)
  6. compute_hardened_nullifier (6-input; ADR-0006 Phase 2 revision)
  7. compute_issuer_leaf (ADR-0014 Poseidon(5))
  7. compute_commitment (2-level)
  8. verify_signature (negative: wrong message)

**How it works:**
  1. Run `cargo run --example gen_vectors -- --output tests/vectors/` on the Rust side.
     This writes JSON files with test vectors.
  2. Run `ts-node tests/vectors/check_vectors.ts` on the TS side.
     This reads the JSON files and asserts every WASM primitive matches byte-for-byte.
  3. Both steps run in CI as the `cross_language_vectors` job.

---

## 9. Integration Invariants

These must be true at EVERY delivery milestone. If any is violated, the system is broken
even if individual components pass their own tests.

**Invariant I-1: Public input count consistency**
  circuit NR_PUBLIC_INPUTS == 32  (ADR-0014 revision; was 31)
  == Rust constant `zk-verifier::NR_PUBLIC_INPUTS`
  == length of `publicSignals` array returned by snarkjs
  == length of `public_inputs` array in `buildVerifyBatchProofIx`
  == `@solid-protocol/verifier::NR_PUBLIC_INPUTS`
  Violation: silent proof rejection or buffer corruption.

**Invariant I-2: Endianness contract**
  All field elements cross-language are 32-byte LITTLE-ENDIAN.
  Solana pubkeys (base58) are 32-byte BIG-ENDIAN and must NOT be passed raw as field elements.
  The `verifierAddress` circuit input at slot [VERIFIER_ADDRESS_INPUT_INDEX = 29]
  (shifted from 28 by ADR-0014) = ID.to_bytes() on-chain = the zk-verifier
  program pubkey.  The SDK must encode this as the same 32-byte sequence
  that ID.to_bytes() returns.
  Violation: SOLID-SEC-031 (verifierAddress mismatch) causes every proof to fail.

**Invariant I-3: Account layout immutability**
  `SchemaTreeBinding` bytes layout ([0..145]) and `GlobalStateBinding` bytes layout ([0..104])
  are PROTOCOL-LEVEL CONSTANTS. Any change requires coordinating: schema-registry (writer),
  solid-light/cpi_helpers.rs (parser), and the verifier (consumer of parsed values).
  Violation: owner checks pass but root values compared against garbage.

**Invariant I-4: Schema hash uniqueness**
  `schema_hash` must be derivable from the schema metadata via the agreed Poseidon formula.
  Currently disabled (SEC-002). When enabled, this becomes a hard constraint:
  a credential issued against schema_hash X implies the holder knows a credential whose
  metadata hashes to X. Without this, the schema universe is unconstrained.

**Invariant I-5: Program ID stability**
  `Anchor.toml`, `declare_id!()`, `deployments/devnet.json`, `cpi_helpers::SCHEMA_REGISTRY_ID`,
  and `CLAUDE.md` must all agree. Enforced by `scripts/check_program_ids.py`.
  `SCHEMA_REGISTRY_ID_BYTES` (the hardcoded byte array) must be kept in sync manually when
  the registry is redeployed.

**Invariant I-6: Canonical ordering**
  Active schemaHashes in any proof must be strictly ascending.
  Enforced in-circuit: `ordering[i]` LessThan(252) check.
  Enforced on-chain: `require!(schema_hash > prev)` in verify_batch_proof.
  SDK: sorts credentials by schemaHash before building circuit inputs.
  Violation: on-chain rejects valid proofs.

**Invariant I-7: Nullifier formula agreement**
  Circuit: Poseidon(masterKey, revocNonce, verifierAddr, queryContextHash, verifierNonce)
  WASM:    wasm/src/lib.rs::computeHardenedNullifier -- SAME formula. Confirmed.
  On-chain: reads public_inputs[0] as the nullifier, does NOT recompute.
  SDK:     extracts publicSignals[0] and converts: bigintToBytes32(BigInt(publicSignals[0]))
  Dead code: NullifierComputer in nullifier_expiry.circom uses OLD 3-arg formula. Delete it.

**Invariant I-8: Per-schema key derivation**
  Circuit: credentialPrivKey = Poseidon(masterKey, schemaHash), (Ax, Ay) = BabyPbk(credPrivKey)
  WASM: deriveCredentialKey(masterKey, schemaHash) -- same formula. Confirmed.
  Holder SDK: MUST use deriveCredentialKey() when building circuit inputs and when checking
  holderPubKeyX (not the master public key).
  Violation: commitment fails holderAx check in-circuit.

**Invariant I-9: Tree binding monotonicity**
  `update_tree_root` enforces `now_slot > last_slot` (monotonic).
  If an indexer tries to call update_tree_root in the same slot twice, the second call fails.
  Indexers must handle this by retrying in the next slot.
  This is intentional -- prevents root regression attacks.

**Invariant I-10: Nullifier PDA seeds**
  NullifierAccount PDA seeds = [b"null", nullifier_bytes[32]].
  The SDK must derive the same PDA when building the transaction.
  `verifier_sdk::deriveNullifierPDA(nullifier)` must use the same seed.

---

## Appendix: Dependency Graph

```
snarkjs + circuit.wasm + .zkey
    |
    v
@solid-protocol/holder
    |-- @solid-protocol/core (wasm re-exports, types)
    |       |-- ts-sdk/packages/core/wasm/ (wasm-pack output dir)
    |               |-- solid-wasm (wasm/) --> solid-core (Rust library)
    |
    v
@solid-protocol/verifier
    |-- @coral-xyz/anchor (Solana program client)
    |-- @solana/web3.js

@solid-protocol/sdk
    |-- @solid-protocol/holder
    |-- @solid-protocol/verifier

scripts/*.ts
    |-- @solid-protocol/sdk

tests/integration/*.test.ts
    |-- solana-bankrun
    |-- @solid-protocol/sdk
    |-- @coral-xyz/anchor

On-chain:
    zk-verifier
        |-- solid-light (cpi_helpers)
        |       |-- solid-core (Poseidon, types)
        |-- alt_bn128 syscalls (Solana runtime)
    issuer-registry
        |-- SPL Account Compression
        |-- SPL Token
    schema-registry
        (standalone, writes raw bytes)
```
