//! Cross-language test-vector generator.
//!
//! Run with:
//!   cargo run --example gen_vectors --manifest-path crates/solid-core/Cargo.toml
//!
//! Writes `tests/vectors/commitment_and_nullifier.json`, consumable by any
//! language SDK to verify byte-level agreement with the reference Rust impl.

use serde::Serialize;
use solid_core::{
    babyjubjub::BJJPublicKey,
    commitment::compute_attestation_commitment,
    nullifier::compute_nullifier,
    poseidon,
};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize)]
struct Vectors {
    description: &'static str,
    commitment: CommitmentVector,
    nullifier: NullifierVector,
}

#[derive(Serialize)]
struct CommitmentVector {
    data_fields: Vec<u64>,
    schema_hash_hex: String,
    holder_pub_x_hex: String,
    holder_pub_y_hex: String,
    salt_hex: String,
    expected_commitment_hex: String,
}

#[derive(Serialize)]
struct NullifierVector {
    master_key_hex: String,
    rev_nonce: u64,
    verifier_addr_hex: String,
    query_hash_hex: String,
    verifier_nonce_hex: String,
    expected_nullifier_hex: String,
}

fn main() -> anyhow::Result<()> {
    // Fixed, reproducible inputs — matching `docs/test_vectors.md`.
    let data_fields = vec![21u64, 840, 1, 0, 0, 0, 0, 0]; // age=21, country=US
    let schema_hash = poseidon::hash_fields_to_bytes(&[1u64, 2, 3])?;
    let holder_x = [7u8; 32];
    let holder_y = [8u8; 32];
    let salt = [42u8; 32];

    let holder_pk = BJJPublicKey { x: holder_x, y: holder_y };
    let commitment = compute_attestation_commitment(&data_fields, &schema_hash, &holder_pk, &salt)?;

    let master_key = [0x11u8; 32];
    let rev_nonce: u64 = 7;
    let verifier_addr = [0x22u8; 32];
    let query_hash = [0x33u8; 32];
    let verifier_nonce = [0x44u8; 32];
    let nullifier = compute_nullifier(&master_key, rev_nonce, &verifier_addr, &query_hash, &verifier_nonce)?;

    let vectors = Vectors {
        description: "SolID cross-language test vectors. Any SDK must reproduce these bytes.",
        commitment: CommitmentVector {
            data_fields,
            schema_hash_hex: hex::encode(schema_hash),
            holder_pub_x_hex: hex::encode(holder_x),
            holder_pub_y_hex: hex::encode(holder_y),
            salt_hex: hex::encode(salt),
            expected_commitment_hex: hex::encode(commitment),
        },
        nullifier: NullifierVector {
            master_key_hex: hex::encode(master_key),
            rev_nonce,
            verifier_addr_hex: hex::encode(verifier_addr),
            query_hash_hex: hex::encode(query_hash),
            verifier_nonce_hex: hex::encode(verifier_nonce),
            expected_nullifier_hex: hex::encode(nullifier),
        },
    };

    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/vectors");
    fs::create_dir_all(&out_dir)?;
    let out_path = out_dir.join("commitment_and_nullifier.json");
    fs::write(&out_path, serde_json::to_string_pretty(&vectors)?)?;
    println!("Wrote {}", out_path.display());
    Ok(())
}
