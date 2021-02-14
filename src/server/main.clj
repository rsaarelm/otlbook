(ns server.main
  (:require [compojure.core :refer [defroutes GET]]
            [net.cgrand.enlive-html :as html]
            [org.httpkit.server :as server]
            [url-normalizer.core :as url]))

; Scraper stuff, goes to scrape module...
(defn scrape-title [res]
  (-> (html/select res [:head :title]) first :content first))

(defn timestamp
  "Create standard timestamp string"
  ([t] (-> t
           (.truncatedTo java.time.temporal.ChronoUnit/SECONDS)
           (.format java.time.format.DateTimeFormatter/ISO_OFFSET_DATE_TIME)))
  ([] (timestamp (java.time.ZonedDateTime/now))))

; TODO: Harden scrape architecture against wonky URLs and servers.
; Simple failure mode, try to scrape a pdf URL.
; Need to wrap the whole scrape attempt in try-catch, recover from error.
; Also needs a timeout,
; don't get caught up with pages that give endless input (eg https://robpike.io/).
; If we can get away with it, cut off web page after 1 sec and try to
; scrape title from whatever parts we got.

(defn save [uri]
  (let
   [url (url/normalize uri)    ; TODO: Handle non-http URIs
    page (html/html-resource url)
    page-title (or (scrape-title page) uri)
    ts (timestamp)]
    ; TODO: Compose outline structure instead of just building string
    (str "<pre>" page-title "\n"
         "\turi: " uri "\n"
         "\tadded: " ts "\n</pre>")))

(defroutes app
  (GET "/" [] "Hello HTTP!")
  (GET "/save/:uri{.*}" [uri] (save uri)))

(defn -main [& args]
  (println "Starting server at http://localhost:8080")
  (server/run-server app {:port 8080}))
