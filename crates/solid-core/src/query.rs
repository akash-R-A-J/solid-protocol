//! Compound query types for ZK predicate evaluation.
//!
//! Supports up to 4 predicates combined with AND/OR logic.
//! Maps directly to the circuit's public inputs.

use serde::{Deserialize, Serialize};

/// Comparison operators matching the Circom circuit's encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Operator {
    /// No operation — selective disclosure (reveal the raw field value)
    Noop = 0,
    /// Equal
    Eq = 1,
    /// Not equal
    Ne = 2,
    /// Greater than
    Gt = 3,
    /// Greater than or equal
    Gte = 4,
    /// Less than
    Lt = 5,
    /// Less than or equal
    Lte = 6,
}

impl Operator {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::Noop),
            1 => Some(Self::Eq),
            2 => Some(Self::Ne),
            3 => Some(Self::Gt),
            4 => Some(Self::Gte),
            5 => Some(Self::Lt),
            6 => Some(Self::Lte),
            _ => None,
        }
    }

    /// Evaluate the predicate: `field_value <op> query_value`
    pub fn evaluate(&self, field_value: u64, query_value: u64) -> bool {
        match self {
            Self::Noop => true,
            Self::Eq => field_value == query_value,
            Self::Ne => field_value != query_value,
            Self::Gt => field_value > query_value,
            Self::Gte => field_value >= query_value,
            Self::Lt => field_value < query_value,
            Self::Lte => field_value <= query_value,
        }
    }
}

/// Compound logic for combining predicates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum CompoundLogic {
    /// All predicates must pass
    And = 0,
    /// At least one predicate must pass
    Or = 1,
}

/// A single query predicate: "field[index] <operator> value"
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Predicate {
    /// Index of the credential in the batch (0..MAX_CREDENTIALS-1) (Phase 3.1)
    pub credential_index: u8,
    /// Index of the attestation data field (0..NUM_FIELDS-1)
    pub field_index: u8,
    /// Comparison operator
    pub operator: Operator,
    /// The comparison value / threshold
    pub value: u64,
}

impl Predicate {
    pub fn new(credential_index: u8, field_index: u8, operator: Operator, value: u64) -> Self {
        Self { credential_index, field_index, operator, value }
    }

    /// Evaluate this predicate against attestation data.
    pub fn evaluate(&self, attestation_data: &[u64]) -> bool {
        if (self.field_index as usize) >= attestation_data.len() {
            return false;
        }
        self.operator.evaluate(attestation_data[self.field_index as usize], self.value)
    }
}

/// A compound query: up to 4 predicates combined with AND/OR logic.
///
/// Maps directly to the circuit's public inputs:
/// - `queryFieldIndices[MAX_PREDICATES]`
/// - `queryOperators[MAX_PREDICATES]`
/// - `queryValues[MAX_PREDICATES]`
/// - `numPredicates`
/// - `compoundLogic`
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompoundQuery {
    /// Schema identifier (Poseidon hash of schema PDA)
    pub schema_hash: [u8; 32],
    /// Active predicates (1-4)
    pub predicates: Vec<Predicate>,
    /// How to combine predicates
    pub compound_logic: CompoundLogic,
    /// Verifier-scoped nonce (prevents cross-context linkage)
    pub verifier_nonce: [u8; 32],
    /// Expiration timestamp (0 = no expiry)
    pub expiration_timestamp: u64,
}

/// Maximum predicates per compound query (circuit parameter).
pub const MAX_PREDICATES: usize = 4;

/// Maximum fields per attestation schema (circuit parameter).
pub const MAX_FIELDS: usize = 8;

impl CompoundQuery {
    /// Create a new compound query with AND logic.
    pub fn new_and(schema_hash: [u8; 32], verifier_nonce: [u8; 32]) -> Self {
        Self {
            schema_hash,
            predicates: Vec::new(),
            compound_logic: CompoundLogic::And,
            verifier_nonce,
            expiration_timestamp: 0,
        }
    }

    /// Create a new compound query with OR logic.
    pub fn new_or(schema_hash: [u8; 32], verifier_nonce: [u8; 32]) -> Self {
        Self {
            schema_hash,
            predicates: Vec::new(),
            compound_logic: CompoundLogic::Or,
            verifier_nonce,
            expiration_timestamp: 0,
        }
    }

    /// Add a predicate. Returns error if exceeding MAX_PREDICATES.
    pub fn add_predicate(&mut self, predicate: Predicate) -> crate::error::Result<()> {
        if self.predicates.len() >= MAX_PREDICATES {
            return Err(crate::SolidError::InvalidInput(format!(
                "Maximum {} predicates per query",
                MAX_PREDICATES
            )));
        }
        if predicate.field_index as usize >= MAX_FIELDS {
            return Err(crate::SolidError::InvalidInput(format!(
                "Field index must be < {}, got {}",
                MAX_FIELDS,
                predicate.field_index
            )));
        }
        self.predicates.push(predicate);
        Ok(())
    }

    /// Set expiration timestamp.
    pub fn with_expiration(mut self, timestamp: u64) -> Self {
        self.expiration_timestamp = timestamp;
        self
    }
}

/// A multi-credential query: up to 4 predicates across up to 4 different credentials.
///
/// Phase 3.1: Composable Identity.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MultiCredentialQuery {
    /// Schema identifiers for each credential in the batch (must be exactly 4)
    pub schema_hashes: [[u8; 32]; crate::MAX_CREDENTIALS],
    /// Active predicates (1-4)
    pub predicates: Vec<Predicate>,
    /// How to combine predicates
    pub compound_logic: CompoundLogic,
    /// Verifier-scoped nonce
    pub verifier_nonce: [u8; 32],
    /// Expiration timestamp
    pub expiration_timestamp: u64,
    /// Shared master global root for all credentials
    pub global_root: [u8; 32],
}

impl MultiCredentialQuery {
    pub fn new(
        schema_hashes: [[u8; 32]; crate::MAX_CREDENTIALS],
        compound_logic: CompoundLogic,
        verifier_nonce: [u8; 32],
        global_root: [u8; 32],
    ) -> Self {
        Self {
            schema_hashes,
            predicates: Vec::new(),
            compound_logic,
            verifier_nonce,
            expiration_timestamp: 0,
            global_root,
        }
    }

    pub fn add_predicate(&mut self, predicate: Predicate) -> crate::error::Result<()> {
        if self.predicates.len() >= crate::MAX_CREDENTIALS {
            return Err(crate::SolidError::InvalidInput(format!(
                "Maximum {} predicates per query",
                crate::MAX_CREDENTIALS
            )));
        }
        if (predicate.credential_index as usize) >= crate::MAX_CREDENTIALS {
            return Err(crate::SolidError::InvalidInput(format!(
                "Credential index must be < {}, got {}",
                crate::MAX_CREDENTIALS,
                predicate.credential_index
            )));
        }
        if (predicate.field_index as usize) >= crate::NUM_FIELDS {
            return Err(crate::SolidError::InvalidInput(format!(
                "Field index must be < {}, got {}",
                crate::NUM_FIELDS,
                predicate.field_index
            )));
        }
        self.predicates.push(predicate);
        Ok(())
    }

    pub fn with_expiration(mut self, timestamp: u64) -> Self {
        self.expiration_timestamp = timestamp;
        self
    }
}

pub struct CircuitMultiQueryInputs {
    pub schema_hashes: [[u8; 32]; crate::MAX_CREDENTIALS],
    pub query_credential_indices: [u8; crate::MAX_CREDENTIALS],
    pub query_field_indices: [u8; crate::MAX_CREDENTIALS],
    pub query_operators: [u8; crate::MAX_CREDENTIALS],
    pub query_values: [u64; crate::MAX_CREDENTIALS],
    pub num_predicates: u8,
    pub compound_logic: u8,
    pub verifier_nonce: [u8; 32],
    pub expiration_timestamp: u64,
    pub global_root: [u8; 32],
}

    /// Evaluate the compound query against attestation data (for testing / client-side validation).
    pub fn evaluate(&self, attestation_data: &[u64]) -> bool {
        if self.predicates.is_empty() {
            return true;
        }
        match self.compound_logic {
            CompoundLogic::And => self.predicates.iter().all(|p| p.evaluate(attestation_data)),
            CompoundLogic::Or => self.predicates.iter().any(|p| p.evaluate(attestation_data)),
        }
    }

    /// Convert to circuit public input arrays.
    ///
    /// Pads unused predicate slots with zeros (operator=NOOP).
    pub fn to_circuit_inputs(&self) -> CircuitQueryInputs {
        let mut field_indices = [0u8; MAX_PREDICATES];
        let mut operators = [0u8; MAX_PREDICATES];
        let mut values = [0u64; MAX_PREDICATES];

        for (i, pred) in self.predicates.iter().enumerate() {
            field_indices[i] = pred.field_index;
            operators[i] = pred.operator as u8;
            values[i] = pred.value;
        }

        CircuitQueryInputs {
            schema_hash: self.schema_hash,
            query_field_indices: field_indices,
            query_operators: operators,
            query_values: values,
            num_predicates: self.predicates.len() as u8,
            compound_logic: self.compound_logic as u8,
            verifier_nonce: self.verifier_nonce,
            expiration_timestamp: self.expiration_timestamp,
        }
    }
}

/// Circuit-ready public input representation.
///
/// All arrays are padded to MAX_PREDICATES. Unused slots have operator=0 (NOOP).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CircuitQueryInputs {
    pub schema_hash: [u8; 32],
    pub query_field_indices: [u8; MAX_PREDICATES],
    pub query_operators: [u8; MAX_PREDICATES],
    pub query_values: [u64; MAX_PREDICATES],
    pub num_predicates: u8,
    pub compound_logic: u8,
    pub verifier_nonce: [u8; 32],
    pub expiration_timestamp: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operator_evaluate() {
        assert!(Operator::Eq.evaluate(21, 21));
        assert!(!Operator::Eq.evaluate(21, 22));
        assert!(Operator::Gte.evaluate(21, 21));
        assert!(Operator::Gte.evaluate(22, 21));
        assert!(!Operator::Gte.evaluate(20, 21));
        assert!(Operator::Lt.evaluate(20, 21));
        assert!(Operator::Noop.evaluate(0, 999));
    }

    #[test]
    fn test_compound_and() {
        let data = [21u64, 840, 1, 0, 0, 0, 0, 0]; // age=21, country=840
        let mut q = CompoundQuery::new_and([0u8; 32], [0u8; 32]);
        q.add_predicate(Predicate::new(0, Operator::Gte, 21)).unwrap(); // age >= 21
        q.add_predicate(Predicate::new(1, Operator::Eq, 840)).unwrap(); // country == US
        assert!(q.evaluate(&data));

        let data_fail = [18u64, 840, 1, 0, 0, 0, 0, 0]; // age=18
        assert!(!q.evaluate(&data_fail));
    }

    #[test]
    fn test_compound_or() {
        let mut q = CompoundQuery::new_or([0u8; 32], [0u8; 32]);
        q.add_predicate(Predicate::new(0, Operator::Gte, 21)).unwrap();
        q.add_predicate(Predicate::new(1, Operator::Eq, 840)).unwrap();

        assert!(q.evaluate(&[18, 840, 0, 0, 0, 0, 0, 0])); // age<21 but country=US
        assert!(!q.evaluate(&[18, 100, 0, 0, 0, 0, 0, 0])); // both fail
    }

    #[test]
    fn test_max_predicates_enforced() {
        let mut q = CompoundQuery::new_and([0u8; 32], [0u8; 32]);
        for i in 0..4 {
            q.add_predicate(Predicate::new(i, Operator::Eq, 1)).unwrap();
        }
        assert!(q.add_predicate(Predicate::new(4, Operator::Eq, 1)).is_err());
    }

    #[test]
    fn test_circuit_inputs_padding() {
        let mut q = CompoundQuery::new_and([0u8; 32], [0u8; 32]);
        q.add_predicate(Predicate::new(0, Operator::Gte, 21)).unwrap();
        let ci = q.to_circuit_inputs();
        assert_eq!(ci.num_predicates, 1);
        assert_eq!(ci.query_operators[0], Operator::Gte as u8);
        assert_eq!(ci.query_operators[1], 0); // NOOP padding
        assert_eq!(ci.query_operators[2], 0);
        assert_eq!(ci.query_operators[3], 0);
    }
}
