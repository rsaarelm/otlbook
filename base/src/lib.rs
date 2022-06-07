mod collection;
pub use collection::Collection;

mod date;
pub use date::VagueDate;

pub mod parse;

mod section;
pub use section::Section;

mod symbol;
pub use symbol::{Sym, Uri};
pub type Symbol = Sym<String>;

pub type Result<T> =
    std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

mod tree;

mod typed_attribute;
