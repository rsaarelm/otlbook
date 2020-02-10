//! Goodreads CSV export file
//
// https://goodreads.com/

use crate::{LibraryEntry, Scrapeable};
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
    #[serde(rename = "Year Published")]
    year_published: String,
    #[serde(rename = "Date Added")]
    date_added: String,
    #[serde(rename = "Date Read")]
    date_read: String,
    #[serde(rename = "Bookshelves")]
    bookshelves: String,
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

impl From<GoodreadsEntry> for LibraryEntry {
    fn from(e: GoodreadsEntry) -> LibraryEntry {
        let mut ret = LibraryEntry::default();

        let isbn13 = e.isbn13.replace("\"", "").replace("=", "");
        if !isbn13.is_empty() {
            ret.uri = Some(format!(
                "isbn:{}",
                isbn13.replace("\"", "").replace("=", "")
            ));
        } else {
            log::info!("{:?} has no ISBN value", e);
        }

        ret.title = Some(e.title);

        if !e.author.is_empty() {
            ret.author = Some(e.author);
        }

        if !e.year_published.is_empty() {
            ret.year = Some(e.year_published);
        }

        if !e.date_added.is_empty() {
            ret.added = Some(e.date_added.replace("/", "-"));
        }

        if !e.date_read.is_empty() {
            ret.read = Some(e.date_read.replace("/", "-"));
        }

        ret.tags = e
            .bookshelves
            .split(", ")
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if !e.notes.is_empty() {
            ret.notes = Some(e.notes);
        }

        ret
    }
}
