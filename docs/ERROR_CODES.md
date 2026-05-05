# Error Codes

These are the product-facing errors wallet, console, SDK, and examples should
surface during devnet.

| Code | Meaning | User Action |
| --- | --- | --- |
| `MANIFEST_INVALID` | Manifest is missing required program, artifact, schema, or tree data. | Use the official manifest URL. |
| `PROGRAM_NOT_DEPLOYED` | One or more program IDs are not executable on the selected cluster. | Wait for devnet deploy or switch cluster. |
| `VK_NOT_FINALIZED` | Verifier key storage exists but is not finalized. | Run protocol initialization. |
| `ARTIFACT_PIN_MISMATCH` | Downloaded artifact hash does not match manifest pin. | Stop and report immediately. |
| `ARTIFACT_NOT_CONFIGURED` | Artifact base URL is missing. | Configure manifest or env. |
| `INDEXER_NOT_CONFIGURED` | Wallet cannot fetch Merkle proofs. | Configure indexer URL. |
| `INDEXER_STALE` | Indexer root lags current devnet state. | Retry later or use a healthy indexer. |
| `MISSING_CREDENTIAL` | Holder has no credential for requested schema. | Import the issuer credential package. |
| `EXPIRED_CREDENTIAL` | Matching credential is expired. | Request a fresh credential. |
| `UNSUPPORTED_SCHEMA` | Wallet/verifier cannot resolve schema metadata. | Use a launch schema or update catalog. |
| `USER_REJECTED` | Holder denied proof request. | No action. |
| `PROOF_REQUEST_TIMEOUT` | Wallet did not answer in time. | Retry. |
| `INVALID_PUBLIC_INPUTS` | Proof public inputs do not match requirement. | Treat as failed verification. |
| `NULLIFIER_REPLAY` | Proof nullifier has already been used. | Treat as failed verification. |
| `TRANSACTION_FAILED` | Solana transaction failed or reverted. | Show logs/signature and retry only if safe. |

## Rule

Do not collapse these into generic "failed" messages. Real testers need to
know whether the failure is wallet state, issuer state, indexer state,
artifact integrity, or on-chain verification.
