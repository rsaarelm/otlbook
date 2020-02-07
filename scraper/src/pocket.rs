//! Pocket export file
//
// https://getpocket.com/

use crate::{LibraryEntry, Scrapeable};
use chrono::TimeZone;
use select::{
    document::Document,
    node::Node,
    predicate::{Name, Predicate},
};
use std::convert::TryFrom;
use std::error::Error;

#[derive(Debug)]
pub struct PocketEntry {
    pub title: String,
    pub added: chrono::DateTime<chrono::Utc>,
    pub uri: String,
    pub tags: Vec<String>,
}

#[derive(Debug)]
pub struct Entries(pub Vec<PocketEntry>);

impl TryFrom<&Scrapeable> for Entries {
    type Error = Box<dyn Error>;

    fn try_from(s: &Scrapeable) -> Result<Entries, Self::Error> {
        let doc = Document::from(s.as_ref());
        if let Some(title) = doc.find(Name("title")).next() {
            if title.text() != "Pocket Export" {
                return Err("Not a Pocket export file")?;
            }
        } else {
            return Err("Not a Pocket export file")?;
        }

        // Find the section of read items
        // (Not bothering with the to-read queue, it'll end up read eventually)
        let reads = doc
            .find(Name("h1").and(|o: &Node| o.text() == "Read Archive"))
            .next()
            .ok_or("No read items found")?;
        // XXX: Skip ahead to get to the list of links
        let reads = reads.next().ok_or("No read items found")?;
        let reads = reads.next().ok_or("No read items found")?;

        let mut ret = Vec::new();
        for item in reads.find(Name("a")) {
            let title = item.text();
            let uri = item.attr("href").unwrap_or("").to_string();
            if uri.is_empty() {
                log::warn!("Empty URI in pocket item {}, discarded", item.html());
                continue;
            }

            let added = item
                .attr("time_added")
                .and_then(|a| a.parse::<i64>().ok())
                .unwrap_or(0);
            let added = chrono::Utc.timestamp(added, 0);

            let tags = item
                .attr("tags")
                .unwrap_or("")
                .split(",")
                .map(|x| x.to_string())
                .collect();

            ret.push(PocketEntry {
                title,
                uri,
                added,
                tags,
            });
        }

        ret.sort_by_key(|e| e.added);

        Ok(Entries(ret))
    }
}

impl From<PocketEntry> for LibraryEntry {
    fn from(e: PocketEntry) -> LibraryEntry {
        LibraryEntry {
            title: Some(e.title),
            uri: Some(e.uri),
            read: Some(format!("{}", e.added)),
            tags: e.tags,
            ..Default::default()
        }
    }
}
