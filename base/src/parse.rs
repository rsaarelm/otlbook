//! Parsing primitives for otlbook notation

use nom::{
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::{digit1, satisfy},
    combinator::{eof, opt, peek, recognize},
    error::ErrorKind,
    multi::many1,
    sequence::{pair, terminated, tuple},
    IResult, Parser,
};

pub fn wiki_word(i: &str) -> IResult<&str, &str> {
    recognize(tuple((
        wiki_word_segment,
        many1(alt((wiki_word_segment, digit1))),
        peek(word_end),
    )))(i)
}

fn wiki_word_segment(i: &str) -> IResult<&str, &str> {
    recognize(pair(
        satisfy(|c: char| c.is_ascii_uppercase()),
        take_while1(|c: char| c.is_ascii_lowercase()),
    ))(i)
}

fn word_end(i: &str) -> IResult<&str, &str> {
    alt((eof, recognize(many1(satisfy(|c| !c.is_alphanumeric())))))(i)
}

/// Parse article titles in notes.
/// Return (todo-state, done-percent, main text, important-item-flag) tuple.
pub fn title(
    mut i: &str,
) -> IResult<&str, (Option<(bool, Option<i32>)>, &str, bool)> {
    let todo_header: Option<(bool, Option<i32>)> =
        if let Ok((rest, (state, percent))) =
            // To-do box with percent indicator.
            pair::<_, _, _, nom::error::Error<_>, _, _>(
                    alt((
                        tag("[_] ").map(|_| false),
                        tag("[X] ").map(|_| true),
                    )),
                    opt(terminated(
                        digit1.map(|s: &str| s.parse::<i32>().unwrap()),
                        tag("% "),
                    )),
                )(i)
        {
            i = rest;
            Some((state, percent))
        } else {
            None
        };

    let i = i.trim_end();
    if let Some(i) = i.strip_suffix(" *") {
        Ok(("", (todo_header, i, true)))
    } else {
        Ok(("", (todo_header, i, false)))
    }
}

/// Combinator for parsing with no trailing input left.
pub fn only<'a, T>(
    p: impl Fn(&'a str) -> IResult<&'a str, T>,
) -> impl FnOnce(
    &'a str,
) -> std::result::Result<T, nom::Err<nom::error::Error<&'a str>>> {
    move |i| match p(i) {
        Ok((rest, ret)) if rest.is_empty() => Ok(ret),
        Ok(_) => {
            Err(nom::Err::Error(nom::error::Error::new(i, ErrorKind::Fail)))
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wiki_word() {
        assert!(wiki_word("").is_err());
        assert!(wiki_word("word").is_err());
        assert!(wiki_word("Word").is_err());
        assert!(wiki_word("aWikiWord").is_err());
        assert!(wiki_word("WikiW").is_err());
        assert!(wiki_word("WikiWordW").is_err());
        assert!(wiki_word("xyz WikiWord").is_err());
        assert!(wiki_word("1984WikiWord").is_err());
        assert_eq!(wiki_word("WikiWord"), Ok(("", "WikiWord")));
        assert_eq!(wiki_word("Wiki1Word2"), Ok(("", "Wiki1Word2")));
        assert_eq!(wiki_word("WikiWord-s"), Ok(("-s", "WikiWord")));
        assert_eq!(wiki_word("Wiki1984Word"), Ok(("", "Wiki1984Word")));
    }

    #[test]
    fn test_only() {
        assert_eq!(only(wiki_word)("WikiWord"), Ok("WikiWord"));
        assert!(only(wiki_word)("WikiWord junk").is_err());
        assert!(only(wiki_word)("WikiWord ").is_err());
    }

    #[test]
    fn test_title() {
        assert_eq!(title(""), Ok(("", (None, "", false))));
        assert_eq!(title("xyzzy"), Ok(("", (None, "xyzzy", false))));
        assert_eq!(title("xyzzy *"), Ok(("", (None, "xyzzy", true))));
        assert_eq!(
            title("[_] xyzzy"),
            Ok(("", (Some((false, None)), "xyzzy", false)))
        );
        assert_eq!(
            title("[X] xyzzy"),
            Ok(("", (Some((true, None)), "xyzzy", false)))
        );
        assert_eq!(
            title("[_] 1% xyzzy"),
            Ok(("", (Some((false, Some(1))), "xyzzy", false)))
        );
        assert_eq!(
            title("[X] 100% xyzzy"),
            Ok(("", (Some((true, Some(100))), "xyzzy", false)))
        );
    }
}
