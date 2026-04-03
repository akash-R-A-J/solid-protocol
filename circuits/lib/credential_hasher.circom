pragma circom 2.1.0;

include "../node_modules/circomlib/circuits/poseidon.circom";

/// Hash NUM_FIELDS attestation data values into a single Poseidon digest.
/// Output: dataHash = Poseidon(data[0], data[1], ..., data[NUM_FIELDS-1])
template CredentialHasher(NUM_FIELDS) {
    signal input data[NUM_FIELDS];
    signal output dataHash;

    component hasher = Poseidon(NUM_FIELDS);
    for (var i = 0; i < NUM_FIELDS; i++) {
        hasher.inputs[i] <== data[i];
    }
    dataHash <== hasher.out;
}
