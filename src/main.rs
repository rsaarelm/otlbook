use parser::OutlineWriter;
use std::io::{self, Read};

struct StdoutWriter;

impl OutlineWriter for StdoutWriter {
    fn text(&mut self, text: &str) {
        print!("{}", text);
    }
}

fn main() {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf).unwrap();
    StdoutWriter.parse(&buf);
}
