; Otlbook-specific formatting of outlines
(ns otlbook.otlbook
  (:require [clojure.core.match :refer [match]]
            [clojure.string :as str]
            [instaparse.core :as insta]
            [otlbook.outline :as outline]
            [otlbook.util :as util]))

; Fold pages with wiki title that have more lines than this into links in the
; HTML view of an outline.
(def max-inlined-wiki-page-length 10)

(def wiki-word-re #"[A-Z][a-z]+([A-Z][a-z]+|[0-9]+)+")

(def line-parser
  (insta/parser
   "<line> = regular | wiki-path | block-line | table-line

    (* Wiki paths are not naturally occurring, mark with scarab *)
    wiki-path = <'¤'> path wiki-word <'.'> 'otl'
    <path> = <'/'> path-segment*
    <path-segment> = #'[^/\\s]+' <'/'>

    (* Various leading characters for a quoted block line.
       > Flowing paragraph
       ; Preformatted text
        Alternative preformatted text (one space after tab indents) *)
    block-line = (
        #'\\s' #'.*' |
        (';' | '>') (<' '> #'.*' | ε))

    table-line = table-row | table-separator
    <table-row> = [space] <'|'> table-cell+ [space]
    table-cell = <space> #'[^|]*' <space> <'|'>
    <table-separator> = [space] <'|'> separator-span+ [space]
    separator-span = <'-'>+(<'+'> | <'|'>)
    <space> = <#'\\s+' | ε>

    <regular> = [start-token space] (mid-token space)* [end-token space]

    <start-token> = checkbox | special-token | !checkbox word
    <end-token> = important | mid-token

    <special-token> = verbatim | image | internal-link | wiki-word | url
    <mid-token> = special-token | word

    checkbox = <'['> ('_' | 'X') <'] '> [#'\\d{0,3}' <'% '>]

    verbatim = <'`'> #'[^`]+' <'`'>
    image = <'!['> #'[^\\]]+' <']'>
    internal-link = <'|'> #'[^|\\s]([^|]*[^|\\s])?' <'|'>
    wiki-word = #'[A-Z][a-z]+([A-Z][a-z]+|[0-9]+)+'

    (* I'm not totally sure where the URL regex came from originally...
       Should it be simpler? *)
    url = #'(https?|ftp):\\/\\/[\\w-+&@#\\/%?=~_|()!:,.;\\[\\]]*[\\w-+&@#\\/%=~_|()]'

    <word> = !special-token #'\\S+'

    important = <'*'>
   "))

(defn- tag
  "Return syntax tag (if any) of line item"
  [item]
  (when (seqable? item) (first item)))

(defn- tagged [& args]
  (let [matcher (set args)]
    (fn [a] (matcher (tag a)))))

(defn- not-tagged [& args]
  (let [matcher (set args)]
    (fn [a] (not (matcher (tag a))))))

(defn- strip-decoration
  "Remove heading elements like progress mark
   and trailing elements like importance marker
   from parsed line."
  [parsed]
  (filter (not-tagged :checkbox :important) parsed))

(defn wiki-word
  "Convert WikiWord title headlines into just the base WikiWord."
  [head]
  (when (string? head)
    (match [(-> head line-parser strip-decoration vec)]
      [[[:wiki-word word]]] word
      [[[:wiki-path & elts]]] (->> (filter (tagged :wiki-word) elts)
                                   (first)
                                   (second))
      :else nil)))

(defn spacify-wiki-word [word]
  (when (and word (wiki-word word))
    (->> (wiki-word word)
         (#(str/split % #"(?=[A-Z])"))          ; Foo123Bar to Foo123 Bar
         (map #(str/split % #"(?<!\d)(?=\d)"))  ; All Foo123 to Foo 123
         (flatten)
         (str/join " "))))

(defn wiki-word-ord
  "Sort WikiWords so that bibilographical words get sorted by year first.

  A bibliography word is a WikiWord where the first numeric element
  is an integer between 1500 and 3000.
  This is assumed to represent the publication year.
  Non-bibilography words are sorted before bibliography words"
  [word]
  (let [nums (->> (str/split word #"\D+")
                  (filter seq)
                  (map #(Integer. %))
                  (vec))]
    (match [nums]
      [[(year :guard #(<= 1500 % 3000)) &rest]]
      [year word]
      :else
      [0 word])))

(defn slug
  "Convert headline into slug string.

  WikiWord titles become base WikiWords.
  Other titles get standard slugification."
  [title]
  (when title
    (if-let [wiki-word (wiki-word title)]
      wiki-word
      (util/slugify title))))

(defn slug-path
  "Convert headline into slug path component.

  WikiWords become /WikiWords, others become /e/others."
  [title]
  (when title
    (if-let [wiki-word (wiki-word title)]
      (str "/" wiki-word)
      (str "/e/" (util/slugify title)))))

(defn- clumping-key
  "Extract a value from a parsed line to see if it clumps with other lines.

  Headline-only outlines with equal clumping keys with clump to one item.
  Nothing else clumps.
  Clumping is used to merge table lines and quoted block lines."
  [parsed]
  (match [(vec parsed)]
    [[[:table-line & _] & _]] :table
    [[[:block-line prefix & _] & _]] [:block prefix]
    ; Specical case, fully empty lines clump with space prefix.
    [[]] [:block " "]
    :else nil))

(defn- block-clumper
  "Reduce function that clumps consecutive items together if possible."
  [acc [head body]]
  (let
   [[last-head last-body] (last acc)
    last-key (clumping-key last-head)
    clumps? (and
             last-key
             (not (seq last-body))
             (= last-key (clumping-key head)))
    ; Hack to make empty lines in space-indented blocks show up.
    head (if (and
              clumps?
              (seqable? head)
              (not (seq head))
              (= last-key [:block " "]))
           [[:block-line " " ""]]
           head)]
    (if clumps?
      ; Merge clumping heads.
      (conj (if (seq acc) (pop acc) []) [(concat last-head head) body])
      ; Fold normally if they don't clump.
      (conj acc [head body]))))

(defn- parse-headlines
  "Parse headlines of an outline and merge consecutive block lines."
  [otl]
  (->> otl
       (map (fn [[head body]] [(when head (line-parser head)) body]))
       (reduce block-clumper [])))

(defn- is-wiki-page-head
  "Return whether parsed headline is the header of a wiki page."
  [parsed-headline]
  (match [(-> parsed-headline strip-decoration vec)]
    [[[:wiki-path & _]]] true
    [[[:wiki-word _]]] true
    :else false))

(defn- line-item->html
  "Convert individual items in a parsed line to enlive HTML."
  [item & {:keys [inline-wiki-title]}]
  (match [item]
    [(s :guard string?)] s
    [[:checkbox "_" & percent]] "☐"   ; Should we show percent too?
    [[:checkbox "X" & percent]] "☑"
    [[:important]] "*"
    [([:wiki-word word] :guard (fn [_] inline-wiki-title))]
    {:tag :strong :content (spacify-wiki-word word)}
    [[(:or :wiki-word :internal-link) link]]
    {:tag :a
     :attrs {:href (slug-path link) :class "wikilink"}
     :content (if (wiki-word link) (spacify-wiki-word link) link)}
    [[:url link]] {:tag :a :attrs {:href link} :content link}
    [[:image path]] {:tag :img :attrs {:src (str "img/" path)}}
    [[:verbatim text]] {:tag :code :content text}
    :else {:tag :pre "TODO: Unhandled element" (str item)}))

(defn- table->html
  [parsed-table]
  (let [rows (map (fn [row]
                    (->> (rest row)
                         (map second)
                         (filter identity)
                         (map str/trim)))
                  parsed-table)

        after-header-separator?
        (->> rows (drop-while #(not (seq %))) second seq not)

        data (filter seq rows)

        ; If there are more than one data lines
        ; and the first data line is separated from the rest,
        ; emit table header tags for the first line.
        first-row-tag
        (if (and (> (count data) 1) after-header-separator?) :th :td)]
    {:tag :table :content
     (map-indexed
      (fn [idx row]
        (let [tag (if (= idx 0) first-row-tag :td)]
          {:tag :tr :content (map (fn [a] {:tag tag :content a}) row)}))
      data)}))

(defn- preformatted->html
  [parsed-preformatted]
  (let
   [lines (map #(if (seqable? %) (nth % 2 "") "") parsed-preformatted)]
    {:tag :pre :content (str/join "\n" lines)}))

(defn- paragraph->html
  [parsed-paragraphs]
  (let
   [lines (map
           #(if (seqable? %) (str/trim (nth % 2 "")) "")
           parsed-paragraphs)
    ; Group paragraphs
    paras (reduce
           (fn [acc elt]
             (if (seq elt)
               (conj (pop acc) (conj (last acc) elt))
               (conj acc [])))
           [[]]
           lines)
    ; Chunk into big lines
    paras (map #(str/join " " %) paras)]
    {:tag :div :content (map (fn [a] {:tag :p :content a}) paras)}))

(defn- parsed-headline->html
  "Convert parsed and clumped headline into enlive HTML."
  [parsed & {:keys [inline-wiki-title]}]
  (let [inline-wiki-title
        ; Only tell line item function to format things differently
        ; if it's an actual wiki title like.
        (and inline-wiki-title (is-wiki-page-head parsed))

        important?
        (= (last parsed) [:important])]
    (match [(vec parsed)]
      ; TODO Generate code for these guys
      ; TODO Generate header row in table
      [[[:table-line & _] & _]] [(table->html parsed)]
      [[[:block-line (:or ";" " ") & _] & _]] [(preformatted->html parsed)]
      [[[:block-line & _] & _]] [(paragraph->html parsed)]

      [[[:wiki-path & parts]]]
      [(line-item->html
        (-> parts reverse second)
        :inline-wiki-title inline-wiki-title)]
      :else
      (let [content
            (->> (interpose " " parsed)
                 (map #(line-item->html
                        %
                        :inline-wiki-title inline-wiki-title))
                 (vec))]
        (if important?
          [{:tag :span :attrs {:class "important"} :content content}]
          content)))))

(declare otl->html)

(defn fragment->html
  "Convert [parsed-headline body-otl] fragments to HTML.

  Recursively convert body to HTML unless it should be collapsed."
  [[parsed-head body]]
  (let
   [heading? (is-wiki-page-head parsed-head)
    long-body? (> (outline/length body) max-inlined-wiki-page-length)
    display-body? (or (not heading?) (not long-body?))]
    {:tag :li
     :content
     (conj
      (vec (parsed-headline->html
            parsed-head
            :inline-wiki-title (and heading? display-body?)))
      (when display-body? (otl->html body)))}))

(defn otl->html
  "Convert an outline into enlive HTML."
  [otl]
  (if (not (seq otl))
    nil
    (let [parsed (parse-headlines otl)]  ; Preprocess headlines
      {:tag :ul
       :content
       (-> (map fragment->html parsed) (vec))})))
