#![cfg_attr(not(test), no_std)]
use core::fmt;
use core::ops::Deref;
use nom::types::CompleteStr;
use nom::{
    self, alt, count, delimited, do_parse, eof, line_ending, many1, map, named, not, one_of, opt,
    pair, peek, preceded, recognize, tag, take_while, take_while1, terminated,
};

/// Converts outline file text into lexical tokens.
pub struct Lexer<'a> {
    source: &'a str,
    block_state: Option<BlockSpec<'a>>,
    line_tokenizer: Option<LineTokenizer<'a>>,
    /// Are we set to parse tag and alias declarations
    in_header: bool,
    /// Leftmost column has depth 1 so we can use 0 for the start of the file.
    previous_depth: usize,
    /// Contain a closing token from previous cycle if needed.
    buffer: Ending<'a>,
}

enum Ending<'a> {
    None,
    Line,
    Block(&'a str),
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Lexer<'a> {
        Lexer {
            source,
            block_state: None,
            line_tokenizer: None,
            in_header: true,
            previous_depth: 0,
            buffer: Ending::None,
        }
    }
}

/// Return whether a prefix indicates a preformatted or a paragraphs block.
pub fn is_preformatted_block(block_prefix: &str) -> bool {
    match block_prefix {
        "" | ":" | ">" | "'''" => false,
        _ => true,
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Token<&'a str>;

    fn next(&mut self) -> Option<Self::Item> {
        // Emit a pending end of element tokens.
        match self.buffer {
            Ending::Block(prefix) => {
                self.buffer = Ending::None;
                return Some(Token::EndBlock(prefix));
            }
            Ending::Line => {
                self.buffer = Ending::None;
                return Some(Token::NewLine);
            }
            Ending::None => {}
        }

        // Still tokenizing the previous line, just keep doing that until it's consumed.
        if let Some(ref mut line_tok) = self.line_tokenizer {
            match line_tok.next() {
                // Catch an indent block specifier in the line.
                // Mark Lexer to be processing that block from here on.
                Some(Token::StartIndentBlock { prefix, syntax }) => {
                    self.block_state = Some(BlockSpec {
                        // previous_depth should be the depth of the line being processed at
                        // this point.
                        depth: self.previous_depth + 1,
                        line_prefix: None,
                        start_prefix: prefix,
                    });
                    return Some(Token::StartIndentBlock { prefix, syntax });
                }
                Some(tok) => return Some(tok),
                None => {
                    self.buffer = Ending::Line;
                    self.line_tokenizer = None;
                    return self.next();
                }
            }
        }

        // Now processing new input, stop if there is none.
        if self.source.is_empty() {
            return None;
        }

        // Check out the next line.
        let (rest, line) = complete_line(CompleteStr(self.source)).unwrap();

        // If we're parsing a text block, see if we can keep going.
        if let Some(BlockSpec {
            depth: d,
            line_prefix,
            start_prefix,
        }) = self.block_state
        {
            if let Some(prefix) = line_prefix {
                // It's a prefix block
                if let Ok((_, line)) = prefix_block_line(d, prefix, line) {
                    self.buffer = Ending::Line;
                    self.source = rest.0;
                    return Some(Token::BlockLine {
                        depth: d,
                        prefix: Some(prefix),
                        text: line.0,
                    });
                } else {
                    self.buffer = Ending::Block(prefix);
                    self.block_state = None;
                    return self.next();
                }
            } else {
                // It's an indent block.
                if let Ok((_, line)) = indent_block_line(d, line) {
                    self.buffer = Ending::Line;
                    self.source = rest.0;
                    // No prefix in token to mark this as an indent block line
                    return Some(Token::BlockLine {
                        depth: d,
                        prefix: None,
                        text: line.0,
                    });
                } else {
                    self.buffer = Ending::Block(start_prefix);
                    self.block_state = None;
                    return self.next();
                }
            }
        }

        // We know we'll consume the line now, so make this official.
        self.source = rest.0;

        // Actually investigate the line now.
        let (line, depth) = depth(line).unwrap();
        if depth > self.previous_depth {
            self.in_header = true;
        } else if depth < self.previous_depth {
            self.in_header = false;
        }
        self.previous_depth = depth;

        if let Ok((_, (prefix, body))) = prefix_block_regular(line) {
            self.block_state = Some(BlockSpec {
                depth,
                line_prefix: Some(prefix.0),
                start_prefix: prefix.0,
            });
            self.buffer = Ending::Line;
            return Some(Token::StartPrefixBlock2 {
                depth,
                prefix: prefix.0,
                first_line: body.0,
            });
        } else if let Ok((_, (prefix, syntax))) = prefix_block_syntax(line) {
            self.block_state = Some(BlockSpec {
                depth,
                line_prefix: Some(prefix.0),
                start_prefix: prefix.0,
            });
            self.buffer = Ending::Line;
            return Some(Token::StartPrefixBlock {
                depth,
                prefix: prefix.0,
                syntax: syntax.0,
            });
        }

        // Start parsing a header or a regular line.
        self.in_header =
            self.in_header && LineTokenizer::new_header(line.0).all(|t| t.is_header_token());
        if self.in_header {
            self.line_tokenizer = Some(LineTokenizer::new_header(line.0));
        } else {
            self.line_tokenizer = Some(LineTokenizer::new(line.0));
        }

        return Some(Token::StartLine(depth));
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Token<S> {
    /// Text block delimited by indention.
    ///
    /// ```notrust
    /// [ ][ ][ ]Some text at depth - 1[prefix][syntax] <- Matched suffix
    /// [ ][ ][ ][ ]Blocks of indented text follow...
    /// ```
    StartIndentBlock {
        prefix: S,
        syntax: S,
    },

    /// Syntax-specifying first line of a prefix delimited text block.
    ///
    /// The syntax string starts with a non-whitespace character and touches the prefix character.
    ///
    /// ```notrust
    /// [ ][ ][ ][ ][prefix][syntax] <- matched line
    /// [ ][ ][ ][ ][prefix] Lines of body text follow...
    /// ```
    StartPrefixBlock {
        depth: usize,
        prefix: S,
        syntax: S,
    },

    /// First line of a syntaxless prefix delimited block
    ///
    /// ```notrust
    /// [ ][ ][ ][ ][prefix] Lines of body text.
    /// ```
    StartPrefixBlock2 {
        depth: usize,
        prefix: S,
        first_line: S,
    },

    /// Further line in a text block started earlier.
    BlockLine {
        depth: usize,
        prefix: Option<S>,
        text: S,
    },

    /// End block with the given prefix.
    EndBlock(S),

    /// Start of a regular, non text block outline line at given depth.
    StartLine(usize),

    WikiTitle(S),
    AliasDefinition(S),
    TagDefinition(S),

    TextFragment(S),
    WhitespaceFragment(S),
    UrlFragment(S),
    WikiWordFragment(S),
    VerbatimFragment(S),
    FileLinkFragment(S),
    AliasLinkFragment(S),
    InlineImageFragment(S),
    ImportanceMarkerFragment,
    NewLine,
}

impl<S> Token<S> {
    fn is_header_token(&self) -> bool {
        match self {
            Token::WhitespaceFragment(_) => true,
            Token::AliasDefinition(_) => true,
            Token::TagDefinition(_) => true,
            _ => false,
        }
    }

    pub fn map<T>(self, f: impl Fn(S) -> T + Copy) -> Token<T> {
        use Token::*;
        match self {
            StartIndentBlock { prefix, syntax } => StartIndentBlock {
                prefix: f(prefix),
                syntax: f(syntax),
            },

            StartPrefixBlock {
                depth,
                prefix,
                syntax,
            } => StartPrefixBlock {
                depth,
                prefix: f(prefix),
                syntax: f(syntax),
            },

            StartPrefixBlock2 {
                depth,
                prefix,
                first_line,
            } => StartPrefixBlock2 {
                depth,
                prefix: f(prefix),
                first_line: f(first_line),
            },

            BlockLine {
                depth,
                prefix,
                text,
            } => BlockLine {
                depth,
                prefix: prefix.map(f),
                text: f(text),
            },

            StartLine(d) => StartLine(d),
            EndBlock(s) => EndBlock(f(s)),
            WikiTitle(s) => WikiTitle(f(s)),
            AliasDefinition(s) => AliasDefinition(f(s)),
            TagDefinition(s) => TagDefinition(f(s)),
            TextFragment(s) => TextFragment(f(s)),
            WhitespaceFragment(s) => WhitespaceFragment(f(s)),
            UrlFragment(s) => UrlFragment(f(s)),
            WikiWordFragment(s) => WikiWordFragment(f(s)),
            VerbatimFragment(s) => VerbatimFragment(f(s)),
            FileLinkFragment(s) => FileLinkFragment(f(s)),
            AliasLinkFragment(s) => AliasLinkFragment(f(s)),
            InlineImageFragment(s) => InlineImageFragment(f(s)),
            ImportanceMarkerFragment => ImportanceMarkerFragment,
            NewLine => NewLine,
        }
    }
}

impl<S: Deref<Target = str> + fmt::Display> fmt::Display for Token<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Token::*;
        match self {
            StartIndentBlock { prefix, syntax } => write!(f, "{}{}", prefix, syntax),
            StartPrefixBlock {
                depth,
                prefix,
                syntax,
            } => {
                for _ in 1..*depth {
                    write!(f, "\t")?;
                }
                write!(f, "{}{}", prefix, syntax)
            }
            StartPrefixBlock2 {
                depth,
                prefix,
                first_line,
            } => {
                for _ in 1..*depth {
                    write!(f, "\t")?;
                }
                write!(f, "{} {}", prefix, first_line)
            }
            BlockLine {
                depth,
                text,
                prefix,
            } => {
                for _ in 1..*depth {
                    write!(f, "\t")?;
                }
                if let Some(prefix) = prefix {
                    write!(f, "{} ", prefix)?;
                }
                write!(f, "{}", text)
            }
            EndBlock(_) => Ok(()),
            StartLine(depth) => {
                for _ in 1..*depth {
                    write!(f, "\t")?;
                }
                Ok(())
            }
            WikiTitle(t) => write!(f, "{}", t),
            AliasDefinition(t) => write!(f, "({})", t),
            TagDefinition(t) => write!(f, "@{}", t),
            TextFragment(t) | WhitespaceFragment(t) | UrlFragment(t) | WikiWordFragment(t) => {
                write!(f, "{}", t)
            }
            VerbatimFragment(t) => write!(f, "`{}`", t),
            FileLinkFragment(t) | AliasLinkFragment(t) => write!(f, "[{}]", t),
            InlineImageFragment(t) => write!(f, "![{}]", t),
            ImportanceMarkerFragment => write!(f, " *"),
            NewLine => writeln!(f),
        }
    }
}

/// Describe the state of the text block object currently being processed.
///
/// Indent blocks get `None` for prefix since their block lines are marked by indentation only.
struct BlockSpec<'a> {
    depth: usize,
    line_prefix: Option<&'a str>,
    start_prefix: &'a str,
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

/// Parse the entire line, match sufficiently indented indent text block
fn indent_block_line(
    expected_indent: usize,
    input: CompleteStr,
) -> nom::IResult<CompleteStr, CompleteStr> {
    alt!(
        input,
        map!(empty_line, |_| CompleteStr(""))
            | preceded!(count!(tag!("\t"), expected_indent - 1), complete_line)
    )
}

fn prefix_block_line<'a>(
    expected_indent: usize,
    prefix: &str,
    input: CompleteStr<'a>,
) -> nom::IResult<CompleteStr<'a>, CompleteStr<'a>> {
    if prefix.is_empty() {
        if let Ok((rest, _)) = empty_line(input) {
            return Ok((rest, CompleteStr("")));
        }
    }

    do_parse!(
        input,
        count!(tag!("\t"), expected_indent - 1)
            >> terminated!(tag!(prefix), tag!(" "))
            >> body: complete_line
            >> (body)
    )
}

named!(prefix_block_regular<CompleteStr, (CompleteStr, CompleteStr)>,
    pair!(
        map!(terminated!(
            opt!(alt!(tag!(":") | tag!(">") | tag!("<") | tag!(";"))),
            tag!(" ")), |t| t.unwrap_or(CompleteStr(""))),
        complete_line));

named!(prefix_block_syntax<CompleteStr, (CompleteStr, CompleteStr)>,
    pair!(
        alt!(tag!(":") | tag!(">") | tag!("<") | tag!(";")),
        complete_line));

named!(wiki_word_segment_head<CompleteStr, char>,
    // XXX: Is there a nice concise way to get |c| c.is_uppercase() here instead?
    one_of!("ABCDEFGHIJKLMNOPQRSTUVWXYZ"));

named!(wiki_word_segment_tail<CompleteStr, CompleteStr>,
    take_while1!(|c: char| c.is_lowercase() || c.is_numeric()));

named!(wiki_word_segment<CompleteStr, CompleteStr>,
    recognize!(pair!(wiki_word_segment_head, wiki_word_segment_tail)));

named!(wiki_word<CompleteStr, CompleteStr>,
    terminated!(
        recognize!(pair!(wiki_word_segment, many1!(wiki_word_segment))),
        peek!(not!(wiki_word_segment_head))));

named!(empty_line<CompleteStr, CompleteStr>,
    terminated!(take_while!(|c: char| c.is_whitespace()), alt!(line_ending | eof!())));

named!(depth<CompleteStr, usize>,
       map!(take_while!(|c| c == '\t'), |s| s.len() + 1));

named!(complete_line<CompleteStr, CompleteStr>,
    terminated!(take_while!(|c| c != '\n' && c != '\r'), alt!(line_ending | eof!())));

named!(verbatim<CompleteStr, CompleteStr>,
    delimited!(tag!("`"), take_while!(|c| c != '`'), tag!("`")));

named!(url<CompleteStr, CompleteStr>,
    recognize!(pair!(
        alt!(tag!("https://") | tag!("http://") | tag!("ftp://")),
        take_while!(is_url_char))));

named!(inline_image<CompleteStr, CompleteStr>,
    delimited!(tag!("!["), take_while!(is_path_char), tag!("]")));

named!(file_link<CompleteStr, CompleteStr>,
    delimited!(
        tag!("["),
        recognize!(pair!(
                alt!(tag!("./") | tag!("../")),
                take_while!(is_path_char))),
        tag!("]")));

named!(alias_link<CompleteStr, CompleteStr>,
    delimited!(tag!("["), take_while!(is_alias_char), tag!("]")));

named!(alias_definition<CompleteStr, CompleteStr>,
    delimited!(tag!("("), take_while!(is_alias_char), tag!(")")));

named!(tag_definition<CompleteStr, CompleteStr>,
    preceded!(tag!("@"), take_while!(is_tag_char)));

named!(importance_marker<CompleteStr, CompleteStr>,
    terminated!(tag!(" *"), eof!()));

named!(indent_block_with_syntax<CompleteStr, (CompleteStr, CompleteStr)>,
    pair!(alt!(tag!("'''") | tag!("```")), complete_line));

named!(indent_block_trail<CompleteStr, (CompleteStr, CompleteStr)>,
    map!(terminated!(alt!(tag!(">") | tag!(";")), eof!()), |p| (p, CompleteStr(""))));

named!(indent_block<CompleteStr, (CompleteStr, CompleteStr)>,
    alt!(indent_block_with_syntax | indent_block_trail));

struct LineTokenizer<'a> {
    text: &'a str,
    current_pos: usize,
    token_start: usize,
    can_start_url: bool,
    can_start_wiki_word: bool,
    header_mode: bool,
    buffer: Option<Token<&'a str>>,
    is_first: bool,
}

impl<'a> LineTokenizer<'a> {
    pub fn new(text: &'a str) -> LineTokenizer<'a> {
        LineTokenizer {
            text,
            current_pos: 0,
            token_start: 0,
            can_start_url: true,
            can_start_wiki_word: true,
            header_mode: false,
            buffer: None,
            is_first: true,
        }
    }

    pub fn new_header(text: &'a str) -> LineTokenizer<'a> {
        LineTokenizer {
            text,
            current_pos: 0,
            token_start: 0,
            can_start_url: true,
            can_start_wiki_word: true,
            header_mode: true,
            buffer: None,
            is_first: true,
        }
    }

    /// Flush pending text out as a token.
    ///
    /// This is called when a non-text token is detected.
    fn flush_text(&mut self) -> Option<Token<&'a str>> {
        if self.current_pos > self.token_start {
            let text = &self.text[self.token_start..self.current_pos];
            self.token_start = self.current_pos;
            if text.chars().all(|c| c.is_whitespace()) {
                Some(Token::WhitespaceFragment(text))
            } else {
                Some(Token::TextFragment(text))
            }
        } else {
            None
        }
    }

    /// Try a parser function against the current input.
    fn test(
        &mut self,
        f: impl Fn(CompleteStr) -> nom::IResult<CompleteStr, CompleteStr>,
    ) -> Option<(&'a str, usize)> {
        match f(CompleteStr(&self.text[self.current_pos..])) {
            Ok((rest, result)) => Some((result.0, self.text.len() - rest.0.len())),
            _ => None,
        }
    }

    /// Queue a new token, return either that or the pending text token.
    fn queue(&mut self, tok: Token<&'a str>, new_pos: usize) -> Option<Token<&'a str>> {
        debug_assert!(self.buffer.is_none());
        let ret = if let Some(t) = self.flush_text() {
            self.buffer = Some(tok);
            Some(t)
        } else {
            if let Token::WikiWordFragment(w) = tok {
                if self.is_first && self.text[new_pos..].is_empty() {
                    Some(Token::WikiTitle(w))
                } else {
                    Some(tok)
                }
            } else {
                Some(tok)
            }
        };
        self.current_pos = new_pos;
        self.token_start = new_pos;
        self.is_first = false;
        ret
    }
}

impl<'a> Iterator for LineTokenizer<'a> {
    type Item = Token<&'a str>;

    fn next(&mut self) -> Option<Self::Item> {
        // Return item stored from previous call.
        if let Some(t) = self.buffer {
            self.buffer = None;
            return Some(t);
        }

        while let Some(c) = &self.text[self.current_pos..].chars().next() {
            if let Ok((rest, (prefix, syntax))) =
                indent_block(CompleteStr(&self.text[self.current_pos..]))
            {
                return self.queue(
                    Token::StartIndentBlock {
                        prefix: prefix.0,
                        syntax: syntax.0,
                    },
                    self.text.len() - rest.len(),
                );
            }
            if let Some((inner, new_pos)) = self.test(verbatim) {
                return self.queue(Token::VerbatimFragment(inner), new_pos);
            }
            if self.can_start_url {
                if let Some((inner, new_pos)) = self.test(url) {
                    self.can_start_url = false;
                    return self.queue(Token::UrlFragment(inner), new_pos);
                }
            }
            if let Some((inner, new_pos)) = self.test(inline_image) {
                return self.queue(Token::InlineImageFragment(inner), new_pos);
            }
            if let Some((inner, new_pos)) = self.test(file_link) {
                return self.queue(Token::FileLinkFragment(inner), new_pos);
            }
            if let Some((inner, new_pos)) = self.test(alias_link) {
                return self.queue(Token::AliasLinkFragment(inner), new_pos);
            }
            if self.header_mode {
                if let Some((inner, new_pos)) = self.test(alias_definition) {
                    return self.queue(Token::AliasDefinition(inner), new_pos);
                }
                if let Some((inner, new_pos)) = self.test(tag_definition) {
                    return self.queue(Token::TagDefinition(inner), new_pos);
                }
            }
            if let Some((_, new_pos)) = self.test(importance_marker) {
                return self.queue(Token::ImportanceMarkerFragment, new_pos);
            }
            if self.can_start_wiki_word {
                if let Some((inner, new_pos)) = self.test(wiki_word) {
                    self.can_start_url = false;
                    return self.queue(Token::WikiWordFragment(inner), new_pos);
                }
            }
            self.can_start_url = !is_url_char(*c);
            self.can_start_wiki_word = !c.is_alphanumeric();
            self.current_pos += c.len_utf8();
        }

        return self.flush_text();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nom::types::CompleteStr as S;

    fn parse(input: &str) -> String {
        let mut buf = String::new();
        for t in Lexer::new(input) {
            buf.push_str(&format!("{:?}", t));
        }
        buf
    }

    #[test]
    fn test_components() {
        assert!(importance_marker(S(" *")).is_ok());
        assert!(importance_marker(S("")).is_err());
        assert!(importance_marker(S(" * x")).is_err());

        assert_eq!(complete_line(S("")), Ok((S(""), S(""))));
        assert_eq!(complete_line(S("\nbaz")), Ok((S("baz"), S(""))));
        assert_eq!(complete_line(S("foobar")), Ok((S(""), S("foobar"))));
        assert_eq!(complete_line(S("foobar\nbaz")), Ok((S("baz"), S("foobar"))));
        assert_eq!(
            complete_line(S("foobar\r\nbaz")),
            Ok((S("baz"), S("foobar")))
        );

        assert_eq!(
            indent_block_line(3, S("\t\t\tcode")),
            Ok((S(""), S("\tcode")))
        );
        assert!(indent_block_line(3, S("\tcode")).is_err());

        assert!(wiki_word(S("")).is_err());
        assert!(wiki_word(S("word")).is_err());
        assert!(wiki_word(S("Word")).is_err());
        assert!(wiki_word(S("aWikiWord")).is_err());
        assert!(wiki_word(S("WikiW")).is_err());
        assert!(wiki_word(S("WikiWordW")).is_err());
        assert_eq!(wiki_word(S("WikiWord")), Ok((S(""), S("WikiWord"))));
        assert_eq!(wiki_word(S("Wiki1Word2")), Ok((S(""), S("Wiki1Word2"))));
        assert_eq!(wiki_word(S("WikiWord-s")), Ok((S("-s"), S("WikiWord"))));
    }

    #[test]
    fn test_parsing() {
        assert_eq!(parse(""), "");
        assert_eq!(
            parse("; A\nB"),
            "StartPrefixBlock2 { depth: 1, prefix: \";\", \
             first_line: \"A\" }NewLineEndBlock(\";\")StartLine(1)\
             TextFragment(\"B\")NewLine"
        );
    }
}
