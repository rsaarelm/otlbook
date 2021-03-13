; Otlbook-specific formatting of outlines
(ns otlbook.otlbook
  (:require [otlbook.outline :as outline]
            [clojure.string :as str]))

(defn wiki-word? [head]
  (when head
    (re-matches #"[A-Z][a-z]+([A-Z][a-z]+|[0-9]+)+" head)))

(defn spacify-wiki-word [word]
  (->> (str/split word #"(?=[A-Z])")          ; Foo123Bar to Foo123 Bar
       (map #(str/split % #"(?<!\d)(?=\d)"))  ; All Foo123 to Foo 123
       (flatten)
       (str/join " ")))
