(ns otlwiki.main
  (:require [clojure.main :as main]
            [clojure.string :as str]
            [clojure.zip :as zip]
            [otlwiki.util :as util]
            [otlwiki.outline :as otl]))

; Data matching the outline saved on disk.
(def ^:dynamic *saved-outline* (atom nil))

; Current mutable in-memory outline data, starts out equal to *saved-outline*.
(def ^:dynamic *outline* (atom nil))

(defn paths
  "List sub-outline file paths for a root outline."
  [outline]
  (->> (:body outline) (map :head)))

(defn outline-at
  "Return sub-outline for a given outline file path."
  [outline path]
  (->> (:body outline) (filter #(= (:head %) path)) (first)))

(defn zipper
  "Turn an outline into a clojure.zip zipper."
  [outline]
  (zip/zipper vector? rest (fn [a c] (into [(first a)] c)) outline))

(defn changed-files
  "List paths of outline files that have been changed in memory."
  []
  (->> (paths @*outline*)
       (filter #(not= (outline-at @*outline* %) (outline-at @*saved-outline* %)))))

(defn save-outline!
  "Save changed outlines to disk."
  []
  (let
   [changed (changed-files)]
    (run!
      #(spit
         % (with-out-str (otl/print-body (outline-at @*outline* %)))) changed)
    (swap! *saved-outline* (fn [_] @*outline*))
    changed))

(defn -main [& args]
  (swap! *saved-outline* (fn [_] (otl/load (first args))))
  (swap! *outline* (fn [_] @*saved-outline*))
  (main/repl :init #(use 'otlwiki.main)))
