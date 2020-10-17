(ns otlwiki.outline
  (:require [clojure.string :as str]
            [otlwiki.util :as util])
  (:refer-clojure :exclude [print load]))

(defn- escape-separator-syntax
  "Parse lone ',' as group separator, escape ',,' into literal ','."
  [line]
  (cond
    (= line ",") :group
    (and (seq line) (every? #{\,} line)) (subs line 1)
    :else line))

(defn- simplify
  "Simplify some redundant patterns with :group symbol and vector wrapping."
  [expr]
  (let [len (count expr)]
    (cond
      ; Turn [:group] to [] and [:group a] to [a]
      ; Only keep prefix nil on list of at least two elements.
      (and (keyword? (first expr)) (< len 3)) (subvec expr 1)
      ; turn [:group [..] ..] into [[..] ..]
      (and (keyword? (first expr)) (vector? (second expr))) (subvec expr 1)
      ; Turn [a] to a
      (and (vector? expr) (= len 1)) (first expr)
      :else expr)))

; Consume chunk of indenty lines up to EOF or less than starting depth

(defn- parse-headline
  [depth input]
  (let
   [[[input-depth line] & lines] input]
    (cond
      (not input) nil
      ; Match empty lines regardless of specified depth.
      (= (str/trim line) "") ["" lines]
      ; Input is above specified depth, fail to parse.
      (< input-depth depth) nil
      ; Input is below specified depth, emit group symbol.
      (< depth input-depth) [:group input]
      ; Input is at correct depth, format and return as headline.
      :else [(escape-separator-syntax line) lines])))

(declare parse-at)

(defn- parse-children
  [depth expr input]
  (let [[child rest] (parse-at (inc depth) input)]
    (if child
      (recur depth (conj expr child) rest)
      [(simplify expr) input])))

(defn- parse-at
  [depth input]
  (let [[headline rest] (parse-headline depth input)]
    (when headline
      (parse-children depth [headline] rest))))

(defn parse
  [input]
  (let
   [lines
    (->>
     (str/split-lines input)
     (map (fn [line]
            (let [depth (count (take-while #{\tab} line))]
              [depth (subs line depth)]))))]
    (loop [expr [], input lines]
      (let [[outline rest] (parse-at 0 input)]
        (if outline
          (recur (conj expr outline) rest)
          expr)))))

(defn- print-line
  [depth first-line? content]
  (let
   [indent (fn [] (dotimes [_ depth] (clojure.core/print \tab)))]
    ; Don't print group separator on first line of new indetation level.
    ; Grouping will be expressed as subsequent indetation there.
    (when-not (and (keyword? content) first-line?)
      (cond
        (keyword? content) (do (indent) (println \,))
        (= (str/trim content) "") (println)
        ; Unescape content that's a literal comma or several.
        (every? #{\,} content) (do (indent) (println (str content \,)))
        :else (do (indent) (println content))))))

(defn- atom? [item] (or (string? item) (keyword? item)))

; Single-item list will be printed at depth.

(defn- print-at
  [depth first-line? input]
  (cond
    (not input) nil
    (atom? input) (print-line depth first-line? input)
    (= (count input) 0) (print-line depth false :group)
    (= (count input) 1)
    (do
      (when (not first-line?) (print-line depth false :group))
      (print-at (inc depth) true (first input)))
    :else
    (do
      (print-at
       (if (atom? (first input)) depth (inc depth))
       first-line?
       (first input))
      (print-at (inc depth) true (second input))
      (run! (partial print-at (inc depth) false) (rest (rest input))))))

(defn print
  [outline]
  (run! (partial print-at 0 true) outline))

(defn load
  "Load single file or directory of .otl files into one big outline."
  [path]
  (let
   [outline-paths (fn [path]
                    (filter #(str/ends-with? % ".otl") (util/crawl-files path)))]
    (->> (outline-paths path)
         (map #(into [%] (parse (slurp %)))))))
