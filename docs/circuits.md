# Circuit Design

v0.3, April 2026. Post-remediation release.

SolID ships two Circom circuits. batch_credential_query.circom is the
production entry point used by all dApps; compound_query.circom is a
single-credential variant kept for tests and simpler use cases.

## Parameters

```
template BatchCredentialQuerySolana(
    TREE_DEPTH, GLOBAL_DEPTH, NUM_FIELDS, NUM_CREDS, MAX_PREDICATES
)

template CompoundQuerySolana(
    TREE_DEPTH, GLOBAL_DEPTH, NUM_FIELDS, MAX_PREDICATES
)
```

Production instantiation:

```
BatchCredentialQuerySolana(20, 20, 8, 4, 4)
CompoundQuerySolana(20, 20, 8, 4)
```

GLOBAL_DEPTH is now a first-class template parameter. Before the remediation
the batch template used the literal 20 throughout its body even when the
caller passed a different TREE_DEPTH. GLOBAL_DEPTH and TREE_DEPTH may diverge
in the future (the global identity tree and the per-schema credential trees
are architecturally independent).

## Public inputs, 31 total

```
index 0        nullifierHash (circuit output)
index 1        globalRoot
index 2..5     merkleRoots[NUM_CREDS]
index 6..9     schemaHashes[NUM_CREDS]
index 10..13   queryCredentialIndices[MAX_PREDICATES]
index 14..17   queryFieldIndices[MAX_PREDICATES]
index 18..21   queryOperators[MAX_PREDICATES]
index 22..25   queryValues[MAX_PREDICATES]
index 26       numPredicates
index 27       compoundLogic (0 AND, 1 OR)
index 28       verifierAddress (must equal zk-verifier program ID)
index 29       verifierNonce
index 30       currentTimestamp
```

The on-chain program declares NR_PUBLIC_INPUTS = 31 in
programs/zk-verifier/src/lib.rs and enforces the index meanings above.

## Invariants enforced in-circuit

Input range checks:

- numPredicates is constrained to be less than or equal to MAX_PREDICATES.
- compoundLogic is constrained to the set zero or one via the identity
  compoundLogic times (compoundLogic minus one) equals zero.

Identity anchoring (IdentityAnchor):

1. Derive the per-schema private key:
   credentialPrivKey equals Poseidon(masterIdentityKey, schemaHash).
2. Derive the per-schema public key:
   credentialPubKeyAx, credentialPubKeyAy equals BabyPbk(credentialPrivKey).
3. Compute the identity leaf:
   identityState equals Poseidon(credentialPubKeyAx,
   credentialPubKeyAy, revocationNonce).
4. Verify identityState belongs to globalRoot via MerkleInclusion.

Every credential in the batch runs its own IdentityAnchor, so the global
tree carries per-schema identity leaves, not one master-key leaf.

Canonical ordering and zero-schema integrity:

- schemaHashes must be strictly ascending for active slots.
- If schemaHashes[i] is zero then every per-credential private input for
  slot i must also be zero (merkleRoot, data, salt, issuer pubkey, issuer
  signature, expirationTimestamp).

Expiration:

- ExpirationChecker runs for every active credential slot.
  Pre-remediation the batch circuit declared currentTimestamp as a public
  input and never used it; expired credentials passed silently.

Credential atoms (CredentialAtom):

- Verifies the issuer EdDSA signature on the commitment.
- Verifies Merkle inclusion of the commitment under merkleRoot using
  Poseidon-hashed MerkleInclusion at TREE_DEPTH.
- Binds the holder's per-schema public key into the commitment preimage.

Compound predicates:

- Each predicate is evaluated by PredicateEvaluator with one-hot operator
  decoding.
- Inactive predicates short-circuit to 1 so they do not break AND; they
  contribute 0 to the OR sum.

Final logic:

- finalResult equals (1 minus compoundLogic) times andResult plus
  compoundLogic times orResult.
- finalResult is constrained to equal one.

Query-context binding:

```
qHasherIndices = Poseidon(queryCredentialIndices || queryFieldIndices)
qHasherOps     = Poseidon(queryOperators          || queryValues)
queryContextHash = Poseidon(qHasherIndices, qHasherOps,
                            numPredicates, compoundLogic)
```

Hardened nullifier:

```
nullifier = Poseidon(
    masterIdentityKey,
    revocationNonce,
    verifierAddress,
    queryContextHash,
    verifierNonce
)
```

The output is exposed as public input index 0.

## compound_query specifics

Beyond the shared invariants compound_query.circom adds:

- schemaHash must be non-zero. This is single-credential so there is no
  padding; a zero schema would mean there is nothing to prove.
- holderBJJPrivKey is a private input constrained to equal Poseidon of the
  master key and schema hash, and BabyPbk of holderBJJPrivKey must equal
  the IdentityAnchor-derived public key. This is a sanity check that the
  holder is holding the correct per-schema key.

## Unused legacy templates

NullifierComputer in lib/nullifier_expiry.circom is the old 3-argument
nullifier. It is retained for historical reference but is never instantiated.
signature_verifier.circom in lib/ is also unused (EdDSA verification happens
inline in CredentialAtom via EdDSAPoseidonVerifier).

## Artifacts

Running circuits/scripts/setup.js produces:

- circuits/build/batch_credential_query.r1cs (constraint system)
- circuits/build/batch_credential_query_js/batch_credential_query.wasm
  (witness generator)
- circuits/build/batch_credential_query_final.zkey (proving key)
- circuits/build/verification_key.json (verification key consumed by
  zk-verifier::store_verification_key)

The script is a single-party ceremony. For mainnet, replace it with a
multi-party Hermez-style ceremony and pin the result to IPFS plus Arweave
content addressing. Cross-referencing the random contribution attestation
is mandatory.

## Host-program contract

The on-chain verifier:

- Requires public_inputs[28] to equal the zk-verifier program ID.
- Loads the global Merkle root from a schema-registry-owned GlobalStateBinding.
- Loads each active credential's tree root from a schema-registry-owned
  SchemaTreeBinding.
- Rejects both types of account if the owner is not schema-registry, closing
  the forged-trust-root attack surface that the pre-remediation version
  allowed.
- Registers a PDA seeded by ["null", nullifierHash_bytes] via Anchor init,
  which atomically rejects replayed proofs.
