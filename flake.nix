{
  description = "SolID Protocol — reproducible dev environment";

  # Philosophy:
  #   * Nix pins the *hard-to-pin* parts (Rust toolchain, Node, pnpm, Python,
  #     system libs) so every contributor sees the exact same compiler and JS
  #     runtime.
  #   * The *Solana-specific* tools (Solana CLI, Anchor, circom, snarkjs,
  #     wasm-pack) bootstrap into a project-local `.toolchain/` cache on first
  #     `nix develop` entry.  Versions are declared ONCE in
  #     `scripts/bootstrap.sh` and verified by SHA-256.  No duplication
  #     between nix and the bootstrap script; the flake is the entrypoint,
  #     the script is the data.
  #
  #   This gives us a byte-identical Rust across macOS/Linux/CI while
  #   sidestepping the fact that the Anza-released Solana binaries are not
  #   distributed via nixpkgs.

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.05";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        # Rust 1.79.0: writes v3 Cargo.lock files by default.  Solana 1.18.22's
        # bundled cargo is pre-1.78 and cannot parse v4, so we MUST pin here.
        # See docs/DEPLOYMENT_AND_TESTING.md §0 for the full explanation.
        rustToolchain = pkgs.rust-bin.stable."1.79.0".default.override {
          extensions = [ "rust-src" "rustfmt" "clippy" ];
          targets = [ "wasm32-unknown-unknown" ];
        };

        commonTools = with pkgs; [
          rustToolchain
          nodejs_18
          nodePackages.pnpm
          python3
          jq
          git
          pkg-config
          openssl
          curl
          bzip2
          gnutar
          libiconv
          coreutils
          gnugrep
          gnused
        ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
          pkgs.darwin.apple_sdk.frameworks.Security
          pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
        ];

        banner = pkgs.writeShellScriptBin "solid-banner" ''
          echo "┌─ SolID Protocol devshell ──────────────────────────────"
          echo "│  Rust:      $(rustc --version)"
          echo "│  Cargo:     $(cargo --version)"
          echo "│  Node:      $(node --version)"
          echo "│  pnpm:      $(pnpm --version)"
          if command -v solana    >/dev/null 2>&1; then echo "│  Solana:    $(solana --version)";    fi
          if command -v anchor    >/dev/null 2>&1; then echo "│  Anchor:    $(anchor --version)";    fi
          if command -v circom    >/dev/null 2>&1; then echo "│  circom:    $(circom --version)";    fi
          if command -v snarkjs   >/dev/null 2>&1; then echo "│  snarkjs:   $(snarkjs --version | head -1)"; fi
          if command -v wasm-pack >/dev/null 2>&1; then echo "│  wasm-pack: $(wasm-pack --version)"; fi
          echo "└────────────────────────────────────────────────────────"
        '';

      in {
        devShells.default = pkgs.mkShell {
          packages = commonTools ++ [ banner ];

          shellHook = ''
            # Bootstrap Solana/Anchor/circom/snarkjs/wasm-pack into the
            # project-local `.toolchain/` cache. Idempotent.
            bash scripts/bootstrap.sh
            export SOLID_TOOLCHAIN_DIR="$PWD/.toolchain"
            export PATH="$SOLID_TOOLCHAIN_DIR/bin:$PATH"
            solid-banner || true
          '';
        };

        # Same shell with quiet bootstrap for CI.
        devShells.ci = pkgs.mkShell {
          packages = commonTools;
          shellHook = ''
            bash scripts/bootstrap.sh >/dev/null
            export SOLID_TOOLCHAIN_DIR="$PWD/.toolchain"
            export PATH="$SOLID_TOOLCHAIN_DIR/bin:$PATH"
          '';
        };
      });
}
