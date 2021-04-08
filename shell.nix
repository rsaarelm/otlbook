let
  pkgs = import <nixpkgs> {};

  log_level = "info";
in
pkgs.mkShell {
  buildInputs = with pkgs; [
    rustup  # TODO: Remove rustup once NixOS rustc version updates
            # Currently included since rustc 1.45 won't build all dependencies
    rustc cargo rustfmt rust-analyzer cargo-outdated clippy

    # Needed by cargo dependencies.
    cmake gcc zlib pkgconfig openssl

    # Webassembly tools
    wabt binaryen

    # Want this for the anki stuff
    anki

    # Utils
    just
  ];

  RUST_BACKTRACE = "1";
  RUST_LOG = "parser=${log_level},scraper=${log_level},olt=${log_level}";
}
