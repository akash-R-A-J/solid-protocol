pragma circom 2.1.0;

// Isolated test driver for `BjjSubgroupCheck` (SOLID-SEC-048 Option B).
//
// Exposes the template's two public inputs (Ax, Ay) so the mocha
// witness-tester at `circuits/test/bjj_subgroup.test.js` can exercise
// it without going through the full `bjj_subgroup_proof.circom`
// main-component framing (which the test driver doesn't need).

include "../../lib/bjj_subgroup.circom";

template BjjSubgroupIsolated() {
    signal input Ax;
    signal input Ay;

    component check = BjjSubgroupCheck();
    check.Ax <== Ax;
    check.Ay <== Ay;
}

component main = BjjSubgroupIsolated();
