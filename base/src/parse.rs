use lazy_static::lazy_static;

pub type Result<'a, T> = std::result::Result<(T, &'a str), &'a str>;

// NB. Probably want to always have the regexps used for word parsing start
// with "^" so the regex engine won't skip over non-parse content looking for
// the match.

pub fn wiki_word(i: &str) -> Result<&str> {
    lazy_static! {
        static ref RE: regex::Regex =
            regex::Regex::new(r"^[A-Z][a-z]+(?:[A-Z][a-z]+|[0-9]+)+\b")
                .unwrap();
    }

    if let Some(m) = RE.find(i) {
        Ok((m.as_str(), &i[m.end()..]))
    } else {
        Err(i)
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
        assert_eq!(wiki_word("WikiWord"), Ok(("WikiWord", "")));
        assert_eq!(wiki_word("Wiki1Word2"), Ok(("Wiki1Word2", "")));
        assert_eq!(wiki_word("WikiWord-s"), Ok(("WikiWord", "-s")));
        assert_eq!(wiki_word("Wiki1984Word"), Ok(("Wiki1984Word", "")));
    }
}
