//! Parsing primitives for otlbook notation

use nom::{
    branch::alt,
    bytes::complete::take_while1,
    character::complete::{digit1, satisfy},
    combinator::{eof, peek, recognize},
    error::ErrorKind,
    multi::many1,
    sequence::{pair, tuple},
    IResult,
};

pub type Result<'a, T> = std::result::Result<(T, &'a str), &'a str>;

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
}
