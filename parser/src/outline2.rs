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

// {{{1  Construct

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
macro_rules! outline_elt {
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
///     Outline2::from("foo"));
///
/// assert_eq!(
///     outline!["foo", "bar"],
///     Outline2::from("\
/// foo
/// bar"));
///
/// assert_eq!(
///     outline![[, "foo"], "bar"],
///     Outline2::from("\
/// \tfoo
/// bar"));
///
/// assert_eq!(
///     outline![["foo", "bar"], "baz"],
///     Outline2::from("\
/// foo
/// \tbar
/// baz"));
/// ```
macro_rules! outline {
    [$($arg:tt),*] => {
        $crate::Outline2(vec![
            $($crate::outline_elt!($arg)),*
        ])
    }
}

// {{{1  Print

fn is_comma_string(s: &str) -> bool {
    s.chars().all(|c| c == ',')
}

impl fmt::Display for Outline2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn print_line(
            f: &mut fmt::Formatter,
            depth: usize,
            s: &str,
        ) -> fmt::Result {
            if s.is_empty() {
                // Don't indent and generate trailing whitespace if the line
                // is empty.
                writeln!(f)
            } else {
                for _ in 0..depth {
                    write!(f, "\t")?;
                }
                writeln!(f, "{}", s)
            }
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
            writeln!(f, "ε")
        } else {
            print(f, 0, self)
        }
    }
}

// {{{1  Parse

fn unescape_comma_string(s: &str) -> &str {
    if is_comma_string(s) {
        &s[1..]
    } else {
        s
    }
}

impl From<&str> for Outline2 {
    fn from(s: &str) -> Self {
        // Preprocess the indent depths of lines.
        //
        // Special case lines that are all whitespace into None values. (This
        // parser does not preserve trailing whitespace on all-whitespace
        // lines.)
        fn process_line(line: &'_ str) -> Option<(usize, &'_ str)> {
            if line.chars().all(|c| c.is_whitespace()) {
                None
            } else {
                let indent = line.chars().take_while(|c| *c == '\t').count();
                Some((indent, &line[indent..]))
            }
        }

        fn parse<'a, I>(
            depth: usize,
            lines: &mut std::iter::Peekable<I>,
        ) -> Outline2
        where
            I: Iterator<Item = Option<(usize, &'a str)>>,
        {
            let mut ret = Vec::new();
            loop {
                // Clone peek value so we can do lines.next() later.
                match lines.peek().cloned() {
                    None => break,
                    Some(Some((indent, _))) if indent < depth => break,
                    Some(None) => {
                        // Interpret it as empty line at current depth.
                        lines.next();
                        let body = parse(depth + 1, lines);
                        ret.push((Some("".to_string()), body));
                    }
                    Some(Some((indent, _))) if indent > depth => {
                        // Going directly to deeper indent depth, emit an
                        // empty headline.
                        let body = parse(depth + 1, lines);
                        ret.push((None, body));
                    }
                    Some(Some((_, ","))) => {
                        lines.next();
                        let body = parse(depth + 1, lines);
                        ret.push((None, body));
                    }
                    Some(Some((_, line))) => {
                        lines.next();
                        let line = unescape_comma_string(line);
                        let body = parse(depth + 1, lines);
                        ret.push((Some(line.to_string()), body));
                    }
                }
            }
            Outline2(ret)
        }

        parse(0, &mut s.lines().map(process_line).peekable())
    }
}
impl FromStr for Outline2 {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.into())
    }
}

/* vim:set foldmethod=marker: */
