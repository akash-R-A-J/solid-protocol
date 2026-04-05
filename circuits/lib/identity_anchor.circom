include "../node_modules/circomlib/circuits/poseidon.circom";
include "../node_modules/circomlib/circuits/babyjub.circom";
include "merkle_inclusion.circom";

/// Phase 3.1: IdentityAnchor
/// Enforces key binding, master-derived hierarchy, and global state anchoring.
template IdentityAnchor(GLOBAL_DEPTH) {
    // Private Inputs
    signal input masterIdentityKey;
    signal input revocationNonce;
    
    // Public Inputs (for anchoring)
    signal input schemaHash;
    signal input globalRoot;
    
    // Auxiliary Proof Inputs
    signal input globalSiblings[GLOBAL_DEPTH];
    signal input globalPathIndices[GLOBAL_DEPTH];

    // Out: The derived credential-specific public key components
    signal output credentialPubKeyAx;
    signal output credentialPubKeyAy;

    // 1. Derive credential-specific private key: Poseidon(masterKey, schemaHash)
    component credKeyDeriver = Poseidon(2);
    credKeyDeriver.inputs[0] <== masterIdentityKey;
    credKeyDeriver.inputs[1] <== schemaHash;
    signal credentialPrivKey <== credKeyDeriver.out;

    // 2. Compute the credential-specific public key
    component bjjDerivation = BabyPbk();
    bjjDerivation.in <== credentialPrivKey;
    credentialPubKeyAx <== bjjDerivation.Ax;
    credentialPubKeyAy <== bjjDerivation.Ay;

    // 3. Compute identityState commitment for Global Anchoring
    // identityState = Poseidon(Ax, Ay, revocationNonce)
    // Note: We anchor the MASTER-level identity (or derived, but the plan 2.1 says holderBJJPubKey)
    // For consistency, we'll anchor the DERIVED key to allow per-app rotation.
    component idStateHasher = Poseidon(3);
    idStateHasher.inputs[0] <== bjjDerivation.Ax;
    idStateHasher.inputs[1] <== bjjDerivation.Ay;
    idStateHasher.inputs[2] <== revocationNonce;
    signal identityState <== idStateHasher.out;

    // 4. Verify Identity exists in Global Root (Light Protocol)
    component globalInclusion = MerkleInclusion(GLOBAL_DEPTH);
    globalInclusion.leaf <== identityState;
    globalInclusion.root <== globalRoot;
    for (var i = 0; i < GLOBAL_DEPTH; i++) {
        globalInclusion.siblings[i] <== globalSiblings[i];
        globalInclusion.pathIndices[i] <== globalPathIndices[i];
    }
}
