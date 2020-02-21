use crate::{from_outline, into_outline};
use std::fmt;
use std::io::{self};
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Eq, PartialEq, Clone, Default)]
/// Base datatype for an indented outline file
pub struct Outline {
    /// Parent line at the element's level of indentation
    ///
    /// May be empty for elements that introduce multiple levels of indentation.
    pub headline: Option<String>,
    /// Child elements, indented one level below this element.
    pub children: Vec<Outline>,
}

pub struct OutlineIter<'a>(Vec<&'a Outline>);

impl<'a> Iterator for OutlineIter<'a> {
    type Item = &'a Outline;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next) = self.0.pop() {
            for c in next.children.iter().rev() {
                self.0.push(c);
            }
            Some(next)
        } else {
            None
        }
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

impl Outline {
    pub fn new(headline: impl Into<String>, children: Vec<Outline>) -> Outline {
        Outline {
            headline: Some(headline.into()),
            children,
        }
    }

    pub fn list(children: Vec<Outline>) -> Outline {
        Outline {
            headline: None,
            children,
        }
    }

    /// Return an iterator that recursively traverses the outline and its children.
    pub fn iter(&self) -> OutlineIter<'_> {
        OutlineIter(vec![self])
    }

    pub fn push(&mut self, outline: Outline) {
        self.children.push(outline);
    }

    pub fn push_str(&mut self, line: impl Into<String>) {
        self.push(Outline::new(line, Vec::new()));
    }

    pub fn is_empty(&self) -> bool {
        self.headline.is_none() && self.children.is_empty()
    }

    /// Extract embedded metadata block from the outline.
    ///
    /// The metadata block is a twice-indented section right below the headline,
    ///
    /// ```
    /// use parser::outline::Outline;
    /// use serde::Deserialize;
    ///
    /// #[derive(Eq, PartialEq, Debug, Deserialize)]
    /// struct Pt {
    ///     x: i32,
    ///     a: String,
    ///     #[serde(default)]
    ///     z: Vec<i32>,
    /// }
    ///
    /// let outline = Outline::from("\
    /// First item
    /// \t\tx 12
    /// \t\ta foobar
    /// \tOutline content starts here");
    /// let first_item = outline.children[0].clone();
    ///
    /// assert_eq!(first_item.extract(), Some(Pt { x: 12, a: "foobar".into(), z: vec![] }));
    ///
    /// let one_line_metadata = Outline::from("\
    /// Item
    /// \t\t123
    /// \tContent").children[0].clone();
    ///
    /// assert_eq!(one_line_metadata.extract::<i32>(), Some(123));
    /// ```
    pub fn extract<T: serde::de::DeserializeOwned>(&self) -> Option<T> {
        if let Some(outline) = self.metadata_block() {
            from_outline(outline).ok()
        } else {
            None
        }
    }

    /// Inject embedded metadata into outline, replacing any existing metadata.
    pub fn inject<T: serde::Serialize>(&mut self, data: T) {
        if self.metadata_block().is_some() {
            self.children.remove(0);
        }
        let mut data = into_outline(data).expect("Couldn't serialize metadata");
        if data.headline.is_some() {
            // Single-liner item, add extra level of identation.
            data = Outline::list(vec![data]);
        }
        self.children.insert(0, data);
    }

    fn metadata_block(&self) -> Option<&Outline> {
        if self.children.is_empty() {
            return None;
        }
        if self.children[0].headline.is_some() {
            return None;
        }
        // Remove the extra indentation level if it's a single line of data
        Some(if self.children[0].children.len() == 1 {
            &self.children[0].children[0]
        } else {
            &self.children[0]
        })
    }

    /// Join another outline to this one in a way that makes sense for the data format.
    ///
    /// If this outline's headline has no children, the other outline's headline will be catenated
    /// to this one's with a space between the headlines.
    ///
    /// Otherwise the other outline will be added to the children of this outline, but if either
    /// child has an empty headline, which indicates that the children are blocks that can't be
    /// told apart, the special comma element will be added in between them.
    pub(crate) fn concatenate(&mut self, other: Outline) {
        if other.is_empty() {
            return;
        }

        if self.children.is_empty() {
            if let Some(o) = other.headline {
                self.headline = Some(
                    self.headline
                        .as_ref()
                        .map(|s| format!("{} {}", s, o))
                        .unwrap_or(o),
                );
            }
            self.children = other.children;
        } else {
            self.concatenate_child(other);
        }
    }

    /// Like `concatenate`, but never tries to merge into headline.
    pub(crate) fn concatenate_child(&mut self, other: Outline) {
        self.push(other);
    }
}

impl From<&str> for Outline {
    fn from(s: &str) -> Outline {
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

        fn parse_children<'a, I>(depth: i32, lines: &mut std::iter::Peekable<I>) -> Vec<Outline>
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

        fn parse<'a, I>(depth: i32, lines: &mut std::iter::Peekable<I>) -> Outline
        where
            I: Iterator<Item = Option<(i32, &'a str)>>,
        {
            match lines.peek().cloned() {
                // End of input
                None => Outline::default(),
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

        parse(-1, &mut s.lines().map(process_line).peekable())
    }
}

impl FromStr for Outline {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.into())
    }
}

impl fmt::Display for Outline {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fn print_line(f: &mut fmt::Formatter, depth: i32, s: &str) -> fmt::Result {
            debug_assert!(depth >= 0);
            for _ in 0..depth {
                write!(f, "\t")?;
            }
            writeln!(f, "{}", s)
        }

        fn print(f: &mut fmt::Formatter, depth: i32, outline: &Outline) -> fmt::Result {
            assert!(depth >= 0 || outline.headline.is_none());

            if let Some(headline) = &outline.headline {
                if headline.is_empty() {
                    writeln!(f)?;
                } else if is_comma_string(headline) {
                    // Escape literal comma in output by turning , into ,,
                    print_line(f, depth, &format!(",{}", headline))?;
                } else {
                    print_line(f, depth, headline)?;
                }
            }

            for (i, c) in outline.children.iter().enumerate() {
                // Add separator commas for group outlines after the first one.
                // The first one also needs the preceding comma if it's completely empty.
                if c.headline.is_none() && (i > 0 || c.children.is_empty()) {
                    print_line(f, depth + 1, ",")?;
                }
                print(f, depth + 1, c)?;
            }

            Ok(())
        }

        print(f, if self.headline.is_some() { 0 } else { -1 }, self)
    }
}

impl fmt::Debug for Outline {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fn indent(f: &mut fmt::Formatter, depth: i32) -> fmt::Result {
            for _ in 0..depth {
                write!(f, "  ")?;
            }
            Ok(())
        }

        fn print(f: &mut fmt::Formatter, depth: i32, outline: &Outline) -> fmt::Result {
            indent(f, depth)?;
            match &outline.headline {
                None => writeln!(f, "ε")?,
                Some(h) => writeln!(f, "{:?}", h)?,
            }

            for c in &outline.children {
                print(f, depth + 1, c)?;
            }

            Ok(())
        }

        print(f, 0, self)
    }
}

// Recursively turn a file or an entire directory into an outline.
impl std::convert::TryFrom<&Path> for Outline {
    type Error = std::io::Error;
    fn try_from(path: &Path) -> Result<Outline, Self::Error> {
        fn is_outline(path: impl AsRef<Path>) -> bool {
            match path.as_ref().metadata() {
                Ok(m) if m.is_dir() => true,
                Ok(m) if m.is_file() && path.as_ref().to_str().unwrap_or("").ends_with(".otl") => {
                    true
                }
                _ => false,
            }
        }
        fn to_headline(path: impl AsRef<Path>) -> Option<String> {
            if let Some(mut path) = path.as_ref().file_name().map_or(None, |p| p.to_str()) {
                if path.ends_with(".otl") {
                    path = &path[..path.len() - 4];
                }

                Some(path.into())
            } else {
                None
            }
        }

        if !is_outline(path) {
            // XXX: Random error content, just want to drop out and fail here.
            return Err(io::Error::from_raw_os_error(0));
        }

        // It's a directory, crawl contents and build outline
        if let Ok(iter) = std::fs::read_dir(path) {
            let mut contents: Vec<PathBuf> =
                iter.filter_map(|e| e.ok().map(|p| p.path())).collect();
            contents.sort_by_key(|p| to_headline(p));

            let children: Vec<Outline> = contents
                .iter()
                .filter_map(|p: &PathBuf| Outline::try_from(p.as_ref() as &Path).ok())
                .collect();

            if children.is_empty() {
                return Err(io::Error::from_raw_os_error(0));
            }

            return Ok(Outline {
                headline: to_headline(path),
                children,
            });
        }

        // It's a file
        if let Ok(text) = std::fs::read_to_string(path) {
            let mut ret: Outline = Outline::from(text.as_ref());

            // Should get us an outline with no headline, just children.
            debug_assert!(ret.headline.is_none());

            // Put filename in as the headline.
            ret.headline = to_headline(path);

            // We should have bailed out earlier if this isn't a headlinable file.
            debug_assert!(ret.headline.is_some());

            // Special case, ".otl" shows up as headline-less
            if ret.headline.as_ref().map_or(false, |s| s.is_empty()) {
                ret.headline = None;
            }

            Ok(ret)
        } else {
            Err(io::Error::from_raw_os_error(0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn test_roundtrip(outline: &Outline) {
        let text = format!("{}", outline);
        let mut parsed = Outline::from(text.as_str());
        if !outline.headline.is_none() {
            parsed = parsed.children[0].clone();
        }
        assert_eq!(&parsed, outline);
    }

    #[test]
    fn test_outline() {
        assert_eq!(Outline::from_str(""), Ok(Outline::default()));
    }

    #[test]
    fn test_metadata_block() {
        let outline = "\
Outline headline
\t\tx 12
\t\ta foobar
\tOutline content starts here"
            .parse::<Outline>()
            .unwrap()
            .children[0]
            .clone();
        test_roundtrip(&outline);

        let metadata = "\
\tx 12
\ta foobar"
            .parse::<Outline>()
            .unwrap()
            .children[0]
            .clone();

        assert_eq!(outline.metadata_block(), Some(&metadata));
        assert_eq!(metadata.metadata_block(), None);
    }

    #[test]
    fn test_comma_escape() {
        assert_eq!(
            Outline::from_str(",,").unwrap().children[0],
            Outline::new(",", Vec::new())
        );
        assert_eq!(
            Outline::from_str(",,,").unwrap().children[0],
            Outline::new(",,", Vec::new())
        );

        assert_eq!(format!("{}", Outline::new(",", Vec::new())), ",,\n");
        assert_eq!(format!("{}", Outline::new(",,", Vec::new())), ",,,\n");

        test_roundtrip(&Outline::from(","));
        test_roundtrip(&Outline::from(",,"));
    }
}
