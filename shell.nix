let
  pkgs = import <nixpkgs> {};

  log_level = "info";
in
pkgs.mkShell {
  buildInputs = with pkgs; [
    rustup  # TODO: Remove rustup once NixOS rustc version updates
            # Currently included since rustc 1.45 won't build all dependencies
    rustc cargo rustfmt rls cargo-outdated clippy

    # Needed by cargo dependencies.
    cmake gcc zlib pkgconfig openssl

    # Webassembly tools
    wabt binaryen

    # Want this for the anki stuff
    anki
  ];

  shellHook = ''
    # Run clippy without showing stuff I don't care about.
    alias clippy="cargo clippy -- -A clippy::cast_lossless"

    # Ensure AnkiConnect is installed

    # XXX: This is very ad hoc and probably fragile, would be nicer if we
    # could tell anki to install a plugin directly from the command line given
    # the plugin id...

    # FIXME: This doesn't work with lorri shell...

    # if [ ! -d ~/.local/share/Anki2/addons21/2055492159 ]; then
    #   echo "AnkiConnect plugin not found, installing..."
    #   mkdir -p ~/.local/share/Anki2/addons21
    #   pushd $(mktemp -d)
    #   git clone https://github.com/FooSoft/anki-connect/
    #   mv anki-connect/plugin ~/.local/share/Anki2/addons21/2055492159
    #   popd
    # fi
  '';

  RUST_BACKTRACE = "1";
  RUST_LOG = "parser=${log_level},scraper=${log_level},olt=${log_level}";
}
