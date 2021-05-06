use serde_derive::Deserialize;
use structopt::StructOpt;

fn main() {
    env_logger::init();

    match Olt::from_args() {
        Olt::Exists { uri } => exists(uri),
    }
}

fn exists(uri: String) {
    log::info!("Loading collection");
    let otl = base::load_collection().unwrap();
    log::info!("Collection loaded, {} entries", otl.count());

    #[derive(Eq, PartialEq, Deserialize)]
    struct Uri {
        uri: String,
    }

    log::info!("Starting URI search...");
    for (head, body) in otl.iter() {
        if let Some(Uri { uri: u }) = body.try_into() {
            if u == uri {
                println!("Found! {:?}", head);
                log::info!("Search successful");
                return;
            }
        }
    }

    log::info!("Search failed");
    println!("Not found");
    std::process::exit(1);
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
}
