(ns otlwiki.util-test
  (:require [clojure.test :refer :all]
            [otlwiki.util :as util]))

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
