#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# SolID Protocol — toolchain bootstrap
#
# Installs Solana CLI, Anchor CLI, circom, snarkjs, and wasm-pack into a
# project-local `.toolchain/` cache at the *exact* versions this repo builds
# against.  Called by `nix develop` and by CI.  Idempotent: re-running is a
# no-op once everything is in place.
#
# Pin versions HERE and only here.  If you bump one, update the matching
# section in docs/DEPLOYMENT_AND_TESTING.md in the same commit.
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

# ─── Pinned versions ────────────────────────────────────────────────────────
readonly SOLANA_VERSION="1.18.22"       # matches Anchor.toml [toolchain]
readonly ANCHOR_VERSION="0.30.1"        # matches Anchor.toml [toolchain]
readonly CIRCOM_VERSION="2.1.9"         # circom 2.1.x per prereqs
readonly SNARKJS_VERSION="0.7.5"        # 0.7 per prereqs
readonly WASM_PACK_VERSION="0.13.1"     # 0.12+ per prereqs

# ─── Cache location ─────────────────────────────────────────────────────────
ROOT_DIR="${SOLID_REPO_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
CACHE_DIR="${SOLID_TOOLCHAIN_DIR:-$ROOT_DIR/.toolchain}"
BIN_DIR="$CACHE_DIR/bin"
mkdir -p "$BIN_DIR"
export PATH="$BIN_DIR:$PATH"

log() { printf '\033[1;36m[bootstrap]\033[0m %s\n' "$*"; }

# ─── Platform detection (for Solana tarball) ────────────────────────────────
detect_platform() {
  local os arch
  os=$(uname -s)
  arch=$(uname -m)
  case "$os-$arch" in
    Linux-x86_64)   echo "x86_64-unknown-linux-gnu" ;;
    Linux-aarch64)  echo "aarch64-unknown-linux-gnu" ;;
    Darwin-x86_64)  echo "x86_64-apple-darwin" ;;
    Darwin-arm64)   echo "aarch64-apple-darwin" ;;
    *) echo "unsupported platform: $os-$arch" >&2; exit 1 ;;
  esac
}

# ─── Solana CLI ─────────────────────────────────────────────────────────────
install_solana() {
  if [ -x "$BIN_DIR/solana" ] && "$BIN_DIR/solana" --version 2>/dev/null | grep -q "$SOLANA_VERSION"; then
    return 0
  fi
  local platform url tarball
  platform=$(detect_platform)
  url="https://release.anza.xyz/v${SOLANA_VERSION}/solana-release-${platform}.tar.bz2"
  tarball="$CACHE_DIR/solana-${SOLANA_VERSION}-${platform}.tar.bz2"

  log "downloading Solana CLI ${SOLANA_VERSION} (${platform})"
  curl -fsSL --retry 3 "$url" -o "$tarball"
  mkdir -p "$CACHE_DIR/solana-${SOLANA_VERSION}"
  tar -xjf "$tarball" -C "$CACHE_DIR/solana-${SOLANA_VERSION}" --strip-components=1
  rm -f "$tarball"
  # Symlink each binary so $BIN_DIR/solana resolves correctly.
  for f in "$CACHE_DIR/solana-${SOLANA_VERSION}/bin"/*; do
    ln -sf "$f" "$BIN_DIR/$(basename "$f")"
  done
  log "Solana CLI installed: $($BIN_DIR/solana --version)"
}

# ─── Anchor CLI ─────────────────────────────────────────────────────────────
install_anchor() {
  if [ -x "$BIN_DIR/anchor" ] && "$BIN_DIR/anchor" --version 2>/dev/null | grep -q "$ANCHOR_VERSION"; then
    return 0
  fi
  log "installing anchor-cli ${ANCHOR_VERSION} (this takes a few minutes on first run)"
  CARGO_INSTALL_ROOT="$CACHE_DIR" cargo install \
    --git https://github.com/coral-xyz/anchor \
    --tag "v${ANCHOR_VERSION}" \
    anchor-cli \
    --locked
  log "Anchor CLI installed: $($BIN_DIR/anchor --version)"
}

# ─── circom ─────────────────────────────────────────────────────────────────
install_circom() {
  if [ -x "$BIN_DIR/circom" ] && "$BIN_DIR/circom" --version 2>/dev/null | grep -q "$CIRCOM_VERSION"; then
    return 0
  fi
  log "building circom ${CIRCOM_VERSION}"
  local tmp
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' RETURN
  git -C "$tmp" clone --quiet --depth 1 --branch "v${CIRCOM_VERSION}" https://github.com/iden3/circom
  (cd "$tmp/circom" && CARGO_INSTALL_ROOT="$CACHE_DIR" cargo install --path circom --locked --quiet)
  log "circom installed: $($BIN_DIR/circom --version)"
}

# ─── snarkjs (Node-based CLI) ───────────────────────────────────────────────
install_snarkjs() {
  if [ -x "$BIN_DIR/snarkjs" ] && "$BIN_DIR/snarkjs" --version 2>/dev/null | head -1 | grep -q "$SNARKJS_VERSION"; then
    return 0
  fi
  log "installing snarkjs@${SNARKJS_VERSION} and ts-node"
  mkdir -p "$CACHE_DIR/npm"
  npm install --silent --prefix "$CACHE_DIR/npm" \
    "snarkjs@${SNARKJS_VERSION}" \
    "ts-node@10" \
    "typescript@5"
  ln -sf "$CACHE_DIR/npm/node_modules/.bin/snarkjs" "$BIN_DIR/snarkjs"
  ln -sf "$CACHE_DIR/npm/node_modules/.bin/ts-node"  "$BIN_DIR/ts-node"
  ln -sf "$CACHE_DIR/npm/node_modules/.bin/tsc"      "$BIN_DIR/tsc"
  log "snarkjs installed: $($BIN_DIR/snarkjs --version | head -1)"
}

# ─── wasm-pack ──────────────────────────────────────────────────────────────
install_wasm_pack() {
  if [ -x "$BIN_DIR/wasm-pack" ] && "$BIN_DIR/wasm-pack" --version 2>/dev/null | grep -q "$WASM_PACK_VERSION"; then
    return 0
  fi
  log "installing wasm-pack ${WASM_PACK_VERSION}"
  CARGO_INSTALL_ROOT="$CACHE_DIR" cargo install --quiet --locked \
    --version "${WASM_PACK_VERSION}" wasm-pack
  log "wasm-pack installed: $($BIN_DIR/wasm-pack --version)"
}

# ─── Cargo.lock v3 invariant ────────────────────────────────────────────────
# Anchor 0.30.1 ships a pre-1.78 cargo in its platform-tools that cannot parse
# v4 lockfiles.  Refuse to continue if the repo's Cargo.lock has drifted.
check_lockfile_v3() {
  local lockfile="$ROOT_DIR/Cargo.lock"
  if [ ! -f "$lockfile" ]; then
    return 0
  fi
  if ! head -5 "$lockfile" | grep -qE '^version = 3($|[[:space:]])'; then
    log "ERROR: $lockfile is not format v3 — anchor build will fail."
    log "       Run from inside 'nix develop':"
    log "         rm Cargo.lock && cargo generate-lockfile"
    log "       Rust 1.79.0 (pinned by the flake) writes v3 by default."
    exit 1
  fi
}

# ─── main ───────────────────────────────────────────────────────────────────
main() {
  check_lockfile_v3
  install_solana
  install_anchor
  install_circom
  install_snarkjs
  install_wasm_pack
}

main "$@"
