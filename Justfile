run:
    clj -m otlwiki.hello

test:
    clj -A:test:runner


    # FIXME: NixOS clj-kondo package build is broken. Using alternative runner
    # in the meantime. Replace with following when it's fixed:
    # clj-kondo --lint src
lint:
    clj -Sdeps '{:deps {clj-kondo {:mvn/version "RELEASE"}}}' -m clj-kondo.main --lint src
