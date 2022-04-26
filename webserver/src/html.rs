use std::fmt;

use base::Section;

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
                        writeln!(f, "<tr><td>{name}</td><td>{val}</td></tr>")?;
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
        /*
        write!(f, "<h1>")?;
        if self.is_important() {
            write!(f, "<strong>{}</strong>", self.title())?;
        } else {
            write!(f, "{}", self.title())?;
        }
        writeln!(f, "</h1>")?;
        writeln!(f, "<ul>")?;

        let mut child = self.child();

        while child.is_some() {
            write!(f, "<li>")?;
            child.fmt_inner(f)?;
            writeln!(f, "</li>")?;
            child = child.sibling();
        }

        writeln!(f, "</ul>")
        */
    }

    /*
    fn fmt_inner(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(wiki_title) = self.wiki_title() {
            // XXX: This is pretty slow.
            let len = self.iter().count();
            // Turn long wiki entries into links.
            if len > 10 {
                return write!(f, "<a href='{}'><strong>{}</strong></a>", wiki_title, wiki_title);
            }

            writeln!(f, "<a href='/{}'>{}</a>", wiki_title, wiki_title)?;
        } else {
            writeln!(f, "{}", self.
        }
    }
    */
}
