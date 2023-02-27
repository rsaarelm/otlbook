use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    io::{prelude::*, stdin},
    path::{Path, PathBuf},
};

use base::{Collection, Section, VagueDate};
use indexmap::IndexMap;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "olt", about = "Outline file processing tool")]
enum Olt {
    #[structopt(
        name = "dump",
        about = "Dump all articles in JSON for externasl tools"
    )]
    Dump,
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
    Insert {
        #[structopt(
            about = "Folder path to insert the items under",
            long = "under"
        )]
        under: Option<String>,
    },
    #[structopt(
        name = "reinsert",
        about = "Rewrite existing entities in notebook read from stdin, insert other items that are not existing entities"
    )]
    #[structopt(
        name = "normalize",
        about = "Load and rewrite entire notebook in normal form"
    )]
    Normalize,
    Reinsert,
    #[structopt(
        name = "scrape",
        about = "Fetch data from URL and print IDM entry to stdout"
    )]
    Scrape {
        url: String,
    },
    #[structopt(name = "tagged", about = "List items with given tags")]
    Tagged {
        #[structopt(parse(from_str), required = true)]
        tags: Vec<String>,
    },
    #[structopt(name = "tags", about = "Show tag cloud")]
    Tags,
    #[structopt(name = "toread", about = "Save a link in the to-read queue")]
    ToRead {
        uri: String,
    },
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
        Olt::Dump => dump(),
        Olt::Dupes => dupes(),
        Olt::Exists { uri } => exists(uri),
        Olt::Import {
            path,
            to_read: to_reads,
        } => import(path, to_reads),
        Olt::Insert { under } => insert(under),
        Olt::Normalize => normalize(),
        Olt::Reinsert => reinsert(),
        Olt::Scrape { url } => scrape(url),
        Olt::Tagged { tags } => tag_search(tags),
        Olt::Tags => tag_histogram(),
        Olt::ToRead { uri } => save_to_read(uri),
        Olt::Webserver { port } => {
            webserver::run(port, Collection::load().or_die())
        }
    }
}

fn dump() {
    use serde_json::{Map, Value};

    let col = Collection::load().or_die();

    let mut array = Vec::new();
    for article in col.iter().filter(|a| a.is_article()) {
        let mut entry = Map::default();
        // Initially override title with the headline.
        //
        // Currently headline will just be thrown out if the title is
        // redefined, might put it in a separate field in the future in the
        // case the values redefine title.
        entry.insert("title".into(), article.title().into());

        // Tags can be inherited from parent nodes, so add them explicitly.
        entry.insert(
            "tags".into(),
            Value::Array(
                article.tags().into_iter().map(|a| a.into()).collect(),
            ),
        );

        for (key, val) in article.borrow().attributes.iter() {
            if key == "tags" {
                // Skip tags when processing the remaining attrs
                continue;
            } else {
                entry.insert(key.into(), val.clone().into());
            }
        }
        array.push(entry);
    }

    print!("{}", serde_json::to_string_pretty(&array).or_die());
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

fn insert(under: Option<String>) {
    let mut col = Collection::load().or_die();

    let mut buf = String::new();
    stdin().read_to_string(&mut buf).or_die();
    // TODO: Trim-if-multiple-lines helper function
    //
    // Multiline input needs to be trimmed so I won't get an empty element.
    // Single-line input needs to keep a trailing newline
    let buf = if buf.trim_end().contains('\n') {
        buf.trim_end()
    } else {
        &buf
    };
    let items: Vec<Section> = idm::from_str(buf).or_die();

    let existing_entities = col
        .iter()
        .filter_map(|s| s.entity_identifier())
        .collect::<HashSet<_>>();

    let path = if let Some(path) = under {
        path
    } else {
        "InBox".to_string()
    };

    let parent = col.find_or_create(&path).or_die();

    let mut count = 0;
    for sec in &items {
        if let Some(id) = sec.entity_identifier() {
            if existing_entities.contains(&id) {
                eprintln!("{:?} already present, skipping", id);
                continue;
            }
        }
        count += 1;
        parent.append(sec.clone());
    }

    col.save().or_die();

    if count > 0 {
        eprintln!("Inserted {} new items", count);
    }
}

fn normalize() {
    let mut col = Collection::load().or_die();
    for root in col.roots() {
        root.taint();
    }
    col.save().or_die();
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

fn retitle() {
    let mut col = Collection::load().or_die();

    for mut item in col.iter() {
        if let Ok(Some(uri)) = item.attr::<String>("uri") {
            let title = item.title();
            if title == uri {
                if let Ok(Some(title)) = scrape::web_page_title(uri.clone()) {
                    eprintln!("{} -> {}", uri, title);
                    item.set_title(title);
                } else {
                    eprintln!("\x1b[1;31mFailed to improve {}\x1b[0m", uri);
                }
            }
        }
    }

    col.save().or_die();
}

fn reurl() {
    let mut col = Collection::load().or_die();

    for mut item in col.iter() {
        if let Ok(Some(_)) = item.attr::<String>("mirror") {
            // Assume items with a mirror attribute are known to be dead.
            continue;
        }

        if let Ok(Some(tags)) = item.attr::<BTreeSet<String>>("tags") {
            if tags.contains(&"dead-link".to_owned()) {
                // Assume link is known to be dead and mirror-less.
                continue;
            }
        }

        if let Ok(Some(uri)) = item.attr::<String>("uri") {
            // TODO 2022-10-02 More principled HTML URI detector
            if !uri.starts_with("http") {
                // Not HTML, skip.
                continue;
            }

            if uri.starts_with("https://doi.org/") {
                // DOI links need to stay as they are, skip.
                continue;
            }

            if let Ok(new_url) = scrape::final_url(uri.clone()) {
                if new_url != uri {
                    eprintln!("{:?} -> {:?}", uri, new_url);
                    item.set_attr("uri", &new_url).or_die();
                }
            } else {
                eprintln!("\x1b[1;31mFailed to scan {}\x1b[0m", uri);
            }
        }
    }

    col.save().or_die();
}

fn scrape(uri: String) {
    if uri.starts_with("isbn:") {
        todo!("Book scraping");
    }

    let mut title = uri.clone();

    if let Some(page_title) = scrape::web_page_title(title.clone()).or_die() {
        title = page_title;
    }

    let node = Section::new(
        title,
        IndexMap::from([
            ("uri".to_string(), uri),
            ("added".to_string(), VagueDate::now().to_string()),
        ]),
    );

    print!("{}", idm::to_string(&node).or_die());
}

fn save_bookmark(uri: String) {
    todo!();
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
