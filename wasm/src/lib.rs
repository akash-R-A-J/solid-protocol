use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

/// Initialize panic hook for better error messages in browser console.
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

// ─── Poseidon Hash ─────────────────────────────────────────────────────────

/// Compute Poseidon hash of u64 field values.
/// Returns 32-byte LE hash.
#[wasm_bindgen(js_name = "poseidonHash")]
pub fn poseidon_hash(fields: &[u64]) -> Result<Vec<u8>, JsError> {
    let hash = solid_core::poseidon::hash_fields_to_bytes(fields)
        .map_err(|e| JsError::new(&format!("{}", e)))?;
    Ok(hash.to_vec())
}

/// Compute Poseidon hash of byte arrays (each 32 bytes LE).
#[wasm_bindgen(js_name = "poseidonHashBytes")]
pub fn poseidon_hash_bytes(inputs: &[u8]) -> Result<Vec<u8>, JsError> {
    if inputs.len() % 32 != 0 {
        return Err(JsError::new("Each input must be 32 bytes"));
    }
    let chunks: Vec<[u8; 32]> = inputs.chunks_exact(32)
        .map(|c| { let mut arr = [0u8; 32]; arr.copy_from_slice(c); arr })
        .collect();
    let hash = solid_core::poseidon::hash_bytes(&chunks)
        .map_err(|e| JsError::new(&format!("{}", e)))?;
    Ok(hash.to_vec())
}

// ─── BabyJubJub Key Operations ─────────────────────────────────────────────

/// Generate a new BabyJubJub keypair.
/// Returns JSON: { privateKey: number[], publicKeyX: number[], publicKeyY: number[] }
#[wasm_bindgen(js_name = "generateBJJKeypair")]
pub fn generate_bjj_keypair() -> Result<JsValue, JsError> {
    let kp = solid_core::babyjubjub::generate_keypair()
        .map_err(|e| JsError::new(&format!("{}", e)))?;

    #[derive(Serialize)]
    struct KeypairResult {
        private_key: Vec<u8>,
        public_key_x: Vec<u8>,
        public_key_y: Vec<u8>,
    }

    let result = KeypairResult {
        private_key: kp.private_key.to_vec(),
        public_key_x: kp.public_key.x.to_vec(),
        public_key_y: kp.public_key.y.to_vec(),
    };

    Ok(serde_wasm_bindgen::to_value(&result)?)
}

/// Sign a message (32 bytes) with a BJJ private key (32 bytes).
/// Returns JSON: { r8x: number[], r8y: number[], s: number[] }
#[wasm_bindgen(js_name = "signMessage")]
pub fn sign_message(private_key: &[u8], message: &[u8]) -> Result<JsValue, JsError> {
    if private_key.len() != 32 || message.len() != 32 {
        return Err(JsError::new("private_key and message must be 32 bytes each"));
    }
    let mut sk = [0u8; 32];
    sk.copy_from_slice(private_key);
    let mut msg = [0u8; 32];
    msg.copy_from_slice(message);

    let sig = solid_core::babyjubjub::sign(&sk, &msg)
        .map_err(|e| JsError::new(&format!("{}", e)))?;

    Ok(serde_wasm_bindgen::to_value(&sig)?)
}

/// Verify an EdDSA-Poseidon signature.
#[wasm_bindgen(js_name = "verifySignature")]
pub fn verify_signature(
    pub_key_x: &[u8], pub_key_y: &[u8],
    message: &[u8],
    r8x: &[u8], r8y: &[u8], s: &[u8],
) -> Result<bool, JsError> {
    let pk = solid_core::babyjubjub::BJJPublicKey {
        x: to_arr32(pub_key_x)?, y: to_arr32(pub_key_y)?,
    };
    let sig = solid_core::babyjubjub::EdDSASignature {
        r8_x: to_arr32(r8x)?, r8_y: to_arr32(r8y)?, s: to_arr32(s)?,
    };
    solid_core::babyjubjub::verify(&pk, &to_arr32(message)?, &sig)
        .map_err(|e| JsError::new(&format!("{}", e)))
}

// ─── Commitment & Nullifier ────────────────────────────────────────────────

/// Compute attestation commitment.
#[wasm_bindgen(js_name = "computeCommitment")]
pub fn compute_commitment(
    data_fields: &[u64],
    schema_hash: &[u8],
    holder_pub_key_x: &[u8],
    holder_pub_key_y: &[u8],
    salt: &[u8],
) -> Result<Vec<u8>, JsError> {
    let pk = solid_core::babyjubjub::BJJPublicKey {
        x: to_arr32(holder_pub_key_x)?, y: to_arr32(holder_pub_key_y)?,
    };
    let commitment = solid_core::commitment::compute_attestation_commitment(
        data_fields, &to_arr32(schema_hash)?, &pk, &to_arr32(salt)?,
    ).map_err(|e| JsError::new(&format!("{}", e)))?;
    Ok(commitment.to_vec())
}

/// Compute nullifier hash.
#[wasm_bindgen(js_name = "computeNullifier")]
pub fn compute_nullifier(
    holder_private_key: &[u8],
    schema_hash: &[u8],
    verifier_nonce: &[u8],
) -> Result<Vec<u8>, JsError> {
    let null = solid_core::nullifier::compute_nullifier(
        &to_arr32(holder_private_key)?,
        &to_arr32(schema_hash)?,
        &to_arr32(verifier_nonce)?,
    ).map_err(|e| JsError::new(&format!("{}", e)))?;
    Ok(null.to_vec())
}

// ─── Identity Management ──────────────────────────────────────────────────

/// Generate an encrypted BJJ identity.
#[wasm_bindgen(js_name = "generateIdentity")]
pub fn generate_identity(passphrase: &str) -> Result<String, JsError> {
    let identity = solid_core::babyjubjub::BJJIdentity::generate(passphrase.as_bytes())
        .map_err(|e| JsError::new(&format!("{}", e)))?;
    identity.export_json().map_err(|e| JsError::new(&format!("{}", e)))
}

/// Unlock an encrypted BJJ identity.
#[wasm_bindgen(js_name = "unlockIdentity")]
pub fn unlock_identity(identity_json: &str, passphrase: &str) -> Result<Vec<u8>, JsError> {
    let identity = solid_core::babyjubjub::BJJIdentity::import_json(identity_json)
        .map_err(|e| JsError::new(&format!("{}", e)))?;
    let key = identity.unlock(passphrase.as_bytes())
        .map_err(|e| JsError::new(&format!("{}", e)))?;
    Ok(key.to_vec())
}

// ─── Helpers ───────────────────────────────────────────────────────────────

fn to_arr32(slice: &[u8]) -> Result<[u8; 32], JsError> {
    if slice.len() != 32 {
        return Err(JsError::new(&format!("Expected 32 bytes, got {}", slice.len())));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(slice);
    Ok(arr)
}
