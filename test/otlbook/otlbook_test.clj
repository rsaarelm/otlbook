(ns otlbook.otlbook-test
  (:require [clojure.test :refer :all]
            [otlbook.otlbook :as otlbook :refer [wiki-word]]))

(deftest test-wikiword
  (is (= (wiki-word "") nil))
  (is (= (wiki-word "Not a wiki word") nil))
  (is (= (wiki-word "fakeWikiWord") nil))
  (is (= (wiki-word "WikiWord") "WikiWord"))
  (is (= (wiki-word "Wiki1234Word") "Wiki1234Word"))

  (is (= (wiki-word "FavoriteThing *") "FavoriteThing"))

  (is (= (wiki-word "[_] WorkInProgress") "WorkInProgress"))
  (is (= (wiki-word "[_] % WorkInProgress") "WorkInProgress"))
  (is (= (wiki-word "[_] 20% WorkInProgress") "WorkInProgress"))
  (is (= (wiki-word "[_] 60% WorkInProgress *") "WorkInProgress"))
  (is (= (wiki-word "[X] WorkInProgress") "WorkInProgress"))

  (is (= (wiki-word "¤/path/to/NotesFile.otl") "NotesFile")))
