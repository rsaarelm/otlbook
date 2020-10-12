(ns otlwiki.outline
  (:require [clojure.string :as str]))

(defn- line
  "Consume input to newline or EOF.

  Newline is consumed but not included in parsed value."
  [^CharSequence s]
  (str/split s #"\r?\n" 2))

; Consume blank line

(defn- escape-separator-syntax
  "Parse lone ',' as group separator, escape ',,' into literal ','."
  [line]
  (cond
    (= line ",") :group
    (and (seq line) (every? #{\,} line)) (subs line 1)
    :else line))

; Consume chunk of indenty lines up to EOF or less than starting depth

(defn- parse-at-indent
  [level input]
  ; TODO
  )

; TODO Use :group instead of nil as the separator token

(defn parse
  "Extract one S-expr from a sequence of outline text lines."
  [lines]
  (let
   [depth (fn [line] (count (take-while #{\tab} line)))

    ; Prettify prefix nil and nesting hackery.
    sanitize
    (fn [expr]
      (let [len (count expr)]
        (cond
          ; Turn [nil] to [] and [nil a] to [a]
          ; Only keep prefix nil on list of at least two elements.
          (and (nil? (first expr)) (< len 3)) (subvec expr 1)
          ; turn [nil [..] ..] into [[..] ..]
          (and (nil? (first expr)) (vector? (second expr))) (subvec expr 1)
          ; Turn [a] to a
          (and (vector? expr) (= len 1)) (first expr)
          :else expr)))

    ; Escape commas that are used to denote a nil separator.
    escape
    (fn [line]
      (cond
        (= line ",") nil
        (and (seq line) (every? #{\,} line)) (subs line 1)
        :else line))

    ; Recursively parse at current parsing depth.
    parse-at
    (fn parse-at [lines current-depth]
      (let [first-line-depth (depth (first lines))]
        (loop [expr (if (= first-line-depth current-depth)
                      [(escape (subs (first lines) current-depth))]
                      [nil])
               input (if (= first-line-depth current-depth)
                       (rest lines)
                       lines)]
          (let [next-depth (depth (first input))
                line (first input)]
            (cond
              (> next-depth current-depth)
              (let [[sub-expr remaining-input] (parse-at input (inc current-depth))]
                (recur (conj expr sub-expr) remaining-input))
              ; Merge empty line to current depth.
              (= line "") (recur (conj expr "") (rest input))
              :else [(sanitize expr) input])))))]

    (parse-at lines 0)))
