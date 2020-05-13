let
  pkgs = import <nixpkgs> {};

  log_level = "info";
in
pkgs.mkShell {
  buildInputs = with pkgs; [
    rustc cargo rustfmt rls cargo-outdated clippy

    # Needed by cargo dependencies.
    cmake gcc zlib pkgconfig openssl

    # Webassembly tools
    wabt binaryen

    # Want this for the anki stuff
    anki
  ];

  shellHook = ''
    # Dynamic linking for Vulkan stuff for wgpu graphics
    export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:${
      with pkgs; pkgs.stdenv.lib.makeLibraryPath [ vulkan-loader ]
    }"

    # Run clippy without showing stuff I don't care about.
    alias clippy="cargo clippy -- -A clippy::cast_lossless"

    # FIXME: Current (2020-04-18) NixOS cargo-outdated is broken, you have to
    # do this stupid thing. Remove alias when it's fixed.
    alias cargo-outdated="cargo-outdated outdated"

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
