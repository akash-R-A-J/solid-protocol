pragma circom 2.1.0;

include "../node_modules/circomlib/circuits/poseidon.circom";
include "../node_modules/circomlib/circuits/mux1.circom";

/// Binary Poseidon Merkle inclusion proof.
///
/// Recomputes `root' = Hn(... H2(H1(leaf, sibling[0]), sibling[1]) ...)` where
/// each step hashes `Poseidon(left, right)` with left/right chosen by `pathIndices[i]`
/// (0 = current on the left, 1 = current on the right). If `enabled == 1`, asserts
/// `root' === root`. If `enabled == 0`, the check is skipped (used for zero-schema
/// placeholders in batch proofs).
///
/// This matches Light Protocol's indexed binary Merkle tree with the Poseidon
/// compression function.
template MerkleInclusion(DEPTH) {
    signal input enabled;
    signal input leaf;
    signal input root;
    signal input siblings[DEPTH];
    signal input pathIndices[DEPTH];

    signal output computedRoot;

    component hashers[DEPTH];
    component muxL[DEPTH];
    component muxR[DEPTH];

    signal running[DEPTH + 1];
    running[0] <== leaf;

    for (var i = 0; i < DEPTH; i++) {
        // pathIndices[i] must be 0 or 1
        pathIndices[i] * (pathIndices[i] - 1) === 0;

        // left  = pathIndices[i] == 0 ? running[i] : siblings[i]
        // right = pathIndices[i] == 0 ? siblings[i] : running[i]
        muxL[i] = Mux1();
        muxL[i].c[0] <== running[i];
        muxL[i].c[1] <== siblings[i];
        muxL[i].s   <== pathIndices[i];

        muxR[i] = Mux1();
        muxR[i].c[0] <== siblings[i];
        muxR[i].c[1] <== running[i];
        muxR[i].s   <== pathIndices[i];

        hashers[i] = Poseidon(2);
        hashers[i].inputs[0] <== muxL[i].out;
        hashers[i].inputs[1] <== muxR[i].out;

        running[i + 1] <== hashers[i].out;
    }

    computedRoot <== running[DEPTH];

    // Conditional equality: enabled * (computedRoot - root) === 0
    signal diff;
    diff <== computedRoot - root;
    enabled * diff === 0;
}
