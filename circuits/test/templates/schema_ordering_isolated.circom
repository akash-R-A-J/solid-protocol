pragma circom 2.1.0;

include "../../node_modules/circomlib/circuits/comparators.circom";
include "../../lib/lt_bn254.circom";

/// SOLID-SEC-050 regression gate -- isolated.
///
/// Exposes ONLY the schemaHashes ordering + zero-schema integrity
/// portion of `batch_credential_query.circom` STEP 0 so the witness
/// tester can exercise canonicality without standing up a full
/// 86K-constraint batch witness (signatures, Merkle proofs, etc).
///
/// The semantics this template enforces:
///
///   1. For every consecutive pair (i, i+1), if schemaHashes[i+1] is
///      active (!= 0), then schemaHashes[i] < schemaHashes[i+1].
///      This is the strict-ascending check on active->active pairs.
///
///   2. SOLID-SEC-050: once a slot is inactive (schemaHash == 0),
///      every following slot MUST also be inactive.  This forces
///      every padding slot to live at the END of the array.
///
///   3. zero-schema integrity: schemaHashes[i] == 0 forces the
///      paired sentinel `data[i]` to 0.  We expose a single sentinel
///      per slot here -- the full circuit zero-constrains many more,
///      but the layout pattern is identical.
///
/// Combined: the only valid encodings of an active set
/// {S_1, ..., S_K} for `K <= NUM_CREDS` are
///       [S_a < S_b < ... < S_z, 0, 0, ..., 0]
/// where the active prefix is sorted and padding fills the tail.
/// Any other ordering, or any "active-after-padding" interleave,
/// rejects.
template SchemaOrderingIsolated(NUM_CREDS) {
    signal input schemaHashes[NUM_CREDS];
    signal input data[NUM_CREDS];

    component isZero[NUM_CREDS];
    for (var i = 0; i < NUM_CREDS; i++) {
        isZero[i] = IsZero();
        isZero[i].in <== schemaHashes[i];

        // Mirror of STEP 0: schema == 0 -> sentinel data forced to 0.
        isZero[i].out * data[i] === 0;
    }

    signal orderingNextNotZero[NUM_CREDS - 1];
    component orderingV2[NUM_CREDS - 1];
    for (var i = 0; i < NUM_CREDS - 1; i++) {
        orderingV2[i] = LessThanBN254();
        orderingV2[i].in[0] <== schemaHashes[i];
        orderingV2[i].in[1] <== schemaHashes[i+1];

        orderingNextNotZero[i] <== 1 - isZero[i+1].out;
        orderingNextNotZero[i] * (1 - orderingV2[i].out) === 0;

        // SOLID-SEC-050: padding canonicality.
        isZero[i].out * (1 - isZero[i+1].out) === 0;
    }
}

component main = SchemaOrderingIsolated(4);
