pragma circom 2.1.0;

include "../../lib/identity_anchor.circom";
include "../../node_modules/circomlib/circuits/comparators.circom";

/// SOLID-SEC-029 regression gate -- isolated.
///
/// Instantiates `IdentityAnchor` directly so the mocha harness can
/// exercise the two interesting shapes without building a full
/// multi-credential batch witness:
///
///   1. `enabled = 1`, `schemaHash != 0`, valid globalRoot +
///       siblings  -> witness calc succeeds.
///   2. `enabled = 0`, `schemaHash = 0`, siblings are *arbitrary*
///       field elements -> witness calc still succeeds, because
///       `MerkleInclusion` gates its root-equality check on `enabled`.
///
/// Shape (2) is the bit SEC-029 fixes: pre-fix, shape (2) would have
/// forced a valid Merkle proof for a zero-schema derived leaf, which
/// is architecturally impossible to produce without a zero-schema
/// entry in the global tree.
///
/// GLOBAL_DEPTH is kept small here (4) so the test fixture is
/// compact; the semantic check does not depend on depth.
template AnchorEnabledIsolated(GLOBAL_DEPTH) {
    signal input enabled;
    signal input masterIdentityKey;
    signal input revocationNonce;
    signal input schemaHash;
    signal input globalRoot;
    signal input globalSiblings[GLOBAL_DEPTH];
    signal input globalPathIndices[GLOBAL_DEPTH];

    signal output credentialPubKeyAx;
    signal output credentialPubKeyAy;

    component anchor = IdentityAnchor(GLOBAL_DEPTH);
    anchor.enabled <== enabled;
    anchor.masterIdentityKey <== masterIdentityKey;
    anchor.revocationNonce <== revocationNonce;
    anchor.schemaHash <== schemaHash;
    anchor.globalRoot <== globalRoot;
    for (var i = 0; i < GLOBAL_DEPTH; i++) {
        anchor.globalSiblings[i] <== globalSiblings[i];
        anchor.globalPathIndices[i] <== globalPathIndices[i];
    }

    credentialPubKeyAx <== anchor.credentialPubKeyAx;
    credentialPubKeyAy <== anchor.credentialPubKeyAy;
}

component main = AnchorEnabledIsolated(4);
