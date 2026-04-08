use once_cell::sync::Lazy;
use std::sync::Mutex;

/// Initialize panic hook for better error messages in browser console.
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

// ─── Phase 3: WASM Memory Bridge (Zero-Copy) ──────────────────────────────
// A static buffer that JS can write to directly via 'WebAssembly.Memory'.
// Prevents high-frequency copying overhead for bulk hashing operations.
static SHARED_BUFFER: Lazy<Mutex<Vec<u8>>> = Lazy::new(|| Mutex::new(vec![0u8; 1024 * 64])); // 64KB initial

#[wasm_bindgen(js_name = "getSharedBufferPointer")]
pub fn get_shared_buffer_pointer() -> *const u8 {
    let buffer = SHARED_BUFFER.lock().unwrap();
    buffer.as_ptr()
}

#[wasm_bindgen(js_name = "resizeSharedBuffer")]
pub fn resize_shared_buffer(new_size: usize) {
    let mut buffer = SHARED_BUFFER.lock().unwrap();
    buffer.resize(new_size, 0);
}

#[wasm_bindgen(js_name = "poseidonHashShared")]
pub fn poseidon_hash_shared(len: usize) -> Result<Vec<u8>, JsError> {
    let buffer = SHARED_BUFFER.lock().unwrap();
    if len > buffer.len() || len % 32 != 0 {
        return Err(JsError::new("Invalid shared buffer length or alignment"));
    }
    
    let chunks: Vec<[u8; 32]> = buffer[..len].chunks_exact(32)
        .map(|c| { let mut arr = [0u8; 32]; arr.copy_from_slice(c); arr })
        .collect();
        
    let hash = solid_core::poseidon::hash_bytes(&chunks)
        .map_err(|e| JsError::new(&format!("{}", e)))?;
    Ok(hash.to_vec())
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

/// Phase 1.1: Derive a deterministic sub-key.
#[wasm_bindgen(js_name = "deriveKey")]
    let mut mk = [0u8; 32];
    mk.copy_from_slice(master_key);
    let mut ctx_arr = [0u8; 32];
    ctx_arr.copy_from_slice(context);

    let dk = solid_core::babyjubjub::derive_key(&mk, &ctx_arr)
        .map_err(|e| JsError::new(&format!("{}", e)))?;
    Ok(dk.to_vec())
}

/// Phase 2.1: Compute identity state commitment.
#[wasm_bindgen(js_name = "computeIdentityState")]
pub fn compute_identity_state(pubkey_x: &[u8], pubkey_y: &[u8], revocation_nonce: u64) -> Result<Vec<u8>, JsError> {
    let mut x = [0u8; 32];
    x.copy_from_slice(pubkey_x);
    let mut y = [0u8; 32];
    y.copy_from_slice(pubkey_y);

    let pk = solid_core::babyjubjub::BJJPublicKey { x, y };
    let id_state = solid_core::identity::IdentityState::new(pk, revocation_nonce);
    
    let commitment = id_state.commitment()
        .map_err(|e| JsError::new(&format!("{}", e)))?;
    Ok(commitment.to_vec())
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
