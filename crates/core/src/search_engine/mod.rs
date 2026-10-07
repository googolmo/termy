//! Terminal search utilities for Termy.

mod engine;
mod mapping;
mod matcher;
mod state;

pub use engine::{SearchConfig, SearchEngine, SearchMode};
pub use mapping::SearchLineMapping;
pub use matcher::{SearchMatch, SearchResults};
pub use state::SearchState;
