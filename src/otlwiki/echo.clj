(ns otlwiki.echo
  (:require [otlwiki.outline :as otl]))

(defn -main
  [& args]
  (let [outline (otl/parse (slurp (first args)))]
    (otl/print outline)))
