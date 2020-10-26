(ns otlwiki.outline
  (:require [clojure.string :as str]
            [otlwiki.util :as util])
  (:refer-clojure :exclude [print load]))

(defrecord Outline [head body])

; This function is mostly for convenient literal-writing in unit tests.
(defn edn->otl
  "Convert a concise nested vector structure into Outline."
  [edn]
  (let
   [edn  ; Normalize by adding implicit nil head if needed.
    (cond
      (string? edn) edn  ; Short circuit before cases below if naked string
      (vector? (first edn)) (into [nil] edn)  ; Implicit nil for [[a] ...]
      (= (count edn) 1) (into [nil] edn)      ; Implicit nil for [a]
      :else edn)]
    (if (string? edn)
      (Outline. edn [])
      (Outline. (first edn) (map edn->otl (rest edn))))))

(defn- escape-separator-syntax
  "Parse lone ',' as group separator, escape ',,' into literal ','."
  [line]
  (cond
    (= line ",") nil
    (and (seq line) (every? #{\,} line)) (subs line 1)
    :else line))

(defn- parse-head
  "Parse headline of an outline segment."
  [depth input]
  (let
   [[[input-depth line] & lines] input]
    (cond
      (not input) nil
      (= (str/trim line) "") ["" lines]  ; Empty line, always match
      (< input-depth depth) nil          ; Above depth, early exit
      (< depth input-depth) [nil input]  ; Below depth, nil head and continue
      :else [(escape-separator-syntax line) lines])))

(declare parse-at)

(defn- parse-body
  "Parse child outlines that form the body of an outline."
  [depth acc input]
  (let [[child rest] (parse-at (inc depth) input)]
    (if child
      (recur depth (conj acc child) rest)
      [acc input])))

(defn- parse-at
  [depth input]
  (let [[head rest :as parsed] (parse-head depth input)]
    (when parsed
      (let [[body rest] (parse-body depth [] rest)]
        [(Outline. head body) rest]))))

(defn parse
  "Parse text input into a sequence of outlines."
  [input]
  (let
   [lines
    (->>
     (str/split-lines input)
     (map (fn [line]
            (let [depth (count (take-while #{\tab} line))]
              [depth (subs line depth)]))))]
    (loop [outlines [], input lines]
      (let [[outline rest] (parse-at 0 input)]
        (if outline
          (recur (conj outlines outline) rest)
          outlines)))))

(defn- print-head
  [head depth]
  (let
   [indent (fn [] (dotimes [_ depth] (clojure.core/print \tab)))]
    (cond
      (nil? head) (do (indent) (clojure.core/print \,))
      (= (str/trim head) "") nil
      ; Unescape head that's a literal comma or several.
      (every? #{\,} head) (do (indent) (clojure.core/print (str head \,)))
      :else (do (indent) (clojure.core/print head)))))

(defn otl-seq
  "Produce a lazy sequence of [outline-node depth sibling-idx] pairs."
  [otl]
  (tree-seq
   (constantly true)
   (fn [[otl depth _]] (map-indexed #(vector %2 (inc depth) %1) (:body otl)))
   [otl 0 0]))

(defn- lines [otl]
  (->> (otl-seq otl)
       (filter (fn [[otl _ idx]] (not (and
                                       (nil? (:head otl))
                                       (not-empty (:body otl))
                                       (= idx 0)))))
       (map (fn [[otl depth _]]
              (with-out-str (print-head (:head otl) depth))))))

; Print whole outline
(defn print [otl] (run! println (lines otl)))

; Print sequence of unindented body outlines without head
(defn print-body [otl] (run! print (:body otl)))

; Debug print for REPL, only print limited number of lines
(defmethod print-method Outline [otl w]
  (let
   [max-display-lines 20
    debug-prn (fn [[otl depth _]]
                (with-out-str
                  (dotimes [_ depth] (clojure.core/print "›…"))
                  (if (nil? (:head otl))
                    (println "ε")
                    (prn (:head otl)))))
    s (map debug-prn (otl-seq otl))]
    (->>
     (concat
      (take max-display-lines s)
      (when (seq (drop max-display-lines s)) ["...\n"]))
     (run! #(.write w %)))))

(defn load
  "Load single file or directory of .otl files into a single root outline."
  [path]
  (let
   [outline-paths
    (fn [path] (filter #(str/ends-with? % ".otl") (util/crawl-files path)))]
    (->> (outline-paths path)
         (map #(Outline. % (parse (slurp %))))
         (Outline. nil))))

(defn paths
  "List sub-outline file paths for a root outline."
  [root-otl]
  (->> (:body root-otl) (map :head)))

(defn outline-at
  "Return sub-outline for a given root outline file path."
  [root-otl path]
  (->> (:body root-otl) (filter #(= (:head %) path)) (first)))
