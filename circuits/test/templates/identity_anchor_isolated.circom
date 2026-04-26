pragma circom 2.1.0;

include "../../lib/identity_anchor.circom";

/// Phase 3.4 regression gate -- isolated.
///
/// Drives `IdentityAnchor` end-to-end with the Merkle inclusion
/// proof disabled (`enabled = 0`), so the mocha harness can validate
/// the (masterIdentityKey, schemaHash) -> (credentialPubKeyAx,
/// credentialPubKeyAy) derivation contract without needing a real
/// global-tree path.
///
/// Concretely this exercises:
///   credentialPrivKey = Poseidon(masterIdentityKey, schemaHash)
///   credentialPubKey  = BabyPbk254(credentialPrivKey)
/// which is exactly the production keypath for credentials issued
/// off-chain by the SDK.  The on-chain leaf check is intentionally
/// skipped here because that's a separate concern with its own
/// regression test layer (cross_language_vectors).
///
/// The fixed depth here matches the production constant
/// `GLOBAL_DEPTH = 20` from circuits/batch_credential_query.circom,
/// so the witness-gen profile (poseidon-2 + BabyPbk254 + 20-deep
/// MerkleInclusion-with-enabled=0) is faithful to the real anchor.
template IdentityAnchorIsolated(GLOBAL_DEPTH) {
    signal input masterIdentityKey;
    signal input schemaHash;
    signal input revocationNonce;

    signal output credentialPubKeyAx;
    signal output credentialPubKeyAy;

    component anchor = IdentityAnchor(GLOBAL_DEPTH);
    anchor.enabled            <== 0;
    anchor.masterIdentityKey  <== masterIdentityKey;
    anchor.revocationNonce    <== revocationNonce;
    anchor.schemaHash         <== schemaHash;
    anchor.globalRoot         <== 0;
    for (var i = 0; i < GLOBAL_DEPTH; i++) {
        anchor.globalSiblings[i]    <== 0;
        anchor.globalPathIndices[i] <== 0;
    }

    credentialPubKeyAx <== anchor.credentialPubKeyAx;
    credentialPubKeyAy <== anchor.credentialPubKeyAy;
}

component main = IdentityAnchorIsolated(20);
