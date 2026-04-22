# SolID Protocol -- Security Registry

Canonical, living tracker for every security finding across every audit.
One file. No fragmentation. Nothing deleted.

- Protocol version under review: v0.3 (April 2026, post-remediation)
- Last audit: 2026-04-22 (`sec/audits/2026-04-22_v0.3_comprehensive_audit.md`)
- Last registry update: 2026-04-22
- Next audit target: after Phase 1 close-out (see audit snapshot, Section 7)

See `sec/README.md` for workflow, severity definitions, and status lifecycle.

---

## Summary

| Severity  | Open | In Progress | Fixed | Verified | Won't Fix | Total |
|-----------|------|-------------|-------|----------|-----------|-------|
| CRITICAL  | 3    | 0           | 0     | 0        | 0         | 3     |
| HIGH      | 9    | 0           | 0     | 0        | 0         | 9     |
| MEDIUM    | 9    | 0           | 0     | 0        | 0         | 9     |
| LOW       | 3    | 0           | 0     | 0        | 0         | 3     |
| INFO      | 3    | 0           | 0     | 0        | 0         | 3     |
| **Total** | 27   | 0           | 0     | 0        | 0         | 27    |

---

## Status board

| ID              | Severity | Status | Title                                                              |
|-----------------|----------|--------|--------------------------------------------------------------------|
| SOLID-SEC-001   | CRITICAL | Open   | Batch circuit: `queryCredentialIndices` / `queryFieldIndices` unconstrained |
| SOLID-SEC-002   | CRITICAL | Open   | `register_schema` Poseidon integrity check commented out           |
| SOLID-SEC-003   | CRITICAL | Open   | `issue_credential` missing schema + tree pubkey binding            |
| SOLID-SEC-004   | HIGH     | Open   | No in-circuit issuer pubkey binding; revoked issuers still verify  |
| SOLID-SEC-005   | HIGH     | Open   | `currentTimestamp` public input not bound to `Clock`               |
| SOLID-SEC-006   | HIGH     | Open   | VK overwrite at chunk 0 has no freeze-gate; truncated VK finalizable|
| SOLID-SEC-007   | HIGH     | Open   | BJJ public keys not subgroup-checked at registration               |
| SOLID-SEC-008   | HIGH     | Open   | Nullifier does not include epoch / global root                     |
| SOLID-SEC-009   | HIGH     | Open   | WASM bridge fractured across 3 locations (Cargo.toml / wasm/ / ts-sdk)|
| SOLID-SEC-010   | HIGH     | Open   | Cross-language test vectors cover only 2 of 10 primitives          |
| SOLID-SEC-011   | HIGH     | Open   | E2E scripts bugged: zkey name, missing import, missing issuer flow |
| SOLID-SEC-012   | HIGH     | Open   | Trusted setup is single-party with timestamp entropy               |
| SOLID-SEC-013   | MEDIUM   | Open   | `slash_issuer` and `submit_fraud_proof` are single-key             |
| SOLID-SEC-014   | MEDIUM   | Open   | `stake_vault` is a single shared PDA across all issuers            |
| SOLID-SEC-015   | MEDIUM   | Open   | `approve_via_trust_anchor` has no minimum-tier gate on target      |
| SOLID-SEC-016   | MEDIUM   | Open   | `transfer_authority` is single-step (no propose/accept)            |
| SOLID-SEC-017   | MEDIUM   | Open   | `solid-prover` uses `ark_std::test_rng()` -> breaks unlinkability  |
| SOLID-SEC-018   | MEDIUM   | Open   | `verifier_config` write lock on every verify caps throughput       |
| SOLID-SEC-019   | MEDIUM   | Open   | `set_binding_status` / `transfer_tree_binding_authority` missing schema-hash re-assertion |
| SOLID-SEC-020   | MEDIUM   | Open   | E2E scripts persist plaintext issuer + holder secrets to `scripts/e2e_state.json` |
| SOLID-SEC-021   | MEDIUM   | Open   | Depth-20 circuit caps global tree at ~250K holders; hard scale cliff |
| SOLID-SEC-022   | LOW      | Open   | Local `IsZero` reimplementation in `credential_atom.circom`        |
| SOLID-SEC-023   | LOW      | Open   | `active_issuers` counter drifts on Cooldown -> Revoked path        |
| SOLID-SEC-024   | LOW      | Open   | `unstake_tokens` uses raw `-=` instead of `checked_sub`            |
| SOLID-SEC-025   | INFO     | Open   | `CheckIssuerStatus` ungated and never called on-chain              |
| SOLID-SEC-026   | INFO     | Open   | `Credential::verify_integrity` never called on-chain               |
| SOLID-SEC-027   | INFO     | Open   | `docs/IMPROVEMENTS_ROADMAP.md` has stale unticked checkboxes       |

---

## Findings -- detail

Each entry records: title, severity, status, introduced-in audit, last
updated, evidence (file:line), description, impact, remediation, and the
regression gate that must ship with the fix.

---

### SOLID-SEC-001 -- Batch circuit query indices unconstrained

- **Severity:** CRITICAL
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:**
  - `circuits/batch_credential_query.circom:56-59,216-217`
  - `circuits/lib/predicate_evaluator.circom:85-112`
- **Description.** The batch circuit declares `queryCredentialIndices[MAX_PREDICATES]`
  and `queryFieldIndices[MAX_PREDICATES]` as public inputs with no range
  constraints. `BatchFieldSelector` internally computes
  `credIs[i].out * selectors[i].value` and returns 0 for any out-of-range
  index. A prover sets `credIndex = 1000`, `queryValue = 0`, `operator = EQ`
  and satisfies `result = 1` for any predicate without holding any credential.
- **Impact.** Full soundness break on batch proofs. Trivially exploitable.
  `compound_query.circom` is affected at `FieldSelector`
  (`predicate_evaluator.circom:64-81`) identically.
- **Remediation.** Add `LessThan(8)` range checks on every index before
  entering the selector. Force a new trusted setup (which the circuit
  change requires anyway -- combine with SOLID-SEC-004 and SOLID-SEC-008 into
  one setup event).
- **Regression gate.** Property-based witness-generation test in
  `tests/circuits/` that feeds 1000 random out-of-range index vectors and
  asserts witness generation fails. On-chain integration test that submits
  such a proof and asserts `verify_batch_proof` rejects.
- **Blocks:** any further production deployment.

---

### SOLID-SEC-002 -- `register_schema` integrity check disabled

- **Severity:** CRITICAL
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:** `programs/schema-registry/src/lib.rs:114-130`
- **Description.** The Poseidon self-consistency check
  `require!(computed_hash == schema_hash)` is commented out with a
  "Simplified for this task" note. Anyone can register a schema whose
  declared metadata does not correspond to the claimed `schema_hash`.
- **Impact.** Root enabler for SOLID-SEC-003. Breaks the integrity promise
  labelled `SEC-06` in the remediation audit -- SEC-06 is not actually closed.
- **Remediation.** Re-enable the check using `light-poseidon` on-chain.
  Budget approx 170K CU for a 16-element input; acceptable for a one-time
  registration.
- **Regression gate.** Unit test: `register_schema` with mismatched
  `schema_hash` fails with `ErrorCode::InvalidSchemaHash`. Add
  `tests/integration/04_schema_and_bindings.test.ts` covering the
  positive and negative cases.
- **Blocks:** mainnet.

---

### SOLID-SEC-003 -- `issue_credential` missing schema + tree pubkey binding

- **Severity:** CRITICAL
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:**
  - `programs/issuer-registry/src/lib.rs:619-716`
  - Account context: `programs/issuer-registry/src/lib.rs:909-937`
- **Description.** The handler derives `PDA(b"tree-authority", schema_hash)`
  but does not: (a) require `schema_hash` correspond to a registered schema,
  (b) require `merkle_tree.key() == SchemaTreeBinding.tree_pubkey`, (c)
  prevent an approved issuer from appending to any SPL-AC tree whose
  authority is `PDA(b"tree-authority", any-32-bytes)`.
- **Impact.** Combined with SOLID-SEC-002, an approved issuer can spawn a
  rogue schema/tree universe that the on-chain verifier accepts as canonical.
  Combined with SOLID-SEC-004, the issuer can then issue self-signed
  credentials that verify.
- **Remediation.** Add `schema_account` and `schema_tree_binding` as
  required accounts. Seed-constrain `schema_account` to
  `[b"schema", name, &[version]]`, require
  `schema_account.schema_hash == schema_hash` and
  `schema_tree_binding.tree_pubkey == merkle_tree.key()`.
- **Regression gate.** Integration tests
  `06_issue_credential_rejects_unregistered_schema` and
  `07_issue_credential_rejects_wrong_tree`.
- **Blocks:** mainnet.

---

### SOLID-SEC-004 -- No in-circuit issuer pubkey binding

- **Severity:** HIGH
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:**
  - Circuit: `circuits/batch_credential_query.circom:81-82` (issuer pubkey is a private input)
  - On-chain: `programs/zk-verifier/src/lib.rs:148-297` (no issuer cross-check)
  - `programs/issuer-registry/src/lib.rs:577-582` (`check_issuer_status` exists but unused)
  - `CLAUDE.md:107-115` (open-work)
- **Description.** The circuit accepts `issuerPubKeyAx/Ay` as private
  inputs. The verifier program has no mechanism (CPI or Merkle membership)
  to assert the key corresponds to a DAO-approved, non-revoked issuer.
- **Impact.** Revoked / slashed issuers' prior signatures remain valid
  in-circuit. A compromised SDK or adversary can produce valid-looking
  proofs against unapproved issuer keys.
- **Remediation (preferred).** Introduce a compressed issuer Merkle tree
  maintained by `issuer-registry`. Add a root-binding PDA and a new
  public input to the circuit requiring in-circuit membership of the
  issuer pubkey. Rotating the root on `revoke_issuer` / `slash_issuer`
  sweeps revoked keys out without a circuit change afterwards.
- **Alternative.** CPI from `verify_batch_proof` into `check_issuer_status`;
  adds CU cost, complicates account ordering.
- **Regression gate.** Circuit witness test requiring a valid issuer
  membership proof. Integration test
  `05_verify_rejects_unapproved_issuer`.
- **Blocks:** external audit close-out. Bundle with SOLID-SEC-001 and
  SOLID-SEC-008 into a single circuit rev + trusted setup.

---

### SOLID-SEC-005 -- `currentTimestamp` public input not bound to `Clock`

- **Severity:** HIGH
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:**
  - `programs/zk-verifier/src/lib.rs:157-198`
  - `circuits/batch_credential_query.circom:65,192-198`
- **Description.** `public_inputs[30]` carries `currentTimestamp`. The
  circuit enforces `currentTimestamp <= expirationTimestamp` per
  credential. The on-chain verifier never compares `public_inputs[30]`
  to `Clock::get()?.unix_timestamp`.
- **Impact.** Attacker supplies `currentTimestamp = 0`, the circuit's
  expiry check becomes `0 <= expiration` for any non-zero expiration.
  Expired credentials still verify.
- **Remediation.** In `verify_batch_proof`:
  ```rust
  let now_ts = Clock::get()?.unix_timestamp as u64;
  let claimed_ts = u64_le_from_32bytes_or_err(&public_inputs[30])?;
  let skew = 300; // 5 minutes
  require!(
      claimed_ts >= now_ts.saturating_sub(skew) &&
      claimed_ts <= now_ts.saturating_add(skew),
      ErrorCode::StaleTimestamp
  );
  ```
- **Regression gate.** Integration test
  `10_verify_expired_credential_rejected` plus
  `11_verify_future_timestamp_rejected` for the upper skew bound.
- **Blocks:** external audit close-out.

---

### SOLID-SEC-006 -- VK chunk 0 overwrite, no freeze, truncated VK finalizable

- **Severity:** HIGH
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:** `programs/zk-verifier/src/lib.rs:82-130`
- **Description.** Chunk-0 path unconditionally writes
  `vk_storage.data = chunk_data`. There is no `vk_frozen` flag, no
  requirement that the program be paused for VK writes, and nothing
  prevents the authority from calling with `is_final_chunk = true` on a
  half-uploaded VK. The parser at `VkBuf::parse` accepts any
  `nr_ic >= 1`.
- **Impact.** A compromised authority can live-replace the VK (universal
  forgery). A bungled upload can finalize a truncated VK with fewer IC
  commitments than the circuit requires, producing bespoke silent
  acceptance bugs.
- **Remediation.**
  1. Add `config.vk_frozen: bool` and `config.vk_generation: u16`.
     Refuse writes when frozen.
  2. Store VKs as `(vk_id, VkStorage)` PDAs; keep at least one deprecated
     generation valid for a grace window.
  3. Include `vk_generation` as a public input; proofs bind to the VK
     they were produced against.
  4. Require `paused == true` for any VK write after generation 0.
- **Regression gate.** Unit tests: cannot finalize with partial chunks,
  cannot overwrite when frozen, cannot verify proof against wrong
  generation.
- **Blocks:** external audit close-out.

---

### SOLID-SEC-007 -- BJJ public keys not subgroup-checked at registration

- **Severity:** HIGH
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:** `crates/solid-core/src/babyjubjub.rs:111-114,125-133`;
  `programs/issuer-registry/src/lib.rs` register_issuer handler
- **Description.** `pubkey_to_affine` checks `is_on_curve()` and
  `!is_zero()` but does not check the cofactor. Order-{1,2,4,8} torsion
  points are accepted. Similarly `register_issuer` stores
  `bjj_pub_key_x/y` with no curve/subgroup check.
- **Impact.** A malicious issuer registers a small-order public key.
  Because `S * B8 = R + h * A` becomes `S * B8 = R` when `A` is
  small-order, the issuer forges signatures trivially on any message.
  Circomlib's in-circuit verifier does cofactor multiplication on R8/A,
  so the circuit-level attack may be blocked -- but off-chain signing
  with small-order keys is undefended.
- **Remediation.** In `pubkey_to_affine`: reject points whose
  `mul_by_cofactor()` is the identity. In `register_issuer`: verify
  on-curve + subgroup check before accepting registration. Both use
  arkworks primitives already in the dependency tree.
- **Regression gate.** Rust unit tests with explicit small-order points.
- **Blocks:** external audit close-out.

---

### SOLID-SEC-008 -- Nullifier lacks epoch / global-root binding

- **Severity:** HIGH
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:**
  - `circuits/batch_credential_query.circom:286-292`
  - `crates/solid-core/src/nullifier.rs:23-40`
- **Description.** Nullifier =
  `Poseidon(masterIdentityKey, revocationNonce, verifierAddress, queryContextHash, verifierNonce)`.
  It does not include the global root or an epoch counter. Change `verifier` or
  `queryContextHash` or `verifierNonce` and the nullifier changes -- there
  is no one-proof-per-credential rate limit beyond those scopes.
  Additionally, if the authority ever regresses a root (via bug, reorg, or
  future misuse), the same nullifier would be unique but the proof would
  semantically apply to two tree generations.
- **Impact.** Defense-in-depth gap. Not immediately exploitable, but a
  single future bug in root-update logic or a deep reorg turns this into
  a replay path.
- **Remediation.** Include `globalRoot` (or a monotonic epoch counter
  stored in `GlobalStateBinding`) in the nullifier preimage. Circuit
  change -- bundle with SOLID-SEC-001 and SOLID-SEC-004 in a single
  new trusted setup.
- **Regression gate.** Witness test: identical inputs across two
  distinct global-root epochs produce distinct nullifiers.
- **Blocks:** external audit close-out.

---

### SOLID-SEC-009 -- WASM bridge fractured across 3 locations

- **Severity:** HIGH
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:**
  - `crates/solid-core/Cargo.toml:10` declares a `wasm` feature
  - `crates/solid-core/src/` contains zero `#[wasm_bindgen]` exports
  - `wasm/src/lib.rs` holds the real bridge
  - `.github/workflows/ci.yml:196-197,256-257,301-303` runs
    `wasm-pack build crates/solid-core`, producing an empty package
  - `ts-sdk/packages/core/package.json` pins `file:../../../wasm/pkg`
    (a dir that neither CI nor any script creates)
- **Description.** Three disagreeing sources of truth for where the
  WASM bridge lives. CI builds from one, SDK imports from another,
  Cargo declares a feature in a third. The `cross_language_vectors`
  job only appears to pass because `check_vectors.ts` is narrow
  (see SOLID-SEC-010).
- **Impact.** The JS SDK's cryptography is not actually anchored to
  the Rust primitives it claims to call. Drift will be silent.
- **Remediation.** Pick one source of truth:
  - (A) Move all `#[wasm_bindgen]` exports into
    `crates/solid-core/src/wasm.rs` behind the `wasm` feature. Delete
    `wasm/` crate. Update CI and SDK path accordingly.
  - (B) Keep `wasm/` standalone. Remove the `wasm` feature from
    `solid-core`. Fix CI to build `wasm/` and SDK to depend on
    `../../../wasm/pkg`.
- **Regression gate.** New CI job `wasm_bridge_smoke`: build pkg,
  load from the SDK path the SDK actually uses, round-trip every
  primitive against `gen_vectors.rs` outputs.
- **Blocks:** mainnet (silent drift is unacceptable for identity
  primitives).

---

### SOLID-SEC-010 -- Cross-language test vectors cover only 2 of 10 primitives

- **Severity:** HIGH
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:**
  - `crates/solid-core/examples/gen_vectors.rs:55,62`
  - `tests/vectors/check_vectors.ts`
  - `tests/vectors/commitment_and_nullifier.json`
- **Description.** Only `attestation_commitment` and `nullifier` are
  covered. Not covered: raw Poseidon, BJJ sign/verify,
  `derive_credential_key`, `computeIdentityState`,
  `QueryBuilder._computeContextHash`, any EdDSA signed-message vector.
  The TS SDK's `QueryBuilder.toCircuitInputs` at
  `ts-sdk/packages/core/src/index.ts:257-279` performs query-context
  hashing in JavaScript -- the drift-prone surface -- with no vector
  guard.
- **Impact.** A silent divergence between Rust, WASM, circuits, and TS
  would ship and only surface when proofs start failing. The
  "cross-language vectors" CI gate in `CLAUDE.md` is advertised as
  byte-for-byte but only covers 20% of the surface.
- **Remediation.** Extend `gen_vectors.rs` to emit vectors for every
  primitive with a TS caller. Add `snarkjs wtns check` vectors for
  circuit-internal signals (Poseidon output, signature verification
  output).
- **Regression gate.** `cross_language_vectors` job fails if any
  TS-side primitive disagrees with the Rust reference.
- **Blocks:** external audit close-out.

---

### SOLID-SEC-011 -- E2E scripts bugged (3 concrete defects)

- **Severity:** HIGH
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:**
  - (a) `circuits/scripts/setup.js:57` writes `circuit_final.zkey`;
    `scripts/prove.ts:123` reads `batch_credential_query_final.zkey`
  - (b) `scripts/prove.ts:105` references `keccak256HashPair` which is
    never imported
  - (c) `scripts/issue.ts` lacks the
    `register_issuer -> stake_tokens -> vote_on_issuer ->
    finalize_voting or approve_via_trust_anchor` sequence, so on a
    clean registry `issueCredential` CPI-fails
- **Impact.** A stranger cannot run E2E from a clean checkout. All
  three are first-run blockers.
- **Remediation.** Fix both filename ends to agree, import the hashing
  helper (or switch to `poseidonHashPair` consistently), and add the
  issuer-approval bootstrap to `scripts/issue.ts` or a new
  `scripts/bootstrap_issuer.ts`.
- **Regression gate.** New CI job `e2e_localnet` that boots
  `solana-test-validator`, runs `scripts/bootstrap_issuer.ts`,
  `scripts/issue.ts`, `scripts/prove.ts` end-to-end, and asserts a
  successful on-chain verification.
- **Blocks:** external audit (auditors cannot reproduce your flow).

---

### SOLID-SEC-012 -- Trusted setup is single-party with timestamp entropy

- **Severity:** HIGH
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:** `circuits/scripts/setup.js:4,47`
  (`'solid-entropy-' + Date.now()`)
- **Description.** Single-party Phase-2 contribution. Entropy derivable
  from the ceremony timestamp. Whoever ran this script knows the toxic
  waste and can forge any proof.
- **Impact.** As documented (`CLAUDE.md:116`,
  `docs/IMPROVEMENTS_ROADMAP.md`). Pre-mainnet blocker.
- **Remediation.** Multi-party ceremony via `snarkjs zkey contribute`
  with at least 10 independent contributors. Attestation hashes
  published. Final zkey hosted on IPFS + Arweave with content
  addressing. Final hash pinned in the registry, in
  `deployments/mainnet.json`, and in `sec/audits/<mainnet-go-live>.md`.
- **Regression gate.** `verify_ceremony.js` that a newcomer can run to
  validate the full attestation transcript.
- **Blocks:** mainnet.

---

### SOLID-SEC-013 -- Slashing / fraud-proof authority is single-key

- **Severity:** MEDIUM
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:**
  - `programs/issuer-registry/src/lib.rs:409` (slash_issuer authority check)
  - `programs/issuer-registry/src/lib.rs:475-478` (submit_fraud_proof)
  - `programs/issuer-registry/src/lib.rs:894` (revoke_issuer)
- **Description.** Token-weighted voting decentralizes issuer approval,
  but slashing -- the economically consequential operation -- is gated
  by a single `registry_config.authority` key. No multisig, no on-chain
  governance proposal flow, no timelock.
- **Impact.** A single key compromise gives the attacker
  universal-slash capability against the issuer set.
- **Remediation.** Route authority through Squads multisig (3-of-5
  minimum) with a 24h challenge window on slashing, 48h timelock on VK
  rotation. Until then, document the threat model explicitly.
- **Regression gate.** Integration test that slashing fails from a
  non-multisig key once wired.
- **Blocks:** production release.

---

### SOLID-SEC-014 -- `stake_vault` is a single shared PDA

- **Severity:** MEDIUM
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:**
  - `programs/issuer-registry/src/lib.rs:750-752,791-793,862-863`
    (all use `[b"stake-vault"]` seed)
- **Description.** Every issuer deposits into the same PDA. Withdrawals
  are by raw lamport manipulation (`try_borrow_mut_lamports`). No
  per-issuer accounting against the shared vault; the stored
  `issuer.staked_amount` is authoritative. Lamports can arrive at the
  PDA address via `solana transfer` without going through this program.
- **Impact.** If the vault ever accumulates untracked lamports, a
  cascade of withdrawals can underflow / brick the vault. No direct
  theft; availability risk.
- **Remediation.** Per-issuer vaults seeded as
  `[b"stake-vault", issuer.authority.as_ref()]`. Simplifies slashing
  math and isolates failure.
- **Regression gate.** Integration test for two independent issuers'
  stake+withdraw flows not interfering.
- **Blocks:** production release.

---

### SOLID-SEC-015 -- `approve_via_trust_anchor` has no minimum-tier gate

- **Severity:** MEDIUM
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:** `programs/issuer-registry/src/lib.rs:533-574`
- **Description.** A `Government` / `Regulated` anchor can approve any
  `Pending` target regardless of the target's tier. No rate limit on
  approvals per anchor.
- **Impact.** A compromised Government-tier key becomes a global
  credential-minting primitive. Centralization risk in the trust-anchor
  design.
- **Remediation.** Require the anchor's tier to be strictly higher than
  the target. Add a per-anchor sliding-window rate limit (e.g., 10
  approvals / 24h / anchor).
- **Regression gate.** Unit test: `Community` cannot be approved by
  `Community`; rate-limit enforcement.
- **Blocks:** production release.

---

### SOLID-SEC-016 -- `transfer_authority` is single-step

- **Severity:** MEDIUM
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:** `programs/zk-verifier/src/lib.rs:140-146`
- **Description.** A misaddressed transfer immediately bricks the
  verifier's governance with no recovery.
- **Impact.** Operator footgun. Non-adversarial but irreversible.
- **Remediation.** Two-step "propose -> accept": `propose_authority`
  stores `pending_authority`; `accept_authority` under the new key
  commits the transfer. Optionally a `cancel_authority_transfer` under
  the old key.
- **Regression gate.** Unit test: proposing does not switch; accepting
  from wrong key fails; accepting from proposed key succeeds.
- **Blocks:** production release.

---

### SOLID-SEC-017 -- `solid-prover` uses deterministic `test_rng`

- **Severity:** MEDIUM
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:** `tools/solid-prover/src/lib.rs:90`
  (`let mut rng = ark_std::test_rng();`)
- **Description.** Groth16 prover randomness is used for the `r`/`s`
  blinding. `test_rng` is deterministic (seeded with 0).
- **Impact.** Privacy (not soundness). Identical witnesses produce
  identical proof bytes, so two proofs for the same holder/credential/query
  are linkable -- breaks the unlinkability story in
  `docs/private_onchain_identity_deep_dive.md`.
- **Remediation.** Use `rand::rngs::OsRng`.
- **Regression gate.** Unit test: two proofs with identical witness
  produce different `(proof.a, proof.b, proof.c)` bytes.
- **Blocks:** production release.

---

### SOLID-SEC-018 -- `verifier_config` write-lock caps throughput

- **Severity:** MEDIUM
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:** `programs/zk-verifier/src/lib.rs:495` (mut on every verify)
- **Description.** Every `verify_batch_proof` takes a write lock on the
  singleton `verifier_config` PDA (to bump `proof_count`). Solana
  schedules txs to disjoint write sets in parallel; one mutable
  singleton serialises the entire verify path.
- **Impact.** Caps sustained verify throughput at roughly 100/s at
  current CU budgets; 1000/s is unreachable without sharding.
- **Remediation.** Either (a) drop `proof_count` and use event-count
  metrics via an indexer, or (b) shard
  `verifier_config -> verifier_shard_{0..N}` with N=16 and modulo-hash
  the proof into a shard, then reduce off-chain.
- **Regression gate.** Load test on devnet: 300 concurrent verifies,
  no schedule-exclusion serialization.
- **Blocks:** real-infra claim. Not production-blocking for low-TPS
  deployments.

---

### SOLID-SEC-019 -- schema-registry handlers missing schema-hash re-assertion

- **Severity:** MEDIUM
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:** `programs/schema-registry/src/lib.rs:322-349,428-452`
- **Description.** `set_binding_status` and `transfer_tree_binding_authority`
  use `UpdateTreeRoot` context (seed-constrained on `schema_hash`) but
  do not re-assert `&data[8..40] == schema_hash.as_slice()` -- unlike
  `update_tree_root` (lines 292-295) which does.
- **Impact.** Low in practice (Anchor's `seeds` constraint already
  matches the PDA address). Defense-in-depth gap: any future refactor
  that reuses `UpdateTreeRoot` with a PDA whose contents can diverge
  from seeds would authorize on stored-authority alone.
- **Remediation.** Add the same `require!` assertion to both handlers.
- **Regression gate.** Unit test covering the invariant.
- **Blocks:** production hardening.

---

### SOLID-SEC-020 -- E2E scripts persist plaintext secrets

- **Severity:** MEDIUM
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:**
  - `scripts/issue.ts:118-125`
  - `.gitignore` (does not exclude `scripts/e2e_state.json`)
- **Description.** Issuer and holder secrets (Solana keypair bytes,
  BJJ private keys) written as plaintext JSON to `scripts/e2e_state.json`.
- **Impact.** A developer pushes local state and leaks long-term
  signing keys. Even on localnet, re-using this pattern for devnet
  would be catastrophic.
- **Remediation.** Write to `/tmp/solid-e2e-state.json` or
  `$XDG_RUNTIME_DIR`. Add `scripts/e2e_state.json` and
  `~/.solid-protocol/` to `.gitignore`. For devnet/mainnet paths,
  require an encrypted keystore (Argon2id + AES-256-GCM consistent
  with the holder story in `docs/key-management.md`).
- **Regression gate.** Pre-commit hook that rejects commits
  containing file names matching `*e2e_state*` or containing BJJ
  private-key magic bytes.
- **Blocks:** external developer onboarding safety.

---

### SOLID-SEC-021 -- Depth-20 circuit caps the protocol at ~250K holders

- **Severity:** MEDIUM
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:**
  - `circuits/batch_credential_query.circom:35-48` (`GLOBAL_DEPTH=20`)
  - `ts-sdk/packages/light/src/index.ts:137` (`maxDepth=20`)
  - `circuits/lib/identity_anchor.circom` (per-schema identity leaf in global tree)
- **Description.** The `GLOBAL_DEPTH` is compile-time baked into the VK.
  Since `identity_leaf` is per-schema, a single holder with N schemas
  occupies N slots. Practical cap for the global tree: ~250K holders
  before a new circuit rev + trusted setup is required.
- **Impact.** Silent scalability cliff. Not called out in any doc
  (not in `docs/infra_roadmap.md`, not in `docs/architecture.md`).
- **Remediation.** Ship a depth-24 circuit rev for the next trusted
  setup (buys 16x capacity). Until then, document the ceiling
  explicitly and add a monitor that alerts at 80% of capacity.
- **Regression gate.** Capacity monitor in the indexer / verifier
  dashboard (SOLID-SEC-018 addressed together).
- **Blocks:** real-infra claim.

---

### SOLID-SEC-022 -- Local `IsZero` reimplementation

- **Severity:** LOW
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:** `circuits/lib/credential_atom.circom:83-90`
- **Description.** Local `IsZero` template co-exists with circomlib's
  `comparators.circom` pulled in by the batch circuit. Template
  resolution depends on include order. Both forms are algebraically
  correct today.
- **Impact.** Soundness-neutral. Maintenance hazard: a future refactor
  that drops `in * out === 0` in the local copy would turn into a
  soundness bug.
- **Remediation.** Delete the local `IsZero`. Include
  `circomlib/comparators.circom` explicitly. Add an R1CS-hash
  regression gate.
- **Regression gate.** Snapshot R1CS hash in `circuits/build/` and
  fail CI on unexpected diff.
- **Blocks:** nothing, hygiene.

---

### SOLID-SEC-023 -- `active_issuers` counter drifts

- **Severity:** LOW
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:**
  - `programs/issuer-registry/src/lib.rs:344-347` (cooldown -> revoked
    with `staked_amount == 0` does not decrement)
  - vs `programs/issuer-registry/src/lib.rs:448` (slash decrements)
- **Description.** `approve_*` paths bump `active_issuers += 1`.
  `slash_issuer` decrements. `withdraw_after_cooldown` that transitions
  to `Revoked` does not. Counter drifts.
- **Impact.** `active_issuers` is informational today. Governance
  thresholds that reference it would drift.
- **Remediation.** Centralize status transitions in one helper that
  always updates the counter at the transition point.
- **Regression gate.** Property-based test on the transition table.
- **Blocks:** nothing, hygiene.

---

### SOLID-SEC-024 -- `unstake_tokens` uses raw `-=`

- **Severity:** LOW
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:** `programs/issuer-registry/src/lib.rs:285`
- **Description.** Protected earlier by `require!(staker_account.amount_staked >= amount)`
  (line 263). Same function at line 251 does `amount_staked += amount`
  without `checked_add`.
- **Impact.** Not currently exploitable. Style / hardening only.
- **Remediation.** `checked_add` / `checked_sub` everywhere on u64
  balance math. Adopt clippy lints `arithmetic_side_effects`.
- **Regression gate.** clippy rule in CI.
- **Blocks:** nothing, hygiene.

---

### SOLID-SEC-025 -- `CheckIssuerStatus` ungated, never called

- **Severity:** INFO
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:** `programs/issuer-registry/src/lib.rs:577-582,883-886`
- **Description.** No signer, no caller check, no on-chain invocation.
  Makes `CLAUDE.md`'s "on-chain fallback via CPI" option look wired up
  when it is not.
- **Impact.** Misleading. No security implication.
- **Remediation.** Either wire it up (addresses SOLID-SEC-004 partially)
  or delete it.
- **Regression gate.** n/a.
- **Blocks:** nothing, clarity.

---

### SOLID-SEC-026 -- `Credential::verify_integrity` never called on-chain

- **Severity:** INFO
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:** `crates/solid-core/src/credential.rs`
- **Description.** Its purpose is SDK-side sanity. The "commitment
  matches attestation data" assertion is only as strong as the
  off-chain issuance path.
- **Impact.** Documentation clarity. No security implication.
- **Remediation.** Add a docstring at the function noting it is an
  SDK-internal invariant, not a consensus check.
- **Regression gate.** n/a.
- **Blocks:** nothing, documentation.

---

### SOLID-SEC-027 -- `IMPROVEMENTS_ROADMAP.md` has stale checkboxes

- **Severity:** INFO
- **Status:** Open
- **Introduced:** 2026-04-22
- **Last updated:** 2026-04-22
- **Evidence:** `docs/IMPROVEMENTS_ROADMAP.md` (all 37 checkboxes unticked
  despite ~24 items closed in code)
- **Description.** The canonical backlog per `CLAUDE.md:102-103` is
  stale and directly contradicts `docs/POST_REMEDIATION_AUDIT.md`.
- **Impact.** Misleads future auditors. An auditor acting only on the
  roadmap would re-open items already closed.
- **Remediation.** Flip closed items in the same PR that cites the
  underlying commit. Add `scripts/check_docs.py` to enforce that closed
  items in the registry and the roadmap agree.
- **Regression gate.** CI job that diff-checks the roadmap against
  the registry's status board.
- **Blocks:** nothing, documentation integrity.

---

## History

| Date       | Audit                                                | Findings added | Findings closed |
|------------|------------------------------------------------------|----------------|-----------------|
| 2026-04-22 | `sec/audits/2026-04-22_v0.3_comprehensive_audit.md`  | SOLID-SEC-001 .. SOLID-SEC-027 | 0 |

---

## Next-cycle checklist (for the auditor running the next pass)

1. For every `Fixed` status here, verify the referenced commit lands
   the claimed regression test and the fix addresses the root cause.
   Flip `Fixed -> Verified` or reopen with a child entry
   `SOLID-SEC-NNN.1`.
2. For every `Open` status here older than 90 days, decide:
   escalate severity, accept-as-`Won't Fix` with compensating control,
   or flag as a milestone slip.
3. Run the full audit methodology against any code changed since the
   last snapshot. Net-new findings append here.
4. Update the summary table counts and the `Last audit` header at top.
5. Create the new snapshot under `sec/audits/<date>_<version>_<slug>.md`.
