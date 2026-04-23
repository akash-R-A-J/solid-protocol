pragma circom 2.1.0;

include "../node_modules/circomlib/circuits/poseidon.circom";
include "../node_modules/circomlib/circuits/babyjub.circom";
include "./merkle_inclusion.circom";

/// Phase 3.1: IdentityAnchor (SOLID-SEC-029 hardening).
///
/// Enforces key binding, master-derived hierarchy, and global-state
/// anchoring.  The `enabled` flag lets the batch parent turn the
/// global-tree inclusion proof OFF for *padding* credential slots:
/// before SOLID-SEC-029, `enabled` was hard-wired to 1 even when the
/// slot's `schemaHash == 0`, forcing the prover to produce a real
/// inclusion proof for a fake per-schema leaf derived from
/// `Poseidon(masterKey, 0)` -- breaking any batch that wasn't
/// maximally full.
///
/// The downstream gate in `batch_credential_query.circom` passes
/// `1 - isZero[i].out`, so padding slots skip the Merkle check and
/// still cannot smuggle non-zero data (STEP 0.5 integrity constraints
/// force every per-credential field to zero when the schema is zero).
template IdentityAnchor(GLOBAL_DEPTH) {
    signal input enabled;

    signal input masterIdentityKey;
    signal input revocationNonce;

    signal input schemaHash;
    signal input globalRoot;

    signal input globalSiblings[GLOBAL_DEPTH];
    signal input globalPathIndices[GLOBAL_DEPTH];

    // Derived per-schema public key exposed to the caller.
    signal output credentialPubKeyAx;
    signal output credentialPubKeyAy;

    // `enabled` must be a bit; prevents callers from passing a scaled
    // value that would silently weaken the inclusion proof.
    enabled * (enabled - 1) === 0;

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

    // 4. Prove `identityState` is a leaf of the on-chain global root,
    //    but only when this slot is active.  `MerkleInclusion.enabled`
    //    skips the root-equality constraint when set to 0, so a
    //    padding slot does not need a real sibling path.
    component globalInclusion = MerkleInclusion(GLOBAL_DEPTH);
    globalInclusion.enabled <== enabled;
    globalInclusion.leaf    <== identityState;
    globalInclusion.root    <== globalRoot;
    for (var i = 0; i < GLOBAL_DEPTH; i++) {
        globalInclusion.siblings[i]    <== globalSiblings[i];
        globalInclusion.pathIndices[i] <== globalPathIndices[i];
    }
}
