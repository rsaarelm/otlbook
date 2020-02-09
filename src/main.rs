/*!
Tool for working with VimOutliner files plus some conventions to embed extra stuff in them.

## The outline data format

Rust data structures can be embedded in outlines in a format somewhat inspired by
[indentation-sensitive Scheme syntax](https://srfi.schemers.org/srfi-49/srfi-49.html). Whitespace
and newline serve as separators and indentation serves as grouping.

Outline notes can have deserializable metadata embedded in them by having a double-indented block
right below the title line

Outline files must be indented with physical tabs. The examples below assume a visual tab width of
2 characters. (The examples in this file are indented with spaces because rustfmt does not like
physical tabs in Rust source files.)

```notrust
#[derive(Default)]
struct ArticleData {
    uri: Uri,
    title: String,
    tags: Vec<Tag>,
    year: Option<Year>,
}:

NoteArticle
    uri http://example.com/
    title Human readable
    tags foo bar
  Actual notes lines
  Go here
```

The deserialization format is not self-describing. The deserializer always operates based on the
type it's deserializing into, and deserialization behavior changes based on the type.

Primitive types like numbers and booleans are serialized the way they are printed and parsed by the
Rust library.

Strings are not quoted. String literals without newlines are inline and end at the end of the line.
String literals with newlines are represented by indented blocks that end when the indentation goes
back over the starting level:

```notrust
title Lorem ipsum
body
  Lorem ipsum dolor sit amet,
  consectetur adipiscing elit,
```

Lists can be represented as inline sequences or vertical lists of lines.

```notrust
Vec<i32>:

1 2 3

  1
  2
  3
```

An inline string list is a special case where the string elements are separated by whitespace. This
means that a list of strings can be inline only if none of the strings have any whitespace in them.
This makes it possible to do things like the inline tag list in the `NoteArticle` example above.
Strings with whitespace in them must be listed vertically.

```notrust
Vec<String>:

foo bar baz

  foo
  bar
  not baz
```

Structs and maps must have whitespace-less keys. The key is parsed in inline mode, then the value
is parsed in regular mode. If struct values do not contain lists or strings with whitespace, the
entire struct can be written inline.

```notrust
struct { x: i32, y: i32, z: i32 }:

x 4 y 10 z 20

  x 4
  y 10
  z 20
```

There is one piece of special syntax, the comma (`,`). There's no other way to separate elements in
a list of indented blocks than dropping to a lower level of indentation and typing a non-whitespace
character. To have an actual lone comma in a vertical string list, type two commas `,,` (and for a
double comma, type three and so on, any run of nothing but commas will get one subtracted from
it).

```notrust
Vec<String>:

    Lorem ipsum dolor sit amet,
    consectetur adipiscing elit,
  ,
    sed do eiusmod tempor incididunt
    ut labore et dolore magna aliqua.
```

The first item can be optionally preceded by comma to make things more consistent for procedural
generators.

### Outline data tricks

To make an "option flags" type where just having the field present in the struct literal part of
the outline data, use `Option<()>` as the value type. The unit type gets deserialized without any
input parsed, and the fields will default to `None` if not present.

```notrust
#[derive(Default)]
struct Options {
    frobnicate: Option<()>,
    despecle: Option<()>,
    tesselate: Option<()>,
}:

  despecle

=>

Options { frobnicate: None, despecle: Some(()), tesselate: None }
```
*/

use parser::{self, outline, Outline, OutlineBody, TagAddress};
use std::collections::BTreeSet;
use std::convert::TryFrom;
use std::fmt;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use structopt::{self, StructOpt};
use walkdir::{DirEntry, WalkDir};

mod eval;
use eval::eval;

fn main() {
    env_logger::init();

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
        _dump: bool,
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

    scraper::check_wayback(target);

    match scraper::scrape(target) {
        Ok(_ret) => {
            println!("TODO scrape okay");
        }
        Err(err) => {
            println!("Scrape error {}", err);
        }
    }

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
