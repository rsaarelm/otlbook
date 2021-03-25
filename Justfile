# Setting debug levels with env variable:
# TAOENSSO_TIMBRE_MIN_LEVEL_EDN=":debug" just server

run *ARGS:
    clj -m otlbook.main {{ARGS}}

server *ARGS:
    clj -m server.main {{ARGS}}

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
