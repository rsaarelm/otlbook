use serde::Deserialize;
use std::error::Error;

#[derive(Eq, PartialEq, Debug, Deserialize)]
struct WaybackAvailable {
    archived_snapshots: WaybackSnapshots,
}

#[derive(Eq, PartialEq, Debug, Deserialize)]
struct WaybackSnapshots {
    closest: Option<WaybackSnapshot>,
}

#[derive(Eq, PartialEq, Debug, Deserialize)]
struct WaybackSnapshot {
    available: bool,
    url: String,
    // XXX: Could use chrono::serde if this was an integer in the JSON. As it stands, just leave it
    // as String, don't care that much.
    timestamp: String,
    status: String,
}

pub fn is_archived_on_wayback(url: &str) -> Result<bool, Box<dyn Error>> {
    todo!() // rewrite in ureq
            /*
            if url::Url::parse(url).is_err() {
                return Err("Not a valid URL".into());
            }

            let result: WaybackAvailable = reqwest::blocking::get(&format!(
                "https://archive.org/wayback/available?url={}",
                url
            ))?
            .json()?;

            Ok(result.archived_snapshots.closest.is_some())
            */
}

pub fn generate_wayback_save_url(url: &str) -> String {
    format!("https://web.archive.org/save/{}", url)
}

/// Print an archiving link for a URL that is not present on wayback machine yet
pub fn check_wayback(target: &str) {
    if url::Url::parse(target).is_err() {
        log::info!("target is not a URL, not checking wayback machine");
        return;
    }
    if let Ok(false) = is_archived_on_wayback(target) {
        println!("URL not found on wayback machine");
        println!(
            "Click to save it now: {}",
            generate_wayback_save_url(target)
        );
    } else {
        log::info!("URL {} found on wayback machine", target);
    }
}
