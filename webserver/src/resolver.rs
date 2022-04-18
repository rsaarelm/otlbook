use std::{convert::TryFrom, str::FromStr};

#[derive(Eq, PartialEq, Debug)]
enum Command {
    ViewArticle(String),
    SaveToRead(String),
    SaveBookmark(String),
}

impl FromStr for Command {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        todo!()
    }
}

impl TryFrom<&rouille::Request> for Command {
    type Error = ();

    fn try_from(value: &rouille::Request) -> Result<Self, Self::Error> {
        Self::from_str(value.raw_url())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_article() {
        use Command::*;

        // Must start with upper case letter
        assert!(Command::from_str(
            "/somethingthatlookslikenosensiblecommandprefix"
        )
        .is_err());
        // Not a wiki word, only one segment
        assert!(Command::from_str("/Article").is_err());
        // Invalid segments
        assert!(Command::from_str("/AWord").is_err());
        assert!(Command::from_str("/ArticleW").is_err());
        // Non-wikiword trailing junk
        assert!(Command::from_str("/WikiWorld-crap").is_err());

        // Not a wiki word, only one segment
        assert_eq!(
            Command::from_str("/WikiWord"),
            Ok(ViewArticle("WikiWord".into()))
        );
        assert_eq!(
            Command::from_str("/Wiki1234"),
            Ok(ViewArticle("Wiki1234".into()))
        );
        assert_eq!(
            Command::from_str("/Wiki1234Word"),
            Ok(ViewArticle("Wiki1234Word".into()))
        );
    }
}
