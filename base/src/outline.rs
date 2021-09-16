use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, fmt, fs, path::Path};

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outline(pub Vec<Section>);

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section(pub idm::Raw<String>, pub Outline);

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
                writeln!(f, "{:?}", title)?;
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

impl std::iter::FromIterator<(String, Outline)> for Outline {
    fn from_iter<U: IntoIterator<Item = (String, Outline)>>(iter: U) -> Self {
        Outline(
            iter.into_iter()
                .map(|(h, b)| Section(idm::Raw(h), b))
                .collect(),
        )
    }
}

impl std::str::FromStr for Outline {
    type Err = idm::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        idm::from_str(s)
    }
}

impl TryFrom<&Path> for Outline {
    type Error = idm::Error;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        let contents = fs::read_to_string(path).map_err(|e| {
            idm::Error::new(format!("Couldn't open path {:?}", path))
        })?;
        let ret = idm::from_str::<Outline>(&contents)?;

        // XXX: Does this really belong here? Seems pretty heavyweight...
        #[cfg(debug_assertions)]
        {
            let reser = idm::to_string_styled_like(&contents, &ret).unwrap();
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
        self.iter().map(|Section(h, _)| h.as_str())
    }

    /// Try to read an attribute deserialized to type.
    ///
    /// Return error if attribute value was found but could not be
    /// deserialized to given type.
    ///
    /// Return Ok(None) if attribute was not found in outline.
    pub fn attr<T: serde::de::DeserializeOwned>(
        &self,
        name: &str,
    ) -> Result<Option<T>, Box<dyn std::error::Error>> {
        for sec in &self.0 {
            match sec.attribute_name() {
                None => {
                    // Out of attribute block when we start hitting
                    // attribute-less sections. Exit.
                    break;
                }
                Some(s) if s.as_str() == name => {
                    let ret = sec.deserialize_attribute()?;
                    return Ok(Some(ret));
                }
                _ => {}
            }
        }
        Ok(None)
    }

    /// Write a typed value to a named struct attribute.
    ///
    /// If the attribute exists in the outline, it is replaced in-place.
    /// If the attribute doesn't exist and the value is non-Default,
    /// add the attribute with the value to the end of the attribute block.
    /// If the value is T::default(), do not insert the attribute and remove
    /// it if it exists.
    pub fn set_attr<T>(
        &mut self,
        name: &str,
        value: &T,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: serde::Serialize + Default + PartialEq,
    {
        // Position in child sections where to insert new attribute
        let mut insert_pos = 0;
        // Is there an existing attribute in insert_pos to be deleted?
        let mut delete_existing = false;

        // Figure out where to insert the new item.
        for (i, sec) in self.0.iter().enumerate() {
            insert_pos = i;
            match sec.attribute_name() {
                None => {
                    // Out of attribute block when we start hitting
                    // attribute-less sections. Exit.
                    break;
                }
                Some(s) if s.as_str() == name => {
                    delete_existing = true;
                    break;
                }
                _ => {}
            }
        }

        if value == &T::default() {
            // Default value, delete attribute.
            if delete_existing {
                self.0.remove(insert_pos);
            }
        } else {
            // Non-default value, do insert.
            let attr = Section::struct_field(name, value)?;
            if delete_existing {
                self.0[insert_pos] = attr;
            } else {
                self.0.insert(insert_pos, attr);
            }
        }
        Ok(())
    }

    pub fn remove_attr(
        &mut self,
        name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Can pick an arbitrary type here. Using String.
        self.set_attr::<String>(name, &String::default())
    }

    pub fn has_attributes(&self) -> bool {
        if let Some(h) = self.0.iter().next() {
            h.attribute_name().is_some()
        } else {
            false
        }
    }
}

impl Section {
    /// Construct a section that's a struct field with the given name and
    /// value.
    pub fn struct_field<T: serde::Serialize>(
        name: &str,
        value: &T,
    ) -> Result<Section, Box<dyn std::error::Error>> {
        // Synthesize struct outline via Dummy struct.
        let ret = idm::to_string(&Dummy { field: value })?;
        let mut ret: Outline = idm::from_str(&ret)?;
        debug_assert!(ret.0.len() == 1);
        // Grab the single section we're interested in.
        let mut ret: Section = ret.pop().unwrap();
        // Rewrite dummy field name to the one we want, and done.
        ret.rewrite_attribute_name(name)?;
        Ok(ret)
    }

    pub fn title(&self) -> &str {
        // TODO: Strip TODO markings prefix, [_] 12%
        // TODO: Strip important item suffix " *"
        self.0.as_str()
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

    pub fn attribute_name(&self) -> Option<String> {
        // TODO: Use nom instead of regex hacks
        // XXX: Can this be made to use str slices for performance?
        lazy_static! {
            static ref RE: regex::Regex =
                regex::Regex::new(r"^([a-z][a-z\-0-9]*):(\s|$)").unwrap();
        }

        RE.captures(self.title()).map(|cs| cs[1].to_string())
    }

    /// Rewrite the attribute name of a section that's a struct field.
    fn rewrite_attribute_name(
        &mut self,
        name: impl AsRef<str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let current_name = match self.attribute_name() {
            Some(name) => name,
            None => {
                return Err("Section is not a struct attribute")?;
            }
        };

        self.0 = idm::Raw(format!(
            "{}{}",
            name.as_ref(),
            &self.title()[current_name.len()..]
        ));
        Ok(())
    }

    /// If this outline is a single struct attribute, try to deserialize the
    /// value into the parameter type.
    fn deserialize_attribute<T: serde::de::DeserializeOwned>(
        &self,
    ) -> Result<T, Box<dyn std::error::Error>> {
        // Ugly hack incoming.
        //
        // Rewrite the section to have "field" for the single attribute,
        // then construct a dummy outline with the rewritten section
        // to deserialize into the Dummy struct type defined below.

        let mut clone = self.clone();
        clone.rewrite_attribute_name("field")?;

        let deser: Dummy<T> =
            idm::from_str(&idm::to_string(&Outline(vec![clone]))?)?;

        Ok(deser.field)
    }

    /// Does the section look like a sub-article instead of just a random
    /// fragment.
    ///
    /// Articles aren't currently very well defined, the current heuristic is
    /// that an article either has a WikiWord title or any attributes.
    pub fn is_article(&self) -> bool {
        self.wiki_title().is_some() || self.1.has_attributes()
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

/// Helper struct for single-field mutations.
#[derive(Serialize, Deserialize)]
struct Dummy<T> {
    field: T,
}
