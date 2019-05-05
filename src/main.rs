use parser::{self, Outline, OutlineBody, SyntaxInfo, TagAddress};
use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;
use structopt::{self, StructOpt};
use walkdir::{DirEntry, WalkDir};

fn main() {
    let opt = Otltool::from_args();
    match opt {
        Otltool::Echo { debug } => echo(debug),
        Otltool::Tags => tags(),
        Otltool::Eval { force } => eval(force),
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

//////////////////////////////// Notebook evaluator

/// Accumulated notebook evaluator state built when traversing the outline.
#[derive(Default)]
struct EvalState {
    /// Concatenate "foo-lib" blocks into `libraries["foo"]`.
    libraries: HashMap<String, String>,
}

impl EvalState {
    /// Concatenate library blocks into library code of the correct language.
    fn accumulate_libraries(&mut self, item: &Outline) {
        if let OutlineBody::Block {
            syntax: Some(s),
            lines,
            ..
        } = item.body()
        {
            if let SyntaxInfo {
                lang: Some(lang),
                is_lib: true,
                ..
            } = SyntaxInfo::new(s)
            {
                let code = self.libraries.entry(lang).or_insert(String::new());
                for line in lines {
                    code.push_str(line);
                    code.push('\n');
                }
            }
        }
    }

    pub fn process_outline(&mut self, outline: &mut Outline) {
        self.accumulate_libraries(outline);

        let mut body: OutlineBody = outline.body().clone();

        if let OutlineBody::Block {
            syntax: Some(ref s),
            ref mut lines,
            ..
        } = body
        {
            if let SyntaxInfo {
                lang: Some(lang),
                is_lib: false,
                checksum,
            } = SyntaxInfo::new(&s)
            {
                if lang == "julia" {
                    // Library code
                    let mut code = self.libraries.entry(lang).or_insert(String::new()).clone();
                    // Print a separator marker to denote the end of script code. We'll use this
                    // later to throw out REPL output from script code.
                    code.push_str("println('\\u241E')\n");

                    let mut script_code = String::new();
                    let mut old_output = String::new();
                    for line in lines.iter() {
                        let target = if line.ends_with("\u{00A0}") {
                            &mut old_output
                        } else {
                            &mut script_code
                        };
                        target.push_str(line);
                        target.push('\n');
                    }

                    code.push_str(&script_code);

                    let mut new_content = script_code.clone();

                    let mut interpreter = Command::new("julia")
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .spawn()
                        .expect("Couldn't start Julia interpreter");

                    interpreter
                        .stdin
                        .as_mut()
                        .expect("No interpreter stdin")
                        .write_all(code.as_bytes())
                        .expect("Script error");

                    let output = interpreter
                        .wait_with_output()
                        .expect("Failed to execute script");

                    // Do not echo the script output into notebook output until we've seen the end
                    // of library code marker.
                    let mut in_script_output = false;
                    if output.status.success() {
                        let output =
                            String::from_utf8(output.stdout).expect("Invalid script output");
                        for line in output.lines() {
                            if in_script_output {
                                new_content.push_str(&format!("{}\u{00A0}\n", line));
                            }
                            if line.ends_with("\u{241E}") {
                                in_script_output = true;
                            }
                        }
                    } else {
                        println!("Script error!");
                    }

                    lines.clear();

                    for line in new_content.lines() {
                        lines.push(line.to_string());
                    }
                }
            }

            *outline = Outline::new_node(outline.indent(), body, Vec::new());
        }

        for i in outline.children_mut() {
            self.process_outline(i);
        }
    }
}

fn eval(_force: bool) {
    let mut buf = String::new();
    let _ = io::stdin().read_to_string(&mut buf);

    let mut outline = Outline::from_str(&buf).unwrap();
    EvalState::default().process_outline(&mut outline);

    // TODO: Eval part
    print!("{}", outline);
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
