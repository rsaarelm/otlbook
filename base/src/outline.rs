use serde_derive::{Deserialize, Serialize};
use std::{convert::TryFrom, fs, path::Path};

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outline(pub Vec<Section>);
pub type Section = (Option<String>, Outline);

impl std::ops::Deref for Outline {
    type Target = Vec<Section>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::iter::FromIterator<Section> for Outline {
    fn from_iter<U: IntoIterator<Item = Section>>(iter: U) -> Self {
        Outline(iter.into_iter().collect())
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
        idm::from_str::<Outline>(&contents).expect("shouldn't happen");
        Ok(idm::from_str(&contents)?)
    }
}

impl Outline {
    pub fn count(&self) -> usize {
        let mut ret = self.0.len();
        for (_, e) in &self.0 {
            ret += e.count();
        }
        ret
    }

    /// Depth-first recursive iteration of outline's child sections
    pub fn iter(&'_ self) -> OutlineIter<'_> {
        OutlineIter {
            current: self,
            pos: 0,
            child: None,
        }
    }

    pub fn try_into<T: serde::de::DeserializeOwned>(&self) -> Option<T> {
        let text: String = idm::to_string(self).expect("Shouldn't happen");
        idm::from_str(&text).ok()
    }
}

// Tree walk
// Yield section, recursively iterate inside...
// Must know to return to next sect...

pub struct OutlineIter<'a> {
    current: &'a Outline,
    pos: usize,
    child: Option<Box<OutlineIter<'a>>>,
}

impl<'a> Iterator for OutlineIter<'a> {
    type Item = &'a (Option<String>, Outline);

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
        self.child = Some(Box::new(item.1.iter()));

        Some(item)
    }
}
