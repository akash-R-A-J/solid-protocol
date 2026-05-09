declare module 'snarkjs' {
  export const groth16: {
    fullProve(
      input: unknown,
      wasmFile: string | Uint8Array,
      zkeyFile: string | Uint8Array,
    ): Promise<{ proof: any; publicSignals: string[] }>;
    verify(
      verificationKey: unknown,
      publicSignals: string[],
      proof: unknown,
    ): Promise<boolean>;
  };
}
