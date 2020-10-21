(ns otlwiki.main
  (:require [clojure.main :as main]
            [clojure.string :as str]
            [otlwiki.util :as util]
            [otlwiki.outline :as otl]))

; Data matching the outline saved on disk.
(def ^:dynamic *saved-outline* (atom nil))

; Current mutable in-memory outline data, starts out equal to *saved-outline*.
(def ^:dynamic *outline* (atom nil))

(defn paths [outline]
  (->> (rest outline) (map first)))

(defn outline-at [outline path]
  (->> (rest outline) (filter #(= (first %) path)) (first) (rest)))

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
    (run! #(spit % (with-out-str (otl/print (outline-at @*outline* %)))) changed)
    (swap! *saved-outline* (fn [_] @*outline*))
    changed))

(defn- load-outlines [path]
  (->> (util/crawl-files path)
       (filter #(str/ends-with? % ".otl"))
       (map #(into [%] (otl/parse (slurp %))))
       (into [:group])))

(defn -main [& args]
  (swap! *saved-outline* (fn [_] (load-outlines (first args))))
  (swap! *outline* (fn [_] @*saved-outline*))
  (main/repl :init #(use 'otlwiki.main)))
