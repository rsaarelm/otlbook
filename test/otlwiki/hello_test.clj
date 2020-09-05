(ns otlwiki.hello-test
  (:require [clojure.test :refer :all]
            [otlwiki.hello :refer :all]))

(defn pair [expr otl]
  (is (= (otl->expr otl) expr))
  (is (= (expr->otl expr) otl)))

(deftest outline-parse-test
  (testing "Outline conversion"
    (pair [] "")
    (pair '["a"]
"a")
    (pair '["a" "b"]
"a
\tb")
    (pair '["a" "b" "c"]
"a
\tb
\tc")
    (pair '["a" ["b" "c"]]
"a
\tb
\t\tc")
    (pair '[["a" "b"] "c"]
",
\ta
\t\tb
\tc")
    (pair '[["a"] "b" "c"]
",
\t\ta
\tb
\tc")
    (pair '[[["a"]] "b" "c"]
",
\t\t\ta
\tb
\tc")
    (pair '["a" ["b"] "c"]
"a
\t\tb
\tc")
    (pair '["a" "b" ["c"]]
"a
\tb
\t,
\t\tc")
    (pair '["a" ["b"] ["c"]]
"a
\t\tb
\t,
\t\tc")
    (pair '["a" [] "c"]
"a
\t,
\tc")
    ; Empty lines don't break structure
    (pair '["a" "b" "" "c"]
"a
\tb

\tc")
    (pair '[nil "a" "b" "c"]
"\ta
\tb
\tc")
    (pair '["a" ["b" "c"] ["d" "e"]]
"a
\tb
\t\tc
\td
\t\te")
    (pair '["a" [nil "b" "c"] [nil "d" "e"]]
"a
\t\tb
\t\tc
\t,
\t\td
\t\te")
    (pair '["a" [[nil "b" "c"]] [nil "d" "e"]]
"a
\t\t\tb
\t\t\tc
\t,
\t\td
\t\te")
    ; Escape literal comma
    (pair '[","]
",,")
    (pair '[",,"]
",,,")))
