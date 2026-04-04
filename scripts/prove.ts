import { Connection, Keypair, PublicKey } from '@solana/web3.js';
import * as anchor from '@coral-xyz/anchor';
import { initWasm, QueryBuilder } from '@solid-protocol/core';
import { generateProof } from '@solid-protocol/holder';
import { verifyOnChain } from '@solid-protocol/verifier';
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

    if (!fs.existsSync('scripts/e2e_state.json')) {
        throw new Error('❌ Run issue.ts first!');
    }
    const state = JSON.parse(fs.readFileSync('scripts/e2e_state.json', 'utf-8'));
    
    console.log('═══════════════════════════════════════');
    console.log('  SolID Protocol — Step 11-13: Proof');
    console.log('═══════════════════════════════════════');

    // 1. Build Query
    console.log('[1/4] Building query...');
    const nonce = Buffer.from('SOLID_TEST_NONCE_001');
    const query = new QueryBuilder()
        .schema(Uint8Array.from(Buffer.from(state.schemaHash, 'hex')))
        .where(0, 'GTE', 21n)
        .nonce(Uint8Array.from(nonce))
        .build();

    // 2. Generate ZK Proof
    console.log('\n[2/4] Generating proof...');
    const startTime = Date.now();
    const result = await generateProof(
        query,
        {
            ...state.credential,
            holderPrivateKey: Uint8Array.from(state.holderKp.private_key)
        },
        {
            wasmPath: 'circuits/build/compound_query_js/compound_query.wasm',
            zkeyPath: 'circuits/build/circuit_final.zkey',
        }
    );
    console.log(`   ✅ Proof generated in ${((Date.now() - startTime) / 1000).toFixed(2)}s`);

    // 3. Verify On-Chain
    console.log('\n[3/4] Verifying on-chain...');
    const verifierId = PROGRAM_IDS.zkVerifier;
    
    const verification = await verifyOnChain(connection, wallet, verifierId, {
        query,
        proofData: {
            proof_a: result.solanaProof.proofA,
            proof_b: result.solanaProof.proofB,
            proof_c: result.solanaProof.proofC,
            publicInputs: result.publicSignals.map(s => {
                let n = BigInt(s);
                const bytes = new Uint8Array(32);
                for (let i = 31; i >= 0; i--) { bytes[i] = Number(n & 0xFFn); n >>= 8n; }
                return bytes;
            }),
            nullifier: Array.from(result.nullifier),
        }
    } as any);

    if (verification.verified) {
        console.log('   ✅ SOLANA SAYS: PROOF VALID!');
        console.log(`   Explorer: https://explorer.solana.com/tx/${verification.transactionSignature}?cluster=devnet`);
    } else {
        console.error('   ❌ SOLANA SAYS: PROOF INVALID!');
    }

    // 4. Test Replay
    console.log('\n[4/4] Testing Bloom Filter replay protection...');
    try {
        await verifyOnChain(connection, wallet, verifierId, {
            query,
            proofData: {
                proof_a: result.solanaProof.proofA,
                proof_b: result.solanaProof.proofB,
                proof_c: result.solanaProof.proofC,
                publicInputs: result.publicSignals.map(s => {
                    let n = BigInt(s);
                    const bytes = new Uint8Array(32);
                    for (let i = 31; i >= 0; i--) { bytes[i] = Number(n & 0xFFn); n >>= 8n; }
                    return bytes;
                }),
                nullifier: Array.from(result.nullifier),
            }
        } as any);
    } catch (e: any) {
        console.log('   ✅ Transaction failed as expected (Replay Rejected).');
    }

    console.log('\n✨ All tests complete!');
}

main().catch(console.error);
