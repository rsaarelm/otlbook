{
  inputs = {
    naersk.url = "github:nmattia/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    pre-commit-hooks.url = "github:cachix/pre-commit-hooks.nix";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, naersk, pre-commit-hooks, utils, ... }:
    utils.lib.eachDefaultSystem (system:
      let
        pname = "otlbook";
        pkgs = import nixpkgs { inherit system; };
        naersk-lib = pkgs.callPackage naersk { };
      in rec {
        checks = {
          pre-commit-check = pre-commit-hooks.lib.${system}.run {
            src = ./.;
            hooks = { nixpkgs-fmt.enable = true; };
          };
        };

        packages.default = naersk-lib.buildPackage {
          src = ./.;
          # Needed to build cargo dependencies.
          buildInputs = with pkgs; [ openssl pkg-config ];
        };

        apps.default = utils.lib.mkApp { drv = packages.default; };

        devShell = with pkgs;
          mkShell {
            buildInputs = [
              cargo
              rustc
              rustfmt
              pre-commit
              rust-analyzer
              rustPackages.clippy
              cargo-outdated

              # Needed by cargo dependencies.
              openssl
              pkg-config

              # Utils
              tokei
              just

              # For anki SRS crate
              # anki
            ];
            RUST_SRC_PATH = rustPlatform.rustLibSrc;
            RUST_BACKTRACE = "1";
          };
      });
}
