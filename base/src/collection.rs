use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use idm::ser::Indentation;
use rayon::prelude::*;
use walkdir::WalkDir;

use crate::{
    section::{RawOutline, RawSection, SectionData},
    Result, Section,
};

/// Representation of a collection of otl files that makes up the knowledge
/// base.
pub struct Collection {
    /// Path the collection was loaded from.
    root_path: PathBuf,

    /// Last seen set of paths, used to determine if files need to be created
    /// or deleted when saving the collection.
    previous_paths: BTreeSet<PathBuf>,
    files: BTreeMap<PathBuf, File>,
}

/// Metadata and contents for a single file in the collection.
struct File {
    section: Section,
    style: Indentation,
}

impl File {
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let outline = RawSection::from(&self.section).outline();

        fs::write(
            path,
            idm::to_string_styled(self.style, &outline)
                .expect("Failed to serialize outline"),
        )?;
        Ok(())
    }
}

/// Load file into raw sections.
///
/// Return path converted into headline as well.
///
/// Parallelizable helper function for file. Currently tree nodes can't be
/// parallelized.
fn load_outline(
    root_path: impl AsRef<Path>,
    path: impl Into<PathBuf>,
) -> Result<(Indentation, String, RawOutline)> {
    let path = path.into();
    log::debug!("load_outline from {:?}", path);
    let headline = path
        .strip_prefix(root_path.as_ref())
        .unwrap()
        .with_extension("") // Strip out the ".otl"
        .to_string_lossy()
        .to_string();

    let contents = fs::read_to_string(path.clone())?;
    // NB. Currently using tabs as the default otlbook style to go with
    // VimOutliner conventions. This should be made customizable somewhere
    // eventually.
    let style = Indentation::infer(&contents).unwrap_or(Indentation::Tabs);

    Ok((
        style,
        headline,
        // FIXME: Remove the final .to_string() when IDM is updated to version with more generic file name setter.
        idm::from_str::<RawOutline>(&contents).map_err(|e| {
            e.with_file_name(path.to_string_lossy().to_string())
        })?,
    ))
}

fn build_section(headline: String, outline: RawOutline) -> Section {
    let RawOutline((attributes,), mut body) = outline;

    // XXX: Reverse-prepend optimization to get around nodes having
    // inefficient append. Nicer approach would be to fix tree node to track
    // last child pointer and have O(1) append op.
    body.reverse();

    let ret = Section::from(SectionData::new(headline, attributes));
    for child in body {
        ret.prepend(child.into());
    }
    ret.cleanse();
    ret
}

impl Collection {
    pub fn load() -> Result<Collection> {
        log::info!("Collection::load: Determining collection path");
        let root_path = if let Ok(path) = std::env::var("OTLBOOK_PATH") {
            PathBuf::from(path)
        } else if let Some(mut path) = dirs::home_dir() {
            path.push("otlbook");
            path
        } else {
            return Err(
                "Cannot find otlbook collection, set env var OTLBOOK_PATH",
            )?;
        };

        log::info!("Collection::load: Collecting .otl files");

        let otl_extension = OsStr::new("otl");
        let file_paths: Vec<_> = WalkDir::new(root_path.clone())
            .into_iter()
            .filter_map(|e| e.map(|e| e.path().to_path_buf()).ok())
            .filter(|e| e.extension() == Some(otl_extension))
            .collect();

        log::info!("Collection::load: Loading {} .otl files", file_paths.len());

        let mut files = BTreeMap::new();
        let mut seen_paths = BTreeSet::new();

        // Load outlines in parallel with rayon.
        for (path, res) in file_paths
            .par_iter()
            .map(|p| (p.clone(), load_outline(&root_path, p)))
            .collect::<Vec<_>>()
            .into_iter()
        {
            let (style, headline, raw_outline) = res?;
            let section = build_section(headline, raw_outline);

            let path = path.strip_prefix(&root_path).unwrap().to_owned();
            files.insert(path.clone(), File { style, section });
            seen_paths.insert(path);
        }

        Ok(Collection {
            root_path,
            previous_paths: seen_paths,
            files,
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = Section> {
        // Construct a mutant iterator that has no current next item but the
        // roots of all the file sections as pending items.
        crate::tree::BreadthFirstNodes {
            next: None,
            pending: self
                .files
                .iter()
                .map(|(_, file)| file.section.clone())
                .collect(),
        }
    }

    pub fn roots(&self) -> impl Iterator<Item = Section> + '_ {
        self.files.iter().map(|(_, file)| file.section.clone())
    }

    /// Save changes after creating the collection or the previous save to
    /// disk to path where the collection was loaded from.
    pub fn save(&mut self) -> Result<()> {
        log::info!("Collection::save started");
        let abs = |relative_path: &PathBuf| self.root_path.join(relative_path);

        let current_paths = self
            .files
            .iter()
            .map(|(p, _)| p)
            .cloned()
            .collect::<BTreeSet<_>>();

        // Delete files that were removed from current set.
        for deleted in self.previous_paths.difference(&current_paths) {
            let path = abs(deleted);
            log::info!("Collection::save deleting removed file {:?}", path);
            fs::remove_file(path)?;
        }

        for (path, file) in self.files.iter() {
            let do_write = if !self.previous_paths.contains(path) {
                log::info!("Collection::save creating new file {:?}", path);
                true
            } else if file.section.is_dirty() {
                log::info!("Collection::save writing changed file {:?}", path);
                true
            } else {
                false
            };

            if do_write {
                file.save(abs(path))?;
                file.section.cleanse();
            }
        }

        self.previous_paths = current_paths;
        Ok(())
    }

    /// Return a node with the given title.
    ///
    /// If the node isn't found in the collection, create a new toplevel item
    /// with the title.
    pub fn find_or_create(&mut self, path: &str) -> Result<Section> {
        // TODO: Replace Node with a smart pointer that supports toplevel
        // detachment.

        let elts: Vec<_> = path.split('/').collect();

        if elts.is_empty() {
            return Err("find_or_create: Bad path")?;
        }

        let mut root = None;

        // Look for existing section.
        // XXX: Ineffective O(n) lookup.
        for node in self.iter() {
            if &node.headline() == elts[0] {
                root = Some(node);
                break;
            }
        }

        let mut node = if let Some(root) = root {
            root
        } else {
            log::info!("Section {:?} not found, creating toplevel item", path);

            let headline = format!("{}.otl", path);
            let section = Section::from(SectionData::new(
                headline.clone(),
                Default::default(),
            ));
            self.files.insert(
                headline.into(),
                File {
                    section: section.clone(),
                    style: Indentation::Tabs,
                },
            );
            section
        };

        'path: for headline in elts[1..].iter() {
            for c in node.children() {
                if &c.headline() == headline {
                    node = c;
                    continue 'path;
                }
            }

            log::info!("Section {:?} not found, appending to node", headline);
            let child = Section::new(headline.to_string(), Default::default());
            node.append(child.clone());
            node = child;
        }

        Ok(node)
    }
}
