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

    # Useful stuff
    html-tidy

    # Want this for the anki stuff
    anki
  ];
  shellHook = ''
    # For cargo-outdated
    export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:${
      with pkgs.xlibs; lib.makeLibraryPath [ openssl zlib ]
    }"
    export PATH=$PATH:~/.cargo/bin

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

    # Ensure AnkiConnect is installed

    # XXX: This is very ad hoc and probably fragile, would be nicer if we
    # could tell anki to install a plugin directly from the command line given
    # the plugin id...

    if [ ! -d ~/.local/share/Anki2/addons21/2055492159 ]; then
      echo "AnkiConnect plugin not found, installing..."
      mkdir -p ~/.local/share/Anki2/addons21
      pushd $(mktemp -d)
      git clone https://github.com/FooSoft/anki-connect/
      mv anki-connect/plugin ~/.local/share/Anki2/addons21/2055492159
      popd
    fi
  '';
}
