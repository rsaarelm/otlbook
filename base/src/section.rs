use crate::parse::{self, only};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Optional identifier for sections.
///
/// Sections that have an `:uri` field use the uri string as identifier. If
/// there is no uri, but the section title is formatted as a WikiWord, the
/// title WikiWord is used. If a section has neither, it does not have an
/// entity identifier and is not considered identical to any other section.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum EntityIdentifier {
    WikiTitle(String),
    Uri(String),
}

#[derive(Clone, Default)]
pub struct SectionData {
    pub headline: String,
    pub attributes: IndexMap<String, String>,
}

impl SectionData {
    pub fn new(headline: String, attributes: IndexMap<String, String>) -> Self {
        SectionData {
            headline,
            attributes,
        }
    }
}

impl From<String> for SectionData {
    fn from(s: String) -> Self {
        SectionData::new(s, Default::default())
    }
}

// Headline and attributes.
pub type Section = crate::tree::NodeRef<SectionData>;

#[derive(Serialize, Deserialize)]
pub(crate) struct RawOutline(
    pub(crate) (IndexMap<String, String>,),
    pub(crate) Vec<RawSection>,
);

/// IDM type for sections.
///
/// The runtime section type made of `NodeRef`s doesn't serialize cleanly on
/// its own.
#[derive(Serialize, Deserialize)]
pub(crate) struct RawSection((String,), RawOutline);

impl RawSection {
    pub fn outline(self) -> RawOutline {
        self.1
    }
}

impl From<&Section> for RawSection {
    fn from(sec: &Section) -> Self {
        RawSection(
            (sec.borrow().headline.clone(),),
            RawOutline(
                (sec.borrow().attributes.clone(),),
                sec.children().map(|c| RawSection::from(&c)).collect(),
            ),
        )
    }
}

impl From<RawSection> for Section {
    fn from(sec: RawSection) -> Self {
        let root = Section::new(SectionData::new(sec.0 .0, sec.1 .0 .0));
        for s in sec.1 .1.into_iter() {
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
    pub fn body_string(&self) -> String {
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
        self.borrow()
            .attributes
            .get(name)
            .map(|s| idm::from_str(s).map_err(Into::into))
            .transpose()
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
        self.borrow_mut()
            .attributes
            .insert(name.to_string(), idm::to_string(value)?);
        Ok(())
    }

    /// Return section headline.
    pub fn headline(&self) -> String {
        self.borrow().headline.clone()
    }

    /// Extract the title part of the headline
    ///
    /// This omits the important item tag and any todo boxes
    pub fn title(&self) -> String {
        let section = self.borrow();
        if let Ok((_, (_, title, _))) = parse::title(&section.headline) {
            title.to_string()
        } else {
            Default::default()
        }
    }

    pub fn set_title(&mut self, new_title: impl Into<String>) {
        let is_important = self.is_important();
        let mut title: String = new_title.into();

        if is_important {
            title.push_str(" *");
        }

        self.borrow_mut().headline = title;
    }

    pub fn is_important(&self) -> bool {
        self.borrow().headline.ends_with(" *")
    }

    /// If headline resolves to WikiWord title, return that.
    pub fn wiki_title(&self) -> Option<String> {
        if let Ok(wiki_word) = only(parse::wiki_word)(&self.title()) {
            Some(wiki_word.to_string())
        } else {
            None
        }
    }

    pub fn is_article(&self) -> bool {
        self.wiki_title().is_some() || self.has_attributes()
    }

    pub fn entity_identifier(&self) -> Option<EntityIdentifier> {
        if let Ok(Some(uri)) = self.attr("uri") {
            Some(EntityIdentifier::Uri(uri))
        } else if let Some(wiki_title) = self.wiki_title() {
            Some(EntityIdentifier::WikiTitle(wiki_title))
        } else {
            None
        }
    }

    pub fn has_attributes(&self) -> bool {
        !self.borrow().attributes.is_empty()
    }
}
