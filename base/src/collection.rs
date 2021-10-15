use crate::{section::RawSection, Result, Section};
use rayon::prelude::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

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
    style: idm::Style,
}

impl File {
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let outline = self
            .section
            .children()
            .map(|n| RawSection::from(&n))
            .collect::<Vec<_>>();

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
fn load_section(
    root_path: impl AsRef<Path>,
    path: impl Into<PathBuf>,
) -> Result<(idm::Style, String, Vec<RawSection>)> {
    let path = path.into();
    log::info!("load_section from {:?}", path);
    let headline = path
        .strip_prefix(root_path.as_ref())
        .unwrap()
        .to_string_lossy()
        .to_string();

    let contents = fs::read_to_string(path)?;
    // NB. Currently using tabs as the default otlbook style to go with
    // VimOutliner conventions. This should be made customizable somewhere
    // eventually.
    let style = idm::infer_indent_style(&contents).unwrap_or(idm::Style::Tabs);

    Ok((
        style,
        headline,
        idm::from_str::<Vec<RawSection>>(&contents)?,
    ))
}

fn build_section(headline: String, mut body: Vec<RawSection>) -> Section {
    // XXX: Reverse-prepend optimization to get around nodes having
    // inefficient append. Nicer approach would be to fix tree node to track
    // last child pointer and have O(1) append op.
    body.reverse();

    let ret = Section::new(headline);
    for child in body {
        ret.prepend(child.into());
    }
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
            .map(|p| (p.clone(), load_section(&root_path, p)))
            .collect::<Vec<_>>()
            .into_iter()
        {
            let (style, headline, raw_section) = res?;
            let section = build_section(headline, raw_section);

            let path = path.strip_prefix(&root_path).unwrap().to_owned();
            files.insert(path.clone(), File { style, section });
            seen_paths.insert(path);
        }

        println!(
            "{:?} {:?}",
            seen_paths,
            files.iter().map(|(k, _)| k).collect::<Vec<_>>()
        );

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
}
