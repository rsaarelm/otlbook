use parser::{outline::Outline, Symbol, VagueDate};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::error::Error;

mod goodreads;
mod google_reader;
mod netscape_bookmarks;
mod pocket;

mod wayback;
pub use wayback::check_wayback;

pub type Uri = String;

/// Data for bookmarks and bibliography.
///
/// ```
/// use parser::outline::Outline;
///
/// let outline = Outline::from("\
/// Feynman Lectures on Physics
/// \t\turi https://www.feynmanlectures.caltech.edu/
/// \t\ttitle The Feynman Lectures on Physics
/// \t\tyear 1964
/// \t\ttags physics
/// \t\tread 2006-01-02").children[0].clone();
///
/// assert!(outline.extract::<scraper::LibraryEntry>().is_some());
/// ```
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct LibraryEntry {
    pub uri: Uri,
    pub title: Option<String>,
    pub author: Option<String>,
    #[serde(default)]
    pub tags: Vec<Symbol>,
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
        } else {
            e.uri.clone()
        };

        // Don't want notes text copied in the metadata
        e.notes = None;

        let mut ret = Outline::new(title, vec![]);
        ret.inject(e);
        notes.lines().for_each(|line| ret.push_str(line));

        ret
    }
}

impl LibraryEntry {
    pub fn from_html(uri: &str, html: &str) -> LibraryEntry {
        use chrono::prelude::*;
        use select::{document::Document, predicate::Name};

        let doc = Document::from(html);

        let title: Option<String> = doc.find(Name("title")).next().map(|e| e.text());
        let localtime: DateTime<Local> = Local::now();
        let localtime: DateTime<FixedOffset> = localtime.with_timezone(localtime.offset());

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
