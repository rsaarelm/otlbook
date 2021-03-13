(ns server.main
  (:require [compojure.core :refer [defroutes GET]]
            [net.cgrand.enlive-html :as html]
            [org.httpkit.server :as server]
            [otlbook.outline :as outline]
            [otlbook.otlbook :as otlbook]
            [otlbook.util :as util]
            [taoensso.timbre :as timbre :refer [info]]
            [url-normalizer.core :as url]))

(def max-inlined-wiki-page-length 10)

; TODO: Optionally use (System/getenv "OTL_PATH")
; to reconfigure the default ~/notes/wiki path
(info "Scanning outlines...")
(def ^:dynamic *outline*
  (atom (outline/load (str (System/getenv "HOME") "/notes/wiki"))))
(info "Scan done.")

(info "Outline contains" (outline/length @*outline*) "items")

; Scraper stuff, goes to scrape module...
(defn scrape-title [res]
  (-> (html/select res [:head :title]) first :content first))

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
          (outline/otl-seq @*outline*))))

; TODO: /uri/ method, look up things by uri, return 404 if thing isn't found

(defn save [uri]
  (let
   [url (try (url/normalize uri) (catch java.net.MalformedURLException uri))
    existing (find-uri (str url))
    ; TODO: Don't waste time scraping if url already exists in db
    ; TODO: Also don't try to scrape if it's not a HTML URI.
    page (html/html-resource url)
    page-title (or (scrape-title page) uri)
    ts (util/timestamp)]
    ; TODO: Compose outline structure instead of just building string
    (prn "Did we find it?" existing)
    (if existing
      (str "<pre>" existing "</pre>\n")
      (str "<pre>" page-title "\n"
           "\turi: " uri "\n"
           "\tadded: " ts "</pre>\n"))))

; TODO: Line parsing
(defn otl->html [otl]
  (when (seq otl)
    {:tag :ul, :content
     (map (fn [[head body]]
            (let
             [heading? (otlbook/wiki-word head)
              print-head (if heading?
                           (otlbook/spacify-wiki-word head)
                           head)]
              {:tag :li
               :content
               (if (and heading?
                        (> (outline/length body) max-inlined-wiki-page-length))
                 [{:tag :a, :attrs {:href (otlbook/slug-path head)}, :content print-head}]
                 [print-head, (otl->html body)])}))
          otl)}))

(html/deftemplate page-template "page.html"
  [head body]
  [:head :title] (html/content head)
  ; TODO: Construct body expression from body outline parameter
  [:body] (html/content
           {:tag :h1, :content (otlbook/spacify-wiki-word head)}
           (otl->html body)))

(defroutes app
  (GET "/" [] (page-template
               "Hello, otlbook"
               @*outline*))
  (GET "/save/:uri{.*}" [uri] (save uri))
  ; Freeform entry title resolution
  (GET "/e/:entry{.*}" [entry]
    ; TODO: 404 when not found
    (let
     [_ (info "Looking up" entry)
      [_ body] (outline/find entry otlbook/slug @*outline*)
      _ (if body (info "Entry found") (info "Not found"))]
      (page-template entry body)))
  (GET ["/:entry" :entry otlbook/wiki-word-re] [entry]
    ; TODO: 404 when not found
    (let
     [_ (info "Looking up" entry)
      [_ body] (outline/find entry otlbook/slug @*outline*)
      _ (if body (info "Entry found") (info "Not found"))]
      (page-template entry body))))

(defn -main [& args]
  (println "Starting server at http://localhost:8080")
  (server/run-server app {:port 8080}))
