(ns otlbook.outline-test
  (:require [clojure.test :refer :all]
            [otlbook.util :as util]
            [otlbook.outline :refer [outline parse to-vec]]))

(defn- sl [s] (util/sl {:tab 2} s))

; Test outline function parameter lists against normalized forms. Nil
; counterpart means the construction should fail.
(def invocation-suite
  [[] [],
   ["a"]  [["a" []]]
   ["a" []]  [["a" []]]
   ["a" "b"]  [["a" []] ["b" []]]
   ["a" ["b"]]  [["a" [["b" []]]]]
   [:xyzzy 1]  [[:xyzzy 1]]
   [:xyzzy 1 "a"]  [[:xyzzy 1] ["a" []]]
   [:xyzzy 1 :plugh 2 "a"]  [[:xyzzy 1] [:plugh 2] ["a" []]]
   [nil ["a"]]  [[nil [["a" []]]]]
   ; Error: Attribute beyond header
   ["a" :xyzzy 1]  nil
   ; Error: Repeated attribute name
   [:xyzzy 1 :xyzzy 2]  nil
   ; Error: Malformed attribute
   [(keyword "foo bar") 1] nil
   [nil]  [[nil []]]])

(deftest outline-invocation
  (run!
   (fn [[input expected]]
     (if expected
       (is (= (to-vec (apply outline input)) expected))
       (is (thrown? Exception (apply outline input)))))
   (partition 2 invocation-suite)))

(deftest outline-properties
  (is (empty? (outline)))
  (is (not (empty? (outline "a"))))
  (is (= (count (outline)) 0))
  (is (= (count (outline "a")) 1))
  (is (= (count (outline :a 1)) 1))
  (is (contains? (outline :a 1) :a))
  (is (not (contains? (outline :a 1) :b)))
  (is (= (assoc (outline) :a 1) (outline :a 1)))
  (is (= (dissoc (outline :a 1) :a) (outline))))

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

   (sl "foo: 1
        bar: 2
        xyzzy")
   [:foo "1", :bar "2", "xyzzy"],

   ; Misformatting, attribute after header stays as text.
   (sl "foo: 1
        xyzzy
        bar: 2")
   [:foo "1", "xyzzy", "bar: 2"],

   ; Body value for attribute
   (sl "foo: 1
        bar:
          2
          3
        xyzzy")
   [:foo "1", :bar (outline "2" "3"), "xyzzy"],

   ; No combining inline and body value
   (sl "foo: 1
        bar: 1
          2
          3
        xyzzy")
   [:foo "1", "bar: 1" ["2" "3"] "xyzzy"],

   ; Repeated attribute, that's a nope.
   (sl "foo: 1
        foo: 2")
   [:foo "1", "foo: 2"],

   "a\n\tb" ["a" ["b"]]])

(deftest outline-parse
  (run!
   (fn [[input expected]]
     (is (= (parse input) (apply outline expected))))
   (partition 2 parse-suite)))
