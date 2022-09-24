use base::{Result, Symbol, VagueDate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

// TODO 2022-09-24 Rewrite this whole crate to mostly just handle HTTP
// queries, leave parsing to import crate.

mod wayback;
pub use wayback::check_wayback;

pub type Uri = String;

/// Data for bookmarks and bibliography.
///
/// ```
/// const ENTRY: &str = "\
/// Feynman Lectures on Physics
///   :uri https://www.feynmanlectures.caltech.edu/
///   :title The Feynman Lectures on Physics
///   :tags physics
///   :published 1964
///   :read 2006-01-02
/// ";
///
/// // XXX: Have the dummy `Vec<()>` parameter to coax LibraryEntry to use
/// // colon-prefixed syntax on reserialize.
///
/// let outline: (String, (scrape::LibraryEntry, Vec<()>)) = idm::from_str(ENTRY).unwrap();
///
/// assert_eq!(outline.0, "Feynman Lectures on Physics");
/// assert_eq!(outline.1.0.uri, "https://www.feynmanlectures.caltech.edu/");
///
/// let re_entry = idm::to_string(&outline).unwrap();
/// assert_eq!(re_entry, ENTRY);
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

// TODO: Rename to Scrapeable once old Scrapeable is gone
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum Scrapeable {
    File { filename: String, content: String },
    WebPage { url: url::Url, content: String },
}

impl Scrapeable {
    pub fn load(url: impl AsRef<str>) -> Result<Scrapeable> {
        use Scrapeable::*;
        let url = url.as_ref();

        // TODO: Make timeout configurable in CLI parameters.
        // Timeout is needed if you hit a weird site like http://robpike.io
        const REQUEST_TIMEOUT: std::time::Duration =
            std::time::Duration::from_secs(2);

        if url.find(':').is_none() {
            // No protocol, assume it's a file. Load from local file system.
            Ok(File {
                filename: url.to_string(),
                content: std::fs::read_to_string(url)?,
            })
        } else if url.starts_with("http:") || url.starts_with("https:") {
            let url: url::Url = url.parse()?;
            // Web page. Try to download over the internet.
            let agent = ureq::AgentBuilder::new()
                .timeout_read(REQUEST_TIMEOUT)
                .build();
            let content = agent.get(url.as_str()).call()?.into_string()?;
            Ok(WebPage { url, content })
        } else {
            Err(format!("Unknown protocol {:?}", url))?
        }
    }

    pub fn scrape(&self) -> Result<Vec<(String, (LibraryEntry, Vec<()>))>> {
        use Scrapeable::*;

        match self {
            File { filename, content } => todo!(),
            WebPage { url, content } => {
                use select::document::Document;
                use select::predicate::Name;

                // Grab web page title.
                let document = Document::from(content.as_ref());
                let title = document
                    .find(Name("title"))
                    .next()
                    .map(|n| n.text())
                    .unwrap_or_else(|| url.to_string());

                // Correct for weird stuff like multi-line text block for
                // title.
                let title = title.trim();
                let title = title
                    .lines()
                    .next()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| url.to_string());

                Ok(vec![(
                    title,
                    (
                        LibraryEntry {
                            uri: url.to_string(),
                            added: Some(VagueDate::now()),
                            ..Default::default()
                        },
                        Default::default(),
                    ),
                )])
            }
        }
    }
}

/*
pub fn scrape(target: &str) -> Result<Vec<LibraryEntry>> {
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
