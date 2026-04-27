//! BabyJubJub EdDSA-Poseidon implementation matching circomlib's `eddsaposeidon.circom`.
//!
//! Uses `ark-ed-on-bn254` for curve arithmetic and circomlib-compatible
//! Poseidon for hashing.  Key design: separately managed BJJ identity
//! (NOT derived from Solana wallet).
//!
//! The signing and verification equations match circomlib exactly:
//!   Sign:   S = r + Poseidon(R8.x, R8.y, A.x, A.y, M) * sk
//!   Verify: S * Base8 == R8 + Poseidon(R8.x, R8.y, A.x, A.y, M) * A
//!
//! ## Dual-target split
//!
//! Items reachable from on-chain BPF code paths are dual-target:
//!   - `BJJPublicKey` (just bytes),
//!   - `derive_public_key` / `BJJKeypair::from_private_key`
//!     (pure curve arithmetic, no RNG / no host-only Poseidon),
//!   - `is_in_prime_order_subgroup` / `require_in_prime_order_subgroup`
//!     (SOLID-SEC-007 subgroup check; called from `issuer-registry`),
//!   - `derive_key` (Poseidon-based KDF; uses byte-oriented `hash_bytes`).
//!
//! Items gated host-only (`cfg(not(target_os = "solana"))`):
//!   - `generate_keypair` (uses `OsRng` / `getrandom`),
//!   - `sign` / `verify` (use `poseidon::hash_fr`, the field-element-typed
//!     hasher that pulls `light-poseidon`'s parameter table — host-only,
//!     SOLID-SEC-010),
//!   - `BJJIdentity` and its encrypted-storage machinery (AES-GCM,
//!     Argon2id, `serde_json`, `std::time` — none of which are reached
//!     from BPF).

use ark_ec::{AffineRepr, CurveGroup};
use ark_ed_on_bn254::{EdwardsAffine, EdwardsProjective, Fq, Fr};
use ark_ff::{BigInteger, PrimeField};
use ark_std::Zero;
use serde::{Deserialize, Serialize};

#[cfg(not(target_os = "solana"))]
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
#[cfg(not(target_os = "solana"))]
use ark_ff::UniformRand;
#[cfg(not(target_os = "solana"))]
use blake2::{Blake2b512, Digest};
#[cfg(not(target_os = "solana"))]
use rand::rngs::OsRng;

use crate::error::{Result, SolidError};
use crate::poseidon;

// ─── Constants ─────────────────────────────────────────────────────────────
//
// ## Two flavors of BabyJubJub
//
// `circomlib`'s `babyjub.circom` works in **native** twisted Edwards form
// with curve coefficients
//
//   a = 168700, d = 168696
//
// and a prime-order generator
//
//   Base8.x_circ = 5299619240641551281634865583518297030282874472190772894086521144482721001553
//   Base8.y_circ = 16950150798460657717958625567821834550301663161624707787222815936182638968203
//
// `ark-ed-on-bn254` (the curve we use for arithmetic on the host) works
// in the **isomorphic normalized** form
//
//   a' = 1, d' = d / a = 168696 * inv(168700) mod q
//        = 9706598848417545097372247223557719406784115219466060233080913168975159366771
//
// The isomorphism between the two forms is
//
//   x_ark = sqrt(a) * x_circ    (with our chosen square root of 168700, see `sqrt_a()`)
//   y_ark = y_circ
//
// On every twisted Edwards curve in normalized form
// `x² + y² = 1 + d'·x²·y²`, scalar multiplication, point addition, and
// the standard subgroup check `r·P == 𝒪` are all preserved by this
// isomorphism, so we get arkworks's well-tested arithmetic for free.
//
// What is NOT preserved is the **byte representation** of the point: the
// circuit, the TS SDK (which uses `circomlibjs.babyJub`), and every
// iden3 tool serialize coordinates in circomlib-native form.  All of
// our **byte-level boundaries** (BJJPublicKey, EdDSASignature.r8_x,
// Poseidon hash inputs that ingest BJJ coordinates as field elements)
// MUST therefore be in circomlib-native form, even though the
// arithmetic underneath is in arkworks form.  That is the entire purpose
// of `sqrt_a()` / `sqrt_a_inv()` and the `affine_*_xy_circ` helpers below.
//
// ## How this was caught
//
// At v0.6 e2e bring-up, the (master, schema) → derive_public_key →
// circuit-side `BabyPbk254(s) == on-chain pk` contract failed.  The
// regression-test harness in
// `crates/solid-core/examples/gen_circuit_vectors.rs` cross-checked
// `derive_public_key(1)` against the circomlib literal `Base8` and
// failed on the `x` coordinate while `y` matched — the unmistakable
// signature of an Edwards-form mismatch.  The `phase3_4_base8_*`
// regression tests below pin the fix.
//
// ## Hard invariants (any change MUST keep all of these holding)
//
//   1. `BJJPublicKey` bytes are in **circomlib-native** form.
//   2. `EdDSASignature.r8_x` bytes are in **circomlib-native** form.
//   3. Every Fq fed to a Poseidon hash that the circuit will hash too
//      (issuer-leaf, credential-leaf, EdDSA challenge) is in
//      **circomlib-native** form.
//   4. `base8()` returns the arkworks-form Base8 so that
//      `s · base8()` evaluated by arkworks is consistent with the
//      iso-mapped `s · Base8_circ` evaluated by the circuit.

// ─── Fq constants without `MontFp!` ───────────────────────────────────────
//
// `ark_ff::MontFp!` is a proc-macro that expands to the correct
// Montgomery representation at compile time.  rust-analyzer
// frequently panics on it when its internal rustc version drifts
// from the workspace `rust-toolchain.toml` pin (same failure mode
// as `serde_derive` ABI mismatch, documented in CLAUDE.md).  The
// decimal values below are the **canonical** 32-byte little-endian
// encodings of the same field elements `MontFp!("…")` would have
// produced; `Fq::from_le_bytes_mod_order` is the stable,
// IDE-friendly construction path.  Wire values are unchanged.
//
/// `sqrt(168700) mod q` as canonical LE bytes (decimal
/// `7214280148105020021932206872019688659210616427216992810330019057549499971851`).
/// Changing the square-root choice mirrors every BJJ public key
/// across `x_circ = 0`; this is part of the wire contract.
const SQRT_A_LE: [u8; 32] = [
    0x0b, 0x39, 0x64, 0x11, 0x02, 0x05, 0xc6, 0xef, 0x2e, 0x32, 0xa2, 0xc2, 0x58, 0x27, 0x07, 0x49,
    0xbf, 0xba, 0x91, 0x55, 0xfb, 0x1f, 0x41, 0xd1, 0xf8, 0xa3, 0x38, 0xfb, 0x4a, 0x23, 0xf3, 0x0f,
];

/// `1 / sqrt(168700) mod q` as canonical LE bytes (decimal
/// `2957874849018779266517920829765869116077630550401372566248359756137677864698`).
const SQRT_A_INV_LE: [u8; 32] = [
    0xfa, 0x3e, 0xae, 0x52, 0x38, 0x44, 0xff, 0x8a, 0xa9, 0x01, 0x46, 0x33, 0x69, 0x6a, 0xa0, 0x24,
    0xb5, 0xac, 0x6e, 0x81, 0x7a, 0xcf, 0xe1, 0x8c, 0x1f, 0x3c, 0x56, 0xd4, 0x0b, 0x19, 0x8a, 0x06,
];

/// BabyJubJub Base8: arkworks-normalized `x` as canonical LE bytes (decimal
/// `15863623088992515880085393097393553694825975317405843389771115419751650972659`).
const BASE8_X_ARK_LE: [u8; 32] = [
    0xf3, 0x53, 0x84, 0xef, 0xf6, 0xa0, 0x2b, 0xc4, 0xb9, 0x92, 0x0c, 0x93, 0xfe, 0xb3, 0xd1, 0x4f,
    0xc6, 0x4b, 0xde, 0x39, 0x7e, 0x3c, 0x1a, 0xd1, 0x6e, 0x4e, 0xba, 0x56, 0x13, 0x7e, 0x12, 0x23,
];

/// Base8 `y` (same in circomlib-native and arkworks-normalized form).
const BASE8_Y_LE: [u8; 32] = [
    0x8b, 0x7d, 0x2d, 0x87, 0x7a, 0x25, 0x3c, 0x4b, 0x77, 0x33, 0xe1, 0xb9, 0x1f, 0x05, 0xe0, 0xfc,
    0xed, 0xf9, 0x6b, 0xd1, 0x1c, 0x2e, 0x57, 0x25, 0x49, 0xb2, 0xa0, 0xf7, 0x03, 0x72, 0x79, 0x25,
];

#[inline]
fn sqrt_a() -> Fq {
    Fq::from_le_bytes_mod_order(&SQRT_A_LE)
}

#[inline]
fn sqrt_a_inv() -> Fq {
    Fq::from_le_bytes_mod_order(&SQRT_A_INV_LE)
}

/// Lift a circomlib-native x coordinate to arkworks-normalized form.
#[inline]
fn x_circ_to_ark(x_circ: Fq) -> Fq {
    sqrt_a() * x_circ
}

/// Project an arkworks-normalized x coordinate back to circomlib-native form.
#[inline]
fn x_ark_to_circ(x_ark: Fq) -> Fq {
    sqrt_a_inv() * x_ark
}

/// Decompose an arkworks-form affine point into circomlib-native (x, y).
/// Used at every byte/Poseidon boundary that is also visible to the circuit.
#[inline]
fn affine_to_circomlib_xy(point: &EdwardsAffine) -> (Fq, Fq) {
    (x_ark_to_circ(point.x), point.y)
}

/// BabyJubJub `Base8` in **arkworks-normalized** form.
///
///   x_ark = sqrt(a) · 5299619240641551281634865583518297030282874472190772894086521144482721001553
///         = 15863623088992515880085393097393553694825975317405843389771115419751650972659
///   y     = 16950150798460657717958625567821834550301663161624707787222815936182638968203
///
/// Project to circomlib-native form via `affine_to_circomlib_xy(...)`
/// before serialization or hashing.  See the module-level discussion
/// of the curve-form iso for the why.
fn base8() -> EdwardsAffine {
    let x_ark = Fq::from_le_bytes_mod_order(&BASE8_X_ARK_LE);
    let y = Fq::from_le_bytes_mod_order(&BASE8_Y_LE);
    EdwardsAffine::new_unchecked(x_ark, y)
}

// ─── Types ─────────────────────────────────────────────────────────────────

/// A BabyJubJub public key (point on the curve).
/// Coordinates are in the BN254 scalar field (= BJJ base field).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BJJPublicKey {
    /// X coordinate, 32 bytes little-endian
    pub x: [u8; 32],
    /// Y coordinate, 32 bytes little-endian
    pub y: [u8; 32],
}

/// An EdDSA-Poseidon signature over BabyJubJub.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdDSASignature {
    /// R8 point X coordinate, 32 bytes little-endian
    pub r8_x: [u8; 32],
    /// R8 point Y coordinate, 32 bytes little-endian
    pub r8_y: [u8; 32],
    /// Scalar S, 32 bytes little-endian
    pub s: [u8; 32],
}

/// A BabyJubJub keypair.
#[derive(Clone, Debug)]
pub struct BJJKeypair {
    /// Private key scalar (BJJ scalar field), 32 bytes LE
    pub private_key: [u8; 32],
    /// Corresponding public key
    pub public_key: BJJPublicKey,
}

impl BJJKeypair {
    /// Create a keypair from an existing private key (used for derived keys).
    pub fn from_private_key(private_key: [u8; 32]) -> Result<Self> {
        let public_key = derive_public_key(&private_key)?;
        Ok(Self {
            private_key,
            public_key,
        })
    }
}

// ─── Internal Conversion Helpers ───────────────────────────────────────────

/// Convert Fq (BJJ base field = BN254 scalar field) to 32 bytes LE.
fn fq_to_bytes(fq: &Fq) -> [u8; 32] {
    poseidon::fr_to_bytes_le(fq)
}

/// Convert 32 bytes LE to Fq.
fn bytes_to_fq(bytes: &[u8; 32]) -> Fq {
    poseidon::bytes_le_to_fr(bytes)
}

/// Convert Fr (BJJ scalar field) to 32 bytes LE.
fn fr_to_bytes(fr: &Fr) -> [u8; 32] {
    let bigint = fr.into_bigint();
    let le_vec = bigint.to_bytes_le();
    let mut out = [0u8; 32];
    let len = le_vec.len().min(32);
    out[..len].copy_from_slice(&le_vec[..len]);
    out
}

/// Convert 32 bytes LE to Fr (BJJ scalar field).
fn bytes_to_fr(bytes: &[u8; 32]) -> Fr {
    Fr::from_le_bytes_mod_order(bytes)
}

/// Convert a Fq value (Poseidon output / point coordinate) to an Fr scalar.
/// Reduces modulo the BJJ subgroup order since Fq > Fr.
fn fq_to_fr(fq: &Fq) -> Fr {
    let bytes = fq_to_bytes(fq);
    Fr::from_le_bytes_mod_order(&bytes)
}

/// Convert an affine point to BJJPublicKey bytes.
///
/// Output bytes are in **circomlib-native form** (`x_circ =
/// sqrt_a_inv() * x_ark`, `y` unchanged) so they match what the circuit, the TS
/// SDK (`circomlibjs.babyJub`), and the on-chain Poseidon-leaf hashers
/// expect.  See the curve-form discussion at the top of this module.
fn affine_to_pubkey(point: &EdwardsAffine) -> BJJPublicKey {
    let (x_circ, y) = affine_to_circomlib_xy(point);
    BJJPublicKey {
        x: fq_to_bytes(&x_circ),
        y: fq_to_bytes(&y),
    }
}

/// Convert BJJPublicKey bytes to an arkworks-form affine point.
///
/// Input bytes are in **circomlib-native form**; we apply the iso
/// `x_ark = sqrt_a() * x_circ` before any curve-equation or subgroup
/// check, otherwise circomlib-form points (the only kind we ever
/// transport on the wire) would all look "off-curve" to arkworks.
///
/// Rejects points that are (a) not on the curve, (b) the Edwards
/// neutral element (0, 1) — not a usable signing key, or (c) in a
/// small-order (cofactor-8) subgroup.  (c) is the SOLID-SEC-007
/// invariant.
///
/// `EdwardsAffine::new` has debug `assert!` statements on both the
/// on-curve and subgroup invariants; we use `new_unchecked` and
/// check explicitly so the error path is a structured `SolidError`
/// rather than a panic.
fn pubkey_to_affine(pk: &BJJPublicKey) -> std::result::Result<EdwardsAffine, SolidError> {
    let x_circ = bytes_to_fq(&pk.x);
    let y = bytes_to_fq(&pk.y);
    let point = EdwardsAffine::new_unchecked(x_circ_to_ark(x_circ), y);
    if !point.is_on_curve() || point.is_zero() {
        return Err(SolidError::PointNotOnCurve);
    }
    if !point.is_in_correct_subgroup_assuming_on_curve() {
        return Err(SolidError::BJJNotInSubgroup);
    }
    Ok(point)
}

/// Public predicate: does `pk` (circomlib-form bytes) lie in the BJJ
/// prime-order subgroup?  Returns `false` for off-curve points, the
/// identity, or any point with a non-trivial cofactor-8 component.
/// Delegates to `ark_ec`'s `is_in_correct_subgroup_assuming_on_curve`,
/// which performs the standard `r * P == O` check (`r` is the scalar-
/// field modulus and `P` decomposes uniquely into prime-order and
/// cofactor components).  SOLID-SEC-007.
pub fn is_in_prime_order_subgroup(pk: &BJJPublicKey) -> bool {
    let x_circ = bytes_to_fq(&pk.x);
    let y = bytes_to_fq(&pk.y);
    let point = EdwardsAffine::new_unchecked(x_circ_to_ark(x_circ), y);
    if !point.is_on_curve() || point.is_zero() {
        return false;
    }
    point.is_in_correct_subgroup_assuming_on_curve()
}

/// Cheap on-chain predicate: does `pk` (circomlib-form bytes) decode
/// to a point on the BabyJubJub curve?  Costs a single curve-equation
/// eval (no scalar mul), so fits comfortably in BPF compute budgets.
/// This is the "consolation gate" used by the issuer-registry bypass
/// build (see SEC-048 / `sec007-skip-onchain`); it is NOT a
/// substitute for the full subgroup check, only a coarse sanity
/// filter against off-curve garbage.
pub fn is_on_curve(pk: &BJJPublicKey) -> bool {
    // SOLID-SEC-052 (BPF-runtime regression closeout, 2026-04-28).
    //
    // The previous implementation went through the circomlib<->arkworks
    // iso `EdwardsAffine::new_unchecked(x_circ_to_ark(x_circ), y).is_on_curve()`,
    // which on host x86 evaluates correctly but on the Solana BPF
    // target rejects valid points emitted by `generate_keypair()` (and
    // any honest off-chain caller of the WASM bridge that passed
    // `isInPrimeOrderSubgroup`).  Surfaced 2026-04-28 during e2e
    // bring-up: `bootstrap_issuer.ts -> register_issuer` consistently
    // failed at the consolation gate even though the off-chain
    // predicate accepted the same bytes.  Host test
    // `test_keygen_passes_on_chain_consolation_gate` passes; the
    // regression is BPF-only.
    //
    // Fix: evaluate the circomlib NATIVE twisted Edwards equation
    //   a * x^2 + y^2  ==  1 + d * x^2 * y^2     (a = 168700, d = 168696)
    // directly in `Fq` using only field add / mul, no iso transform,
    // no `EdwardsAffine` constructor, no arkworks `is_on_curve()` call.
    // This is the same equation circomlib's `babyjub.circom` enforces,
    // so a key that lies on the circomlib curve passes here AND in the
    // circuit.  All operations are plain `Fq * Fq` and `Fq + Fq` which
    // arkworks BN254 fields handle identically on host and BPF.
    let x = bytes_to_fq(&pk.x);
    let y = bytes_to_fq(&pk.y);
    let xx = x * x;
    let yy = y * y;
    let a = Fq::from(168700u64);
    let d = Fq::from(168696u64);
    let lhs = a * xx + yy;
    let rhs = Fq::from(1u64) + d * xx * yy;
    lhs == rhs
}

/// Cheap on-chain predicate: is `pk` (circomlib-form bytes) the
/// Edwards neutral element (i.e. unusable as a signing key)?  Same
/// BPF-affordable cost profile as `is_on_curve` and used in the same
/// bypass path.  SEC-048.
///
/// Note: the Edwards neutral element (0, 1) is invariant under the
/// circomlib↔arkworks iso (`x = 0` maps to `x = 0`), so applying the
/// transform here is a no-op for valid identity bytes; we still apply
/// it for symmetry, so any future change to the transform doesn't
/// silently bypass this gate.
pub fn is_identity(pk: &BJJPublicKey) -> bool {
    // SOLID-SEC-052 sibling fix: same BPF-incompat reason as
    // `is_on_curve` above.  In circomlib-native form the Edwards
    // neutral element is `(x = 0, y = 1)` (the iso preserves it
    // because `sqrt(a) * 0 == 0`), so a direct equality check on
    // canonicalised `Fq` values is sufficient and avoids the
    // BPF-broken arkworks constructor path.
    let x = bytes_to_fq(&pk.x);
    let y = bytes_to_fq(&pk.y);
    x == Fq::from(0u64) && y == Fq::from(1u64)
}

/// Fail-closed form of `is_in_prime_order_subgroup`.  Use this at
/// every on-chain or off-chain site that takes a BJJ public key as
/// an untrusted input (issuer registration, signature verification,
/// cross-device import).  Bytes are in **circomlib-native form**.
/// SOLID-SEC-007.
pub fn require_in_prime_order_subgroup(pk: &BJJPublicKey) -> Result<()> {
    let x_circ = bytes_to_fq(&pk.x);
    let y = bytes_to_fq(&pk.y);
    let point = EdwardsAffine::new_unchecked(x_circ_to_ark(x_circ), y);
    if !point.is_on_curve() {
        return Err(SolidError::PointNotOnCurve);
    }
    if point.is_zero() {
        // Identity is not a usable signing key.
        return Err(SolidError::BJJNotInSubgroup);
    }
    if !point.is_in_correct_subgroup_assuming_on_curve() {
        return Err(SolidError::BJJNotInSubgroup);
    }
    Ok(())
}

// ─── Key Generation ────────────────────────────────────────────────────────

/// Generate a fresh BabyJubJub keypair.
///
/// The private key is a random scalar in the BJJ scalar field.
/// The public key is `private_key * Base8`.
///
/// **Host-only.**  Uses `rand::rngs::OsRng` / `getrandom`, which is not
/// available on the Solana BPF runtime.  On-chain code never generates
/// keys; issuers and holders do that off-chain.
#[cfg(not(target_os = "solana"))]
pub fn generate_keypair() -> Result<BJJKeypair> {
    let sk = Fr::rand(&mut OsRng);
    let pk_point = base8().mul_bigint(sk.into_bigint()).into_affine();

    Ok(BJJKeypair {
        private_key: fr_to_bytes(&sk),
        public_key: affine_to_pubkey(&pk_point),
    })
}

/// Derive the public key from a private key.
pub fn derive_public_key(private_key: &[u8; 32]) -> Result<BJJPublicKey> {
    let sk = bytes_to_fr(private_key);
    if sk.is_zero() {
        return Err(SolidError::BJJKey("Private key is zero".into()));
    }
    let pk_point = base8().mul_bigint(sk.into_bigint()).into_affine();
    Ok(affine_to_pubkey(&pk_point))
}

/// Phase 1.1: Derive a deterministic sub-key from a master key and context.
///
/// credentialKey = Poseidon(masterKey, schemaHash)
/// nullifierKey  = Poseidon(masterKey, verifierAddress)
pub fn derive_key(master_key: &[u8; 32], context: &[u8; 32]) -> Result<[u8; 32]> {
    poseidon::hash_bytes(&[*master_key, *context])
}

// ─── EdDSA-Poseidon Signing ───────────────────────────────────────────────

/// Sign a message (field element as bytes) using EdDSA-Poseidon.
///
/// Algorithm matches circomlib's `eddsaposeidon.circom` verification:
///   1. r = deterministic_nonce(sk, msg)
///   2. R8 = r * Base8
///   3. h = Poseidon(R8.x_circ, R8.y, A.x_circ, A.y, msg) reduced to BJJ scalar
///   4. S = r + h * sk
///
/// The circuit verifies: S · Base8 == R8 + h · A
///
/// **Coord form note.**  Curve arithmetic happens in arkworks-normalized
/// form, but all four BJJ coordinates fed to Poseidon as well as the
/// `r8_x` byte field of the returned `EdDSASignature` are in
/// **circomlib-native form**.  This is what lets the same signature
/// verify in `circuits/lib/eddsa_verifier.circom` and in this Rust
/// `verify()` against the same `BJJPublicKey` bytes.
///
/// **Host-only.**  Uses `Blake2b512` for the deterministic nonce and
/// `poseidon::hash_fr` (host-only field-element hasher) for the
/// challenge.  Issuance signing happens off-chain at the issuer; the
/// chain only verifies the resulting signature inside the ZK circuit.
#[cfg(not(target_os = "solana"))]
pub fn sign(private_key: &[u8; 32], message: &[u8; 32]) -> Result<EdDSASignature> {
    let sk = bytes_to_fr(private_key);
    if sk.is_zero() {
        return Err(SolidError::Signature("Private key is zero".into()));
    }

    let b8 = base8();
    let pk_point = b8.mul_bigint(sk.into_bigint()).into_affine();

    // Deterministic nonce: r = Blake2b(sk || msg) reduced to BJJ scalar field
    let mut hasher = Blake2b512::new();
    hasher.update(private_key);
    hasher.update(message);
    let nonce_hash = hasher.finalize();
    let r = Fr::from_le_bytes_mod_order(&nonce_hash[..]);

    // R8 = r * Base8 (computed in arkworks-normalized form)
    let r8_point = b8.mul_bigint(r.into_bigint()).into_affine();

    // Project R8 and A into circomlib-native form for hashing/serialization.
    let (r8_x_circ, r8_y_circ) = affine_to_circomlib_xy(&r8_point);
    let (pk_x_circ, pk_y_circ) = affine_to_circomlib_xy(&pk_point);

    // Challenge hash uses **circomlib-native** coordinates so the circuit
    // (which only ever sees circomlib coords) computes the same `h`.
    let msg_fq = bytes_to_fq(message);
    let h_fq = poseidon::hash_fr(&[r8_x_circ, r8_y_circ, pk_x_circ, pk_y_circ, msg_fq])?;

    // Reduce hash to BJJ scalar field (Fq > Fr, need mod reduction)
    let h = fq_to_fr(&h_fq);

    // S = r + h * sk (mod BJJ subgroup order)
    let s = r + h * sk;

    Ok(EdDSASignature {
        r8_x: fq_to_bytes(&r8_x_circ),
        r8_y: fq_to_bytes(&r8_y_circ),
        s: fr_to_bytes(&s),
    })
}

/// Verify an EdDSA-Poseidon signature.
///
/// Checks: S · Base8 == R8 + Poseidon(R8.x_circ, R8.y, A.x_circ, A.y, msg) · A
///
/// `BJJPublicKey` and `EdDSASignature.r8_x` bytes are interpreted in
/// **circomlib-native form**; we lift to arkworks-normalized form for
/// the curve check.  See the curve-form discussion at the top of this
/// module.
///
/// **Host-only.**  Uses `poseidon::hash_fr` (host-only field-element
/// hasher).  On-chain verification happens inside the ZK circuit
/// (`circuits/batch_credential_query.circom`); this function is for
/// off-chain test vectors and SDK self-checks.
#[cfg(not(target_os = "solana"))]
pub fn verify(
    public_key: &BJJPublicKey,
    message: &[u8; 32],
    signature: &EdDSASignature,
) -> Result<bool> {
    // pubkey_to_affine applies the circomlib→arkworks lift internally
    // and runs on-curve + subgroup gates.
    let pk_point = pubkey_to_affine(public_key)?;
    let (pk_x_circ, pk_y_circ) = affine_to_circomlib_xy(&pk_point);

    // R8 bytes are circomlib-form on the wire; lift to arkworks form
    // for the curve check.  Sign() derives R8 = r · Base8, which is
    // always in the prime-order subgroup, so a signature whose R8 has
    // a cofactor-8 component is malformed and we fail-closed
    // (SOLID-SEC-007).
    let r8_x_circ = bytes_to_fq(&signature.r8_x);
    let r8_y_circ = bytes_to_fq(&signature.r8_y);
    let r8_point = EdwardsAffine::new_unchecked(x_circ_to_ark(r8_x_circ), r8_y_circ);
    if !r8_point.is_on_curve() {
        return Err(SolidError::PointNotOnCurve);
    }
    if !r8_point.is_in_correct_subgroup_assuming_on_curve() {
        return Err(SolidError::BJJNotInSubgroup);
    }

    // Reconstruct S scalar
    let s = bytes_to_fr(&signature.s);

    // Challenge hash uses **circomlib-native** coordinates.
    let msg_fq = bytes_to_fq(message);
    let h_fq = poseidon::hash_fr(&[r8_x_circ, r8_y_circ, pk_x_circ, pk_y_circ, msg_fq])?;
    let h = fq_to_fr(&h_fq);

    // LHS: S · Base8
    let b8 = base8();
    let lhs = b8.mul_bigint(s.into_bigint()).into_affine();

    // RHS: R8 + h · A
    let h_a = pk_point.mul_bigint(h.into_bigint());
    let rhs = (EdwardsProjective::from(r8_point) + h_a).into_affine();

    Ok(lhs == rhs)
}

// ─── Encrypted Key Storage (host-only) ─────────────────────────────────────
//
// AES-256-GCM, Argon2id, `serde_json`, and `std::time::SystemTime` are not
// available on the Solana BPF runtime.  None of this is reached from
// on-chain code paths -- holders generate / unlock identities client-side.

/// Portable BJJ identity bundle with encrypted private key.
///
/// The private key is encrypted with AES-256-GCM using a passphrase-derived
/// key (Argon2id). This enables secure storage and cross-device transfer.
#[cfg(not(target_os = "solana"))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BJJIdentity {
    pub version: u8,
    pub public_key: BJJPublicKey,
    pub encrypted_private_key: EncryptedKey,
    pub metadata: IdentityMetadata,
}

#[cfg(not(target_os = "solana"))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedKey {
    pub ciphertext: Vec<u8>,
    pub nonce: [u8; 12],
    pub salt: [u8; 32],
}

#[cfg(not(target_os = "solana"))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IdentityMetadata {
    pub created_at: u64,
    pub wallet_bindings: Vec<[u8; 32]>,
    pub rotated_from: Option<BJJPublicKey>,
}

#[cfg(not(target_os = "solana"))]
impl BJJIdentity {
    /// Generate a new identity with a fresh BJJ keypair.
    /// Private key is encrypted with the given passphrase via Argon2id + AES-256-GCM.
    pub fn generate(passphrase: &[u8]) -> Result<Self> {
        let keypair = generate_keypair()?;

        // Derive encryption key from passphrase via Argon2id
        let salt: [u8; 32] = rand::random();
        let encryption_key = Self::derive_key(passphrase, &salt)?;

        // Encrypt private key with AES-256-GCM
        let cipher = Aes256Gcm::new_from_slice(&encryption_key)
            .map_err(|e| SolidError::Encryption(format!("Cipher init: {}", e)))?;
        let nonce_bytes: [u8; 12] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, keypair.private_key.as_ref())
            .map_err(|e| SolidError::Encryption(format!("Encrypt: {}", e)))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(Self {
            version: 1,
            public_key: keypair.public_key,
            encrypted_private_key: EncryptedKey {
                ciphertext,
                nonce: nonce_bytes,
                salt,
            },
            metadata: IdentityMetadata {
                created_at: now,
                wallet_bindings: vec![],
                rotated_from: None,
            },
        })
    }

    /// Decrypt and return the BJJ private key.
    pub fn unlock(&self, passphrase: &[u8]) -> Result<[u8; 32]> {
        let encryption_key = Self::derive_key(passphrase, &self.encrypted_private_key.salt)?;
        let cipher = Aes256Gcm::new_from_slice(&encryption_key)
            .map_err(|e| SolidError::Decryption(format!("Cipher init: {}", e)))?;
        let nonce = Nonce::from_slice(&self.encrypted_private_key.nonce);
        let plaintext = cipher
            .decrypt(nonce, self.encrypted_private_key.ciphertext.as_ref())
            .map_err(|e| SolidError::Decryption(format!("Decrypt: {}", e)))?;

        let mut key = [0u8; 32];
        if plaintext.len() != 32 {
            return Err(SolidError::Decryption("Invalid key length".into()));
        }
        key.copy_from_slice(&plaintext);
        Ok(key)
    }

    /// Bind this identity to a Solana wallet public key.
    pub fn bind_wallet(&mut self, wallet_pubkey: [u8; 32]) {
        if !self.metadata.wallet_bindings.contains(&wallet_pubkey) {
            self.metadata.wallet_bindings.push(wallet_pubkey);
        }
    }

    /// Export as JSON string for cross-device transfer.
    pub fn export_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| SolidError::Serialization(e.to_string()))
    }

    /// Import from JSON string.
    pub fn import_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(|e| SolidError::Serialization(e.to_string()))
    }

    /// Derive an AES-256 encryption key from a passphrase using Argon2id.
    fn derive_key(passphrase: &[u8], salt: &[u8; 32]) -> Result<[u8; 32]> {
        let params = argon2::Params::new(65536, 3, 4, Some(32))
            .map_err(|e| SolidError::Encryption(format!("Argon2 params: {}", e)))?;
        let argon2 =
            argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
        let mut key = [0u8; 32];
        argon2
            .hash_password_into(passphrase, salt, &mut key)
            .map_err(|e| SolidError::Encryption(format!("Argon2: {}", e)))?;
        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_ec::Group;

    #[test]
    fn test_keygen_produces_valid_keypair() {
        let kp = generate_keypair().unwrap();
        assert_ne!(kp.private_key, [0u8; 32]);
        assert_ne!(kp.public_key.x, [0u8; 32]);
        assert_ne!(kp.public_key.y, [0u8; 32]);
    }

    #[test]
    fn test_keygen_passes_on_chain_consolation_gate() {
        // Mirror of the on-chain `register_issuer` consolation gate
        // under `sec007-skip-onchain`: every freshly generated keypair
        // MUST satisfy `is_on_curve(pk) && !is_identity(pk)` AND the
        // stricter `is_in_prime_order_subgroup(pk)`.  Regression gate
        // for the e2e bootstrap_issuer failure 2026-04-28: on-chain
        // is_on_curve was rejecting WASM-generated keys that off-chain
        // isInPrimeOrderSubgroup accepted -- if this host test passes,
        // the regression is BPF-specific (arkworks compilation), not
        // a logic bug.
        for _ in 0..16 {
            let kp = generate_keypair().unwrap();
            assert!(
                is_on_curve(&kp.public_key),
                "is_on_curve must accept generated key (x={:?}, y={:?})",
                hex::encode(kp.public_key.x),
                hex::encode(kp.public_key.y),
            );
            assert!(
                !is_identity(&kp.public_key),
                "is_identity must reject generated key"
            );
            assert!(
                is_in_prime_order_subgroup(&kp.public_key),
                "is_in_prime_order_subgroup must accept generated key"
            );
        }
    }

    #[test]
    fn test_derive_public_key_deterministic() {
        let kp = generate_keypair().unwrap();
        let pk2 = derive_public_key(&kp.private_key).unwrap();
        assert_eq!(kp.public_key, pk2);
    }

    #[test]
    fn test_derive_rejects_zero_key() {
        assert!(derive_public_key(&[0u8; 32]).is_err());
    }

    #[test]
    fn test_sign_verify_roundtrip() {
        let kp = generate_keypair().unwrap();
        let msg = poseidon::fr_to_bytes_le(&ark_bn254::Fr::from(42u64));
        let sig = sign(&kp.private_key, &msg).unwrap();
        let valid = verify(&kp.public_key, &msg, &sig).unwrap();
        assert!(valid, "Signature should verify for correct key+message");
    }

    #[test]
    fn test_sign_verify_wrong_message() {
        let kp = generate_keypair().unwrap();
        let msg1 = poseidon::fr_to_bytes_le(&ark_bn254::Fr::from(42u64));
        let msg2 = poseidon::fr_to_bytes_le(&ark_bn254::Fr::from(99u64));
        let sig = sign(&kp.private_key, &msg1).unwrap();
        let valid = verify(&kp.public_key, &msg2, &sig).unwrap();
        assert!(!valid, "Signature should NOT verify for wrong message");
    }

    #[test]
    fn test_sign_verify_wrong_key() {
        let kp1 = generate_keypair().unwrap();
        let kp2 = generate_keypair().unwrap();
        let msg = poseidon::fr_to_bytes_le(&ark_bn254::Fr::from(42u64));
        let sig = sign(&kp1.private_key, &msg).unwrap();
        let valid = verify(&kp2.public_key, &msg, &sig).unwrap();
        assert!(!valid, "Signature should NOT verify for wrong key");
    }

    #[test]
    fn test_sign_deterministic() {
        let kp = generate_keypair().unwrap();
        let msg = poseidon::fr_to_bytes_le(&ark_bn254::Fr::from(42u64));
        let sig1 = sign(&kp.private_key, &msg).unwrap();
        let sig2 = sign(&kp.private_key, &msg).unwrap();
        assert_eq!(sig1, sig2, "Same key+message should produce same signature");
    }

    #[test]
    fn test_identity_generate_and_unlock() {
        let identity = BJJIdentity::generate(b"test-passphrase-123").unwrap();
        let sk = identity.unlock(b"test-passphrase-123").unwrap();
        // Verify the unlocked key matches the public key
        let pk = derive_public_key(&sk).unwrap();
        assert_eq!(identity.public_key, pk);
    }

    #[test]
    fn test_identity_wrong_passphrase_fails() {
        let identity = BJJIdentity::generate(b"correct-password").unwrap();
        let result = identity.unlock(b"wrong-password");
        assert!(result.is_err());
    }

    #[test]
    fn test_identity_json_roundtrip() {
        let identity = BJJIdentity::generate(b"passphrase").unwrap();
        let json = identity.export_json().unwrap();
        let recovered = BJJIdentity::import_json(&json).unwrap();
        assert_eq!(identity.public_key, recovered.public_key);
        assert_eq!(identity.version, recovered.version);
    }

    #[test]
    fn test_identity_wallet_binding() {
        let mut identity = BJJIdentity::generate(b"pass").unwrap();
        let wallet = [42u8; 32];
        identity.bind_wallet(wallet);
        identity.bind_wallet(wallet); // duplicate
        assert_eq!(identity.metadata.wallet_bindings.len(), 1);
        assert_eq!(identity.metadata.wallet_bindings[0], wallet);
    }

    // ─── SOLID-SEC-007: BJJ subgroup check regression gate ─────────────────

    /// Every honestly-generated keypair's public key is in the prime-
    /// order subgroup, because `generate_keypair` derives it as
    /// `sk * Base8` where Base8 is the cofactor-cleared generator.
    #[test]
    fn test_subgroup_accepts_honest_keypair() {
        for _ in 0..8 {
            let kp = generate_keypair().unwrap();
            assert!(
                is_in_prime_order_subgroup(&kp.public_key),
                "honest keypair pubkey must lie in prime-order subgroup"
            );
            assert!(require_in_prime_order_subgroup(&kp.public_key).is_ok());
        }
    }

    /// The Edwards neutral element `(0, 1)` is the identity.  It is
    /// trivially in every subgroup but is not a usable signing key;
    /// `require_in_prime_order_subgroup` rejects it fail-closed.
    #[test]
    fn test_subgroup_rejects_identity() {
        let mut y_bytes = [0u8; 32];
        y_bytes[0] = 1; // Fq::one() in little-endian
        let identity_pk = BJJPublicKey {
            x: [0u8; 32],
            y: y_bytes,
        };
        assert!(matches!(
            require_in_prime_order_subgroup(&identity_pk),
            Err(SolidError::BJJNotInSubgroup)
        ));
    }

    /// `(0, -1)` is a known order-2 point on BJJ (doubles to the
    /// identity).  It sits entirely in the cofactor-8 subgroup and
    /// must be rejected by the subgroup check.
    #[test]
    fn test_subgroup_rejects_order_two_point() {
        let neg_one_fq = -Fq::from(1u64);
        let neg_one_bytes = fq_to_bytes(&neg_one_fq);
        let order_two_pk = BJJPublicKey {
            x: [0u8; 32],
            y: neg_one_bytes,
        };
        // First sanity: this point must be on curve (Edwards equation
        // -x^2 + y^2 = 1 + d*x^2*y^2 with x=0, y=-1: LHS = 1, RHS = 1).
        let x = bytes_to_fq(&order_two_pk.x);
        let y = bytes_to_fq(&order_two_pk.y);
        let point = EdwardsAffine::new_unchecked(x, y);
        assert!(point.is_on_curve(), "(0, -1) must lie on BJJ");
        // Doubling (0, -1) yields the identity.
        let doubled = EdwardsProjective::from(point).double().into_affine();
        assert!(doubled.is_zero(), "(0, -1) is 2-torsion");
        // Core invariant: subgroup check must reject it.
        assert!(!is_in_prime_order_subgroup(&order_two_pk));
        assert!(matches!(
            require_in_prime_order_subgroup(&order_two_pk),
            Err(SolidError::BJJNotInSubgroup)
        ));
    }

    /// Off-curve bytes must fail with `PointNotOnCurve`, not
    /// `BJJNotInSubgroup` -- the distinction matters for debugging.
    #[test]
    fn test_subgroup_rejects_off_curve_point() {
        let off_curve = BJJPublicKey {
            x: [7u8; 32],
            y: [11u8; 32],
        };
        assert!(!is_in_prime_order_subgroup(&off_curve));
        assert!(matches!(
            require_in_prime_order_subgroup(&off_curve),
            Err(SolidError::PointNotOnCurve)
        ));
    }

    /// `verify()` must reject a signature whose R8 component is a
    /// small-order point, even if the algebra would otherwise
    /// accept.  This is defence-in-depth; honest signing never
    /// produces such R8 values.
    #[test]
    fn test_verify_rejects_small_order_r8() {
        let kp = generate_keypair().unwrap();
        let msg = poseidon::fr_to_bytes_le(&ark_bn254::Fr::from(42u64));
        let sig = sign(&kp.private_key, &msg).unwrap();
        // Replace the (honest) R8 with the order-2 point (0, -1).
        let neg_one_fq = -Fq::from(1u64);
        let mut malformed = sig.clone();
        malformed.r8_x = [0u8; 32];
        malformed.r8_y = fq_to_bytes(&neg_one_fq);
        match verify(&kp.public_key, &msg, &malformed) {
            Err(SolidError::BJJNotInSubgroup) => {}
            Ok(_) | Err(_) => panic!("verify must reject small-order R8 with BJJNotInSubgroup"),
        }
    }

    // ─── Phase-3.4: BASE8 must match circomlib literal ─────────────────────
    //
    // Load-bearing.  Every BabyJubJub key derived by this SDK -- issuer
    // signing keys, holder credential keys, EdDSA R8 -- is computed as
    // `s * Base8`.  The in-circuit `BabyPbk254` template (see
    // `circuits/lib/identity_anchor.circom`) hard-codes the same Base8
    // literal.  If they ever drift, every off-chain pubkey lands on a
    // coset rotated from the on-chain expectation and every proof fails
    // (silently up to the leaf-hash mismatch deep inside the verifier).
    //
    // Pre-fix history: prior to v0.6 e2e bring-up, `base8()` was computed
    // as `8 * EdwardsProjective::generator()` on the (incorrect) belief
    // that arkworks' GENERATOR was the cofactor-8 full-curve generator.
    // It is in fact a different prime-order generator, so multiplying by
    // 8 yielded yet another prime-order point disjoint from circomlib's
    // Base8.  This regression test pins the literal so it cannot drift.

    /// Decimal string -> 32-byte little-endian Fq encoding, used to
    /// pin the literal coordinates the same way production code uses
    /// `Fq::from_le_bytes_mod_order` (no `MontFp!` in tests either).
    fn fq_from_dec_le_bytes(dec: &str) -> [u8; 32] {
        use num_bigint::BigUint;
        let n = BigUint::parse_bytes(dec.as_bytes(), 10).expect("decimal");
        let mut le = n.to_bytes_le();
        le.resize(32, 0u8);
        let mut out = [0u8; 32];
        out.copy_from_slice(&le);
        out
    }

    #[test]
    fn phase3_4_base8_matches_circomlib_literal() {
        const BASE8_X: &str =
            "5299619240641551281634865583518297030282874472190772894086521144482721001553";
        const BASE8_Y: &str =
            "16950150798460657717958625567821834550301663161624707787222815936182638968203";

        let b8 = base8();

        // (1) On the curve.  `base8()` returns the arkworks-normalized
        // form, so `is_on_curve` and the subgroup check both run on
        // arkworks-form coordinates with no transform required.
        assert!(b8.is_on_curve(), "Base8 must lie on the BabyJubJub curve");

        // (2) Non-identity.
        assert!(!b8.is_zero(), "Base8 must not be the Edwards neutral");

        // (3) In the prime-order subgroup.
        assert!(
            b8.is_in_correct_subgroup_assuming_on_curve(),
            "Base8 must lie in the prime-order subgroup"
        );

        // (4) Coordinate match against circomlib literal in
        // **circomlib-native form** -- the only form anyone outside
        // this Rust file ever sees.  We project via the iso first.
        let (b8_x_circ, b8_y_circ) = affine_to_circomlib_xy(&b8);
        let expected_x = fq_from_dec_le_bytes(BASE8_X);
        let expected_y = fq_from_dec_le_bytes(BASE8_Y);
        assert_eq!(
            fq_to_bytes(&b8_x_circ),
            expected_x,
            "Base8.x_circ mismatch vs circomlib literal -- SDK ↔ circuit divergence reopened",
        );
        assert_eq!(
            fq_to_bytes(&b8_y_circ),
            expected_y,
            "Base8.y mismatch vs circomlib literal -- SDK ↔ circuit divergence reopened",
        );
    }

    /// Pinned vector: derive_public_key(1) must equal Base8 itself
    /// (in circomlib-native form, which is what `BJJPublicKey` carries),
    /// and Base8.{x_circ, y} must equal the circomlib literal decimals.
    /// Belt-and-braces against any future refactor that swaps the
    /// generator on one side of `derive_public_key` only.
    #[test]
    fn phase3_4_derive_public_key_one_equals_base8() {
        const BASE8_X: &str =
            "5299619240641551281634865583518297030282874472190772894086521144482721001553";
        const BASE8_Y: &str =
            "16950150798460657717958625567821834550301663161624707787222815936182638968203";

        let mut sk = [0u8; 32];
        sk[0] = 1;
        let pk = derive_public_key(&sk).expect("sk=1 must derive");

        // BJJPublicKey is in circomlib-native form by construction.
        assert_eq!(pk.x, fq_from_dec_le_bytes(BASE8_X));
        assert_eq!(pk.y, fq_from_dec_le_bytes(BASE8_Y));

        // And it equals affine_to_pubkey(base8()) under the same projection.
        let b8 = base8();
        assert_eq!(pk, affine_to_pubkey(&b8));
    }

    // ─── Phase-3.4: circuit ↔ Rust contract regression gate ────────────────
    //
    // These tests pin down the contract that the in-circuit `BabyPbk254`
    // template (defined in `circuits/lib/identity_anchor.circom`) must
    // produce the same Edwards point as `derive_public_key` for any
    // 32-byte little-endian scalar in [0, p).  Specifically:
    //
    //   * Rust  : `derive_public_key(s)` interprets `s` LE-mod-r as an
    //              Fr scalar, then computes `s * Base8`.
    //   * Circuit: `BabyPbk254` decomposes `s` into 254 canonical bits
    //              (`Num2Bits_strict`) then runs `EscalarMulFix(254, BASE8)`
    //              -- equivalent to computing `s * Base8` in the curve
    //              group.
    //
    // Because Base8 has order r, `s * Base8 == (s mod r) * Base8`, so the
    // outputs agree byte-for-byte.  These tests do not run the circuit
    // (that lives under `circuits/test/`); they pin down the Rust side
    // of the contract so a future drift gets caught at `cargo test`.
    //
    // Pre-fix history: `circomlib::BabyPbk()` used `Num2Bits(253)` which
    // hard-asserts on inputs whose 253rd bit is set.  ~33% of Poseidon(2)
    // outputs hit that branch.  The Rust impl never had this bug because
    // `Fr::from_le_bytes_mod_order` reduces before scalar multiplication.

    /// Helper: little-endian scalar -> 32-byte private key.  Caller
    /// guarantees the value is in [0, p).
    fn fr_le_bytes(value: ark_bn254::Fr) -> [u8; 32] {
        let mut out = [0u8; 32];
        let bigint = value.into_bigint();
        let v = bigint.to_bytes_le();
        out[..v.len()].copy_from_slice(&v);
        out
    }

    /// Helper: 32-byte little-endian -> scalar mod r (BJJ subgroup order).
    fn priv_to_fr(bytes: &[u8; 32]) -> Fr {
        Fr::from_le_bytes_mod_order(bytes)
    }

    /// derive_public_key must accept private keys whose 254-bit scalar
    /// value has bit 253 set.  This is the regression vector for the
    /// `Num2Bits(253)` failure that broke the in-circuit `BabyPbk()`.
    /// We construct such a value explicitly (`p - 1`, definitely > 2^253),
    /// confirm it is non-zero mod r, and verify Rust derives a valid,
    /// in-subgroup public key.
    #[test]
    fn phase3_4_derive_public_key_accepts_bit253_set_scalar() {
        // p - 1 in BN254 scalar field = ark_bn254::Fr's largest representable.
        let mod_minus_one = -ark_bn254::Fr::from(1u64);
        let priv_bytes = fr_le_bytes(mod_minus_one);

        // Sanity: bit 253 of the integer-value is set.
        assert!(
            (priv_bytes[31] & 0b0010_0000) != 0,
            "p-1 must have bit 253 set (was: 0x{:02x})",
            priv_bytes[31]
        );

        // Reduced mod r is non-zero, otherwise the curve op would yield
        // the identity point.
        assert!(!priv_to_fr(&priv_bytes).is_zero());

        let pk = derive_public_key(&priv_bytes).expect("must succeed");
        assert!(
            is_in_prime_order_subgroup(&pk),
            "derived pk must lie in the prime-order subgroup"
        );
        assert!(
            !is_identity(&pk),
            "p-1 reduced mod r is non-trivial, pk must be non-identity"
        );
    }

    /// derive_public_key(s) must equal derive_public_key(s mod r).  This
    /// is the algebraic identity that lets the circuit's `BabyPbk254`
    /// (which never reduces) and Rust's `derive_public_key` (which
    /// reduces via `from_le_bytes_mod_order`) agree byte-for-byte.
    #[test]
    fn phase3_4_derive_public_key_equals_after_reduction() {
        // Pick scalar with bit 253 set (regression case).
        let mod_minus_one = -ark_bn254::Fr::from(1u64);
        let priv_raw = fr_le_bytes(mod_minus_one);
        let pk_raw = derive_public_key(&priv_raw).unwrap();

        // Reduce mod r explicitly and re-derive.
        let reduced = priv_to_fr(&priv_raw);
        let priv_reduced = fr_to_bytes(&reduced);
        let pk_reduced = derive_public_key(&priv_reduced).unwrap();

        assert_eq!(
            pk_raw, pk_reduced,
            "raw 254-bit scalar and (s mod r) must derive the same point"
        );
    }

    /// For at least one (master, schema) pair sampled from a small
    /// reproducible space, Poseidon(master, schema) has bit 253 set.
    /// This pins down the empirical claim in the BabyPbk254 docstring
    /// (~33% of Poseidon(2) outputs span the [2^253, p) range).
    ///
    /// If this test ever fails to find a hit, our circuit fix is
    /// not actually being exercised by the deterministic regression
    /// vectors and we need to update the search space.
    #[test]
    fn phase3_4_poseidon_can_produce_bit253_set_scalar() {
        let mut found = false;
        let mut sample = [0u8; 32];
        for i in 0u32..256 {
            let master = [i as u8; 32];
            let schema = [(i.wrapping_mul(7)) as u8; 32];
            let cred_priv = poseidon::hash_bytes(&[master, schema]).unwrap();
            if (cred_priv[31] & 0b0010_0000) != 0 {
                found = true;
                sample = cred_priv;
                break;
            }
        }
        assert!(
            found,
            "expected at least one (master, schema) sample with credentialPrivKey \
             bit 253 set in 256 trials; if this fails, expand search space"
        );

        // Smoke: such a scalar still derives a valid pk.
        let pk = derive_public_key(&sample).unwrap();
        assert!(is_in_prime_order_subgroup(&pk));
    }

    /// Cross-check that derive_public_key matches `Base8.mul_bigint(sk)` directly.
    /// (Belt-and-braces: catches accidental swap of generator or coordinate
    /// system in any future refactor of `derive_public_key`.)
    #[test]
    fn phase3_4_derive_matches_explicit_scalar_mul() {
        for seed in 1u64..16u64 {
            let sk_fr = Fr::from(seed * 0x1234_5678);
            let priv_bytes = fr_le_bytes(ark_bn254::Fr::from(seed * 0x1234_5678));
            // Note: Fr (BJJ) and ark_bn254::Fr have different moduli, so
            // we round-trip via priv_bytes to keep semantics aligned with
            // `derive_public_key`'s `bytes_to_fr` step.
            let pk_via_api = derive_public_key(&priv_bytes).unwrap();

            let expected_point = base8().mul_bigint(sk_fr.into_bigint()).into_affine();
            let pk_explicit = affine_to_pubkey(&expected_point);
            assert_eq!(pk_via_api, pk_explicit);
        }
    }

    /// Property test: for many (master, schema) pairs, every derived
    /// per-schema public key is on the curve and in the prime-order
    /// subgroup.  Pre-Phase-3.4, this was vacuously true because
    /// the in-circuit BabyPbk would have rejected ~33% of cases at
    /// witness time -- Rust still computed the point fine, but
    /// off-chain SDK and on-chain proof would have desynced.
    #[test]
    fn phase3_4_all_derived_credential_pubkeys_in_subgroup() {
        let mut bit253_count = 0u32;
        for i in 0u32..128 {
            let master = [(i.wrapping_mul(13).wrapping_add(7)) as u8; 32];
            let schema = [(i.wrapping_mul(31).wrapping_add(11)) as u8; 32];
            let cred_priv = poseidon::hash_bytes(&[master, schema]).unwrap();
            if (cred_priv[31] & 0b0010_0000) != 0 {
                bit253_count += 1;
            }
            let pk = derive_public_key(&cred_priv).unwrap();
            assert!(
                is_in_prime_order_subgroup(&pk),
                "iter {} produced pk outside the prime-order subgroup",
                i
            );
        }
        // We expect roughly 33% of the sample to hit the regression
        // window.  Allow a wide tolerance (>= 1) because this test is a
        // probabilistic existence check; the strict assertion that every
        // pk is in the subgroup is what really matters.
        assert!(
            bit253_count >= 1,
            "no bit-253-set scalar seen across 128 trials; regression coverage gap"
        );
    }
}
