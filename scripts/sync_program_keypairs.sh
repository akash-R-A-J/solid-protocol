#!/usr/bin/env bash
# sync_program_keypairs.sh
#
# Anchor reads program keypairs from `target/deploy/*-keypair.json` at
# `anchor deploy` time, but `target/` is gitignored, so a fresh checkout
# would otherwise let `anchor build` silently regenerate fresh keypairs
# and break every hard-coded canonical program ID. This script copies the
# tracked keypairs from `keys/localnet/` into `target/deploy/` if (and
# only if) the target copy is missing or differs. Idempotent.
#
# Run from the repo root before `anchor build` or `anchor deploy`.
# Safe to run unconditionally; CI invokes it as a pre-build step.
#
# Flags:
#   --reset-state    Also wipe the local E2E state cache
#                    ($TMPDIR/solid-e2e-<uid>/state.json or the
#                    XDG_RUNTIME_DIR equivalent).  Pass this whenever the
#                    validator was `solana-test-validator --reset`, so the
#                    next `npm run e2e` starts from a clean cache.  Without
#                    this, a stale `governanceMint` (or any cached pubkey)
#                    can force `initialize.ts` into the stale-state self-
#                    heal path documented at SOLID-SEC-079 / NF.

set -euo pipefail

RESET_STATE=0
for arg in "$@"; do
    case "$arg" in
        --reset-state) RESET_STATE=1 ;;
        --help|-h)
            sed -n '2,/^$/p' "${BASH_SOURCE[0]}" | sed 's/^# //;s/^#//'
            exit 0
            ;;
        *)
            echo "error: unknown flag: $arg" >&2
            echo "usage: $0 [--reset-state]" >&2
            exit 2
            ;;
    esac
done

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="${REPO_ROOT}/keys/localnet"
DST="${REPO_ROOT}/target/deploy"

if [[ ! -d "${SRC}" ]]; then
    echo "error: ${SRC} does not exist" >&2
    exit 1
fi

mkdir -p "${DST}"

if [[ "${RESET_STATE}" -eq 1 ]]; then
    UID_NUM="$(id -u)"
    CACHE_DIR_XDG="${XDG_RUNTIME_DIR:-}/solid-e2e"
    CACHE_DIR_TMP="${TMPDIR:-/tmp}solid-e2e-${UID_NUM}"
    # Normalise the macOS TMPDIR trailing slash + the explicit absolute case.
    CACHE_DIR_TMP_ALT="${TMPDIR:-/tmp}/solid-e2e-${UID_NUM}"
    CACHE_DIR_FALLBACK="/tmp/solid-e2e-${UID_NUM}"
    for dir in "${CACHE_DIR_XDG}" "${CACHE_DIR_TMP}" "${CACHE_DIR_TMP_ALT}" "${CACHE_DIR_FALLBACK}"; do
        if [[ -n "${dir}" && -f "${dir}/state.json" ]]; then
            rm -f "${dir}/state.json"
            echo "state cache cleared: ${dir}/state.json"
        fi
    done
fi

for program in zk_verifier issuer_registry schema_registry; do
    src_file="${SRC}/${program}-keypair.json"
    dst_file="${DST}/${program}-keypair.json"

    if [[ ! -f "${src_file}" ]]; then
        echo "error: missing ${src_file}" >&2
        exit 1
    fi

    if [[ -f "${dst_file}" ]] && cmp -s "${src_file}" "${dst_file}"; then
        echo "${program}: target/deploy already in sync"
        continue
    fi

    cp "${src_file}" "${dst_file}"
    chmod 600 "${dst_file}"
    echo "${program}: copied keys/localnet -> target/deploy"
done

echo "all program keypairs synced."
