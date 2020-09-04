(ns otlwiki.hello
  (:gen-class)
  (:require [org.httpkit.server :as server]))

(defn handler
  [req]
  {:status  200
   :headers {}
   :body    "Hello server!"})

(defn create-server
  [port]
  (server/run-server handler {:port port}))

(defn otl->expr [otl] []) ;TODO

(defn expr->otl [expr] "") ;TODO

(defn -main
  [& args]
  (println "Starting server in http://localhost:8080/")
  (create-server 8080))
