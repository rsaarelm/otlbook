use parser::{self, Outline, OutlineBody, TagAddress, outline};
use std::collections::BTreeSet;
use std::convert::TryFrom;
use std::fmt;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use structopt::{self, StructOpt};
use walkdir::{DirEntry, WalkDir};

mod eval;
use eval::eval;

mod scrape;

fn main() {
    let opt = Otltool::from_args();
    match opt {
        Otltool::Echo { debug } => echo(debug),
        Otltool::Tags => tags(),
        Otltool::Eval { force } => eval(force),
        Otltool::Extract { syntax } => extract(&syntax),
        Otltool::Save { target } => save(&target),
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

    #[structopt(name = "eval", about = "Evaluate script blocks piped through stdin")]
    Eval {
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

    #[structopt(
        name = "extract",
        about = "Extract deindented fragments of specific syntax from the outline"
    )]
    Extract {
        #[structopt(parse(from_str))]
        syntax: String,
    },

    #[structopt(name = "save", about = "Save target into bookmarks")]
    Save {
        #[structopt(parse(from_str))]
        target: String,
    },
}

fn echo(debug: bool) {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf).unwrap();
    let outline: Outline = buf.parse().unwrap();

    if debug {
        println!(
            "{}",
            ron::ser::to_string_pretty(&outline, Default::default()).unwrap()
        );
    } else {
        print!("{}", outline);
    }
}

//////////////////////////////// Tag generation

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
        tags.tags.extend(outline.ctags(0, path));
    }

    println!("{}", tags);
}

//////////////////////////////// Code block extraction

fn extract(syntax: &str) {
    fn echo_blocks(syntax: &str, outline: &Outline) {
        match outline.body() {
            OutlineBody::Block {
                syntax: Some(s),
                lines,
                ..
            } => {
                if s.split_whitespace().next() == Some(syntax) {
                    for line in lines {
                        println!("{}", line);
                    }
                }
            }
            _ => {}
        }

        for i in outline.children() {
            echo_blocks(syntax, i);
        }
    }

    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf).unwrap();
    let outline: Outline = buf.parse().unwrap();

    echo_blocks(syntax, &outline);
}

//////////////////////////////// Scraping

trait KnowledgeBase {
    fn uris(&self) -> Vec<String>;
}

impl KnowledgeBase for outline::Outline {
    fn uris(&self) -> Vec<String> {
        fn crawl_for_uris(acc: &mut Vec<String>, outline: &outline::Outline) {
            let mut metadata = outline.metadata();
            if let Some(uri) = metadata.remove("uri") {
                acc.push(uri);
            }
            for o in &outline.children {
                crawl_for_uris(acc, o);
            }
        }

        let mut ret = Vec::new();
        crawl_for_uris(&mut ret, self);
        ret
    }
}

pub fn save(target: &str) {
    // TODO: Save to Bookmarks.otl
    // let path = path_or_die();

    let db = load_database_or_die();
    let uris = db.uris();

    if uris.iter().any(|t| t.as_ref() as &str == target) {
        // TODO: Could tell where it's saved?
        println!("URI is already saved in your notes");
        return;
    }

    scrape::check_wayback(target);

    scrape::scrape(target);

    // TODO: Create an entry and save it.
}

//////////////////////////////// System utilities

/// Find .otl files under a path.
fn otl_paths(root: impl AsRef<Path>) -> impl Iterator<Item = PathBuf> {
    fn is_otl(entry: &DirEntry) -> bool {
        entry.file_type().is_file()
            && entry
                .file_name()
                .to_str()
                .map_or(false, |s| s.ends_with(".otl"))
    }

    WalkDir::new(root)
        .follow_links(true)
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

/// Look for default otlbook path from OTLBOOK_PATH environment variable.
fn otlbook_path() -> Option<PathBuf> {
    let path = std::env::var("OTLBOOK_PATH").ok()?;
    Some(path.into())
}

fn path_or_die() -> PathBuf {
    match otlbook_path() {
        Some(path) => path,
        None => {
            println!("Please define your .otl file directory in environment variable OTLBOOK_PATH");
            std::process::exit(1);
        }
    }
}

fn load_database_or_die() -> outline::Outline {
    let path = path_or_die();
    let outline: outline::Outline =
        TryFrom::try_from(path.as_ref() as &Path).expect("Couldn't read OTLBOOK_PATH");
    if outline.is_empty() {
        println!("No outline files found in OTLBOOK_PATH");
        std::process::exit(1);
    }

    outline
}
