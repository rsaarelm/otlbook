(ns otlbook.otlbook-test
  (:require [clojure.test :refer :all]
            [otlbook.otlbook :as otlbook :refer [wiki-word]]))

(deftest test-wikiword
  (is (= (wiki-word "Not a wiki word") nil))
  (is (= (wiki-word "WikiWord") "WikiWord"))
  (is (= (wiki-word "Wiki1234Word") "Wiki1234Word"))

  (is (= (wiki-word "FavoriteThing *") "FavoriteThing"))
  (is (= (wiki-word "/path/to/NotesFile.otl") "NotesFile")))
