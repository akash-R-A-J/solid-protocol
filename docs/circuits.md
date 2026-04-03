# Circuit Design

> Technical details of the compound query ZK circuit

## Circuit: `CompoundQuerySolana(20, 8, 4)`

**Parameters:**
- `TREE_DEPTH = 20` — Merkle tree depth (supports ~1M credentials)
- `NUM_FIELDS = 8` — Fields per attestation schema
- `MAX_PREDICATES = 4` — Maximum predicates per compound query

**Estimated constraints:** ~19,000

## Sub-Circuits

| Circuit | File | Purpose |
|---|---|---|
| `CredentialHasher` | `lib/credential_hasher.circom` | `Poseidon(data[0..7])` |
| `SignatureVerifier` | `lib/signature_verifier.circom` | EdDSA-Poseidon issuer sig verification |
| `MerkleInclusion` | `lib/merkle_inclusion.circom` | SMT inclusion proof (Light Protocol tree) |
| `PredicateEvaluator` | `lib/predicate_evaluator.circom` | Field comparison (7 operators) |
| `FieldSelector` | `lib/predicate_evaluator.circom` | Mux to select field by index |
| `NullifierComputer` | `lib/nullifier_expiry.circom` | `Poseidon(privKey, schema, nonce)` |
| `ExpirationChecker` | `lib/nullifier_expiry.circom` | Timestamp validity check |

## Signal Map

### Public Inputs (Verifier-provided)

| Signal | Type | Description |
|---|---|---|
| `merkleRoot` | Fr | Current Light Protocol state root |
| `schemaHash` | Fr | Schema identifier |
| `issuerPubKeyAx` | Fr | Issuer BJJ public key X |
| `issuerPubKeyAy` | Fr | Issuer BJJ public key Y |
| `queryFieldIndices[4]` | Fr | Which fields to check |
| `queryOperators[4]` | Fr | Comparison operators (0-6) |
| `queryValues[4]` | Fr | Threshold values |
| `numPredicates` | Fr | Active predicate count (1-4) |
| `compoundLogic` | Fr | 0 = AND, 1 = OR |
| `verifierNonce` | Fr | Anti-replay nonce |
| `currentTimestamp` | Fr | Current Unix timestamp |

### Private Inputs (Holder-provided)

| Signal | Type | Description |
|---|---|---|
| `attestationData[8]` | Fr | Raw credential field values |
| `salt` | Fr | Commitment randomness |
| `holderBJJPrivKey` | Fr | Holder's BJJ private key |
| `holderBJJPubKeyAx` | Fr | Holder's BJJ public key X |
| `holderBJJPubKeyAy` | Fr | Holder's BJJ public key Y |
| `issuerSigR8x` | Fr | Issuer signature R8.x |
| `issuerSigR8y` | Fr | Issuer signature R8.y |
| `issuerSigS` | Fr | Issuer signature S scalar |
| `merkleSiblings[20]` | Fr | Merkle proof path |
| `merklePathIndices[20]` | Fr | Path direction bits |
| `expirationTimestamp` | Fr | Credential expiry (0 = never) |

### Public Output

| Signal | Type | Description |
|---|---|---|
| `nullifierHash` | Fr | `Poseidon(privKey, schema, nonce)` |

## Verification Equation

The circuit proves:
1. ✅ Commitment = Poseidon(dataHash, schema, holderX, holderY, salt)
2. ✅ Issuer signature is valid over the commitment
3. ✅ Commitment exists in the Merkle tree (Light Protocol)
4. ✅ Attestation data satisfies the compound query
5. ✅ Credential is not expired
6. ✅ Nullifier is correctly computed (anti-replay)

## Trusted Setup

```bash
cd circuits/

# 1. Compile circuit
circom compound_query.circom --r1cs --wasm --sym --output build/ \
  -l node_modules/circomlib/circuits

# 2. Run trusted setup
node scripts/setup.js

# Output:
#   build/circuit_final.zkey      ← proving key
#   build/verification_key.json   ← embed in on-chain verifier
```

## Proof Generation Performance

| Platform | Time | Notes |
|---|---|---|
| Browser (WASM) | 15-30s | snarkjs + circuit WASM |
| Node.js | 8-15s | snarkjs native |
| Rust (rapid-snark) | 2-4s | Future optimization |
