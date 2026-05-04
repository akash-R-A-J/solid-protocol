/* tslint:disable */
/* eslint-disable */
export const memory: WebAssembly.Memory;
export const poseidonHash: (a: number, b: number, c: number) => void;
export const poseidonHashBytes: (a: number, b: number, c: number) => void;
export const generateBJJKeypair: (a: number) => void;
export const signMessage: (a: number, b: number, c: number, d: number, e: number) => void;
export const verifySignature: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number) => void;
export const computeCommitment: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => void;
export const computeHardenedNullifier: (a: number, b: number, c: number, d: bigint, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => void;
export const generateIdentity: (a: number, b: number, c: number) => void;
export const unlockIdentity: (a: number, b: number, c: number, d: number, e: number) => void;
export const deriveKey: (a: number, b: number, c: number, d: number, e: number) => void;
export const deriveCredentialKey: (a: number, b: number, c: number, d: number, e: number) => void;
export const computeIdentityState: (a: number, b: number, c: number, d: number, e: number, f: bigint) => void;
export const isBjjInPrimeOrderSubgroup: (a: number, b: number, c: number, d: number, e: number) => void;
export const init: () => void;
export const __wbindgen_malloc: (a: number, b: number) => number;
export const __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
export const __wbindgen_exn_store: (a: number) => void;
export const __wbindgen_free: (a: number, b: number, c: number) => void;
export const __wbindgen_add_to_stack_pointer: (a: number) => number;
export const __wbindgen_start: () => void;
