pragma circom 2.1.0;

include "node_modules/circomlib/circuits/poseidon.circom";
include "node_modules/circomlib/circuits/comparators.circom";
include "node_modules/circomlib/circuits/babyjub.circom";
include "lib/identity_anchor.circom";
include "lib/credential_atom.circom";
include "lib/predicate_evaluator.circom";
include "lib/nullifier_expiry.circom";

/// ═══════════════════════════════════════════════════════════════════════════
/// SolID Compound Query Circuit
/// ═══════════════════════════════════════════════════════════════════════════
///
/// Proves: "I hold a valid credential matching a compound query, without
///          revealing the credential data."
///
/// Parameters:
///   TREE_DEPTH    = 20   (supports ~1M credentials)
///   NUM_FIELDS    = 8    (attestation data fields)
///   MAX_PREDICATES = 4   (compound query size)
///
/// Public inputs:
///   - merkleRoot, schemaHash, issuerPubKeyAx/Ay
///   - queryFieldIndices, queryOperators, queryValues
///   - numPredicates, compoundLogic
///   - verifierNonce, currentTimestamp
///
/// Private inputs:
///   - attestationData, salt, holderBJJPrivKey
///   - issuerSigR8x/R8y/S
///   - merkleSiblings, merklePathIndices
///   - expirationTimestamp
///
template CompoundQuerySolana(TREE_DEPTH, NUM_FIELDS, MAX_PREDICATES) {

    // ─── Public Inputs ────────────────────────────────────────────────
    signal input globalRoot;
    signal input merkleRoot;
    signal input schemaHash;
    signal input issuerPubKeyAx;
    signal input issuerPubKeyAy;
    signal input queryFieldIndices[MAX_PREDICATES];
    signal input queryOperators[MAX_PREDICATES];
    signal input queryValues[MAX_PREDICATES];
    signal input numPredicates;
    signal input compoundLogic; // 0 = AND, 1 = OR
    signal input verifierNonce;
    signal input currentTimestamp;

    // ─── Private Inputs ───────────────────────────────────────────────
    signal input masterIdentityKey;
    signal input revocationNonce;
    signal input globalSiblings[20];
    signal input globalPathIndices[20];
    
    signal input attestationData[NUM_FIELDS];
    signal input salt;
    signal input issuerSigR8x;
    signal input issuerSigR8y;
    signal input issuerSigS;
    signal input merkleSiblings[TREE_DEPTH];
    signal input merklePathIndices[TREE_DEPTH];
    signal input expirationTimestamp;
    signal input holderBJJPrivKey;

    // ─── Public Output ────────────────────────────────────────────────
    signal output nullifierHash;

    // ═════════════════════════════════════════════════════════════════
    // STEP 0: Identity Binding (Bind PrivKey to PubKey)
    //   Enforce that the provided PrivKey derives the provided PubKey
    // ═════════════════════════════════════════════════════════════════
    component anchor = IdentityAnchor(20);
    anchor.masterIdentityKey <== masterIdentityKey;
    anchor.revocationNonce <== revocationNonce;
    anchor.schemaHash <== schemaHash;
    anchor.globalRoot <== globalRoot;
    for (var i = 0; i < 20; i++) {
        anchor.globalSiblings[i] <== globalSiblings[i];
        anchor.globalPathIndices[i] <== globalPathIndices[i];
    }

    component holderKeyDerivation = BabyPbk();
    holderKeyDerivation.in <== holderBJJPrivKey;
    holderKeyDerivation.Ax === anchor.credentialPubKeyAx;
    holderKeyDerivation.Ay === anchor.credentialPubKeyAy;

    // ═════════════════════════════════════════════════════════════════
    // STEP 2: Credential Verification (Phase 3.1)
    // ═════════════════════════════════════════════════════════════════
    component credAtom = CredentialAtom(NUM_FIELDS, TREE_DEPTH);
    credAtom.schemaHash <== schemaHash;
    credAtom.merkleRoot <== merkleRoot;
    credAtom.issuerPubKeyAx <== issuerPubKeyAx;
    credAtom.issuerPubKeyAy <== issuerPubKeyAy;

    for (var i = 0; i < NUM_FIELDS; i++) {
        credAtom.attestationData[i] <== attestationData[i];
    }
    credAtom.salt <== salt;
    credAtom.holderBJJPubKeyAx <== anchor.credentialPubKeyAx;
    credAtom.holderBJJPubKeyAy <== anchor.credentialPubKeyAy;
    credAtom.issuerSigR8x <== issuerSigR8x;
    credAtom.issuerSigR8y <== issuerSigR8y;
    credAtom.issuerSigS <== issuerSigS;
    for (var i = 0; i < TREE_DEPTH; i++) {
        credAtom.merkleSiblings[i] <== merkleSiblings[i];
        credAtom.merklePathIndices[i] <== merklePathIndices[i];
    }

    // ═════════════════════════════════════════════════════════════════
    // STEP 3: Predicate Evaluation (Phase 3.1)
    // ═════════════════════════════════════════════════════════════════
    component evaluators[MAX_PREDICATES];
    signal predicateResults[MAX_PREDICATES];
    signal isActive[MAX_PREDICATES];

    for (var i = 0; i < MAX_PREDICATES; i++) {
        evaluators[i] = PredicateEvaluator();
        
        component selector = FieldSelector(NUM_FIELDS);
        for (var j = 0; j < NUM_FIELDS; j++) {
            selector.data[j] <== attestationData[j];
        }
        selector.index <== queryFieldIndices[i];

        evaluators[i].fieldValue <== selector.value;
        evaluators[i].operator <== queryOperators[i];
        evaluators[i].queryValue <== queryValues[i];

        component activeCheck = LessThan(8);
        activeCheck.in[0] <== i;
        activeCheck.in[1] <== numPredicates;
        isActive[i] <== activeCheck.out;
        
        // Evaluation for AND: Inactive predicates pass (=1)
        predicateResults[i] <== isActive[i] * evaluators[i].result + (1 - isActive[i]);
    }

    // ═════════════════════════════════════════════════════════════════
    // STEP 6: Compound logic (AND / OR)
    // ═════════════════════════════════════════════════════════════════

    // AND result: all active must be 1 (product of results)
    signal and01 <== predicateResults[0] * predicateResults[1];
    signal and012 <== and01 * predicateResults[2];
    signal andResult <== and012 * predicateResults[3];

    // OR result: at least one active must be 1 (sum of active results)
    signal activeResults[MAX_PREDICATES];
    for (var i = 0; i < MAX_PREDICATES; i++) {
        activeResults[i] <== isActive[i] * evaluators[i].result;
    }
    signal orSum <== activeResults[0] + activeResults[1] + activeResults[2] + activeResults[3];
    component orCheck = GreaterThan(8);
    orCheck.in[0] <== orSum;
    orCheck.in[1] <== 0;
    signal orResult <== orCheck.out;

    // Select based on compoundLogic: 0=AND, 1=OR
    component logicIsOr = IsEqual();
    logicIsOr.in[0] <== compoundLogic;
    logicIsOr.in[1] <== 1;

    // Mux: split into two intermediate products (one multiplication each)
    signal selectAnd <== (1 - logicIsOr.out) * andResult;
    signal selectOr <== logicIsOr.out * orResult;
    signal finalResult <== selectAnd + selectOr;

    // Constrain: proof only valid if query passes
    finalResult === 1;

    // ═════════════════════════════════════════════════════════════════
    // STEP 7: Expiration check
    // ═════════════════════════════════════════════════════════════════
    component expiryCheck = ExpirationChecker();
    expiryCheck.currentTimestamp <== currentTimestamp;
    expiryCheck.expirationTimestamp <== expirationTimestamp;
    expiryCheck.valid === 1;

    // ═════════════════════════════════════════════════════════════════
    // STEP 8: Compute nullifier (Phase 2.4 - Scope Binding)
    //   nullifier = Poseidon(masterIdentityKey, verifierAddress, schemaHash)
    // ═════════════════════════════════════════════════════════════════
    component nullifier = Poseidon(3);
    nullifier.inputs[0] <== masterIdentityKey;
    nullifier.inputs[1] <== verifierAddress;
    nullifier.inputs[2] <== schemaHash;
    nullifierHash <== nullifier.out;
}

// Instantiate with production parameters
component main {public [
    globalRoot,
    merkleRoot,
    schemaHash,
    issuerPubKeyAx,
    issuerPubKeyAy,
    queryFieldIndices,
    queryOperators,
    queryValues,
    numPredicates,
    compoundLogic,
    verifierAddress,
    verifierNonce,
    currentTimestamp
]} = CompoundQuerySolana(20, 8, 4);
