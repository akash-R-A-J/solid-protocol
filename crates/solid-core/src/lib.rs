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
/// Protocol-wide architectural constants
pub const MAX_CREDENTIALS: usize = 4;
pub const NUM_FIELDS: usize = 8;
pub const TREE_DEPTH: usize = 20;

pub mod babyjubjub;
pub mod error;
pub mod identity;
pub mod nullifier;
pub mod poseidon;
pub mod query;
pub mod sas;
pub mod schema;

// ─── Host-only modules ─────────────────────────────────────────────────────
//
// These modules pull host-only crypto (`serde_json`, `rand`, `aes-gcm`,
// `argon2`) and/or call the host-only field-element Poseidon path
// (`poseidon::hash_fr` / `hash_fields`) and the host-only EdDSA primitives
// (`babyjubjub::sign` / `verify`).  They are never reached from on-chain
// code (issuer-registry / schema-registry / zk-verifier) and therefore are
// excluded from the BPF compilation unit.  See `babyjubjub.rs` and
// `poseidon.rs` for the rationale and the on-chain reachability map.
#[cfg(not(target_os = "solana"))]
pub mod commitment;
#[cfg(not(target_os = "solana"))]
pub mod credential;
#[cfg(not(target_os = "solana"))]
pub mod multi_cred;

pub use babyjubjub::{BJJKeypair, BJJPublicKey, EdDSASignature};
#[cfg(not(target_os = "solana"))]
pub use credential::{Credential, CredentialBuilder};
pub use error::{Result, SolidError};
pub use query::{
    CircuitMultiQueryInputs, CircuitQueryInputs, CompoundLogic, CompoundQuery,
    MultiCredentialQuery, Operator, Predicate, MAX_FIELDS, MAX_PREDICATES,
};
