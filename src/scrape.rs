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

pub fn scrape(target: &str) {
    if let Ok(mut goodreads) = try_goodreads(target) {
        // Oldest will be last, switch it to be first
        goodreads.reverse();

        for book in &goodreads {
            print!("{}", Outline::from(book));
        }
        println!();
    }
    log::info!("Unknown target '{}'", target);
}
/*
fn scrape_url(url: Url) -> Result<Outline, ()> {
}
*/
