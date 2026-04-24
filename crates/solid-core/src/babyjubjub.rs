//! BabyJubJub EdDSA-Poseidon implementation matching circomlib's `eddsaposeidon.circom`.
//!
//! Uses `ark-ed-on-bn254` for curve arithmetic and `light-poseidon` for hashing.
//! Key design: separately managed BJJ identity (NOT derived from Solana wallet).
//!
//! The signing and verification equations match circomlib exactly:
//!   Sign:   S = r + Poseidon(R8.x, R8.y, A.x, A.y, M) * sk
//!   Verify: S * Base8 == R8 + Poseidon(R8.x, R8.y, A.x, A.y, M) * A

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use ark_ec::{AffineRepr, CurveGroup, Group};
use ark_ed_on_bn254::{EdwardsAffine, EdwardsProjective, Fq, Fr};
use ark_ff::{BigInteger, PrimeField, UniformRand};
use ark_std::Zero;
use blake2::{Blake2b512, Digest};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

use crate::error::{Result, SolidError};
use crate::poseidon;

// ─── Constants ─────────────────────────────────────────────────────────────

/// BabyJubJub Base8 point — the generator used by circomlib.
/// This is 8 * G (cofactor-cleared) as defined in `babyjub.circom`.
///
/// Base8.x = 5299619240641551281634865583518297030282874472190772894086521144482721001553
/// Base8.y = 16950150798460657717958625567821834550301663161624707787222815936182638968203
fn base8() -> EdwardsAffine {
    // Compute 8 * generator from arkworks
    let generator = EdwardsProjective::generator();
    let base8 = generator.double().double().double();
    base8.into_affine()
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
fn affine_to_pubkey(point: &EdwardsAffine) -> BJJPublicKey {
    BJJPublicKey {
        x: fq_to_bytes(&point.x),
        y: fq_to_bytes(&point.y),
    }
}

/// Convert BJJPublicKey bytes to an affine point.
///
/// Rejects points that are (a) not on the curve, (b) the Edwards
/// neutral element (0, 1) -- not a usable signing key, or (c) in a
/// small-order (cofactor-8) subgroup.  (c) is the SOLID-SEC-007
/// invariant.
///
/// `EdwardsAffine::new` has debug `assert!` statements on both the
/// on-curve and subgroup invariants; we use `new_unchecked` and
/// check explicitly so the error path is a structured `SolidError`
/// rather than a panic.
fn pubkey_to_affine(pk: &BJJPublicKey) -> std::result::Result<EdwardsAffine, SolidError> {
    let x = bytes_to_fq(&pk.x);
    let y = bytes_to_fq(&pk.y);
    let point = EdwardsAffine::new_unchecked(x, y);
    if !point.is_on_curve() || point.is_zero() {
        return Err(SolidError::PointNotOnCurve);
    }
    if !point.is_in_correct_subgroup_assuming_on_curve() {
        return Err(SolidError::BJJNotInSubgroup);
    }
    Ok(point)
}

/// Public predicate: does `pk` lie in the BJJ prime-order subgroup?
/// Returns `false` for off-curve points, the identity, or any point
/// with a non-trivial cofactor-8 component.  Delegates to
/// `ark_ec`'s `is_in_correct_subgroup_assuming_on_curve`, which
/// performs the standard `r * P == O` check (`r` is the scalar-
/// field modulus and `P` decomposes uniquely into prime-order and
/// cofactor components).  SOLID-SEC-007.
pub fn is_in_prime_order_subgroup(pk: &BJJPublicKey) -> bool {
    let x = bytes_to_fq(&pk.x);
    let y = bytes_to_fq(&pk.y);
    let point = EdwardsAffine::new_unchecked(x, y);
    if !point.is_on_curve() || point.is_zero() {
        return false;
    }
    point.is_in_correct_subgroup_assuming_on_curve()
}

/// Fail-closed form of `is_in_prime_order_subgroup`.  Use this at
/// every on-chain or off-chain site that takes a BJJ public key as
/// an untrusted input (issuer registration, signature verification,
/// cross-device import).  SOLID-SEC-007.
pub fn require_in_prime_order_subgroup(pk: &BJJPublicKey) -> Result<()> {
    let x = bytes_to_fq(&pk.x);
    let y = bytes_to_fq(&pk.y);
    let point = EdwardsAffine::new_unchecked(x, y);
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
///   3. h = Poseidon(R8.x, R8.y, A.x, A.y, msg) reduced to BJJ scalar
///   4. S = r + h * sk
///
/// The circuit verifies: S * Base8 == R8 + h * A
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

    // R8 = r * Base8
    let r8_point = b8.mul_bigint(r.into_bigint()).into_affine();

    // Challenge hash: h = Poseidon(R8.x, R8.y, A.x, A.y, msg)
    let msg_fq = bytes_to_fq(message);
    let h_fq = poseidon::hash_fr(&[r8_point.x, r8_point.y, pk_point.x, pk_point.y, msg_fq])?;

    // Reduce hash to BJJ scalar field (Fq > Fr, need mod reduction)
    let h = fq_to_fr(&h_fq);

    // S = r + h * sk (mod BJJ subgroup order)
    let s = r + h * sk;

    Ok(EdDSASignature {
        r8_x: fq_to_bytes(&r8_point.x),
        r8_y: fq_to_bytes(&r8_point.y),
        s: fr_to_bytes(&s),
    })
}

/// Verify an EdDSA-Poseidon signature.
///
/// Checks: S * Base8 == R8 + Poseidon(R8.x, R8.y, A.x, A.y, msg) * A
pub fn verify(
    public_key: &BJJPublicKey,
    message: &[u8; 32],
    signature: &EdDSASignature,
) -> Result<bool> {
    let pk_point = pubkey_to_affine(public_key)?;

    // Reconstruct R8 point.  Sign() derives R8 = r * Base8, which is
    // always in the prime-order subgroup, so a signature whose R8 has
    // a cofactor-8 component is malformed and we fail-closed
    // (SOLID-SEC-007).
    let r8_x = bytes_to_fq(&signature.r8_x);
    let r8_y = bytes_to_fq(&signature.r8_y);
    let r8_point = EdwardsAffine::new_unchecked(r8_x, r8_y);
    if !r8_point.is_on_curve() {
        return Err(SolidError::PointNotOnCurve);
    }
    if !r8_point.is_in_correct_subgroup_assuming_on_curve() {
        return Err(SolidError::BJJNotInSubgroup);
    }

    // Reconstruct S scalar
    let s = bytes_to_fr(&signature.s);

    // Challenge hash: h = Poseidon(R8.x, R8.y, A.x, A.y, msg)
    let msg_fq = bytes_to_fq(message);
    let h_fq = poseidon::hash_fr(&[r8_x, r8_y, pk_point.x, pk_point.y, msg_fq])?;
    let h = fq_to_fr(&h_fq);

    // LHS: S * Base8
    let b8 = base8();
    let lhs = b8.mul_bigint(s.into_bigint()).into_affine();

    // RHS: R8 + h * A
    let h_a = pk_point.mul_bigint(h.into_bigint());
    let rhs = (EdwardsProjective::from(r8_point) + h_a).into_affine();

    Ok(lhs == rhs)
}

// ─── Encrypted Key Storage ─────────────────────────────────────────────────

/// Portable BJJ identity bundle with encrypted private key.
///
/// The private key is encrypted with AES-256-GCM using a passphrase-derived
/// key (Argon2id). This enables secure storage and cross-device transfer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BJJIdentity {
    pub version: u8,
    pub public_key: BJJPublicKey,
    pub encrypted_private_key: EncryptedKey,
    pub metadata: IdentityMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedKey {
    pub ciphertext: Vec<u8>,
    pub nonce: [u8; 12],
    pub salt: [u8; 32],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IdentityMetadata {
    pub created_at: u64,
    pub wallet_bindings: Vec<[u8; 32]>,
    pub rotated_from: Option<BJJPublicKey>,
}

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
        let encryption_key =
            Self::derive_key(passphrase, &self.encrypted_private_key.salt)?;
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
        serde_json::to_string_pretty(self)
            .map_err(|e| SolidError::Serialization(e.to_string()))
    }

    /// Import from JSON string.
    pub fn import_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(|e| SolidError::Serialization(e.to_string()))
    }

    /// Derive an AES-256 encryption key from a passphrase using Argon2id.
    fn derive_key(passphrase: &[u8], salt: &[u8; 32]) -> Result<[u8; 32]> {
        let params = argon2::Params::new(65536, 3, 4, Some(32))
            .map_err(|e| SolidError::Encryption(format!("Argon2 params: {}", e)))?;
        let argon2 = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
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

    #[test]
    fn test_keygen_produces_valid_keypair() {
        let kp = generate_keypair().unwrap();
        assert_ne!(kp.private_key, [0u8; 32]);
        assert_ne!(kp.public_key.x, [0u8; 32]);
        assert_ne!(kp.public_key.y, [0u8; 32]);
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
}
