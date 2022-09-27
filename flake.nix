{
  inputs = {
    naersk.url = "github:nmattia/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, naersk, ... }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        naersk-lib = pkgs.callPackage naersk { };
      in
      {
        defaultPackage = naersk-lib.buildPackage {
          src = ./.;
          pname = "otlbook";
          # Needed to build cargo dependencies.
          buildInputs = with pkgs; [ openssl pkg-config ];
        };

        apps.default = utils.lib.mkApp {
          drv = self.defaultPackage."${system}";
        };

        devShell = with pkgs; mkShell {
          buildInputs = [
            cargo
            rustc
            rustfmt
            pre-commit
            rust-analyzer
            rustPackages.clippy

            # Needed by cargo dependencies.
            openssl pkg-config

            # Utils
            tokei just

            # For anki SRS crate
            # anki
          ];
          RUST_SRC_PATH = rustPlatform.rustLibSrc;
          RUST_BACKTRACK = "1";
        };
      });
}
