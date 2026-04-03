pragma circom 2.1.0;

include "../node_modules/circomlib/circuits/eddsaposeidon.circom";

/// Verify an EdDSA-Poseidon signature from the issuer over the commitment.
/// Uses circomlib's EdDSAPoseidonVerifier internally.
template SignatureVerifier() {
    signal input enabled;
    signal input Ax;       // Issuer public key X
    signal input Ay;       // Issuer public key Y
    signal input S;        // Signature scalar
    signal input R8x;      // Signature R8 point X
    signal input R8y;      // Signature R8 point Y
    signal input M;        // Message (commitment hash)

    component verifier = EdDSAPoseidonVerifier();
    verifier.enabled <== enabled;
    verifier.Ax <== Ax;
    verifier.Ay <== Ay;
    verifier.S <== S;
    verifier.R8x <== R8x;
    verifier.R8y <== R8y;
    verifier.M <== M;
}
