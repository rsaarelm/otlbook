use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

pub type Section = crate::tree::NodeRef<String>;

/// IDM type for sections.
///
/// The runtime section type made of `NodeRef`s doesn't serialize cleanly on
/// its own.
#[derive(Serialize, Deserialize)]
pub(crate) struct RawSection(pub idm::Raw<String>, pub Vec<RawSection>);

impl From<&Section> for RawSection {
    fn from(sec: &Section) -> Self {
        RawSection(
            idm::Raw(sec.borrow().clone()),
            sec.children().map(|c| RawSection::from(&c)).collect(),
        )
    }
}

impl From<RawSection> for Section {
    fn from(sec: RawSection) -> Self {
        let root = Section::new(sec.0 .0);
        for s in sec.1.into_iter() {
            root.append(Section::from(s));
        }
        root
    }
}

impl Serialize for Section {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        RawSection::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Section {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let section: RawSection =
            serde::Deserialize::deserialize(deserializer)?;
        Ok(Section::from(section))
    }
}

impl Section {
    /// Return IDM string representation made from this section's body lines.
    fn body_string(&self) -> String {
        idm::to_string(&RawSection::from(self).1).expect("Shouldn't happen")
    }

    /// Try to deserialize the body of this section into given type.
    pub fn try_into<T: serde::de::DeserializeOwned>(&self) -> Option<T> {
        idm::from_str(&self.body_string()).ok()
    }

    /// Try to create `Section` from arbitrary data via IDM serialization.
    pub fn from_data<T: serde::ser::Serialize>(
        data: &T,
    ) -> crate::Result<Self> {
        Ok(idm::from_str(&idm::to_string(data)?)?)
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
        for sec in self.children() {
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
        todo!();
    }

    /// If this section matches IDM attribute syntax, return the attribute
    /// name.
    fn attribute_name(&self) -> Option<String> {
        // TODO: Use nom instead of regex hacks
        // XXX: Can this be made to use str slices for performance?
        lazy_static! {
            static ref RE: regex::Regex =
                regex::Regex::new(r"^([a-z][a-z\-0-9]*):(\s|$)").unwrap();
        }

        RE.captures(&self.headline()).map(|cs| cs[1].to_string())
    }

    /// Return section headline.
    pub fn headline(&self) -> String {
        self.borrow().clone()
    }

    /// If headline resolves to WikiWord title, return that.
    pub fn wiki_title(&self) -> Option<String> {
        // TODO: Use nom instead of regex hacks
        lazy_static! {
            static ref RE: regex::Regex =
                regex::Regex::new(r"^([A-Z][a-z]+)(([A-Z][a-z]+)|([0-9]+))+$")
                    .unwrap();
        }

        let title = self.headline();
        if RE.is_match(&title) {
            Some(title)
        } else {
            None
        }
    }

    /// If this outline is a single struct attribute, try to deserialize the
    /// value into the parameter type.
    fn deserialize_attribute<T: serde::de::DeserializeOwned>(
        &self,
    ) -> Result<T, Box<dyn std::error::Error>> {
        /// Helper struct for single-field mutations.
        #[derive(Serialize, Deserialize)]
        struct Dummy<T> {
            field: T,
        }

        // XXX: Ugly hack incoming.

        // In a dummy clone outline, rewrite the section to have "field" as
        // the field name for the single attribute.
        let mut clone = self.deep_clone();
        clone.rewrite_attribute_name("field")?;

        // Then deserialize the result into `Dummy` struct that has a "field"
        // field of the type we want.
        let deser: Dummy<T> =
            idm::from_str(&idm::to_string(&RawSection::from(&clone))?)?;

        Ok(deser.field)
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

        let new_headline: String = format!(
            "{}{}",
            name.as_ref(),
            &self.borrow()[current_name.len()..]
        );
        *self.borrow_mut() = new_headline;

        Ok(())
    }

    pub fn is_article(&self) -> bool {
        self.wiki_title().is_some() || self.has_attributes()
    }

    pub fn has_attributes(&self) -> bool {
        self.children()
            .next()
            .map(|c| c.attribute_name().is_some())
            .unwrap_or(false)
    }
}
