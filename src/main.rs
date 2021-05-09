use base::Section;
use serde_derive::Deserialize;
use std::collections::{HashMap, BTreeSet};
use structopt::StructOpt;

fn main() {
    env_logger::init();

    match Olt::from_args() {
        Olt::Exists { uri } => exists(uri),
        Olt::Dupes => dupes(),
        Olt::Tags => tag_histogram(),
    }
}

fn exists(uri: String) {
    let otl = base::load_collection().unwrap();

    #[derive(Eq, PartialEq, Deserialize)]
    struct Uri {
        uri: String,
    }

    log::info!("Start URI search");
    for Section(head, body) in otl.iter() {
        if let Some(Uri { uri: u }) = body.try_into() {
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

fn dupes() {
    let otl = base::load_collection().unwrap();
    let mut count = HashMap::new();

    log::info!("Start WikiTitle crawl");
    for section in otl.iter() {
        if let Some(title) = section.wiki_title() {
            *count.entry(title).or_insert(0) += 1;
        }
    }
    log::info!("Finished WikiTitle crawl, {} titles", count.len());

    for (t, &n) in &count {
        if n > 1 {
            println!("{}", t);
        }
    }
}

fn tag_histogram() {
    let otl = base::load_collection().unwrap();

    #[derive(Eq, PartialEq, Deserialize)]
    struct Tags {
        tags: BTreeSet<String>,
    }

    let mut hist = HashMap::new();
    log::info!("Start URI search");
    for Section(_, body) in otl.iter() {
        if let Some(Tags { tags: ts }) = body.try_into() {
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

#[derive(StructOpt, Debug)]
#[structopt(name = "olt", about = "Outline file processing tool")]
enum Olt {
    #[structopt(
        name = "uri-exists",
        about = "Check if URI is saved in collection"
    )]
    Exists {
        #[structopt(parse(from_str))]
        uri: String,
    },
    #[structopt(name = "dupes", about = "List duplicate entries")]
    Dupes,
    #[structopt(name = "tags", about = "Show tag cloud")]
    Tags,
}
