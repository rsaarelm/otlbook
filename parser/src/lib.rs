use md5::Digest;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while1, take_while_m_n},
    character::complete::{line_ending, one_of},
    combinator::{map, map_res, not, opt, peek, recognize, verify},
    multi::{count, many0, many1},
    sequence::{delimited, pair, preceded, terminated},
};
use serde::{Deserialize, Serialize};
use std::default;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;
use std::str::FromStr;

// TODO: Make outline::Outline the main Outline type, figure out what to do with the one below.
pub mod outline;

#[derive(Eq, PartialEq, Debug, Default, Serialize, Deserialize)]
/// Representation of an outliner-formatted text document.
pub struct Outline {
    /// Number of extra indentation steps relative to the outline's parent.
    indent: usize,
    body: OutlineBody,
    children: Vec<Outline>,
}

impl Outline {
    /// Create a new complete outline from a list of child nodes.
    pub fn new(children: Vec<Outline>) -> Outline {
        Outline {
            indent: 0,
            body: Default::default(),
            children,
        }
    }

    /// Create a new child node in an outline.
    ///
    /// A full outline is equivalent to a child node with indent 0 and an empty `Default` body.
    pub fn new_node(indent: usize, body: OutlineBody, children: Vec<Outline>) -> Outline {
        if body.is_indent_block() && !children.is_empty() {
            // Everything indented deeper than an indent block directly below the block line
            // belongs to the block, the block node can't have child nodes.
            panic!("Indent block node can't have children");
        }

        Outline {
            indent,
            body,
            children,
        }
    }

    /// Load the outline from file path.
    ///
    /// The outline will get a toplevel name derived from the file name.
    pub fn load(path: impl AsRef<Path>) -> Result<Outline, Box<dyn Error>> {
        // TODO: Error handling instead of unwraps.
        let basename = path
            .as_ref()
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let text = fs::read_to_string(path)?;

        // Parsing the outline should succeed with any string.
        let mut ret: Outline = text.parse().unwrap();
        ret.body = (outline_body(0, &*basename).unwrap().1).1;
        Ok(ret)
    }

    pub fn aliases(&self) -> impl Iterator<Item = &String> {
        self.children
            .iter()
            .take_while(|c| c.can_be_header())
            .flat_map(|c| {
                c.fragments().filter_map(|f| match f {
                    Fragment::AliasDefinition(s) => Some(s),
                    _ => None,
                })
            })
    }

    pub fn tags(&self) -> impl Iterator<Item = &String> {
        self.children
            .iter()
            .take_while(|c| c.can_be_header())
            .flat_map(|c| {
                c.fragments().filter_map(|f| match f {
                    Fragment::TagLink(s) => Some(s),
                    _ => None,
                })
            })
    }

    /// Return title if outline is toplevel of a wiki article.
    pub fn wiki_title(&self) -> Option<&str> {
        match self.body {
            OutlineBody::Line(ref fs) => {
                if let [Fragment::WikiWord(word)] = &fs[..] {
                    Some(word.as_str())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn ctags(
        &self,
        depth: usize,
        path: &str,
    ) -> impl Iterator<Item = (String, usize, String, TagAddress)> {
        let child_tags: Vec<(String, usize, String, TagAddress)> = self
            .children
            .iter()
            .flat_map(|c| c.ctags(depth + 1, path))
            .collect();
        let mut tags = Vec::new();
        if let Some(title) = self.wiki_title() {
            let addr = if depth == 0 {
                TagAddress::LineNum(0)
            } else {
                TagAddress::Search(title.to_string())
            };

            tags.push((title.to_string(), depth, path.to_string(), addr.clone()));
            for a in self.aliases() {
                tags.push((a.to_string(), depth, path.to_string(), addr.clone()));
            }
        }

        tags.into_iter().chain(child_tags.into_iter())
    }

    /// Return true if outline is a block and should be preformatted.
    pub fn is_preformatted_block(&self) -> bool {
        match self.body {
            OutlineBody::Block { ref prefix, .. } => match prefix.as_str() {
                "" | ":" | ">" | "'''" => false,
                _ => true,
            },
            _ => false,
        }
    }

    fn can_be_header(&self) -> bool {
        match self.body {
            OutlineBody::Line(ref fs) => fs.iter().all(|f| f.can_be_header()),
            _ => false,
        }
    }

    fn fragments(&self) -> impl Iterator<Item = &Fragment> {
        match self.body {
            OutlineBody::Line(ref fs) => fs.iter(),
            OutlineBody::Block {
                indent_line: Some(ref fs),
                ..
            } => fs.iter(),
            _ => (&[]).iter(),
        }
    }

    fn fmt_with_depth(&self, f: &mut fmt::Formatter, depth: usize) -> fmt::Result {
        use OutlineBody::*;

        fn indent(f: &mut fmt::Formatter, i: usize) -> fmt::Result {
            for _ in 1..i {
                write!(f, "\t")?;
            }
            Ok(())
        }

        let depth = depth + self.indent;

        if depth > 0 {
            match self.body {
                Line(ref line) => {
                    indent(f, depth)?;
                    for x in line {
                        write!(f, "{}", x)?;
                    }
                    writeln!(f)?;
                }
                Block {
                    ref indent_line,
                    ref syntax,
                    ref prefix,
                    ref lines,
                    ..
                } => {
                    if let Some(line) = indent_line {
                        // It's an indent block.
                        indent(f, depth)?;
                        for x in line {
                            write!(f, "{}", x)?;
                        }
                        write!(f, "{}", prefix)?;
                        if let Some(syntax) = syntax {
                            write!(f, "{}", syntax)?;
                        }
                        writeln!(f)?;

                        for line in lines {
                            if line.is_empty() {
                                writeln!(f)?;
                            } else {
                                indent(f, depth + 1)?;
                                writeln!(f, "{}", line)?;
                            }
                        }
                    } else {
                        // It's a prefix block
                        if let Some(syntax) = syntax {
                            indent(f, depth)?;
                            writeln!(f, "{}{}", prefix, syntax)?;
                        }
                        for line in lines {
                            if line.is_empty() && prefix.is_empty() {
                                writeln!(f)?;
                            } else {
                                indent(f, depth)?;
                                writeln!(f, "{} {}", prefix, line)?;
                            }
                        }
                    }
                }
            }
        }

        for c in &self.children {
            c.fmt_with_depth(f, depth + 1)?;
        }
        Ok(())
    }

    pub fn indent(&self) -> usize {
        self.indent
    }

    pub fn body(&self) -> &OutlineBody {
        &self.body
    }

    pub fn children(&self) -> impl Iterator<Item = &'_ Outline> {
        self.children.iter()
    }

    pub fn children_mut(&mut self) -> impl Iterator<Item = &'_ mut Outline> {
        self.children.iter_mut()
    }
}

impl FromStr for Outline {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match outline(0, s) {
            Ok((_, ret)) => Ok(ret),
            Err(_) => Err(()),
        }
    }
}

impl fmt::Display for Outline {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // XXX: Okay, hacky part incoming.
        //
        // The toplevel outline (depth == 0) is special. It has no body, only children, and all the
        // children must be printed without indentation. When recursing down and printing the child
        // outlines with Display, they're going to look just like the toplevel otherwise, but will
        // have a non-empty body. So we do a switcheroo here with the indent depth where a bodied
        // outline will start at depth 1 (actually showing things) and the bodiless one will start
        // at 0 so that its children will all be on the first-visible depth 1.
        self.fmt_with_depth(f, if self.body.is_empty() { 0 } else { 1 })
    }
}

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum OutlineBody {
    Line(Vec<Fragment>),
    Block {
        // An indent block gets the preceding line as a part of the block.
        // The block is an indent block if `indent_line` exists.
        indent_line: Option<Vec<Fragment>>,
        syntax: Option<String>,
        prefix: String,
        lines: Vec<String>,
    },
}

impl OutlineBody {
    pub fn is_empty(&self) -> bool {
        match self {
            OutlineBody::Line(v) => v.is_empty(),
            _ => false,
        }
    }

    pub fn is_indent_block(&self) -> bool {
        match self {
            OutlineBody::Block { indent_line, .. } => indent_line.is_some(),
            _ => false,
        }
    }
}

impl default::Default for OutlineBody {
    fn default() -> Self {
        OutlineBody::Line(Vec::new())
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum TagAddress {
    LineNum(usize),
    Search(String),
}

impl fmt::Display for TagAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TagAddress::LineNum(n) => write!(f, "{}", n),
            TagAddress::Search(expr) => write!(f, "/^\\t\\*{}$/", expr),
        }
    }
}

/// Parsed information from a syntax line.
#[derive(Default, Eq, PartialEq, Debug)]
pub struct SyntaxInfo {
    /// Scripting language used
    pub lang: Option<String>,
    /// Is this a library section instead of a script?
    ///
    /// Library sections are catenated to all script blocks that come after them.
    pub is_lib: bool,
    /// Checksum for input and output from last script evaluation.
    ///
    /// No need to re-evaluate the script if the checksum still matches the text.
    pub checksum: Option<Digest>,
}

impl SyntaxInfo {
    pub fn new(source: &str) -> SyntaxInfo {
        if let Ok((_, ((lang, is_lib), checksum))) = syntax_line(source) {
            SyntaxInfo {
                lang: Some(lang),
                is_lib,
                checksum,
            }
        } else {
            Default::default()
        }
    }
}

fn outline(depth: usize, input: &str) -> nom::IResult<&str, Outline> {
    if input.is_empty() {
        Ok((input, Default::default()))
    } else {
        non_empty_outline(depth, input)
    }
}

/// Parse text into an outline
fn non_empty_outline(depth: usize, input: &str) -> nom::IResult<&str, Outline> {
    if log::log_enabled!(log::Level::Trace) {
        let line = input.lines().next();
        log::trace!("Parsing outline {:?}, depth: {}", line, depth);
    }

    if input.is_empty() {
        return nom_err(input);
    }

    let (rest, (body_depth, body)) = if depth == 0 {
        // Depth 0 means we're parsing an entire file and the parent line doesn't exist.
        Ok((input, (0, OutlineBody::default())))
    } else {
        outline_body(depth, input)
    }?;

    let (rest, children) = child_outlines(body_depth + 1, rest)?;

    Ok((
        rest,
        Outline {
            indent: body_depth - depth,
            body,
            children,
        },
    ))
}

fn outline_body(min_indent: usize, input: &str) -> nom::IResult<&str, (usize, OutlineBody)> {
    let (line_start, d) = depth(input).unwrap();
    if d < min_indent {
        log::trace!("outline_body: Above expected indent level");
        return nom_err(input);
    }

    if let Ok((rest, _)) = empty_line(input) {
        log::trace!("outline_body: Empty line");
        return Ok((rest, (min_indent, OutlineBody::default())));
    }

    if let Ok(ret) = prefix_block(min_indent, input) {
        log::trace!("outline_body: parsed prefix block {:?}", ret);
        return Ok(ret);
    }

    let mut line = Vec::new();
    let mut current = line_start;
    loop {
        if let Ok((rest, _)) = eol(current) {
            return Ok((rest, (d, OutlineBody::Line(line))));
        }

        if let Ok((rest, (prefix, syntax))) = indent_block_start(current) {
            let (rest, lines) = indent_block_lines(d + 1, rest).unwrap_or((rest, Vec::new()));
            log::trace!(
                "outline_body: Indent block {:?}, {} of {} lines",
                line,
                prefix,
                lines.len()
            );
            return Ok((
                rest,
                (
                    d,
                    OutlineBody::Block {
                        indent_line: Some(line),
                        syntax: syntax.map(|s| s.to_string()),
                        prefix: prefix.to_string(),
                        lines: lines,
                    },
                ),
            ));
        }

        if let Ok((rest, f)) = fragment(current) {
            log::trace!("outline_body: Pushed fragment {:?}", f);
            line.push(f);
            current = rest;
        } else {
            break;
        }
    }
    Ok((current, (d, OutlineBody::Line(line))))
}

// Return indent and body on match.
fn prefix_block(min_indent: usize, input: &str) -> nom::IResult<&str, (usize, OutlineBody)> {
    fn prefix_block_line<'a>(
        indent: usize,
        prefix: &str,
        i: &'a str,
    ) -> nom::IResult<&'a str, &'a str> {
        debug_assert!(indent > 0);

        if i.is_empty() {
            return nom_err(i);
        }

        // Allow empty lines in space-prefix blocks that don't need to have even the indent.
        if prefix.is_empty() {
            if let Ok((rest, _)) = empty_line(i) {
                return Ok((rest, ""));
            }
        }

        // Allow non-empty prefix to be separated from text with tab as well as space, to avoid the
        // more confusing alternative of parsing a non-empty prefix + tab as a syntax line.
        let (i, _) = count(tag("\t"), indent - 1)(i)?;
        let (i, _) = terminated(
            tag(prefix),
            one_of(if prefix.is_empty() { " " } else { " \t" }),
        )(i)?;
        complete_line(i)
    }

    fn prefix_block_syntax_line<'a>(
        indent: usize,
        prefix: &str,
        i: &'a str,
    ) -> nom::IResult<&'a str, &'a str> {
        debug_assert!(indent > 0);
        if prefix.is_empty() {
            return nom_err(i);
        }

        let (i, _) = count(tag("\t"), indent - 1)(i)?;
        let (i, _) = tag(prefix)(i)?;
        verify(complete_line, |s: &str| match s.chars().next() {
            Some(' ') | Some('\t') | None => false,
            _ => true,
        })(i)
    }

    // Need to figure out the depth here, since it's allowed to be deeper than min_indent.
    let mut indent = 1;
    let mut prefix = String::new();

    for c in input.chars() {
        match c {
            '\t' => {
                indent += 1;
            }
            ':' | ';' | '>' | '<' => {
                prefix = format!("{}", c);
                break;
            }
            ' ' => break,
            _ => return nom_err(input),
        }
    }
    if indent < min_indent {
        return nom_err(input);
    }

    log::trace!("prefix_block indent: {}, prefix: {:?}", indent, prefix);

    let (rest, (syntax, lines)) = pair(
        opt(|i| prefix_block_syntax_line(indent, &prefix, i)),
        many0(|i| prefix_block_line(indent, &prefix, i)),
    )(input)?;

    log::trace!("prefix_block syntax_line: {:?}, lines: {:?}", syntax, lines);

    if syntax.is_none() && lines.is_empty() {
        return nom_err(input);
    }

    Ok((
        rest,
        (
            indent,
            OutlineBody::Block {
                indent_line: None,
                syntax: syntax.map(|s| s.to_string()),
                prefix,
                lines: lines.into_iter().map(|s| s.to_string()).collect(),
            },
        ),
    ))
}

fn child_outlines(min_indent: usize, input: &str) -> nom::IResult<&str, Vec<Outline>> {
    if input.is_empty() {
        return Ok((input, Vec::new()));
    }

    let outline = |i| non_empty_outline(min_indent, i);

    many0(outline)(input)
}

/// Parse text block delimited only by sufficient indentation.
fn indent_block_lines(min_indent: usize, input: &str) -> nom::IResult<&str, Vec<String>> {
    many0(|i| indent_block_line(min_indent, i))(input)
}

fn indent_block_line(expected_indent: usize, input: &str) -> nom::IResult<&str, String> {
    if input.is_empty() {
        return nom_err(input);
    }

    alt((
        map(empty_line, |_| String::new()),
        map(
            preceded(count(tag("\t"), expected_indent - 1), complete_line),
            |s| s.to_string(),
        ),
    ))(input)
}

/// Line element fragments.
#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum Fragment {
    WikiWord(String),
    Verbatim(String),
    InlineImage(String),
    FileLink(String),
    AliasLink(String),
    AliasDefinition(String),
    TagLink(String),
    Url(String),
    ImportanceMarker,
    Text(String),
}

impl Fragment {
    /// Return whether the fragment can be part of an outline header.
    ///
    /// The header is the initial lines of the outline that can only contain alias definitions and
    /// tag declarations.
    fn can_be_header(&self) -> bool {
        use Fragment::*;
        match self {
            TagLink(_) | AliasDefinition(_) => true,
            Text(ref s) => s.chars().all(|c| c.is_whitespace()),
            _ => false,
        }
    }
}

impl fmt::Display for Fragment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Fragment::*;
        match self {
            Verbatim(s) => write!(f, "`{}`", s),
            InlineImage(s) => write!(f, "![{}]", s),
            FileLink(s) | AliasLink(s) => write!(f, "[{}]", s),
            AliasDefinition(s) => write!(f, "({})", s),
            TagLink(s) => write!(f, "@{}", s),
            ImportanceMarker => write!(f, " *"),
            Url(s) | WikiWord(s) | Text(s) => write!(f, "{}", s),
        }
    }
}

fn fragment(input: &str) -> nom::IResult<&str, Fragment> {
    if eol(input).is_ok() {
        return nom_err(input);
    }
    if indent_block_start(input).is_ok() {
        return nom_err(input);
    }
    if let Ok(ret) = terminable_fragment(input) {
        return Ok(ret);
    }

    let mut pos = 0;
    while let Some(c) = &input[pos..].chars().next() {
        pos += c.len_utf8();
        if eol(&input[pos..]).is_ok() {
            break;
        }
        if indent_block_start(&input[pos..]).is_ok() {
            break;
        }
        if terminable_fragment(&input[pos..]).is_ok() {
            break;
        }
    }

    if pos > 0 {
        Ok((&input[pos..], Fragment::Text(input[..pos].to_string())))
    } else {
        nom_err(input)
    }
}

// Fragments whose end can be determined using nom.
// Text is the "all the other stuff" fragment that is handled in the main framgent function.
fn terminable_fragment(i: &str) -> nom::IResult<&str, Fragment> {
    alt((
        map(wiki_word, |s| Fragment::WikiWord(s.to_string())),
        map(verbatim, |s| Fragment::Verbatim(s.to_string())),
        map(inline_image, |s| Fragment::InlineImage(s.to_string())),
        map(file_link, |s| Fragment::FileLink(s.to_string())),
        map(alias_link, |s| Fragment::AliasLink(s.to_string())),
        map(alias_definition, |s| {
            Fragment::AliasDefinition(s.to_string())
        }),
        map(tag_link, |s| Fragment::TagLink(s.to_string())),
        map(url, |s| Fragment::Url(s.to_string())),
        map(importance_marker, |_| Fragment::ImportanceMarker),
    ))(i)
}

fn wiki_word_segment(i: &str) -> nom::IResult<&str, &str> {
    recognize(pair(wiki_word_segment_head, wiki_word_segment_tail))(i)
}

fn wiki_word_segment_head(i: &str) -> nom::IResult<&str, char> {
    one_of("ABCDEFGHIJKLMNOPQRSTUVWXYZ")(i)
}

fn wiki_word_segment_tail(i: &str) -> nom::IResult<&str, &str> {
    take_while1(|c: char| c.is_lowercase() || c.is_numeric())(i)
}

fn wiki_word(i: &str) -> nom::IResult<&str, &str> {
    terminated(
        recognize(preceded(
            take_while(|c: char| c.is_numeric()), // Allow numbers at start
            pair(wiki_word_segment, many1(wiki_word_segment)),
        )),
        peek(not(wiki_word_segment_head)),
    )(i)
}

fn empty_line(i: &str) -> nom::IResult<&str, &str> {
    recognize(terminated(many0(one_of(" \t")), eol))(i)
}

fn depth(i: &str) -> nom::IResult<&str, usize> {
    map(take_while(|c| c == '\t'), |s: &str| s.len() + 1)(i)
}

fn complete_line(i: &str) -> nom::IResult<&str, &str> {
    terminated(take_while(|c| c != '\n' && c != '\r'), eol)(i)
}

fn verbatim(i: &str) -> nom::IResult<&str, &str> {
    delimited(tag("`"), take_while(|c| c != '`'), tag("`"))(i)
}

fn url(i: &str) -> nom::IResult<&str, &str> {
    recognize(pair(
        alt((tag("https://"), tag("http://"), tag("ftp://"))),
        take_while(is_url_char),
    ))(i)
}

fn inline_image(i: &str) -> nom::IResult<&str, &str> {
    delimited(tag("!["), take_while(is_path_char), tag("]"))(i)
}

fn file_link(i: &str) -> nom::IResult<&str, &str> {
    delimited(
        tag("["),
        recognize(pair(alt((tag("./"), tag("../"))), take_while(is_path_char))),
        tag("]"),
    )(i)
}

fn alias_definition(i: &str) -> nom::IResult<&str, &str> {
    delimited(tag("("), take_while(is_alias_char), tag(")"))(i)
}

fn alias_link(i: &str) -> nom::IResult<&str, &str> {
    delimited(tag("["), take_while(is_alias_char), tag("]"))(i)
}

fn tag_link(i: &str) -> nom::IResult<&str, &str> {
    preceded(tag("@"), take_while(is_tag_char))(i)
}

fn importance_marker(i: &str) -> nom::IResult<&str, &str> {
    terminated(tag(" *"), peek(eol))(i)
}

fn indent_block_with_syntax(i: &str) -> nom::IResult<&str, (&str, Option<&str>)> {
    map(
        pair(alt((tag("'''"), tag("```"))), complete_line),
        |(a, b)| (a, if b.is_empty() { None } else { Some(b) }),
    )(i)
}

// Any regular line ending with > or ; indicates subsequent lines with deeper indentation are an
// indent block.
fn indent_block_trail(i: &str) -> nom::IResult<&str, (&str, Option<&str>)> {
    map(terminated(alt((tag(">"), tag(";"))), eol), |p| (p, None))(i)
}

fn indent_block_start(i: &str) -> nom::IResult<&str, (&str, Option<&str>)> {
    alt((indent_block_with_syntax, indent_block_trail))(i)
}

fn syntax_line(i: &str) -> nom::IResult<&str, ((String, bool), Option<Digest>)> {
    pair(syntax_lang, opt(checksum))(i)
}

// Return ([lang name], [is library block]).
fn syntax_lang(i: &str) -> nom::IResult<&str, (String, bool)> {
    alt((lang_lib, lang_script))(i)
}

fn lang_lib(i: &str) -> nom::IResult<&str, (String, bool)> {
    map(
        terminated(take_while1(is_lang_char), tag("-lib")),
        |s: &str| (s.to_string(), true),
    )(i)
}

fn lang_script(i: &str) -> nom::IResult<&str, (String, bool)> {
    map(take_while1(is_lang_char), |s: &str| (s.to_string(), false))(i)
}

fn checksum(i: &str) -> nom::IResult<&str, Digest> {
    const LEN: usize = std::mem::size_of::<Digest>();
    map(
        preceded(
            pair(take_while1(|c: char| c.is_whitespace()), tag("md5:")),
            count(hex_byte, LEN),
        ),
        |bytes| {
            let mut array = [0; LEN];
            array.copy_from_slice(&bytes[..LEN]);
            Digest(array)
        },
    )(i)
}

fn hex_byte(i: &str) -> nom::IResult<&str, u8> {
    map_res(
        take_while_m_n(2, 2, |c: char| c.is_digit(16)),
        |hex: &str| u8::from_str_radix(&*hex, 16),
    )(i)
}

/// Match newline or EOF
fn eol(i: &str) -> nom::IResult<&str, &str> {
    if i.is_empty() {
        Ok((i, ""))
    } else {
        line_ending(i)
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

fn is_lang_char(c: char) -> bool {
    match c {
        '_' => true,
        c if c.is_alphanumeric() => true,
        _ => false,
    }
}

/// Helper function for the verbose Nom error expression
fn nom_err<I, T>(input: I) -> nom::IResult<I, T> {
    // XXX: ErrorKind variant is arbitrary.
    return Err(nom::Err::Error(nom::error::make_error(
        input,
        nom::error::ErrorKind::Tag,
    )));
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::{assert_eq, assert_ne};
    use ron;

    fn parse_test(input: &str, output: &str) {
        let outline: Outline = input.parse().unwrap();
        // Outline type must echo back verbatim (except maybe trailing whitespace)

        assert_eq!(&format!("{}", outline), input);

        // Check that the parse result matches the example serialization.
        assert_eq!(outline, ron::de::from_str(output).unwrap());
    }

    /// Slightly lossy format for small outlines for writing literals in tests
    fn flatten(outline: &Outline) -> Vec<String> {
        fn walk(o: &Outline, acc: &mut Vec<String>) {
            match &o.body {
                OutlineBody::Line(frags) => acc.extend(frags.iter().map(|f| format!("{}", f))),
                OutlineBody::Block {
                    indent_line: Some(frags),
                    lines,
                    ..
                } => {
                    acc.extend(frags.iter().map(|f| format!("{}", f)));
                    for line in lines {
                        acc.push(line.clone());
                    }
                }
                OutlineBody::Block { lines, .. } => {
                    for line in lines {
                        acc.push(line.clone());
                    }
                }
            }

            for c in &o.children {
                walk(c, acc);
            }
        }
        let mut ret = Vec::new();
        walk(outline, &mut ret);
        ret
    }

    #[test]
    fn test_components() {
        assert!(empty_line("").is_ok());
        assert!(empty_line(" ").is_ok());
        assert!(empty_line("\t").is_ok());
        assert!(empty_line("\t\t ").is_ok());
        assert!(empty_line("\n").is_ok());
        assert!(empty_line("\njunk").is_ok());

        assert!(importance_marker(" *").is_ok());
        assert!(importance_marker("").is_err());
        assert!(importance_marker(" * x").is_err());

        assert_eq!(complete_line(""), Ok(("", "")));
        assert_eq!(complete_line("\nbaz"), Ok(("baz", "")));
        assert_eq!(complete_line("foobar"), Ok(("", "foobar")));
        assert_eq!(complete_line("foobar\nbaz"), Ok(("baz", "foobar")));
        assert_eq!(complete_line("foobar\r\nbaz"), Ok(("baz", "foobar")));

        assert_eq!(
            indent_block_line(3, "\t\t\tcode"),
            Ok(("", "\tcode".to_string()))
        );
        assert!(indent_block_line(3, "\tcode").is_err());

        assert!(wiki_word("").is_err());
        assert!(wiki_word("word").is_err());
        assert!(wiki_word("Word").is_err());
        assert!(wiki_word("aWikiWord").is_err());
        assert!(wiki_word("WikiW").is_err());
        assert!(wiki_word("WikiWordW").is_err());
        assert_eq!(wiki_word("WikiWord"), Ok(("", "WikiWord")));
        assert_eq!(wiki_word("Wiki1Word2"), Ok(("", "Wiki1Word2")));
        assert_eq!(wiki_word("WikiWord-s"), Ok(("-s", "WikiWord")));
        assert_eq!(wiki_word("1984WikiWord"), Ok(("", "1984WikiWord")));
    }

    #[test]
    fn test_oneliner_outlines() {
        // Run tests with RUST_LOG="parser=TRACE" cargo test --all
        env_logger::init();

        let outline: Outline = "".parse().unwrap();
        assert_eq!(outline, Outline::default());

        let outline: Outline = "a".parse().unwrap();
        assert_eq!(flatten(&outline), vec!["a"]);

        let outline: Outline = "\ta".parse().unwrap();
        assert_eq!(flatten(&outline), vec!["a"]);

        let outline: Outline = "; x".parse().unwrap();
        assert_eq!(flatten(&outline), vec!["x"]);

        let outline: Outline = "[file.txt]".parse().unwrap();
        assert_eq!(flatten(&outline), vec!["[file.txt]"]);

        let outline: Outline = "![image.jpg]".parse().unwrap();
        assert_eq!(flatten(&outline), vec!["![image.jpg]"]);

        let outline: Outline = "trail-indent>".parse().unwrap();
        assert_eq!(flatten(&outline), vec!["trail-indent"]);

        let outline: Outline = " space-indent>".parse().unwrap();
        assert_eq!(flatten(&outline), vec!["space-indent>"]);
    }

    #[test]
    fn test_full_outline() {
        parse_test(
            include_str!("../test/test.otl"),
            include_str!("../test/test.ron"),
        );
        parse_test(
            include_str!("../test/otlbook.otl"),
            include_str!("../test/otlbook.ron"),
        );
    }

    #[test]
    fn test_aliases() {
        let outline: Outline = "(Alias) (A2)\nSeparator\n(A3)".parse().unwrap();
        assert_eq!(
            outline.aliases().cloned().collect::<Vec<String>>(),
            vec!["Alias", "A2"]
        );
    }

    #[test]
    fn test_tags() {
        let outline: Outline = "@tag1 @tag2\nSeparator\n@tag3".parse().unwrap();
        assert_eq!(
            outline.tags().cloned().collect::<Vec<String>>(),
            vec!["tag1", "tag2"]
        );
    }

    #[test]
    fn test_syntax_info() {
        assert_eq!(syntax_lang("julia"), Ok(("", ("julia".to_string(), false))));
        assert_eq!(
            syntax_lang("julia-lib"),
            Ok(("", ("julia".to_string(), true)))
        );

        assert_eq!(
            SyntaxInfo::new(""),
            SyntaxInfo {
                ..Default::default()
            }
        );

        assert_eq!(
            SyntaxInfo::new("julia"),
            SyntaxInfo {
                lang: Some("julia".to_string()),
                ..Default::default()
            }
        );

        assert_eq!(
            SyntaxInfo::new("julia trailing garbage"),
            SyntaxInfo {
                lang: Some("julia".to_string()),
                ..Default::default()
            }
        );

        assert_eq!(
            SyntaxInfo::new("julia-lib"),
            SyntaxInfo {
                lang: Some("julia".to_string()),
                is_lib: true,
                ..Default::default()
            }
        );

        assert_eq!(
            SyntaxInfo::new("julia md5:4f41243847da693a4f356c0486114bc6"),
            SyntaxInfo {
                lang: Some("julia".to_string()),
                checksum: Some(Digest([
                    0x4f, 0x41, 0x24, 0x38, 0x47, 0xda, 0x69, 0x3a, 0x4f, 0x35, 0x6c, 0x04, 0x86,
                    0x11, 0x4b, 0xc6
                ])),
                ..Default::default()
            }
        );
    }
}
