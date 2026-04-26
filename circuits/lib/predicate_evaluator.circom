pragma circom 2.1.0;

include "../node_modules/circomlib/circuits/comparators.circom";
include "../node_modules/circomlib/circuits/mux1.circom";

/// Evaluate a single predicate: field_value <operator> query_value
/// Operators: 0=NOOP 1=EQ 2=NE 3=GT 4=GTE 5=LT 6=LTE
/// Output: 1 if predicate passes, 0 if fails.
template PredicateEvaluator() {
    signal input fieldValue;
    signal input operator;
    signal input queryValue;
    signal output result;

    // Comparison components
    component isEq = IsEqual();
    isEq.in[0] <== fieldValue;
    isEq.in[1] <== queryValue;

    // Domain contract: `fieldValue` and `queryValue` must each fit in
    // 252 bits.  This is a protocol-level constraint inherited from
    // `circomlib::LessThan(n) / GreaterThan(n)`, which assert
    // `n <= 252`.
    //
    // `fieldValue` is sourced from issuer-signed credential leaves;
    // the issuer SDK is responsible for keeping attribute values
    // within the 252-bit domain (in practice all realistic
    // attributes -- ages, dates, balances, geohash-style integers --
    // are far smaller).  `queryValue` is a verifier-supplied
    // constant exposed as a public input; the on-chain
    // `verify_batch_proof` does not currently bound it, so verifiers
    // are expected to keep query values in-domain.
    //
    // This is *not* the same problem as the schemaHash ordering
    // overflow fixed in Phase 3.4 (see `lt_bn254.circom`): schema
    // hashes are full-domain Poseidon outputs by construction,
    // whereas predicate operands are domain-specific scalars.
    // Promoting these to 254-bit-safe comparators would require a
    // matching SDK / on-chain range-check on the operands and is
    // tracked under Phase 4.
    component isGt = GreaterThan(252);
    isGt.in[0] <== fieldValue;
    isGt.in[1] <== queryValue;

    component isLt = LessThan(252);
    isLt.in[0] <== fieldValue;
    isLt.in[1] <== queryValue;

    // Derived comparisons
    signal gte <== isGt.out + isEq.out - isGt.out * isEq.out; // GTE = GT OR EQ
    signal lte <== isLt.out + isEq.out - isLt.out * isEq.out; // LTE = LT OR EQ
    signal ne <== 1 - isEq.out;

    // Operator decoding (one-hot via equality checks)
    component opIs[7];
    for (var i = 0; i < 7; i++) {
        opIs[i] = IsEqual();
        opIs[i].in[0] <== operator;
        opIs[i].in[1] <== i;
    }

    // Select result based on operator:
    // op=0 (NOOP) → always 1
    // op=1 (EQ)   → isEq
    // op=2 (NE)   → ne
    // op=3 (GT)   → isGt
    // op=4 (GTE)  → gte
    // op=5 (LT)   → isLt
    // op=6 (LTE)  → lte

    // Each product must be its own constraint (R1CS allows one multiplication per constraint)
    signal prod0 <== opIs[0].out * 1;           // NOOP → 1
    signal prod1 <== opIs[1].out * isEq.out;    // EQ
    signal prod2 <== opIs[2].out * ne;           // NE
    signal prod3 <== opIs[3].out * isGt.out;    // GT
    signal prod4 <== opIs[4].out * gte;          // GTE
    signal prod5 <== opIs[5].out * isLt.out;    // LT
    signal prod6 <== opIs[6].out * lte;          // LTE

    // Sum is now purely linear (no multiplications) → valid R1CS
    result <== prod0 + prod1 + prod2 + prod3 + prod4 + prod5 + prod6;
}

/// Select a field value from attestation data by index.
template FieldSelector(NUM_FIELDS) {
    signal input data[NUM_FIELDS];
    signal input index;
    signal output value;

    component indexEq[NUM_FIELDS];
    signal mux[NUM_FIELDS];

    var acc = 0;
    for (var i = 0; i < NUM_FIELDS; i++) {
        indexEq[i] = IsEqual();
        indexEq[i].in[0] <== index;
        indexEq[i].in[1] <== i;
        mux[i] <== indexEq[i].out * data[i];
        acc += mux[i];
    }
    value <== acc;
}

/// Select a field value from a batch of credentials by credentialIndex and fieldIndex.
/// Phase 3.1: Composable Identity.
template BatchFieldSelector(NUM_CREDS, NUM_FIELDS) {
    signal input data[NUM_CREDS][NUM_FIELDS];
    signal input credIndex;
    signal input fieldIndex;
    signal output value;

    component selectors[NUM_CREDS];
    component credIs[NUM_CREDS];
    signal mux[NUM_CREDS];

    for (var i = 0; i < NUM_CREDS; i++) {
        selectors[i] = FieldSelector(NUM_FIELDS);
        for (var j = 0; j < NUM_FIELDS; j++) {
            selectors[i].data[j] <== data[i][j];
        }
        selectors[i].index <== fieldIndex;

        credIs[i] = IsEqual();
        credIs[i].in[0] <== credIndex;
        credIs[i].in[1] <== i;

        mux[i] <== credIs[i].out * selectors[i].value;
    }

    signal sum01 <== mux[0] + mux[1];
    signal sum23 <== mux[2] + mux[3];
    value <== sum01 + sum23;
}
