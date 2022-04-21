use std::{convert::TryFrom, str::FromStr};

#[derive(Eq, PartialEq, Debug)]
pub enum Command {
    ViewArticle(String),
    SaveToRead(String),
    SaveBookmark(String),
}

impl FromStr for Command {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use Command::*;

        // Special case, starting with upper case letter points directly to
        // article.
        if let Some(s) = s.strip_prefix("/") {
            if s.chars().next().map_or(false, |c| c.is_ascii_uppercase()) {
                return Ok(ViewArticle(s.into()));
            }
        }

        if let Some(s) = s.strip_prefix("/a/") {
            return Ok(ViewArticle(s.into()));
        }

        if let Some(s) = s.strip_prefix("/read/") {
            return Ok(SaveToRead(s.into()));
        }

        if let Some(s) = s.strip_prefix("/save/") {
            return Ok(SaveBookmark(s.into()));
        }

        Err(())
    }
}

impl TryFrom<&rouille::Request> for Command {
    type Error = ();

    fn try_from(value: &rouille::Request) -> Result<Self, Self::Error> {
        Self::from_str(value.raw_url())
    }
}
