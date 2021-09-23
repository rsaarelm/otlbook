use base::{Collection, Outline, Section};
use std::{
    collections::{BTreeSet, HashMap},
    time::Duration,
};
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
        Olt::Tags => tag_histogram(),
        Olt::Tagged { tags } => tag_search(tags),
    }
}

fn dupes() {
    let col = Collection::new().or_die();
    let mut count = HashMap::new();

    log::info!("Start WikiTitle crawl");
    for section in col.outline().walk() {
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
    for section in col.outline().walk() {
        if let Ok(Some(uri)) = section.1.attr::<String>("uri") {
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
    let col = Collection::new().or_die();

    log::info!("Start URI search");
    for Section(head, body) in col.outline().walk() {
        if let Ok(Some(u)) = body.attr::<String>("uri") {
            if u == uri {
                println!("Found! {:?}", head);
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
    let page = scraper::download_page(
        url::Url::parse(&target).expect("Invalid URL"),
        Duration::new(5, 0),
    )
    .expect("Scrape failed");

    println!("{:?}", page);
}

fn tag_search(tags: Vec<String>) {
    let tags = tags.into_iter().collect::<BTreeSet<_>>();
    let col = Collection::new().or_die();

    fn crawl(
        search_tags: &BTreeSet<String>,
        inherited_tags: &BTreeSet<String>,
        current: &Outline,
    ) {
        for sec in current.iter() {
            // Only look for articles
            if sec.is_article() {
                let tags = if let Ok(Some(tags)) =
                    sec.1.attr::<BTreeSet<String>>("tags")
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
                    println!(
                        "{}",
                        idm::to_string(&Outline(vec![sec.clone()]))
                            .unwrap_or("err".to_string())
                    );

                    // Don't crawl into children that might also match, we
                    // already printed them.
                    continue;
                }

                crawl(search_tags, &tags, &sec.1);
            } else {
                crawl(search_tags, inherited_tags, &sec.1);
            }
        }
    }

    crawl(&tags, &BTreeSet::new(), col.outline());
}

fn tag_histogram() {
    let col = Collection::new().or_die();

    let mut hist = HashMap::new();
    log::info!("Start URI search");
    for Section(_, body) in col.outline().walk() {
        if let Ok(Some(ts)) = body.attr::<BTreeSet<String>>("tags") {
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
