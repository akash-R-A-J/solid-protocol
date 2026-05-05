import { Keypair } from '@solana/web3.js';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

export function resolveKeypairPath(): string {
  return (
    process.env.SOLID_KEYPAIR_PATH ??
    process.env.SOLANA_KEYPAIR_PATH ??
    path.join(os.homedir(), '.config/solana/id.json')
  );
}

export function loadKeypair(): Keypair {
  const keypairPath = resolveKeypairPath();
  const secretKey = JSON.parse(fs.readFileSync(keypairPath, 'utf-8'));
  return Keypair.fromSecretKey(Uint8Array.from(secretKey));
}
