with import <nixpkgs> {};

stdenv.mkDerivation {
  name = "wasm-env";
  buildInputs = [
    rustup
    # Dev stuff cargo dependencies might need
    cmake gcc zlib pkgconfig openssl

    # Webassembly tools
    wabt binaryen

    # Server
    lighttpd
  ];
  shellHook = ''
    # Rust environment, use a local cargo dir
    export CARGO_HOME=$PWD/.cargo
    export PATH=$PATH:$CARGO_HOME/bin
    export RUST_BACKTRACE=1

    # Basic compiler
    rustup install nightly
    rustup default nightly
    rustup update

    # WASM setup
    rustup target add wasm32-unknown-emscripten
    NIX_ENFORCE_PURITY=0 cargo install wasm-pack

    # Development helpers
    rustup component add rls-preview rust-analysis rust-src
    rustup component add rls-preview rust-analysis rust-src --toolchain nightly
    rustup component add rustfmt-preview clippy-preview --toolchain nightly
    NIX_ENFORCE_PURITY=0 cargo install cargo-outdated

    # Dev commands
    alias run-webserver="lighttpd -D -f lighttpd.conf"
  '';
}
