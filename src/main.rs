use parser::OutlineWriter;
use std::io::{self, Read};

struct StdoutWriter;

impl OutlineWriter for StdoutWriter {
    fn text(&mut self, text: &str) {
        print!("{}", text);
    }
}

#[derive(Default)]
struct DebugWriter {
    buf: String,
}

impl DebugWriter {
    fn flush(&mut self) {
        if !self.buf.is_empty() {
            print!("{:?} ", self.buf);
            self.buf = String::new();
        }
    }
}

impl OutlineWriter for DebugWriter {
    fn start_line(&mut self, depth: i32) {
        for _ in 0..depth {
            print!("\t");
        }
    }

    fn end_line(&mut self) {
        self.flush();
        println!("");
    }

    fn text(&mut self, text: &str) {
        self.buf.push_str(text);
    }

    fn text_block_line(&mut self, depth: i32, prefix: Option<&str>, text: &str) {
        self.start_line(depth);
        println!("[block-line {} {:?}]", prefix.unwrap_or(""), text);
    }

    fn paragraph_break(&mut self) {
        self.flush();
        print!("[para]");
    }

    fn important_line(&mut self) {
        self.flush();
        print!("[important!]");
    }

    fn importance_marker(&mut self) {}

    fn verbatim_text(&mut self, verbatim: &str) {
        self.flush();
        print!("[verb {:?}]", verbatim);
    }

    fn wiki_word_link(&mut self, wiki_word: &str) {
        self.flush();
        print!("[wiki-word {}]", wiki_word);
    }

    fn wiki_word_heading(&mut self, wiki_word: &str) {
        self.flush();
        print!("[heading {}]", wiki_word);
    }

    fn alias_link(&mut self, wiki_alias: &str) {
        self.flush();
        print!("[link {}]", wiki_alias);
    }

    fn url(&mut self, url: &str) {
        self.flush();
        print!("[web-link {}]", url);
    }

    fn inline_image(&mut self, image_path: &str) {
        self.flush();
        print!("[img {}]", image_path);
    }

    fn local_link(&mut self, file_path: &str) {
        self.flush();
        print!("[file-link {}]", file_path);
    }

    fn alias_definition(&mut self, alias: &str) {
        self.flush();
        print!("[aka {}]", alias);
    }

    fn tag_definition(&mut self, tag: &str) {
        self.flush();
        print!("[tag {}]", tag);
    }
}

fn main() {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf).unwrap();
    DebugWriter::default().parse(&buf);
}
