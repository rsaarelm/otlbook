with import <nixpkgs> {};
mkShell {
  buildInputs = [
    clojure
    jdk11_headless
    clojure-lsp
    # FIXME: clj-kondo package broken as of 2020-09-06
    # clj-kondo
    just
  ];
}
