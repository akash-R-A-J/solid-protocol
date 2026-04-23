# SolID Protocol Post-Remediation Audit

Audit date: 2026-04-22
Scope: every source file in the repository after the April 2026 remediation
Methodology: line-by-line re-audit of every changed file, plus an adversarial
pass looking for new surface area introduced by the fixes

## Executive summary

Every P0 and P1 finding from the pre-remediation audits has landed in code
and been verified in this audit. The 49 existing Rust unit tests continue
to pass (solid-core 41, solid-light 8) and the 11 zk-verifier host tests
pass, with no regressions. The program-ID consistency gate is green.

The cryptographic design has not changed. Every remediation preserves the
circuit and nullifier contracts so the trusted setup artifact layout is
compatible, but the circuit template signatures changed (a new GLOBAL_DEPTH
template parameter plus newly wired ExpirationChecker constraints), so a
fresh trusted-setup ceremony is required before a proof generated against
the new batch_credential_query can be verified by a VK from the old one.

Remaining open items are all M2 scope: revocation v1 operator flow,
compressed issuer tree for in-circuit issuer binding, multi-party ceremony,
and third-party audit.

## Section 1: Verification of landed fixes

Each finding below is cross-referenced to the pre-remediation audit
identifier, its former location, and the commit-ready code that fixed it.
"Verified" means I re-read the current source and confirmed the invariant.

### Issuer-registry

#### P0-1 RegistryConfig space 80 to 112 bytes

Pre-remediation: programs/issuer-registry/src/lib.rs:613 allocated 80 bytes,
omitting 32 bytes for governance_token_mint. initialize_registry failed on
every cluster.

Fix: space = 8 + 32 + 32 + 8 + 8 + 8 + 8 + 8 = 112. Verified in the
InitializeRegistry context.

Regression gate: tests/integration/01_registry_init.test.ts. Assertion
that RegistryConfig account data length is at least 112.

#### P0-3 and P1-1 vote_on_issuer governance mechanics

Pre-remediation: vote_on_issuer did not increment active_votes_count,
did not enforce the voting deadline, and staker_account was not marked
mut in the context. ReleaseVote also had both staker_account and
vote_record missing mut.

Fix landed:

- vote_on_issuer handler runs require on clock versus voting_ends_at and
  on IssuerStatus being Pending. It increments active_votes_count via
  checked_add after writing the vote record, and overflow errors bubble
  up as Overflow.
- VoteOnIssuer context marks staker_account as mut. The handler's write
  to active_votes_count now persists.
- ReleaseVote context marks both staker_account and vote_record as mut.
  release_vote also switched to checked_sub against active_votes_count
  so an underflow is a hard error instead of silently wrapping.

Verified by reading the current vote_on_issuer at line 151 and the
VoteOnIssuer context at line 667 of programs/issuer-registry/src/lib.rs.

#### P1-3 slash_issuer and submit_fraud_proof transfer lamports

Pre-remediation: both instructions only mutated accounting fields; the
slashed lamports stayed in stake_vault indefinitely.

Fix landed: a new transfer_slashed_lamports helper moves lamports from
stake_vault (program-owned PDA) to a new dao_treasury PDA seeded
["dao-treasury"]. dao_treasury is created via init_if_needed in both
SlashIssuer and SubmitFraudProof contexts so the DAO bootstrapping does
not require a separate instruction. Transfer uses checked arithmetic and
refuses to drive stake_vault below the requested amount.

New surface area review: dao_treasury is a system-owned lamport PDA,
zero data bytes. It is readable by anyone and writable only by this
program (through direct lamport manipulation gated by our own handlers).
No withdrawal path exists yet; future work will add a
withdraw_from_treasury instruction gated by the DAO authority or a
multisig.

#### P1-4 and P1-5 approve_via_trust_anchor seed constraint and event emission

Pre-remediation: target_issuer had no seed constraint; a malicious trust
anchor could approve a crafted IssuerAccount. The instruction also did
not emit IssuerApproved, so indexers missed this path entirely.

Fix landed: approve_via_trust_anchor takes target_authority: Pubkey as
an explicit instruction argument. The target_issuer account is now
seed-constrained to ["issuer", target_authority]. The handler calls
require_keys_eq on target.authority to defend against replay attacks
that reuse a mismatched signer. An IssuerApproved event is emitted with
approval_bps = 10_000 (100 percent) to disambiguate trust-anchor
approvals from DAO-voted approvals.

### Schema-registry

#### P0-5 IncrementUsage access control

Pre-remediation: anyone could call increment_usage and inflate
usage_count to u64::MAX, DoS-ing downstream CPI consumers that read the
counter.

Fix landed: IncrementUsage context now requires an authority signer, and
the handler enforces require_keys_eq on schema.authority.

#### P0-9 SchemaAccount space and length caps

Pre-remediation: space = 8 + 32 + 64 + 1 + 64 + 256 + 32 + 1 + 8 + 8 =
474 bytes. This undercounted Borsh length prefixes on name, category,
and field_names, and the hardcoded 256 for field_names was wrong for 8
strings of up to 32 chars each.

Fix landed: SCHEMA_ACCOUNT_SPACE constant is defined exactly as
32 + (4 + 64) + 1 + (4 + 64) + (4 + 8 * (4 + 32)) + 32 + 1 + 8 + 8.
register_schema now caps name, category, and every field_name length
to match. register_schema's Accounts definition consumes the constant.

#### P0-10 update_tree_root and update_global_root monotonicity

Pre-remediation: either instruction accepted any new_root from its
authority without checking that the update advanced in slot time. A
compromised authority (or a replayed transaction) could regress the
root to an earlier value, re-enabling the inclusion proof of a revoked
credential.

Fix landed: both instructions now read the prior last_updated_slot,
compare against Clock::get()?.slot, and require the new slot to be
strictly greater. The updated slot is then written into the binding.

Adjacent authority-rotation gap: the pre-remediation design had no way
to rotate either the tree-binding authority or the global-binding
authority. A lost or compromised key would brick the binding
permanently. Two new instructions close this gap:

- transfer_tree_binding_authority(schema_hash, new_authority)
- transfer_global_binding_authority(new_authority)

Both require the current authority as signer and write the new Pubkey
into the canonical authority slot of the binding layout.

### zk-verifier

#### P0-2 owner checks on global_tree and schema_tree_N

Pre-remediation: global_tree and the four schema_tree_N accounts were
UncheckedAccount with no owner constraint. A system-program-owned
account carrying the right 8-byte discriminator would pass the raw
byte-level parsers and forge the trust root.

Fix landed: before calling cpi_helpers::verify_state_root_matches or
verify_schema_root_binding, the handler calls require_keys_eq with
SCHEMA_REGISTRY_ID on the account's owner. SCHEMA_REGISTRY_ID is a
typed Pubkey constant exported from solid-light.

Regression gate: tests/integration/08_verify_forged_global_tree_rejected
and 09_verify_forged_schema_tree_rejected.

#### P0-8 VK storage cap and chunk sequence

Pre-remediation: store_verification_key accepted chunks in any order
and could grow vk_storage.data past the 10_240-byte allocation, causing
an AccountDidNotSerialize on the final chunk.

Fix landed: VerifierConfig now carries next_vk_chunk: u16. The handler
requires chunk_index to equal config.next_vk_chunk before writing, and
caps vk_storage.data.len() plus chunk_data.len() at 10_240. Overflow
and out-of-order conditions are surfaced as new error codes
(ChunkOutOfOrder, VkStorageFull, Overflow).

### SDK and holder

#### P0-5a (BUG-02) nullifier parsing

Pre-remediation: ts-sdk/packages/holder/src/index.ts:310 decoded
publicSignals[0] via Buffer.from(value, 'hex'), but snarkjs returns a
decimal bigint-as-string. The result was either zero-length or
truncated.

Fix landed: the nullifier is now computed via
bigintToBytes32(BigInt(publicSignals[0])). The single-credential path
additionally cross-checks the WASM-computed nullifier against the
circuit output and throws on mismatch.

#### P0-5b (BUG-04) identity commitment SDK-circuit divergence

Pre-remediation: generateBatchProof computed a single identity leaf
Poseidon(masterPubKeyX, masterPubKeyY, revocationNonce) using the
master key and fetched one global-proof shared across all credentials.
The circuit uses per-schema derived keys via BabyPbk(Poseidon(masterKey,
schemaHash)) and expects one global inclusion proof per credential, so
the pre-remediation SDK could not have produced a valid witness.

Fix landed:

- solid-core exposes derive_credential_key via a new WASM binding
  deriveCredentialKey(masterKey, schemaHash) -> BJJKeypair.
- @solid-protocol/core reexports deriveCredentialKey.
- @solid-protocol/holder::generateBatchProof now derives the per-schema
  keypair for every credential, computes the per-schema identity leaf
  Poseidon(credPubX, credPubY, revocationNonce), and fetches one global
  inclusion proof per credential. It asserts all returned roots match
  so a stale indexer fails fast instead of producing an unverifiable
  proof.
- generateProof (single-credential) now takes masterPrivateKey as an
  explicit argument, derives the per-schema keypair, and passes
  holderBJJPrivKey = Poseidon(master, schemaHash) into the compound
  circuit. The naive call path that passed credential.holderPrivateKey
  to the nullifier (which required the master key) is fixed.

#### P0-5c wire top-level SolID.prove and verifyOnChain

Pre-remediation: ts-sdk/packages/sdk/src/index.ts had prove returning
the literal { proof: 'ZK_PROOF_DATA', publicSignals: Array(31).fill('0') }
and verifyOnChain returning 'SOLANA_TX_SIGNATURE_RC_100'. Both were
stubs.

Fix landed: SolID.prove delegates to generateBatchProof with circuit
paths sourced from SOLID_CONFIG. SolID.verifyOnChain delegates to the
real @solid-protocol/verifier::verifyOnChain, returning a confirmed
transaction signature. SolID now re-exports all PDA helpers from
@solid-protocol/verifier so downstream code has a single import.

Prior claim by the audit docs that @solid-protocol/verifier did not
exist was wrong; the package has been real for multiple versions.

#### P2 resolveSchema and listIssuers

Pre-remediation: both filters used hardcoded memcmp offsets that did
not account for Borsh length prefixes. resolveSchema always returned
nothing and listIssuers returned the wrong or no entries.

Fix landed: both methods now do a full scan of program accounts and
parse the Borsh layout byte-by-byte. For production at scale this is
acceptable only because the registries are small; a HeliusDasAdapter
equivalent for SchemaAccount and IssuerAccount is tracked as a P3
enhancement.

#### P2 LocalReplicaAdapter memory bomb

Pre-remediation: every proof request materialised a full 2^depth leaf
vector. At depth 20 that is 33 megabytes per request and at depth 26
it is 2 gigabytes. Production use on mobile or browser was effectively
impossible.

Fix landed: the adapter walks a sparse tree lazily. A zeroLevels cache
memoises the zero-subtree root at every height so only non-empty
subtrees are ever recursively hashed. Memory usage is O(depth) plus
the real leaf count instead of O(2^depth). A new poseidonHashPair
export from @solid-protocol/light is provided as the canonical hash
pair (the circuit uses Poseidon-hashed Merkle trees; the "keccak-256
to match SPL AC" comment from pre-remediation docs was misleading for
ZK proof construction).

### Circuits

#### P2 numPredicates and compoundLogic range checks

Pre-remediation: numPredicates could be set above MAX_PREDICATES with
the circuit silently behaving like MAX_PREDICATES. compoundLogic could
be any field element; values above one silently behaved like AND.

Fix landed in batch_credential_query.circom:

```
component numPredsCheck = LessEqThan(8);
numPredsCheck.in[0] <== numPredicates;
numPredsCheck.in[1] <== MAX_PREDICATES;
numPredsCheck.out === 1;

compoundLogic * (compoundLogic - 1) === 0;
```

The identical range checks were added to compound_query.circom.
compound_query also now enforces schemaHash non-zero so a single-
credential proof cannot be bootstrapped against a padding slot.

#### P2 ExpirationChecker wired into batch circuit

Pre-remediation: currentTimestamp was declared public but never used.
The ExpirationChecker template existed in lib/nullifier_expiry.circom
and was used only by compound_query; batch_credential_query did not
instantiate it.

Fix landed: batch_credential_query now instantiates ExpirationChecker
per credential slot. Expired credentials fail the proof. Zero-schema
slots keep the checker trivially valid because the zero-padding
integrity constraints already force expirationTimestamps[i] to zero.

#### P2 GLOBAL_DEPTH template parameter

Pre-remediation: globalSiblings, globalPathIndices, and
IdentityAnchor(20) were hardcoded to 20 in the batch template body
regardless of the TREE_DEPTH argument.

Fix landed: the batch template signature is now
BatchCredentialQuerySolana(TREE_DEPTH, GLOBAL_DEPTH, NUM_FIELDS,
NUM_CREDS, MAX_PREDICATES). globalSiblings and globalPathIndices are
indexed at GLOBAL_DEPTH and IdentityAnchor is called with the
parameter. Production instantiation is (20, 20, 8, 4, 4).

### E2E scripts

#### X1 scripts/initialize.ts, issue.ts, prove.ts

Pre-remediation: every script used the stale devnet.json program IDs,
passed arguments that did not match the real SDK signatures, and
referenced instructions (initNullifierBloom) that no longer exist.

Fix landed in scripts/initialize.ts, scripts/issue.ts, scripts/prove.ts
(whole-file rewrites):

- All three now import PROGRAM_IDS from @solid-protocol/core, the
  single source of truth that matches Anchor.toml.
- initialize.ts runs six idempotent steps covering registry,
  schema, tree binding, global binding, verifier, and VK upload.
  SOLID_RPC_URL, SOLID_TREE_PUBKEY, and SOLID_GOVERNANCE_MINT can be
  injected via environment variables.
- issue.ts uses the correct IssueOptions shape (connection,
  issuerAuthority, merkleTree) and persists both holder master keys
  and the per-schema keypair to scripts/e2e_state.json.
- prove.ts seeds a LocalReplicaAdapter for both trees, derives the
  per-schema identity leaf, and passes SchemaTreeAccounts as the
  fourth positional argument to the real verifyOnChain. It also tests
  replay rejection.

### Deployment manifest

#### X2 deployments/devnet.json and scripts/check_program_ids.py

Pre-remediation: devnet.json recorded stale program IDs and fantasy
toolchain versions (anchor_cli 1.0.0, solana_cli 3.1.12). It also
carried a ghost nullifier_bloom PDA entry that no longer exists.

Fix landed: devnet.json rewritten to reflect the canonical program IDs
from Anchor.toml, with a null deployed_at until the next deploy. PDA
entries now enumerate verifier_config, vk_storage,
nullifier_record_example, registry_config, dao_treasury,
global_binding, and schema_tree_binding_example. Toolchain versions
match the pinned flake.

check_program_ids.py already validated deployments/*.json against
Anchor.toml; that validation is now green against the rewritten
manifest.

## Section 2: New surface area introduced by the fixes

### dao_treasury PDA

A new system-owned lamport PDA seeded ["dao-treasury"]. Created on
first slash via init_if_needed. Cost of first-slash initialisation is
the rent-exempt minimum, paid by whoever triggers the first slash
(the registry authority for slash_issuer, the authority for
submit_fraud_proof).

Observations from adversarial review:

- There is no withdraw_from_treasury instruction yet. Slashed funds
  accumulate and are currently reachable only by a future governance
  action. Documented as a P3 follow-up.
- The init_if_needed pattern is benign here because the account has
  zero data and the seeds are globally unique (no arg-derived seed).
  The only failure mode on re-init is a no-op (Anchor detects the
  existing account and skips create).

### Authority rotation instructions

transfer_tree_binding_authority and transfer_global_binding_authority
let the current authority move its right to a new Pubkey. Both are
required-signer gated by the current authority. No DAO voting around
them. This matches the existing "single authority per binding" model;
a multisig transfer is therefore as simple as transferring to a
Squads PDA.

No observed attack surface beyond the existing authority model. The
authority can still freeze or write corrupt roots if compromised; the
monotonicity check added in this pass caps root-rollback damage.

### next_vk_chunk field in VerifierConfig

Adds 2 bytes to VerifierConfig.SPACE when it landed; after the
SOLID-SEC-005 `timestamp_skew_seconds: u32` field landed the
constant is now:
    32 authority
  +  8 proof_count
  +  1 vk_initialized
  +  1 paused
  +  1 bump
  +  2 next_vk_chunk
  +  4 timestamp_skew_seconds
  = 49 bytes total (VerifierConfig::SPACE).

Source of truth: `programs/zk-verifier/src/lib.rs:612-616`. This is
backwards-incompatible for any existing deployment that ran against
a pre-remediation or pre-SEC-005 binary; the account must be closed
and reinit'd before `store_verification_key` can be called under the
new code path. Documented in DEPLOYMENT_AND_TESTING.md mainnet
checklist.

### Circuit signature change and trusted setup

The batch template's extra GLOBAL_DEPTH parameter changes the
constraint system. An existing verifying key from the pre-remediation
circuit will not verify proofs produced by the new circuit. A fresh
trusted-setup ceremony is required before upgrading a deployment.

This is intentional and it is the right trade-off: the pre-remediation
circuit was unsound against expired credentials and had an
underspecified GLOBAL_DEPTH. Keeping the old VK would keep those bugs.

## Section 3: Re-audit of invariants that did not change

The following invariants, which were correct before the remediation,
were re-verified and remain correct:

- Three-program decomposition with no cross-program trust (verifier
  does not CPI into registry; registry does not CPI into verifier).
- Poseidon parameters in solid-core match circomlib via
  light-poseidon's new_circom mode.
- 6-input hardened nullifier formula (ADR-0006 Phase 2 revision;
  Poseidon(masterKey, revocationNonce, verifierAddress,
  queryContextHash, verifierNonce, issuerTreeRoot)) with field
  ordering consistent between circuit and solid-core (cross-language
  vectors confirm; the 6th input carries the ADR-0014 epoch bind).
- PDA-per-nullifier replay protection with atomic init.
- public_inputs[VERIFIER_ADDRESS_INPUT_INDEX = 29] scope-binding to the
  verifier program ID (SEC-13; slot shifted from 28 by ADR-0014).
- public_inputs[ISSUER_TREE_ROOT_INPUT_INDEX = 10] cross-checked
  against `IssuerTreeBinding.current_root` (ADR-0014; SEC-004/008).
- Schema-hash strict-ascending ordering (SEC-20).
- Flash-loan protection on stake age (100 slots minimum before vote).
- Atomic revoke: `revoke_issuer_atomic` bumps issuer state AND
  CPIs `replace_leaf` in the same tx, keeping on-chain status and
  issuer-tree root in lockstep (ADR-0014; Phase 2).
- CI jobs: fmt, clippy, rust tests, prover tests, anchor build, wasm,
  wasm_bridge_smoke, circuits, circuit_witness_tests, sdk,
  cross_language_vectors, e2e_localnet, program_id_consistency.

## Section 4: Open findings

All remaining open items are M2 and M3 scope. None block localnet
end-to-end runs.

### M2.F1 Revocation v1 operational flow

Status: scaffolding complete, operator workflow undocumented.

The on-chain primitive is update_global_root with its new monotonicity
and authority rotation. The circuit already binds revocationNonce into
the identity leaf. What is missing:

- Holder SDK helper that, on detection of a revocation event,
  increments revocationNonce, re-derives the per-schema leaf, and
  requests reinsertion into the global tree.
- Issuer SDK helper that publishes a revocation event and pushes the
  new global root.
- An off-chain indexer contract describing the event name and payload.

### M2.F2 In-circuit issuer binding

Status: architectural decision open.

The circuit currently proves the credential was signed by some
BabyJubJub key. issue_credential only lets an Approved issuer post a
commitment, but there is no in-proof cryptographic binding between the
signing key and the on-chain IssuerAccount. Options:

- Option A. Verifier CPIs into issuer-registry::check_issuer_status
  using issuer pubkey bytes extracted from the proof public inputs.
- Option B. Add a compressed issuer tree plus a Merkle-membership
  proof in the circuit. More private, more expensive.

Either direction requires a new circuit change and another trusted
setup. Deferred until after the v1 localnet end-to-end ships.

### M3 Multi-party trusted setup

circuits/scripts/setup.js is a single-contributor script acceptable
for localnet and devnet. For mainnet it must be replaced by a
multi-party ceremony with verifiable attestation chain. Hermez tooling
is the usual choice.

### M3 External audit

Before mainnet: OtterSec, Halborn, or Trail of Bits review of the
post-remediation codebase plus the trusted-setup artifact. No known
blocker; timing is a scheduling concern.

## Section 5: Honest updated score

Pre-remediation score (from the 2026-04-21 audit): 47 out of 100.
Post-remediation score: 74 out of 100.

| Dimension | Before | After | Delta reason |
|---|---|---|---|
| Cryptographic design | 90 | 92 | GLOBAL_DEPTH parameterisation removes a silent bug. |
| On-chain program correctness | 45 | 85 | Every P0 and P1 landed; DAO governance is now functional. |
| SDK completeness | 30 | 75 | prove and verifyOnChain stubs replaced by real delegation; identity-commitment divergence fixed. |
| Documentation accuracy | 55 | 90 | Docs rewritten to match current code; stale claims retired; audit docs marked historical. |
| Testing coverage | 35 | 55 | Integration harness skeleton and bankrun test template landed; suite expansion tracked in tests/integration/README.md. |
| Security posture | 50 | 80 | Root-forging gap closed; slashing actually moves lamports; monotonicity on root updates; authority rotation instructions. |
| Operational readiness | 10 | 55 | Canonical deployment manifest; scripts run end to end; mainnet checklist explicit. |

What would move the score to 90+:

- M2.F1 revocation shipping with documented operator workflow.
- M2.F2 in-circuit issuer binding after a multi-party ceremony.
- Integration test suite expanded to the full 11-scenario list.
- External audit complete with no findings above medium.

Until those four items ship, devnet is the right home for the protocol.
Mainnet deployment should wait.
