pragma circom 2.1.0;

include "../node_modules/circomlib/circuits/poseidon.circom";
include "../node_modules/circomlib/circuits/babyjub.circom";
include "./merkle_inclusion.circom";

/// Phase 3.1: IdentityAnchor
/// Enforces key binding, master-derived hierarchy, and global state anchoring.
template IdentityAnchor(GLOBAL_DEPTH) {
    signal input masterIdentityKey;
    signal input revocationNonce;

    signal input schemaHash;
    signal input globalRoot;

    signal input globalSiblings[GLOBAL_DEPTH];
    signal input globalPathIndices[GLOBAL_DEPTH];

    // Derived per-schema public key exposed to the caller.
    signal output credentialPubKeyAx;
    signal output credentialPubKeyAy;

    // 1. Derive per-schema private key: Poseidon(masterKey, schemaHash).
    component credKeyDeriver = Poseidon(2);
    credKeyDeriver.inputs[0] <== masterIdentityKey;
    credKeyDeriver.inputs[1] <== schemaHash;
    signal credentialPrivKey;
    credentialPrivKey <== credKeyDeriver.out;

    // 2. Derive the per-schema public key via BabyJubJub scalar multiplication.
    component bjjDerivation = BabyPbk();
    bjjDerivation.in <== credentialPrivKey;
    credentialPubKeyAx <== bjjDerivation.Ax;
    credentialPubKeyAy <== bjjDerivation.Ay;

    // 3. Compute the identityState commitment the global tree stores at this leaf.
    //    identityState = Poseidon(Ax, Ay, revocationNonce)
    component idStateHasher = Poseidon(3);
    idStateHasher.inputs[0] <== bjjDerivation.Ax;
    idStateHasher.inputs[1] <== bjjDerivation.Ay;
    idStateHasher.inputs[2] <== revocationNonce;
    signal identityState;
    identityState <== idStateHasher.out;

    // 4. Prove `identityState` is a leaf of the on-chain global root.
    component globalInclusion = MerkleInclusion(GLOBAL_DEPTH);
    globalInclusion.enabled <== 1;
    globalInclusion.leaf    <== identityState;
    globalInclusion.root    <== globalRoot;
    for (var i = 0; i < GLOBAL_DEPTH; i++) {
        globalInclusion.siblings[i]    <== globalSiblings[i];
        globalInclusion.pathIndices[i] <== globalPathIndices[i];
    }
}
