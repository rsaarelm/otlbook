(ns otlwiki.hello-test
  (:require [clojure.test :refer :all]
            [otlwiki.util :as util]
            [otlwiki.outline :as outline]
            [otlwiki.hello :refer :all]))

(defn- sl [s] (util/sl {:tab 2} s))

(deftest sl-test
  (is (= (util/sl "") ""))
  (is (= (util/sl "a") "a"))
  (is (= (util/sl "
                   a") "a"))
  (is (= (util/sl "a
                   b") "a\nb"))
  (is (= (util/sl "a
                   b
                     c") "a\nb\n  c"))
  (is (= (util/sl {:tab 2} "
                            foo
                              bar
                                  baz")
         "foo\n\tbar\n\t\t\tbaz"))
  (is (= (util/sl {:tab 2} "
                            foo
                              bar
                                 baz")
         "foo\n\tbar\n\t\t baz")))

(def outline-test-suite
  ["" [""],

   "," [[]],

   "a" ["a"],

   "\ta" [["a"]],

   ; Escape the grouping syntax when you want a literal single comma
   ",," [","],

   ",,," [",,"],

   (sl "
        a
          b")
   [["a" "b"]],

   (sl "
        a
          ,,")
   [["a" ","]],

   (sl "
        a
          b
        c
          d")
   [["a" "b"] ["c" "d"]],

   (sl "
        a
          b
          c")
   [["a" "b" "c"]],

   (sl "
        a
          b
            c")
   [["a" ["b" "c"]]],

   "\ta\n\t\tb\n\tc"
   [[["a" "b"] "c"]],

   "\t\ta\n\tb\n\tc"
   [[["a"] "b" "c"]],

   "\t\t\ta\n\tb\n\tc"
   [[[["a"]] "b" "c"]],

   (sl "
        a
            b
          c")
   [["a" ["b"] "c"]],

   (sl "
        a
            b
            c
          d")
   [["a" [:group "b" "c"] "d"]],

   (sl "
        a
          b
          ,
            c")
   [["a" "b" ["c"]]],

   (sl "
        a
            b
          ,
            c")
   [["a" ["b"] ["c"]]],

   (sl "
        a
          ,
          c")
   [["a" [] "c"]],

   ; Empty lines don't break structure
   (sl "
        a
          b

          c")
   [["a" ["b" ""] "c"]],

   ; Can't use sl here because all lines are indented
   "\ta\n\tb\n\tc"
   [[:group "a" "b" "c"]],

   (sl "
        a
          b
            c
          d
            e")
   [["a" ["b" "c"] ["d" "e"]]],

   (sl "
        a
            b
            c
          ,
            d
            e")
   [["a" [:group "b" "c"] [:group "d" "e"]]],

   (sl "
        a
              b
              c
          ,
            d
            e")
   [["a" [[:group "b" "c"]] [:group "d" "e"]]],

   (sl "a
        b")
   ["a" "b"],

   (sl "
        a
          b
        c
          d")
   [["a" "b"] ["c" "d"]],

   (sl "
        a
          b

        c
          d")
   [["a" ["b" ""]] ["c" "d"]]])

(deftest outline-parse
  (run!
   (fn [[input expected]]
     (is (= (outline/parse input) expected)))
   (partition 2 outline-test-suite)))

(deftest outline-print
  (let
   [blacklist #{","}

    test
    (fn [[expected input]]
      (is (= (with-out-str (outline/print input)) (str expected "\n"))))]
    (->>
     (partition 2 outline-test-suite)
     (filter (fn [[s _]] (not (blacklist s))))
     (run! test))))
