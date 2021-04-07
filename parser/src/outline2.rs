use std::fmt;
use std::str::FromStr;

// TODO: Rename into Outline, remove previous Outline type when done

/// Base datatype for an indented outline file.
///
/// The outline is a list of title, child outline pairs.
///
/// A missing title for the first item will just be omitted when printing and
/// the body is printed directly with the additional indentation. Missing
/// titles further in the list will be represented with the element separator
/// comma.
#[derive(Eq, PartialEq, Clone, Hash, Default)]
pub struct Outline2(pub Vec<(Option<String>, Outline2)>);

impl fmt::Display for Outline2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

serde_plain::derive_deserialize_from_str!(Outline2, "outline");
serde_plain::derive_serialize_from_display!(Outline2);

impl FromStr for Outline2 {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        enum Line<'a> {
            /// Regular text
            Text { indent: i32, line: &'a str },
            /// Element separator comma
            Split { depth: i32 },
            /// Empty line
            Empty,
        }

        // Preprocess the indent depths of lines.
        //
        // Special case lines that are all whitespace into None values. (This
        // parser does not preserve trailing whitespace on all-whitespace
        // lines.)
        fn process_line(line: &'_ str) -> Line<'_> {
            if line.chars().all(|c| c.is_whitespace()) {
                Line::Empty
            } else {
                let indent = line.chars().take_while(|c| *c == '\t').count();
                let line = &line[indent..];
                Line::Text { indent, text: &line[indent..] }
            }
        }

        // Parse routine...
        // Know the depth, parse until you pop out (Peekable)
        //

        fn parse<'a, I>(
            depth: i32,
            lines: &mut std::iter::Peekable<I>,
        ) -> Outline2
        where
            I: Iterator<Item = Option<(i32, &'a str)>>,
        {
            let mut ret = Outline2::default();
            loop {
                match lines.peek() {
                    None => return ret,
                    Some(Some((d, _))) if *d < depth => return ret,
                    Some(None) => {
                        // Empty line.
                        lines.next();
                        ret.0.push(("".to_string(), Default::default()));
                    }
                    Some(Some((d, line))) if *d == depth => {
                        // At expected depth.
                        lines.next();
                        let body = parse(depth + 1, lines);
                        ret.0.push((line.to_string(), body));
                    }
                    Some(Some((d, line))) if *d > depth => {
                    }
                }
            }
        }

        /*
        fn parse_children<'a, I>(
            depth: i32,
            lines: &mut std::iter::Peekable<I>,
        ) -> Vec<Outline2>
        where
            I: Iterator<Item = Option<(i32, &'a str)>>,
        {
            let mut ret = Vec::new();
            // Keep parsing child outlines until EOF or indentation dropping below current depth.
            loop {
                match lines.peek() {
                    None => return ret,
                    Some(Some((d, _))) if *d < depth => return ret,
                    _ => ret.push(parse(depth, lines)),
                }
            }
        }

        fn parse<'a, I>(
            depth: i32,
            lines: &mut std::iter::Peekable<I>,
        ) -> Outline2
        where
            I: Iterator<Item = Option<(i32, &'a str)>>,
        {
            match lines.peek().cloned() {
                // End of input
                None => Outline2::default(),
                // Empty line
                Some(None) => {
                    lines.next();
                    Outline {
                        headline: Some(String::new()),
                        children: parse_children(depth + 1, lines),
                    }
                }
                Some(Some((d, text))) => {
                    let headline = if d == depth {
                        lines.next();
                        // Group separator comma, is equivalent to empty headline in a place where
                        // an empty line isn't syntactically possible
                        if text == "," {
                            None
                        } else {
                            Some(String::from(unescape_comma_string(text)))
                        }
                    } else if d > depth {
                        None
                    } else {
                        panic!("Outline parser dropped out of depth")
                    };
                    Outline {
                        headline,
                        children: parse_children(depth + 1, lines),
                    }
                }
            }
        }
        */

        parse(-1, &mut s.lines().map(process_line).peekable())
    }
}
