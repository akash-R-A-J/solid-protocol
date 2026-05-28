# @solid-protocol/core

WASM-backed cryptographic primitives for SolID Protocol. This is the foundation
of every other `@solid-protocol/*` package — it loads the Rust `solid-core`
WASM bridge and exposes typed wrappers around Poseidon hashing, BabyJubJub
EdDSA, schema-hash derivation, commitment construction, and nullifier
computation.

No crypto is reimplemented in TypeScript. Every function ends in a single
`wasm-bindgen` call into bytes that are byte-identical to what the on-chain
program computes. The Rust-↔-TS cross-language vector suite
(`tests/vectors/commitment_and_nullifier.json` in the protocol repo) is the
load-bearing CI gate that enforces this contract.

## Install

```bash
npm install @solid-protocol/core
```

## Initialize the WASM module before any call

```ts
import { initWasm } from "@solid-protocol/core";

await initWasm();
```

`initWasm()` auto-detects the runtime and loads `../wasm-web/` in the browser
or `../wasm/` under Node. Built-in source of truth: the protocol README's
"Compile WebAssembly Primitives" step writes the WASM output into
`ts-sdk/packages/core/wasm/`.

## Minimal example — derive a holder identity and compute a commitment

```ts
import {
  initWasm,
  generateKeypair,
  computeCommitment,
  poseidonHashBytes,
} from "@solid-protocol/core";

await initWasm();

const keypair = generateKeypair();
const schemaHash = poseidonHashBytes([new Uint8Array(32) /* placeholder */]);
const salt = crypto.getRandomValues(new Uint8Array(32));
const dataFields = [25n, 356n, 1n, 3n, 2n, 1778315365n, 356n, 0n]; // 8 fields

const commitment = computeCommitment(
  dataFields,
  schemaHash,
  keypair.public_key_x,
  keypair.public_key_y,
  salt,
);
```

## Public surface

| Group | Functions |
| --- | --- |
| Init | `initWasm` |
| Poseidon | `poseidonHash`, `poseidonHashBytes`, `poseidonCompressBytes` |
| Schema | `computeSchemaHash` |
| BabyJubJub | `generateKeypair`, `isInPrimeOrderSubgroup`, `sign`, `verify` |
| Commitment / Nullifier | `computeCommitment`, `computeNullifier`, `computeIdentityCommitment`, `computeIdentityState`, `computeIssuerLeaf` |
| Identity | `generateIdentity`, `unlockIdentity`, `deriveKey`, `deriveCredentialKey` |
| Query | `QueryBuilder`, `OP_MAP`, `Predicate`, `MultiCredentialQuery` |

Constants exported for convenience: `MAX_CREDENTIALS`, `MAX_PREDICATES`,
`NUM_FIELDS`, `TREE_DEPTH`, `PROGRAM_IDS`.

## Subgroup check (SEC-048 Phase E)

`isInPrimeOrderSubgroup(publicKeyX, publicKeyY)` is a UX/pre-submit early-fail
predicate. The load-bearing soundness gate is the on-chain Groth16 subgroup
proof consumed by `register_issuer`; a failing key cannot produce a valid
proof, so on-chain verify always rejects. Use the predicate to avoid paying
for snarkjs proving and a tx submission before you find out the key is bad.

## Related packages

- `@solid-protocol/verifier` — app and verifier integrations (high-level).
- `@solid-protocol/holder` — holder-side proof generation.
- `@solid-protocol/issuer` — issuer-side credential issuance.
- `@solid-protocol/channel` — encrypted credential delivery.
- `@solid-protocol/light` — SPL Account Compression adapter and Merkle proof utilities.
- `@solid-protocol/sdk` — single-import facade over the above.

## License

MIT
