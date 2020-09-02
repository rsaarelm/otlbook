with import <nixpkgs> {};
mkShell {
  buildInputs = [
    clojure
    just
  ];
}
