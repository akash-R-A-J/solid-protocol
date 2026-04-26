pragma circom 2.1.0;

include "../node_modules/circomlib/circuits/poseidon.circom";
include "../node_modules/circomlib/circuits/babyjub.circom";
include "../node_modules/circomlib/circuits/bitify.circom";
include "../node_modules/circomlib/circuits/escalarmulfix.circom";
include "./merkle_inclusion.circom";

/// BabyPbk254
///
/// 254-bit-safe replacement for `circomlib::BabyPbk()`.
///
/// `circomlib::BabyPbk()` decomposes its input via `Num2Bits(253)` and
/// then applies `EscalarMulFix(253, BASE8)`.  That contract is wrong
/// for full-domain BN254 field elements: Poseidon outputs span the
/// entire BN254 scalar field [0, p) where p is a 254-bit prime, so
/// roughly 26% of derived `Poseidon(masterKey, schemaHash)` values
/// have bit 253 set and trip the `Num2Bits(253)` overflow assertion.
///
/// The off-chain Rust counterpart in
/// `crates/solid-core/src/babyjubjub.rs::derive_public_key` already
/// handles this correctly by reducing modulo the BabyJubJub subgroup
/// order r (~2^251) before scalar multiplication.  Because Base8 has
/// order r, multiplying by either the raw 254-bit Poseidon output or
/// the same value reduced mod r yields the **same** Edwards point.
/// So the public key produced by this template is byte-identical to
/// the one stored on the issuer leaf and used by the off-chain SDK.
///
/// This template uses `Num2Bits_strict()` (which attaches an
/// `AliasCheck` enforcing the input lies in [0, p)) and then
/// `EscalarMulFix(254, BASE8)`, sweeping all 254 bits.  No information
/// is lost and there is no implicit reduction inside the circuit.
///
/// Hard invariant: any caller MUST be passing a value already in the
/// canonical Fp range; a Poseidon output trivially satisfies this.
template BabyPbk254() {
    signal input  in;
    signal output Ax;
    signal output Ay;

    var BASE8[2] = [
        5299619240641551281634865583518297030282874472190772894086521144482721001553,
        16950150798460657717958625567821834550301663161624707787222815936182638968203
    ];

    component pvkBits = Num2Bits_strict();
    pvkBits.in <== in;

    component mulFix = EscalarMulFix(254, BASE8);
    for (var i = 0; i < 254; i++) {
        mulFix.e[i] <== pvkBits.out[i];
    }
    Ax <== mulFix.out[0];
    Ay <== mulFix.out[1];
}

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

    // 2. Derive the per-schema public key via BabyJubJub scalar
    //    multiplication.  Uses the local `BabyPbk254` (above) instead
    //    of `circomlib::BabyPbk()` because Poseidon outputs are
    //    254-bit and the circomlib variant rejects ~26% of valid
    //    field elements.  See the BabyPbk254 docstring and ADR
    //    Phase 3.4 (circuit/Rust contract divergence closeout).
    component bjjDerivation = BabyPbk254();
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
