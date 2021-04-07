use std::fmt;
use std::{iter::FromIterator, str::FromStr};

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

impl Outline2 {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

fn is_comma_string(s: &str) -> bool {
    s.chars().all(|c| c == ',')
}

fn unescape_comma_string(s: &str) -> &str {
    if is_comma_string(s) {
        &s[1..]
    } else {
        s
    }
}

impl fmt::Display for Outline2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn print_line(
            f: &mut fmt::Formatter,
            depth: usize,
            s: &str,
        ) -> fmt::Result {
            for _ in 0..depth {
                write!(f, "\t")?;
            }
            writeln!(f, "{}", s)
        }

        fn print(
            f: &mut fmt::Formatter,
            depth: usize,
            otl: &Outline2,
        ) -> fmt::Result {
            for (idx, (title, body)) in otl.0.iter().enumerate() {
                match title {
                    // Escape literal comma titles.
                    Some(s) if is_comma_string(s) => {
                        print_line(f, depth, &format!(",{}", s))?
                    }
                    // Regular title.
                    Some(s) => print_line(f, depth, s)?,
                    // Can skip empty title when printing first item.
                    None if idx == 0 => {}
                    // Otherwise we need to print the separator comma.
                    None => print_line(f, depth, ",")?,
                }
                // Print body.
                print(f, depth + 1, &body)?;
            }
            Ok(())
        }

        print(f, 0, self)
    }
}

impl fmt::Debug for Outline2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn print(
            f: &mut fmt::Formatter,
            depth: usize,
            otl: &Outline2,
        ) -> fmt::Result {
            for (title, body) in &otl.0 {
                for _ in 0..depth {
                    write!(f, "  ")?;
                }
                match title {
                    Some(s) => writeln!(f, "{:?}", s)?,
                    None => writeln!(f, "ε")?,
                }
                print(f, depth + 1, &body)?;
            }

            Ok(())
        }

        if self.is_empty() {
            write!(f, "ε")
        } else {
            print(f, 0, self)
        }
    }
}

impl FromIterator<(Option<String>, Outline2)> for Outline2 {
    fn from_iter<T: IntoIterator<Item = (Option<String>, Outline2)>>(
        iter: T,
    ) -> Self {
        Outline2(iter.into_iter().collect())
    }
}

serde_plain::derive_deserialize_from_str!(Outline2, "outline");
serde_plain::derive_serialize_from_display!(Outline2);

#[macro_export(local_inner_macros)]
macro_rules! _outline_elt {
    ([$arg:expr, $($child:tt),+]) => {
        (Some($arg.to_string()), outline![$($child),+])
    };
    ([, $($child:tt),+]) => {
        (None, outline![$($child),+])
    };
    ($arg:expr) => {
        (Some($arg.to_string()), $crate::Outline2::default())
    };
}

#[macro_export]
/// Construct outline literals.
///
/// ```
/// use std::iter::FromIterator;
/// use parser::{Outline2, outline};
///
/// outline!["foo", ["bar", "baz"]];
/// assert_eq!(
///     outline![],
///     Outline2::default());
///
/// assert_eq!(
///     outline!["foo"],
///     Outline2::from_iter(vec![
///             (Some("foo".to_string()), Outline2::default())
///         ].into_iter()));
///
/// assert_eq!(
///     outline!["foo", "bar"],
///     Outline2::from_iter(vec![
///             (Some("foo".to_string()), Outline2::default()),
///             (Some("bar".to_string()), Outline2::default())
///         ].into_iter()));
///
/// assert_eq!(
///     outline![[, "foo"], "bar"],
///     Outline2::from_iter(vec![
///             (None, Outline2::from_iter(vec![
///                 (Some("foo".to_string()), Outline2::default())
///             ].into_iter())),
///             (Some("bar".to_string()), Outline2::default())
///         ].into_iter()));
///
/// assert_eq!(
///     outline![["foo", "bar"], "baz"],
///     Outline2::from_iter(vec![
///             (Some("foo".to_string()), Outline2::from_iter(vec![
///                 (Some("bar".to_string()), Outline2::default())
///             ].into_iter())),
///             (Some("baz".to_string()), Outline2::default())
///         ].into_iter()));
/// ```
macro_rules! outline {
    [$($arg:tt),*] => {
        $crate::Outline2(vec![
            $($crate::_outline_elt!($arg)),*
        ])
    }
}
/*
macro_rules! _outline {
    [] => { $crate::Outline2::default() };

    [[$a:tt], $b:tt] => {
        $crate::Outline2(vec![outline!
}
*/

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
                Line::Text {
                    indent: indent as i32,
                    line: &line[indent..],
                }
            }
        }

        // Parse routine...
        // Know the depth, parse until you pop out (Peekable)
        //

        /*
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
        */

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

        //parse(-1, &mut s.lines().map(process_line).peekable())
        todo!();
    }
}
