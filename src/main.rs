use parser::{Lexer, Token};
use std::collections::BTreeMap;
use std::env;
use std::fmt;
use std::io::{self, Read};
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

    fn from_str(s: &str) -> Result<Self, Self::Err> {}
}

struct Outline {
    children: Vec<Outline>,
    aliases: Vec<String>,
    tags: Vec<String>,
    body: OutlineObject,
}

impl Outline {
    fn parse<T: Deref<Target = str>>(
        depth: usize,
        tokens: &[Token<T>],
    ) -> (&[Token<T>], Result<Outline, ()>) {
        for (i, t) in tokens.iter().enumerate() {
            match t {}
        }
        unimplemented!();
    }
}

enum TagAddress {
    LineNum(usize),
    Search(String),
}

impl TagAddress {
    fn start_of_file() -> TagAddress {
        TagAddress::LineNum(0)
    }

    fn otl_search(tagname: &str) -> TagAddress {
        TagAddress::Search(format!(r"^\t\*{}$", tagname))
    }
}

impl fmt::Display for TagAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TagAddress::LineNum(n) => write!(f, "{}", n),
            TagAddress::Search(expr) => write!(f, "/{}/", expr),
        }
    }
}

struct CTags {
    // Include depth in key so that tags deeper in the outline are give a lower priority in case
    // there are multiple instances of the same tag name. Want the higher-up version to be more
    // authoritative.
    tags: BTreeMap<(String, usize), (String, TagAddress)>,
}

impl CTags {
    fn insert_tag(&mut self, tag_name: &str, depth: usize, target_name: &str, path: &str) {
        let key = (tag_name.to_string(), depth);
        let addr = if depth == 0 {
            TagAddress::start_of_file()
        } else {
            TagAddress::otl_search(target_name)
        };

        self.tags.insert(key, (path.to_string(), addr));
    }
}

impl fmt::Display for CTags {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for ((tag, _), (path, addr)) in &self.tags {
            writeln!(f, "{}\t{}\t{}", tag, path, addr)?;
        }
        Ok(())
    }
}

fn tags() {
    otl_paths(env::current_dir().expect("Invalid working directory"));
}

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
