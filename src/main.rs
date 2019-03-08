use parser::{Lexer, Token};
use std::io::{self, Read};
use structopt::{self, StructOpt};

#[derive(StructOpt)]
#[structopt(name = "otltool", about = "Outline processing tool")]
enum Otltool {
    #[structopt(name = "echo", about = "Test by parsing and echoing stdin input")]
    Echo,

    #[structopt(name = "tags", about = "Generate ctags file from local .otl files")]
    Tags,

    #[structopt(name = "jeval", about = "Pipe stdin outline through J evaluator")]
    JEval,

    #[structopt(
        name = "anki",
        about = "Extract and upload Anki cards from local .otl files"
    )]
    Anki,
}

fn echo() {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf).unwrap();

    fn print_depth(d: usize) {
        for _ in 0..(d - 1) {
            print!("\t");
        }
    }

    for tok in Lexer::new(&buf) {
        use Token::*;
        match tok {
            StartIndentBlock { prefix, syntax } => print!("{}{}", prefix, syntax),
            StartPrefixBlock {
                depth,
                prefix,
                syntax,
            } => {
                print_depth(depth);
                print!("{}{}", prefix, syntax);
            }
            StartPrefixBlock2 {
                depth,
                prefix,
                first_line,
            } => {
                print_depth(depth);
                print!("{} {}", prefix, first_line);
            }
            BlockLine {
                depth,
                text,
                prefix,
            } => {
                print_depth(depth);
                if let Some(prefix) = prefix {
                    print!("{} ", prefix);
                }
                print!("{}", text);
            }
            EndBlock(_) => {}
            StartLine(depth) => print_depth(depth),
            WikiTitle(t) => print!("{}", t),
            AliasDefinition(t) => print!("({})", t),
            TagDefinition(t) => print!("@{}", t),
            TextFragment(t) | WhitespaceFragment(t) | UrlFragment(t) | WikiWordFragment(t) => {
                print!("{}", t)
            }
            VerbatimFragment(t) => print!("`{}`", t),
            FileLinkFragment(t) | AliasLinkFragment(t) => print!("[{}]", t),
            InlineImageFragment(t) => print!("![{}]", t),
            ImportanceMarkerFragment => print!(" *"),
            NewLine => println!(),
        }
    }
}

fn main() {
    let opt = Otltool::from_args();
    match opt {
        Otltool::Echo => echo(),
        _ => unimplemented!(),
    }
}
