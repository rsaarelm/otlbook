(ns otlwiki.echo
  (:require
    [clojure.string :as str]
    [otlwiki.util :as util]
    [otlwiki.outline :as otl]))

(defn- outline-paths [path]
  (filter #(str/ends-with? % ".otl") (util/crawl-files path)))

(defn- path->outline [path]
  (into [path] (otl/parse (slurp path))))

(defn -main
  [& args]
  (otl/print
    (into [] (map path->outline (outline-paths (first args))))))
