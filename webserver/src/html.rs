use std::{fmt, str::FromStr};

use base::{Section, Uri};

/// Display a value as HTML.
pub trait HtmlFmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;
}

/// Wrapper type for HTML-formatted display.
#[derive(Debug)]
pub struct Html<T: HtmlFmt>(pub T);

impl<T: HtmlFmt> fmt::Display for Html<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        HtmlFmt::fmt(&self.0, f)
    }
}

impl HtmlFmt for String {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl<T: HtmlFmt> HtmlFmt for Vec<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in self {
            write!(f, " ")?;
            i.fmt(f)?;
        }
        Ok(())
    }
}

impl HtmlFmt for Section {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn write(
            elt: &Section,
            tag: &str,
            f: &mut fmt::Formatter<'_>,
        ) -> fmt::Result {
            write!(f, "<{tag}>")?;
            let text = elt.title();
            let is_important = elt.is_important();

            if is_important {
                write!(f, "<strong>")?;
            }

            write!(f, "{text}")?;

            if is_important {
                write!(f, "</strong>")?;
            }
            writeln!(f, "</{tag}>")?;

            // Print attributes
            {
                let elt = elt.borrow();
                if !elt.attributes.is_empty() {
                    writeln!(f, "<table>")?;
                    for (name, val) in &elt.attributes {
                        match name.as_ref() {
                            "uri" => {
                                let val = Html(
                                    Uri::from_str(val)
                                        .unwrap_or(Uri::Http("Err".into())),
                                );

                                writeln!(
                                    f,
                                    "<tr><td>{name}</td><td>{val}</td></tr>"
                                )?;
                            }
                            _ => writeln!(
                                f,
                                "<tr><td>{name}</td><td>{val}</td></tr>"
                            )?,
                        }
                    }
                    writeln!(f, "</table>")?;
                }
            }

            writeln!(f, "<ul>")?;
            let mut child = elt.child();
            while let Some(ref node) = child {
                write!(f, "<li>")?;
                write(node, "div", f)?;
                writeln!(f, "</li>")?;
                child = node.sibling();
            }
            writeln!(f, "</ul>")
        }

        write(self, "h1", f)
    }
}

impl HtmlFmt for Uri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Uri::Http(s) => write!(f,
                "<a href='{s}'>{s}</a>"),
            Uri::Isbn(s) => write!(f,
                "<a href='https://openlibrary.org/search?isbn={s}'>isbn:{s}</a>")
        }
    }
}
