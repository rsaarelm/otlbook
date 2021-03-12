(ns otlbook.util
  (:require [clojure.string :as str]
            [clojure.java.io :as io]))

(defn- to-tab-indented
  [tab-width line]
  (let
   [num-spaces (count (take-while #{\space} line))
    tabs (apply str (repeat (quot num-spaces tab-width) "\t"))]
    (str tabs (subs line (* (count tabs) tab-width)))))

(defn sl
  "Multiline string literal pretty-parsing.

    (sl \"these
         are
           the lines\")
    ; => \"these\\nare\\n  the lines\\n\"

    (sl \"
         are
           the lines\")
    ; => \"are\\n  the lines\\n\"

   Ignores the first line if it's empty. This lets you have the second line
   have a different indentation from the first line of the actual input.

   You can also specify that the result should be indented with tabs of
   specific width:

    (sl {:tabs 2} \"
         one
           two\")
    ; => \"one\\n\\ttwo\\n\"
  "

  ([opts s]
   ; XXX: Everything will explode unless your opts are exactly {:tab [number]}
   (->> (str/split-lines (sl s))
        (map (partial to-tab-indented (:tab opts)))
        (str/join "\n")))

  ([s]
   (let
    [blank-prefix
     (fn [s]
       (when s (subs s 0 (count (take-while #(Character/isWhitespace %) s)))))

     deindent
     (fn [line-seq]
       (let
        [prefix (blank-prefix (first line-seq))
         deindent-line
         (fn [s]
           (if (= (str/trim s) "") ""
               (do
                 (when-not (str/starts-with? s prefix)
                   (throw (Error.
                           "Line does not share first line's indentation")))
                 (subs s (count prefix)))))]
         (map deindent-line line-seq)))

     lines (str/split-lines s)]
     (->> (deindent (rest lines))
          (#(if (= (first lines) "") % (cons (first lines) %)))
          (str/join "\n")))))

(defn crawl-files
  "Return all files under path for dir path and path itself for file path."
  [path]
  (let [path (io/file path)]
    (if
     (.isFile path)
      [(.getAbsolutePath path)]
      (->> path (file-seq) (filter #(.isFile %)) (map #(.getAbsolutePath %))))))

(defn unfold [step seed]
  (when-let [[val new-seed] (step seed)]
    (cons val (lazy-seq (unfold step new-seed)))))

(defn timestamp
  "Create standard timestamp string"
  ([t] (-> t
           (.truncatedTo java.time.temporal.ChronoUnit/SECONDS)
           (.format java.time.format.DateTimeFormatter/ISO_OFFSET_DATE_TIME)))
  ([] (timestamp (java.time.ZonedDateTime/now))))
