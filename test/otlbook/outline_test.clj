(ns otlbook.outline-test
  (:require [clojure.test :refer :all]
            [otlbook.util :as util]
            [otlbook.outline :refer [outline parse otl-vec]]))

(defn- sl [s] (util/sl {:tab 2} s))

; Test outline function parameter lists against normalized forms. Nil
; counterpart means the construction should fail.
(def invocation-suite
  [[] [],
   ["a"]  [["a" []]]
   ["a" []]  [["a" []]]
   ["a" "b"]  [["a" []] ["b" []]]
   ["a" ["b"]]  [["a" [["b" []]]]]
   [nil ["a"]]  [[nil [["a" []]]]]
   [nil]  [[nil []]]])

(deftest outline-invocation
  (run!
   (fn [[input expected]]
     (if expected
       (is (= (otl-vec (apply outline input)) expected))
       (is (thrown? Exception (apply outline input)))))
   (partition 2 invocation-suite)))

(deftest outline-properties
  (is (empty? (outline)))
  (is (not (empty? (outline "a"))))
  (is (= (count (outline)) 0))
  (is (= (count (outline "a")) 1))
  (is (contains? (outline "a: 1") :a))
  (is (= (:a (outline "a: 1")) "1"))
  (is (not (contains? (outline "a: 1") :b)))
  (is (= (assoc (outline) :a 1) (outline "a: 1")))
  (is (= (dissoc (outline "a: 1") :a) (outline)))
  (is (= (dissoc (outline "a: 1" "b: 2") :b) (outline "a: 1"))))

; Expected values are (outline) argument lists, not raw outline data as in
; invocation-suite.
(def parse-suite
  ["" []
   "," [nil]
   "a" ["a"]
   "\ta" [nil ["a"]]

   ; Escape the grouping syntax when you want a literal single comma
   ",," [","],
   ",,," [",,"],

   (sl "
        a
          b")
   ["a" ["b"]],

   (sl "
        a
          ,,")
   ["a" [","]],

   (sl "
        a
          b
        c
          d")
   ["a" ["b"] "c" ["d"]],

   (sl "
        a
          b
          c")
   ["a" ["b" "c"]],

   (sl "
        a
          b
            c")
   ["a" ["b" ["c"]]],

   "\ta\n\t\tb\n\tc"
   [nil ["a" ["b"] "c"]],

   "\t\ta\n\tb\n\tc"
   [nil [nil ["a"] "b" "c"]],

   "\t\t\ta\n\tb\n\tc"
   [nil [nil [nil ["a"]] "b" "c"]],

   (sl "
        a
            b
          c")
   ["a" [nil ["b"] "c"]],

   (sl "
        a
            b
            c
          d")
   ["a" [nil ["b" "c"] "d"]],

   (sl "
        a
          b
          ,
            c")
   ["a" ["b" nil ["c"]]],

   (sl "
        a
            b
          ,
            c")
   ["a" [nil ["b"] nil ["c"]]],

   (sl "
        a
          ,
          c")
   ["a" [nil [] "c"]],

   ; Empty lines don't break structure
   (sl "
        a
          b

          c")
   ["a" ["b" "" "c"]],

   ; Can't use sl here because all lines are indented
   "\ta\n\tb\n\tc"
   [nil ["a" "b" "c"]],

   (sl "
        a
          b
            c
          d
            e")
   ["a" ["b" ["c"] "d" ["e"]]],

   (sl "
        a
            b
            c
          ,
            d
            e")
   ["a" [nil ["b" "c"] nil ["d" "e"]]],

   (sl "
        a
              b
              c
          ,
            d
            e")
   ["a" [nil [nil ["b" "c"]] nil ["d" "e"]]],

   (sl "a
        b")
   ["a" "b"],

   (sl "
        a
          b
        c
          d")
   ["a" ["b"] "c" ["d"]],

   (sl "
        a
          b

        c
          d")
   ["a" ["b" ""] "c" ["d"]],

   "a\n\tb" ["a" ["b"]]])

(deftest outline-parse
  (run!
   (fn [[input expected]]
     (is (= (parse input) (apply outline expected))))
   (partition 2 parse-suite)))
