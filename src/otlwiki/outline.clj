(ns otlwiki.outline
  (:require [clojure.string :as str]))

(defn- line
  "Consume input to newline or EOF.

  Newline is consumed but not included in parsed value."
  [s]
  (when s (str/split s #"\r?\n" 2)))

; Consume blank line

(defn- escape-separator-syntax
  "Parse lone ',' as group separator, escape ',,' into literal ','."
  [line]
  (cond
    (= line ",") :group
    (and (seq line) (every? #{\,} line)) (subs line 1)
    :else line))

(defn- indent-depth [line] (count (take-while #{\tab} line)))

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
   [[line rest] (line input)
    input-depth (indent-depth line)]
    (cond
      (not input) nil
      ; Match empty lines regardless of specified depth.
      (= (str/trim line) "") ["" rest]
      ; Input is above specified depth, fail to parse.
      (< input-depth depth) nil
      ; Input is below specified depth, emit group symbol.
      (< depth input-depth) [:group input]
      ; Input is at correct depth, format and return as headline.
      :else [(escape-separator-syntax (subs line depth)) rest])))

(declare parse-at-indent)

(defn- parse-children
  [depth expr input]
  (let [[child rest] (parse-at-indent (inc depth) input)]
    (if child
      (recur depth (conj expr child) rest)
      [(simplify expr) input])))

(defn- parse-at-indent
  [depth input]
  (let [[headline rest] (parse-headline depth input)]
    (when headline
      (parse-children depth [headline] rest))))

(defn parse
  [input]
  (loop [expr [], input input]
    (let [[outline rest] (parse-at-indent 0 input)]
      (if outline
        (recur (conj expr outline) rest)
        expr))))
