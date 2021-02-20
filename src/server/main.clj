(ns server.main
  (:require [compojure.core :refer [defroutes GET]]
            [net.cgrand.enlive-html :as html]
            [otlbook.outline :as outline]
            [org.httpkit.server :as server]
            [taoensso.timbre :as timbre :refer [info]]
            [url-normalizer.core :as url]))

; TODO: Optionally use (System/getenv "OTL_PATH")
; to reconfigure the default ~/notes/wiki path
(info "Scanning outlines...")
(def ^:dynamic *outline*
  (atom (outline/load (str (System/getenv "HOME") "/notes/wiki"))))
(info "Scan done.")

(defn otl-seq []
  (tree-seq
   (constantly true)
   #(filter (fn [[k _]] (not (keyword? k))) (second %))
   [nil @*outline*]))

(info "Outline contains" (count (otl-seq)) "items")

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

(defn- find-uri [uri]
  (info "Looking for" uri)
  (first (filter
           ; TODO: url/normalize http uris
           (fn [[_ body]] (= (:uri body) uri))
           (otl-seq))))

; TODO: /uri/ method, look up things by uri, return 404 if thing isn't found

(defn save [uri]
  (let
   [url (try (url/normalize uri) (catch java.net.MalformedURLException uri))
    existing (find-uri (str url))
    ; TODO: Don't waste time scraping if url already exists in db
    ; TODO: Also don't try to scrape if it's not a HTML URI.
    page (html/html-resource url)
    page-title (or (scrape-title page) uri)
    ts (timestamp)]
    ; TODO: Compose outline structure instead of just building string
    (prn "Did we find it?" existing)
    (if existing
      (str "<pre>" existing "</pre>\n")
      (str "<pre>" page-title "\n"
           "\turi: " uri "\n"
           "\tadded: " ts "</pre>\n"))))

(defroutes app
  (GET "/" [] "Hello HTTP!")
  (GET "/save/:uri{.*}" [uri] (save uri)))

(defn -main [& args]
  (println "Starting server at http://localhost:8080")
  (server/run-server app {:port 8080}))
