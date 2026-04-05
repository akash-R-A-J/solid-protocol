include "poseidon.circom";
include "merkle_inclusion.circom";
include "signature_verifier.circom";

/// Phase 3.1: CredentialAtom
/// Verifies one credential + Merkle inclusion + Identity binding.
template CredentialAtom(NUM_FIELDS, TREE_DEPTH) {
    // Public Inputs
    signal input schemaHash;
    signal input merkleRoot;
    signal input issuerPubKeyAx;
    signal input issuerPubKeyAy;

    // Private Inputs
    signal input attestationData[NUM_FIELDS];
    signal input salt;
    signal input holderBJJPubKeyAx;
    signal input holderBJJPubKeyAy;
    signal input issuerSigR8x;
    signal input issuerSigR8y;
    signal input issuerSigS;
    signal input merkleSiblings[TREE_DEPTH];
    signal input merklePathIndices[TREE_DEPTH];

    // Out: The message hash (attestation commitment) for downstream evaluation
    signal output attestationHash;

    // 1. Compute attestation commitment: Poseidon(data, salt, holderPubKey)
    component dataHasher = Poseidon(NUM_FIELDS + 3);
    for (var i = 0; i < NUM_FIELDS; i++) {
        dataHasher.inputs[i] <== attestationData[i];
    }
    dataHasher.inputs[NUM_FIELDS] <== salt;
    dataHasher.inputs[NUM_FIELDS+1] <== holderBJJPubKeyAx;
    dataHasher.inputs[NUM_FIELDS+2] <== holderBJJPubKeyAy;
    attestationHash <== dataHasher.out;

    // 2. Verify Issuer Signature
    component sigVerifier = SignatureVerifier();
    sigVerifier.pubKeyX <== issuerPubKeyAx;
    sigVerifier.pubKeyY <== issuerPubKeyAy;
    sigVerifier.R8x <== issuerSigR8x;
    sigVerifier.R8y <== issuerSigR8y;
    sigVerifier.S <== issuerSigS;
    sigVerifier.M <== attestationHash;

    // 3. Verify Merkle Inclusion
    component inclusion = MerkleInclusion(TREE_DEPTH);
    inclusion.leaf <== attestationHash;
    inclusion.root <== merkleRoot;
    for (var i = 0; i < TREE_DEPTH; i++) {
        inclusion.siblings[i] <== merkleSiblings[i];
        inclusion.pathIndices[i] <== merklePathIndices[i];
    }
}
