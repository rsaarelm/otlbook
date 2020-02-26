//! Netscape bookmarks file
//
// http://fileformats.archiveteam.org/wiki/Netscape_bookmarks

use crate::{LibraryEntry, Scrapeable};
use parser::{Symbol, VagueDate};
use select::{document::Document, predicate::Name};
use std::collections::BTreeSet;
use std::convert::TryFrom;
use std::error::Error;

#[derive(Debug)]
pub struct NetscapeBookmarksEntry {
    pub title: String,
    pub uri: String,
    pub added: VagueDate,
    pub tags: BTreeSet<Symbol>,
    pub notes: Option<String>,
}

#[derive(Debug)]
pub struct Entries(pub Vec<NetscapeBookmarksEntry>);

impl TryFrom<&Scrapeable> for Entries {
    type Error = Box<dyn Error>;

    fn try_from(s: &Scrapeable) -> Result<Entries, Self::Error> {
        if !s.starts_with("<!DOCTYPE NETSCAPE-Bookmark") {
            return Err("not a bookmark file")?;
        }

        let doc = Document::from(s.as_ref());

        let mut ret = Vec::new();

        let mut node = doc.find(Name("dt")).next();
        while let Some(item) = node {
            if let Some("dt") = item.name() {
                // TODO: Replace panicing unwraps with error handling.
                let a = item.find(Name("a")).next().unwrap();
                let title = a.text();
                let uri = if let Some(uri) = a.attr("href") {
                    uri.to_string()
                } else {
                    log::warn!("No URI in bookmark item");
                    continue;
                };

                let added = a
                    .attr("add_date")
                    .and_then(|a| a.parse::<i64>().ok())
                    .unwrap_or(0);
                let added = VagueDate::from_timestamp(added);
                let tags = a
                    .attr("tags")
                    .unwrap_or("")
                    .split(",")
                    .filter_map(|s| Symbol::new(s).ok())
                    .filter(|s| !s.is_empty())
                    .collect();

                ret.insert(
                    0,
                    NetscapeBookmarksEntry {
                        title,
                        uri,
                        added,
                        tags,
                        notes: None,
                    },
                );
            }
            if let Some("dd") = item.name() {
                if ret.is_empty() {
                    log::warn!("Malformed bookmark file");
                    continue;
                }
                ret[0].notes = Some(item.text());
            }
            node = item.next();
        }

        ret.sort_by_key(|o| o.added);

        Ok(Entries(ret))
    }
}

impl From<NetscapeBookmarksEntry> for LibraryEntry {
    fn from(e: NetscapeBookmarksEntry) -> LibraryEntry {
        LibraryEntry {
            uri: e.uri,
            title: Some(e.title),
            added: Some(e.added),
            tags: e.tags,
            notes: e.notes,
            via: Some("Netscape bookmarks".into()),
            ..Default::default()
        }
    }
}
