use base::{Collection, Section};
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
        name = "scrape",
        about = "Scrape an external resource into references"
    )]
    Scrape {
        #[structopt(parse(from_str))]
        target: String,
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
}

fn main() {
    env_logger::init();

    match Olt::from_args() {
        Olt::Dupes => dupes(),
        Olt::Exists { uri } => exists(uri),
        Olt::Scrape { target } => scrape(target),
        Olt::Server { port } => {
            webserver::run(port).unwrap();
        }
        Olt::Tags => tag_histogram(),
        Olt::Tagged { tags } => tag_search(tags),
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

fn scrape(target: String) {
    let page = scraper::Scrapeable::load(target).expect("Invalid URL");
    let entry = page
        .scrape()
        .expect("Failed to scrape")
        .into_iter()
        .next()
        .expect("Failed to scrape");

    print!(
        "{}",
        idm::to_string_styled(idm::Style::Tabs, &entry).unwrap()
    );
}

fn tag_search(tags: Vec<String>) {
    let tags = tags.into_iter().collect::<BTreeSet<_>>();
    let col = Collection::load().or_die();

    fn crawl(
        search_tags: &BTreeSet<String>,
        inherited_tags: &BTreeSet<String>,
        current: &Section,
    ) {
        todo!();
        // FIXME, figure out recursion logic for new Section type
        /*
        for sec in current.iter() {
            // Only look for articles
            if sec.is_article() {
                let tags = if let Ok(Some(tags)) =
                    sec.attr::<BTreeSet<String>>("tags")
                {
                    tags
                } else {
                    Default::default()
                }
                .union(inherited_tags)
                .cloned()
                .collect::<BTreeSet<String>>();

                if search_tags.is_subset(&tags) {
                    // Found!
                    println!("{}", sec);

                    // Don't crawl into children that might also match, we
                    // already printed them.
                    continue;
                }

                crawl(search_tags, &tags, &sec);
            } else {
                crawl(search_tags, inherited_tags, &sec);
            }
        }
        */
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

/// Trait for top-level error handling.
pub trait OrDie {
    type Value;

    fn or_die(self) -> Self::Value;
}

impl<T, E: std::fmt::Display> OrDie for Result<T, E> {
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
