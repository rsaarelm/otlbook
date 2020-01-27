use chrono::prelude::*;
use parser::outline::Outline;
use serde::Deserialize;
use std::error::Error;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Goodreads {
    #[serde(rename = "Title")]
    title: String,
    #[serde(rename = "Author")]
    author: String,
    #[serde(rename = "ISBN")]
    isbn: String,
    #[serde(rename = "ISBN13")]
    isbn13: String,
    #[serde(rename = "Year Published")]
    year_published: String,
    #[serde(rename = "Date Added")]
    date_added: String,
    #[serde(rename = "Date Read")]
    date_read: String,
    #[serde(rename = "Bookshelves")]
    bookshelves: String,
    #[serde(rename = "Private Notes")]
    notes: String,
}

impl From<&Goodreads> for Outline {
    fn from(g: &Goodreads) -> Outline {
        let mut ret = Outline::new(g.title.clone(), Vec::new());
        ret.push_str(format!("title {}", g.title));
        ret.push_str(format!("author {}", g.author));
        if !g.isbn13.is_empty() {
            ret.push_str(format!(
                "uri isbn:{}",
                g.isbn13.replace("\"", "").replace("=", "")
            ));
        } else {
            log::warn!("ISBN missing for book '{}'", g.title);
        }
        if !g.year_published.is_empty() {
            ret.push_str(format!("year {}", g.year_published));
        }
        if !g.date_added.is_empty() {
            ret.push_str(format!("added {}", g.date_added.replace("/", "-")));
        }
        if !g.date_read.is_empty() {
            ret.push_str(format!("read {}", g.date_read.replace("/", "-")));
        }

        if !g.bookshelves.is_empty() {
            let mut tags = String::new();
            for tag in g.bookshelves.split(", ") {
                if !tags.is_empty() {
                    tags.push_str(" ");
                }
                tags.push_str(tag);
            }
            ret.push_str(format!("tags {}", tags));
        }

        if !g.notes.is_empty() {
            ret.push_str(format!("notes {}", g.notes));
        }

        ret
    }
}

pub fn try_goodreads(path: impl AsRef<Path>) -> Result<Vec<Goodreads>, Box<dyn Error>> {
    let mut rdr = csv::Reader::from_path(path)?;
    let mut ret = Vec::new();
    for result in rdr.deserialize() {
        ret.push(result?);
    }
    Ok(ret)
}

pub fn try_netscape_bookmarks(path: impl AsRef<Path>) -> Result<Outline, Box<dyn Error>> {
    use select::document::Document;
    use select::predicate::Name;

    let text = std::fs::read_to_string(path)?;
    if !text.starts_with("<!DOCTYPE NETSCAPE-Bookmark") {
        return Err("not a bookmark file")?;
    }
    let doc = Document::from(text.as_ref());

    let mut ret = vec![];

    let mut node = doc.find(Name("dt")).next();
    while let Some(item) = node {
        if let Some("dt") = item.name() {
            // TODO: Replace panicing unwraps with error handling.
            let a = item.find(Name("a")).next().unwrap();
            let title = a.text();
            ret.insert(0, Outline::new(&title, vec![]));
            ret[0].push_str(format!("title {}", title));
            ret[0].push_str(format!("uri {}", a.attr("href").unwrap()));
            let add_date = a.attr("add_date").unwrap().parse::<i64>().unwrap();
            let add_date = Utc
                .timestamp(add_date, 0)
                .to_rfc3339_opts(SecondsFormat::Secs, true);
            ret[0].push_str(format!("added {}", add_date));
            ret[0].push_str(format!(
                "tags {}",
                a.attr("tags").unwrap().replace(",", " ")
            ));
        }
        if let Some("dd") = item.name() {
            if ret.is_empty() {
                log::warn!("Malformed bookmark file");
                continue;
            }
            ret[0].push(Outline::new(
                "quote:",
                item.text()
                    .lines()
                    .map(|s| Outline::new(s, vec![]))
                    .collect(),
            ));
        }
        node = item.next();
    }

    Ok(Outline {
        headline: None,
        children: ret,
    })
}

pub fn try_url(maybe_url: &str) -> Result<Outline, Box<dyn Error>> {
    use select::document::Document;
    use select::predicate::Name;

    let body = reqwest::blocking::get(maybe_url)?.text()?;
    let doc = Document::from(body.as_ref());

    let title: Option<String> = doc.find(Name("title")).next().map(|e| e.text());

    let mut ret = Outline::new(title.as_ref().map_or(maybe_url, |s| s.as_ref()), Vec::new());
    if let Some(title) = title {
        ret.push_str(format!("title {}", title));
    }
    ret.push_str(format!("uri {}", maybe_url));

    let localtime: DateTime<Local> = Local::now();
    ret.push_str(format!(
        "added {}",
        localtime.to_rfc3339_opts(SecondsFormat::Secs, true)
    ));

    Ok(ret)
}

pub fn scrape(target: &str) {
    if let Ok(outline) = try_url(target) {
        print!("{}", outline);
    } else if let Ok(outline) = try_netscape_bookmarks(target) {
        print!("{}", outline);
    } else if let Ok(mut goodreads) = try_goodreads(target) {
        // Oldest will be last, switch it to be first
        goodreads.reverse();

        for book in &goodreads {
            print!("{}", Outline::from(book));
        }
        println!();
    }
    log::info!("Unknown target '{}'", target);
}
