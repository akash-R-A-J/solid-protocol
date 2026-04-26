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

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="${REPO_ROOT}/keys/localnet"
DST="${REPO_ROOT}/target/deploy"

if [[ ! -d "${SRC}" ]]; then
    echo "error: ${SRC} does not exist" >&2
    exit 1
fi

mkdir -p "${DST}"

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
