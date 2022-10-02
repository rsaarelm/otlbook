use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    io::{prelude::*, stdin},
    path::{Path, PathBuf},
};

use base::{Collection, Result, Section};
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
    #[structopt(
        name = "exists",
        about = "Check if a given entity already exists in the notebook"
    )]
    Exists {
        #[structopt(parse(from_str))]
        uri: String,
    },
    #[structopt(
        name = "import",
        about = "Import entries from other formats and print to stdout"
    )]
    Import {
        #[structopt(parse(from_str), required = true)]
        path: PathBuf,
        #[structopt(
            about = "Import to-read items instead of already read items",
            long = "to-read"
        )]
        to_read: bool,
    },
    #[structopt(
        name = "insert",
        about = "Insert items read from stdin to notebook if they're not entities already in it"
    )]
    Insert,
    #[structopt(
        name = "reinsert",
        about = "Rewrite existing entities in notebook read from stdin, insert other items that are not existing entities"
    )]
    Reinsert,
    #[structopt(name = "tagged", about = "List items with given tags")]
    Tagged {
        #[structopt(parse(from_str), required = true)]
        tags: Vec<String>,
    },
    #[structopt(name = "tags", about = "Show tag cloud")]
    Tags,
    #[structopt(name = "toread", about = "Save a link in the to-read queue")]
    ToRead { uri: String },
    #[structopt(
        name = "webserver",
        about = "Run the otlbook web server for the current collection"
    )]
    Webserver {
        #[structopt(default_value = "8080")]
        port: u32,
    },
}

fn main() {
    env_logger::init();

    match Olt::from_args() {
        Olt::Dupes => dupes(),
        Olt::Exists { uri } => exists(uri),
        Olt::Import {
            path,
            to_read: to_reads,
        } => import(path, to_reads),
        Olt::Insert => insert(),
        Olt::Reinsert => reinsert(),
        Olt::Tagged { tags } => tag_search(tags),
        Olt::Tags => tag_histogram(),
        Olt::ToRead { uri } => save_to_read(uri),
        Olt::Webserver { port } => {
            webserver::run(port, Collection::load().or_die())
        }
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

fn import(path: impl AsRef<Path>, import_to_reads: bool) {
    let text = fs::read_to_string(path).or_die();
    // TODO 2022-10-01 Support other types than Pocket (eg. Goodreads)

    let collection = if import_to_reads {
        import::pocket::import_to_read(&text).or_die()
    } else {
        import::pocket::import_read(&text).or_die()
    };

    print!("{}", idm::to_string(&collection).or_die());
}

fn insert() {
    let mut col = Collection::load().or_die();

    let mut buf = String::new();
    stdin().read_to_string(&mut buf).or_die();
    // XXX: Need to trim the buf or I'll end up with a blank section in the
    // end.
    let items: Vec<Section> = idm::from_str(buf.trim_end()).or_die();

    let existing_entities = col
        .iter()
        .filter_map(|s| s.entity_identifier())
        .collect::<HashSet<_>>();

    let inbox = col.find_or_create("InBox");

    let mut count = 0;
    for sec in &items {
        if let Some(id) = sec.entity_identifier() {
            if existing_entities.contains(&id) {
                eprintln!("{:?} already present, skipping", id);
                continue;
            }
        }
        count += 1;
        inbox.append(sec.clone());
    }

    col.save().or_die();

    if count > 0 {
        eprintln!("Inserted {} new items", count);
    }
}

fn reinsert() {
    todo!();
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
                println!("{}", current.borrow().headline);
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
    todo!();
    /*
    let mut col = Collection::load().or_die();

    let section_data = scrape(uri).or_die();
    let scraped_uri = &section_data.1 .0.uri;

    // TODO 2022-10-01 See insert for a more up to date way to do this...
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
    */
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
