pragma circom 2.1.0;

include "node_modules/circomlib/circuits/poseidon.circom";
include "node_modules/circomlib/circuits/comparators.circom";
include "lib/credential_hasher.circom";
include "lib/signature_verifier.circom";
include "lib/merkle_inclusion.circom";
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
///   - nullifierHash (output)
///
/// Private inputs:
///   - attestationData, salt, holderBJJPrivKey
///   - issuerSigR8x/R8y/S
///   - merkleSiblings, merklePathIndices
///   - expirationTimestamp
///
template CompoundQuerySolana(TREE_DEPTH, NUM_FIELDS, MAX_PREDICATES) {

    // ─── Public Inputs ────────────────────────────────────────────────
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
    signal input attestationData[NUM_FIELDS];
    signal input salt;
    signal input holderBJJPrivKey;
    signal input holderBJJPubKeyAx;
    signal input holderBJJPubKeyAy;
    signal input issuerSigR8x;
    signal input issuerSigR8y;
    signal input issuerSigS;
    signal input merkleSiblings[TREE_DEPTH];
    signal input merklePathIndices[TREE_DEPTH];
    signal input expirationTimestamp;

    // ─── Public Output ────────────────────────────────────────────────
    signal output nullifierHash;

    // ═════════════════════════════════════════════════════════════════
    // STEP 1: Hash attestation data
    // ═════════════════════════════════════════════════════════════════
    component dataHasher = CredentialHasher(NUM_FIELDS);
    for (var i = 0; i < NUM_FIELDS; i++) {
        dataHasher.data[i] <== attestationData[i];
    }

    // ═════════════════════════════════════════════════════════════════
    // STEP 2: Compute attestation commitment
    //   commitment = Poseidon(dataHash, schemaHash, holderPubX, holderPubY, salt)
    // ═════════════════════════════════════════════════════════════════
    component commitmentHasher = Poseidon(5);
    commitmentHasher.inputs[0] <== dataHasher.dataHash;
    commitmentHasher.inputs[1] <== schemaHash;
    commitmentHasher.inputs[2] <== holderBJJPubKeyAx;
    commitmentHasher.inputs[3] <== holderBJJPubKeyAy;
    commitmentHasher.inputs[4] <== salt;

    signal commitment <== commitmentHasher.out;

    // ═════════════════════════════════════════════════════════════════
    // STEP 3: Verify issuer EdDSA-Poseidon signature over commitment
    // ═════════════════════════════════════════════════════════════════
    component sigVerifier = SignatureVerifier();
    sigVerifier.enabled <== 1;
    sigVerifier.Ax <== issuerPubKeyAx;
    sigVerifier.Ay <== issuerPubKeyAy;
    sigVerifier.S <== issuerSigS;
    sigVerifier.R8x <== issuerSigR8x;
    sigVerifier.R8y <== issuerSigR8y;
    sigVerifier.M <== commitment;

    // ═════════════════════════════════════════════════════════════════
    // STEP 4: Verify Merkle inclusion
    // ═════════════════════════════════════════════════════════════════
    component merkle = MerkleInclusion(TREE_DEPTH);
    merkle.root <== merkleRoot;
    merkle.leaf <== commitment;
    for (var i = 0; i < TREE_DEPTH; i++) {
        merkle.siblings[i] <== merkleSiblings[i];
        merkle.pathIndices[i] <== merklePathIndices[i];
    }

    // ═════════════════════════════════════════════════════════════════
    // STEP 5: Evaluate compound predicates
    // ═════════════════════════════════════════════════════════════════
    component selectors[MAX_PREDICATES];
    component evaluators[MAX_PREDICATES];
    signal predicateResults[MAX_PREDICATES];

    // Check which predicates are active
    component activeCheck[MAX_PREDICATES];
    signal isActive[MAX_PREDICATES];

    for (var i = 0; i < MAX_PREDICATES; i++) {
        // Is this predicate slot active? (i < numPredicates)
        activeCheck[i] = LessThan(8);
        activeCheck[i].in[0] <== i;
        activeCheck[i].in[1] <== numPredicates;
        isActive[i] <== activeCheck[i].out;

        // Select field value by index
        selectors[i] = FieldSelector(NUM_FIELDS);
        for (var j = 0; j < NUM_FIELDS; j++) {
            selectors[i].data[j] <== attestationData[j];
        }
        selectors[i].index <== queryFieldIndices[i];

        // Evaluate predicate
        evaluators[i] = PredicateEvaluator();
        evaluators[i].fieldValue <== selectors[i].value;
        evaluators[i].operator <== queryOperators[i];
        evaluators[i].queryValue <== queryValues[i];

        // Active predicates use real result; inactive ones pass (=1)
        predicateResults[i] <== isActive[i] * evaluators[i].result + (1 - isActive[i]);
    }

    // ═════════════════════════════════════════════════════════════════
    // STEP 6: Compound logic (AND / OR)
    // ═════════════════════════════════════════════════════════════════

    // AND result: all must be 1
    // R1CS only allows one multiplication per constraint, so chain pairwise:
    signal and01 <== predicateResults[0] * predicateResults[1];
    signal and012 <== and01 * predicateResults[2];
    signal andResult <== and012 * predicateResults[3];

    // OR result: at least one must be 1
    signal orSum <== predicateResults[0] + predicateResults[1] +
                     predicateResults[2] + predicateResults[3];
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
    // STEP 8: Compute nullifier
    // ═════════════════════════════════════════════════════════════════
    component nullifier = NullifierComputer();
    nullifier.holderPrivKey <== holderBJJPrivKey;
    nullifier.schemaHash <== schemaHash;
    nullifier.verifierNonce <== verifierNonce;
    nullifierHash <== nullifier.nullifier;
}

// Instantiate with production parameters
component main {public [
    merkleRoot,
    schemaHash,
    issuerPubKeyAx,
    issuerPubKeyAy,
    queryFieldIndices,
    queryOperators,
    queryValues,
    numPredicates,
    compoundLogic,
    verifierNonce,
    currentTimestamp
]} = CompoundQuerySolana(20, 8, 4);
