> SolID Protocol -- Current State
> ===============================
>
> Per-component snapshot of the implementation as of v0.6
> (post-Phase-2, pre-Phase-3-close).  Every row is a direct mirror of
> a section in [`SYSTEM_VISION.md`](./SYSTEM_VISION.md) so the two
> documents can be read side-by-side: vision on the left, current
> reality on the right.
>
> Conventions used here:
>
>   - `[X]`  closed   -- in code, in CI, internally audited.
>   - `[~]`  partial  -- in code; lacks audit / regression coverage
>                        / a piece of the design.
>   - `[ ]`  open     -- not in code; tracked in
>                        [`IMPROVEMENTS_ROADMAP.md`](./IMPROVEMENTS_ROADMAP.md)
>                        or an ADR.
>
> File-and-line references use `path:line` per CLAUDE.md.
> ASCII-only per CLAUDE.md.

# 1. Programs (the contract)

The three Anchor programs are the contract.  Per the working-style
note in CLAUDE.md, programs and circuits are core: if they are
correct, scripts adapt to them; if they are wrong, they must be
fixed first.

## 1.1 zk-verifier

| Item                                                              | Status | Where                                                      |
| ----------------------------------------------------------------- | ------ | ---------------------------------------------------------- |
| `verify_batch_proof` instruction                                  | [X]    | programs/zk-verifier/src/lib.rs                            |
| Owner-check on `global_tree` (schema-registry-owned)              | [X]    | programs/zk-verifier/src/lib.rs                            |
| Owner-check on `schema_tree_N` (schema-registry-owned)            | [X]    | programs/zk-verifier/src/lib.rs                            |
| Owner-check on `issuer_tree_binding` (issuer-registry-owned)      | [X]    | ADR-0014; programs/zk-verifier/src/lib.rs                  |
| 32 public inputs (post-ADR-0014); `ISSUER_TREE_ROOT_INPUT_INDEX`  | [X]    | programs/zk-verifier/src/lib.rs                            |
| 6-input Poseidon nullifier preimage (includes issuer_tree_root)   | [X]    | circuits + zk-verifier consts (SOLID-SEC-008)              |
| Nullifier PDA write (replay-reject)                               | [X]    | programs/zk-verifier/src/lib.rs                            |
| `request_vk_rotation` + `commit_vk_rotation` (48h timelock)       | [X]    | ADR-0015 part 1                                            |
| Freeze gate (`vk_finalized=false` blocks verify during rotation)  | [X]    | ADR-0015 part 1                                            |
| `VerifierConfig::SPACE = 60` (with rotate fields)                 | [X]    | programs/zk-verifier/src/lib.rs                            |
| `vk_generation` bound into circuit publics                        | [ ]    | ADR-0015 part 2; needs next trusted-setup cycle            |
| Per-instruction CU receipts captured in regression suite           | [ ]    | tests/integration scenarios 02..11 unimplemented           |
| Multi-party trusted-setup ceremony                                | [ ]    | SOLID-SEC-012; mainnet blocker                             |

## 1.2 issuer-registry

| Item                                                              | Status | Where                                                      |
| ----------------------------------------------------------------- | ------ | ---------------------------------------------------------- |
| `RegistryConfig` and `IssuerAccount` data layouts                 | [X]    | programs/issuer-registry/src/lib.rs                        |
| `register_issuer` (stake + BJJ pubkey)                            | [X]    | programs/issuer-registry/src/lib.rs                        |
| `sec007-skip-onchain` feature flag for BJJ subgroup CU            | [X]    | programs/issuer-registry/src/lib.rs                        |
| `vote_on_issuer` + `finalize_voting` (DAO admission)              | [X]    | programs/issuer-registry/src/lib.rs                        |
| `append_issuer_leaf` (CPI: SPL AC append, signed by `issuer-tree-authority`) | [X] | programs/issuer-registry/src/lib.rs |
| `update_issuer_tree_root` (push new root into binding)            | [X]    | programs/issuer-registry/src/lib.rs                        |
| `revoke_issuer_atomic` (one-ix replace_leaf + nonce bump + root)  | [X]    | programs/issuer-registry/src/lib.rs                        |
| `initialize_issuer_tree_binding` (init-only)                      | [X]    | programs/issuer-registry/src/lib.rs                        |
| `issue_credential` (CPI: SPL AC append, signed by `[b"tree-authority", schema_hash]`) | [X] | programs/issuer-registry/src/lib.rs:1582 |
| `request_withdrawal_atomic` (mirror of `revoke_issuer_atomic`)    | [ ]    | SOLID-SEC-044 (LOW)                                        |
| Squads 3-of-5 gating on `issuer_tree_operator`                    | [ ]    | SOLID-SEC-043 (MEDIUM); mainnet blocker                    |
| Cross-language vectors: 3/10 primitives covered                   | [~]    | SOLID-SEC-010 (HIGH); tests/vectors/                       |

## 1.3 schema-registry

| Item                                                              | Status | Where                                                      |
| ----------------------------------------------------------------- | ------ | ---------------------------------------------------------- |
| `register_schema` (Schema PDA = (b"schema", name, [version]))     | [X]    | programs/schema-registry/src/lib.rs                        |
| `initialize_global_binding` (singleton)                           | [X]    | programs/schema-registry/src/lib.rs                        |
| `initialize_tree_binding(schema_hash, tree_pubkey)` (init-only)   | [X]    | programs/schema-registry/src/lib.rs:189                    |
| `update_global_root` / `update_schema_root`                       | [X]    | programs/schema-registry/src/lib.rs                        |
| `IncrementUsage` access control                                   | [ ]    | P1-2 in IMPROVEMENTS_ROADMAP.md                            |

# 2. Circuits

| Item                                                              | Status | Where                                                      |
| ----------------------------------------------------------------- | ------ | ---------------------------------------------------------- |
| `batch_credential_query.circom` with 32 public inputs (ADR-0014)  | [X]    | circuits/batch_credential_query.circom                     |
| Phase 3.4: `LessThanBN254` (254-bit-safe schema ordering)         | [X]    | circuits/lib/lt_bn254.circom; `batch_credential_query.circom` |
| Phase 3.4: `BabyPbk254` (254-bit-safe credPriv to pubkey)         | [X]    | circuits/lib/identity_anchor.circom                        |
| Circuit vectors + mocha regression (Rust ground truth)            | [X]    | `crates/solid-core/examples/gen_circuit_vectors.rs`; `circuits/test/`; CI `cross_language_vectors` |
| `nullifier_*.circom` with 6-input Poseidon                        | [X]    | circuits/                                                  |
| `credential_hasher.circom` (5-arg Poseidon over commitment)       | [X]    | circuits/lib/credential_hasher.circom                      |
| Constraint: `numPredicates <= MAX_PREDICATES`                     | [ ]    | P2-1                                                       |
| Constraint: `compoundLogic` is binary                             | [ ]    | P2-2                                                       |
| `expirationTimestamps` constraint                                 | [ ]    | P2-3                                                       |
| `globalSiblings` depth no longer hardcoded                        | [ ]    | P2-7                                                       |
| `vk_generation` in circuit publics                                | [ ]    | ADR-0015 part 2; batches with next trusted-setup cycle     |
| Trusted setup ceremony artifacts published                        | [ ]    | SOLID-SEC-012; setup currently single-party in `circuits/scripts/setup.js` |

# 3. WASM bridge (top-level `wasm/` crate)

| Item                                                              | Status | Where                                                      |
| ----------------------------------------------------------------- | ------ | ---------------------------------------------------------- |
| `wasm-pack build wasm/ --target nodejs` produces a working bundle | [X]    | wasm/                                                      |
| Smoke test exists                                                 | [X]    | scripts/wasm_bridge_smoke.mjs                              |
| Smoke test wired as CI regression gate                            | [ ]    | tracked in todos                                           |
| `crates/solid-core` stays BPF-compatible (no `#[wasm_bindgen]`)   | [X]    | SOLID-SEC-028 / ADR-0002                                   |

# 4. TypeScript SDK

| Package                       | Status | Notes                                                      |
| ----------------------------- | ------ | ---------------------------------------------------------- |
| `@solid-protocol/core`        | [X]    | Poseidon, BJJ keypair, PROGRAM_IDS                         |
| `@solid-protocol/light`       | [X]    | LocalReplicaAdapter, PDA derivers, SPL AC ID exports       |
| `@solid-protocol/issuer`      | [X]    | `issueCredential` builds the CPI'd issue tx                |
| `@solid-protocol/holder`      | [~]   | Has `generateProof`; `SolID.prove()` is incomplete (P2-14) |
| `@solid-protocol/verifier`    | [ ]    | Missing entirely (P0-6)                                    |

# 5. Scripts (the wrappers)

Per the working-style rule, scripts are wrappers that respect the
contract.  A script does **not** invent new flow; it sequences
program instructions in the order the programs require.  When the
program is correct (e.g. init-only `initialize_*_tree_binding`),
the script follows that contract -- the script does not paper over
it with placeholders.

| Script                                | Status | Notes                                                                                                   |
| ------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------- |
| `scripts/initialize.ts`               | [X]    | Steps 1..7 closed.  Step 3 (schema-tree-binding) and step 5 (issuer-tree-binding) now correctly **skip** when `SOLID_TREE_PUBKEY` / `SOLID_ISSUER_TREE_PUBKEY` are unset, deferring to the dedicated bootstrap scripts.  State writes are idempotent: tree pubkeys are preserved across re-runs. See **§5.1** below for the historical context. |
| `scripts/backfill_issuer_tree.ts`     | [X]    | Reference for the one-shot tree+authority+binding pattern.                                              |
| `scripts/bootstrap_schema_tree.ts`    | [X]    | Per-schema tree bootstrap (mirrors `backfill_issuer_tree.ts`).  Wired into `npm run e2e` between `backfill-issuer-tree` and `bootstrap-issuer`. |
| `scripts/bootstrap_issuer.ts`         | [X]    | Compresses register / vote / approve / enroll for one issuer.                                           |
| `scripts/issue.ts`                    | [X]    | Consumes the schema-tree binding produced by `bootstrap_schema_tree.ts`.                                |
| `scripts/prove.ts`                    | [X]    | Generates Groth16 proof, verifies on-chain.                                                             |
| `scripts/check_program_ids.py`        | [X]    | CI gate.                                                                                                |
| `scripts/build_idls.mjs`              | [X]    | Denamespaces account names; required for Anchor TS client to find `issuerAccount` / `schemaAccount`.    |

## 5.1 The schema-tree gap (closed 2026-04-26)

Historically `scripts/initialize.ts` called
`schema-registry::initialize_tree_binding(schema_hash, PublicKey.default)`
when `SOLID_TREE_PUBKEY` was unset.  That instruction is init-only
(programs/schema-registry/src/lib.rs:189) -- once the binding is
created, `tree_pubkey` is immutable -- so a default-keyed binding
was permanently broken.  Any subsequent attempt to point it at the
real tree failed with `already in use`, and `issue_credential` could
never succeed against it because the SPL AC account at
`PublicKey.default` does not exist.

This is now closed in three commits:

  1. **`scripts/initialize.ts`** -- step 3 (schema-tree-binding) and
     step 5 (issuer-tree-binding) **skip** when their respective env
     pubkeys are unset.  State writes preserve any tree pubkey a
     prior bootstrap run wrote, so `npm run init-onchain` is
     re-runnable mid-pipeline without bricking downstream scripts.
     The init-only-contract rationale is captured in the file's
     module docstring.
  2. **`scripts/bootstrap_schema_tree.ts`** (new) -- one-shot
     bootstrap that:
       a. `SystemProgram::createAccount` for the SPL AC tree, sized
          via `getConcurrentMerkleTreeAccountSize`.
       b. SPL AC `init_empty_merkle_tree` with the operator wallet
          as a temporary authority (a PDA cannot sign this ix
          directly from a TS client; the strictly-cleaner fix is an
          in-program `initialize_schema_tree` wrapper, tracked as a
          future contract addition).
       c. SPL AC `transfer_authority` -> `("tree-authority",
          schema_hash)` PDA owned by issuer-registry.
       d. `schema-registry::initialize_tree_binding(schema_hash,
          tree.publicKey)` with the real pubkey.
     Re-run safe; idempotent at every step.  Refuses non-localnet
     RPCs absent `SOLID_ALLOW_NON_LOCALNET=1` (SOLID-SEC-039).
  3. **`package.json::e2e`** -- now runs
     `bootstrap-schema-tree` between `backfill-issuer-tree` and
     `bootstrap-issuer`:

         build:idl
         init-onchain
         backfill-issuer-tree
         bootstrap-schema-tree   <-- new
         bootstrap-issuer
         issue
         prove

This mirrors the `backfill_issuer_tree.ts` pattern exactly; the
only difference is the binding-PDA seed (`b"schema-tree-binding"`,
`schema_hash` vs. `b"issuer-tree-binding"`) and the authority-PDA
seed (`b"tree-authority"`, `schema_hash` vs. `b"issuer-tree-authority"`).

The trade-off in **5.1.b** is one transaction-window during which
the schema tree's authority is the operator wallet, immediately
closed by `transfer_authority`.  This is identical to the trade-off
`backfill_issuer_tree.ts` makes for the issuer tree and is
documented at `scripts/backfill_issuer_tree.ts:201-227`.  The
strictly-cleaner fix (an in-program `initialize_schema_tree`
wrapper) is a future contract addition, not a regression.

## 5.2 Canonical batch-slot layout: actives first, padding last (closed 2026-04-26; circuit comparator hardened 2026-04-27)

**Historical (pre-Phase-3.4, 2026-04-26 and earlier).**  The batch
circuit enforced strictly ascending `schemaHashes` across active
slots via `LessThan(252)` (`Num2Bits(253)` on a modular difference),
which spuriously rejected valid full-width Poseidon outputs whenever
bit 253 of the witness difference was set.  The holder SDK therefore
sorted actives first and padding last so `(h_i, h_{i+1})` and
`(h, 0)` witnesses stayed inside the 253-bit comparator envelope.

**Current (Phase 3.4, 2026-04-27).**  `circuits/batch_credential_query.circom`
replaces that comparator with `LessThanBN254` from
`circuits/lib/lt_bn254.circom` (`Num2Bits_strict` + `AliasCheck` over
the full BN254 scalar field).  The witness is now 254-bit-safe; the
actives-first / padding-last SDK sort remains the contract that
matches the on-chain verifier's position-agnostic ascending check
(`programs/zk-verifier/src/lib.rs`).

The following paragraph documents **why** the SDK sort existed and
remains load-bearing for verifier alignment (not because the circuit
still rejects bit-253 ordering witnesses).

The on-chain verifier's contract for the same constraint
(`programs/zk-verifier/src/lib.rs:495-508`) is *position-agnostic*:
it iterates slots `0..4`, skips any whose `(merkle_root,
schema_hash)` are both zero, and requires only that the surviving
schemas are strictly ascending.  `[h, 0, 0, 0]`, `[0, h, 0, 0]`,
`[h1, 0, h2, 0]` and `[0, 0, 0, h]` are all on-chain-valid for one
or two active credentials.

The circuit ordering template is still written as a strict ascending
chain across the four batch slots; the verifier walks slots in index
order but skips empty `(merkle_root, schema_hash)` pairs.  Keeping
actives first in the SDK avoids packing holes that would force the
circuit to compare unrelated schema hashes across a padding slot in
a way that is harder to reason about, even though `LessThanBN254`
no longer has the old 253-bit false-reject pathology.

Sorting active credentials first therefore remains the recommended layout:

  - active->active pairs: `LessThanBN254` enforces `schemaHashes[i] <
    schemaHashes[i+1]` whenever slot `i+1` is non-zero
    (`orderingNextNotZero[i] = 1` in `batch_credential_query.circom`).
  - transitions into padding: the same gate is multiplied by
    `orderingNextNotZero[i]`, so `(h, 0)` pairs do not need to satisfy
    `h < 0`.
  - padding->padding: both sides zero; the ordering line is gated off.

The SDK contract (`ts-sdk/packages/holder/src/index.ts`
`generateBatchProof`) therefore sorts:

  1. actives (where `schemaHash != 0`) first, ascending by
     `schemaHash` hex,
  2. padding (`schemaHash == 0`) last.

This is consistent with the on-chain verifier's contract -- no
ordering disagreement is introduced.  The 254-bit-safe comparator
shipped under Phase 3.4 without waiting for the next trusted-setup
cycle (it is not a VK-changing semantic shift to public inputs; it
fixes unsound witness rejection on honest inputs).

## 5.3 BabyJubJub curve form + rust-analyzer-stable Fq literals (closed 2026-04-27)

`ark-ed-on-bn254` uses the **normalized** twisted Edwards model
(`a' = 1`); circomlib uses the **native** model (`a = 168700`,
`d = 168696`).  They are isomorphic via `x_ark = sqrt(a) * x_circ`,
`y_ark = y_circ`.  Every wire byte in `BJJPublicKey`, EdDSA hash
inputs, and Poseidon leaves that include a coordinate must stay in
**circomlib-native** form; `crates/solid-core/src/babyjubjub.rs`
applies the forward / inverse map at each boundary.

Pinned field elements (`sqrt(a)`, `1/sqrt(a)`, and the arkworks-form
Base8 coordinates) are encoded as `const [u8; 32]` little-endian
limbs consumed by `Fq::from_le_bytes_mod_order`, not `ark_ff::MontFp!`,
so IDE proc-macro expansion cannot panic while the values stay
identical to the decimal literals they replaced.

Regression surface: `cargo test -p solid-core --lib babyjubjub`;
`cargo run -p solid-core --example gen_circuit_vectors`;
`cd circuits && npm test` (isolated `lt_bn254`, `babypbk254`,
`identity_anchor` templates witness-checked against the JSON
fixture; CI regenerates both vector files and asserts `git diff
--exit-code`).

# 6. Hard-invariant matrix (mirror of SYSTEM_VISION §3.3)

| Invariant                                                              | Enforced by                                                        | Status |
| ---------------------------------------------------------------------- | ------------------------------------------------------------------ | ------ |
| Anchor.toml + declare_id! + deployments/*.json all match               | scripts/check_program_ids.py (CI)                                  | [X]    |
| Rust + TypeScript primitives byte-for-byte                             | tests/vectors/ (CI gate exists; coverage 3/10 primitives)          | [~]    |
| Verifier owner-checks global_tree                                      | zk-verifier::verify_batch_proof                                    | [X]    |
| Verifier owner-checks schema_tree_N                                    | zk-verifier::verify_batch_proof                                    | [X]    |
| Verifier owner-checks issuer_tree_binding (ADR-0014)                   | zk-verifier::verify_batch_proof                                    | [X]    |
| 6-input Poseidon nullifier (includes issuer_tree_root)                 | circuits + zk-verifier consts                                      | [X]    |
| VK rotation 48h timelock + freeze gate (ADR-0015 part 1)               | zk-verifier::request/commit_vk_rotation                            | [X]    |
| `vk_generation` in circuit publics (ADR-0015 part 2)                   | requires next trusted-setup cycle                                  | [ ]    |
| Batch slots: actives first, padding last (§5.2; aligns verifier walk) | holder SDK `generateBatchProof` sort                       | [X]    |
| 254-bit-safe schemaHash comparator (`LessThanBN254`)                   | circuits/lib/lt_bn254.circom + batch circuit               | [X]    |
| `verification_key.sha256` content-addressed gate                       | initialize.ts SOLID-SEC-041 gate                                   | [X]    |
| Squads 3-of-5 governance for issuer-tree-operator (SOLID-SEC-043)      |                                                                    | [ ]    |
| `request_withdrawal_atomic` mirrors `revoke_issuer_atomic`             | SOLID-SEC-044                                                      | [ ]    |
| Multi-party trusted setup ceremony                                     | SOLID-SEC-012                                                      | [ ]    |

# 7. Open work pointers

For issue-level granularity see:

  - [`IMPROVEMENTS_ROADMAP.md`](./IMPROVEMENTS_ROADMAP.md)
    -- P0/P1/P2/P3 backlog (load-bearing list).
  - [`E2E_BLOCKERS.md`](./E2E_BLOCKERS.md)
    -- per-cluster E2E run blockers and receipts.
  - [`FORWARD_ROADMAP.md`](./FORWARD_ROADMAP.md)
    -- Phase-3 close-out planning.
  - [`REVOCATION_DESIGN.md`](./REVOCATION_DESIGN.md)
    -- v1 revocation operator workflow (holder SDK helper +
       indexer event contract still open).
  - [`MODULE_CONTRACTS.md`](./MODULE_CONTRACTS.md)
    -- account layouts and ix interfaces in detail.
  - [`sec/audits/2026-04-24_v0.6_deep_comprehensive_audit.md`]
    -- the canonical post-Phase-2 audit.

# 8. Update protocol for this document

Whenever a row in §1, §2, or §5 changes status, update this file in
the same PR.  CLAUDE.md treats `IMPROVEMENTS_ROADMAP.md` as the
issue-level backlog; this document is the component-level mirror of
the vision and should never disagree with the deployed reality.

# 9. Session deltas (2026-04-28)

- **Circuit revision.**  `batch_credential_query.circom` gained
  the SEC-050 padding-canonicality constraint
  `isZero[i].out * (1 - isZero[i+1].out) === 0`.  Trusted-setup
  re-run; new VK pin
  `8385b82b032f65e505c784b28486ca8bec7da3f3d4b97b82724e697734565146`
  (the prior pin `debca4c0...` is stale).
- **`crates/solid-core/src/babyjubjub.rs::is_on_curve` and
  `is_identity`** rewritten to evaluate the circomlib-native
  twisted-Edwards equation directly (`a*x^2 + y^2 == 1 +
  d*x^2*y^2`, `a = 168700, d = 168696`).  Pure `Fq * Fq` /
  `Fq + Fq` so behaviour matches host and BPF byte-for-byte.
  Tracked as SOLID-SEC-052.
- **WASM bridge rebuilt** (`wasm-pack build wasm/`).  Required
  on any commit that touches the byte-format helpers in
  `crates/solid-core/src/babyjubjub.rs`; cff06c2 missed this
  step and silently broke the off-chain<->on-chain wire contract.
  Process gate added at `docs/E2E_BLOCKERS.md` B11.
- **Program-side closures.**  SEC-045 (atomic binding update),
  SEC-049 (replace_leaf discriminator), SEC-044 (verified).
  See `plan/RESUME.md` §"2026-04-28" and
  `sec/SECURITY_REGISTRY.md` for receipts.
- **Open trackers introduced.**  SEC-051 (all-padding circuit;
  defer to next setup cycle), B11 (WASM rebuild gate), B12
  (EdDSA witness drift in `CredentialAtom`; live e2e edge --
  `npm run prove` rejects).
- **Test counts.**  Workspace 167/167 cargo + 39/39 circuit
  witness-tester (mocha) green.
- **E2E status.**  Steps 1-17 of the runbook + `init-onchain`
  + `backfill-issuer-tree` + `bootstrap-schema-tree` +
  `bootstrap-issuer` + `issue` all green.  Live edge:
  `npm run prove`.
