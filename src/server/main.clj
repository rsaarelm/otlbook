(ns server.main
  (:require [org.httpkit.server :as server]))

(defn app [req]
  {:status  200
   :headers {"Content-Type" "text/html"}
   :body    "hello HTTP!"})

(defn -main [& args]
  (println "Starting server at http://localhost:8080")
  (server/run-server app {:port 8080}))
