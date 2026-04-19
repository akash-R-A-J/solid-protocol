pragma circom 2.1.0;

include "../node_modules/circomlib/circuits/poseidon.circom";
include "../node_modules/circomlib/circuits/eddsaposeidon.circom";
include "./merkle_inclusion.circom";

/// Phase 3.1: CredentialAtom
/// Verifies one credential: commitment integrity + issuer signature + Merkle inclusion.
///
/// Canonical commitment formula (matches `crates/solid-core/src/commitment.rs`):
///   dataHash   = Poseidon(data[0], ..., data[N-1])
///   commitment = Poseidon(dataHash, schemaHash, holderAx, holderAy, salt)
///
/// The circuit outputs `attestationHash` = `commitment` so the upstream circuit
/// (batch_credential_query) can Merkle-verify inclusion with the same leaf.
template CredentialAtom(NUM_FIELDS, TREE_DEPTH) {
    // Public inputs
    signal input schemaHash;
    signal input merkleRoot;
    signal input issuerPubKeyAx;
    signal input issuerPubKeyAy;

    // Private inputs
    signal input attestationData[NUM_FIELDS];
    signal input salt;
    signal input holderBJJPubKeyAx;
    signal input holderBJJPubKeyAy;
    signal input issuerSigR8x;
    signal input issuerSigR8y;
    signal input issuerSigS;
    signal input merkleSiblings[TREE_DEPTH];
    signal input merklePathIndices[TREE_DEPTH];

    // If the slot is a zero-schema placeholder, we bypass signature / Merkle checks.
    // `enabled = 1 - isZero(schemaHash)`.
    component schemaIsZero = IsZero();
    schemaIsZero.in <== schemaHash;
    signal enabled;
    enabled <== 1 - schemaIsZero.out;

    // Output: commitment (leaf value)
    signal output attestationHash;

    // Step 1: dataHash = Poseidon(data[0..N-1])
    component dataHasher = Poseidon(NUM_FIELDS);
    for (var i = 0; i < NUM_FIELDS; i++) {
        dataHasher.inputs[i] <== attestationData[i];
    }
    signal dataHash;
    dataHash <== dataHasher.out;

    // Step 2: commitment = Poseidon(dataHash, schemaHash, holderAx, holderAy, salt)
    component commitHasher = Poseidon(5);
    commitHasher.inputs[0] <== dataHash;
    commitHasher.inputs[1] <== schemaHash;
    commitHasher.inputs[2] <== holderBJJPubKeyAx;
    commitHasher.inputs[3] <== holderBJJPubKeyAy;
    commitHasher.inputs[4] <== salt;
    attestationHash <== commitHasher.out;

    // Step 3: Verify issuer signature over the commitment (circomlib EdDSAPoseidonVerifier).
    component sigVerifier = EdDSAPoseidonVerifier();
    sigVerifier.enabled <== enabled;
    sigVerifier.Ax      <== issuerPubKeyAx;
    sigVerifier.Ay      <== issuerPubKeyAy;
    sigVerifier.R8x     <== issuerSigR8x;
    sigVerifier.R8y     <== issuerSigR8y;
    sigVerifier.S       <== issuerSigS;
    sigVerifier.M       <== attestationHash;

    // Step 4: Merkle inclusion in the schema-scoped credential tree.
    // Uses a binary Merkle tree (matches Light Protocol indexed tree semantics).
    component inclusion = MerkleInclusion(TREE_DEPTH);
    inclusion.enabled <== enabled;
    inclusion.leaf    <== attestationHash;
    inclusion.root    <== merkleRoot;
    for (var i = 0; i < TREE_DEPTH; i++) {
        inclusion.siblings[i]    <== merkleSiblings[i];
        inclusion.pathIndices[i] <== merklePathIndices[i];
    }
}

template IsZero() {
    signal input in;
    signal output out;
    signal inv;
    inv <-- in != 0 ? 1 / in : 0;
    out <== -in * inv + 1;
    in * out === 0;
}
