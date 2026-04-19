//! Phase 3.1: Multi-Credential Witness Generator.
//!
//! Aggregates up to `MAX_CREDENTIALS` (=4) `Credential`s under a single master
//! identity and emits the signal map consumed by `circuits/batch_credential_query.circom`.
//!
//! Invariants enforced before witness emission:
//!   1. All credentials belong to `master_identity_pub_key` (SEC-17).
//!   2. Canonical ordering: batch is sorted by `schema_hash` ascending (SEC-20).
//!   3. Short batches are padded with zero-schema placeholders that still carry
//!      the master identity key (so `IdentityAnchor` doesn't reject them).

use std::collections::HashMap;

use num_bigint::BigInt;

use crate::babyjubjub::BJJPublicKey;
use crate::credential::Credential;
use crate::error::{Result, SolidError};
use crate::query::MultiCredentialQuery;

/// Per-credential inclusion proof in the schema-scoped Merkle tree.
#[derive(Clone, Debug)]
pub struct CredentialMerkleProof {
    /// Merkle root of the credential-tree at proof time (public input)
    pub merkle_root: [u8; 32],
    /// Authentication path siblings (length must equal `TREE_DEPTH` = 20)
    pub siblings: Vec<[u8; 32]>,
    /// Path index bits (0 = left, 1 = right) — length must match siblings
    pub path_indices: Vec<u64>,
}

impl CredentialMerkleProof {
    pub fn empty() -> Self {
        Self {
            merkle_root: [0u8; 32],
            siblings: vec![[0u8; 32]; crate::TREE_DEPTH],
            path_indices: vec![0; crate::TREE_DEPTH],
        }
    }
}

/// Per-credential path in the **global** identity/state tree.
#[derive(Clone, Debug)]
pub struct GlobalInclusionProof {
    pub siblings: Vec<[u8; 32]>,
    pub path_indices: Vec<u64>,
}

impl GlobalInclusionProof {
    pub fn empty() -> Self {
        Self {
            siblings: vec![[0u8; 32]; crate::TREE_DEPTH],
            path_indices: vec![0; crate::TREE_DEPTH],
        }
    }
}

/// The complete, ordered input bundle for a batch proof.
pub struct BatchCredentialWitness {
    pub master_identity_key: [u8; 32],
    pub master_identity_pub_key: BJJPublicKey,
    pub revocation_nonce: u64,
    pub credentials: Vec<Credential>,
    pub credential_proofs: Vec<CredentialMerkleProof>,
    pub global_proofs: Vec<GlobalInclusionProof>,
    pub query: MultiCredentialQuery,
}

impl BatchCredentialWitness {
    pub fn new(
        master_key: [u8; 32],
        master_pub: BJJPublicKey,
        rev_nonce: u64,
        query: MultiCredentialQuery,
    ) -> Self {
        Self {
            master_identity_key: master_key,
            master_identity_pub_key: master_pub,
            revocation_nonce: rev_nonce,
            credentials: Vec::new(),
            credential_proofs: Vec::new(),
            global_proofs: Vec::new(),
            query,
        }
    }

    /// Add a credential (plus proofs) to the batch.
    ///
    /// SEC-17: enforces that the credential belongs to the master identity.
    pub fn add_credential(
        &mut self,
        cred: Credential,
        credential_proof: CredentialMerkleProof,
        global_proof: GlobalInclusionProof,
    ) -> Result<()> {
        if self.credentials.len() >= crate::MAX_CREDENTIALS {
            return Err(SolidError::InvalidInput(format!(
                "Batch full (max {})",
                crate::MAX_CREDENTIALS
            )));
        }
        if cred.holder_pub_key.x != self.master_identity_pub_key.x
            || cred.holder_pub_key.y != self.master_identity_pub_key.y
        {
            return Err(SolidError::InvalidInput(
                "SEC-17: credential does not belong to master identity".into(),
            ));
        }
        if credential_proof.siblings.len() != crate::TREE_DEPTH
            || credential_proof.path_indices.len() != crate::TREE_DEPTH
            || global_proof.siblings.len() != crate::TREE_DEPTH
            || global_proof.path_indices.len() != crate::TREE_DEPTH
        {
            return Err(SolidError::InvalidInput(format!(
                "merkle proof length must be {}",
                crate::TREE_DEPTH
            )));
        }

        self.credentials.push(cred);
        self.credential_proofs.push(credential_proof);
        self.global_proofs.push(global_proof);
        Ok(())
    }

    /// Pad the batch to `MAX_CREDENTIALS` with zero-schema placeholders owned by the master key.
    pub fn pad(&mut self) {
        while self.credentials.len() < crate::MAX_CREDENTIALS {
            self.credentials.push(placeholder_credential(
                self.master_identity_pub_key.clone(),
            ));
            self.credential_proofs.push(CredentialMerkleProof::empty());
            self.global_proofs.push(GlobalInclusionProof::empty());
        }
    }

    /// SEC-20: sort the batch ascending by `schema_hash`. Returns the permutation
    /// mapping `original_index -> new_index` so the caller can remap predicate
    /// `credential_index` fields accordingly.
    pub fn canonicalize(&mut self) -> Vec<usize> {
        let n = self.credentials.len();
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| self.credentials[a].schema_hash.cmp(&self.credentials[b].schema_hash));

        let creds = std::mem::take(&mut self.credentials);
        let cproofs = std::mem::take(&mut self.credential_proofs);
        let gproofs = std::mem::take(&mut self.global_proofs);
        let mut new_c = Vec::with_capacity(n);
        let mut new_cp = Vec::with_capacity(n);
        let mut new_gp = Vec::with_capacity(n);
        let mut inverse = vec![0usize; n];
        for (new_idx, &orig) in order.iter().enumerate() {
            inverse[orig] = new_idx;
            new_c.push(creds[orig].clone());
            new_cp.push(cproofs[orig].clone());
            new_gp.push(gproofs[orig].clone());
        }
        self.credentials = new_c;
        self.credential_proofs = new_cp;
        self.global_proofs = new_gp;
        inverse
    }

    /// Produce the Circom signal map consumed by `batch_credential_query.circom`.
    pub fn to_circom_signals(&self) -> HashMap<String, Vec<BigInt>> {
        fn to_big(bytes: &[u8; 32]) -> BigInt {
            BigInt::from_bytes_le(num_bigint::Sign::Plus, bytes)
        }

        let mut s: HashMap<String, Vec<BigInt>> = HashMap::new();

        // Global identity
        s.insert("globalRoot".into(), vec![to_big(&self.query.global_root)]);
        s.insert("masterIdentityKey".into(), vec![to_big(&self.master_identity_key)]);
        s.insert(
            "revocationNonce".into(),
            vec![BigInt::from(self.revocation_nonce)],
        );

        let mut global_siblings = Vec::with_capacity(crate::MAX_CREDENTIALS * crate::TREE_DEPTH);
        let mut global_indices = Vec::with_capacity(crate::MAX_CREDENTIALS * crate::TREE_DEPTH);
        for i in 0..crate::MAX_CREDENTIALS {
            let gp = self
                .global_proofs
                .get(i)
                .cloned()
                .unwrap_or_else(GlobalInclusionProof::empty);
            for sib in &gp.siblings {
                global_siblings.push(to_big(sib));
            }
            for &idx in &gp.path_indices {
                global_indices.push(BigInt::from(idx));
            }
        }
        s.insert("globalSiblings".into(), global_siblings);
        s.insert("globalPathIndices".into(), global_indices);

        // Per-credential bundles
        let mut merkle_roots = Vec::new();
        let mut schema_hashes = Vec::new();
        let mut data = Vec::new();
        let mut salts = Vec::new();
        let mut issuer_r8xs = Vec::new();
        let mut issuer_r8ys = Vec::new();
        let mut issuer_ss = Vec::new();
        let mut issuer_ax = Vec::new();
        let mut issuer_ay = Vec::new();
        let mut merkle_siblings = Vec::new();
        let mut merkle_path_indices = Vec::new();
        let mut exp_ts = Vec::new();

        for i in 0..crate::MAX_CREDENTIALS {
            let cred = &self.credentials[i];
            let proof = &self.credential_proofs[i];

            merkle_roots.push(to_big(&proof.merkle_root));
            schema_hashes.push(to_big(&cred.schema_hash));

            for k in 0..crate::NUM_FIELDS {
                let v = cred.attestation_data.get(k).copied().unwrap_or(0);
                data.push(BigInt::from(v));
            }
            salts.push(to_big(&cred.salt));
            issuer_r8xs.push(to_big(&cred.issuer_signature.r8_x));
            issuer_r8ys.push(to_big(&cred.issuer_signature.r8_y));
            issuer_ss.push(to_big(&cred.issuer_signature.s));
            issuer_ax.push(to_big(&cred.issuer_pub_key.x));
            issuer_ay.push(to_big(&cred.issuer_pub_key.y));

            for sib in &proof.siblings {
                merkle_siblings.push(to_big(sib));
            }
            for &idx in &proof.path_indices {
                merkle_path_indices.push(BigInt::from(idx));
            }
            exp_ts.push(BigInt::from(cred.expiration_timestamp));
        }

        s.insert("merkleRoots".into(), merkle_roots);
        s.insert("schemaHashes".into(), schema_hashes);
        s.insert("data".into(), data);
        s.insert("salts".into(), salts);
        s.insert("issuerSigR8xs".into(), issuer_r8xs);
        s.insert("issuerSigR8ys".into(), issuer_r8ys);
        s.insert("issuerSigSs".into(), issuer_ss);
        s.insert("issuerPubKeyAxs".into(), issuer_ax);
        s.insert("issuerPubKeyAys".into(), issuer_ay);
        s.insert("merkleSiblings".into(), merkle_siblings);
        s.insert("merklePathIndices".into(), merkle_path_indices);
        s.insert("expirationTimestamps".into(), exp_ts);

        // Query signals (padded to MAX_PREDICATES)
        let mut q_cred_idx = Vec::with_capacity(crate::MAX_PREDICATES);
        let mut q_field_idx = Vec::with_capacity(crate::MAX_PREDICATES);
        let mut q_ops = Vec::with_capacity(crate::MAX_PREDICATES);
        let mut q_vals = Vec::with_capacity(crate::MAX_PREDICATES);
        for i in 0..crate::MAX_PREDICATES {
            match self.query.predicates.get(i) {
                Some(p) => {
                    q_cred_idx.push(BigInt::from(p.credential_index));
                    q_field_idx.push(BigInt::from(p.field_index));
                    q_ops.push(BigInt::from(p.operator as u8));
                    q_vals.push(BigInt::from(p.value));
                }
                None => {
                    q_cred_idx.push(BigInt::from(0));
                    q_field_idx.push(BigInt::from(0));
                    q_ops.push(BigInt::from(0));
                    q_vals.push(BigInt::from(0));
                }
            }
        }
        s.insert("queryCredentialIndices".into(), q_cred_idx);
        s.insert("queryFieldIndices".into(), q_field_idx);
        s.insert("queryOperators".into(), q_ops);
        s.insert("queryValues".into(), q_vals);
        s.insert(
            "numPredicates".into(),
            vec![BigInt::from(self.query.predicates.len())],
        );
        s.insert(
            "compoundLogic".into(),
            vec![BigInt::from(self.query.compound_logic as u8)],
        );

        // Verifier / time context
        s.insert(
            "verifierAddress".into(),
            vec![to_big(&self.query.verifier_address)],
        );
        s.insert(
            "verifierNonce".into(),
            vec![to_big(&self.query.verifier_nonce)],
        );
        s.insert(
            "currentTimestamp".into(),
            vec![BigInt::from(self.query.current_timestamp)],
        );

        s
    }
}

/// Zero-schema placeholder that still points at the master identity so the
/// circuit's `IdentityAnchor` gate doesn't reject it. The credential's signature
/// fields are zeroed; the circuit is expected to skip verification for zero-schema
/// slots (honoured by Circom via the branch on `schemaHashes[i] == 0`).
fn placeholder_credential(master_pub: BJJPublicKey) -> Credential {
    use crate::babyjubjub::EdDSASignature;
    Credential {
        schema_hash: [0u8; 32],
        attestation_data: vec![0u64; crate::NUM_FIELDS],
        issuer_signature: EdDSASignature {
            r8_x: [0u8; 32],
            r8_y: [0u8; 32],
            s: [0u8; 32],
        },
        issuer_pub_key: BJJPublicKey { x: [0u8; 32], y: [0u8; 32] },
        holder_pub_key: master_pub,
        salt: [0u8; 32],
        commitment: [0u8; 32],
        tree_leaf_index: None,
        expiration_timestamp: 0,
        issued_at: 0,
    }
}
