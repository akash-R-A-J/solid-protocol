import { Connection, Keypair, PublicKey } from '@solana/web3.js';
import * as anchor from '@coral-xyz/anchor';
import { initWasm, generateKeypair, poseidonHashBytes } from '@solid-protocol/core';
import { issueCredential } from '@solid-protocol/issuer';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

// ─── Program IDs ───────────────────────────────────────────────────────────
const PROGRAM_IDS = {
    schemaRegistry:  new PublicKey('2oma2yU2vfSYNR8sPDq5JvRbrtetyk5wuuGzwGYupAsH'),
    zkVerifier:      new PublicKey('FhtEvsUwxRRT8nf3uaxK6iAefFqvqCYimc9vqnH5XEgr'),
    issuerRegistry:  new PublicKey('6ewriDVJLTaeBG7AsjuobAfyhDWfCz681RDWMFq5Laeo'),
};

// ─── IDL Loader ───────────────────────────────────────────────────────────
function loadIdl(name: string): any {
    const snakeName = name.replace(/[A-Z]/g, letter => `_${letter.toLowerCase()}`);
    const seaids = [name, snakeName];
    const subdirs = ['idl', 'types', 'deploy'];
    for (const subdir of subdirs) {
        for (const filename of seaids) {
            const p = path.join('target', subdir, `${filename}.json`);
            if (fs.existsSync(p)) {
                const idl = JSON.parse(fs.readFileSync(p, 'utf-8'));
                if (!idl.address && PROGRAM_IDS[name as keyof typeof PROGRAM_IDS]) {
                    idl.address = PROGRAM_IDS[name as keyof typeof PROGRAM_IDS].toBase58();
                }
                return idl;
            }
        }
    }
    throw new Error(`IDL for ${name} not found`);
}

async function main() {
    await initWasm();

    const RPC_URL = 'https://api.devnet.solana.com';
    const connection = new Connection(RPC_URL, 'confirmed');

    // Load wallet
    const keypairPath = path.join(os.homedir(), '.config/solana/id.json');
    const secretKey = JSON.parse(fs.readFileSync(keypairPath, 'utf-8'));
    const wallet = Keypair.fromSecretKey(Uint8Array.from(secretKey));

    console.log('═══════════════════════════════════════');
    console.log('  SolID Protocol — Step 10: Issuance');
    console.log('═══════════════════════════════════════');

    // 1. Generate Identities
    console.log('[1/3] Generating identities...');
    const issuerKp = generateKeypair();
    const holderKp = generateKeypair();

    // 2. Define Schema & Data
    const fields = ['age', 'country_code', 'region', 'id_type', 'verification_level', 'issued_date', 'nationality', '_reserved'];
    const schemaHash = poseidonHashBytes([Buffer.from(fields.join(','))]);
    console.log(`   Schema Hash: ${Buffer.from(schemaHash).toString('hex')}`);

    const attestationData = [
        21n,            // age
        840n,           // US
        1n,             // CA
        0n,             // Passport
        2n,             // High
        BigInt(Math.floor(Date.now() / 1000)),
        840n,
        0n
    ];

    // 3. Issue & Compress
    console.log('\n[2/3] Issuing and compressing credential...');
    const credential = await issueCredential(
        issuerKp.private_key,
        issuerKp.public_key_x,
        issuerKp.public_key_y,
        {
            schemaHash: schemaHash,
            attestationData: attestationData,
            holderPubKeyX: holderKp.public_key_x,
            holderPubKeyY: holderKp.public_key_y,
        },
        { payer: wallet }
    );

    console.log('\n[3/3] Credential Created!');
    console.log(`   Commitment: ${Buffer.from(credential.commitment).toString('hex')}`);
    
    // Save state
    const state = {
        issuerKp,
        holderKp,
        credential,
        attestationData: attestationData.map(n => n.toString()),
        schemaHash: Buffer.from(schemaHash).toString('hex')
    };
    fs.writeFileSync('scripts/e2e_state.json', JSON.stringify(state, null, 2));
    console.log('\n✅ State saved to scripts/e2e_state.json');
}

main().catch(console.error);
