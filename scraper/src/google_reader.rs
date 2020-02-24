//! Google reader JSON takeout

use crate::{LibraryEntry, Scrapeable, VagueDate};
use serde::Deserialize;
use std::convert::TryFrom;
use std::error::Error;
use std::io::Write;
use std::process::{Command, Stdio};

// Don't convert the cached contents into notes, this makes for larger files VimOutliner can
// handle. Notes isn't really supposed to be a mirroring mechanism, more for handwritten stuff.
const PANDOC_NOTES: bool = false;

#[derive(Debug, Deserialize)]
pub struct GoogleReaderTakeout {
    // Put a bunch of fields here just to make it harder to accidentally match a different JSON.
    id: String,
    title: String,
    updated: i64,
    direction: String,
    pub items: Vec<GoogleReaderItem>,
}

#[derive(Debug, Deserialize)]
pub struct GoogleReaderItem {
    pub id: String,
    pub categories: Vec<String>,
    pub published: i64,
    pub title: Option<String>,
    pub canonical: Option<Vec<GoogleReaderSource>>,
    pub alternate: Vec<GoogleReaderSource>,
    pub content: Option<GoogleReaderContent>,
    pub author: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GoogleReaderContent {
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct GoogleReaderSource {
    pub href: String,
}

impl TryFrom<&Scrapeable> for GoogleReaderTakeout {
    type Error = Box<dyn Error>;

    fn try_from(s: &Scrapeable) -> Result<Self, Self::Error> {
        let mut ret: Self = serde_json::from_str(&s.0)?;
        ret.items.sort_by_key(|e: &GoogleReaderItem| e.published);
        Ok(ret)
    }
}

impl From<GoogleReaderItem> for LibraryEntry {
    fn from(e: GoogleReaderItem) -> LibraryEntry {
        let mut ret = LibraryEntry::default();
        if let Some(canonical) = e.canonical {
            ret.uri = canonical
                .iter()
                .next()
                .map(|a| a.href.clone())
                .unwrap_or("err".into());
        } else if let Some(alt) = e.alternate.iter().next() {
            ret.uri = alt.href.clone();
        }

        ret.title = e.title;
        ret.published = Some(VagueDate::from_timestamp(e.published));
        ret.author = e.author;

        if PANDOC_NOTES {
            // XXX: Hacky. Use systen Pandoc to convert content to Markdown
            // Also very slow. Blank out the content field beforehand for faster conversion?
            if let Some(content) = e.content {
                let content = content.content;

                if let Ok(md) = html2md(content.as_str()) {
                    ret.notes = Some(md);
                }
            }
        }

        ret
    }
}

fn html2md(html: &str) -> Result<String, Box<dyn Error>> {
    // Could also use some Markdown for the output, though that tends to be noisier. Plaintext
    // version loses URLs.
    let mut pandoc = Command::new("pandoc")
        .arg("-f")
        .arg("html")
        .arg("-t")
        .arg("plain")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    pandoc
        .stdin
        .as_mut()
        .ok_or("Proc err")?
        .write_all(html.as_bytes())?;

    let output = pandoc.wait_with_output()?;
    if output.status.success() {
        let output = String::from_utf8(output.stdout)?;
        return Ok(output);
    } else {
        return Err("pandoc failed")?;
    }
}
