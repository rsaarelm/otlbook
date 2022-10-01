use std::{error::Error, fmt, str::FromStr};

use serde::{de, Deserialize, Deserializer, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};

/// A string-like type that's guaranteed to be a single word without whitespace.
///
/// Used in outline data declarations, inline lists must consist of symbol-like values.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Debug)]
pub struct Sym<T: AsRef<str>>(T);

impl<T: AsRef<str>> std::ops::Deref for Sym<T> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl Default for Sym<String> {
    fn default() -> Self {
        // XXX This has to be somewhat arbitrary, since `Symbol` can't be an
        // empty string, but we want it so we can derive `Default` for
        // symbol-carrying structs. Currently leaning towards a general
        // convention of using "-" to mean empty/missing in a symbol-expecting
        // context.
        Sym::new("-".to_string()).unwrap()
    }
}

impl<T: AsRef<str>> Sym<T> {
    pub fn new<U: Into<T>>(value: U) -> Result<Self, ()> {
        let value = value.into();

        if value.as_ref().is_empty() {
            return Err(());
        }
        if value.as_ref().chars().any(|c| c.is_whitespace()) {
            return Err(());
        }
        Ok(Sym(value))
    }
}

impl<'a, T: AsRef<str> + FromStr<Err = E>, E: Error + 'static> FromStr
    for Sym<T>
{
    type Err = Box<dyn Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let inner = T::from_str(s)?;
        match Sym::new(inner) {
            Err(_) => {
                return Err("err")?;
            }
            Ok(ok) => {
                return Ok(ok);
            }
        }
    }
}

impl<T: AsRef<str>> fmt::Display for Sym<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.as_ref())
    }
}

impl<'de, T: AsRef<str> + Deserialize<'de> + fmt::Debug> Deserialize<'de>
    for Sym<T>
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let inner = T::deserialize(deserializer)?;
        match Sym::new(inner) {
            Ok(ret) => Ok(ret),
            Err(_) => Err(de::Error::custom("Invalid symbol")),
        }
    }
}

#[macro_export]
macro_rules! sym {
    ($fmt:expr) => {
        $crate::Sym::new($fmt).expect("Invalid symbol")
    };

    ($fmt:expr, $($arg:expr),*) => {
        $crate::Sym::new(format!($fmt, $($arg),*)).expect("Invalid symbol")
    };
}

#[derive(
    Clone, Eq, PartialEq, Hash, Debug, DeserializeFromStr, SerializeDisplay,
)]
pub enum Uri {
    Http(String),
    Isbn(String),
}

impl FromStr for Uri {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Err(s.into());
        }

        if let Some(isbn) = s.strip_prefix("isbn:") {
            Ok(Uri::Isbn(isbn.into()))
        } else {
            // TODO: Validate HTTP URIs
            Ok(Uri::Http(s.into()))
        }
    }
}

impl fmt::Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Uri::Http(s) => write!(f, "{}", s),
            Uri::Isbn(s) => write!(f, "isbn:{}", s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sym;

    #[test]
    fn test_symbol() {
        type Symbol<'a> = Sym<&'a str>;

        assert!(Symbol::new("foobar").is_ok());
        assert!(Symbol::new("").is_err());
        assert!(Symbol::new("foo bar").is_err());
        assert!(Symbol::new("  foobar").is_err());
        assert!(Symbol::new("foo\nbar").is_err());
        assert!(Symbol::new("foobar\n").is_err());

        let mut tags: Vec<Symbol> = vec![sym!("b"), sym!("c"), sym!("a")];
        tags.sort();
        let tags: Vec<String> =
            tags.into_iter().map(|c| c.to_string()).collect();
        assert_eq!(tags.join(" "), "a b c");
    }

    #[test]
    fn test_symbol_literal() {
        let s1: Sym<String> = sym!("foo");
        let s2: Sym<&str> = sym!("bar");
        assert_eq!(&format!("{}", s1), "foo");
        assert_eq!(&format!("{}", s2), "bar");
    }
}
