(ns otlbook.outline
  (:require [clojure.core.match :refer [match]]
            [clojure.string :as str]
            [otlbook.util :as util])
  (:refer-clojure :exclude [print load]))

(def ^:private attribute-name
  "Resolve attribute name as keyword from outline head string."
  (memoize
   (fn [head]
     (when head
       (keyword
        (second (re-find #"^([a-z][a-z\-0-9]*):( |$)" head)))))))

(defn- attribute-value [[head body]]
  ; Return inline value, or body if inline value is empty.
  ; XXX: It's an error to have both inline and body values.
  ; Current behavior is to ignore possible body values if inline value is found
  (or (second (re-find #"^[a-z][a-z\-0-9]*: (.*)" head)) body))

(declare outline)

(defn- attribute-item
  "Construct [head body] outline item from attribute keyword and value."
  [attr value]
  ; TODO: Handle values that go into body.
  [(str (name attr) ": " value) (outline)])

(defn- err [& s] (throw (IllegalArgumentException. (apply str s))))

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
      (and (string? head) (str/includes? head "\n"))
      (err "Headline contains multiple lines")

      (not (or (string? head) (nil? head)))
      (err "Bad head type " head)

      (not (instance? Outline body))
      (err "Bad body type"))
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
     (some
      (fn [[head _ :as item]]
        (when (= (attribute-name head) k)
          (attribute-value item)))
      contents)
     not-found))

  clojure.lang.IPersistentMap
  (assoc [self k v]
    (when-not (keyword? k) (err "Can only assoc keyword keys"))
    (if (get self k)
      ; Replace existing
      (Outline.
       (vec (map (fn [[head _ :as item]]
                   (if (= (attribute-name head) k) (attribute-item k v) item))
                 contents)))
      ; Insert new
      (Outline. (vec (concat [(attribute-item k v)] contents)))))
  (assocEx [_ k v] (err "Not implemented"))  ; This shouldn't be needed?
  (containsKey [_ k]
    (boolean (some #(= (attribute-name (first %)) k) contents)))
  (without [_ k]
    (Outline. (vec (filter #(not= (attribute-name (first %)) k) contents)))))

; Should this be {:depth :idx :line} instead, map instead of triple?
(defn lines
  "Generate lazy sequence of [depth nth-child head-string] from outline."
  [otl]
  (->>
   [-1 0 [nil otl]]
   ; Tree-seq [head body] items so iteration to body is possible.
   (tree-seq
    (constantly true)
    (fn [[depth _ [_ body]]]
      (map-indexed (fn [idx [h b]] [(inc depth) idx [h b]]) body)))
   ; Snip the dummy head item
   (rest)
   ; Remove bodies from sequence, only produce heads.
   (map (fn [[depth idx [head _]]] [depth idx head]))))

; Debug print
(defmethod print-method Outline [otl w]
  (let
   [max-display-lines 20
    print (fn [[depth _ line]]
            (with-out-str
              (dotimes [_ depth] (clojure.core/print "›…"))
              (if (nil? line)
                (println "ε")
                (prn line))))
    s (map print (lines otl))]
    (->>
     (concat
      (take max-display-lines s)
      (when (seq (drop max-display-lines s)) ["...\n"]))
     (run! #(.write w %)))))

(defn print [otl]
  (let
   [format-line (fn [idx line]
                  (cond
                    ; Empty head at start of outline, don't print anything
                    (and (not line) (= idx 0)) nil
                    ; Empty line otherwise, print separator comma
                    (not line) ","
                    ; Escape a literal comma
                    (every? #{\,} line) (str line ",")
                    :else line))
    print (fn [[depth idx line]]
            (if-let [line (format-line idx line)]
              (dotimes [_ depth] (clojure.core/print \tab))
              (println line)))]
    (run! print (lines otl))))

(defn outline
  "Construct an outline from arguments.

  (outline)                                          ; Empty outline
  (outline \"Line 1\" \"Line 2\")                    ; Outline with two lines
  (outline \"Line 1\" \"Line 2\" [\"Child 1\"])      ; Nested outline
  (outline nil [\"Subline\"])                        ; Double indentation

  Line strings must not contain newlines."
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

         :else
         (err "Bad arguments")))]
     (into (outline) (normalize [] args)))))

(defn otl-vec [otl]
  (vec (map (fn [[k v]] [k (if (instance? Outline v) (otl-vec v) v)]) otl)))

; XXX Is this useful or does it just mangle the data?
(defn otl-seq [otl]
  (tree-seq
   (constantly true)
   second
   [nil otl]))

(defn length [otl] (count (otl-seq otl)))

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
      (not line) [otl rest]  ; At EOF, exit
      (< input-depth depth) [otl input]  ; Popping out of depth, exit
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
