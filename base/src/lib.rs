use std::{convert::TryFrom, ffi::OsStr, iter::FromIterator, path::PathBuf};

mod outline;
pub use outline::{Outline, Section};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn load_collection() -> Result<Outline> {
    use rayon::prelude::*;
    use walkdir::WalkDir;
    use std::collections::BTreeMap;

    log::info!("load_collection: Determining collection path");
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

    log::info!("load_collection: Collecting .otl files");

    let otl_extension = OsStr::new("otl");
    let files: Vec<_> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.map(|e| e.path().to_path_buf()).ok())
        .filter(|e| e.extension() == Some(otl_extension))
        .collect();

    log::info!("load_collection: Loading {} .otl files", files.len());
    // Collect into BTreeMap so we automagically get the toplevel sorted
    // lexically by filenames.
    let sections: BTreeMap<_, _> = files
        .into_par_iter()
        .map(|p| {
            (
                Some(format!("{:?}", p)),
                Outline::try_from(p.as_ref())
                    .expect("load_collection: Failed to parse outline"),
            )
        })
        .collect();
    log::info!("load_collection: Merging loaded outlines");
    Ok(Outline::from_iter(sections))
}
