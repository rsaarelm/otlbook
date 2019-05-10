use md5;
use parser::{self, Outline, OutlineBody, SyntaxInfo};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::process::{Command, Stdio};
use std::str::FromStr;

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

pub fn eval(force: bool) {
    let mut buf = String::new();
    let _ = io::stdin().read_to_string(&mut buf);

    let mut outline = Outline::from_str(&buf).unwrap();
    EvalState::default().process_outline(force, &mut outline);

    print!("{}", outline);
}
