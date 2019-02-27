use lazy_static::lazy_static;
use regex::Regex;

/// Describe the state of the text block object currently being processed.
enum BlockSpec {
    Indented { depth: i32, prefix: String },
    Prefixed { depth: i32, prefix: String },
}

/// Receiver interface for parsed outline file text.
///
/// Many methods have default implementations that echo the standard outline syntax.
pub trait OutlineWriter: Sized {
    fn start_line(&mut self, depth: i32) {
        for _ in 0..depth {
            self.text("\t");
        }
    }

    /// Called at the end of a regular line.
    fn end_line(&mut self) {
        self.text("\n");
    }

    /// Text block delimited by indention.
    ///
    /// ```notrust
    /// [ ][ ][ ]Some text at depth - 1[prefix][syntax]
    /// [ ][ ][ ][ ]Block of indented text at depth. (All these lines in [text])
    /// [ ][ ][ ][ ]Multiple lines.
    /// [ ][ ][ ][ ]Dropping out of deeper indention ends
    /// [ ][ ][ ]...
    /// ```
    fn start_indent_block(&mut self, depth: i32, prefix: &str, syntax: &str) {
        self.text(prefix);
        self.text(syntax);
    }

    fn indent_block_line(&mut self, depth: i32, prefix: &str, text: &str) {
        self.start_line(depth);
        self.text(text);
        self.end_line();
    }

    /// Text block marked by a prefix.
    ///
    /// First line with syntax specifier hugging the prefix is optional.
    /// Main body lines must have a space separating prefix from text.
    ///
    /// ```notrust
    /// [ ][ ][ ][ ]([prefix][syntax])
    /// [ ][ ][ ][ ][prefix] Lines of body text.
    /// [ ][ ][ ][ ][prefix] Keep going until the prefix changes,
    /// [ ][ ][ ][ ][prefix] the indentation level changes,
    /// [ ][ ][ ][ ][prefix] or there's a new syntax specifier line where
    /// [ ][ ][ ][ ][prefix] there is no space between prefix and text.
    /// ```
    fn start_prefix_block_with_syntax(&mut self, depth: i32, prefix: &str, syntax: &str) {
        self.start_line(depth);
        self.text(prefix);
        self.text(syntax);
        self.end_line();
    }

    /// Called when there's no syntax specified to denote new block starting.
    fn start_prefix_block(&mut self, depth: i32, prefix: &str) {}

    fn prefix_block_line(&mut self, depth: i32, prefix: &str, text: &str) {
        self.start_line(depth);
        self.text(prefix);
        self.text(" ");
        self.text(text);
        self.end_line();
    }

    /// Write some regular text in the current element.
    fn text(&mut self, text: &str);

    /// Signal a paragraph break in a non-preformatted text block.
    fn paragraph_break(&mut self) {}

    /// Handler for verbatim text.
    fn verbatim_text(&mut self, verbatim: &str) {
        self.text("`");
        self.text(verbatim);
        self.text("`");
    }

    /// Handler for links to WikiWords.
    fn wiki_word_link(&mut self, wiki_word: &str) {
        self.text(wiki_word);
    }

    /// Wiki word alone on a line, site for the word.
    fn wiki_word_heading(&mut self, wiki_word: &str) {
        self.text(wiki_word);
    }

    /// Handler for links to wikiword aliases.
    fn alias_link(&mut self, wiki_alias: &str) {
        self.text("[");
        self.text(wiki_alias);
        self.text("]");
    }

    /// Handler for web URLs.
    fn url(&mut self, url: &str) {
        self.text(url);
    }

    /// Handler for local inline images, ![./image_path]
    fn inline_image(&mut self, image_path: &str) {
        self.text("![");
        self.text(image_path);
        self.text("]");
    }

    /// Handler for local file links, [./file_path]
    fn local_link(&mut self, file_path: &str) {
        self.text("[");
        self.text(file_path);
        self.text("]");
    }

    /// Handler for alias definitions, (Alias)
    fn alias_definition(&mut self, alias: &str) {
        self.text("(");
        self.text(alias);
        self.text(")");
    }

    /// Handler for tags, @tag
    fn tag_definition(&mut self, tag: &str) {
        self.text("@");
        self.text(tag);
    }

    /// Handler for WikiWord titles
    fn wiki_title(&mut self, title: &str) {
        self.text(title);
    }

    fn parse(&mut self, input: &str) {
        use BlockSpec::*;

        /// Tag fragments inside a line for markup.
        fn parse_fragment<'a, W>(writer: &mut W, line: &'a str) -> &'a str
        where
            W: OutlineWriter + Sized,
        {
            lazy_static! {
                static ref VERBATIM_TEXT: Regex = Regex::new(r"`([^`]+)`").unwrap();
                static ref URL: Regex = Regex::new(
                    r"\b(https?|ftp)://[-A-Za-z0-9+&@#/%?=~_|!:,.;()]*[-A-Za-z0-9+&@#/%=~_|()]"
                )
                .unwrap();
                static ref INLINE_IMAGE: Regex = Regex::new(r"!\[(.*?)\]").unwrap();
                static ref LOCAL_LINK: Regex = Regex::new(r"\[(\.\.?/.*?)\]").unwrap();
                static ref ALIAS_LINK: Regex = Regex::new(r"\[([A-Z][A-Za-z0-9.-_]?)\]").unwrap();
                static ref WIKI_WORD: Regex = Regex::new(r"\b([A-Z][a-z0-9]+){2,}\b").unwrap();
            }

            // Slice up the line based on regular expressions for the elements. The further up on
            // the if-else chain an element is, the higher precedence it has.
            if let Some(c) = VERBATIM_TEXT.captures(line) {
                let (m, item) = (c.get(0).unwrap(), c.get(1).unwrap());
                parse_fragment(writer, &line[..m.start()]);
                writer.verbatim_text(&line[item.start()..item.end()]);
                return &line[m.end()..];
            } else if let Some(m) = URL.find(line) {
                parse_fragment(writer, &line[..m.start()]);
                writer.url(&line[m.start()..m.end()]);
                return &line[m.end()..];
            } else if let Some(c) = INLINE_IMAGE.captures(line) {
                let (m, item) = (c.get(0).unwrap(), c.get(1).unwrap());
                parse_fragment(writer, &line[..m.start()]);
                writer.inline_image(&line[item.start()..item.end()]);
                return &line[m.end()..];
            } else if let Some(c) = LOCAL_LINK.captures(line) {
                let (m, item) = (c.get(0).unwrap(), c.get(1).unwrap());
                parse_fragment(writer, &line[..m.start()]);
                writer.local_link(&line[item.start()..item.end()]);
                return &line[m.end()..];
            } else if let Some(c) = ALIAS_LINK.captures(line) {
                let (m, item) = (c.get(0).unwrap(), c.get(1).unwrap());
                parse_fragment(writer, &line[..m.start()]);
                writer.alias_link(&line[item.start()..item.end()]);
                return &line[m.end()..];
            } else if let Some(m) = WIKI_WORD.find(line) {
                parse_fragment(writer, &line[..m.start()]);
                writer.wiki_word_link(&line[m.start()..m.end()]);
                return &line[m.end()..];
            } else {
                writer.text(line);
                return &"";
            }
        }

        fn parse_tag_alias_line<'a, W>(writer: &mut W, line: &'a str) -> &'a str
        where
            W: OutlineWriter + Sized,
        {
            lazy_static! {
                static ref ALIAS_DEF: Regex = Regex::new(r"\(([A-Z][A-Za-z0-9.-_]*)\)").unwrap();
                static ref TAG: Regex = Regex::new(r"@([a-zA-Z0-9-]+)").unwrap();
            }
            if let Some(c) = ALIAS_DEF.captures(line) {
                let (m, item) = (c.get(0).unwrap(), c.get(1).unwrap());
                parse_fragment(writer, &line[..m.start()]);
                writer.alias_definition(&line[item.start()..item.end()]);
                return &line[m.end()..];
            } else if let Some(c) = TAG.captures(line) {
                let (m, item) = (c.get(0).unwrap(), c.get(1).unwrap());
                parse_fragment(writer, &line[..m.start()]);
                writer.tag_definition(&line[item.start()..item.end()]);
                return &line[m.end()..];
            } else {
                writer.text(line);
                return &"";
            }
        }

        lazy_static! {
            static ref WIKI_TITLE: Regex = Regex::new(r"^([A-Z][a-z0-9]+){2,}$").unwrap();
            static ref ALIASES_LINE: Regex =
                Regex::new(r"^(\(([A-Z][A-Za-z0-9.-_]*)\)\s*)+$").unwrap();
            static ref TAGS_LINE: Regex = Regex::new(r"^((@[a-zA-Z0-9-]+)\s*)+$").unwrap();

            static ref EMPTY_LINE: Regex = Regex::new(r"^\s*$").unwrap();

            static ref INDENT_BLOCK: Regex = Regex::new(r"(>|;)$|('''|```)(.*)$").unwrap();
            // NB: Space-prefix blocks can't have a syntax.
            static ref PREFIX_BLOCK_SYNTAX: Regex = Regex::new(r"^(>|<|;|:)(\S.*)$").unwrap();
            // NB: Space-prefix block is indicated by absence of the prefix match
            static ref PREFIX_BLOCK: Regex = Regex::new(r"^(>|<|;|:)? (.*)$").unwrap();
        }

        // Set to Some(depth) when entering an indented block at that depth.
        // A line with lower than block_indent depth will then exit the block.
        let mut block_indent = None;

        // Header is the lines immediately after a wiki heading. It consists of alias declaration
        // and tag declaration lines only, any other type of line ends the header. Alias or tag
        // declarations are ignored outside of a header.
        let mut in_header = true;
        for mut line in input.lines() {
            let depth = line.chars().take_while(|&c| c == '\t').count() as i32;

            let is_empty = EMPTY_LINE.is_match(line);

            // Continuing an indent block?
            if let Some(Indented { depth: d, prefix }) = &block_indent {
                if depth < *d && !is_empty {
                    // Out of block, zero indent memory and return to normal logic.
                    block_indent = None;
                } else {
                    if is_empty && (prefix == ">" || prefix == "'''") {
                        self.paragraph_break();
                    }
                    let cut_line = if is_empty { "" } else { &line[*d as usize..] };
                    // Within block, push text as-is and continue
                    self.indent_block_line(*d, &prefix, cut_line);
                    continue;
                }
            }

            // Starting a prefix block?
            if let Some(c) = PREFIX_BLOCK_SYNTAX.captures(line) {
                // New prefix block with syntax specifier, always starts a block.
                let prefix = c.get(1).unwrap().as_str();
                let syntax = c.get(2).unwrap().as_str();
                block_indent = Some(Prefixed {
                    depth,
                    prefix: prefix.to_string(),
                });
                in_header = false;
                self.start_prefix_block_with_syntax(depth, prefix, syntax);
                continue;
            }

            // Continuing or starting a prefix block?
            if let Some(c) = PREFIX_BLOCK.captures(line) {
                let prefix = c.get(1).map(|t| t.as_str()).unwrap_or(&"");
                let body = c.get(2).unwrap().as_str();
                if let Some(Prefixed {
                    depth: d,
                    prefix: p,
                }) = &block_indent
                {
                    if *d == depth && p == prefix {
                        // Carry on.
                        if EMPTY_LINE.is_match(body) && (p == ":" || p == ">") { self.paragraph_break() }
                        self.prefix_block_line(depth, prefix, body);
                        continue;
                    }
                }
                // Starting a new block!
                block_indent = Some(Prefixed {
                    depth,
                    prefix: prefix.to_string(),
                });
                in_header = false;
                self.start_prefix_block(depth, prefix);
                // Don't bother with paragraph break even if it's an empty line, nothing before to
                // break against.
                self.prefix_block_line(depth, prefix, body);
                continue;
            }

            // Special case, empty lines in the moddle of a space-prefixed block. Treat these as
            // part of the block.
            if let Some(Prefixed { depth: d, prefix: p }) = &block_indent {
                if p.as_str() == "" && is_empty {
                    self.paragraph_break();
                    self.prefix_block_line(*d, "", "");
                }
            }

            block_indent = None;

            line = &line[depth as usize..];

            self.start_line(depth);
            let mut syntax = String::new();

            if let Some(c) = INDENT_BLOCK.captures(line) {
                let m = c.get(0).unwrap();
                line = &line[..m.start()];

                if let Some(prefix) = c.get(1) {
                    // Single char at end prefix, this never has a syntax.
                    block_indent = Some(Indented {
                        depth: depth + 1,
                        prefix: prefix.as_str().to_string(),
                    });
                } else if let Some(prefix) = c.get(2) {
                    // Three chars prefix, may be followed by syntax.
                    syntax = c.get(3).unwrap().as_str().to_string();
                    block_indent = Some(Indented {
                        depth: depth + 1,
                        prefix: prefix.as_str().to_string(),
                    });
                }
            }

            if WIKI_TITLE.is_match(line) {
                self.wiki_title(line);
                in_header = true;
            } else if in_header && (ALIASES_LINE.is_match(line) || TAGS_LINE.is_match(line)) {
                while !line.is_empty() {
                    line = parse_tag_alias_line(self, line);
                }
            } else {
                while !line.is_empty() {
                    line = parse_fragment(self, line);
                }
                in_header = false;
            }

            if let Some(Indented { depth, prefix }) = &block_indent {
                // This was set but we didn't loop through the block processor earlier,
                // the block was just started at this line.
                self.start_indent_block(*depth, prefix, &syntax);
            }

            self.end_line();
        }
    }
}
