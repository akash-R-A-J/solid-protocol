import type { CredentialBundle } from '../src/types.js';

/**
 * A fully-populated CredentialBundle used across channel tests.
 *
 * The cryptographic content is structurally valid (correct shapes and
 * lengths) but is not a real on-chain commitment.  Channel tests verify
 * transport behaviour, not credential validity -- the latter is the job
 * of the issuer/holder/verifier packages.
 */
export function makeBundle(overrides: Partial<CredentialBundle> = {}): CredentialBundle {
  return {
    version: 1,
    schemaHash: '11'.repeat(32),
    schemaName: 'basic_identity_v1',
    attestationData: ['25', '840', '2', '1', '0', '0', '0', '0'],
    issuerSignature: {
      R8x: '12345678901234567890123456789012345678901234567890',
      R8y: '98765432109876543210987654321098765432109876543210',
      S: '11223344556677889900112233445566778899001122334455',
    },
    issuerPublicKey: {
      x: '11111111111111111111111111111111111111111111111111',
      y: '22222222222222222222222222222222222222222222222222',
    },
    holderPublicKey: {
      x: '33333333333333333333333333333333333333333333333333',
      y: '44444444444444444444444444444444444444444444444444',
    },
    salt: '12345678901234567890',
    commitment: 'aa'.repeat(32),
    expirationTimestamp: 0,
    treeAddress: 'TreE1111111111111111111111111111111111111111',
    merkleProof: {
      siblings: ['cc'.repeat(32), 'dd'.repeat(32)],
      pathIndices: [0, 1],
      leafIndex: 7,
    },
    issuerName: 'Console Test Issuer',
    issuerAuthority: 'AutH1111111111111111111111111111111111111111',
    issuerStatusEpoch: '1',
    issuerRevocationNonce: '0',
    issuerTreeLeafIndex: '0',
    ...overrides,
  };
}
