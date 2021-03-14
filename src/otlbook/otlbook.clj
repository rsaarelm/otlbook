; Otlbook-specific formatting of outlines
(ns otlbook.otlbook
  (:require [clojure.core.match :refer [match]]
            [instaparse.core :as insta]
            [otlbook.outline :as outline]
            [otlbook.util :as util]
            [clojure.string :as str]))

(def wiki-word-re #"[A-Z][a-z]+([A-Z][a-z]+|[0-9]+)+")

(def wikiword-parser
  (insta/parser
   "root = <path> wikiword <'.otl'> | wikiword | wikiword <' *'>
    wikiword = #'[A-Z][a-z]+([A-Z][a-z]+|[0-9]+)+'
    path = <'/'> path-segment*
    path-segment = #'[^/]+' <'/'>"))

(defn wiki-word
  "Convert WikiWord title headlines into just the base WikiWord."
  [head]
  (when (string? head)
    (let
     [parse (wikiword-parser head)]
      (when-not (insta/failure? parse)
        (insta/transform {:root second} parse)))))

(defn wiki-word-ord
  "Sort WikiWords so that bibilographical words get sorted by year first.

  A bibliography word is a WikiWord where the first numeric element
  is an integer between 1500 and 3000.
  This is assumed to represent the publication year.
  Non-bibilography words are sorted before bibliography words"
  [word]
  (let [nums (->> (str/split word #"\D+")
                  (filter seq)
                  (map #(Integer. %))
                  (vec))]
    (match [nums]
      [[(year :guard #(<= 1500 % 3000)) &rest]]
      [year word]
      :else
      [0 word])))

(defn spacify-wiki-word [word]
  (when (and word (wiki-word word))
    (->> (wiki-word word)
         (#(str/split % #"(?=[A-Z])"))          ; Foo123Bar to Foo123 Bar
         (map #(str/split % #"(?<!\d)(?=\d)"))  ; All Foo123 to Foo 123
         (flatten)
         (str/join " "))))

(defn slug
  "Convert headline into slug string.

  WikiWord titles become base WikiWords.
  Other titles get standard slugification."
  [title]
  (when title
    (if-let [wiki-word (wiki-word title)]
      wiki-word
      (util/slugify title))))

(defn slug-path
  "Convert headline into slug path component.

  WikiWords become /WikiWords, others become /e/others."
  [title]
  (when title
    (if-let [wiki-word (wiki-word title)]
      (str "/" wiki-word)
      (str "/e/" (util/slugify title)))))
