use serde::{Deserialize, Serialize};
use parser::into_outline;
use std::convert::TryFrom;
use std::error::Error;
use parser::outline::Outline;

mod goodreads;
mod netscape_bookmarks;
mod pocket;

mod wayback;
pub use wayback::check_wayback;

pub type Uri = String;

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct LibraryEntry {
    pub title: Option<String>,
    pub uri: Option<Uri>,
    pub author: Option<String>,
    pub tags: Vec<String>,
    /// Publication year of item
    pub year: Option<String>, // TODO: Special type for Year
    /// When the item was read
    pub read: Option<String>, // TODO: Special variable-precision date type
    /// When the item was added to read list
    pub added: Option<String>, // TODO: Special variable-precision date type
    pub links: Vec<Uri>,
    pub notes: Option<String>,
}

impl From<LibraryEntry> for Outline {
    fn from(mut e: LibraryEntry) -> Outline {
        let notes = if let Some(n) = &e.notes {
            n.clone()
        } else {
            String::new()
        };

        let title = if let Some(t) = &e.title {
            t.clone()
        } else if let Some(u) = &e.uri {
            u.clone()
        } else {
            "n/a".to_string()
        };

        // Don't want notes text copied in the metadata
        e.notes = None;

        let metadata = into_outline(e).unwrap();
        let mut ret = Outline::new(title, vec![metadata]);
        for line in notes.lines() {
            ret.push_str(line);
        }
        ret
    }
}

impl LibraryEntry {
    /// Return whether the entry looks like it's describing a thing.
    ///
    /// Different sources don't guarantee either titles or uris, so entry validity can't be ensured
    /// at type level but must be verified after the fact.
    pub fn is_valid(&self) -> bool {
        self.title.is_some() || self.uri.is_some()
    }

    pub fn from_html(uri: &str, html: &str) -> LibraryEntry {
        use chrono::prelude::*;
        use select::{document::Document, predicate::Name};

        let doc = Document::from(html);

        let title: Option<String> = doc.find(Name("title")).next().map(|e| e.text());
        let localtime: DateTime<Local> = Local::now();

        LibraryEntry {
            uri: Some(uri.to_string()),
            added: Some(format!(
                "{}",
                localtime.to_rfc3339_opts(SecondsFormat::Secs, true)
            )),
            title,
            ..Default::default()
        }
    }
}

/// Wrapper that indicates that the contents are a potential scraping source.
///
/// Used as a TryFrom source for scraped formats.
#[derive(Clone, Debug)]
pub(crate) struct Scrapeable(pub String);

impl std::ops::Deref for Scrapeable {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Scrapeable {
    pub fn get(target: &str) -> Result<Scrapeable, Box<dyn Error>> {
        // TODO: Make timeout configurable in CLI parameters.
        // Timeout is needed if you hit a weird site like http://robpike.io
        const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

        if url::Url::parse(target).is_ok() {
            // Looks like a web page, try to download the contents.
            let request = reqwest::blocking::Client::new()
                .get(target)
                .timeout(REQUEST_TIMEOUT)
                .send()?;
            Ok(Scrapeable(request.text()?))
        } else {
            // Assume it's a file
            Ok(Scrapeable(std::fs::read_to_string(target)?))
        }
    }
}

pub fn scrape(target: &str) -> Result<Vec<LibraryEntry>, Box<dyn Error>> {
    let ret = Scrapeable::get(target)?;
    if let Ok(goodreads) = goodreads::Entries::try_from(&ret) {
        Ok(goodreads.0.into_iter().map(|x| x.into()).collect())
    } else if let Ok(pocket) = pocket::Entries::try_from(&ret) {
        Ok(pocket.0.into_iter().map(|x| x.into()).collect())
    } else if let Ok(pocket) = netscape_bookmarks::Entries::try_from(&ret) {
        Ok(pocket.0.into_iter().map(|x| x.into()).collect())
    } else if url::Url::parse(target).is_ok() {
        // Scrape generic webpage
        Ok(vec![LibraryEntry::from_html(target, &ret.0)])
    } else {
        Err("Couldn't scrape target")?
    }
}
