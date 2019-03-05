#![no_std]
use nom::{self, alt, delimited, named, pair, preceded, recognize, tag, take_while};

/// Receiver interface for parsed outline file text.
///
/// Many methods have default implementations that echo the standard outline syntax.
pub trait OutlineWriter: Sized {
    fn start_line(&mut self, depth: i32) {
        for _ in 0..depth {
            self.text("\t");
        }
    }

    /// Write some regular text in the current element.
    fn text(&mut self, text: &str);

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
    fn start_indent_block(
        &mut self,
        depth: i32,
        prefix: &str,
        block_type: BlockType,
        syntax: &str,
    ) {
        self.text(prefix);
        self.text(syntax);
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
    fn start_prefix_block(
        &mut self,
        depth: i32,
        prefix: &str,
        block_type: BlockType,
        syntax: Option<&str>,
    ) {
        if let Some(syntax) = syntax {
            self.start_line(depth);
            self.text(prefix);
            self.text(syntax);
            self.end_line();
        }
    }

    fn text_block_line(&mut self, depth: i32, prefix: Option<&str>, text: &str) {
        if !text.is_empty() || prefix.map_or(false, |s| !s.is_empty()) {
            self.start_line(depth);
            if let Some(prefix) = prefix {
                self.text(prefix);
                if !text.is_empty() {
                    self.text(" ");
                }
            }
            self.text(text);
        }

        self.end_line();
    }

    fn end_text_block(&mut self, prefix: &str, block_type: BlockType) {}

    /// Signal a paragraph break in a non-preformatted text block.
    fn paragraph_break(&mut self) {}

    /// Called at the start of a highlighted line.
    fn important_line(&mut self) {}

    fn importance_marker(&mut self) {
        self.text(" *");
    }

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

    fn parse(&mut self, input: &str) {
        use BlockSpec::*;

        let mut block_indent = None;
        let mut prev_depth = -1;
        let mut in_header = true;

        for mut line in input.lines() {
            let depth = line.chars().take_while(|&c| c == '\t').count() as i32;
            let is_empty = is_empty(line);

            if !is_empty {
                if depth > prev_depth {
                    in_header = true;
                }
                prev_depth = depth;
            }

            // Indent block.
            if let Some(Indented { depth: d, prefix }) = &block_indent {
                if depth < *d && !is_empty {
                    // Out of block, zero indent memory and return to normal logic.
                    block_indent.map(|b| b.end(self));
                    block_indent = None;
                } else {
                    let cut_line = if is_empty { "" } else { &line[*d as usize..] };
                    if is_empty && (prefix == &">" || prefix == &"'''") {
                        self.paragraph_break();
                    }
                    // Within block, push text as-is and continue
                    self.text_block_line(*d, None, cut_line);
                    continue;
                }
            }

            line = &line[depth as usize..];

            // Prefix block
            let space_prefix_block = line.starts_with(" ");
            if space_prefix_block
                || line.starts_with(";")
                || line.starts_with(":")
                || line.starts_with("<")
                || line.starts_with(">")
            {
                in_header = false;

                let prefix = if space_prefix_block || line.is_empty() {
                    ""
                } else {
                    &line[..1]
                };

                let syntax = if !line.is_empty() { &line[1..] } else { "" };
                let current_spec = Prefixed { depth, prefix };
                let block_type = current_spec.block_type();

                if !space_prefix_block
                    && syntax.chars().next().map_or(false, |c| !c.is_whitespace())
                {
                    // Legit syntax line, start a new prefix block.
                    block_indent.map(|b| b.end(self));
                    block_indent = Some(current_spec);
                    self.start_prefix_block(depth, prefix, block_type, Some(syntax));
                    continue;
                }

                let body = if line.len() > prefix.len() {
                    &line[prefix.len() + 1..]
                } else {
                    ""
                };

                // Do we add to a pre-existing block?
                if let Some(Prefixed {
                    depth: d,
                    prefix: p,
                }) = &block_indent
                {
                    if *d == depth && *p == prefix {
                        if body.is_empty() && block_type == BlockType::Paragraphs {
                            self.paragraph_break();
                        }
                        self.text_block_line(depth, Some(prefix), body);
                        continue;
                    } else {
                    }
                }
                // Pre-existing block doesn't match, start a new one.
                block_indent.map(|b| b.end(self));
                block_indent = Some(current_spec);
                self.start_prefix_block(depth, prefix, block_type, None);
                self.text_block_line(depth, Some(prefix), body);
                continue;
            }

            // If we got this far, we've fallen through all block processing so clear the cached
            // block state.
            block_indent.map(|b| b.end(self));
            block_indent = None;

            // Don't emit anything for an empty line.
            if is_empty {
                self.end_line();
                continue;
            }

            // Regular line
            self.start_line(depth);

            let mut token_count = 0;
            let mut is_header_line = true;

            for tok in Tokenizer::new(line) {
                token_count += 1;
                if !tok.is_tag_def() && !tok.is_alias_def() {
                    is_header_line = false;
                }
                if tok == Token::ImportanceMarker {
                    self.important_line();
                }
            }

            // Aliases and tags can only be defined at the start of a block (in_header)
            if in_header && is_header_line {
                for tok in Tokenizer::new(line) {
                    match tok {
                        Token::WhiteSpaceText(s) => self.text(s),
                        Token::TagDefinition(s) => self.tag_definition(s),
                        Token::AliasDefinition(s) => self.alias_definition(s),
                        _ => panic!("Should not happen"),
                    }
                }
                self.end_line();
                continue;
            } else {
                in_header = false;
            }

            for tok in Tokenizer::new(line) {
                match tok {
                    Token::Text(s) => self.text(s),
                    Token::WhiteSpaceText(s) => self.text(s),
                    Token::Verbatim(s) => self.verbatim_text(s),
                    Token::Url(s) => self.url(s),
                    Token::WikiWord(s) => {
                        if token_count == 1 {
                            self.wiki_word_heading(s)
                        } else {
                            self.wiki_word_link(s)
                        }
                    }
                    Token::FileLink(s) => self.local_link(s),
                    Token::InlineImage(s) => self.inline_image(s),
                    Token::AliasLink(s) => self.alias_link(s),

                    // Just emit regular text if alias or tag definitions are lexed outside of
                    // header lines.
                    Token::AliasDefinition(s) => {
                        self.text("(");
                        self.text(s);
                        self.text(")");
                    }
                    Token::TagDefinition(s) => {
                        self.text("@");
                        self.text(s);
                    }

                    Token::ImportanceMarker => {
                        self.importance_marker();
                    }

                    Token::IndentBlock(prefix, syntax) => {
                        block_indent = Some(Indented {
                            depth: depth + 1,
                            prefix,
                        });
                        self.start_indent_block(
                            depth,
                            prefix,
                            block_indent.as_ref().unwrap().block_type(),
                            syntax,
                        );
                    }
                }
            }
            self.end_line();
        }
    }
}

/// Type of a text block.
#[derive(Eq, PartialEq)]
pub enum BlockType {
    /// Block text should be formatted into paragraphs.
    Paragraphs,

    /// Block text's newlines and whitespace should be rendered as is.
    Preformatted,
}

/// Describe the state of the text block object currently being processed.
enum BlockSpec<'a> {
    Indented { depth: i32, prefix: &'a str },
    Prefixed { depth: i32, prefix: &'a str },
}

impl<'a> BlockSpec<'a> {
    fn prefix(&self) -> &str {
        match self {
            BlockSpec::Indented { prefix, .. } => prefix,
            BlockSpec::Prefixed { prefix, .. } => prefix,
        }
    }

    fn block_type(&self) -> BlockType {
        match self.prefix() {
            "" | ":" | ">" | "'''" => BlockType::Paragraphs,
            _ => BlockType::Preformatted,
        }
    }

    fn end(&self, writer: &mut impl OutlineWriter) {
        writer.end_text_block(self.prefix(), self.block_type());
    }
}

/// Character that can show up in an URL.
///
/// See https://tools.ietf.org/html/rfc3986#appendix-A
fn is_url_char(c: char) -> bool {
    match c {
        '-' | '.' | '_' | '~' | ':' | '/' | '?' | '#' | '[' | ']' | '@' | '!' | '$' | '&'
        | '\'' | '(' | ')' | '*' | '+' | ',' | ';' | '=' => true,
        c if c.is_alphanumeric() => true,
        _ => false,
    }
}

fn is_path_char(c: char) -> bool {
    match c {
        '-' | '.' | '_' | '/' => true,
        c if c.is_alphanumeric() => true,
        _ => false,
    }
}

fn is_alias_char(c: char) -> bool {
    match c {
        '-' | '.' | '_' | '/' => true,
        c if c.is_alphanumeric() => true,
        _ => false,
    }
}

fn is_tag_char(c: char) -> bool {
    match c {
        '-' | '_' => true,
        c if c.is_alphanumeric() => true,
        _ => false,
    }
}

fn wiki_word(input: &str) -> nom::IResult<&str, &str> {
    let mut parts = 0;
    let mut at_part_start = false;
    let mut pos = 0;
    while let Some(c) = &input[pos..].chars().next() {
        if c.is_uppercase() {
            if !at_part_start {
                at_part_start = true;
            } else {
                break;
            }
        } else if c.is_lowercase() || c.is_numeric() {
            if !at_part_start && parts == 0 {
                // Before the word started, error.
                break;
            } else if at_part_start {
                parts += 1;
                at_part_start = false;
            }
        } else {
            break;
        }

        pos += c.len_utf8();
    }

    if parts >= 2 && !at_part_start {
        Ok((&input[pos..], &input[..pos]))
    } else {
        // TODO: Is there a more concise way to declare your own errors?
        Err(nom::Err::Error(nom::Context::Code(
            input,
            nom::ErrorKind::Custom(0),
        )))
    }
}

named!(
    verbatim<&str, &str>,
    delimited!(tag!("`"), take_while!(|c| c != '`'), tag!("`"))
);

named!(
    url<&str, &str>,
    recognize!(pair!(
        alt!(tag!("https://") | tag!("http://") | tag!("ftp://")),
        take_while!(is_url_char))));

named!(
    inline_image<&str, &str>,
    delimited!(tag!("!["), take_while!(is_path_char), tag!("]"))
);

named!(
    file_link<&str, &str>,
    delimited!(
        tag!("["),
        recognize!(pair!(
                alt!(tag!("./") | tag!("../")),
                take_while!(is_path_char))),
        tag!("]"))
    );

named!(alias_link<&str, &str>, delimited!(tag!("["), take_while!(is_alias_char), tag!("]")));

named!(alias_definition<&str, &str>, delimited!(tag!("("), take_while!(is_alias_char), tag!(")")));

named!(tag_definition<&str, &str>, preceded!(tag!("@"), take_while!(is_tag_char)));

fn importance_marker(input: &str) -> nom::IResult<&str, &str> {
    if input == " *" {
        Ok(("", input))
    } else {
        Err(nom::Err::Error(nom::Context::Code(
            input,
            nom::ErrorKind::Custom(0),
        )))
    }
}

fn indent_block(input: &str) -> nom::IResult<&str, (&str, &str)> {
    if input.len() == 1 {
        if input.starts_with(";") || input.starts_with(">") {
            return Ok(("", (input, "")));
        }
    } else if input.starts_with("```") || input.starts_with("'''") {
        return Ok(("", (&input[..3], &input[3..])));
    }

    Err(nom::Err::Error(nom::Context::Code(
        input,
        nom::ErrorKind::Custom(0),
    )))
}

fn is_empty(input: &str) -> bool {
    for c in input.chars() {
        if !c.is_whitespace() {
            return false;
        }
    }
    true
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Token<'a> {
    Text(&'a str),
    WhiteSpaceText(&'a str),
    Verbatim(&'a str),
    Url(&'a str),
    WikiWord(&'a str),
    FileLink(&'a str),
    InlineImage(&'a str),
    AliasLink(&'a str),
    AliasDefinition(&'a str),
    TagDefinition(&'a str),
    IndentBlock(&'a str, &'a str),
    ImportanceMarker,
}

impl<'a> Token<'a> {
    fn is_alias_def(&self) -> bool {
        match self {
            Token::WhiteSpaceText(_) => true,
            Token::AliasDefinition(_) => true,
            _ => false,
        }
    }

    fn is_tag_def(&self) -> bool {
        match self {
            Token::WhiteSpaceText(_) => true,
            Token::TagDefinition(_) => true,
            _ => false,
        }
    }
}

struct Tokenizer<'a> {
    text: &'a str,
    current_pos: usize,
    token_start: usize,
    can_start_url: bool,
    can_start_wiki_word: bool,
    buffer: Option<Token<'a>>,
}

impl<'a> Tokenizer<'a> {
    pub fn new(text: &'a str) -> Tokenizer<'a> {
        Tokenizer {
            text,
            current_pos: 0,
            token_start: 0,
            can_start_url: true,
            can_start_wiki_word: true,
            buffer: None,
        }
    }

    /// Flush pending text out as a token.
    ///
    /// This is called when a non-text token is detected.
    fn flush_text(&mut self) -> Option<Token<'a>> {
        if self.current_pos > self.token_start {
            let text = &self.text[self.token_start..self.current_pos];
            self.token_start = self.current_pos;
            if text.chars().all(|c| c.is_whitespace()) {
                Some(Token::WhiteSpaceText(text))
            } else {
                Some(Token::Text(text))
            }
        } else {
            None
        }
    }

    /// Try a parser function against the current input.
    fn test(&mut self, f: impl Fn(&str) -> nom::IResult<&str, &str>) -> Option<(&'a str, usize)> {
        match f(&self.text[self.current_pos..]) {
            Ok((rest, result)) => Some((result, self.text.len() - rest.len())),
            _ => None,
        }
    }

    /// Queue a new token, return either that or the pending text token.
    fn queue(&mut self, tok: Token<'a>, new_pos: usize) -> Option<Token<'a>> {
        debug_assert!(self.buffer.is_none());
        let ret = if let Some(t) = self.flush_text() {
            self.buffer = Some(tok);
            Some(t)
        } else {
            Some(tok)
        };
        self.current_pos = new_pos;
        self.token_start = new_pos;
        ret
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        // Return item stored from previous call.
        if let Some(t) = self.buffer {
            self.buffer = None;
            return Some(t);
        }

        while let Some(c) = &self.text[self.current_pos..].chars().next() {
            if let Ok((rest, (prefix, syntax))) = indent_block(&self.text[self.current_pos..]) {
                return self.queue(
                    Token::IndentBlock(prefix, syntax),
                    self.text.len() - rest.len(),
                );
            }
            if let Some((inner, new_pos)) = self.test(verbatim) {
                return self.queue(Token::Verbatim(inner), new_pos);
            }
            if self.can_start_url {
                if let Some((inner, new_pos)) = self.test(url) {
                    self.can_start_url = false;
                    return self.queue(Token::Url(inner), new_pos);
                }
            }
            if let Some((inner, new_pos)) = self.test(inline_image) {
                return self.queue(Token::InlineImage(inner), new_pos);
            }
            if let Some((inner, new_pos)) = self.test(file_link) {
                return self.queue(Token::FileLink(inner), new_pos);
            }
            if let Some((inner, new_pos)) = self.test(alias_link) {
                return self.queue(Token::AliasLink(inner), new_pos);
            }
            if let Some((inner, new_pos)) = self.test(alias_definition) {
                return self.queue(Token::AliasDefinition(inner), new_pos);
            }
            if let Some((inner, new_pos)) = self.test(tag_definition) {
                return self.queue(Token::TagDefinition(inner), new_pos);
            }
            if let Some((_, new_pos)) = self.test(importance_marker) {
                return self.queue(Token::ImportanceMarker, new_pos);
            }
            if self.can_start_wiki_word {
                if let Some((inner, new_pos)) = self.test(wiki_word) {
                    self.can_start_url = false;
                    return self.queue(Token::WikiWord(inner), new_pos);
                }
            }
            self.can_start_url = !is_url_char(*c);
            self.can_start_wiki_word = !c.is_alphanumeric();
            self.current_pos += c.len_utf8();
        }

        return self.flush_text();
    }
}
