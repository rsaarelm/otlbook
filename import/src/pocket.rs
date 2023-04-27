//! Pocket export file
//
// https://getpocket.com/

use base::{Result, Symbol, VagueDate};
use select::{
    document::Document,
    node::Node,
    predicate::{Name, Predicate},
};
use serde::Serialize;

use std::collections::BTreeSet;

#[derive(Debug, Serialize)]
pub struct Entry {
    pub uri: String,
    pub tags: BTreeSet<Symbol>,
    pub added: Option<VagueDate>,
    pub read: Option<VagueDate>,
    // Always "getpocket.com", so might as well skip allocating String
    pub via: &'static str,
}

type Section = ((String,), ((Entry,), ()));

pub fn import_to_read(s: &str) -> Result<Vec<Section>> {
    import(s, "Unread")
}

pub fn import_read(s: &str) -> Result<Vec<Section>> {
    import(s, "Read Archive")
}

fn import(s: &str, title: &str) -> Result<Vec<Section>> {
    let doc = Document::from(s);
    if let Some(title) = doc.find(Name("title")).next() {
        if title.text() != "Pocket Export" {
            return Err("Not a Pocket export file")?;
        }
    } else {
        return Err("Not a Pocket export file")?;
    }

    parse_section(&doc, title)
}

fn parse_section(doc: &Document, title: &str) -> Result<Vec<Section>> {
    let reads = doc
        .find(Name("h1").and(|o: &Node| o.text() == title))
        .next()
        .ok_or("No read items found")?;
    // XXX: Skip ahead to get to the list of links
    let reads = reads.next().ok_or("No read items found")?;
    let reads = reads.next().ok_or("No read items found")?;

    let mut ret = Vec::new();
    for item in reads.find(Name("a")) {
        ret.push(parse_entry(item, title == "Unread")?);
    }

    // Sort by date
    ret.sort_by_key(|(_, ((Entry { read, added, .. },), _))| {
        read.unwrap_or_else(|| added.unwrap_or(VagueDate::Year(0)))
    });

    Ok(ret)
}

fn parse_entry(item: Node, to_read: bool) -> Result<Section> {
    let mut title = item.text().to_owned();
    let uri = item.attr("href").unwrap_or("").to_string();
    if uri.is_empty() {
        return Err(format!(
            "Empty URI in pocket item {}, discarded",
            item.html()
        )
        .into());
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

    Ok((
        (title,),
        (
            (Entry {
                uri,
                added: if to_read { Some(added) } else { None },
                read: if !to_read { Some(added) } else { None },
                tags,
                via: "getpocket.com",
            },),
            (),
        ),
    ))
}
