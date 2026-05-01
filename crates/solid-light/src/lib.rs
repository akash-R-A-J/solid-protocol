//! # SolID Light Protocol Integration
//!
//! On-chain CPI helpers for interacting with Light Protocol's
//! compressed state trees from SolID programs.
//!
//! This crate provides:
//! - Compressed credential account types
//! - CPI helpers for leaf insertion (issuance)
//! - CPI helpers for state root verification (proof checking)
//! - CPI helpers for leaf nullification (revocation)

pub mod cpi_helpers;
pub mod credential_tree;
pub mod groth16;
