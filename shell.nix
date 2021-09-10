let
  pkgs = import <nixpkgs> {};

  log_level = "info";
in
pkgs.mkShell {
  buildInputs = with pkgs; [
    rustc
    cargo rustfmt rust-analyzer cargo-outdated clippy

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
