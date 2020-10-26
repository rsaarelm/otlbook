(ns otlwiki.main
  (:require [clojure.main :as main]
            [clojure.zip :as zip]
            [otlwiki.outline :as otl]))

; Data matching the outline saved on disk.
(def ^:dynamic *saved-outline* (atom nil))

; Current mutable in-memory outline data, starts out equal to *saved-outline*.
(def ^:dynamic *outline* (atom nil))

(defn zipper
  "Turn an outline into a clojure.zip zipper."
  [outline]
  (zip/zipper vector? rest (fn [a c] (into [(first a)] c)) outline))

(defn changed-files
  "List paths of outline files that have been changed in memory."
  []
  (->> (otl/paths @*outline*)
       (filter #(not=
                 (otl/outline-at @*outline* %)
                 (otl/outline-at @*saved-outline* %)))))

(defn save-outline!
  "Save changed outlines to disk."
  []
  (let
   [changed (changed-files)]
    (run!
     #(spit
       % (with-out-str
           (otl/print-body (otl/outline-at @*outline* %)))) changed)
    (swap! *saved-outline* (fn [_] @*outline*))
    changed))

(defn -main [& args]
  (swap! *saved-outline* (fn [_] (otl/load (first args))))
  (swap! *outline* (fn [_] @*saved-outline*))
  (main/repl :init #(use 'otlwiki.main)))
