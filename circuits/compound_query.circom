pragma circom 2.1.0;

include "node_modules/circomlib/circuits/poseidon.circom";
include "node_modules/circomlib/circuits/comparators.circom";
include "node_modules/circomlib/circuits/babyjub.circom";
include "lib/identity_anchor.circom";
include "lib/credential_atom.circom";
include "lib/predicate_evaluator.circom";
include "lib/nullifier_expiry.circom";
include "lib/merkle_inclusion.circom";   // ADR-0014 issuer-tree inclusion

/// ═══════════════════════════════════════════════════════════════════════════
/// SolID Compound Query Circuit (Hardened Phase 3.5 + ADR-0014 revision)
/// ═══════════════════════════════════════════════════════════════════════════
///
/// Proves: "I hold a valid credential matching a compound query."
///
/// Kept in lock-step with `batch_credential_query.circom` for the
/// ADR-0014 revision: the nullifier is 6-Poseidon and the issuer-tree
/// inclusion proof is mandatory.  Single-credential variant; the
/// BATCH variant is the one `scripts/prove.ts` exercises.
///
template CompoundQuerySolana(TREE_DEPTH, GLOBAL_DEPTH, ISSUER_TREE_DEPTH, NUM_FIELDS, MAX_PREDICATES) {

    // --- Public Inputs ------------------------------------------------
    signal input globalRoot;
    signal input merkleRoot;
    signal input schemaHash;
    signal input issuerPubKeyAx;
    signal input issuerPubKeyAy;
    signal input issuerTreeRoot;              // ADR-0014 / SEC-004 / SEC-008
    signal input queryFieldIndices[MAX_PREDICATES];
    signal input queryOperators[MAX_PREDICATES];
    signal input queryValues[MAX_PREDICATES];
    signal input numPredicates;
    signal input compoundLogic;
    signal input verifierAddress;
    signal input verifierNonce;
    signal input currentTimestamp;

    // --- Private Inputs -----------------------------------------------
    signal input masterIdentityKey;
    signal input revocationNonce;
    signal input globalSiblings[GLOBAL_DEPTH];
    signal input globalPathIndices[GLOBAL_DEPTH];

    signal input attestationData[NUM_FIELDS];
    signal input salt;
    signal input issuerSigR8x;
    signal input issuerSigR8y;
    signal input issuerSigS;
    signal input merkleSiblings[TREE_DEPTH];
    signal input merklePathIndices[TREE_DEPTH];
    signal input expirationTimestamp;
    signal input holderBJJPrivKey;

    // ADR-0014 issuer-tree leaf components + membership proof.
    signal input issuerAuthority;
    signal input issuerStatusEpoch;
    signal input issuerRevocationNonce;
    signal input issuerSiblings[ISSUER_TREE_DEPTH];
    signal input issuerPathIndices[ISSUER_TREE_DEPTH];

    // --- Public Output ------------------------------------------------
    signal output nullifierHash;

    // --- Input Range Checks -------------------------------------------
    // Bound numPredicates and compoundLogic exactly as in the batch circuit.
    // A single-credential proof still needs these because the predicate
    // evaluation loop and the AND/OR mux share the same underspecification.
    component numPredsCheck = LessEqThan(8);
    numPredsCheck.in[0] <== numPredicates;
    numPredsCheck.in[1] <== MAX_PREDICATES;
    numPredsCheck.out === 1;

    compoundLogic * (compoundLogic - 1) === 0;

    // SOLID-SEC-001: bound every `queryFieldIndices[i]` into
    // `[0, NUM_FIELDS)`. Without this, `FieldSelector` silently
    // accepts out-of-range indices, which lets the prover point a
    // predicate at nothing and trivially satisfy it (particularly
    // under OR).
    component fieldIdxChecks[MAX_PREDICATES];
    for (var i = 0; i < MAX_PREDICATES; i++) {
        fieldIdxChecks[i] = LessThan(8);
        fieldIdxChecks[i].in[0] <== queryFieldIndices[i];
        fieldIdxChecks[i].in[1] <== NUM_FIELDS;
        fieldIdxChecks[i].out === 1;
    }

    // Schema must be non-zero. The batch circuit tolerates zero-schema
    // padding because it is multi-credential; compound_query is single-credential
    // and a zero schema here would mean there is nothing to prove.
    component schemaIsZero = IsZero();
    schemaIsZero.in <== schemaHash;
    schemaIsZero.out === 0;

    // ================================================================
    // STEP 0: Identity Binding (Bind PrivKey to PubKey)
    // ================================================================
    //   SOLID-SEC-029 compat: compound_query is single-credential;
    //   schema is constrained non-zero above, so the anchor is
    //   always active.
    component anchor = IdentityAnchor(GLOBAL_DEPTH);
    anchor.enabled <== 1;
    anchor.masterIdentityKey <== masterIdentityKey;
    anchor.revocationNonce <== revocationNonce;
    anchor.schemaHash <== schemaHash;
    anchor.globalRoot <== globalRoot;
    for (var i = 0; i < GLOBAL_DEPTH; i++) {
        anchor.globalSiblings[i] <== globalSiblings[i];
        anchor.globalPathIndices[i] <== globalPathIndices[i];
    }

    component holderKeyDerivation = BabyPbk();
    holderKeyDerivation.in <== holderBJJPrivKey;
    holderKeyDerivation.Ax === anchor.credentialPubKeyAx;
    holderKeyDerivation.Ay === anchor.credentialPubKeyAy;

    // ═════════════════════════════════════════════════════════════════
    // STEP 0.75: Issuer-tree Membership (ADR-0014; SEC-004)
    //   leaf = Poseidon(issuerAuthority, issuerPubKeyAx, issuerPubKeyAy,
    //                   issuerStatusEpoch, issuerRevocationNonce)
    //   Single-credential circuit, so enabled is always 1; the main
    //   guard is `schemaIsZero.out === 0` above.
    // ═════════════════════════════════════════════════════════════════
    component issuerLeafHasher = Poseidon(5);
    issuerLeafHasher.inputs[0] <== issuerAuthority;
    issuerLeafHasher.inputs[1] <== issuerPubKeyAx;
    issuerLeafHasher.inputs[2] <== issuerPubKeyAy;
    issuerLeafHasher.inputs[3] <== issuerStatusEpoch;
    issuerLeafHasher.inputs[4] <== issuerRevocationNonce;

    component issuerInclusion = MerkleInclusion(ISSUER_TREE_DEPTH);
    issuerInclusion.enabled <== 1;
    issuerInclusion.leaf    <== issuerLeafHasher.out;
    issuerInclusion.root    <== issuerTreeRoot;
    for (var i = 0; i < ISSUER_TREE_DEPTH; i++) {
        issuerInclusion.siblings[i]    <== issuerSiblings[i];
        issuerInclusion.pathIndices[i] <== issuerPathIndices[i];
    }

    // ═════════════════════════════════════════════════════════════════
    // STEP 1: Credential Verification (Phase 3.1)
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
    // STEP 2: Predicate Evaluation (Phase 3.1)
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
    // STEP 3: Compound Logic (AND/OR)
    // ═════════════════════════════════════════════════════════════════
    signal and01 <== predicateResults[0] * predicateResults[1];
    signal and012 <== and01 * predicateResults[2];
    signal andResult <== and012 * predicateResults[3];

    signal activeResults[MAX_PREDICATES];
    for (var i = 0; i < MAX_PREDICATES; i++) {
        activeResults[i] <== isActive[i] * evaluators[i].result;
    }
    signal orSum <== activeResults[0] + activeResults[1] + activeResults[2] + activeResults[3];
    component orCheck = GreaterThan(8);
    orCheck.in[0] <== orSum;
    orCheck.in[1] <== 0;
    signal orResult <== orCheck.out;

    // compoundLogic is already constrained to {0, 1} above, so we can use it
    // directly as the selector here.
    signal finalResult <== (1 - compoundLogic) * andResult + compoundLogic * orResult;
    finalResult === 1;

    // ═════════════════════════════════════════════════════════════════
    // STEP 4: Expiration & Context (Phase 3.5 Handening)
    // ═════════════════════════════════════════════════════════════════
    component expiryCheck = ExpirationChecker();
    expiryCheck.currentTimestamp <== currentTimestamp;
    expiryCheck.expirationTimestamp <== expirationTimestamp;
    expiryCheck.valid === 1;

    component qHasherIndices = Poseidon(MAX_PREDICATES);
    for (var i = 0; i < MAX_PREDICATES; i++) {
        qHasherIndices.inputs[i] <== queryFieldIndices[i];
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
    // STEP 5: Hardened Nullifier (ADR-0006 + ADR-0014 revision)
    //   nullifier = Poseidon6(masterKey, revocationNonce, verifierAddress,
    //                         queryContextHash, verifierNonce, issuerTreeRoot)
    //   `issuerTreeRoot` addition binds every proof to a specific
    //   issuer-tree epoch (SOLID-SEC-008 epoch replay protection).
    // ═════════════════════════════════════════════════════════════════
    component nullifier = Poseidon(6);
    nullifier.inputs[0] <== masterIdentityKey;
    nullifier.inputs[1] <== revocationNonce;
    nullifier.inputs[2] <== verifierAddress;
    nullifier.inputs[3] <== queryContextHash;
    nullifier.inputs[4] <== verifierNonce;
    nullifier.inputs[5] <== issuerTreeRoot;
    nullifierHash <== nullifier.out;
}

// Instantiate with production parameters
component main {public [
    globalRoot,
    merkleRoot,
    schemaHash,
    issuerPubKeyAx,
    issuerPubKeyAy,
    issuerTreeRoot,
    queryFieldIndices,
    queryOperators,
    queryValues,
    numPredicates,
    compoundLogic,
    verifierAddress,
    verifierNonce,
    currentTimestamp
]} = CompoundQuerySolana(20, 20, 16, 8, 4);
