use parser::{self, Lexer, Token};
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use structopt::{self, StructOpt};
use walkdir::{DirEntry, WalkDir};

fn main() {
    let opt = Otltool::from_args();
    match opt {
        Otltool::Echo { debug } => echo(debug),
        Otltool::Tags => tags(),
        _ => unimplemented!(),
    }
}

#[derive(StructOpt)]
#[structopt(name = "otltool", about = "Outline file processing tool")]
enum Otltool {
    #[structopt(name = "echo", about = "Test by parsing and echoing stdin input")]
    Echo {
        #[structopt(long = "debug", help = "Print debug versions of tokens")]
        debug: bool,
    },

    #[structopt(name = "tags", about = "Generate ctags file from local .otl files")]
    Tags,

    #[structopt(name = "jeval", about = "Pipe stdin outline through J evaluator")]
    JEval {
        #[structopt(
            long = "force",
            help = "Ignore cached checksums, re-evaluate everything"
        )]
        force: bool,
    },

    #[structopt(
        name = "anki",
        about = "Extract and upload Anki cards from local .otl files"
    )]
    Anki {
        #[structopt(
            long = "dump",
            help = "Print tab-separated plaintext export instead of uploading to Anki"
        )]
        dump: bool,
    },

    #[structopt(
        name = "server",
        about = "Run a web server displaying outlines in HTML"
    )]
    Server,
}

fn echo(debug: bool) {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf).unwrap();

    for tok in Lexer::new(&buf) {
        use Token::*;
        if debug {
            match tok {
                StartLine(n)
                | BlockLine { depth: n, .. }
                | StartPrefixBlock { depth: n, .. }
                | StartPrefixBlock2 { depth: n, .. } => {
                    for _ in 1..n {
                        print!(" ");
                    }
                }
                _ => {}
            }
            print!("{:?}", tok);
            match tok {
                NewLine | EndBlock(_) => println!(),
                _ => {}
            }
        } else {
            print!("{}", tok);
        }
    }
}

//////////////////////////////// Outline type

#[derive(Debug)]
enum OutlineObject {
    WikiTitle(String),
    Line(Vec<Token<String>>),
    Block {
        syntax: Option<String>,
        is_preformatted: bool,
        body: String,
    },
}

impl FromStr for Outline {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let tokens: Vec<Token<String>> = Lexer::new(s)
            .map(|t| t.map(|s: &str| s.to_string()))
            .collect();

        let input = tokens.as_slice();
        Ok(Outline::parse(0, input).unwrap().1)
    }
}

#[derive(Debug)]
struct Outline {
    depth: usize,
    body: OutlineObject,
    children: Vec<Outline>,
}

impl Outline {
    /// Load the outline from file path.
    ///
    /// The outline will get a toplevel name derived from the file name.
    fn load(path: impl AsRef<Path>) -> Result<Outline, Box<Error>> {
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
        ret.body = OutlineObject::WikiTitle(basename);
        Ok(ret)
    }

    /// Construct the object for a part of an outline from token stream.
    fn parse<T: Deref<Target = str> + Clone>(
        depth: usize,
        input: &[Token<T>],
    ) -> Result<(&[Token<T>], Outline), &[Token<T>]> {
        let mut input = input;

        let body = if depth == 0 {
            OutlineObject::Line(Vec::new())
        } else {
            let (rest, body) = match Outline::parse_body(depth, input) {
                Ok(pair) => pair,
                Err(_) => return Err(input),
            };
            input = rest;
            body
        };

        let mut children = Vec::new();
        while let Ok((rest, child)) = Outline::parse(depth + 1, input) {
            children.push(child);
            input = rest;
        }

        debug_assert!(
            depth > 0 || input.is_empty(),
            "Parsing an entire file, but there still remains unparsed input"
        );

        Ok((
            input,
            Outline {
                depth,
                body,
                children,
            },
        ))
    }

    /// Construct body of an outline object from token stream.
    fn parse_body<T: Deref<Target = str> + Clone>(
        depth: usize,
        input: &[Token<T>],
    ) -> Result<(&[Token<T>], OutlineObject), &[Token<T>]> {
        use Token::*;

        if input.is_empty() {
            return Err(input);
        }

        if let [StartPrefixBlock {
            depth: d,
            prefix,
            syntax,
        }, NewLine] = &input[..2]
        {
            if *d >= depth {
                let mut body = String::new();
                let rest =
                    match Outline::parse_block_lines(&mut body, depth, Some(prefix), &input[2..]) {
                        Ok((rest, _)) => rest,
                        Err(rest) => rest,
                    };
                return Ok((
                    rest,
                    OutlineObject::Block {
                        syntax: Some(syntax.to_string()),
                        is_preformatted: parser::is_preformatted_block(prefix),
                        body,
                    },
                ));
            }
        } else if let [StartPrefixBlock2 {
            depth: d,
            prefix,
            first_line,
        }, NewLine] = &input[..2]
        {
            if *d >= depth {
                let mut body = String::new();
                body.push_str(first_line);
                body.push_str("\n");
                let rest =
                    match Outline::parse_block_lines(&mut body, depth, Some(prefix), &input[2..]) {
                        Ok((rest, _)) => rest,
                        Err(rest) => rest,
                    };

                return Ok((
                    rest,
                    OutlineObject::Block {
                        syntax: None,
                        is_preformatted: parser::is_preformatted_block(prefix),
                        body,
                    },
                ));
            }
        } else if let [StartIndentBlock { prefix, syntax }, NewLine] = &input[..2] {
            let mut body = String::new();
            // We're parsing this as a child of the line the block prefix is on, so depth is
            // already at +1 compared to the prefix line and we don't need to indent it further.
            let rest = match Outline::parse_block_lines(&mut body, depth, None, &input[2..]) {
                Ok((rest, _)) => rest,
                Err(rest) => rest,
            };

            return Ok((
                rest,
                OutlineObject::Block {
                    syntax: Some(syntax.to_string()),
                    is_preformatted: parser::is_preformatted_block(prefix),
                    body,
                },
            ));
        }

        if let [StartLine(d), WikiTitle(s), NewLine] = &input[..3] {
            if *d >= depth {
                return Ok((&input[3..], OutlineObject::WikiTitle(s.to_string())));
            }
        }

        if let &StartLine(d) = &input[0] {
            if d >= depth {
                return Outline::parse_line_body(&input[1..]);
            }
        }

        return Err(input);
    }

    fn parse_line_body<T: Deref<Target = str> + Clone>(
        input: &[Token<T>],
    ) -> Result<(&[Token<T>], OutlineObject), &[Token<T>]> {
        use Token::*;
        let mut i = input;
        let mut parts = Vec::new();

        loop {
            if i.is_empty() {
                break;
            }
            match &i[0] {
                NewLine => {
                    i = &i[1..];
                    break;
                }
                // StartIndentBlock is treated as the next parse unit.
                StartIndentBlock { .. } => break,

                // Line must end with NewLine or StartIndentBlock
                StartPrefixBlock { .. }
                | StartPrefixBlock2 { .. }
                | BlockLine { .. }
                | StartLine(_) => {
                    debug_assert!(false, "Malformed token stream");
                    break;
                }
                tok => {
                    parts.push(tok.clone().map(|s| s.to_string()));
                    i = &i[1..];
                }
            }
        }

        Ok((i, OutlineObject::Line(parts)))
    }

    fn parse_block_lines<'a, T: Deref<Target = str>>(
        buf: &mut String,
        depth: usize,
        prefix: Option<&str>,
        input: &'a [Token<T>],
    ) -> Result<(&'a [Token<T>], ()), &'a [Token<T>]> {
        use Token::*;
        let mut i = input;
        let mut saw_input = !buf.is_empty();
        loop {
            if i.is_empty() {
                break;
            }
            if let [BlockLine {
                depth: d,
                prefix: p,
                text,
            }, NewLine] = &i[..2]
            {
                if *d == depth && p.as_ref().map(|s| s.deref()) == prefix {
                    buf.push_str(text);
                    buf.push_str("\n");
                    i = &i[2..];
                    saw_input = true;
                }
            } else if let EndBlock(_) = i[0] {
                saw_input = true;
                i = &i[1..];
                break;
            } else {
                // If we hit BlockLines, should've ended with EndBlock.
                debug_assert!(!saw_input);
                break;
            }
        }

        if saw_input {
            Ok((i, ()))
        } else {
            Err(input)
        }
    }

    fn tokens(&self) -> &[Token<String>] {
        match self.body {
            OutlineObject::Line(ref ts) => &ts,
            _ => &[],
        }
    }

    fn aliases(&self) -> impl Iterator<Item = &str> {
        self.children.iter().flat_map(|c| {
            c.tokens().into_iter().filter_map(|t| {
                if let Token::AliasDefinition(s) = t {
                    Some(s.as_str())
                } else {
                    None
                }
            })
        })
    }

    fn tag_definitions(&self) -> impl Iterator<Item = &str> {
        self.children.iter().flat_map(|c| {
            c.tokens().into_iter().filter_map(|t| {
                if let Token::TagDefinition(s) = t {
                    Some(s.as_str())
                } else {
                    None
                }
            })
        })
    }

    /// Return title if outline is toplevel of a wiki article.
    fn wiki_title(&self) -> Option<&str> {
        match self.body {
            OutlineObject::WikiTitle(ref s) => Some(s.as_str()),
            _ => None,
        }
    }

    fn ctags(&self, path: &str) -> impl Iterator<Item = (String, usize, String, TagAddress)> {
        let child_tags: Vec<(String, usize, String, TagAddress)> =
            self.children.iter().flat_map(|c| c.ctags(path)).collect();
        let mut tags = Vec::new();
        if let Some(title) = self.wiki_title() {
            let addr = if self.depth == 0 {
                TagAddress::LineNum(0)
            } else {
                TagAddress::Search(title.to_string())
            };

            tags.push((
                title.to_string(),
                self.depth,
                path.to_string(),
                addr.clone(),
            ));
            for a in self.aliases() {
                tags.push((a.to_string(), self.depth, path.to_string(), addr.clone()));
            }
        }

        tags.into_iter().chain(child_tags.into_iter())
    }
}

//////////////////////////////// Tag generation

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
enum TagAddress {
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

#[derive(Default)]
struct CTags {
    // Include depth in key so that tags deeper in the outline are give a lower priority in case
    // there are multiple instances of the same tag name. Want the higher-up version to be more
    // authoritative.
    tags: BTreeSet<(String, usize, String, TagAddress)>,
}

impl fmt::Display for CTags {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (tag, _, path, addr) in &self.tags {
            writeln!(f, "{}\t{}\t{}", tag, path, addr)?;
        }
        Ok(())
    }
}

fn tags() {
    let mut tags = CTags::default();

    for path in otl_paths("./") {
        let path = path.strip_prefix("./").unwrap().to_str().unwrap();
        let outline = Outline::load(path).unwrap();
        tags.tags.extend(outline.ctags(path));
    }

    println!("{}", tags);
}

//////////////////////////////// Filesystem tools

fn otl_paths(root: impl AsRef<Path>) -> impl Iterator<Item = PathBuf> {
    fn is_otl(entry: &DirEntry) -> bool {
        entry.file_type().is_file()
            && entry
                .file_name()
                .to_str()
                .map_or(false, |s| s.ends_with(".otl"))
    }

    WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            if is_otl(&e) {
                Some(e.into_path())
            } else {
                None
            }
        })
}
