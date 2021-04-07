mod anki;
pub use anki::parse_cloze;

mod date;
pub use date::VagueDate;

mod de;
pub use de::from_outline;

mod ser;
pub use ser::into_outline;

// TODO: Deprecate
pub mod old_outline;

mod outline;
pub use outline::Outline;

mod outline2;
pub use outline2::Outline2;

mod symbol;
pub use symbol::Sym;

pub type Symbol = Sym<String>;

mod util;
pub use util::normalize_title;

#[cfg(test)]
mod tests;
