//! Pocket export file
//
// https://getpocket.com/

use crate::{LibraryEntry, Scrapeable};
use base::{Symbol, VagueDate};
use select::{
    document::Document,
    node::Node,
    predicate::{Name, Predicate},
};
use std::collections::BTreeSet;
use std::convert::TryFrom;
use std::error::Error;

#[derive(Debug)]
pub struct PocketEntry {
    pub title: String,
    pub added: VagueDate,
    pub uri: String,
    pub tags: BTreeSet<Symbol>,
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
            let added = VagueDate::from_timestamp(added);

            let tags = item
                .attr("tags")
                .unwrap_or("")
                .split(',')
                .filter_map(|x: &str| Symbol::new(x).ok())
                .collect::<BTreeSet<Symbol>>();

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
            uri: e.uri,
            title: Some(e.title),
            read: Some(e.added),
            tags: e.tags,
            via: Some("getpocket.com".into()),
            ..Default::default()
        }
    }
}
