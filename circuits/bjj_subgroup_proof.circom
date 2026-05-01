pragma circom 2.1.0;

// SOLID-SEC-048 Option B (registration-time subgroup proof) -- main
// circuit.
//
// Public inputs: (Ax, Ay) = the BabyJubJub coordinates of the issuer
// pubkey being registered.  No private inputs (the proof does not
// hide anything; it just attests that the public inputs satisfy the
// prime-order subgroup invariant).
//
// On-chain consumer: `programs/issuer-registry/src/lib.rs::register_issuer`
// will accept this proof alongside the candidate pubkey and verify
// it on-chain via the same alt_bn128 syscall path as
// `verify_batch_proof_v2`.  Invalid proofs reject the registration.
//
// Trusted setup: a NEW Phase-2 over the existing
// `circuits/trusted_setup/pot_final.ptau` (Powers-of-Tau Phase 1 is
// circuit-agnostic so the existing 131K-capacity PTAU covers this
// ~2.5K-constraint circuit with massive headroom).  The new zkey
// lands at `circuits/build/bjj_subgroup_proof.zkey`; the new VK at
// `circuits/build/bjj_subgroup_verification_key.json`; the SHA-256
// pin at `circuits/build/bjj_subgroup_verification_key.sha256`.
//
// Mainnet: requires SOLID-SEC-012 multi-party Phase-2 for THIS
// circuit in addition to the main batch circuit's Phase-2.  Same
// PTAU, same contributor pool.

include "lib/bjj_subgroup.circom";

template BjjSubgroupProof() {
    signal input Ax;
    signal input Ay;

    component check = BjjSubgroupCheck();
    check.Ax <== Ax;
    check.Ay <== Ay;
}

component main {public [Ax, Ay]} = BjjSubgroupProof();
