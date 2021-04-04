use anki_connect::Card;
use nom::{
    bytes::complete::{tag, take_while1},
    character::complete::{line_ending, one_of},
    combinator::{not, peek, recognize},
    multi::many1,
    sequence::{delimited, pair, terminated},
    IResult,
};
use parser::{Outline, Symbol};
use serde::Deserialize;
use std::convert::TryFrom;
use std::path::Path;

pub trait OutlineUtils {
    /// Return list of tags defined in this outline node.
    fn tags(&self) -> Vec<Symbol>;

    /// Recursively find Anki cards for the whole outline.
    fn anki_cards(&self) -> Vec<anki_connect::Card>;

    /// Does this outline describe a file repository?
    ///
    /// The headline must be empty and all child outlines must be file outlines.
    fn is_repository_outline(&self) -> bool;

    /// Return title of wiki article if outline headline defines one.
    fn wiki_title(&self) -> Option<&str>;

    /// Return ctags search command for this node if it should get one.
    fn ctags_search_string(&self) -> Option<String>;

    /// Return first child with non-empty headline.
    ///
    /// By convention this is a concise definition of a wiki concept.
    fn lead(&self) -> Option<&Self>;

    /// Return whether first non-empty headline starts with prefix
    fn starts_with(&self, prefix: &str) -> bool;

    /// Return whether last descendant headline of outline ends in suffix.
    fn ends_with(&self, suffix: &str) -> bool;
}

impl OutlineUtils for Outline {
    fn tags(&self) -> Vec<Symbol> {
        // TODO: Also handle @tag1 @tag2 style tags

        #[derive(Deserialize)]
        struct TagsData {
            tags: Vec<Symbol>,
        }

        if let Some(tags_data) = self.extract::<TagsData>() {
            tags_data.tags
        } else {
            Vec::new()
        }
    }

    fn anki_cards(&self) -> Vec<Card> {
        fn traverse(cards: &mut Vec<Card>, tags: &[Symbol], o: &Outline) {
            let mut tags = tags.to_owned();
            tags.extend_from_slice(&o.tags());

            // Filter out comments that start with ; before processing cards.
            // XXX: Maybe the comment parsing should be a whole separate phase?
            let new_cards = o
                .headline
                .as_ref()
                .filter(|h| !h.starts_with(';'))
                .and_then(|h| parser::parse_cloze(&tags, h).ok())
                .unwrap_or_else(Vec::new);
            cards.extend_from_slice(&new_cards);

            // Is this a wiki concept with a definition in the lead paragraph?
            if let (Some(headline), Some(lead)) = (o.headline.as_ref(), o.lead()) {
                if lead.starts_with("* ") && lead.ends_with(".") {
                    // TODO: Better Outline to Anki conversion?
                    let back = format!("{}", lead);

                    let front = if let Some(wiki_title) = o.wiki_title() {
                        pretty_title(wiki_title)
                    } else {
                        headline.to_string()
                    };

                    cards.push(Card {
                        front,
                        back: back.trim_end().into(),
                        tags: tags.iter().map(|t| t.to_string()).collect(),
                    });
                }
            }

            for c in &o.children {
                traverse(cards, &tags, c);
            }
        }

        let mut cards = Vec::new();
        traverse(&mut cards, &Vec::new(), self);
        cards
    }

    fn is_repository_outline(&self) -> bool {
        self.headline.is_none() && self.children.iter().all(|o| <&Path>::try_from(o).is_ok())
    }

    fn wiki_title(&self) -> Option<&str> {
        // - Complete WikiWord
        // - *Alias*
        // - Path where base is Alias
        if let Some(headline) = &self.headline {
            if let Ok(wiki_word) = complete(wiki_word)(headline) {
                return Some(wiki_word.1);
            } else if let Ok(alias_word) =
                complete(delimited(tag("*"), alias_name, tag("*")))(headline)
            {
                return Some(alias_word.1);
            }
        }
        if let Some(filename) = <&Path>::try_from(self)
            .ok()
            .and_then(|p| p.file_stem())
            .and_then(|p| p.to_str())
        {
            if filename.starts_with('.') {
                return None;
            }
            if let Ok(alias_word) = complete(alias_name)(filename) {
                return Some(alias_word.1);
            }
        }
        None
    }

    fn ctags_search_string(&self) -> Option<String> {
        if self.wiki_title().is_none() {
            return None;
        }

        if <&Path>::try_from(self).is_ok() {
            // This looks like it's a full file, return "line number 0"
            return Some("0".into());
        }

        if let Some(headline) = &self.headline {
            return Some(format!("/^\\t\\*{}$/", headline));
        }
        None
    }

    fn lead(&self) -> Option<&Self> {
        for c in &self.children {
            if c.headline.is_some() {
                return Some(c);
            }
        }
        None
    }

    fn starts_with(&self, prefix: &str) -> bool {
        if let Some(first) = self.iter().filter_map(|o| o.headline.as_ref()).next() {
            first.starts_with(prefix)
        } else {
            false
        }
    }

    fn ends_with(&self, suffix: &str) -> bool {
        if let Some(last) = self.iter().filter_map(|o| o.headline.as_ref()).last() {
            last.ends_with(suffix)
        } else {
            false
        }
    }
}

fn complete<'a, F>(f: F) -> impl FnMut(&'a str) -> IResult<&'a str, &'a str>
where
    F: FnMut(&'a str) -> IResult<&'a str, &'a str>,
{
    fn eol(i: &str) -> IResult<&str, &str> {
        if i.is_empty() {
            Ok((i, ""))
        } else {
            line_ending(i)
        }
    }

    terminated(f, eol)
}

fn wiki_word(i: &str) -> IResult<&str, &str> {
    fn wiki_word_segment(i: &str) -> IResult<&str, &str> {
        recognize(pair(wiki_word_segment_head, wiki_word_segment_tail))(i)
    }

    fn wiki_word_segment_head(i: &str) -> IResult<&str, char> {
        one_of("ABCDEFGHIJKLMNOPQRSTUVWXYZ")(i)
    }

    fn wiki_word_segment_tail(i: &str) -> IResult<&str, &str> {
        take_while1(|c: char| c.is_lowercase())(i)
    }

    terminated(
        recognize(pair(wiki_word_segment, many1(wiki_word_segment))),
        peek(not(wiki_word_segment_head)),
    )(i)
}

fn alias_name(i: &str) -> IResult<&str, &str> {
    fn is_alias_char(c: char) -> bool {
        match c {
            '-' | '.' | '_' | '/' => true,
            c if c.is_alphanumeric() => true,
            _ => false,
        }
    }

    take_while1(is_alias_char)(i)
}

/// Convert WikiTitles into Wiki Titles.
fn pretty_title(title: &str) -> String {
    let mut chars = title.chars().peekable();
    let mut ret = String::new();
    while let Some(c) = chars.next() {
        ret.push(c);
        if !c.is_whitespace() && chars.peek().map_or(false, |c| c.is_uppercase()) {
            ret.push(' ');
        }
    }
    ret
}

#[cfg(test)]
mod tests {
    use super::*;
    use parser::Outline;
    use pretty_assertions::assert_eq;

    fn h(text: &str) -> Outline {
        Outline::new(text, Vec::new())
    }

    fn o(text: &str) -> Outline {
        let mut o: Outline = text.into();
        o.lift_singleton();
        o
    }

    #[test]
    fn test_wiki_title() {
        assert_eq!(h("WikiWord").wiki_title(), Some("WikiWord"));
        assert_eq!(h("WikiWord ").wiki_title(), None);
        assert_eq!(h("WikiWord and stuff").wiki_title(), None);

        assert_eq!(h("*Alias*").wiki_title(), Some("Alias"));
        assert_eq!(h("*Alias* ").wiki_title(), None);
        assert_eq!(h("*Ali as*").wiki_title(), None);
        assert_eq!(h("Alias").wiki_title(), None);
        assert_eq!(h("*Alias* and stuff").wiki_title(), None);

        assert_eq!(h("\x1cpath/to/Filename.otl").wiki_title(), Some("Filename"));
        assert_eq!(h("\x1cpath/to/$$garbage$$.otl").wiki_title(), None);
        assert_eq!(h("\x1cpath/to/.otl").wiki_title(), None);
    }

    #[test]
    fn test_lead() {
        assert_eq!(
            o("Headline
\tLead")
            .lead(),
            Some(&o("Lead"))
        );
    }
}
