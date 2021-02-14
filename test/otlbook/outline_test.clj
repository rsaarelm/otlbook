(ns otlbook.outline-test
  (:require [clojure.test :refer :all]
            [otlbook.util :as util]
            [otlbook.outline :as outline :refer [edn->otl]]))

(defn- sl [s] (util/sl {:tab 2} s))

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
   [["a" [nil "b" "c"] "d"]],

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
   [[nil "a" "b" "c"]],

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
   [["a" [nil "b" "c"] [nil "d" "e"]]],

   (sl "
        a
              b
              c
          ,
            d
            e")
   [["a" [[nil "b" "c"]] [nil "d" "e"]]],

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

(comment (map edn->otl [["a" ["b" ""]] ["c" "d"]]))
(comment (+ 1 2 3))

(defn- convert [edn] (into [] (map edn->otl edn)))

(deftest outline-parse
  (run!
   (fn [[input expected]]
     (is (= (outline/parse input) (convert expected))))
   (partition 2 outline-test-suite)))

(deftest outline-print
  (let
   [blacklist #{","}

    test
    (fn [[expected input]]
      (is (=
           (with-out-str (run! outline/print (convert input)))
           (str expected "\n"))))]
    (->>
     (partition 2 outline-test-suite)
     (filter (fn [[s _]] (not (blacklist s))))
     (run! test))))
