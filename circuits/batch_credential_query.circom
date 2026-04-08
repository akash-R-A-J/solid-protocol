pragma circom 2.1.0;

include "node_modules/circomlib/circuits/poseidon.circom";
include "node_modules/circomlib/circuits/comparators.circom";
include "node_modules/circomlib/circuits/babyjub.circom";
include "lib/identity_anchor.circom";
include "lib/credential_atom.circom";
include "lib/predicate_evaluator.circom";
include "lib/nullifier_expiry.circom";

/// ═══════════════════════════════════════════════════════════════════════════
/// BatchCredentialQuerySolana (Phase 3.1)
/// ═══════════════════════════════════════════════════════════════════════════
///
/// Proves: "I hold a set of 4 valid credentials that satisfy a compound query."
///
/// Parameters:
///   TREE_DEPTH    = 20
///   NUM_FIELDS    = 8
///   NUM_CREDS     = 4   (The architectural baseline)
///   MAX_PREDICATES = 4
///
template BatchCredentialQuerySolana(TREE_DEPTH, NUM_FIELDS, NUM_CREDS, MAX_PREDICATES) {

    // ─── Public Inputs ────────────────────────────────────────────────
    signal input globalRoot;
    signal input merkleRoots[NUM_CREDS];
    signal input schemaHashes[NUM_CREDS];
    
    // Query specification
    signal input queryCredentialIndices[MAX_PREDICATES];
    signal input queryFieldIndices[MAX_PREDICATES];
    signal input queryOperators[MAX_PREDICATES];
    signal input queryValues[MAX_PREDICATES];
    signal input numPredicates;
    signal input compoundLogic; // 0=AND, 1=OR
    
    signal input verifierAddress;
    signal input verifierNonce;
    signal input currentTimestamp;

    // ─── Private Inputs ───────────────────────────────────────────────
    // SEC-17: Master identity key shared across all credentials.
    signal input masterIdentityKey;
    signal input revocationNonce;
    
    // Phase 3.1: Global siblings for EACH credential to maintain unlinkability
    signal input globalSiblings[NUM_CREDS][20];
    signal input globalPathIndices[NUM_CREDS][20];
    
    // Batch data (N=4)
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
    
    // ─── Public Output ────────────────────────────────────────────────
    signal output nullifierHash;

    // ═════════════════════════════════════════════════════════════════
    // STEP 0: Identity Binding (Holder-Centric Alignment)
    //   Each credential in the batch uses its own schema-derived key.
    // ═════════════════════════════════════════════════════════════════
    component anchors[NUM_CREDS];
    for (var i = 0; i < NUM_CREDS; i++) {
        anchors[i] = IdentityAnchor(20);
        anchors[i].masterIdentityKey <== masterIdentityKey;
        anchors[i].revocationNonce <== revocationNonce;
        anchors[i].schemaHash <== schemaHashes[i];
        anchors[i].globalRoot <== globalRoot;
        for (var j = 0; j < 20; j++) {
            anchors[i].globalSiblings[j] <== globalSiblings[i][j];
            anchors[i].globalPathIndices[j] <== globalPathIndices[i][j];
        }
    }

    // ═════════════════════════════════════════════════════════════════
    // STEP 0.5: Canonical Ordering & Zero-Schema Integrity (Phase 3.5)
    //   1. Enforce schemaHashes[i] < schemaHashes[i+1] for active creds.
    //   2. Enforce schemaHash == 0 => data/root == 0 (Padding Integrity).
    // ═════════════════════════════════════════════════════════════════
    component isZero[NUM_CREDS];
    component ordering[NUM_CREDS - 1];
    for (var i = 0; i < NUM_CREDS; i++) {
        isZero[i] = IsZero();
        isZero[i].in <== schemaHashes[i];
        
        // Integrity: If schema is 0, merkleRoot must be 0
        isZero[i].out * merkleRoots[i] === 0;
        
        // Integrity: If schema is 0, all data fields must be 0
        for (var j = 0; j < NUM_FIELDS; j++) {
            isZero[i].out * data[i][j] === 0;
        }
        
        // Integrity: If schema is 0, salt must be 0
        isZero[i].out * salts[i] === 0;

        // Integrity: If schema is 0, issuer public key must be 0
        isZero[i].out * issuerPubKeyAxs[i] === 0;
        isZero[i].out * issuerPubKeyAys[i] === 0;

        // Integrity: If schema is 0, signatures must be 0
        isZero[i].out * issuerSigR8xs[i] === 0;
        isZero[i].out * issuerSigR8ys[i] === 0;
        isZero[i].out * issuerSigSs[i] === 0;

        // Integrity: If schema is 0, expiration must be 0 (prevent time-malleability)
        isZero[i].out * expirationTimestamps[i] === 0;
    }
    
    for (var i = 0; i < NUM_CREDS - 1; i++) {
        ordering[i] = LessThan(252);
        ordering[i].in[0] <== schemaHashes[i];
        ordering[i].in[1] <== schemaHashes[i+1];
        
        // If next is not zero, then current < next must be true
        signal nextNotZero <== 1 - isZero[i+1].out;
        nextNotZero * (1 - ordering[i].out) === 0;
    }

    // ═════════════════════════════════════════════════════════════════
    // STEP 1: Verify all Credential Atoms (N=4)
    // ═════════════════════════════════════════════════════════════════
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

    // ═════════════════════════════════════════════════════════════════
    // STEP 2: Batch Predicate Evaluation
    // ═════════════════════════════════════════════════════════════════
    component selectors[MAX_PREDICATES];
    component evaluators[MAX_PREDICATES];
    signal predicateResults[MAX_PREDICATES];
    signal isActive[MAX_PREDICATES];

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

        component activeCheck = LessThan(8);
        activeCheck.in[0] <== i;
        activeCheck.in[1] <== numPredicates;
        isActive[i] <== activeCheck.out;
        
        predicateResults[i] <== isActive[i] * evaluators[i].result + (1 - isActive[i]);
    }

    // ═════════════════════════════════════════════════════════════════
    // STEP 3: AND/OR Logic
    // ═════════════════════════════════════════════════════════════════
    signal and01 <== predicateResults[0] * predicateResults[1];
    signal and012 <== and01 * predicateResults[2];
    signal andResult <== and012 * predicateResults[3];

    signal orSum <== (isActive[0] * evaluators[0].result) + 
                    (isActive[1] * evaluators[1].result) + 
                    (isActive[2] * evaluators[2].result) + 
                    (isActive[3] * evaluators[3].result);
    component orCheck = GreaterThan(8);
    orCheck.in[0] <== orSum;
    orCheck.in[1] <== 0;
    signal orResult <== orCheck.out * 1;

    component logicIsOr = IsEqual();
    logicIsOr.in[0] <== compoundLogic;
    logicIsOr.in[1] <== 1;

    signal finalResult <== (1 - logicIsOr.out) * andResult + logicIsOr.out * orResult;
    finalResult === 1;

    // ═════════════════════════════════════════════════════════════════
    // STEP 4: Query Context Hashing (SEC-13)
    //   Prevents "Query Malleability" by binding the proof to specific predicates.
    // ═════════════════════════════════════════════════════════════════
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
    
    signal queryContextHash <== qHasherFinal.out;

    // ═════════════════════════════════════════════════════════════════
    // STEP 5: Hardened Nullifier (Anti-Replay & Identity Rotation)
    // ═════════════════════════════════════════════════════════════════
    // nullifier = Poseidon(masterKey, revocationNonce, verifierAddress, queryContextHash, verifierNonce)
    component nullifier = Poseidon(5);
    nullifier.inputs[0] <== masterIdentityKey;
    nullifier.inputs[1] <== revocationNonce;
    nullifier.inputs[2] <== verifierAddress;
    nullifier.inputs[3] <== queryContextHash;
    nullifier.inputs[4] <== verifierNonce;
    nullifierHash <== nullifier.out;
}

// ─── Component Instantiation ──────────────────────────────────────
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
]} = BatchCredentialQuerySolana(20, 8, 4, 4);
