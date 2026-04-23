pragma circom 2.1.0;

include "../../node_modules/circomlib/circuits/comparators.circom";

/// SOLID-SEC-001 regression gate -- isolated.
///
/// Mirrors the range checks wired into `batch_credential_query.circom`
/// STEP-0 (see the big SOLID-SEC-001 block there).  Isolating the
/// constraints into their own template lets the mocha harness witness-
/// calculate against them directly -- full happy-path witness gen for
/// the main circuit would require valid Poseidon commitments, BJJ
/// signatures, and Merkle tree siblings, which is orders-of-magnitude
/// more setup code for no stronger a regression signal.
///
/// If these constraints ever regress in the main circuit, the same
/// `LessThan(8).out === 1` assertion fires; this template is therefore
/// a faithful stand-in.
template RangeCheckIsolated(MAX_PREDICATES, NUM_CREDS, NUM_FIELDS) {
    signal input queryCredentialIndices[MAX_PREDICATES];
    signal input queryFieldIndices[MAX_PREDICATES];

    component credChecks[MAX_PREDICATES];
    component fieldChecks[MAX_PREDICATES];
    for (var i = 0; i < MAX_PREDICATES; i++) {
        credChecks[i] = LessThan(8);
        credChecks[i].in[0] <== queryCredentialIndices[i];
        credChecks[i].in[1] <== NUM_CREDS;
        credChecks[i].out === 1;

        fieldChecks[i] = LessThan(8);
        fieldChecks[i].in[0] <== queryFieldIndices[i];
        fieldChecks[i].in[1] <== NUM_FIELDS;
        fieldChecks[i].out === 1;
    }
}

component main = RangeCheckIsolated(4, 4, 8);
