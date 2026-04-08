use ark_bn254::{Bn254, Fr};
use ark_groth16::{Groth16, ProvingKey, Proof};
use ark_relations::r1cs::SynthesisError;
use ark_snark::SNARK;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ark_circom::{CircomBuilder, CircomConfig};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;
use num_bigint::BigInt;

#[derive(Error, Debug)]
pub enum ProverError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Synthesis error: {0}")]
    Synthesis(#[from] SynthesisError),
    #[error("Invalid witness: {0}")]
    InvalidWitness(String),
    #[error("Circuit integrity failure: expected {0}, got {1}")]
    IntegrityFailure(String, String),
    #[error("External error: {0}")]
    External(String),
}

pub struct SolIDProver {
    pub proving_key: ProvingKey<Bn254>,
    pub circuit_config: CircomConfig,
    pub expected_r1cs_hash: String,
}

impl SolIDProver {
    /// Initialize with artifact paths and integrity check.
    pub fn new(
        r1cs_path: impl AsRef<Path>,
        wasm_path: impl AsRef<Path>,
        zkey_path: impl AsRef<Path>,
        expected_r1cs_hash: &str,
    ) -> Result<Self, ProverError> {
        // 1. SEC-21: Circuit Integrity Verification
        let mut file = std::fs::File::open(&r1cs_path)?;
        let mut hasher = Sha256::new();
        std::io::copy(&mut file, &mut hasher)?;
        let actual_hash = hex::encode(hasher.finalize());

        if actual_hash != expected_r1cs_hash {
            return Err(ProverError::IntegrityFailure(
                expected_r1cs_hash.to_string(),
                actual_hash,
            ));
        }

        // 2. Load Proving Key (zkey)
        let mut zkey_file = std::fs::File::open(zkey_path)?;
        let proving_key = ProvingKey::<Bn254>::deserialize_compressed(&mut zkey_file)
            .map_err(|e| ProverError::Serialization(e.to_string()))?;

        // 3. Configure Circom Builder
        let circuit_config = CircomConfig::new(r1cs_path, wasm_path)
            .map_err(|e| ProverError::External(e.to_string()))?;

        Ok(Self {
            proving_key,
            circuit_config,
            expected_r1cs_hash: expected_r1cs_hash.to_string(),
        })
    }

    /// PHASE 4: Native Batch Proof Generation (N=4)
    pub fn generate_batch_proof(
        &self,
        signals: HashMap<String, Vec<BigInt>>,
    ) -> Result<Proof<Bn254>, ProverError> {
        let mut builder = CircomBuilder::new(self.circuit_config.clone());
        
        // Populate inputs
        for (name, values) in signals {
            for val in values {
                builder.push_input(&name, val);
            }
        }

        // Build circuit and compute witness
        let circuit = builder.build()
            .map_err(|e| ProverError::External(e.to_string()))?;

        let mut rng = ark_std::test_rng();
        let proof = Groth16::<Bn254>::prove(&self.proving_key, circuit, &mut rng)?;

        Ok(proof)
    }

    /// Convert arkworks Proof to Solana-compatible Big-Endian format.
    pub fn to_solana_format(proof: &Proof<Bn254>) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let mut a_bytes = Vec::new();
        proof.a.serialize_uncompressed(&mut a_bytes).unwrap();
        // Solana expects Big-Endian for point coordinates
        let a = reverse_endianness_g1(&a_bytes);

        let mut b_bytes = Vec::new();
        proof.b.serialize_uncompressed(&mut b_bytes).unwrap();
        let b = reverse_endianness_g2(&b_bytes);

        let mut c_bytes = Vec::new();
        proof.c.serialize_uncompressed(&mut c_bytes).unwrap();
        let c = reverse_endianness_g1(&c_bytes);

        (a, b, c)
    }
}

fn reverse_endianness_g1(bytes: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; 64];
    for i in 0..32 {
        out[i] = bytes[31 - i];       // X
        out[i + 32] = bytes[63 - i];  // Y
    }
    out
}

fn reverse_endianness_g2(bytes: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; 128];
    for i in 0..32 {
        out[i] = bytes[31 - i];           // X_c1
        out[i + 32] = bytes[63 - i];      // X_c0
        out[i + 64] = bytes[95 - i];      // Y_c1
        out[i + 96] = bytes[127 - i];     // Y_c0
    }
    out
}
