//! # SolID Core
//!
//! Core cryptographic primitives for the SolID Protocol — Private Onchain Identity
//! Infrastructure for Solana.
//!
//! This crate provides:
//! - **Poseidon hash**: Circomlib-compatible SNARK-friendly hashing
//! - **BabyJubJub EdDSA**: Key generation, signing, verification matching circomlib's `eddsaposeidon.circom`
//! - **Attestation commitments**: Deterministic credential hashing for Merkle tree leaves
//! - **Nullifiers**: Unlinkable anti-replay tokens per verifier context
//! - **Types**: Query predicates, schema definitions, credential structures
//!
//! All cryptographic functions produce outputs identical to circomlib, ensuring
//! proofs generated off-chain verify correctly in on-chain Groth16 verification.

pub mod babyjubjub;
pub mod commitment;
pub mod credential;
pub mod error;
pub mod nullifier;
pub mod poseidon;
pub mod query;
pub mod sas;
pub mod schema;

pub use error::SolidError;
