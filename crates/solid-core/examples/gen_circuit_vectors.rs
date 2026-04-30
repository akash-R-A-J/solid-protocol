//! Phase-3.4 circuit-side test-vector generator.
//!
//! Run with:
//!   cargo run --example gen_circuit_vectors \
//!       --manifest-path crates/solid-core/Cargo.toml
//!
//! Writes `circuits/test/fixtures/circuit_vectors.json`, the ground-truth
//! file consumed by the mocha tests in `circuits/test/`:
//!
//!   * `lt_bn254.test.js`            -> `lt_bn254` block
//!   * `babypbk254.test.js`          -> `baby_pbk254` block
//!   * `identity_anchor_derivation.test.js` -> `identity_anchor` block
//!
//! Why a separate generator (vs. extending `gen_vectors.rs`):
//!   - `gen_vectors` produces the cross-language byte-vector contract
//!     (commitment + nullifier) consumed by the TS SDK and gated by the
//!     `cross_language_vectors` CI job.
//!   - This file produces the *circuit*-input contract: same Rust
//!     ground truth, but expressed as decimal field elements suitable
//!     for direct paste into circom witness inputs.  Mixing the two
//!     formats in one file would couple two unrelated CI gates.
//!
//! All vectors are deterministic (seeded by index, no RNG) so CI can
//! diff the JSON file byte-for-byte after edits.  The set is curated
//! to exercise:
//!
//!   * Small ascending / equal / descending pairs (`lt_bn254`).
//!   * Boundary cases: 0, 1, p-1.
//!   * Bit-253-set scalars -- the regression class for the original
//!     `Num2Bits(253)` failure inside `circomlib::BabyPbk()` and the
//!     `LessThan(252)` overflow in the schema-hash ordering check.
//!
//! Hard invariant for downstream consumers:
//!   - All field elements are emitted as **decimal strings**, not hex,
//!     because circom's witness generator coerces strings to `BigInt`
//!     and accepts decimal natively.  Hex would round-trip but adds an
//!     extra parsing layer in the JS test harness.
//!   - Each input MUST already be canonical mod p (BN254 scalar field).
//!     `Fr::from(u64)` and `bytes_le_to_fr` enforce this; we never emit
//!     >= p values.

use anyhow::Context;
use ark_bn254::Fr as Bn254Fr;
use ark_ed_on_bn254::{Fq as BjjFq, Fr as BjjFr};
use ark_ff::{BigInteger, PrimeField};
use num_bigint::BigUint;
use serde::Serialize;
use solid_core::{
    babyjubjub::{derive_public_key, is_in_prime_order_subgroup, is_on_curve, BJJPublicKey},
    poseidon,
};
use std::fs;
use std::path::PathBuf;

// ─── JSON shape ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct CircuitVectors {
    description: &'static str,
    /// BN254 scalar field prime, as decimal.  Useful for the JS test
    /// harness to assert that every emitted field element lies in
    /// [0, bn254_prime) without re-deriving it.
    bn254_prime_dec: String,
    /// BabyJubJub subgroup order r, as decimal.  Needed by
    /// `babypbk254.test.js` to verify `s * Base8 == (s mod r) * Base8`.
    bjj_subgroup_order_dec: String,
    /// BabyJubJub generator Base8, in decimal field-element coords.
    bjj_base8: BjjPoint,
    lt_bn254: Vec<LtVector>,
    baby_pbk254: Vec<BabyPbkVector>,
    identity_anchor: Vec<IdentityAnchorVector>,
}

#[derive(Serialize)]
struct BjjPoint {
    x_dec: String,
    y_dec: String,
}

/// Vector for the standalone `LessThanBN254` template.
#[derive(Serialize)]
struct LtVector {
    /// Human-readable description of what this vector exercises.
    note: &'static str,
    /// First operand (in[0]) as decimal field element.
    a_dec: String,
    /// Second operand (in[1]) as decimal field element.
    b_dec: String,
    /// Expected `out`: 1 iff a < b (strict, integer interpretation in
    /// [0, p)), 0 otherwise.
    expected_lt: u8,
    /// Whether bit 253 of `a` and / or `b` is set.  Vectors with at
    /// least one operand in [2^253, p) are the regression class for
    /// the old `circomlib::LessThan(252)` overflow.
    a_bit253_set: bool,
    b_bit253_set: bool,
}

/// Vector for the standalone `BabyPbk254` template.
#[derive(Serialize)]
struct BabyPbkVector {
    note: &'static str,
    /// Private key as decimal scalar in [0, p).  The circuit
    /// decomposes via `Num2Bits_strict()` and computes
    /// `EscalarMulFix(254, BASE8)`.
    sk_dec: String,
    /// Expected public key X coordinate as decimal field element.
    pk_x_dec: String,
    /// Expected public key Y coordinate as decimal field element.
    pk_y_dec: String,
    /// `Fr::from_le_bytes_mod_order(sk_bytes)` reduced into the BJJ
    /// subgroup order r, as decimal.  Useful for the JS test to
    /// double-check `sk * Base8 == sk_reduced * Base8`.
    sk_reduced_dec: String,
    /// Whether bit 253 of `sk` is set.
    bit253_set: bool,
}

/// Vector for end-to-end `IdentityAnchor` derivation
/// (master, schema) -> credPriv -> (pk_x, pk_y).
#[derive(Serialize)]
struct IdentityAnchorVector {
    note: &'static str,
    /// Master key as decimal field element (32 LE bytes -> Fp).
    master_dec: String,
    /// Schema hash as decimal field element (32 LE bytes -> Fp).
    schema_dec: String,
    /// Expected `Poseidon(master, schema)` as decimal field element.
    cred_priv_dec: String,
    /// Expected pk_x as decimal field element.
    pk_x_dec: String,
    /// Expected pk_y as decimal field element.
    pk_y_dec: String,
    /// Whether bit 253 of `cred_priv` is set.  Vectors with this flag
    /// true exercise the original `Num2Bits(253)` regression inside
    /// `circomlib::BabyPbk()`.
    bit253_set: bool,
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Convert a 32-byte little-endian buffer to a decimal string by
/// reading as an unsigned integer.  No reduction is applied; caller
/// guarantees the value is canonical (< p) where required.
fn bytes_le_to_dec(bytes: &[u8; 32]) -> String {
    BigUint::from_bytes_le(bytes).to_str_radix(10)
}

/// Convert a BJJ field element (Fq) to decimal string.
fn bjj_fq_to_dec(fq: &BjjFq) -> String {
    BigUint::from_bytes_le(&fq.into_bigint().to_bytes_le()).to_str_radix(10)
}

/// Convert a BJJ scalar (Fr) to decimal string.
fn bjj_fr_to_dec(fr: &BjjFr) -> String {
    BigUint::from_bytes_le(&fr.into_bigint().to_bytes_le()).to_str_radix(10)
}

/// Convert a BN254 Fr to decimal string.
fn bn254_fr_to_dec(fr: &Bn254Fr) -> String {
    BigUint::from_bytes_le(&fr.into_bigint().to_bytes_le()).to_str_radix(10)
}

/// Test whether bit 253 of a 32-byte little-endian value is set.
/// (Bit 253 == byte 31, mask 0b0010_0000.)
fn bit253_set(bytes: &[u8; 32]) -> bool {
    (bytes[31] & 0b0010_0000) != 0
}

/// Pair (pk_x, pk_y) for a private-key scalar `sk` (mod r).  Mirrors
/// `derive_public_key`, but takes an `Fr` directly to share the same
/// `s * Base8` line for both BabyPbk vectors and IdentityAnchor
/// vectors.
fn pk_for_priv(priv_bytes: &[u8; 32]) -> BJJPublicKey {
    derive_public_key(priv_bytes).expect("derive_public_key must succeed")
}

/// Construct the canonical 32-byte LE representation of `Bn254Fr::from(u64)`.
fn small_bn254_bytes(value: u64) -> [u8; 32] {
    let fr = Bn254Fr::from(value);
    let mut out = [0u8; 32];
    let v = fr.into_bigint().to_bytes_le();
    out[..v.len()].copy_from_slice(&v);
    out
}

/// `p - 1` in BN254 scalar field, as canonical 32-byte LE.
fn bn254_p_minus_one_bytes() -> [u8; 32] {
    let neg_one = -Bn254Fr::from(1u64);
    let mut out = [0u8; 32];
    let v = neg_one.into_bigint().to_bytes_le();
    out[..v.len()].copy_from_slice(&v);
    out
}

/// Spread a u32 seed across all 32 bytes via a simple SplitMix-style
/// LCG so that distinct seeds produce distinct buffers.  An earlier
/// version of this helper used `[(s * mul + add) as u8; 32]`, which
/// collapsed the search space to 256 distinct (master, schema) pairs
/// (bytes are u8) -- every `seed_offset` that was a multiple of 256
/// then hit the same first bit-253 match, so the regression vector
/// set was full of duplicates and one of the LT regression vectors
/// even had `a == b` while claiming `expected_lt = 1`.
///
/// This expansion keeps the buffer fully deterministic but lets every
/// `seed_offset` reach a different first match.
fn spread_seed_bytes(seed: u32, mul: u32, add: u32) -> [u8; 32] {
    let mut state = seed.wrapping_mul(mul).wrapping_add(add);
    let mut out = [0u8; 32];
    for byte in out.iter_mut() {
        // SplitMix32 mixing constants (0x9E3779B1 = phi*2^32, the
        // classic Knuth multiplicative hash; 0x7F4A7C15 from
        // SplitMix64). The mixing step doesn't have to be cryptographic
        // -- we only need distinct seeds to produce distinct buffers
        // with low collision probability across our 2048-trial loop.
        state = state.wrapping_mul(0x9E37_79B1).wrapping_add(0x7F4A_7C15);
        *byte = ((state >> 16) & 0xFF) as u8;
    }
    out
}

/// Walk the deterministic seed stream and return the `skip`-th
/// (master, schema, credPriv) triple whose Poseidon credPriv has
/// bit 253 set.  `skip = 0` returns the first match, `skip = 1` the
/// second, etc., so distinct `skip` values are guaranteed to yield
/// distinct triples (the previous offset-based API could collide
/// when the search windows overlapped).
fn find_bit253_master_schema(skip: u32) -> ([u8; 32], [u8; 32], [u8; 32]) {
    const MAX_TRIALS: u32 = 1u32 << 16;
    let mut found: u32 = 0;
    for s in 0u32..MAX_TRIALS {
        let master = spread_seed_bytes(s, 13, 7);
        let schema = spread_seed_bytes(s, 31, 11);
        let cred_priv = poseidon::hash_bytes(&[master, schema]).expect("poseidon");
        if bit253_set(&cred_priv) {
            if found == skip {
                return (master, schema, cred_priv);
            }
            found = found.checked_add(1).expect("found counter overflow");
        }
    }
    panic!(
        "exhausted {MAX_TRIALS} deterministic trials before reaching skip = {skip} \
         (only {found} bit-253-set hits found)"
    );
}

// ─── Vector builders ────────────────────────────────────────────────────────

fn build_lt_vectors() -> Vec<LtVector> {
    let mut out = Vec::new();

    let zero = small_bn254_bytes(0);
    let one = small_bn254_bytes(1);
    let two = small_bn254_bytes(2);
    let p_minus_one = bn254_p_minus_one_bytes();

    // Small ascending: 0 < 1.
    out.push(LtVector {
        note: "small ascending: 0 < 1",
        a_dec: bytes_le_to_dec(&zero),
        b_dec: bytes_le_to_dec(&one),
        expected_lt: 1,
        a_bit253_set: false,
        b_bit253_set: false,
    });
    // Small descending: 1 > 0 (lt = 0).
    out.push(LtVector {
        note: "small descending: 1 > 0",
        a_dec: bytes_le_to_dec(&one),
        b_dec: bytes_le_to_dec(&zero),
        expected_lt: 0,
        a_bit253_set: false,
        b_bit253_set: false,
    });
    // Equal: 5 == 5 (lt = 0).
    let five = small_bn254_bytes(5);
    out.push(LtVector {
        note: "equal: 5 == 5",
        a_dec: bytes_le_to_dec(&five),
        b_dec: bytes_le_to_dec(&five),
        expected_lt: 0,
        a_bit253_set: false,
        b_bit253_set: false,
    });
    // Adjacent: 1 < 2.
    out.push(LtVector {
        note: "adjacent: 1 < 2",
        a_dec: bytes_le_to_dec(&one),
        b_dec: bytes_le_to_dec(&two),
        expected_lt: 1,
        a_bit253_set: false,
        b_bit253_set: false,
    });
    // Edge: 0 < p-1.
    out.push(LtVector {
        note: "edge: 0 < p-1 (b in bit-253 range)",
        a_dec: bytes_le_to_dec(&zero),
        b_dec: bytes_le_to_dec(&p_minus_one),
        expected_lt: 1,
        a_bit253_set: false,
        b_bit253_set: bit253_set(&p_minus_one),
    });
    // Edge: p-1 > 0.
    out.push(LtVector {
        note: "edge: p-1 > 0 (a in bit-253 range)",
        a_dec: bytes_le_to_dec(&p_minus_one),
        b_dec: bytes_le_to_dec(&zero),
        expected_lt: 0,
        a_bit253_set: bit253_set(&p_minus_one),
        b_bit253_set: false,
    });
    // Edge: p-1 == p-1.
    out.push(LtVector {
        note: "edge: p-1 == p-1",
        a_dec: bytes_le_to_dec(&p_minus_one),
        b_dec: bytes_le_to_dec(&p_minus_one),
        expected_lt: 0,
        a_bit253_set: bit253_set(&p_minus_one),
        b_bit253_set: bit253_set(&p_minus_one),
    });

    // Regression: two real Poseidon-output schema hashes, both with
    // bit-253 set.  This is the exact shape of the ordering check
    // inside batch_credential_query.  Compute `expected_lt` from the
    // actual integer comparison so that a future generator change
    // that lands cred_a == cred_b doesn't silently produce a wrong
    // expected value.
    let (_, _, cred_a) = find_bit253_master_schema(0);
    let (_, _, cred_b) = find_bit253_master_schema(1);
    assert!(
        cred_a != cred_b,
        "regression vectors require distinct cred_priv bytes",
    );
    let a_int = BigUint::from_bytes_le(&cred_a);
    let b_int = BigUint::from_bytes_le(&cred_b);
    let (lo, hi, lo_bit253, hi_bit253) = if a_int < b_int {
        (cred_a, cred_b, bit253_set(&cred_a), bit253_set(&cred_b))
    } else {
        (cred_b, cred_a, bit253_set(&cred_b), bit253_set(&cred_a))
    };
    let lo_int = BigUint::from_bytes_le(&lo);
    let hi_int = BigUint::from_bytes_le(&hi);
    let expected_asc = if lo_int < hi_int { 1u8 } else { 0u8 };
    let expected_desc = if hi_int < lo_int { 1u8 } else { 0u8 };
    out.push(LtVector {
        note: "regression: poseidon hash with bit-253 set, ascending",
        a_dec: bytes_le_to_dec(&lo),
        b_dec: bytes_le_to_dec(&hi),
        expected_lt: expected_asc,
        a_bit253_set: lo_bit253,
        b_bit253_set: hi_bit253,
    });
    out.push(LtVector {
        note: "regression: poseidon hash with bit-253 set, descending",
        a_dec: bytes_le_to_dec(&hi),
        b_dec: bytes_le_to_dec(&lo),
        expected_lt: expected_desc,
        a_bit253_set: hi_bit253,
        b_bit253_set: lo_bit253,
    });

    // Deterministic samples from the full domain to cover non-edge
    // pairs.  Seed mixes Poseidon to avoid linear sequences.
    for i in 0u64..6 {
        let a_seed = poseidon::hash_bytes(&[small_bn254_bytes(i * 2 + 1), [0u8; 32]]).unwrap();
        let b_seed = poseidon::hash_bytes(&[small_bn254_bytes(i * 2 + 2), [0u8; 32]]).unwrap();
        let a_int = BigUint::from_bytes_le(&a_seed);
        let b_int = BigUint::from_bytes_le(&b_seed);
        let expected = if a_int < b_int { 1 } else { 0 };
        out.push(LtVector {
            note: "deterministic full-domain sample",
            a_dec: bytes_le_to_dec(&a_seed),
            b_dec: bytes_le_to_dec(&b_seed),
            expected_lt: expected,
            a_bit253_set: bit253_set(&a_seed),
            b_bit253_set: bit253_set(&b_seed),
        });
    }

    out
}

fn build_baby_pbk_vectors() -> Vec<BabyPbkVector> {
    let mut out = Vec::new();

    fn make_vector(note: &'static str, sk_bytes: &[u8; 32]) -> BabyPbkVector {
        let pk = pk_for_priv(sk_bytes);
        // Sanity: every emitted pk must be in the prime-order subgroup,
        // otherwise the circuit's `BabyPbk254` would produce a witness
        // that the on-chain BabyJubJub subgroup check would reject.
        assert!(
            is_in_prime_order_subgroup(&pk),
            "generated pk for {note} is not in the prime-order subgroup",
        );
        let sk_reduced = BjjFr::from_le_bytes_mod_order(sk_bytes);
        BabyPbkVector {
            note,
            sk_dec: bytes_le_to_dec(sk_bytes),
            pk_x_dec: bytes_le_to_dec(&pk.x),
            pk_y_dec: bytes_le_to_dec(&pk.y),
            sk_reduced_dec: bjj_fr_to_dec(&sk_reduced),
            bit253_set: bit253_set(sk_bytes),
        }
    }

    // sk = 1 -> pk = Base8.
    out.push(make_vector(
        "sk = 1 (pk should equal Base8)",
        &small_bn254_bytes(1),
    ));
    // sk = 2.
    out.push(make_vector("sk = 2", &small_bn254_bytes(2)));
    // sk = 7.
    out.push(make_vector("sk = 7", &small_bn254_bytes(7)));
    // sk = 0xCAFE_BABE.
    out.push(make_vector(
        "sk = 0xCAFEBABE",
        &small_bn254_bytes(0xCAFE_BABE),
    ));

    // Regression: sk with bit 253 set.
    out.push(make_vector(
        "regression: bit-253 set (p - 1)",
        &bn254_p_minus_one_bytes(),
    ));

    // Several deterministic Poseidon-derived sks, including at least
    // two with bit 253 set (covers the credentialPrivKey =
    // Poseidon(masterKey, schemaHash) production pathway).
    for skip in 0u32..6 {
        let (_, _, cred_priv) = find_bit253_master_schema(skip);
        out.push(make_vector(
            "regression: poseidon-derived sk with bit-253 set",
            &cred_priv,
        ));
    }
    // ...and a few without bit 253 set.
    for i in 0u64..4 {
        let mut sk = small_bn254_bytes(i + 100);
        sk[3] = 0x42;
        sk[7] = 0x55;
        // Force bit 253 unset by zeroing top byte's bit.
        sk[31] &= 0b0001_1111;
        out.push(make_vector("deterministic sk (bit-253 unset)", &sk));
    }

    out
}

fn build_identity_anchor_vectors() -> Vec<IdentityAnchorVector> {
    let mut out = Vec::new();

    fn make_vector(
        note: &'static str,
        master: &[u8; 32],
        schema: &[u8; 32],
    ) -> IdentityAnchorVector {
        let cred_priv = poseidon::hash_bytes(&[*master, *schema]).expect("poseidon");
        let pk = pk_for_priv(&cred_priv);
        assert!(
            is_in_prime_order_subgroup(&pk),
            "{note}: pk not in prime-order subgroup",
        );
        // master/schema bytes are arbitrary 32-byte buffers and MAY be
        // >= p.  Reduce to canonical Fr first so downstream tests can
        // feed them straight into the circuit as field elements.
        let master_fr = Bn254Fr::from_le_bytes_mod_order(master);
        let schema_fr = Bn254Fr::from_le_bytes_mod_order(schema);
        IdentityAnchorVector {
            note,
            master_dec: bn254_fr_to_dec(&master_fr),
            schema_dec: bn254_fr_to_dec(&schema_fr),
            cred_priv_dec: bytes_le_to_dec(&cred_priv),
            pk_x_dec: bytes_le_to_dec(&pk.x),
            pk_y_dec: bytes_le_to_dec(&pk.y),
            bit253_set: bit253_set(&cred_priv),
        }
    }

    // Plain sample (likely bit-253 unset).
    out.push(make_vector(
        "plain (master = 0x01..., schema = 0x02...)",
        &[0x01u8; 32],
        &[0x02u8; 32],
    ));
    // Master and schema = 0x42... pattern.
    out.push(make_vector(
        "deterministic 0x42 / 0x69 pattern",
        &[0x42u8; 32],
        &[0x69u8; 32],
    ));

    // Three regression vectors with bit-253 set credPriv.  This is the
    // primary phase-3.4 regression class -- pre-fix, witness gen for
    // these would die in `Num2Bits(253)` inside `circomlib::BabyPbk()`.
    for skip in 0u32..3 {
        let (master, schema, _) = find_bit253_master_schema(skip);
        out.push(make_vector(
            "regression: credPriv with bit-253 set",
            &master,
            &schema,
        ));
    }

    // Two more samples that span the full master key space without
    // bit-253 hits.  Like the regression cases above, these use the
    // SplitMix-style `spread_seed_bytes` so distinct seeds yield
    // distinct buffers across all 32 bytes.
    for seed in [3u32, 17u32].iter() {
        let master = spread_seed_bytes(*seed, 7, 1);
        let schema = spread_seed_bytes(*seed, 11, 2);
        // Skip if it happens to land in bit-253 range.
        let cred = poseidon::hash_bytes(&[master, schema]).unwrap();
        if !bit253_set(&cred) {
            out.push(make_vector(
                "deterministic (bit-253 unset)",
                &master,
                &schema,
            ));
        }
    }

    out
}

// ─── Main ───────────────────────────────────────────────────────────────────

fn main() -> anyhow::Result<()> {
    // BN254 scalar field prime.
    let bn254_prime = {
        // -1 + 1 = 0; we want the modulus, so read from `from_bigint`
        // by inspecting `Bn254Fr::MODULUS` via PrimeField trait.
        let modulus_repr = <Bn254Fr as PrimeField>::MODULUS;
        BigUint::from_bytes_le(&modulus_repr.to_bytes_le()).to_str_radix(10)
    };

    // BJJ subgroup order (Fr modulus on twisted Edwards over BN254).
    let bjj_subgroup_order = {
        let modulus_repr = <BjjFr as PrimeField>::MODULUS;
        BigUint::from_bytes_le(&modulus_repr.to_bytes_le()).to_str_radix(10)
    };

    // Base8 generator on BJJ.  We define BASE8 as `derive_public_key(1)`,
    // i.e. the point arkworks's `Base8` resolves to, expressed in
    // the same LE-bytes -> decimal convention used everywhere else in
    // this fixture file.  This guarantees the JSON's BASE8 matches
    // exactly what the circuit will reproduce by setting sk = 1.
    //
    // Cross-check the result against the literal circomlib decimals
    // inlined into `circuits/lib/identity_anchor.circom::BabyPbk254`,
    // numerically.  If arkworks ever changes its generator convention,
    // or if someone retypos the circuit constants, this assertion
    // catches the drift before we emit the fixtures.
    let base8 = {
        let pk_one = pk_for_priv(&small_bn254_bytes(1));
        // Sanity: must be in prime-order subgroup (else circuit's
        // BabyPbk254 would emit a witness rejected by the on-chain
        // BJJ subgroup check).
        assert!(
            is_in_prime_order_subgroup(&pk_one),
            "derive_public_key(1) must be in the BJJ prime-order subgroup",
        );
        let x_dec = bytes_le_to_dec(&pk_one.x);
        let y_dec = bytes_le_to_dec(&pk_one.y);

        // Numeric cross-check vs. the circomlib BASE8 literals used
        // inside `BabyPbk254`.  These must agree, modulo the BN254
        // base field if necessary; arkworks's coordinates are
        // canonical Fq elements so a direct equality check is correct.
        let circomlib_x =
            "5299619240641551281634865583518297030282874472190772894086521144482721001553";
        let circomlib_y =
            "16950150798460657717958625567821834550301663161624707787222815936182638968203";
        assert_eq!(
            x_dec, circomlib_x,
            "BASE8.x mismatch between arkworks and circomlib literal in BabyPbk254"
        );
        assert_eq!(
            y_dec, circomlib_y,
            "BASE8.y mismatch between arkworks and circomlib literal in BabyPbk254"
        );

        // Defence-in-depth: confirm the BJJ contract holds for the
        // emitted bytes.  `is_on_curve` and `is_in_prime_order_subgroup`
        // already apply the circomlib<->arkworks coordinate isomorphism
        // internally, so this is the right surface to assert against.
        assert!(is_on_curve(&pk_one), "BASE8 must lie on BabyJubJub");
        assert!(
            is_in_prime_order_subgroup(&pk_one),
            "BASE8 must be in the BJJ prime-order subgroup",
        );

        BjjPoint { x_dec, y_dec }
    };

    let vectors = CircuitVectors {
        description: "Phase-3.4 circuit ground-truth vectors. \
                      Generated by crates/solid-core/examples/gen_circuit_vectors.rs. \
                      Consumed by circuits/test/{lt_bn254,babypbk254,identity_anchor_derivation}.test.js. \
                      All field elements are decimal strings in [0, p).",
        bn254_prime_dec: bn254_prime,
        bjj_subgroup_order_dec: bjj_subgroup_order,
        bjj_base8: base8,
        lt_bn254: build_lt_vectors(),
        baby_pbk254: build_baby_pbk_vectors(),
        identity_anchor: build_identity_anchor_vectors(),
    };

    // Sanity: every emitted decimal must parse to a BigUint < p.
    let p = BigUint::parse_bytes(vectors.bn254_prime_dec.as_bytes(), 10).unwrap();
    for v in &vectors.lt_bn254 {
        for s in [&v.a_dec, &v.b_dec] {
            let val = BigUint::parse_bytes(s.as_bytes(), 10).unwrap();
            assert!(
                val < p,
                "lt_bn254 vector emitted out-of-domain element: {s}"
            );
        }
    }
    for v in &vectors.baby_pbk254 {
        for s in [&v.sk_dec, &v.pk_x_dec, &v.pk_y_dec] {
            let val = BigUint::parse_bytes(s.as_bytes(), 10).unwrap();
            assert!(
                val < p,
                "baby_pbk254 vector emitted out-of-domain element: {s}"
            );
        }
    }
    for v in &vectors.identity_anchor {
        for s in [
            &v.master_dec,
            &v.schema_dec,
            &v.cred_priv_dec,
            &v.pk_x_dec,
            &v.pk_y_dec,
        ] {
            let val = BigUint::parse_bytes(s.as_bytes(), 10).unwrap();
            assert!(
                val < p,
                "identity_anchor vector emitted out-of-domain element: {s}",
            );
        }
    }

    // Sanity: at least one bit-253-set vector in baby_pbk254 and
    // identity_anchor.  This is the regression class we are pinning
    // down; if it ever drops to zero, the test suite is no longer
    // covering the original failure.
    let lt_bit253 = vectors
        .lt_bn254
        .iter()
        .filter(|v| v.a_bit253_set || v.b_bit253_set)
        .count();
    let pbk_bit253 = vectors.baby_pbk254.iter().filter(|v| v.bit253_set).count();
    let anchor_bit253 = vectors
        .identity_anchor
        .iter()
        .filter(|v| v.bit253_set)
        .count();
    assert!(
        lt_bit253 >= 2,
        "lt_bn254: <2 bit-253 vectors (have {lt_bit253})"
    );
    assert!(
        pbk_bit253 >= 2,
        "baby_pbk254: <2 bit-253 vectors (have {pbk_bit253})"
    );
    assert!(
        anchor_bit253 >= 2,
        "identity_anchor: <2 bit-253 vectors (have {anchor_bit253})"
    );

    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../circuits/test/fixtures");
    fs::create_dir_all(&out_dir)
        .with_context(|| format!("create_dir_all {}", out_dir.display()))?;
    let out_path = out_dir.join("circuit_vectors.json");
    let mut json = serde_json::to_string_pretty(&vectors)?;
    json.push('\n');
    fs::write(&out_path, json).with_context(|| format!("write {}", out_path.display()))?;
    println!(
        "Wrote {} ({} lt + {} babyPbk + {} anchor vectors; bit-253 cov: lt={} pbk={} anchor={})",
        out_path.display(),
        vectors.lt_bn254.len(),
        vectors.baby_pbk254.len(),
        vectors.identity_anchor.len(),
        lt_bit253,
        pbk_bit253,
        anchor_bit253,
    );

    // Suppress unused-warning for these helpers when one of the helpers
    // (e.g. bjj_fq_to_dec) is not used in the current vector set.
    let _ = bjj_fq_to_dec;
    let _ = bn254_fr_to_dec;

    Ok(())
}
