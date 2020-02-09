//! Netscape bookmarks file
//
// http://fileformats.archiveteam.org/wiki/Netscape_bookmarks

use crate::{LibraryEntry, Scrapeable};
use chrono::{TimeZone, Utc};
use select::{document::Document, predicate::Name};
use std::convert::TryFrom;
use std::error::Error;

#[derive(Debug)]
pub struct NetscapeBookmarksEntry {
    pub title: String,
    pub uri: String,
    pub added: chrono::DateTime<chrono::Utc>,
    pub tags: Vec<String>,
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
                let added = Utc.timestamp(added, 0);
                let tags = a
                    .attr("tags")
                    .unwrap_or("")
                    .split(",")
                    .map(|s| s.to_string())
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
            title: Some(e.title),
            uri: Some(e.uri),
            added: Some(format!("{}", e.added)),
            tags: e.tags,
            notes: e.notes,
            ..Default::default()
        }
    }
}
