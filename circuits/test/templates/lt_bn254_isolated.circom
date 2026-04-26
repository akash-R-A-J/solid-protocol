pragma circom 2.1.0;

include "../../lib/lt_bn254.circom";

/// Phase 3.4 regression gate -- isolated.
///
/// Drives `LessThanBN254` directly so the mocha harness can witness-
/// calculate the 254-bit-safe schema-hash ordering check used inside
/// `batch_credential_query.circom` STEP-3.5b without having to spin up
/// the full proving pipeline (Merkle paths, BJJ signatures, etc).
///
/// `LessThanBN254` is a strict (a < b) comparator over the canonical
/// BN254 scalar field [0, p).  Each input is decomposed with
/// `Num2Bits_strict()` (an `AliasCheck` enforces in < p), so over-
/// large inputs MUST fail witness generation -- this template is the
/// faithful regression vehicle for that contract.
template LtBn254Isolated() {
    signal input a;
    signal input b;
    signal output out;

    component cmp = LessThanBN254();
    cmp.in[0] <== a;
    cmp.in[1] <== b;
    out <== cmp.out;
}

component main = LtBn254Isolated();
