//! Goodreads CSV export file
//
// https://goodreads.com/

use crate::{LibraryEntry, Scrapeable, VagueDate};
use parser::{sym, Symbol};
use serde::Deserialize;
use std::convert::TryFrom;
use std::error::Error;

#[derive(Debug, Deserialize)]
pub struct GoodreadsEntry {
    #[serde(rename = "Title")]
    title: String,
    #[serde(rename = "Author")]
    author: String,
    #[serde(rename = "ISBN")]
    isbn: String,
    #[serde(rename = "ISBN13")]
    isbn13: String,
    #[serde(rename = "My Rating")]
    my_rating: i32,
    #[serde(rename = "Year Published")]
    year_published: String,
    #[serde(rename = "Date Read")]
    date_read: String,
    #[serde(rename = "Date Added")]
    date_added: String,
    #[serde(rename = "Bookshelves")]
    bookshelves: String,
    #[serde(rename = "Exclusive Shelf")]
    exclusive_shelf: String,
    #[serde(rename = "Private Notes")]
    notes: String,
}

#[derive(Debug)]
pub struct Entries(pub Vec<GoodreadsEntry>);

impl TryFrom<&Scrapeable> for Entries {
    type Error = Box<dyn Error>;

    fn try_from(s: &Scrapeable) -> Result<Entries, Self::Error> {
        let mut rdr = csv::Reader::from_reader(s.as_bytes());
        let mut ret = Vec::new();
        for result in rdr.deserialize() {
            let result: GoodreadsEntry = result?;
            if result.title.is_empty() {
                log::info!("Skipping invalid goodreads entry (no title): {:?}", result);
            } else {
                ret.push(result);
            }
        }
        Ok(Entries(ret))
    }
}

fn normalize_isbn_13(isbn: &str) -> String {
    let mut digits: Vec<u32> = isbn.chars().filter_map(|c| c.to_digit(10)).collect();

    // ISBN-13 check digit algorithm
    // https://en.wikipedia.org/wiki/ISBN#ISBN-13_check_digit_calculation
    let sum: u32 = digits
        .iter()
        .enumerate()
        .take(digits.len() - 1)
        .map(|(i, &x)| if i % 2 == 0 { x } else { x * 3 })
        .sum::<u32>()
        % 10;
    let idx = digits.len() - 1;
    digits[idx] = 10 - sum;

    digits
        .into_iter()
        .filter_map(|i| std::char::from_digit(i, 10))
        .collect()
}

fn isbn_10_to_13(isbn10: &str) -> String {
    normalize_isbn_13(&format!("978{}", isbn10))
}

impl From<GoodreadsEntry> for LibraryEntry {
    fn from(e: GoodreadsEntry) -> LibraryEntry {
        let mut ret = LibraryEntry::default();
        ret.via = Some("goodreads.com".into());

        let mut isbn13 = e.isbn13.replace("\"", "").replace("=", "");
        let isbn10 = e.isbn.replace("\"", "").replace("=", "");

        if isbn13.is_empty() && !isbn10.is_empty() {
            isbn13 = isbn_10_to_13(&isbn10);
        }

        if !isbn13.is_empty() {
            ret.uri = format!("isbn:{}", isbn13);
        } else {
            ret.uri = format!(
                "title:{}",
                parser::normalize_title(&format!("{} {}", e.title, e.author))
            );
            log::info!("{:?} has no ISBN value", e);
            if e.title.is_empty() {
                log::warn!("{:?} has no title!", e);
            }
        }

        ret.title = Some(e.title);

        if !e.author.is_empty() {
            ret.author = Some(e.author);
        }

        if !e.year_published.is_empty() {
            ret.published = e.year_published.parse().map(VagueDate::Year).ok();
        }

        if !e.date_added.is_empty() {
            ret.added = e.date_added.replace("/", "-").parse().ok();
        }

        if !e.date_read.is_empty() {
            ret.read = e.date_read.replace("/", "-").parse().ok();
        }

        ret.tags = e
            .bookshelves
            .split(", ")
            .filter_map(|s| Symbol::new(s).ok())
            .collect();

        if let Ok(exclusive_shelf) = Symbol::new(e.exclusive_shelf) {
            if !ret.tags.contains(&exclusive_shelf) {
                ret.tags.insert(exclusive_shelf);
            }
        }

        if e.my_rating != 0 {
            ret.tags.insert(sym!("rating-{}", e.my_rating));
        }

        if !e.notes.is_empty() {
            ret.notes = Some(e.notes);
        }

        ret
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isbn_13() {
        assert_eq!(isbn_10_to_13("0262510871"), "9780262510875");
        assert_eq!(isbn_10_to_13("0465026567"), "9780465026562");
    }
}
