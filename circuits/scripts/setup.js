/**
 * Single-party trusted setup for `batch_credential_query.circom`.
 *
 * Run: `node scripts/setup.js`
 *
 * ────────────────────────────────────────────────────────────────────
 * TESTNET / DEVELOPMENT ONLY.
 *
 * A single-party ceremony means whoever runs this script holds the
 * toxic waste. Anyone holding the toxic waste can forge universal
 * proofs against the resulting VK. Tracked as SOLID-SEC-012; the
 * mainnet ceremony replaces this with a multi-party run with
 * attestation chain.
 * ────────────────────────────────────────────────────────────────────
 *
 * Output (matches `scripts/prove.ts` expectations exactly):
 *   circuits/build/batch_credential_query.r1cs               (from `npm run compile`)
 *   circuits/build/batch_credential_query_js/                (witness gen)
 *   circuits/build/batch_credential_query_final.zkey         (the prover key)
 *   circuits/build/verification_key.json                     (uploaded to on-chain verifier)
 *   circuits/trusted_setup/pot_final.ptau                    (optional; not consumed downstream)
 */
const snarkjs = require('snarkjs');
const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

const BUILD_DIR = path.join(__dirname, '..', 'build');
const SETUP_DIR = path.join(__dirname, '..', 'trusted_setup');

/// Circuit identity -- keep in one place. If you add another circuit,
/// parametrise this script rather than forking it.
const CIRCUIT_NAME = 'batch_credential_query';
// 2^17 = 131,072 constraints; batch_credential_query currently
// reports 86,616 non-linear constraints (snarkjs r1cs info, 2026-04-25),
// leaving ~34% headroom. If you add predicates / fields / creds and
// the constraint count crosses ~110K, bump to PTAU_POWER = 18 and
// re-run the trusted setup ceremony.
const PTAU_POWER = 17;

/// Canonical contribution entropy.
///
/// We deliberately use a 256-bit OS-level random string instead of
/// `'solid-entropy-' + Date.now()` (the old script's value). A wall-
/// clock timestamp is low-entropy and predictable; an attacker who
/// learned the approximate ceremony time could brute-force the toxic
/// waste. `crypto.randomBytes(32)` uses the platform CSPRNG.
///
/// This helper returns a hex string snarkjs accepts as the ceremony
/// "entropy" parameter.
function secureEntropy(label) {
    const buf = crypto.randomBytes(32);
    return `${label}|${buf.toString('hex')}`;
}

async function main() {
    if (!fs.existsSync(BUILD_DIR)) fs.mkdirSync(BUILD_DIR, { recursive: true });
    if (!fs.existsSync(SETUP_DIR)) fs.mkdirSync(SETUP_DIR, { recursive: true });

    const r1csPath = path.join(BUILD_DIR, `${CIRCUIT_NAME}.r1cs`);
    if (!fs.existsSync(r1csPath)) {
        console.error(
            `ERROR: ${r1csPath} not found.\n` +
            `Run \`cd circuits && npm run compile\` first.`,
        );
        process.exit(1);
    }

    console.log('═══════════════════════════════════════════════════════════');
    console.log(`  SolID Protocol -- Trusted Setup (TESTNET; SOLID-SEC-012)`);
    console.log(`  Circuit: ${CIRCUIT_NAME}`);
    console.log('═══════════════════════════════════════════════════════════\n');

    const { getCurveFromName } = require('ffjavascript');
    const curve = await getCurveFromName('bn128');

    const ptauPath0 = path.join(SETUP_DIR, 'pot_0000.ptau');
    const ptauPath1 = path.join(SETUP_DIR, 'pot_0001.ptau');
    const ptauFinal = path.join(SETUP_DIR, 'pot_final.ptau');

    console.log(`[1/5] Generating Powers of Tau (2^${PTAU_POWER})...`);
    await snarkjs.powersOfTau.newAccumulator(curve, PTAU_POWER, ptauPath0);

    console.log('[2/5] Contributing to ceremony...');
    await snarkjs.powersOfTau.contribute(
        ptauPath0, ptauPath1,
        'SolID first contribution',
        secureEntropy('ptau'),
    );

    console.log('[3/5] Preparing Phase 2...');
    await snarkjs.powersOfTau.preparePhase2(ptauPath1, ptauFinal);

    // ─── Phase 2: circuit-specific setup ────────────────────────────
    //
    // The output filename MUST match what `scripts/prove.ts` reads.
    // `prove.ts` imports a const; any change here has to land in both.

    console.log('[4/5] Generating circuit-specific keys...');
    const zkey0 = path.join(BUILD_DIR, `${CIRCUIT_NAME}_0000.zkey`);
    const zkeyFinal = path.join(BUILD_DIR, `${CIRCUIT_NAME}_final.zkey`);

    await snarkjs.zKey.newZKey(r1csPath, ptauFinal, zkey0);
    await snarkjs.zKey.contribute(
        zkey0, zkeyFinal,
        'SolID circuit contribution',
        secureEntropy('zkey'),
    );

    console.log('[5/5] Exporting verification key...');
    const vkPath = path.join(BUILD_DIR, 'verification_key.json');
    const vk = await snarkjs.zKey.exportVerificationKey(zkeyFinal);
    const vkJsonBytes = Buffer.from(JSON.stringify(vk, null, 2), 'utf8');
    fs.writeFileSync(vkPath, vkJsonBytes);

    // SOLID-SEC-041.  Pin a content hash for `verification_key.json`
    // right next to the artifact.  `scripts/initialize.ts` refuses to
    // upload any VK that does not match this hash (or the
    // `SOLID_VK_SHA256` env override), so a stale build directory,
    // mis-merged branch, or tampered artifact aborts before the
    // on-chain store.  Hash is over the exact bytes written above,
    // so snarkjs output ordering drift would surface as a hash change.
    const vkHash = crypto.createHash('sha256').update(vkJsonBytes).digest('hex');
    const vkHashPath = path.join(BUILD_DIR, 'verification_key.sha256');
    fs.writeFileSync(vkHashPath, `${vkHash}\n`);

    // Publish a hash of the final zkey in the console so CI + PR reviewers
    // can cross-check that the zkey on disk matches what the setup produced.
    const zkeyBytes = fs.readFileSync(zkeyFinal);
    const zkeyHash = crypto.createHash('sha256').update(zkeyBytes).digest('hex');

    console.log('\n' + '─'.repeat(60));
    console.log(`VK   sha256 : ${vkHash}`);
    console.log(`zkey sha256 : ${zkeyHash}`);
    console.log('─'.repeat(60));
    console.log(`VK          : ${vkPath}`);
    console.log(`VK sha256   : ${vkHashPath}`);
    console.log(`zkey        : ${zkeyFinal}`);
    console.log(`ptau        : ${ptauFinal}`);
    console.log(
        '\nTESTNET ONLY. Do not distribute as a production proving key.\n' +
        'Tracked: SOLID-SEC-012 (multi-party ceremony for mainnet).\n' +
        '\nRecord the VK sha256 in the release notes / ADR of any sanctioned\n' +
        'circuit revision.  Operators must set SOLID_VK_SHA256 to the\n' +
        'published hash (or leave it unset to accept the in-tree\n' +
        'verification_key.sha256 file) before running initialize.ts.',
    );

    // Cleanup intermediate files. The final zkey + VK + ptau stay; they are
    // the only artifacts downstream tooling consumes.
    if (fs.existsSync(ptauPath0)) fs.unlinkSync(ptauPath0);
    if (fs.existsSync(ptauPath1)) fs.unlinkSync(ptauPath1);
    if (fs.existsSync(zkey0)) fs.unlinkSync(zkey0);

    await curve.terminate();
}

main().catch((err) => {
    console.error('Setup failed:', err);
    process.exit(1);
});
