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
  ([lines] (otl->expr lines 0))
  ([lines current-depth]

   (defn depth [line] (count (take-while #(= % \tab) line)))

   (defn sanitize [expr]
     "Remove prefix nil from short exprs"
     (let [len (count expr)]
      (cond
        ; Turn [nil] to [] and [nil a] to [a]
        ; Only keep prefix nil on list of at least two elements.
        (and (nil? (first expr)) (< len 3)) (rest expr)
        ; Turn [a] to a
        (and (vector? expr) (= len 1)) (first expr)
        :else expr)))

   (defn escape [line]
     "Escape commas that are used to denote a nil separator."
     (cond
       (= line ",") nil
       (and (not (empty? line)) (every? #(= % \,) line)) (subs line 1)
       :else line))

   (let [expr (escape (subs (first lines) current-depth))]
     (loop [expr [expr] input (rest lines)]
       (let [next-depth (depth (first input))
             line (first input)]
         (cond
           ; More than one indent level
           (> next-depth (inc current-depth))
           ;; FIXME Borked
           (let [[sub-expr remaining-input] (otl->expr input (+ current-depth 2))]
             (recur (conj expr (conj [nil] sub-expr)) remaining-input))
           (> next-depth current-depth)
           (let [[sub-expr remaining-input] (otl->expr input (inc current-depth))]
             (recur (conj expr sub-expr) remaining-input))
           ; Empty lines have depth 0, but are always assumed to go in current
           ; depth.
           (empty? input) [(sanitize expr) []]
           (empty? line) (recur (conj expr "") (rest input))
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
