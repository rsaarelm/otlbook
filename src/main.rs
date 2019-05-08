use md5;
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

    pub fn process_outline(&mut self, force: bool, outline: &mut Outline) {
        self.accumulate_libraries(outline);

        self.run_script(force, outline);

        for i in outline.children_mut() {
            self.process_outline(force, i);
        }
    }

    fn run_script(&mut self, force: bool, outline: &mut Outline) {
        if let OutlineBody::Block {
            syntax: Some(syntax),
            lines,
            prefix,
            indent_line,
        } = outline.body()
        {
            if let SyntaxInfo {
                lang: Some(lang),
                is_lib: false,
                checksum,
            } = SyntaxInfo::new(&syntax)
            {
                // TODO: This is currently a mix of Julia-specific hackery and the general
                // machinery for script expansion. Probably want to support more languages than
                // Julia eventually, so figure out a way to factor the language-specific stuff
                // (eg. the workaround for the semicolon suppression bug, calling the actual
                // interpreter) from the otlbook side stuff (Using NBSP to mark output lines).
                if lang == "julia" {
                    // Code that gets executed by interpreter.
                    let lib = self.libraries.entry(lang).or_insert(String::new()).clone();

                    // Text where the checksum is derived from. Code and output.
                    let mut checksum_text = lib.clone();

                    let mut code = lib.clone();

                    // Print a separator marker to denote the end of script code. We'll use this
                    // later to throw out REPL output from script code.
                    code.push_str("println('\\u241E')\n");

                    let mut script_code = String::new();
                    for line in lines.iter() {
                        // Read lines that don't end with the output marker character into script
                        // code.
                        if !line.ends_with("\u{00A0}") {
                            script_code.push_str(line);
                            script_code.push('\n');

                            code.push_str(line);

                            // XXX: Julia has a bug where semicolons don't suppress output when
                            // code is piped in through stdin
                            // <https://github.com/JuliaLang/julia/issues/26320>. As a hacked
                            // workaround for this, ending semicolons get the separator marker
                            // appended to them so that the output up to the semicolon will be
                            // scrubbed when it's processed later. As a side effect, non-semicolon
                            // lines before the semicolon-terminated one will also have their
                            // outputs scrubbed.
                            //
                            // The hackery should be removed when the Julia bug has been fixed.
                            if line.ends_with(";") {
                                code.push_str("println('\\u241E');");
                            }

                            code.push('\n');
                        }

                        // Both input and output go into checksum text.
                        checksum_text.push_str(line);
                        checksum_text.push('\n');
                    }

                    let current_digest = md5::compute(checksum_text.as_bytes());

                    // Checksum on syntax line matches the one we just computed, so it looks like
                    // nothing has changed. Unless force flag is set, we can stop here and not
                    // execute the script.
                    if !force && checksum == Some(current_digest) {
                        return;
                    }

                    // Otherwise run the script code through the interpreter.
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

                    let mut script_output = String::new();
                    if output.status.success() {
                        let output =
                            String::from_utf8(output.stdout).expect("Invalid script output");
                        for line in output.lines() {
                            script_output.push_str(&format!("{}\u{00A0}\n", line));
                            // The magic separator char denotes regions that should not be output.
                            // When it's encountered, wipe out everything that's been written.
                            if line.ends_with("\u{241E}") {
                                script_output.clear();
                            }
                        }
                    } else {
                        println!("Script error!");
                    }

                    // Compute new checksum and update the outline with the checksum and the new
                    // output.
                    let mut new_checksum_text = lib.clone();
                    new_checksum_text.push_str(&script_code);
                    new_checksum_text.push_str(&script_output);

                    let mut new_content = String::new();
                    new_content.push_str(&script_code);
                    new_content.push_str(&script_output);

                    *outline = Outline::new_node(
                        outline.indent(),
                        OutlineBody::Block {
                            lines: new_content.lines().map(|x| x.to_string()).collect(),
                            syntax: Some(format!(
                                "julia md5:{:x}",
                                md5::compute(new_checksum_text.as_bytes())
                            )),
                            prefix: prefix.clone(),
                            indent_line: indent_line.clone(),
                        },
                        Vec::new(),
                    );
                }
            }
        }
    }
}

fn eval(force: bool) {
    let mut buf = String::new();
    let _ = io::stdin().read_to_string(&mut buf);

    let mut outline = Outline::from_str(&buf).unwrap();
    EvalState::default().process_outline(force, &mut outline);

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
