pragma circom 2.1.0;

include "node_modules/circomlib/circuits/poseidon.circom";
include "node_modules/circomlib/circuits/comparators.circom";
include "node_modules/circomlib/circuits/babyjub.circom";
include "lib/identity_anchor.circom";
include "lib/credential_atom.circom";
include "lib/predicate_evaluator.circom";
include "lib/nullifier_expiry.circom";
include "lib/merkle_inclusion.circom";   // ADR-0014 issuer-tree inclusion

/// ============================================================================
/// BatchCredentialQuerySolana (Phase 3.6)
/// ============================================================================
///
/// Proves "I hold a set of up to NUM_CREDS valid credentials that satisfy a
/// compound query" without revealing identity, attribute values, or which
/// credentials were inspected.
///
/// Public inputs (NR_PUBLIC_INPUTS = 32 total post ADR-0014; index in brackets):
///   [0]      nullifierHash (circuit output)
///   [1]      globalRoot
///   [2..5]   merkleRoots[NUM_CREDS]
///   [6..9]   schemaHashes[NUM_CREDS]
///   [10]     issuerTreeRoot            (ADR-0014; SEC-004 / SEC-008)
///   [11..14] queryCredentialIndices[MAX_PREDICATES]
///   [15..18] queryFieldIndices[MAX_PREDICATES]
///   [19..22] queryOperators[MAX_PREDICATES]
///   [23..26] queryValues[MAX_PREDICATES]
///   [27]     numPredicates
///   [28]     compoundLogic (0=AND, 1=OR)
///   [29]     verifierAddress
///   [30]     verifierNonce
///   [31]     currentTimestamp
///
/// Parameters:
///   TREE_DEPTH         Depth of each per-schema SPL AC credential tree (default 20)
///   GLOBAL_DEPTH       Depth of the identity tree (default 20, may diverge)
///   ISSUER_TREE_DEPTH  Depth of the singleton issuer tree (default 16 per ADR-0014)
///   NUM_FIELDS         Attributes per credential (default 8)
///   NUM_CREDS          Credentials per batch (default 4)
///   MAX_PREDICATES     Query predicates per proof (default 4)
///
/// Hardening added in Phase 3.6 (relative to the 3.1 baseline):
///   1. GLOBAL_DEPTH is parameterised instead of hardcoded 20.
///   2. numPredicates is range-checked against MAX_PREDICATES.
///   3. compoundLogic is constrained to {0, 1}.
///   4. Expiration is enforced per active credential against currentTimestamp
///      (previously the public input was declared but never constrained,
///      meaning expired credentials passed the batch verifier silently).
///
/// Hardening added in Phase 2 revision (ADR-0014; SEC-004 + SEC-008):
///   5. Every active credential must prove Merkle membership of an
///      *Approved* issuer leaf in `issuerTreeRoot`. The leaf binds the
///      BJJ signing key (issuerPubKeyAx/Ay), the Solana authority, and
///      a monotonic revocation_nonce together.  Revoking an issuer bumps
///      their leaf's revocation_nonce -> tree root changes -> every
///      pre-revocation proof's membership check fails AND the embedded
///      nullifier universe shifts (below).
///   6. Nullifier preimage extended to 6 Poseidon inputs with
///      `issuerTreeRoot` appended, binding every proof to a specific
///      issuer-tree epoch (SEC-008 epoch replay protection).
template BatchCredentialQuerySolana(TREE_DEPTH, GLOBAL_DEPTH, ISSUER_TREE_DEPTH, NUM_FIELDS, NUM_CREDS, MAX_PREDICATES) {

    // --- Public Inputs ------------------------------------------------------
    signal input globalRoot;
    signal input merkleRoots[NUM_CREDS];
    signal input schemaHashes[NUM_CREDS];
    signal input issuerTreeRoot;                   // ADR-0014 / SEC-004 / SEC-008

    // Query specification
    signal input queryCredentialIndices[MAX_PREDICATES];
    signal input queryFieldIndices[MAX_PREDICATES];
    signal input queryOperators[MAX_PREDICATES];
    signal input queryValues[MAX_PREDICATES];
    signal input numPredicates;
    signal input compoundLogic;

    signal input verifierAddress;
    signal input verifierNonce;
    signal input currentTimestamp;

    // --- Private Inputs -----------------------------------------------------
    signal input masterIdentityKey;
    signal input revocationNonce;

    // Per-credential global-tree membership (per-schema identity leaves).
    signal input globalSiblings[NUM_CREDS][GLOBAL_DEPTH];
    signal input globalPathIndices[NUM_CREDS][GLOBAL_DEPTH];

    // Batch data
    signal input data[NUM_CREDS][NUM_FIELDS];
    signal input salts[NUM_CREDS];
    signal input issuerSigR8xs[NUM_CREDS];
    signal input issuerSigR8ys[NUM_CREDS];
    signal input issuerSigSs[NUM_CREDS];
    signal input issuerPubKeyAxs[NUM_CREDS];
    signal input issuerPubKeyAys[NUM_CREDS];
    signal input merkleSiblings[NUM_CREDS][TREE_DEPTH];
    signal input merklePathIndices[NUM_CREDS][TREE_DEPTH];
    signal input expirationTimestamps[NUM_CREDS];

    // Per-credential issuer-tree membership (ADR-0014).
    //   `issuerAuthority` is the Solana authority Pubkey of the issuer,
    //   encoded as a big-endian field element (SEC-031).
    //   `issuerStatusEpoch` is the slot at which the issuer was flipped
    //   to Approved; changes on every status transition.
    //   `issuerRevocationNonce` is a monotonic counter bumped on every
    //   revoke_issuer / re-approval (distinct from `revocationNonce`
    //   above, which is the *holder*'s nonce).
    //   Leaf =
    //     Poseidon5(issuerAuthority, issuerPubKeyAx, issuerPubKeyAy,
    //               issuerStatusEpoch, issuerRevocationNonce).
    signal input issuerAuthorities[NUM_CREDS];
    signal input issuerStatusEpochs[NUM_CREDS];
    signal input issuerRevocationNonces[NUM_CREDS];
    signal input issuerSiblings[NUM_CREDS][ISSUER_TREE_DEPTH];
    signal input issuerPathIndices[NUM_CREDS][ISSUER_TREE_DEPTH];

    // --- Public Output ------------------------------------------------------
    signal output nullifierHash;

    // --- Input Range Checks -------------------------------------------------
    // Bound numPredicates so that a prover cannot claim more predicates than
    // the template supports. Without this, submitting numPredicates > MAX
    // silently behaves like numPredicates == MAX (activeCheck caps via
    // iteration) but the semantics are undefined.
    component numPredsCheck = LessEqThan(8);
    numPredsCheck.in[0] <== numPredicates;
    numPredsCheck.in[1] <== MAX_PREDICATES;
    numPredsCheck.out === 1;

    // compoundLogic must be exactly 0 (AND) or 1 (OR). Without this, any
    // value greater than 1 silently behaves like AND.
    compoundLogic * (compoundLogic - 1) === 0;

    // SOLID-SEC-001: bound every `queryCredentialIndices[i]` into
    // `[0, NUM_CREDS)` and every `queryFieldIndices[i]` into
    // `[0, NUM_FIELDS)`.  Without these, `BatchFieldSelector` silently
    // accepts out-of-range indices, which allows a prover to "point"
    // a predicate at a credential or field that never enters the
    // integrity / expiration / inclusion chain.  Combined with
    // `compoundLogic == OR`, that silently admits unconstrained
    // predicates.  `LessThan(8)` is sufficient: both bounds are
    // <= 64 in every current instantiation.
    component credIdxChecks[MAX_PREDICATES];
    component fieldIdxChecks[MAX_PREDICATES];
    for (var i = 0; i < MAX_PREDICATES; i++) {
        credIdxChecks[i] = LessThan(8);
        credIdxChecks[i].in[0] <== queryCredentialIndices[i];
        credIdxChecks[i].in[1] <== NUM_CREDS;
        credIdxChecks[i].out === 1;

        fieldIdxChecks[i] = LessThan(8);
        fieldIdxChecks[i].in[0] <== queryFieldIndices[i];
        fieldIdxChecks[i].in[1] <== NUM_FIELDS;
        fieldIdxChecks[i].out === 1;
    }

    // ========================================================================
    // STEP 0: Canonical Ordering and Zero-Schema Integrity
    //   - schemaHashes strictly ascending for active credentials.
    //   - schemaHash == 0 forces every per-credential private input to zero,
    //     preventing a prover from smuggling data through inactive slots.
    //
    //   Moved above STEP 0.5 (identity anchors) for SOLID-SEC-029 so
    //   `isZero[i].out` is in scope when we wire `anchors[i].enabled`.
    // ========================================================================
    component isZero[NUM_CREDS];
    component ordering[NUM_CREDS - 1];
    for (var i = 0; i < NUM_CREDS; i++) {
        isZero[i] = IsZero();
        isZero[i].in <== schemaHashes[i];

        isZero[i].out * merkleRoots[i] === 0;
        for (var j = 0; j < NUM_FIELDS; j++) {
            isZero[i].out * data[i][j] === 0;
        }
        isZero[i].out * salts[i] === 0;
        isZero[i].out * issuerPubKeyAxs[i] === 0;
        isZero[i].out * issuerPubKeyAys[i] === 0;
        isZero[i].out * issuerSigR8xs[i] === 0;
        isZero[i].out * issuerSigR8ys[i] === 0;
        isZero[i].out * issuerSigSs[i] === 0;
        isZero[i].out * expirationTimestamps[i] === 0;

        // ADR-0014: zero-constrain the new per-credential issuer fields
        // for padding slots.  Without these, a prover could smuggle
        // arbitrary authority / epoch / nonce values into inactive
        // slots.  Merkle-inclusion for padding slots is skipped below
        // via the `enabled` flag; these integrity constraints prevent
        // any non-inclusion side-channel.
        isZero[i].out * issuerAuthorities[i] === 0;
        isZero[i].out * issuerStatusEpochs[i] === 0;
        isZero[i].out * issuerRevocationNonces[i] === 0;
    }

    for (var i = 0; i < NUM_CREDS - 1; i++) {
        ordering[i] = LessThan(252);
        ordering[i].in[0] <== schemaHashes[i];
        ordering[i].in[1] <== schemaHashes[i+1];

        signal nextNotZero;
        nextNotZero <== 1 - isZero[i+1].out;
        nextNotZero * (1 - ordering[i].out) === 0;
    }

    // ========================================================================
    // STEP 0.5: Identity Binding
    //   Each credential uses its own schema-derived key. The batch circuit
    //   needs one global-tree inclusion proof per credential because every
    //   schema has a distinct identity leaf
    //   Poseidon(derivedAx_i, derivedAy_i, revocationNonce).
    //
    //   SOLID-SEC-029: pass `enabled = 1 - isZero[i].out` so that padding
    //   slots skip the global-tree inclusion check (the STEP-0 integrity
    //   constraints already zero out every per-credential signal in
    //   inactive slots, so they cannot be used to smuggle state).
    // ========================================================================
    component anchors[NUM_CREDS];
    for (var i = 0; i < NUM_CREDS; i++) {
        anchors[i] = IdentityAnchor(GLOBAL_DEPTH);
        anchors[i].enabled <== 1 - isZero[i].out;
        anchors[i].masterIdentityKey <== masterIdentityKey;
        anchors[i].revocationNonce <== revocationNonce;
        anchors[i].schemaHash <== schemaHashes[i];
        anchors[i].globalRoot <== globalRoot;
        for (var j = 0; j < GLOBAL_DEPTH; j++) {
            anchors[i].globalSiblings[j] <== globalSiblings[i][j];
            anchors[i].globalPathIndices[j] <== globalPathIndices[i][j];
        }
    }

    // ========================================================================
    // STEP 0.75: Issuer-tree Membership (ADR-0014; SEC-004)
    //
    //   For every ACTIVE slot, prove that an issuer leaf exists in the
    //   singleton issuer tree whose root is the public input
    //   `issuerTreeRoot`.  The leaf binds, in Poseidon(5) form:
    //     issuer_leaf = Poseidon(
    //         issuerAuthority,       // Solana authority pubkey (BE field)
    //         issuerPubKeyAx,        // BJJ x-coord (already in-circuit)
    //         issuerPubKeyAy,        // BJJ y-coord (already in-circuit)
    //         issuerStatusEpoch,     // monotonic; bumps on status txn
    //         issuerRevocationNonce, // monotonic; bumps on revoke/re-approve
    //     )
    //
    //   The BJJ key in the leaf is the LOAD-BEARING bit (see ADR-0014
    //   "leaf composition"): without it, a prover could supply an
    //   approved authority + an attacker-chosen BJJ key pair, and the
    //   signature-verify in the credential atom would succeed against
    //   that forged BJJ key.  Embedding (Ax, Ay) in the leaf ties the
    //   on-chain issuer state to the in-circuit signing key.
    //
    //   Padding slots skip the inclusion proof via `enabled = 0`.  The
    //   integrity constraints in STEP 0 already zero-constrain
    //   issuerAuthorities/issuerStatusEpochs/issuerRevocationNonces for
    //   padding slots, so the skipped check cannot be used to smuggle
    //   state.
    // ========================================================================
    component issuerLeafHasher[NUM_CREDS];
    component issuerInclusion[NUM_CREDS];
    for (var i = 0; i < NUM_CREDS; i++) {
        issuerLeafHasher[i] = Poseidon(5);
        issuerLeafHasher[i].inputs[0] <== issuerAuthorities[i];
        issuerLeafHasher[i].inputs[1] <== issuerPubKeyAxs[i];
        issuerLeafHasher[i].inputs[2] <== issuerPubKeyAys[i];
        issuerLeafHasher[i].inputs[3] <== issuerStatusEpochs[i];
        issuerLeafHasher[i].inputs[4] <== issuerRevocationNonces[i];

        issuerInclusion[i] = MerkleInclusion(ISSUER_TREE_DEPTH);
        issuerInclusion[i].enabled <== 1 - isZero[i].out;
        issuerInclusion[i].leaf    <== issuerLeafHasher[i].out;
        issuerInclusion[i].root    <== issuerTreeRoot;
        for (var j = 0; j < ISSUER_TREE_DEPTH; j++) {
            issuerInclusion[i].siblings[j]    <== issuerSiblings[i][j];
            issuerInclusion[i].pathIndices[j] <== issuerPathIndices[i][j];
        }
    }

    // ========================================================================
    // STEP 1: Credential Atoms
    // ========================================================================
    component atoms[NUM_CREDS];
    for (var i = 0; i < NUM_CREDS; i++) {
        atoms[i] = CredentialAtom(NUM_FIELDS, TREE_DEPTH);
        atoms[i].schemaHash <== schemaHashes[i];
        atoms[i].merkleRoot <== merkleRoots[i];
        atoms[i].issuerPubKeyAx <== issuerPubKeyAxs[i];
        atoms[i].issuerPubKeyAy <== issuerPubKeyAys[i];

        for (var j = 0; j < NUM_FIELDS; j++) {
            atoms[i].attestationData[j] <== data[i][j];
        }
        atoms[i].salt <== salts[i];
        atoms[i].holderBJJPubKeyAx <== anchors[i].credentialPubKeyAx;
        atoms[i].holderBJJPubKeyAy <== anchors[i].credentialPubKeyAy;
        atoms[i].issuerSigR8x <== issuerSigR8xs[i];
        atoms[i].issuerSigR8y <== issuerSigR8ys[i];
        atoms[i].issuerSigS <== issuerSigSs[i];
        for (var j = 0; j < TREE_DEPTH; j++) {
            atoms[i].merkleSiblings[j] <== merkleSiblings[i][j];
            atoms[i].merklePathIndices[j] <== merklePathIndices[i][j];
        }
    }

    // ========================================================================
    // STEP 1.5: Expiration (per active credential)
    //   ExpirationChecker returns 1 iff expirationTimestamp == 0 or
    //   currentTimestamp <= expirationTimestamp. Zero slots are inactive and
    //   expiration is already forced to zero by the integrity constraints
    //   above, so the checker trivially returns 1 for them.
    // ========================================================================
    component expiry[NUM_CREDS];
    for (var i = 0; i < NUM_CREDS; i++) {
        expiry[i] = ExpirationChecker();
        expiry[i].currentTimestamp <== currentTimestamp;
        expiry[i].expirationTimestamp <== expirationTimestamps[i];
        expiry[i].valid === 1;
    }

    // ========================================================================
    // STEP 2: Batch Predicate Evaluation
    // ========================================================================
    component selectors[MAX_PREDICATES];
    component evaluators[MAX_PREDICATES];
    signal predicateResults[MAX_PREDICATES];
    signal isActive[MAX_PREDICATES];

    component activeChecks[MAX_PREDICATES];
    for (var i = 0; i < MAX_PREDICATES; i++) {
        selectors[i] = BatchFieldSelector(NUM_CREDS, NUM_FIELDS);
        for (var n = 0; n < NUM_CREDS; n++) {
            for (var f = 0; f < NUM_FIELDS; f++) {
                selectors[i].data[n][f] <== data[n][f];
            }
        }
        selectors[i].credIndex <== queryCredentialIndices[i];
        selectors[i].fieldIndex <== queryFieldIndices[i];

        evaluators[i] = PredicateEvaluator();
        evaluators[i].fieldValue <== selectors[i].value;
        evaluators[i].operator <== queryOperators[i];
        evaluators[i].queryValue <== queryValues[i];

        activeChecks[i] = LessThan(8);
        activeChecks[i].in[0] <== i;
        activeChecks[i].in[1] <== numPredicates;
        isActive[i] <== activeChecks[i].out;

        predicateResults[i] <== isActive[i] * evaluators[i].result + (1 - isActive[i]);
    }

    // ========================================================================
    // STEP 3: AND / OR Logic
    // ========================================================================
    signal and01;
    signal and012;
    signal andResult;
    and01 <== predicateResults[0] * predicateResults[1];
    and012 <== and01 * predicateResults[2];
    andResult <== and012 * predicateResults[3];

    signal orSum;
    orSum <== (isActive[0] * evaluators[0].result) +
              (isActive[1] * evaluators[1].result) +
              (isActive[2] * evaluators[2].result) +
              (isActive[3] * evaluators[3].result);
    component orCheck = GreaterThan(8);
    orCheck.in[0] <== orSum;
    orCheck.in[1] <== 0;
    signal orResult;
    orResult <== orCheck.out;

    signal finalResult;
    finalResult <== (1 - compoundLogic) * andResult + compoundLogic * orResult;
    finalResult === 1;

    // ========================================================================
    // STEP 4: Query Context Hashing
    // ========================================================================
    component qHasherIndices = Poseidon(MAX_PREDICATES * 2);
    for (var i = 0; i < MAX_PREDICATES; i++) {
        qHasherIndices.inputs[i*2] <== queryCredentialIndices[i];
        qHasherIndices.inputs[i*2+1] <== queryFieldIndices[i];
    }

    component qHasherOps = Poseidon(MAX_PREDICATES * 2);
    for (var i = 0; i < MAX_PREDICATES; i++) {
        qHasherOps.inputs[i*2] <== queryOperators[i];
        qHasherOps.inputs[i*2+1] <== queryValues[i];
    }

    component qHasherFinal = Poseidon(4);
    qHasherFinal.inputs[0] <== qHasherIndices.out;
    qHasherFinal.inputs[1] <== qHasherOps.out;
    qHasherFinal.inputs[2] <== numPredicates;
    qHasherFinal.inputs[3] <== compoundLogic;

    signal queryContextHash;
    queryContextHash <== qHasherFinal.out;

    // ========================================================================
    // STEP 5: Hardened Nullifier (ADR-0006 + ADR-0014 revision; SEC-008)
    //   nullifier = Poseidon(masterKey,
    //                        revocationNonce,       // holder's
    //                        verifierAddress,
    //                        queryContextHash,
    //                        verifierNonce,
    //                        issuerTreeRoot)        // NEW: epoch bind
    //
    //   Binding the nullifier to `issuerTreeRoot` is what closes the
    //   post-revocation replay window (SOLID-SEC-008).  When any
    //   issuer is revoked, the tree root changes -> the nullifier a
    //   new proof computes is drawn from a different universe than
    //   the one a pre-revocation cached proof used -> on-chain
    //   nullifier-PDA collision CANNOT happen between epochs, and
    //   the membership check above ALSO rejects the pre-revocation
    //   proof because its siblings no longer recompute to the new
    //   root.  Both directions of the epoch boundary are closed.
    // ========================================================================
    component nullifier = Poseidon(6);
    nullifier.inputs[0] <== masterIdentityKey;
    nullifier.inputs[1] <== revocationNonce;
    nullifier.inputs[2] <== verifierAddress;
    nullifier.inputs[3] <== queryContextHash;
    nullifier.inputs[4] <== verifierNonce;
    nullifier.inputs[5] <== issuerTreeRoot;
    nullifierHash <== nullifier.out;
}

// --- Component Instantiation -------------------------------------------------
component main {public [
    globalRoot,
    merkleRoots,
    schemaHashes,
    issuerTreeRoot,
    queryCredentialIndices,
    queryFieldIndices,
    queryOperators,
    queryValues,
    numPredicates,
    compoundLogic,
    verifierAddress,
    verifierNonce,
    currentTimestamp
]} = BatchCredentialQuerySolana(20, 20, 16, 8, 4, 4);
