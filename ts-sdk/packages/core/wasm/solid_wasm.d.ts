/* tslint:disable */
/* eslint-disable */
/**
 * Initialize panic hook for better error messages in browser console.
 */
export function init(): void;
/**
 * Compute Poseidon hash of u64 field values. Returns 32-byte LE hash.
 */
export function poseidonHash(fields: BigUint64Array): Uint8Array;
/**
 * Compute Poseidon hash of byte arrays (each 32 bytes LE).
 */
export function poseidonHashBytes(inputs: Uint8Array): Uint8Array;
/**
 * Generate a new BabyJubJub keypair. Returns `{ privateKey, publicKeyX, publicKeyY }`.
 */
export function generateBJJKeypair(): any;
/**
 * Sign a message (32 bytes) with a BJJ private key (32 bytes).
 */
export function signMessage(private_key: Uint8Array, message: Uint8Array): any;
/**
 * Verify an EdDSA-Poseidon signature.
 */
export function verifySignature(pub_key_x: Uint8Array, pub_key_y: Uint8Array, message: Uint8Array, r8x: Uint8Array, r8y: Uint8Array, s: Uint8Array): boolean;
/**
 * Compute the attestation commitment.
 *
 * `commitment = Poseidon(dataHash, schemaHash, holderX, holderY, salt)`
 * where `dataHash = Poseidon(data[0..NUM_FIELDS-1])`.
 */
export function computeCommitment(data_fields: BigUint64Array, schema_hash: Uint8Array, holder_pub_key_x: Uint8Array, holder_pub_key_y: Uint8Array, salt: Uint8Array): Uint8Array;
/**
 * Compute the hardened (6-input) nullifier hash.
 *
 * `nullifier = Poseidon(masterKey, revocationNonce, verifierAddress,`
 * `                     queryContextHash, verifierNonce, issuerTreeRoot)`
 *
 * ADR-0014 / SOLID-SEC-008: the 6th input binds every proof to a
 * specific issuer-tree epoch.  See `crates/solid-core/src/nullifier.rs`
 * for the full rationale.
 */
export function computeHardenedNullifier(master_key: Uint8Array, revocation_nonce: bigint, verifier_address: Uint8Array, query_context_hash: Uint8Array, verifier_nonce: Uint8Array, issuer_tree_root: Uint8Array): Uint8Array;
/**
 * Generate an encrypted BJJ identity bundle (JSON).
 */
export function generateIdentity(passphrase: string): string;
/**
 * Unlock an encrypted BJJ identity, returning the master private key (32 bytes).
 */
export function unlockIdentity(identity_json: string, passphrase: string): Uint8Array;
/**
 * Derive a deterministic sub-key: `Poseidon(masterKey, context)`.
 */
export function deriveKey(master_key: Uint8Array, context: Uint8Array): Uint8Array;
/**
 * Derive the per-schema BabyJubJub keypair the circuit expects.
 *
 * Mirrors `IdentityAnchor` in `circuits/lib/identity_anchor.circom`:
 *
 * ```text
 *   credentialPrivKey = Poseidon(masterIdentityKey, schemaHash)
 *   (credentialPubKeyAx, credentialPubKeyAy) = BabyPbk(credentialPrivKey)
 * ```
 *
 * Returns `{ privateKey, publicKeyX, publicKeyY }` matching the shape of
 * `generateBJJKeypair`. The holder SDK uses this to compute the per-schema
 * identity leaf `Poseidon(pubKeyX, pubKeyY, revocationNonce)` that actually
 * sits in the global-state tree, instead of the wrong
 * `Poseidon(masterPubKeyX, masterPubKeyY, revocationNonce)` used by the
 * pre-remediation SDK (BUG-04).
 */
export function deriveCredentialKey(master_key: Uint8Array, schema_hash: Uint8Array): any;
/**
 * Compute the identity-state commitment: `Poseidon(pubKeyX, pubKeyY, revocationNonce)`.
 */
export function computeIdentityState(pubkey_x: Uint8Array, pubkey_y: Uint8Array, revocation_nonce: bigint): Uint8Array;
/**
 * SOLID-SEC-007: predicate check for BJJ subgroup membership, exposed to the
 * TS SDK for early client-side validation before a `register_issuer` call is
 * signed.  Returns `true` iff the point is on the curve, not the identity,
 * and lies in the prime-order subgroup (no cofactor-8 torsion component).
 * The on-chain program is the definitive enforcement point; this is a UX
 * shortcut so holders / issuers get immediate feedback instead of a
 * confirmed-transaction rejection.
 */
export function isBjjInPrimeOrderSubgroup(pubkey_x: Uint8Array, pubkey_y: Uint8Array): boolean;
