with import <nixpkgs> {};
mkShell {
  buildInputs = [
    clojure
    jdk11_headless
    clojure-lsp
    just
  ];
}
