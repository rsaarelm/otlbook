use crate::{Uri, VagueDate};

// TODO: Make this a separate type, needs custom HtmlFmt handling.
pub type Tag = crate::Symbol;

// TODO: Separate type
// TODO: This type can be a http URL or a local WikiWord.
pub type Link = String;

// TODO: Parsing
pub type Http = String;

/// Enumeration of established typed attributes.
pub enum TypedAttribute {
    Uri(Uri),
    Tags(Vec<Tag>),
    Via(Vec<Link>),
    Added(VagueDate),
    Links(Vec<Http>),
}
