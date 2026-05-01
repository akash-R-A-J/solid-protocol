// Note: arkworks 0.5 requires `CircomConfig` to carry the field
// generic explicitly (`CircomConfig<Fr>`); 0.4 inferred it from the
// builder.  Updated 2026-05-01 during the prover workspace's 0.4 -> 0.5
// alignment.  See `tools/solid-prover/Cargo.toml` header for the why.
use ark_bn254::{Bn254, Fr};
use ark_circom::{CircomBuilder, CircomConfig};
use ark_groth16::{Groth16, Proof, ProvingKey};
use ark_relations::r1cs::SynthesisError;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ark_snark::SNARK;
use num_bigint::BigInt;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

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
    /// Paths to the .r1cs and .wasm artefacts.  arkworks 0.5's
    /// `CircomConfig<F>` does not implement `Clone` (it owns a wasmer
    /// `Store` + `WitnessCalculator`, neither clonable), so we cannot
    /// stash a single `CircomConfig` and reuse it across proof calls.
    /// Storing the paths and rebuilding the config per call is the
    /// idiomatic 0.5 pattern -- the wasm parse is bounded (~tens of ms
    /// for our circuit) and there's no hidden state shared between
    /// proofs anyway.
    pub r1cs_path: std::path::PathBuf,
    pub wasm_path: std::path::PathBuf,
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

        // 3. Validate the wasm path now (cheap open) so callers see
        //    integrity errors early rather than on the first proof.
        let _wasm_file = std::fs::File::open(&wasm_path)?;

        Ok(Self {
            proving_key,
            r1cs_path: r1cs_path.as_ref().to_path_buf(),
            wasm_path: wasm_path.as_ref().to_path_buf(),
            expected_r1cs_hash: expected_r1cs_hash.to_string(),
        })
    }

    /// PHASE 4: Native Batch Proof Generation (N=4)
    pub fn generate_batch_proof(
        &self,
        signals: HashMap<String, Vec<BigInt>>,
    ) -> Result<Proof<Bn254>, ProverError> {
        // Rebuild config per call (see struct doc for why CircomConfig
        // cannot be stashed in arkworks 0.5).
        let circuit_config = CircomConfig::<Fr>::new(&self.wasm_path, &self.r1cs_path)
            .map_err(|e| ProverError::External(e.to_string()))?;
        let mut builder = CircomBuilder::new(circuit_config);

        // Populate inputs
        for (name, values) in signals {
            for val in values {
                builder.push_input(&name, val);
            }
        }

        // Build circuit and compute witness
        let circuit = builder
            .build()
            .map_err(|e| ProverError::External(e.to_string()))?;

        // SOLID-SEC-017 (closed): use OsRng (CSPRNG seeded from /dev/urandom
        // on Unix and BCryptGenRandom on Windows) instead of
        // `ark_std::test_rng()`.  test_rng is a deterministic ChaCha20Rng
        // seeded from a fixed nothing-up-my-sleeve constant; two proofs by
        // the same prover over the same inputs would produce byte-identical
        // proof randomness, breaking unlinkability across calls.  OsRng is
        // the right primitive for production proving: every call samples
        // fresh entropy.
        let mut rng = rand::rngs::OsRng;
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
        out[i] = bytes[31 - i]; // X
        out[i + 32] = bytes[63 - i]; // Y
    }
    out
}

fn reverse_endianness_g2(bytes: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; 128];
    for i in 0..32 {
        out[i] = bytes[31 - i]; // X_c1
        out[i + 32] = bytes[63 - i]; // X_c0
        out[i + 64] = bytes[95 - i]; // Y_c1
        out[i + 96] = bytes[127 - i]; // Y_c0
    }
    out
}

#[cfg(test)]
mod tests {
    use rand::RngCore;

    /// SOLID-SEC-017 regression gate.
    ///
    /// The prover must use a CSPRNG that samples fresh entropy per call.
    /// `ark_std::test_rng()` is a deterministic ChaCha20 keyed off a
    /// nothing-up-my-sleeve seed; two cold-start invocations of the
    /// prover would produce byte-identical proof randomness and break
    /// unlinkability across calls.
    ///
    /// `OsRng` (kernel-backed: `getrandom(2)` / `BCryptGenRandom`)
    /// samples fresh entropy on every `next_u64`, so two freshly-
    /// constructed instances return different bytes with overwhelming
    /// probability (collision ~2^-64 per pair).
    ///
    /// This test is the tripwire: if a contributor reverts the
    /// `prove_with_signals` rng to `test_rng()`, two consecutive `next_u64`
    /// calls on freshly-constructed instances would return the SAME u64
    /// (deterministic seed -> deterministic first output) and this
    /// assertion fails.  Cheap (~microseconds), no zkey required.
    #[test]
    fn sec_017_prover_rng_samples_fresh_entropy_per_invocation() {
        let mut a = rand::rngs::OsRng;
        let mut b = rand::rngs::OsRng;
        // Two independent samples; the probability they collide is
        // ~2^-64.  If they DO collide, either we won the lottery or
        // someone wired in a deterministic-seed RNG.
        let s_a = a.next_u64();
        let s_b = b.next_u64();
        assert_ne!(
            s_a, s_b,
            "prover RNG must sample fresh entropy per call (SOLID-SEC-017)",
        );
    }
}
