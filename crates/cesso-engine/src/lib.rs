//! Search and evaluation for cesso.

pub mod eval;
pub mod search;

pub use eval::evaluate;
pub use search::{SearchResult, Searcher};
