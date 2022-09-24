//! Pocket export file
//
// https://getpocket.com/

use base::{Symbol, VagueDate};
use select::{
    document::Document,
    node::Node,
    predicate::{Name, Predicate},
};
use serde::Serialize;
use std::error::Error;
use std::{collections::BTreeSet, str::FromStr};

#[derive(Debug, Serialize)]
pub struct Entry {
    pub uri: String,
    pub tags: BTreeSet<Symbol>,
    pub added: VagueDate,
    // Always "getpocket.com", so might as well skip allocating String
    pub via: &'static str,
}

#[derive(Serialize)]
pub struct Collection {
    #[serde(rename = "Read")]
    pub read: Vec<(String, (Entry, Vec<()>))>,
    #[serde(rename = "ToRead")]
    pub to_read: Vec<(String, (Entry, Vec<()>))>,
}

impl FromStr for Collection {
    type Err = Box<dyn Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        fn parse_section(
            doc: &Document,
            title: &str,
        ) -> Result<Vec<(String, (Entry, Vec<()>))>, Box<dyn Error>> {
            let reads = doc
                .find(Name("h1").and(|o: &Node| o.text() == title))
                .next()
                .ok_or("No read items found")?;
            // XXX: Skip ahead to get to the list of links
            let reads = reads.next().ok_or("No read items found")?;
            let reads = reads.next().ok_or("No read items found")?;

            let mut ret = Vec::new();
            for item in reads.find(Name("a")) {
                let mut title = item.text().to_owned();
                let uri = item.attr("href").unwrap_or("").to_string();
                if uri.is_empty() {
                    log::warn!(
                        "Empty URI in pocket item {}, discarded",
                        item.html()
                    );
                    continue;
                }

                let added = item
                    .attr("time_added")
                    .and_then(|a| a.parse::<i64>().ok())
                    .unwrap_or(0);
                let added = VagueDate::from_timestamp(added);

                let mut tags = item
                    .attr("tags")
                    .unwrap_or("")
                    .split(',')
                    .filter_map(|x: &str| Symbol::new(x).ok())
                    .collect::<BTreeSet<Symbol>>();

                let star = Symbol::new("*").unwrap();
                // Convert '*' in tags into title marker
                if tags.contains(&star) {
                    tags.remove(&star);
                    title.push_str(" *");
                }

                ret.push((
                    title,
                    (
                        Entry {
                            uri,
                            added,
                            tags,
                            via: "getpocket.com",
                        },
                        Vec::new(),
                    ),
                ));
            }

            ret.sort_by_key(|e| e.1 .0.added);

            Ok(ret)
        }

        let doc = Document::from(s);
        if let Some(title) = doc.find(Name("title")).next() {
            if title.text() != "Pocket Export" {
                return Err("Not a Pocket export file")?;
            }
        } else {
            return Err("Not a Pocket export file")?;
        }

        let read = parse_section(&doc, "Read Archive")?;
        let to_read = parse_section(&doc, "Unread")?;

        Ok(Collection { read, to_read })
    }
}
