/**
 * Trusted setup ceremony script for the compound query circuit.
 * Run: node scripts/setup.js
 */
const snarkjs = require('snarkjs');
const fs = require('fs');
const path = require('path');

const BUILD_DIR = path.join(__dirname, '..', 'build');
const SETUP_DIR = path.join(__dirname, '..', 'trusted_setup');

async function main() {
    // Ensure directories exist
    if (!fs.existsSync(BUILD_DIR)) fs.mkdirSync(BUILD_DIR, { recursive: true });
    if (!fs.existsSync(SETUP_DIR)) fs.mkdirSync(SETUP_DIR, { recursive: true });

    const r1csPath = path.join(BUILD_DIR, 'compound_query.r1cs');
    if (!fs.existsSync(r1csPath)) {
        console.error('ERROR: Circuit not compiled. Run `npm run compile` first.');
        process.exit(1);
    }

    console.log('═══════════════════════════════════════');
    console.log('  SolID Protocol — Trusted Setup');
    console.log('═══════════════════════════════════════\n');

    // Phase 1: Powers of Tau
    console.log('[1/5] Generating Powers of Tau...');
    const ptauPath0 = path.join(SETUP_DIR, 'pot_0000.ptau');
    const ptauPath1 = path.join(SETUP_DIR, 'pot_0001.ptau');
    const ptauFinal = path.join(SETUP_DIR, 'pot_final.ptau');

    await snarkjs.powersOfTau.newAccumulator(
        snarkjs.bn128, 16, ptauPath0
    );

    console.log('[2/5] Contributing to ceremony...');
    await snarkjs.powersOfTau.contribute(
        ptauPath0, ptauPath1,
        'SolID first contribution', 'solid-entropy-' + Date.now()
    );

    console.log('[3/5] Preparing Phase 2...');
    await snarkjs.powersOfTau.preparePhase2(ptauPath1, ptauFinal);

    // Phase 2: Circuit-specific setup
    console.log('[4/5] Generating circuit-specific keys...');
    const zkey0 = path.join(BUILD_DIR, 'circuit_0000.zkey');
    const zkeyFinal = path.join(BUILD_DIR, 'circuit_final.zkey');

    await snarkjs.zKey.newZKey(r1csPath, ptauFinal, zkey0);
    await snarkjs.zKey.contribute(
        zkey0, zkeyFinal,
        'SolID circuit contribution', 'solid-circuit-entropy-' + Date.now()
    );

    // Export verification key
    console.log('[5/5] Exporting verification key...');
    const vkPath = path.join(BUILD_DIR, 'verification_key.json');
    const vk = await snarkjs.zKey.exportVerificationKey(zkeyFinal);
    fs.writeFileSync(vkPath, JSON.stringify(vk, null, 2));

    console.log('\n✅ Trusted setup complete!');
    console.log(`   Verification key: ${vkPath}`);
    console.log(`   Proving key:      ${zkeyFinal}`);

    // Print circuit info
    const r1csInfo = await snarkjs.r1cs.info(r1csPath);
    console.log(`\n   Constraints: ${r1csInfo.nConstraints}`);
}

main().catch(console.error);
