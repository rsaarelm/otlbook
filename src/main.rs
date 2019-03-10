use parser::{Lexer, Token};
use std::io::{self, Read};
use structopt::{self, StructOpt};

#[derive(StructOpt)]
#[structopt(name = "otltool", about = "Outline processing tool")]
enum Otltool {
    #[structopt(name = "echo", about = "Test by parsing and echoing stdin input")]
    Echo {
        #[structopt(long = "debug", help = "Print debug versions of tokens")]
        debug: bool,
    },

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

fn main() {
    let opt = Otltool::from_args();
    match opt {
        Otltool::Echo { debug } => echo(debug),
        _ => unimplemented!(),
    }
}
