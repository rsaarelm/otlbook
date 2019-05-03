use nom::types::CompleteStr;
use nom::{
    self, alt, count, delimited, do_parse, eof, line_ending, many0, many1, map, named, not, one_of,
    opt, pair, peek, preceded, recognize, tag, take_while, take_while1, terminated, verify,
};
use nom::{Context, ErrorKind};
use serde::{Deserialize, Serialize};
use std::default;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;
use std::str::FromStr;

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
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
    pub fn load(path: impl AsRef<Path>) -> Result<Outline, Box<Error>> {
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
        ret.body = (outline_body(0, CompleteStr(&*basename)).unwrap().1).1;
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
        match outline(0, CompleteStr(s)) {
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

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
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

/// Parse text into an outline
fn outline(depth: usize, input: CompleteStr<'_>) -> nom::IResult<CompleteStr<'_>, Outline> {
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

fn outline_body(
    min_indent: usize,
    input: CompleteStr<'_>,
) -> nom::IResult<CompleteStr<'_>, (usize, OutlineBody)> {
    let (line_start, d) = depth(input).unwrap();
    if d < min_indent {
        return nom_err(input);
    }

    if let Ok((rest, _)) = empty_line(input) {
        return Ok((rest, (min_indent, OutlineBody::default())));
    }

    if let Ok(ret) = prefix_block(min_indent, input) {
        return Ok(ret);
    }

    let mut line = Vec::new();
    let mut current = line_start;
    loop {
        if let Ok((rest, _)) = alt!(current, line_ending | eof!()) {
            return Ok((rest, (d, OutlineBody::Line(line))));
        }

        if let Ok((rest, (prefix, syntax))) = indent_block_start(current) {
            let (rest, lines) = indent_block_lines(d + 1, rest).unwrap_or((rest, Vec::new()));
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
            line.push(f);
            current = rest;
        } else {
            break;
        }
    }
    Ok((current, (d, OutlineBody::Line(line))))
}

// Return indent and body on match.
fn prefix_block(
    min_indent: usize,
    input: CompleteStr,
) -> nom::IResult<CompleteStr, (usize, OutlineBody)> {
    fn prefix_block_line<'a>(
        indent: usize,
        prefix: &str,
        input: CompleteStr<'a>,
    ) -> nom::IResult<CompleteStr<'a>, CompleteStr<'a>> {
        debug_assert!(indent > 0);

        // Allow empty lines in space-prefix blocks that don't need to have even the indent.
        if prefix.is_empty() {
            if let Ok((rest, _)) = empty_line(input) {
                return Ok((rest, CompleteStr("")));
            }
        }

        // Allow non-empty prefix to be separated from text with tab as well as space, to avoid the
        // more confusing alternative of parsing a non-empty prefix + tab as a syntax line.
        do_parse!(
            input,
            count!(tag!("\t"), indent - 1)
                >> terminated!(
                    tag!(prefix),
                    one_of!(if prefix.is_empty() { " " } else { " \t" })
                )
                >> body: complete_line
                >> (body)
        )
    }

    fn prefix_block_syntax_line<'a>(
        indent: usize,
        prefix: &str,
        input: CompleteStr<'a>,
    ) -> nom::IResult<CompleteStr<'a>, CompleteStr<'a>> {
        debug_assert!(indent > 0);
        if prefix.is_empty() {
            return nom_err(input);
        }

        do_parse!(
            input,
            count!(tag!("\t"), indent - 1)
                >> tag!(prefix)
                // XXX: Ugly, using recognize! on complete_line seems to pull in the trailing
                // newline.
                >> body: verify!(complete_line, |s: CompleteStr| match s.chars().next() {
                    Some(' ') | Some('\t') | None => false,
                    _ => true })
                >> (body)
        )
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

    let (rest, (syntax, lines)) = pair!(
        input,
        opt!(|i| prefix_block_syntax_line(indent, &prefix, i)),
        many0!(|i| prefix_block_line(indent, &prefix, i))
    )?;

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

fn child_outlines(
    min_indent: usize,
    input: CompleteStr<'_>,
) -> nom::IResult<CompleteStr<'_>, Vec<Outline>> {
    let outline = |i| outline(min_indent, i);

    many0!(input, outline)
}

/// Parse text block delimited only by sufficient indentation.
fn indent_block_lines(
    min_indent: usize,
    input: CompleteStr,
) -> nom::IResult<CompleteStr, Vec<String>> {
    many0!(input, |i| indent_block_line(min_indent, i))
}

fn indent_block_line(
    expected_indent: usize,
    input: CompleteStr,
) -> nom::IResult<CompleteStr, String> {
    alt!(
        input,
        map!(empty_line, |_| String::new())
            | map!(
                preceded!(count!(tag!("\t"), expected_indent - 1), complete_line),
                |s| s.to_string()
            )
    )
}

/// Line element fragments.
#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
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

fn fragment(input: CompleteStr) -> nom::IResult<CompleteStr, Fragment> {
    if alt!(input, line_ending | eof!()).is_ok() {
        return nom_err(input);
    }
    if indent_block_start(input).is_ok() {
        return nom_err(input);
    }
    if let Ok(ret) = terminable_fragment(input) {
        return Ok(ret);
    }

    let mut pos = 0;
    while let Some(next_p) = &input[pos..].char_indices().skip(1).next().map(|(i, _)| i) {
        debug_assert!(*next_p > 0);
        pos += *next_p;
        if terminable_fragment(CompleteStr(&input[pos..])).is_ok() {
            break;
        }
        if indent_block_start(CompleteStr(&input[pos..])).is_ok() {
            break;
        }
        if alt!(&input[pos..], line_ending | eof!()).is_ok() {
            break;
        }
    }

    if pos > 0 {
        Ok((
            CompleteStr(&input[pos..]),
            Fragment::Text(input[..pos].to_string()),
        ))
    } else {
        nom_err(input)
    }
}

// Fragments whose end can be determined using nom.
// Text is the "all the other stuff" fragment that is handled in the main framgent function.
named!(terminable_fragment<CompleteStr, Fragment>,
    alt!(map!(wiki_word, |s| Fragment::WikiWord(s.to_string()))
       | map!(verbatim, |s| Fragment::Verbatim(s.to_string()))
       | map!(inline_image, |s| Fragment::InlineImage(s.to_string()))
       | map!(file_link, |s| Fragment::FileLink(s.to_string()))
       | map!(alias_link, |s| Fragment::AliasLink(s.to_string()))
       | map!(alias_definition, |s| Fragment::AliasDefinition(s.to_string()))
       | map!(tag_link, |s| Fragment::TagLink(s.to_string()))
       | map!(url, |s| Fragment::Url(s.to_string()))
       | map!(importance_marker, |_| Fragment::ImportanceMarker)
    ));

named!(wiki_word_segment<CompleteStr, CompleteStr>,
    recognize!(pair!(wiki_word_segment_head, wiki_word_segment_tail)));

named!(wiki_word_segment_head<CompleteStr, char>,
    // XXX: Is there a nice concise way to get |c| c.is_uppercase() here instead?
    one_of!("ABCDEFGHIJKLMNOPQRSTUVWXYZ"));

named!(wiki_word_segment_tail<CompleteStr, CompleteStr>,
    take_while1!(|c: char| c.is_lowercase() || c.is_numeric()));

named!(wiki_word<CompleteStr, CompleteStr>,
    terminated!(
        recognize!(
            preceded!(take_while!(|c: char| c.is_numeric()),
            pair!(wiki_word_segment, many1!(wiki_word_segment)))
        ),
        peek!(not!(wiki_word_segment_head))));

named!(empty_line<CompleteStr, CompleteStr>,
    terminated!(recognize!(many0!(one_of!(" \t"))), alt!(line_ending | eof!())));

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

named!(alias_definition<CompleteStr, CompleteStr>,
    delimited!(tag!("("), take_while!(is_alias_char), tag!(")")));

named!(alias_link<CompleteStr, CompleteStr>,
    delimited!(tag!("["), take_while!(is_alias_char), tag!("]")));

named!(tag_link<CompleteStr, CompleteStr>,
    preceded!(tag!("@"), take_while!(is_tag_char)));

named!(importance_marker<CompleteStr, CompleteStr>,
    terminated!(tag!(" *"), peek!(alt!(line_ending | eof!()))));

named!(indent_block_with_syntax<CompleteStr, (CompleteStr, Option<CompleteStr>)>,
    map!(pair!(alt!(tag!("'''") | tag!("```")), complete_line), |(a, b)| (a, if b.is_empty() { None } else { Some(b) })));

named!(indent_block_trail<CompleteStr, (CompleteStr, Option<CompleteStr>)>,
    map!(terminated!(alt!(tag!(">") | tag!(";")), alt!(line_ending | eof!())), |p| (p, None)));

named!(indent_block_start<CompleteStr, (CompleteStr, Option<CompleteStr>)>,
    alt!(indent_block_with_syntax | indent_block_trail));

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

/// Helper function for the verbose Nom error expression
fn nom_err<I, T>(input: I) -> nom::IResult<I, T> {
    return Err(nom::Err::Error(Context::Code(input, ErrorKind::Custom(1))));
}

#[cfg(test)]
mod tests {
    use super::*;
    use nom::types::CompleteStr as S;
    use ron;

    fn parse_test(input: &str, output: &str) {
        let outline: Outline = input.parse().unwrap();
        // Outline type must echo back verbatim (except maybe trailing whitespace)

        println!(
            "{}",
            ron::ser::to_string_pretty(&outline, Default::default()).unwrap()
        );

        // Provide more eyeballable outputs in case the test fails.
        println!("{}", input); // This is what you want
        println!("{}", outline); // This is what you get

        assert_eq!(&format!("{}", outline), input);

        // Check that the parse result matches the example serialization.
        assert_eq!(outline, ron::de::from_str(output).unwrap());
    }

    #[test]
    fn test_components() {
        assert!(empty_line(S("")).is_ok());
        assert!(empty_line(S(" ")).is_ok());
        assert!(empty_line(S("\t")).is_ok());
        assert!(empty_line(S("\t\t ")).is_ok());
        assert!(empty_line(S("\n")).is_ok());
        assert!(empty_line(S("\njunk")).is_ok());

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
            Ok((S(""), "\tcode".to_string()))
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
        assert_eq!(wiki_word(S("1984WikiWord")), Ok((S(""), S("1984WikiWord"))));
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
}
