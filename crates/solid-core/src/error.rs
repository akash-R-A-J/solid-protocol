use thiserror::Error;

/// Unified error type for all SolID core cryptographic operations.
#[derive(Error, Debug)]
pub enum SolidError {
    #[error("Poseidon hash failed: {0}")]
    PoseidonHash(String),

    #[error("BabyJubJub key error: {0}")]
    BJJKey(String),

    #[error("EdDSA signature error: {0}")]
    Signature(String),

    #[error("Attestation commitment error: {0}")]
    Commitment(String),

    #[error("Nullifier computation error: {0}")]
    Nullifier(String),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Decryption error: {0}")]
    Decryption(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Point not on curve")]
    PointNotOnCurve,

    #[error("Point is not in the BabyJubJub prime-order subgroup (cofactor-8 component rejected)")]
    BJJNotInSubgroup,

    #[error("Signature verification failed")]
    VerificationFailed,
}

pub type Result<T> = std::result::Result<T, SolidError>;
