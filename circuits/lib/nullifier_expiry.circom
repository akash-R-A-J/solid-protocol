pragma circom 2.1.0;

include "../node_modules/circomlib/circuits/poseidon.circom";
include "../node_modules/circomlib/circuits/comparators.circom";

/// Compute a nullifier hash for anti-replay.
/// nullifier = Poseidon(holderPrivKey, schemaHash, verifierNonce)
template NullifierComputer() {
    signal input holderPrivKey;
    signal input schemaHash;
    signal input verifierNonce;
    signal output nullifier;

    component hasher = Poseidon(3);
    hasher.inputs[0] <== holderPrivKey;
    hasher.inputs[1] <== schemaHash;
    hasher.inputs[2] <== verifierNonce;
    nullifier <== hasher.out;
}

/// Check credential expiration: currentTimestamp <= expirationTimestamp
/// If expirationTimestamp == 0, always valid (no expiry).
template ExpirationChecker() {
    signal input currentTimestamp;
    signal input expirationTimestamp;
    signal output valid;

    component isZero = IsZero();
    isZero.in <== expirationTimestamp;

    // If expiration is 0, always valid
    // Otherwise, check currentTimestamp <= expirationTimestamp
    component lte = LessEqThan(252);
    lte.in[0] <== currentTimestamp;
    lte.in[1] <== expirationTimestamp;

    // valid = isZero OR lte
    valid <== isZero.out + lte.out - isZero.out * lte.out;
}
