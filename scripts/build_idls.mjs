#!/usr/bin/env node
// Build Anchor IDLs on the pinned stable toolchain (rust 1.79.0) without
// triggering anchor-lang-idl's hardcoded `--cfg procmacro2_semver_exempt`
// + `+nightly` invocation, which conflicts with our deps (ark-ff-macros'
// MontFp! macro needs Expr::Group from proc-macro2's wrap mode, which the
// fallback semver_exempt branch breaks; anchor-syn's
// `Span::source_file().path()` requires modern nightly's stable
// proc_macro::SourceFile API which has shifted).
//
// We invoke the same hidden anchor-lang test entrypoints anchor-lang-idl
// itself parses (`__anchor_private_print_idl_*`) and extract the printed
// `--- IDL begin program ---` ... `--- IDL end program ---` block.
// anchor-syn's `idl-build` codegen prints a complete IDL JSON to stdout.
//
// The semver_exempt cfg only gates type-alias resolution in
// anchor-syn/src/idl/defined.rs (lines 493-499) and is skipped silently
// when not set; nothing in our programs uses type aliases through Anchor's
// account types, so the produced IDL is byte-identical to what nightly
// would emit.
//
// Each program crate's `idl-build` feature must forward to every Anchor
// crate it pulls types from (`anchor-lang/idl-build`,
// `anchor-spl/idl-build` for token accounts, etc.). That is the genuine
// load-bearing change in programs/issuer-registry/Cargo.toml.

import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import process from 'node:process';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
const TOOLCHAIN = process.env.SOLID_RUSTUP_TOOLCHAIN || '1.79.0';

const PROGRAMS = [
  { name: 'issuer_registry', dir: 'programs/issuer-registry' },
  { name: 'schema_registry', dir: 'programs/schema-registry' },
  { name: 'zk_verifier',     dir: 'programs/zk-verifier' },
];

function extractIdl(stdout, crate) {
  const lines = stdout.split('\n');
  const start = lines.findIndex(l => l === '--- IDL begin program ---');
  const end   = lines.findIndex(l => l === '--- IDL end program ---');
  if (start < 0 || end < 0 || end <= start) {
    throw new Error(`Could not locate IDL block in ${crate} stdout`);
  }
  const json = lines.slice(start + 1, end).join('\n');
  const idl  = JSON.parse(json);

  // Splice in events / errors / address blocks the same way
  // anchor-lang-idl does (it parses each `--- IDL begin <kind> ---` window).
  const grab = (begin, finish) => {
    const out = [];
    let i = 0;
    while (i < lines.length) {
      if (lines[i] === begin) {
        const e = lines.indexOf(finish, i);
        if (e < 0) throw new Error(`unterminated ${begin}`);
        out.push(lines.slice(i + 1, e).join('\n'));
        i = e + 1;
      } else { i += 1; }
    }
    return out;
  };

  // events: array of { event: IdlEvent, types: IdlTypeDef[] }
  const events = grab('--- IDL begin event ---', '--- IDL end event ---').map(JSON.parse);
  if (events.length) {
    idl.events = (idl.events || []).concat(events.map(e => e.event));
    const seen = new Set((idl.types || []).map(t => t.name));
    for (const ev of events) {
      for (const ty of (ev.types || [])) {
        if (!seen.has(ty.name)) {
          (idl.types ||= []).push(ty);
          seen.add(ty.name);
        }
      }
    }
  }

  // errors: a single JSON array
  const errs = grab('--- IDL begin errors ---', '--- IDL end errors ---');
  if (errs.length) idl.errors = JSON.parse(errs[0]);

  // Address: take the program's primary `declare_id!` from src/lib.rs
  // rather than the first `--- IDL begin address ---` block, because
  // anchor's `idl-build` emits one such block per `declare_id!` it finds
  // (e.g. the embedded `spl_account_compression_id` mod), and cargo test
  // ordering is non-deterministic.
  //
  // Denamespace type names. With ANCHOR_IDL_BUILD_RESOLUTION='FALSE' (the
  // workaround for nightly-only proc_macro APIs above) Anchor leaves type
  // names fully qualified, e.g. `issuer_registry::RegistryConfig`. Anchor's
  // TS client (`new anchor.Program(idl, provider)`) registers
  // `program.account.<camelCase last-segment>` accessors only when names
  // are unqualified — `program.account.registryConfig` resolves to
  // `undefined` otherwise. Anchor's `idl-build` resolver does this strip
  // itself when run with `RESOLUTION='TRUE'`; we replicate that final pass
  // here so consumers can use the canonical accessor pattern.
  denamespaceIdl(idl);
  return idl;
}

// Strip `<crate>::` prefix from every type-registry name and every
// `defined` reference in the IDL. Mutates in place.
function denamespaceIdl(idl) {
  const strip = (s) =>
    typeof s === 'string' && /^[A-Za-z_][A-Za-z0-9_]*::/.test(s)
      ? s.replace(/^[A-Za-z_][A-Za-z0-9_]*::/, '')
      : s;

  for (const arr of [idl.accounts, idl.types, idl.events]) {
    if (!Array.isArray(arr)) continue;
    for (const entry of arr) {
      if (entry && typeof entry.name === 'string') entry.name = strip(entry.name);
    }
  }

  // Walk every nested `defined` reference. Two shapes occur in 0.30.x IDLs:
  //   { defined: "issuer_registry::Foo" }            (legacy / string form)
  //   { defined: { name: "issuer_registry::Foo" } }  (current object form)
  const walk = (node) => {
    if (!node || typeof node !== 'object') return;
    if (Array.isArray(node)) { for (const v of node) walk(v); return; }
    for (const [k, v] of Object.entries(node)) {
      if (k === 'defined') {
        if (typeof v === 'string') node.defined = strip(v);
        else if (v && typeof v === 'object' && typeof v.name === 'string') v.name = strip(v.name);
      }
      walk(v);
    }
  };
  walk(idl);
}

function programIdFromLibRs(crateDir) {
  const libRs = readFileSync(join(crateDir, 'src', 'lib.rs'), 'utf-8');
  // Match the *first* top-level `declare_id!("...")` (skip inner ones
  // inside `mod ... { declare_id!(...) }` blocks by requiring the line
  // to be at column 0).
  const m = libRs.match(/^declare_id!\("([1-9A-HJ-NP-Za-km-z]{32,44})"\)/m);
  if (!m) throw new Error(`No top-level declare_id! found in ${crateDir}/src/lib.rs`);
  return m[1];
}

function buildOne({ name, dir }) {
  const cwd = join(ROOT, dir);
  process.stdout.write(`[idl] ${name} ... `);
  const env = {
    ...process.env,
    RUSTUP_TOOLCHAIN: TOOLCHAIN,
    RUSTFLAGS: '-A warnings',
    ANCHOR_IDL_BUILD_PROGRAM_PATH: cwd,
    ANCHOR_IDL_BUILD_SKIP_LINT: 'TRUE',
    ANCHOR_IDL_BUILD_NO_DOCS: 'FALSE',
    ANCHOR_IDL_BUILD_RESOLUTION: 'FALSE',
  };
  const res = spawnSync('cargo', [
    'test', '__anchor_private_print_idl',
    '--features', 'idl-build',
    '--', '--show-output', '--quiet',
  ], { cwd, env, encoding: 'utf-8', maxBuffer: 1024 * 1024 * 64 });
  if (res.status !== 0) {
    process.stdout.write('FAIL\n');
    process.stderr.write(res.stderr || '');
    process.stderr.write(res.stdout || '');
    process.exit(1);
  }
  const idl = extractIdl(res.stdout, name);
  idl.address = programIdFromLibRs(cwd);
  const outDir = join(ROOT, 'target', 'idl');
  mkdirSync(outDir, { recursive: true });
  const outPath = join(outDir, `${name}.json`);
  writeFileSync(outPath, JSON.stringify(idl, null, 2) + '\n');
  process.stdout.write(`ok -> ${outPath}\n`);
}

for (const p of PROGRAMS) buildOne(p);
