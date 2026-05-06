import {
  computeCommitment,
  deriveCredentialKey,
  initWasm,
  isInPrimeOrderSubgroup,
  verify,
} from '@solid-protocol/core';

export interface CredentialIntegrityBundle {
  schemaHash: string;
  attestationData: string[];
  issuerSignature: {
    R8x: string;
    R8y: string;
    S: string;
  };
  issuerPublicKey: {
    x: string;
    y: string;
  };
  holderPublicKey: {
    x: string;
    y: string;
  };
  salt: string;
  commitment: string;
}

export async function assertCredentialIsSoundForWallet(
  bundle: CredentialIntegrityBundle,
  masterSeed: Uint8Array,
): Promise<void> {
  await initWasm();
  const schemaHash = hexToBytes32(bundle.schemaHash, 'schemaHash');
  const derivedHolder = deriveCredentialKey(masterSeed, schemaHash);
  const holderPubKeyX = decimalFieldToBytes(bundle.holderPublicKey.x, 'holderPublicKey.x');
  const holderPubKeyY = decimalFieldToBytes(bundle.holderPublicKey.y, 'holderPublicKey.y');

  if (
    bytesToHex(derivedHolder.public_key_x) !== bytesToHex(holderPubKeyX)
    || bytesToHex(derivedHolder.public_key_y) !== bytesToHex(holderPubKeyY)
  ) {
    throw new Error('WRONG_HOLDER: credential was not issued to this wallet-derived holder key for the schema.');
  }
  if (!isInPrimeOrderSubgroup(holderPubKeyX, holderPubKeyY)) {
    throw new Error('INVALID_HOLDER_KEY: holder BabyJubJub public key is not in the prime-order subgroup.');
  }

  const issuerPubKeyX = decimalFieldToBytes(bundle.issuerPublicKey.x, 'issuerPublicKey.x');
  const issuerPubKeyY = decimalFieldToBytes(bundle.issuerPublicKey.y, 'issuerPublicKey.y');
  if (!isInPrimeOrderSubgroup(issuerPubKeyX, issuerPubKeyY)) {
    throw new Error('INVALID_ISSUER_KEY: issuer BabyJubJub public key is not in the prime-order subgroup.');
  }

  const commitment = hexToBytes32(bundle.commitment, 'commitment');
  const computedCommitment = computeCommitment(
    bundle.attestationData.map((value) => BigInt(value)),
    schemaHash,
    holderPubKeyX,
    holderPubKeyY,
    decimalFieldToBytes(bundle.salt, 'salt'),
  );
  if (bytesToHex(computedCommitment) !== bytesToHex(commitment)) {
    throw new Error('INVALID_COMMITMENT: credential commitment does not match its attestation data and holder key.');
  }

  if (!verify(issuerPubKeyX, issuerPubKeyY, commitment, {
    r8_x: decimalFieldToBytes(bundle.issuerSignature.R8x, 'issuerSignature.R8x'),
    r8_y: decimalFieldToBytes(bundle.issuerSignature.R8y, 'issuerSignature.R8y'),
    s: decimalFieldToBytes(bundle.issuerSignature.S, 'issuerSignature.S'),
  })) {
    throw new Error('INVALID_ISSUER_SIGNATURE: issuer signature does not verify over the credential commitment.');
  }
}

function decimalFieldToBytes(value: string, name: string): Uint8Array {
  if (!/^(0|[1-9][0-9]*)$/.test(value.trim())) {
    throw new Error(`${name} must be a canonical unsigned decimal string`);
  }
  const bytes = new Uint8Array(32);
  let remaining = BigInt(value);
  for (let i = 0; i < 32; i++) {
    bytes[i] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  if (remaining !== 0n) {
    throw new Error(`${name} overflows 32 bytes`);
  }
  return bytes;
}

function hexToBytes32(value: string, name: string): Uint8Array {
  const hex = value.trim().replace(/^0x/i, '').toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(hex)) {
    throw new Error(`${name} must be a 32-byte hex string`);
  }
  const bytes = new Uint8Array(32);
  for (let i = 0; i < 32; i++) {
    bytes[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
}
