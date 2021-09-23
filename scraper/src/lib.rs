use base::{Symbol, VagueDate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::error::Error;

// FIXME: Re-enable these.
//mod goodreads;
//mod google_reader;
//mod netscape_bookmarks;
//mod pocket;

mod wayback;
pub use wayback::check_wayback;

pub type Uri = String;

/// Data for bookmarks and bibliography.
///
/// ```
/// let outline: (idm::Raw<String>, scraper::LibraryEntry) = idm::from_str("\
/// Feynman Lectures on Physics
///   uri: https://www.feynmanlectures.caltech.edu/
///   title: The Feynman Lectures on Physics
///   published: 1964
///   tags: physics
///   read: 2006-01-02").unwrap();
///
/// assert_eq!(outline.0.0, "Feynman Lectures on Physics");
/// assert_eq!(outline.1.uri, "https://www.feynmanlectures.caltech.edu/");
/// ```
#[derive(Default, Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct LibraryEntry {
    pub uri: Uri,
    pub title: Option<String>,
    pub author: Option<String>,
    #[serde(default)]
    pub tags: BTreeSet<Symbol>,
    /// Publication year of item
    pub published: Option<VagueDate>,
    /// When the item was read
    pub read: Option<VagueDate>,
    /// When the item was added to read list
    pub added: Option<VagueDate>,
    /// Preferred live location for dead links.
    ///
    /// If `mirror` is defined, assume the main `uri` is dead.
    pub mirror: Option<Uri>,
    /// Additional links
    #[serde(default)]
    pub links: Vec<Uri>,
    /// Where was this imported from
    pub via: Option<String>,
    pub _contents: Option<String>,
}

impl LibraryEntry {
    pub fn from_html(uri: &str, html: &str) -> LibraryEntry {
        use chrono::prelude::*;
        use select::{document::Document, predicate::Name};

        let doc = Document::from(html);

        let title: Option<String> =
            doc.find(Name("title")).next().map(|e| e.text());
        let localtime: DateTime<Local> = Local::now();
        let localtime: DateTime<FixedOffset> =
            localtime.with_timezone(localtime.offset());

        LibraryEntry {
            uri: uri.to_string(),
            added: Some(VagueDate::DateTime(localtime)),
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
    pub fn get(
        target: &str,
    ) -> Result<Scrapeable, Box<dyn Error + Send + Sync>> {
        // TODO: Make timeout configurable in CLI parameters.
        // Timeout is needed if you hit a weird site like http://robpike.io
        const REQUEST_TIMEOUT: std::time::Duration =
            std::time::Duration::from_secs(2);

        if let Ok(url) = url::Url::parse(target) {
            Ok(Scrapeable(download_page(url, REQUEST_TIMEOUT)?))
        } else {
            // Assume it's a file
            Ok(Scrapeable(std::fs::read_to_string(target)?))
        }
    }
}

pub fn download_page(
    url: url::Url,
    timeout: std::time::Duration,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let agent = ureq::AgentBuilder::new().timeout_read(timeout).build();

    Ok(agent.get(url.as_str()).call()?.into_string()?)
}

/*
pub fn scrape(target: &str) -> Result<Vec<LibraryEntry>, Box<dyn Error>> {
    let ret = Scrapeable::get(target)?;
    if let Ok(goodreads) = goodreads::Entries::try_from(&ret) {
        Ok(goodreads.0.into_iter().map(|x| x.into()).collect())
    } else if let Ok(google_takeout) = google_reader::GoogleReaderTakeout::try_from(&ret) {
        Ok(google_takeout.items.into_iter().map(|x| x.into()).collect())
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
*/
