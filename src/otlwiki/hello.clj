(ns otlwiki.hello
  (:gen-class)
  (:require [org.httpkit.server :as server]
            [clojure.core.match :refer [match]]
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
     (match expr
            [nil] []
            [nil a] [a]
            [a] a
            :else expr))
   (defn escape [line]
     "Escape commas that are used to denote a nil separator."
     (cond
       (= line ",") nil
       (every? #(= % \,) line) (subs line 1)
       :else line))
   (let [expr (escape (subs (first lines) current-depth))]
     (loop [expr [expr] input (rest lines)]
       (let [next-depth (depth (first input))
             line (first input)]
         (cond
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
