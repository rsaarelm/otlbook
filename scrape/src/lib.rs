use std::collections::HashMap;

use base::Result;
use serde::Deserialize;

// TODO: Make timeout configurable in CLI parameters.
// Timeout is needed if you hit a weird site like http://robpike.io
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// Try to download a web page from the internet.
pub fn download_web_page(url: impl AsRef<str>) -> Result<String> {
    let url: url::Url = url.as_ref().parse()?;
    let agent = ureq::AgentBuilder::new()
        .timeout_read(REQUEST_TIMEOUT)
        .build();
    Ok(agent.get(url.as_str()).call()?.into_string()?)
}

/// Get possibly redirected url.
pub fn final_url(url: impl AsRef<str>) -> Result<String> {
    let url: url::Url = url.as_ref().parse()?;
    let agent = ureq::AgentBuilder::new()
        .timeout_read(REQUEST_TIMEOUT)
        .build();
    Ok(agent.get(url.as_str()).call()?.get_url().into())
}

/// Helper function for parsing the title only.
///
/// A lot of the time you only want this.
pub fn web_page_title(url: impl AsRef<str>) -> Result<Option<String>> {
    use select::{document::Document, predicate::Name};

    let content = download_web_page(url)?;
    let document = Document::from(content.as_ref());

    let title = document
        .find(Name("title"))
        .next()
        .map(|n| n.text())
        .unwrap_or_else(Default::default);

    // Correct for weird stuff like multi-line text block for
    // title.
    let title = title.trim();
    let title = title
        .lines()
        .next()
        .map(|s| s.to_string())
        .unwrap_or_else(Default::default);

    if title.is_empty() {
        Ok(None)
    } else {
        Ok(Some(title))
    }
}

pub fn is_archived_on_wayback(url: impl AsRef<str>) -> Result<bool> {
    #[derive(Deserialize)]
    #[allow(dead_code)]
    struct WaybackAvailable {
        url: String,
        archived_snapshots: HashMap<String, Snapshot>,
    }

    #[derive(Deserialize)]
    #[allow(dead_code)]
    struct Snapshot {
        status: String,
        available: bool,
        url: String,
        timestamp: String,
    }

    // Make sure the initial parameter looks like an URL, then throw this
    // value away. It's only here to see if the parse succeeds.
    let url: url::Url = url.as_ref().parse()?;

    // Then make the actual URL that's querying wayback machine.
    let url: url::Url =
        format!("https://archive.org/wayback/available?url={}", url.as_ref())
            .parse()?;

    let agent = ureq::AgentBuilder::new()
        .timeout_read(REQUEST_TIMEOUT)
        .build();

    let response: WaybackAvailable =
        agent.get(url.as_str()).call()?.into_json()?;
    Ok(response
        .archived_snapshots
        .get("closest")
        .map_or(false, |e| e.available))
}
