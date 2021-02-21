(ns otlbook.outline
  (:require [clojure.core.match :refer [match]]
            [clojure.string :as str]
            [instaparse.core :as insta]
            [otlbook.util :as util])
  (:refer-clojure :exclude [print load]))

(def attribute-re #"[a-z][a-z\-0-9]*")

(def attr-parser
  (insta/parser
   (str "<root> = attr <':'> value
         attr = " (pr-str attribute-re) "
         value = (<' '> #'.*' | #'')")))

(defn- err [& s] (throw (IllegalArgumentException. (apply str s))))

(defn header-only?
  "Return true if the outline has only header elements.

  Once the outline has any non-header elements,
  new header elements cannot be conj-ed to the end."
  [otl]
  ; We assume it's well-formed, so we only need to check whether the last line
  ; of a non-empty outline is a header line.
  (or (empty? (.contents otl))
      (-> (last (.contents otl)) first keyword?)))

(deftype Outline [contents]
  ; Just reimplement vec stuff for linear access
  clojure.lang.Seqable
  (seq [_] (.seq contents))

  clojure.lang.Reversible
  (rseq [_] (.rseq contents))

  clojure.lang.IPersistentCollection
  (count [_] (.count contents))
  (cons [self [head body]]
    ; Content validation
    (cond
      (and (keyword? head) (not (header-only? self)))
      (err "Adding attribute " head " after header")

      (and (keyword? head) (contains? self head))
      (err "Repeated attribute " head)

      (and (keyword? head) (not (re-matches attribute-re (name head))))
      (err "Invalid keyword format '" head "'")

      (and (string? head) (str/includes? head "\n"))
      (err "Headline contains multiple lines")

      (not (or (keyword? head) (string? head) (nil? head)))
      (err "Bad head type " head)

      (and (not (keyword? head)) (not (instance? Outline body)))
      (err "Body is not Outline")

      (and (not (keyword? head))
           (not (or (instance? Outline body) (nil? body))))
      (err "Bad body value " body))
    (Outline. (.cons contents [head body])))

  (empty [_] (.empty contents))
  (equiv [_ other]
    (and (isa? (class other) Outline)
         (.equiv contents (.contents other))))

  ; Outline shows up for sequential?, seen as viable child node
  clojure.lang.Sequential

  clojure.lang.Indexed
  (nth [_ i] (nth contents i))
  (nth [_ i default] (nth contents i default))

  java.lang.Iterable
  (iterator [_] (.iterator contents))

  clojure.lang.ILookup
  (valAt [self k] (.valAt self k nil))
  (valAt [_ k not-found]
    (or
     (some (fn [[k' v]] (when (= k' k) v)) contents)
     not-found))

  clojure.lang.IPersistentMap
  (assoc [self k v]
    (when-not (keyword? k) (err "Can only assoc keyword keys"))
    (if (get self k)
      ; Replace existing
      (Outline. (vec (map (fn [[k' v']] (if (= k' k) [k' v] [k' v'])) contents)))
      ; Insert new
      (Outline. (vec (concat [[k v]] contents)))))
  (assocEx [_ k v] (err "Not implemented"))  ; This shouldn't be needed?
  (containsKey [_ k] (boolean (some #(= (first %) k) contents)))
  (without [_ k] (Outline. (vec (filter #(not= (first %) k) contents)))))

; Debug print
(defmethod print-method Outline [otl w]
  (let
   [indent (fn [depth] (dotimes [_ depth] (clojure.core/print "›…")))

    print-otl
    (fn print-otl [depth otl]
      (run! (fn [[_ [h b]]]
              (cond
                (nil? h) (do (indent depth) (println "ε"))
                (keyword? h) (do (indent depth) (prn h b))
                h (do (indent depth) (prn h)))
              (when (and b (not (keyword? h)))
                (print-otl (inc depth) b))) (map vector (range) otl)))]
    ; TODO: Print limited amount of lines if outline is > ~20 lines
    (.write w (with-out-str (print-otl 0 otl)))))

(defn print [otl]
  (let
   [indent (fn [depth] (dotimes [_ depth] (clojure.core/print \tab)))

    print-otl
    (fn print-otl [depth otl]
      (run! (fn [[idx [h b]]]
              (cond
                (and (> idx 0) (nil? h)) (do (indent depth) (println \,))
                (keyword? h) (do (indent depth) (println (str (name h) ":") b))
          ; TODO: Escape literal comma line with extra comma
                h (do (indent depth) (println h)))
              (when (and b (not (keyword? h)))
                (print-otl (inc depth) b))) (map vector (range) otl)))]
    (print-otl 0 otl)))

(defn outline
  "Construct an outline from arguments.

  (outline)                                          ; Empty outline
  (outline \"Line 1\" \"Line 2\")                    ; Outline with two lines
  (outline :uri \"https://example.com\" \"Line 1\")  ; Outline with attribute
  (outline \"Line 1\" \"Line 2\" [\"Child 1\"])      ; Nested outline
  (outline nil [\"Subline\"])                        ; Double indentation

  Outlines must be well-formed:
  - Attributes are not allowed after a non-attribute line
  - Each attribute name must occur at most once
  - Line strings must not contain newlines"
  ([] (Outline. []))
  ([& args]
   (let
    [headline? #(or (string? %) (nil? %))
     body? sequential?

     normalize
     (fn [pairs args]
       (match [args]
         [([] :seq)] pairs

         ; Headline with body.
         [([(head :guard headline?) (body :guard body?) & rest] :seq)]
         (recur
          (conj pairs [head (if (seq body) (apply outline body) (outline))])
          rest)

         ; Headline followed by non-body, infer standalone line.
         [([(head :guard headline?) & rest] :seq)]
         (recur (conj pairs [head (outline)]) rest)

         [([(k :guard keyword?) v & rest] :seq)]
         (recur (conj pairs [k v]) rest)

         :else
         (err "Bad arguments")))]
     (into (outline) (normalize [] args)))))

(defn to-vec [otl]
  (vec (map (fn [[k v]] [k (if (instance? Outline v) (to-vec v) v)]) otl)))

(defn- indents-lines
  "Convert input text into [indent-depth deindented-line] pairs."
  [input]
  (if (= input "")
    []
    (let
     [process-line
      (fn [lines line]
        (conj lines
              (if (= (str/trim line) "")
                ; Snap empty lines to depth of previous line
                [(or (-> lines last first) 0) ""]
                (let [depth (count (take-while #{\tab} line))]
                  [depth (subs line depth)]))))]
      (reduce process-line [] (str/split-lines input)))))

(defn- to-attr
  "Try to parse an outline item into attributes.

  Return nil if parsing fails."
  [[head body]]
  (let [{attr :attr, v :value} (into {} (attr-parser head))
        v (not-empty v)]
    (cond
      ; Couldn't parse attr, regular item
      (not attr) nil
      ; Both inline and child value, not valid for an attr
      ; (this is a probable linter error)
      (and v (not-empty body)) nil
      ; Multiline value in body
      (not-empty body) [(keyword attr) body]
      ; Inline value
      :else [(keyword attr) v])))

; Keep making attrs while all have been attrs
(defn- parse-attrs
  "Parse header items into attributes.

  The first non-attribute item marks the end of the header.
  No items after it will be parsed as attributes."
  [otl]
  (let
   [parse-header (fn [acc [item & rest :as otl]]
                   (if-let [[attr _ :as item] (to-attr item)]
                     (if (not (contains? acc attr))  ; Stop on repeated attr
                       (recur (conj acc item) rest)
                       (into acc otl))
                     (into acc otl)))]
    (parse-header (outline) otl)))

(defn- parse-at
  "Parse an outline body assuming given depth."
  [otl depth input]
  (let
   [[[input-depth line] & rest] input
    double-indent? (and input-depth (> input-depth depth))
    head (cond
           double-indent? nil
           (= line ",") nil  ; Block separator
           (and (seq line) (every? #{\,} line)) (subs line 1)  ; Escaped comma
           :else line)]  ; Regular line
    (cond
      (not line) [(parse-attrs otl) rest]  ; At EOF, exit
      (< input-depth depth) [(parse-attrs otl) input]  ; Popping out of depth, exit
      :else (let
             [rest (if double-indent? input rest)
              [body rest] (parse-at (outline) (inc depth) rest)]
              (recur (conj otl [head body]) depth rest)))))

(defn parse
  "Parse text input into a sequence of outlines."
  [input]
  (->> (indents-lines (str/trimr input))
       (parse-at (outline) 0)
       (first)))

(defn load
  "Load single file or directory of .otl files into a single root outline."
  [path]
  (->> (util/crawl-files path)
       (filter #(str/ends-with? % ".otl"))
       (sort)
       (map #(vector % (parse (slurp %))))
       (into (outline))))
