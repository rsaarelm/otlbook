use lazy_static::lazy_static;
use serde_derive::{Deserialize, Serialize};
use std::{convert::TryFrom, fmt, fs, path::Path};

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outline(pub Vec<Section>);

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section(pub Option<String>, pub Outline);

impl fmt::Debug for Outline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn print(
            f: &mut fmt::Formatter,
            depth: usize,
            otl: &Outline,
        ) -> fmt::Result {
            for Section(title, body) in &otl.0 {
                for _ in 0..depth {
                    write!(f, "  ")?;
                }
                match title {
                    None => writeln!(f, "ø")?,
                    Some(s) => writeln!(f, "{:?}", s)?,
                }
                print(f, depth + 1, &body)?;
            }

            Ok(())
        }

        if self.is_empty() {
            writeln!(f, "ø")
        } else {
            print(f, 0, self)
        }
    }
}

impl std::ops::Deref for Outline {
    type Target = Vec<Section>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Outline {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl std::iter::FromIterator<(Option<String>, Outline)> for Outline {
    fn from_iter<U: IntoIterator<Item = (Option<String>, Outline)>>(
        iter: U,
    ) -> Self {
        Outline(iter.into_iter().map(|(h, b)| Section(h, b)).collect())
    }
}

impl std::str::FromStr for Outline {
    type Err = idm::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        idm::from_str(s)
    }
}

impl TryFrom<&Path> for Outline {
    type Error = Box<dyn std::error::Error>;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        let contents = fs::read_to_string(path)?;
        let ret =
            idm::from_str::<Outline>(&contents).expect("shouldn't happen");

        #[cfg(debug_assertions)]
        {
            let reser = idm::to_string(&ret).unwrap();
            if reser != contents {
                use std::fs::File;
                use std::io::prelude::*;

                log::warn!("{:?} does not reserialize cleanly", path);

                let mut file = File::create(
                    Path::new("/tmp/").join(path.file_name().unwrap()),
                )
                .unwrap();
                file.write_all(reser.as_bytes()).unwrap();
            }
        }
        Ok(ret)
    }
}

impl Section {
    pub fn title(&self) -> &str {
        // TODO: Strip TODO markings prefix, [_] 12%
        // TODO: Strip important item suffix " *"
        self.0.as_ref().map(|s| s.as_ref()).unwrap_or("")
    }

    /// If headline resolves to WikiWord title, return that
    pub fn wiki_title(&self) -> Option<String> {
        // TODO: Use nom instead of regex hacks
        lazy_static! {
            static ref RE: regex::Regex =
                regex::Regex::new(r"^([A-Z][a-z]+)(([A-Z][a-z]+)|([0-9]+))+$")
                    .unwrap();
        }
        if RE.is_match(self.title()) {
            Some(self.title().to_string())
        } else {
            None
        }
    }
}

impl Outline {
    pub fn count(&self) -> usize {
        let mut ret = self.0.len();
        for Section(_, e) in &self.0 {
            ret += e.count();
        }
        ret
    }

    /// Depth-first recursive iteration of outline's child sections
    pub fn walk(&'_ self) -> OutlineWalker<'_> {
        OutlineWalker {
            current: self,
            pos: 0,
            child: None,
        }
    }

    pub fn walk_mut(&'_ mut self) -> OutlineWalkerMut<'_> {
        OutlineWalkerMut {
            current: self,
            pos: 0,
            child: None,
        }
    }

    pub fn try_into<T: serde::de::DeserializeOwned>(&self) -> Option<T> {
        let text: String = idm::to_string(self).expect("Shouldn't happen");
        idm::from_str(&text).ok()
    }

    /// Iterate non-empty headlines of outline.
    pub fn headlines(&self) -> impl Iterator<Item = &str> {
        self.iter()
            .filter_map(|Section(h, _)| h.as_ref().map(|h| h.as_str()))
    }
}

// Tree walk
// Yield section, recursively iterate inside...
// Must know to return to next sect...

pub struct OutlineWalker<'a> {
    current: &'a Outline,
    pos: usize,
    child: Option<Box<OutlineWalker<'a>>>,
}

impl<'a> Iterator for OutlineWalker<'a> {
    type Item = &'a Section;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref mut child) = self.child {
            if let Some(e) = child.next() {
                return Some(e);
            } else {
                self.child = None;
            }
        }

        if self.pos >= self.current.len() {
            return None;
        }

        let item = &self.current[self.pos];
        self.pos += 1;
        self.child = Some(Box::new(item.1.walk()));

        Some(item)
    }
}

pub struct OutlineWalkerMut<'a> {
    current: &'a mut Outline,
    pos: usize,
    child: Option<Box<OutlineWalkerMut<'a>>>,
}

impl<'a> Iterator for OutlineWalkerMut<'a> {
    type Item = &'a mut Section;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref mut child) = self.child {
            if let Some(e) = child.next() {
                return Some(e);
            } else {
                self.child = None;
            }
        }

        if self.pos >= self.current.len() {
            return None;
        }

        // haha unsafe &mut Iterator go brrr
        unsafe {
            let item = self.current.as_mut_ptr().add(self.pos);
            self.pos += 1;
            self.child = Some(Box::new((*item).1.walk_mut()));
            Some(&mut *item)
        }
    }
}
