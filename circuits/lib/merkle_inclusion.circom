pragma circom 2.1.0;

include "../node_modules/circomlib/circuits/smt/smtverifier.circom";

/// Verify Merkle inclusion of a credential commitment in a Light Protocol tree.
template MerkleInclusion(DEPTH) {
    signal input root;              // Public: Merkle root (on-chain state root)
    signal input leaf;              // Private: the commitment value
    signal input siblings[DEPTH];   // Private: Merkle proof path
    signal input pathIndices[DEPTH]; // Private: left/right path bits
    signal output verified;

    component smtVerifier = SMTVerifier(DEPTH);
    smtVerifier.enabled <== 1;
    smtVerifier.root <== root;
    smtVerifier.siblings <== siblings;
    smtVerifier.oldKey <== 0;
    smtVerifier.oldValue <== 0;
    smtVerifier.isOld0 <== 0;
    smtVerifier.key <== leaf;
    smtVerifier.value <== leaf;
    smtVerifier.fnc <== 0; // 0 = inclusion proof

    // CRITICAL FIX: Constrain output to 1 to ensure inclusion is verified
    smtVerifier.out === 1;
    verified <== smtVerifier.out;
}
