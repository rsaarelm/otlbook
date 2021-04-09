mod anki;
pub use anki::parse_cloze;

mod date;
pub use date::VagueDate;

mod de;

mod ser;

// TODO: Deprecate
pub mod old_de;

// TODO: Deprecate
pub mod old_ser;

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
mod old_tests;
