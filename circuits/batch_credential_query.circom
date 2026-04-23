pragma circom 2.1.0;

include "node_modules/circomlib/circuits/poseidon.circom";
include "node_modules/circomlib/circuits/comparators.circom";
include "node_modules/circomlib/circuits/babyjub.circom";
include "lib/identity_anchor.circom";
include "lib/credential_atom.circom";
include "lib/predicate_evaluator.circom";
include "lib/nullifier_expiry.circom";

/// ============================================================================
/// BatchCredentialQuerySolana (Phase 3.6)
/// ============================================================================
///
/// Proves "I hold a set of up to NUM_CREDS valid credentials that satisfy a
/// compound query" without revealing identity, attribute values, or which
/// credentials were inspected.
///
/// Public inputs (NR_PUBLIC_INPUTS = 31 total; index in brackets):
///   [0]      nullifierHash (circuit output)
///   [1]      globalRoot
///   [2..5]   merkleRoots[NUM_CREDS]
///   [6..9]   schemaHashes[NUM_CREDS]
///   [10..13] queryCredentialIndices[MAX_PREDICATES]
///   [14..17] queryFieldIndices[MAX_PREDICATES]
///   [18..21] queryOperators[MAX_PREDICATES]
///   [22..25] queryValues[MAX_PREDICATES]
///   [26]     numPredicates
///   [27]     compoundLogic (0=AND, 1=OR)
///   [28]     verifierAddress
///   [29]     verifierNonce
///   [30]     currentTimestamp
///
/// Parameters:
///   TREE_DEPTH     Depth of each per-schema SPL AC credential tree (default 20)
///   GLOBAL_DEPTH   Depth of the identity tree (default 20, may diverge)
///   NUM_FIELDS     Attributes per credential (default 8)
///   NUM_CREDS      Credentials per batch (default 4)
///   MAX_PREDICATES Query predicates per proof (default 4)
///
/// Hardening added in Phase 3.6 (relative to the 3.1 baseline):
///   1. GLOBAL_DEPTH is parameterised instead of hardcoded 20.
///   2. numPredicates is range-checked against MAX_PREDICATES.
///   3. compoundLogic is constrained to {0, 1}.
///   4. Expiration is enforced per active credential against currentTimestamp
///      (previously the public input was declared but never constrained,
///      meaning expired credentials passed the batch verifier silently).
template BatchCredentialQuerySolana(TREE_DEPTH, GLOBAL_DEPTH, NUM_FIELDS, NUM_CREDS, MAX_PREDICATES) {

    // --- Public Inputs ------------------------------------------------------
    signal input globalRoot;
    signal input merkleRoots[NUM_CREDS];
    signal input schemaHashes[NUM_CREDS];

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
    // STEP 5: Hardened Nullifier
    //   nullifier = Poseidon(masterKey, revocationNonce, verifierAddress,
    //                        queryContextHash, verifierNonce)
    // ========================================================================
    component nullifier = Poseidon(5);
    nullifier.inputs[0] <== masterIdentityKey;
    nullifier.inputs[1] <== revocationNonce;
    nullifier.inputs[2] <== verifierAddress;
    nullifier.inputs[3] <== queryContextHash;
    nullifier.inputs[4] <== verifierNonce;
    nullifierHash <== nullifier.out;
}

// --- Component Instantiation -------------------------------------------------
component main {public [
    globalRoot,
    merkleRoots,
    schemaHashes,
    queryCredentialIndices,
    queryFieldIndices,
    queryOperators,
    queryValues,
    numPredicates,
    compoundLogic,
    verifierAddress,
    verifierNonce,
    currentTimestamp
]} = BatchCredentialQuerySolana(20, 20, 8, 4, 4);
