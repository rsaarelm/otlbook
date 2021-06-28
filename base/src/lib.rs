mod collection;
pub use collection::Collection;

mod date;
pub use date::VagueDate;

mod outline;
pub use outline::{Outline, Section};

mod symbol;
pub use symbol::Sym;
pub type Symbol = Sym<String>;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
