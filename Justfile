run *ARGS:
    clj -m otlwiki.main {{ARGS}}

test:
    clj -A:test:runner

repl:
    clj -A:dev

uberjar:
    clj -A:uberjar

lint:
    clj -Sdeps '{:deps {clj-kondo/clj-kondo {:mvn/version "RELEASE"}}}' -m clj-kondo.main --lint src

fmt:
    clojure -Sdeps '{:deps {cljfmt/cljfmt {:mvn/version "RELEASE"}}}' -m cljfmt.main fix src/ test/ dev/ deps.edn

nrepl:
    clj -R:nREPL -m nrepl.cmdline
