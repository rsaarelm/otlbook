let
  pkgs = import <nixpkgs> {};

  # Overlay for nightly rust
  rust-overlay = import (builtins.fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz");
  nixpkgs = import <nixpkgs> { overlays = [ rust-overlay ]; };

  log_level = "info";
in
pkgs.mkShell {
  buildInputs = with pkgs; [
    nixpkgs.rust-bin.nightly.latest.default
    nixpkgs.rust-analyzer
    nixpkgs.cargo-outdated
    nixpkgs.cargo-udeps

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
  RUST_LOG = "anki=${log_level},base=${log_level},cache=${log_level},scraper=${log_level},tangle=${log_level},weave=${log_level},webserver=${log_level}";
}
