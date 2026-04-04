import { Connection, Keypair, PublicKey, SystemProgram } from '@solana/web3.js';
import * as anchor from '@coral-xyz/anchor';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

// ─── Program IDs ───────────────────────────────────────────────────────────
const PROGRAM_IDS = {
    schemaRegistry:  new PublicKey('2oma2yU2vfSYNR8sPDq5JvRbrtetyk5wuuGzwGYupAsH'),
    zkVerifier:      new PublicKey('FhtEvsUwxRRT8nf3uaxK6iAefFqvqCYimc9vqnH5XEgr'),
    issuerRegistry:  new PublicKey('6ewriDVJLTaeBG7AsjuobAfyhDWfCz681RDWMFq5Laeo'),
};

const RPC_URL = 'https://api.devnet.solana.com';

// ─── Load Environment ──────────────────────────────────────────────────────
const keypairPath = path.join(os.homedir(), '.config/solana/id.json');
const secretKey = JSON.parse(fs.readFileSync(keypairPath, 'utf-8'));
const wallet = Keypair.fromSecretKey(Uint8Array.from(secretKey));
const connection = new Connection(RPC_URL, 'confirmed');

const provider = new anchor.AnchorProvider(
    connection,
    new anchor.Wallet(wallet),
    { commitment: 'confirmed' }
);
anchor.setProvider(provider);

// ─── VK Serialization Functions ───────────────────────────────────────────

function fieldToBytes(s: string): Uint8Array {
    let n = BigInt(s);
    const bytes = new Uint8Array(32);
    for (let i = 31; i >= 0; i--) {
        bytes[i] = Number(n & 0xFFn);
        n >>= 8n;
    }
    return bytes;
}

function serializeG1(point: string[]): Uint8Array {
    const buf = new Uint8Array(64);
    buf.set(fieldToBytes(point[0]), 0);
    buf.set(fieldToBytes(point[1]), 32);
    return buf;
}

function serializeG2(point: string[][]): Uint8Array {
    const buf = new Uint8Array(128);
    buf.set(fieldToBytes(point[0][0]), 0);
    buf.set(fieldToBytes(point[0][1]), 32);
    buf.set(fieldToBytes(point[1][0]), 64);
    buf.set(fieldToBytes(point[1][1]), 96);
    return buf;
}

// ─── IDL Loader ───────────────────────────────────────────────────────────

function loadIdl(name: string): any {
    const snakeName = name.replace(/[A-Z]/g, letter => `_${letter.toLowerCase()}`);
    const seaids = [name, snakeName];
    const subdirs = ['idl', 'types', 'deploy'];
    
    for (const subdir of subdirs) {
        for (const filename of seaids) {
            const p = path.join('target', subdir, `${filename}.json`);
            if (fs.existsSync(p)) {
                console.log(`   Loading IDL: ${p}`);
                const idl = JSON.parse(fs.readFileSync(p, 'utf-8'));
                // Anchor 0.30+ internal fix: ensure address is present
                if (!idl.address && PROGRAM_IDS[name as keyof typeof PROGRAM_IDS]) {
                    idl.address = PROGRAM_IDS[name as keyof typeof PROGRAM_IDS].toBase58();
                }
                return idl;
            }
        }
    }
    throw new Error(`IDL for ${name} not found in target/ (checked idl/, types/, deploy/)`);
}

async function main() {
    console.log('═══════════════════════════════════════');
    console.log('  SolID Protocol — On-Chain Init');
    console.log('═══════════════════════════════════════');
    console.log(`Wallet: ${wallet.publicKey.toBase58()}`);

    // Load IDLs
    const issuerIdl = loadIdl('issuerRegistry');
    const schemaIdl = loadIdl('schemaRegistry');
    const zkIdl     = loadIdl('zkVerifier');

    // Use constructor compatible with 0.30.1
    const issuerProgram = new anchor.Program(issuerIdl, provider);
    const schemaProgram = new anchor.Program(schemaIdl, provider);
    const zkProgram     = new anchor.Program(zkIdl,     provider);

    // Load WASM for Poseidon
    const { initWasm, poseidonHashBytes } = require('@solid-protocol/core');
    await initWasm();

    // 1. Initialize Issuer Registry
    console.log('\n[1/5] Initializing Issuer Registry...');
    const [registryPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('registry-config')],
        PROGRAM_IDS.issuerRegistry
    );
    try {
        await issuerProgram.methods.initializeRegistry(
            new anchor.BN(1_000_000_000), // 1 SOL
            new anchor.BN(86400),
            new anchor.BN(6000),
        ).accounts({
            registryConfig: registryPda,
            authority: wallet.publicKey,
            systemProgram: SystemProgram.programId,
        }).rpc();
        console.log('   ✅ Initialized');
    } catch (e: any) {
        console.log('   ⚠️  Skip: ' + (e.message.includes('already in use') ? 'Already active' : e.message));
    }

    // 2. Register Schema
    console.log('\n[2/5] Registering basic_identity_v1 schema...');
    const schemaName = 'basic_identity_v1';
    const fields = ['age', 'country_code', 'region', 'id_type', 'verification_level', 'issued_date', 'nationality', '_reserved'];
    
    // Chunk/Pad metadata string to 32-byte blocks for Poseidon
    const metadataBytes = Buffer.from(fields.join(','));
    const chunkCount = Math.ceil(metadataBytes.length / 32);
    const schemaChunks: Uint8Array[] = [];
    for (let i = 0; i < chunkCount; i++) {
        const chunk = new Uint8Array(32);
        const source = metadataBytes.subarray(i * 32, (i + 1) * 32);
        chunk.set(source);
        schemaChunks.push(chunk);
    }
    const schemaHash = poseidonHashBytes(schemaChunks);
    console.log(`   Schema Hash: ${Buffer.from(schemaHash).toString('hex')}`);

    const [schemaPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('schema'), Buffer.from(schemaName)],
        PROGRAM_IDS.schemaRegistry
    );
    try {
        await schemaProgram.methods.registerSchema(
            schemaName,
            1,
            'Identity',
            fields,
            Array.from(schemaHash),
        ).accounts({
            schemaAccount: schemaPda,
            authority: wallet.publicKey,
            systemProgram: SystemProgram.programId,
        }).rpc();
        console.log('   ✅ Registered');
    } catch (e: any) {
        console.log('   ⚠️  Skip: ' + (e.message.includes('already in use') ? 'Already active' : e.message));
    }

    // 3. Initialize ZK Verifier
    console.log('\n[3/5] Initializing ZK Verifier...');
    const [verifierConfigPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('verifier-config')],
        PROGRAM_IDS.zkVerifier
    );
    try {
        await zkProgram.methods.initialize()
            .accounts({
                verifierConfig: verifierConfigPda,
                authority: wallet.publicKey,
                systemProgram: SystemProgram.programId,
            }).rpc();
        console.log('   ✅ Initialized');
    } catch (e: any) {
        console.log('   ⚠️  Skip: ' + (e.message.includes('already in use') ? 'Already active' : e.message));
    }

    // 4. Upload VK
    console.log('\n[4/5] Uploading Verification Key...');
    const [vkStoragePda] = PublicKey.findProgramAddressSync(
        [Buffer.from('vk-storage'), verifierConfigPda.toBuffer()],
        PROGRAM_IDS.zkVerifier
    );

    const vkJson = JSON.parse(fs.readFileSync('circuits/build/verification_key.json', 'utf-8'));
    const vkBytes = Buffer.concat([
        serializeG1(vkJson.vk_alpha_1),
        serializeG2(vkJson.vk_beta_2),
        serializeG2(vkJson.vk_gamma_2),
        serializeG2(vkJson.vk_delta_2),
        Buffer.concat(vkJson.IC.map(serializeG1))
    ]);

    const CHUNK_SIZE = 900;
    const chunks = Math.ceil(vkBytes.length / CHUNK_SIZE);
    for (let i = 0; i < chunks; i++) {
        const chunk = vkBytes.subarray(i * CHUNK_SIZE, (i + 1) * CHUNK_SIZE);
        await zkProgram.methods.storeVerificationKey(
            i,
            Array.from(chunk),
            i === chunks - 1,
        ).accounts({
            verifierConfig: verifierConfigPda,
            vkStorage: vkStoragePda,
            authority: wallet.publicKey,
            systemProgram: SystemProgram.programId,
        }).rpc();
        process.stdout.write(`   🚀 Uploading VK chunk ${i + 1}/${chunks}...\r`);
    }
    console.log(`\n   ✅ VK Uploaded (${vkBytes.length} bytes)`);

    // 5. Initialize Bloom Filter
    console.log('\n[5/5] Initializing Bloom filter...');
    const [nullifierBloomPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('nullifier-bloom'), verifierConfigPda.toBuffer()],
        PROGRAM_IDS.zkVerifier
    );
    try {
        await zkProgram.methods.initNullifierBloom()
            .accounts({
                nullifierBloom: nullifierBloomPda,
                verifierConfig: verifierConfigPda,
                authority: wallet.publicKey,
                systemProgram: SystemProgram.programId,
            }).rpc();
        console.log('   ✅ Initialized');
    } catch (e: any) {
        console.log('   ⚠️  Skip: ' + (e.message.includes('already in use') ? 'Already active' : e.message));
    }

    console.log('\n✨ On-chain initialization complete!');
}

main().catch(console.error);
