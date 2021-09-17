use crate::{Outline, Result, Section};
use rayon::prelude::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryFrom,
    ffi::OsStr,
    fs,
    iter::FromIterator,
    path::PathBuf,
};
use walkdir::WalkDir;

/// Representation of a collection of otl files that makes up the knowledge
/// base.
pub struct Collection {
    /// Path the collection was loaded from.
    path: PathBuf,
    /// State of collection when loaded.
    loaded: Outline,
    /// Current state of in-memory collection.
    current: Outline,
}

impl Collection {
    pub fn new() -> Result<Collection> {
        log::info!("load_collection: Determining collection path");
        let path = if let Ok(path) = std::env::var("OTLBOOK_PATH") {
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
        let files: Vec<_> = WalkDir::new(path.clone())
            .into_iter()
            .filter_map(|e| e.map(|e| e.path().to_path_buf()).ok())
            .filter(|e| e.extension() == Some(otl_extension))
            .collect();

        log::info!("load_collection: Loading {} .otl files", files.len());

        // Collect into BTreeMap so we automagically get the toplevel sorted
        // lexically by filenames.
        let mut sections = BTreeMap::new();

        for (name, outline) in files
            .into_par_iter()
            .map(|p| {
                // Path names in outline must have the base path stripped out.
                (
                    format!(
                        "{}",
                        p.strip_prefix(&path).unwrap().to_str().unwrap()
                    ),
                    Outline::try_from(p.as_ref()).map_err(|e| {
                        e.file_name(format!("{}", p.to_string_lossy()))
                    }),
                )
            })
            .collect::<Vec<_>>()
            .into_iter()
        {
            sections.insert(name, outline?);
        }

        log::info!("load_collection: Merging loaded outlines");

        let loaded = Outline::from_iter(sections);
        let current = loaded.clone();

        Ok(Collection {
            path,
            loaded,
            current,
        })
    }

    /// Get the in-memory collection outline.
    pub fn outline(&self) -> &Outline {
        &self.current
    }

    /// Get the mutable in-memory collection outline.
    pub fn outline_mut(&mut self) -> &mut Outline {
        &mut self.current
    }

    /// Save changes after creating the collection or the previous save to
    /// disk to path where the collection was loaded from.
    pub fn save(&mut self) -> Result<()> {
        // Check for validity
        {
            // All toplevel items must define a filename.
            let mut headlines = self
                .current
                .iter()
                .map(|Section(h, _)| h)
                .collect::<Vec<_>>();

            // All toplevel filenames must be unique.
            headlines.sort();
            let len1 = headlines.len();
            headlines.dedup();
            if headlines.len() < len1 {
                panic!("Collection::save: Repeated file name in collection");
            }
        }

        let current = self
            .current
            .iter()
            .map(|Section(h, b)| (PathBuf::from(h.as_str()), b))
            .collect::<BTreeMap<PathBuf, &Outline>>();

        let loaded = self
            .loaded
            .iter()
            .map(|Section(h, b)| (PathBuf::from(h.as_str()), b))
            .collect::<BTreeMap<PathBuf, &Outline>>();

        // Remove files that were deleted from current set.
        let deleted_files = loaded
            .keys()
            .collect::<BTreeSet<_>>()
            .difference(&current.keys().collect::<BTreeSet<_>>())
            .cloned()
            .collect::<BTreeSet<_>>();

        for f in &deleted_files {
            let mut path = self.path.clone();
            path.push(f);
            log::info!("Deleting removed file {:?}", path);
            fs::remove_file(path)?;
        }

        for f in current.keys() {
            let o = &current[f];
            if let Some(o2) = loaded.get(f) {
                if o2 == o {
                    log::debug!("Nothing changed for {:?}", f);
                    continue;
                }
            }

            let mut path = self.path.clone();
            path.push(f);
            log::info!("File {:?} changed, saving to {:?}", f, path);

            fs::write(
                path,
                idm::to_string(o).expect("Failed to serialize outline"),
            )?;
        }

        Ok(())
    }
}
