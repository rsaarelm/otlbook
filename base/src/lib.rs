mod collection;
pub use collection::Collection;

mod outline;
pub use outline::{Outline, Section};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
