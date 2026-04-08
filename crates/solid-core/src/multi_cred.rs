use std::collections::HashMap;
use num_bigint::BigInt;
use crate::{Credential, MultiCredentialQuery, BJJPublicKey};

/// Phase 3.1: Multi-Credential Witness Generator
/// Collects N=4 credentials and prepares signal mapping for Circom.
pub struct BatchCredentialWitness {
    pub master_identity_key: [u8; 32],
    pub master_identity_pub_key: BJJPublicKey,
    pub revocation_nonce: u64,
    pub credentials: Vec<Credential>,
    pub global_siblings: Vec<Vec<[u8; 32]>>,
    pub global_path_indices: Vec<Vec<u64>>,
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
            global_siblings: Vec::new(),
            global_path_indices: Vec::new(),
            query,
        }
    }

    /// Add a credential to the batch, verifying it belongs to the same master identity.
    pub fn add_credential(
        &mut self,
        cred: Credential,
        global_proof: (Vec<[u8; 32]>, Vec<u64>),
    ) -> Result<(), String> {
        if self.credentials.len() >= crate::MAX_CREDENTIALS {
            return Err("Batch full (N=4)".into());
        }

        // SEC-17: Identity Binding Cohesion Check
        if cred.holder_pub_key != self.master_identity_pub_key {
            return Err("Credential identity mismatch. All credentials in batch must belong to the same master identity.".into());
        }

        self.credentials.push(cred);
        self.global_siblings.push(global_proof.0);
        self.global_path_indices.push(global_proof.1);
        Ok(())
    }

    /// Automatically pad the batch to NUM_CREDS (4) with zero-schemas and zero-data.
    pub fn pad(&mut self) {
        let dummy_key = BJJPublicKey { x: [0u8; 32], y: [0u8; 32] };
        while self.credentials.len() < crate::MAX_CREDENTIALS {
            self.credentials.push(Credential {
                schema_hash: [0u8; 32],
                merkle_root: [0u8; 32],
                issuer_pub_key: dummy_key.clone(),
                holder_pub_key: self.master_identity_pub_key.clone(),
                attestation_data: [0u64; 8],
                salt: [0u8; 32],
                issuer_sig_r8: [0u8; 32],
                issuer_sig_s: [0u8; 32],
                merkle_siblings: vec![[0u8; 32]; 20],
                merkle_path_indices: vec![0; 20],
                expiration_timestamp: 0,
            });
        }
        
        while self.global_siblings.len() < crate::MAX_CREDENTIALS {
            self.global_siblings.push(vec![[0u8; 32]; 20]);
            self.global_path_indices.push(vec![0; 20]);
        }
    }

    /// PHASE 4: Bridge to Circom
    pub fn to_circom_signals(&self) -> HashMap<String, Vec<BigInt>> {
        let mut signals = HashMap::new();

        fn to_bigint(bytes: &[u8; 32]) -> BigInt {
            BigInt::from_bytes_le(num_bigint::Sign::Plus, bytes)
        }

        // 1. Context & Global State
        signals.insert("globalRoot".to_string(), vec![to_bigint(&self.query.global_root)]);
        signals.insert("masterIdentityKey".to_string(), vec![to_bigint(&self.master_identity_key)]);
        signals.insert("revocationNonce".to_string(), vec![BigInt::from(self.revocation_nonce)]);
        
        let mut flat_global_siblings = Vec::new();
        let mut flat_global_indices = Vec::new();
        
        for i in 0..crate::MAX_CREDENTIALS {
            let siblings = self.global_siblings.get(i).cloned().unwrap_or_else(|| vec![[0u8; 32]; 20]);
            let indices = self.global_path_indices.get(i).cloned().unwrap_or_else(|| vec![0; 20]);
            for sibling in &siblings {
                flat_global_siblings.push(to_bigint(sibling));
            }
            for &idx in &indices {
                flat_global_indices.push(BigInt::from(idx));
            }
        }
        signals.insert("globalSiblings".to_string(), flat_global_siblings);
        signals.insert("globalPathIndices".to_string(), flat_global_indices);

        // 2. Batch Data (N=4)
        let mut merkle_roots = Vec::new();
        let mut schema_hashes = Vec::new();
        let mut data = Vec::new();
        let mut salts = Vec::new();
        let mut issuer_sig_r8xs = Vec::new();
        let mut issuer_sig_r8ys = Vec::new();
        let mut issuer_sig_ss = Vec::new();
        let mut issuer_pub_key_axs = Vec::new();
        let mut issuer_pub_key_ays = Vec::new();
        let mut merkle_siblings = Vec::new();
        let mut merkle_path_indices = Vec::new();
        let mut expiration_timestamps = Vec::new();
        
        for i in 0..crate::MAX_CREDENTIALS {
             let cred = &self.credentials[i];
             merkle_roots.push(to_bigint(&cred.merkle_root));
             schema_hashes.push(to_bigint(&cred.schema_hash));
             
             for &f in &cred.attestation_data {
                 data.push(BigInt::from(f));
             }
             salts.push(to_bigint(&cred.salt));
             issuer_sig_r8xs.push(to_bigint(&cred.issuer_sig_r8));
             issuer_sig_r8ys.push(BigInt::from(0)); // Deduced or Y-coord if available
             issuer_sig_ss.push(to_bigint(&cred.issuer_sig_s));
             issuer_pub_key_axs.push(to_bigint(&cred.issuer_pub_key.x));
             issuer_pub_key_ays.push(to_bigint(&cred.issuer_pub_key.y));
             
             for s in &cred.merkle_siblings {
                 merkle_siblings.push(to_bigint(s));
             }
             for &idx in &cred.merkle_path_indices {
                 merkle_path_indices.push(BigInt::from(idx));
             }
             expiration_timestamps.push(BigInt::from(cred.expiration_timestamp));
        }
        
        signals.insert("merkleRoots".to_string(), merkle_roots);
        signals.insert("schemaHashes".to_string(), schema_hashes);
        signals.insert("data".to_string(), data);
        signals.insert("salts".to_string(), salts);
        signals.insert("issuerSigR8xs".to_string(), issuer_sig_r8xs);
        signals.insert("issuerSigR8ys".to_string(), issuer_sig_r8ys);
        signals.insert("issuerSigSs".to_string(), issuer_sig_ss);
        signals.insert("issuerPubKeyAxs".to_string(), issuer_pub_key_axs);
        signals.insert("issuerPubKeyAys".to_string(), issuer_pub_key_ays);
        signals.insert("merkleSiblings".to_string(), merkle_siblings);
        signals.insert("merklePathIndices".to_string(), merkle_path_indices);
        signals.insert("expirationTimestamps".to_string(), expiration_timestamps);

        // 3. Query signals
        signals.insert("queryCredentialIndices".to_string(), self.query.predicates.iter().map(|p| BigInt::from(p.credential_index)).chain(std::iter::repeat(BigInt::from(0))).take(crate::MAX_PREDICATES).collect());
        signals.insert("queryFieldIndices".to_string(), self.query.predicates.iter().map(|p| BigInt::from(p.field_index)).chain(std::iter::repeat(BigInt::from(0))).take(crate::MAX_PREDICATES).collect());
        signals.insert("queryOperators".to_string(), self.query.predicates.iter().map(|p| BigInt::from(p.operator as u8)).chain(std::iter::repeat(BigInt::from(0))).take(crate::MAX_PREDICATES).collect());
        signals.insert("queryValues".to_string(), self.query.predicates.iter().map(|p| BigInt::from(p.value)).chain(std::iter::repeat(BigInt::from(0))).take(crate::MAX_PREDICATES).collect());
        signals.insert("numPredicates".to_string(), vec![BigInt::from(self.query.predicates.len())]);
        signals.insert("compoundLogic".to_string(), vec![BigInt::from(self.query.compound_logic as u8)]);

        // 4. Verifier info
        signals.insert("verifierAddress".to_string(), vec![to_bigint(&self.query.verifier_addr)]);
        signals.insert("verifierNonce".to_string(), vec![to_bigint(&self.query.verifier_nonce)]);
        signals.insert("currentTimestamp".to_string(), vec![BigInt::from(self.query.current_timestamp)]);

        signals
    }
}
