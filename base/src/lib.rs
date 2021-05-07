use std::{convert::TryFrom, ffi::OsStr, iter::FromIterator, path::PathBuf};

mod outline;
pub use outline::{Outline, Section};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn load_collection() -> Result<Outline> {
    use walkdir::WalkDir;

    log::info!("Loading collection");
    let root = if let Ok(path) = std::env::var("OTLBOOK_PATH") {
        PathBuf::from(path)
    } else if let Some(mut path) = dirs::home_dir() {
        path.push("otlbook");
        path
    } else {
        return Err(
            "Cannot find otlbook collection, set env var OTLBOOK_PATH",
        )?;
    };

    let mut result = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if entry.path().extension() != Some(OsStr::new("otl")) {
            continue;
        }
        if let Ok(outline) = Outline::try_from(entry.path()) {
            result.push((Some(format!("{:?}", entry.path())), outline));
        }
    }
    log::info!("Collection loaded");

    Ok(Outline::from_iter(result))
}
