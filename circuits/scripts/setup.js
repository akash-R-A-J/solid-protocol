/**
 * Single-party trusted setup -- parameterised over circuit name.
 *
 * Run:
 *   node scripts/setup.js                                    # default: batch_credential_query
 *   node scripts/setup.js --circuit bjj_subgroup_proof       # SOLID-SEC-048 subgroup gate
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
 * Phase 1 (Powers of Tau) is circuit-agnostic and SHARED across all
 * SolID circuits.  The script reuses `circuits/trusted_setup/pot_final.ptau`
 * if present (post-2026-05-01: it is, from the prior batch-circuit
 * ceremony) -- if it is missing, it is generated fresh at PTAU_POWER.
 * This means SEC-048's subgroup ceremony and the main batch circuit's
 * ceremony share a Phase 1; only Phase 2 is per-circuit.
 *
 * Outputs (per circuit name):
 *   For circuit_name = 'batch_credential_query' (default):
 *     circuits/build/batch_credential_query_final.zkey
 *     circuits/build/verification_key.json
 *     circuits/build/verification_key.sha256
 *
 *   For circuit_name = 'bjj_subgroup_proof':
 *     circuits/build/bjj_subgroup_proof_final.zkey
 *     circuits/build/bjj_subgroup_verification_key.json
 *     circuits/build/bjj_subgroup_verification_key.sha256
 *
 *   Shared:
 *     circuits/trusted_setup/pot_final.ptau
 */
const snarkjs = require('snarkjs');
const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

const BUILD_DIR = path.join(__dirname, '..', 'build');
const SETUP_DIR = path.join(__dirname, '..', 'trusted_setup');

/// Recognised circuit names + their VK output filenames.  The default
/// circuit ('batch_credential_query') keeps the historical
/// `verification_key.json` filename so existing operator tooling and
/// `scripts/initialize.ts` continue to work without changes.  Other
/// circuits use a circuit-name-prefixed VK file to avoid collision.
const CIRCUITS = {
    batch_credential_query: {
        vk_filename: 'verification_key.json',
        vk_hash_filename: 'verification_key.sha256',
    },
    bjj_subgroup_proof: {
        vk_filename: 'bjj_subgroup_verification_key.json',
        vk_hash_filename: 'bjj_subgroup_verification_key.sha256',
    },
};

/// CLI parsing: `--circuit <name>`; default to batch_credential_query
/// for backward compatibility with operator runbooks.
function parseArgs(argv) {
    let circuit = 'batch_credential_query';
    for (let i = 2; i < argv.length; i++) {
        if (argv[i] === '--circuit' && i + 1 < argv.length) {
            circuit = argv[i + 1];
            i++;
        }
    }
    if (!CIRCUITS[circuit]) {
        console.error(
            `ERROR: unknown circuit '${circuit}'.  Known: ${Object.keys(CIRCUITS).join(', ')}.`,
        );
        process.exit(2);
    }
    return circuit;
}

const CIRCUIT_NAME = parseArgs(process.argv);
const CIRCUIT_META = CIRCUITS[CIRCUIT_NAME];

// 2^17 = 131,072 constraints.  Sized for batch_credential_query
// (currently ~86,616 non-linear constraints; ~34% headroom);
// bjj_subgroup_proof (currently ~2,200 non-linear constraints) fits
// trivially.  If batch constraints cross ~110K, bump PTAU_POWER to 18
// and re-run BOTH ceremonies -- the new PTAU file replaces the shared
// pot_final.ptau.
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

    if (fs.existsSync(ptauFinal)) {
        // Phase 1 is circuit-agnostic; reuse the shared PTAU.  This is
        // what makes the SEC-048 subgroup ceremony cheap to add: just a
        // fresh Phase 2 against the existing PTAU, no second Phase 1.
        console.log(`[1/5] Reusing existing PTAU: ${ptauFinal}`);
        console.log('[2/5] Skipped (PTAU already prepared).');
        console.log('[3/5] Skipped (Phase 2 prep already done).');
    } else {
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
    }

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
    const vkPath = path.join(BUILD_DIR, CIRCUIT_META.vk_filename);
    const vk = await snarkjs.zKey.exportVerificationKey(zkeyFinal);
    const vkJsonBytes = Buffer.from(JSON.stringify(vk, null, 2), 'utf8');
    fs.writeFileSync(vkPath, vkJsonBytes);

    // SOLID-SEC-041.  Pin a content hash for the VK file right next to
    // the artifact.  `scripts/initialize.ts` refuses to upload any VK
    // that does not match this hash (or the `SOLID_VK_SHA256` env
    // override), so a stale build directory, mis-merged branch, or
    // tampered artifact aborts before the on-chain store.  Hash is over
    // the exact bytes written above, so snarkjs output ordering drift
    // would surface as a hash change.
    const vkHash = crypto.createHash('sha256').update(vkJsonBytes).digest('hex');
    const vkHashPath = path.join(BUILD_DIR, CIRCUIT_META.vk_hash_filename);
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
