use base::{Collection, Result, Section};
use scraper::LibraryEntry;
use std::collections::{BTreeSet, HashMap};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "olt", about = "Outline file processing tool")]
enum Olt {
    #[structopt(name = "dupes", about = "List duplicate entries")]
    Dupes,
    #[structopt(
        name = "uri-exists",
        about = "Check if URI is saved in collection"
    )]
    Exists {
        #[structopt(parse(from_str))]
        uri: String,
    },
    #[structopt(
        name = "server",
        about = "Run the otlbook web server for the current collection"
    )]
    Server {
        #[structopt(default_value = "8080")]
        port: u32,
    },
    #[structopt(name = "tags", about = "Show tag cloud")]
    Tags,
    #[structopt(name = "tagged", about = "List items with given tags")]
    Tagged {
        #[structopt(parse(from_str), required = true)]
        tags: Vec<String>,
    },
    #[structopt(name = "toread", about = "Save a link in the to-read queue")]
    ToRead { uri: String },
}

fn main() {
    env_logger::init();

    match Olt::from_args() {
        Olt::Dupes => dupes(),
        Olt::Exists { uri } => exists(uri),
        Olt::Server { port } => {
            webserver::run(port).unwrap();
        }
        Olt::Tags => tag_histogram(),
        Olt::Tagged { tags } => tag_search(tags),
        Olt::ToRead { uri } => save_to_read(uri),
    }
}

fn dupes() {
    let col = Collection::load().or_die();
    let mut count = HashMap::new();

    log::info!("Start WikiTitle crawl");
    for section in col.iter() {
        if let Some(title) = section.wiki_title() {
            *count.entry(title).or_insert(0) += 1;
        }
    }
    log::info!("Finished WikiTitle crawl, {} titles", count.len());

    for (t, &n) in &count {
        if n > 1 {
            println!("WikiWord dupes: {}", t);
        }
    }

    log::info!("Start uri crawl");
    let mut count = HashMap::new();
    for section in col.iter() {
        if let Ok(Some(uri)) = section.attr::<String>("uri") {
            *count.entry(uri).or_insert(0) += 1;
        }
    }
    log::info!("Finished uri crawl, {} items;", count.len());

    for (t, &n) in &count {
        if n > 1 {
            println!("uri dupes: {}", t);
        }
    }
}

fn exists(uri: String) {
    let col = Collection::load().or_die();

    log::info!("Start URI search");
    for section in col.iter() {
        if let Ok(Some(u)) = section.attr::<String>("uri") {
            if u == uri {
                println!("Found! {:?}", section.headline());
                log::info!("URI search successful");
                return;
            }
        }
    }

    log::info!("Failed URI search");
    println!("Not found");
    std::process::exit(1);
}

fn scrape(target: String) -> Result<(String, LibraryEntry)> {
    let page = scraper::Scrapeable::load(target)?;
    Ok(page.scrape()?.into_iter().next().expect("Failed to scrape"))
}

fn tag_search(tags: Vec<String>) {
    let tags = tags.into_iter().collect::<BTreeSet<_>>();
    let col = Collection::load().or_die();

    fn crawl(
        search_tags: &BTreeSet<String>,
        inherited_tags: &BTreeSet<String>,
        current: &Section,
    ) {
        if current.is_article() {
            let tags = current
                .attr::<BTreeSet<String>>("tags")
                .ok()
                .flatten()
                .unwrap_or_else(|| Default::default())
                .union(inherited_tags)
                .cloned()
                .collect::<BTreeSet<String>>();

            if search_tags.is_subset(&tags) {
                // Found!
                println!("{}", current);

                // Don't crawl into children that might also match, we
                // already printed them.
                return;
            }

            for sec in current.children() {
                crawl(search_tags, &tags, &sec);
            }
        } else {
            for sec in current.children() {
                crawl(search_tags, inherited_tags, &sec);
            }
        }
    }

    for root in col.roots() {
        crawl(&tags, &BTreeSet::new(), &root);
    }
}

fn tag_histogram() {
    let col = Collection::load().or_die();

    let mut hist = HashMap::new();
    log::info!("Start URI search");
    for section in col.iter() {
        if let Ok(Some(ts)) = section.attr::<BTreeSet<String>>("tags") {
            for t in &ts {
                *hist.entry(t.to_string()).or_insert(0) += 1;
            }
        }
    }

    // Sort by largest first
    for (n, t) in &hist
        .into_iter()
        .map(|(t, n)| (-(n as i32), t))
        .collect::<BTreeSet<_>>()
    {
        println!("{}  {}", t, -n);
    }
}

fn save_to_read(uri: String) {
    let mut col = Collection::load().or_die();

    let section_data = scrape(uri).or_die();
    let scraped_uri = &section_data.1.uri;

    // TODO: Use a compact API in collection to search this.
    log::info!("Start URI search");
    for section in col.iter() {
        if let Ok(Some(u)) = section.attr::<String>("uri") {
            if &u == scraped_uri {
                log::info!("URI search successful");
                eprintln!(
                    "Uri {:?} already present in collection.",
                    scraped_uri
                );
                std::process::exit(1);
            }
        }
    }

    log::info!("URI not found, scraping new entry");
    let entry = Section::from_data(&section_data).or_die();

    let to_read = col.find_or_create("ToRead");
    to_read.append(entry);

    col.save().or_die();
}

/// Trait for top-level error handling.
pub trait OrDie {
    type Value;

    fn or_die(self) -> Self::Value;
}

impl<T, E: std::fmt::Display> OrDie for std::result::Result<T, E> {
    type Value = T;

    fn or_die(self) -> Self::Value {
        match self {
            Ok(val) => val,
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    }
}
