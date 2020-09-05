(ns otlwiki.hello
  (:gen-class)
  (:require [org.httpkit.server :as server]
            [clojure.string :as str]))

(defn handler
  [req]
  {:status  200
   :headers {}
   :body    "Hello server!"})

(defn create-server
  [port]
  (server/run-server handler {:port port}))

(defn otl->expr
  "Extra one S-expr from a sequence of outline text lines."
  ([lines] (otl->expr lines 0))
  ([lines current-depth]
   (let [
    depth (fn [line] (count (take-while #{\tab} line)))

    sanitize (fn [expr]
     "Prettify prefix nil and nesting hackery."
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

    escape (fn [line]
     "Escape commas that are used to denote a nil separator."
     (cond
       (= line ",") nil
       (and (not (empty? line)) (every? #{\,} line)) (subs line 1)
       :else line))

    first-line-depth (depth (first lines))]

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
           (let [[sub-expr remaining-input] (otl->expr input (inc current-depth))]
             (recur (conj expr sub-expr) remaining-input))
           ; Merge empty line to current depth.
           (= line "") (recur (conj expr "") (rest input))
           :else [(sanitize expr) input]))))))

(defn otl->exprs [otl]
  ; TODO: Get more than first one
  (let [lines (str/split-lines otl)]
    (first (otl->expr lines))))

(defn expr->otl [expr] "") ;TODO

(defn -main
  [& args]
  (println "Starting server in http://localhost:8080/")
  (create-server 8080))
