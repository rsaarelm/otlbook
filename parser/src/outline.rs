use std::fmt;
use std::str::FromStr;

#[derive(Eq, PartialEq, Debug, Default)]
/// Base datatype for an indented outline file
pub struct Outline<T> {
    /// Parent line at the element's level of indentation
    ///
    /// May be empty for elements that introduce multiple levels of indentation.
    line: Option<T>,
    /// Child elements, indented one level below this element.
    children: Vec<Outline<T>>,
}

/// Starting point for outlines that extracts just the indent structure.
pub type BasicOutline = Outline<String>;

impl FromStr for BasicOutline {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Preprocess the indent depths of lines.
        //
        // Special case lines that are all whitespace into None values. (This parser does not
        // preserve trailing whitespace on all-whitespace lines.)
        fn process_line(line: &str) -> Option<(i32, &str)> {
            if line.chars().all(|c| c.is_whitespace()) {
                None
            } else {
                let indent = line.chars().take_while(|c| *c == '\t').count();
                Some((indent as i32, &line[indent..]))
            }
        }

        fn parse_children<'a, I>(
            depth: i32,
            lines: &mut std::iter::Peekable<I>,
        ) -> Vec<BasicOutline>
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

        fn parse<'a, I>(depth: i32, lines: &mut std::iter::Peekable<I>) -> BasicOutline
        where
            I: Iterator<Item = Option<(i32, &'a str)>>,
        {
            match lines.peek().cloned() {
                None => BasicOutline::default(),
                Some(None) => {
                    lines.next();
                    Outline {
                        line: Some(String::new()),
                        children: parse_children(depth + 1, lines),
                    }
                }
                Some(Some((d, text))) => {
                    let line = if d == depth {
                        lines.next();
                        Some(String::from(text))
                    } else if d > depth {
                        None
                    } else {
                        panic!("Outline parser dropped out of depth")
                    };
                    Outline {
                        line,
                        children: parse_children(depth + 1, lines),
                    }
                }
            }
        }

        Ok(parse(-1, &mut s.lines().map(process_line).peekable()))
    }
}

impl fmt::Display for BasicOutline {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fn print(f: &mut fmt::Formatter, depth: i32, outline: &BasicOutline) -> fmt::Result {
            assert!(depth >= 0 || outline.line.is_none());

            if let Some(line) = &outline.line {
                if line.is_empty() {
                    writeln!(f)?;
                } else {
                    for _ in 0..depth {
                        write!(f, "\t")?;
                    }

                    writeln!(f, "{}", line)?;
                }
            }

            for c in &outline.children {
                print(f, depth + 1, c)?;
            }

            Ok(())
        }

        print(f, if self.line.is_some() { 0 } else { -1 }, self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outline() {
        assert_eq!(BasicOutline::from_str(""), Ok(Outline::default()));
    }
}
