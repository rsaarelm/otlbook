run:
    clj -m otlwiki.hello

test:
    clj -A:test:runner

uberjar:
    clj -A:uberjar

lint:
    clj -Sdeps '{:deps {clj-kondo {:mvn/version "RELEASE"}}}' -m clj-kondo.main --lint src

fmt:
    clojure -Sdeps '{:deps {cljfmt {:mvn/version "RELEASE"}}}' -m cljfmt.main fix src/ test/ deps.edn

nrepl:
    clj -R:nREPL -m nrepl.cmdline
